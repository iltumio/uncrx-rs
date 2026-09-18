use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{
    env, fs,
    io::{self, Stdout},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};
use uncrx_rs::extract::{extract_crx_file_cancellable, ExtractionOptions};

#[derive(Debug, Clone)]
enum AppState {
    FileBrowser,
    Processing,
    Success(String),
    Error(String),
}

#[derive(Debug, Clone)]
enum FileSystemItem {
    Directory(PathBuf),
    CrxFile(PathBuf),
    ParentDirectory,
}

impl FileSystemItem {
    fn name(&self) -> String {
        match self {
            FileSystemItem::Directory(path) => {
                format!(
                    "📁 {}/",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                )
            }
            FileSystemItem::CrxFile(path) => {
                format!(
                    "📄 {}",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                )
            }
            FileSystemItem::ParentDirectory => "📁 ../".to_string(),
        }
    }
}

#[derive(Debug)]
struct App {
    state: AppState,
    items: Vec<FileSystemItem>,
    selected_item: ListState,
    current_dir: PathBuf,
    output_dir: PathBuf,
    options: ExtractionOptions,
    worker: Option<JoinHandle<Result<PathBuf>>>,
    cancelled: Arc<AtomicBool>,
}

impl App {
    fn new(current_dir: PathBuf, output_dir: PathBuf, options: ExtractionOptions) -> Result<App> {
        let mut app = App {
            state: AppState::FileBrowser,
            items: Vec::new(),
            selected_item: ListState::default(),
            current_dir: current_dir.clone(),
            output_dir,
            options,
            worker: None,
            cancelled: Arc::new(AtomicBool::new(false)),
        };

        app.refresh_items()?;
        if !app.items.is_empty() {
            app.selected_item.select(Some(0));
        }

        Ok(app)
    }

    fn refresh_items(&mut self) -> Result<()> {
        self.items.clear();

        // Add parent directory entry if not at root
        if self.current_dir.parent().is_some() {
            self.items.push(FileSystemItem::ParentDirectory);
        }

        let mut directories = Vec::new();
        let mut crx_files = Vec::new();

        for entry in fs::read_dir(&self.current_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                directories.push(FileSystemItem::Directory(path));
            } else if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "crx" {
                        crx_files.push(FileSystemItem::CrxFile(path));
                    }
                }
            }
        }

        // Sort directories and files separately
        directories.sort_by(|a, b| match (a, b) {
            (FileSystemItem::Directory(a), FileSystemItem::Directory(b)) => {
                a.file_name().cmp(&b.file_name())
            }
            _ => std::cmp::Ordering::Equal,
        });

        crx_files.sort_by(|a, b| match (a, b) {
            (FileSystemItem::CrxFile(a), FileSystemItem::CrxFile(b)) => {
                a.file_name().cmp(&b.file_name())
            }
            _ => std::cmp::Ordering::Equal,
        });

        // Add directories first, then files
        self.items.extend(directories);
        self.items.extend(crx_files);

        Ok(())
    }

    fn next_item(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let i = match self.selected_item.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.selected_item.select(Some(i));
    }

    fn previous_item(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let i = match self.selected_item.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.selected_item.select(Some(i));
    }

    fn handle_enter(&mut self) -> Result<()> {
        if let Some(selected) = self.selected_item.selected() {
            if selected < self.items.len() {
                match &self.items[selected] {
                    FileSystemItem::Directory(path) => {
                        self.current_dir = path.clone();
                        self.refresh_items()?;
                        self.selected_item.select(Some(0));
                    }
                    FileSystemItem::CrxFile(path) => {
                        self.start_extraction(path.clone())?;
                    }
                    FileSystemItem::ParentDirectory => {
                        if let Some(parent) = self.current_dir.parent() {
                            self.current_dir = parent.to_path_buf();
                            self.refresh_items()?;
                            self.selected_item.select(Some(0));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn start_extraction(&mut self, path: PathBuf) -> Result<()> {
        let name = path.file_stem().context("Missing filename")?;
        anyhow::ensure!(
            name != "." && name != "..",
            "Invalid extraction directory name"
        );
        let destination = self.output_dir.join(name);
        let options = self.options.clone();
        self.cancelled.store(false, Ordering::Relaxed);
        let cancelled = Arc::clone(&self.cancelled);
        self.worker = Some(
            std::thread::Builder::new()
                .name("crx-extract".into())
                .spawn(move || {
                    extract_crx_file_cancellable(&path, &destination, &options, &cancelled)?;
                    Ok(destination)
                })?,
        );
        self.state = AppState::Processing;
        Ok(())
    }

    fn poll_worker(&mut self) {
        if !self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            return;
        }
        if let Some(worker) = self.worker.take() {
            self.state = match worker.join() {
                Ok(Ok(path)) => AppState::Success(path.display().to_string()),
                Ok(Err(error)) => AppState::Error(format!("{error:#}")),
                Err(_) => AppState::Error("Extraction worker panicked".into()),
            };
        }
    }

    fn reset_to_browser(&mut self) {
        self.state = AppState::FileBrowser;
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Each restoration is attempted even if another one fails.
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            crossterm::cursor::Show
        );
    }
}

pub fn run_tui(output_dir: PathBuf, options: ExtractionOptions) -> Result<()> {
    let current_dir = env::current_dir()?;
    let mut app = App::new(current_dir.clone(), current_dir.join(output_dir), options)?;
    // Install before setup so partial initialization and unwinding also restore.
    let _guard = TerminalGuard;
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    run_app(&mut terminal, &mut app)
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        app.poll_worker();
        terminal.draw(|f| ui(f, app))?;

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match &app.state {
                    AppState::FileBrowser => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Down | KeyCode::Char('j') => app.next_item(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous_item(),
                        KeyCode::Enter => {
                            app.handle_enter()?;
                        }
                        KeyCode::Char('r') => {
                            app.refresh_items()?;
                            if !app.items.is_empty() {
                                app.selected_item.select(Some(0));
                            }
                        }
                        _ => {}
                    },
                    AppState::Processing => {
                        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                            return Ok(());
                        }
                    }
                    AppState::Success(_) | AppState::Error(_) => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Enter | KeyCode::Char(' ') => app.reset_to_browser(),
                        _ => {}
                    },
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new("UnCRX-RS Terminal UI")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Footer with instructions
    let instructions = match &app.state {
        AppState::FileBrowser => "↑/↓: Navigate | Enter: Open/Extract | R: Refresh | Q/Esc: Quit",
        AppState::Processing => "Processing... | Q/Esc: Cancel and quit",
        AppState::Success(_) | AppState::Error(_) => {
            "Enter/Space: Back to file browser | Q/Esc: Quit"
        }
    };

    let footer = Paragraph::new(instructions)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);

    // Main content
    match &app.state {
        AppState::FileBrowser => {
            render_file_browser(f, chunks[1], app);
        }
        AppState::Processing => {
            render_processing(f, chunks[1]);
        }
        AppState::Success(output_path) => {
            render_success(f, chunks[1], output_path);
        }
        AppState::Error(error_msg) => {
            render_error(f, chunks[1], error_msg);
        }
    }
}

fn render_file_browser(f: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    let current_dir_display = app.current_dir.to_string_lossy();
    let title = format!("File Browser - {}", current_dir_display);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default());

    if app.items.is_empty() {
        let no_items = Paragraph::new("No directories or CRX files found in current directory")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(block);
        f.render_widget(no_items, area);
    } else {
        let items: Vec<ListItem> = app
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let display_name = item.name();

                let style = if Some(i) == app.selected_item.selected() {
                    Style::default().fg(Color::Black).bg(Color::White)
                } else {
                    match item {
                        FileSystemItem::Directory(_) | FileSystemItem::ParentDirectory => {
                            Style::default().fg(Color::Blue)
                        }
                        FileSystemItem::CrxFile(_) => Style::default().fg(Color::Green),
                    }
                };

                ListItem::new(Line::from(Span::styled(display_name, style)))
            })
            .collect();

        let items_list = List::new(items)
            .block(block)
            .highlight_style(Style::default().fg(Color::Black).bg(Color::White));

        f.render_stateful_widget(items_list, area, &mut app.selected_item);
    }
}

