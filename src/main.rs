pub mod cli;
mod tui_app;

use clap::Parser;
use cli::{errors::UncrxCliError, helpers::exit_with_error};
use std::{env, fs, path::Path};
use uncrx_rs::uncrx::helpers::parse_crx;
use zip::ZipArchive;

#[derive(Parser)]
#[command(name = "uncrx-rs")]
#[command(author = "Manuel Tumiati <tumiatimanuel@gmail.com>")]
#[command(version = "1.0")]
#[command(about = "Easily convert a CRX Extension to a zip file", long_about = None)]
#[command(next_line_help = true)]
struct Cli {
    /// CRX file to convert
    filename: String,
    #[arg(short, long)]
    output_dir: Option<String>,
}

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

pub fn main() {
    // If no arguments provided, launch TUI mode
    if env::args().len() == 1 {
        if let Err(err) = tui_app::run_tui() {
            eprintln!("TUI Error: {}", err);
            std::process::exit(1);
        }
        return;
    }

    // CLI mode - parse arguments and process
    let cli = Cli::parse();
    let filename = cli.filename;

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

    let output_base_dir = match cli.output_dir {
        Some(path) => current_dir.join(path),
        None => current_dir.join("out"),
    };

    if !output_base_dir.exists() {
        fs::create_dir_all(&output_base_dir).expect("Failed to create base output directory");
    }

    // Create a directory with the same name as the CRX file (without extension)
    let crx_name = crx_file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("extension");

    let extract_dir = output_base_dir.join(crx_name);

    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir).expect("Failed to remove existing directory");
    }

    fs::create_dir_all(&extract_dir).expect("Failed to create extraction directory");

    // Extract zip contents to the directory
    extract_zip_to_directory(&extension.zip, &extract_dir).expect("Failed to extract zip contents");

    println!(
        "Successfully extracted {} to {}",
        filename,
        extract_dir.display()
    );
}
