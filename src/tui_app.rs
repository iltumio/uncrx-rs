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
};
use uncrx_rs::uncrx::helpers::parse_crx;

#[derive(Debug, Clone)]
enum AppState {
    FileBrowser,
    Processing,
    Success(String),
    Error(String),
}

#[derive(Debug)]
struct App {
    state: AppState,
    files: Vec<PathBuf>,
    selected_file: ListState,
    current_dir: PathBuf,
    output_dir: PathBuf,
}

impl App {
    fn new() -> Result<App, Box<dyn std::error::Error>> {
        let current_dir = env::current_dir()?;
        let output_dir = current_dir.join("out");

        let mut app = App {
            state: AppState::FileBrowser,
            files: Vec::new(),
            selected_file: ListState::default(),
            current_dir: current_dir.clone(),
            output_dir,
        };

        app.refresh_files()?;
        if !app.files.is_empty() {
            app.selected_file.select(Some(0));
        }

        Ok(app)
    }

    fn refresh_files(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.files.clear();

        for entry in fs::read_dir(&self.current_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "crx" {
                        self.files.push(path);
                    }
                }
            }
        }

        self.files.sort();
        Ok(())
    }

    fn next_file(&mut self) {
        if self.files.is_empty() {
            return;
        }

        let i = match self.selected_file.selected() {
            Some(i) => {
                if i >= self.files.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.selected_file.select(Some(i));
    }

    fn previous_file(&mut self) {
        if self.files.is_empty() {
            return;
        }

        let i = match self.selected_file.selected() {
            Some(i) => {
                if i == 0 {
                    self.files.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.selected_file.select(Some(i));
    }

    fn convert_selected_file(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(selected) = self.selected_file.selected() {
            if selected < self.files.len() {
                let file_path = &self.files[selected];
                self.state = AppState::Processing;

                match self.convert_crx_file(file_path) {
                    Ok(output_path) => {
                        self.state = AppState::Success(output_path);
                    }
                    Err(e) => {
                        self.state = AppState::Error(e.to_string());
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

        let output_file = self.output_dir.join(format!("{}.zip", file_name));
        fs::write(&output_file, &extension.zip)?;

        Ok(output_file.to_string_lossy().to_string())
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
                        KeyCode::Down | KeyCode::Char('j') => app.next_file(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous_file(),
                        KeyCode::Enter => {
                            app.convert_selected_file()?;
                        }
                        KeyCode::Char('r') => {
                            app.refresh_files()?;
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
        AppState::FileBrowser => "↑/↓: Navigate | Enter: Convert | R: Refresh | Q/Esc: Quit",
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
    let block = Block::default()
        .title("CRX Files")
        .borders(Borders::ALL)
        .style(Style::default());

    if app.files.is_empty() {
        let no_files = Paragraph::new("No CRX files found in current directory")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(block);
        f.render_widget(no_files, area);
    } else {
        let items: Vec<ListItem> = app
            .files
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                let style = if Some(i) == app.selected_file.selected() {
                    Style::default().fg(Color::Black).bg(Color::White)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(Span::styled(filename, style)))
            })
            .collect();

        let files_list = List::new(items)
            .block(block)
            .highlight_style(Style::default().fg(Color::Black).bg(Color::White));

        f.render_stateful_widget(files_list, area, &mut app.selected_file.clone());
    }
}

fn render_processing(f: &mut Frame, area: ratatui::layout::Rect) {
    let processing = Paragraph::new("Converting CRX file to ZIP...\n\nPlease wait...")
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(Block::default().title("Processing").borders(Borders::ALL));

    f.render_widget(processing, area);
}

fn render_success(f: &mut Frame, area: ratatui::layout::Rect, output_path: &str) {
    let success_msg = format!(
        "✓ Conversion successful!\n\nOutput file: {}\n\nPress Enter or Space to continue",
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
        "✗ Error occurred during conversion:\n\n{}\n\nPress Enter or Space to continue",
        error_msg
    );

    let error = Paragraph::new(error_text)
        .style(Style::default().fg(Color::Red))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(Block::default().title("Error").borders(Borders::ALL));

    f.render_widget(error, area);
}