fn render_processing(f: &mut Frame, area: ratatui::layout::Rect) {
    let processing = Paragraph::new("Extracting CRX file contents...\n\nPlease wait...")
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(Block::default().title("Processing").borders(Borders::ALL));

    f.render_widget(processing, area);
}

fn render_success(f: &mut Frame, area: ratatui::layout::Rect, output_path: &str) {
    let success_msg = format!(
        "✓ Extraction successful!\n\nExtracted to: {}\n\nPress Enter or Space to continue",
        output_path
    );

    let success = Paragraph::new(success_msg)
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(Block::default().title("Success").borders(Borders::ALL));

    f.render_widget(success, area);
}

fn render_error(f: &mut Frame, area: ratatui::layout::Rect, error_msg: &str) {
    let error_text = format!(
        "✗ Error occurred during extraction:\n\n{}\n\nPress Enter or Space to continue",
        error_msg
    );

    let error = Paragraph::new(error_text)
        .style(Style::default().fg(Color::Red))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(Block::default().title("Error").borders(Borders::ALL));

    f.render_widget(error, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use std::time::Instant;

    #[test]
    fn browser_runs_real_worker_and_renders_processing_and_result() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("fixture.crx");
        fs::write(&input, include_bytes!("mock/test-extension.crx")).unwrap();
        let mut app = App::new(
            root.path().to_path_buf(),
            root.path().join("out"),
            ExtractionOptions::default(),
        )
        .unwrap();
        let index = app
            .items
            .iter()
            .position(|item| matches!(item, FileSystemItem::CrxFile(_)))
            .unwrap();
        app.selected_item.select(Some(index));
        app.handle_enter().unwrap();
        assert!(matches!(app.state, AppState::Processing));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| ui(f, &mut app)).unwrap();
        let start = Instant::now();
        while matches!(app.state, AppState::Processing) {
            assert!(start.elapsed() < Duration::from_secs(10));
            app.poll_worker();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(matches!(app.state, AppState::Success(_)), "{:?}", app.state);
        assert!(root.path().join("out/fixture/manifest.json").exists());
        // Same real handler must refuse overwriting an existing extraction.
        app.reset_to_browser();
        app.handle_enter().unwrap();
        while matches!(app.state, AppState::Processing) {
            assert!(start.elapsed() < Duration::from_secs(10));
            app.poll_worker();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            matches!(app.state, AppState::Error(ref message) if message.contains("already exists"))
        );
    }

    #[test]
    fn dropping_app_cancels_and_joins_worker() {
        let root = tempfile::tempdir().unwrap();
        let mut app = App::new(
            root.path().to_path_buf(),
            root.path().join("out"),
            ExtractionOptions::default(),
        )
        .unwrap();
        let flag = Arc::clone(&app.cancelled);
        let worker_flag = Arc::clone(&flag);
        app.worker = Some(std::thread::spawn(move || {
            while !worker_flag.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
            anyhow::bail!("cancelled")
        }));
        drop(app);
        assert!(flag.load(Ordering::Relaxed));
    }
}
