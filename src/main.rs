pub mod cli;
mod tui_app;

use clap::Parser;
use cli::{errors::UncrxCliError, helpers::exit_with_error};
use std::{env, fs};
use uncrx_rs::uncrx::helpers::parse_crx;

#[derive(Parser)]
#[command(name = "uncrx-rs")]
#[command(author = "Manuel Tumiati <tumiatimanuel@gmail.com>")]
#[command(version = "1.0")]
#[command(about = "Easily convert a CRX Extension to a zip file", long_about = None)]
#[command(next_line_help = true)]
struct Cli {
    /// CRX file to convert (if not provided, TUI mode will launch)
    filename: Option<String>,
    #[arg(short, long)]
    output_dir: Option<String>,
}

pub fn main() {
    let cli = Cli::parse();

    // If no filename is provided, launch TUI mode
    let filename = match cli.filename {
        Some(file) => file,
        None => {
            // Launch TUI mode
            if let Err(err) = tui_app::run_tui() {
                eprintln!("TUI Error: {}", err);
                std::process::exit(1);
            }
            return;
        }
    };

    // CLI mode - process the provided file
    match filename.ends_with(".crx") {
        true => {}
        false => {
            exit_with_error(UncrxCliError::UnsupportedFileType);
        }
    }

    let current_dir = env::current_dir().expect("Failed to get current directory");

    let crx_file_path = current_dir.join(&filename);

    if !crx_file_path.exists() {
        exit_with_error(UncrxCliError::NotFound(
            crx_file_path.to_str().unwrap().to_string(),
        ));
    }

    let data = fs::read(crx_file_path.to_str().unwrap()).expect("Failed to read file");

    let extension = parse_crx(&data).expect("Failed to parse crx");

    let output_dir = match cli.output_dir {
        Some(path) => current_dir.join(path),
        None => current_dir.join("out"),
    };

    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).expect("Failed to create directory");
    }

    let output_file = output_dir.join("extension.zip");

    fs::write(output_file, &extension.zip).expect("Failed to write file");
}
