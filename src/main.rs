#[cfg(feature = "tui")]
mod tui_app;

use anyhow::{Context, Result};
use clap::Parser;
use std::{path::PathBuf, time::Duration};
use uncrx_rs::extract::{extract_crx_file, ExtractionOptions};

#[derive(Parser)]
#[command(
    name = "uncrx",
    version,
    about = "Extract CRX2/CRX3 archives (signatures are not verified)"
)]
struct Cli {
    /// CRX file to extract. With no filename, launch the TUI when enabled.
    filename: Option<PathBuf>,
    #[arg(short, long, default_value = "out")]
    output_dir: PathBuf,
    /// Replace an existing extraction only after the new one succeeds.
    #[arg(long)]
    overwrite: bool,
    #[arg(long, default_value_t = 256 * 1024 * 1024)]
    max_input_bytes: u64,
    #[arg(long, default_value_t = 256 * 1024 * 1024)]
    max_entry_bytes: u64,
    #[arg(long, default_value_t = 1024 * 1024 * 1024)]
    max_total_bytes: u64,
    #[arg(long, default_value_t = 10_000)]
    max_entries: usize,
    #[arg(long, default_value_t = 120)]
    timeout_seconds: u64,
}

fn run(cli: Cli) -> Result<()> {
    let options = ExtractionOptions {
        overwrite: cli.overwrite,
        max_input_bytes: cli.max_input_bytes,
        max_entry_bytes: cli.max_entry_bytes,
        max_total_bytes: cli.max_total_bytes,
        max_entries: cli.max_entries,
        max_duration: Duration::from_secs(cli.timeout_seconds),
    };
    let Some(input) = cli.filename else {
        #[cfg(feature = "tui")]
        return tui_app::run_tui(cli.output_dir, options);
        #[cfg(not(feature = "tui"))]
        anyhow::bail!("A CRX filename is required (TUI support is disabled)");
    };
    let name = input.file_stem().context("Input must have a filename")?;
    anyhow::ensure!(
        name != "." && name != "..",
        "Invalid extraction directory name"
    );
    let destination = cli.output_dir.join(name);
    extract_crx_file(&input, &destination, &options)?;
    println!(
        "Successfully extracted {} to {}",
        input.display(),
        destination.display()
    );
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run(Cli::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
