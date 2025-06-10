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
    path::{Path, PathBuf},
};
use uncrx_rs::uncrx::helpers::parse_crx;
use zip::ZipArchive;

fn extract_zip_to_directory(
    zip_data: &[u8],
    extract_to: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let cursor = std::io::Cursor::new(zip_data);
    let mut archive = ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => extract_to.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            // Directory
            fs::create_dir_all(&outpath)?;
        } else {
            // File
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

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
}

impl App {
    fn new() -> Result<App, Box<dyn std::error::Error>> {
        let current_dir = env::current_dir()?;
        let output_dir = current_dir.join("out");

        let mut app = App {
            state: AppState::FileBrowser,
            items: Vec::new(),
            selected_item: ListState::default(),
            current_dir: current_dir.clone(),
            output_dir,
        };

        app.refresh_items()?;
        if !app.items.is_empty() {
            app.selected_item.select(Some(0));
        }

        Ok(app)
    }

    fn refresh_items(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

    fn handle_enter(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(selected) = self.selected_item.selected() {
            if selected < self.items.len() {
                match &self.items[selected] {
                    FileSystemItem::Directory(path) => {
                        self.current_dir = path.clone();
                        self.refresh_items()?;
                        self.selected_item.select(Some(0));
                    }
                    FileSystemItem::CrxFile(path) => {
                        self.state = AppState::Processing;
                        match self.convert_crx_file(path) {
                            Ok(output_path) => {
                                self.state = AppState::Success(output_path);
                            }
                            Err(e) => {
                                self.state = AppState::Error(e.to_string());
                            }
                        }
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

    fn convert_crx_file(&self, crx_path: &PathBuf) -> Result<String, Box<dyn std::error::Error>> {
        let data = fs::read(crx_path)?;
        let extension = parse_crx(&data)?;

        if !self.output_dir.exists() {
            fs::create_dir_all(&self.output_dir)?;
        }

        let file_name = crx_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("extension");

        let extract_dir = self.output_dir.join(file_name);

        if extract_dir.exists() {
            fs::remove_dir_all(&extract_dir)?;
        }

        fs::create_dir_all(&extract_dir)?;

        // Extract zip contents to the directory
        extract_zip_to_directory(&extension.zip, &extract_dir)?;

        Ok(extract_dir.to_string_lossy().to_string())
    }

    fn reset_to_browser(&mut self) {
        self.state = AppState::FileBrowser;
    }
}

pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let mut app = App::new()?;
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| ui(f, app))?;

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
                        // Wait for processing to complete
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

fn ui(f: &mut Frame, app: &App) {
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
        AppState::Processing => "Processing...",
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

fn render_file_browser(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
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

        f.render_stateful_widget(items_list, area, &mut app.selected_item.clone());
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
