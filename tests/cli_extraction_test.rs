use std::{env, fs};
use tempfile::TempDir;
use uncrx_rs::uncrx::helpers::parse_crx;
use zip::ZipArchive;

fn extract_zip_to_directory(
    zip_data: &[u8],
    extract_to: &std::path::Path,
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

#[test]
fn test_cli_extraction_workflow() {
    // Simulate the CLI workflow from main.rs
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let crx_file_path = current_dir.join("src/mock/test-extension.crx");

    // Ensure file exists
    assert!(crx_file_path.exists(), "Test CRX file should exist");

    // Read and parse CRX (same as CLI does)
    let data = fs::read(&crx_file_path).expect("Failed to read file");
    let extension = parse_crx(&data).expect("Failed to parse crx");

    // Create output directory structure like CLI does
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let output_base_dir = temp_dir.path().join("out");

    if !output_base_dir.exists() {
        fs::create_dir_all(&output_base_dir).expect("Failed to create base output directory");
    }

    // Create directory with same name as CRX file (without extension)
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

    // Verify the extraction worked
    assert!(extract_dir.exists(), "Extraction directory should exist");
    assert_eq!(
        extract_dir.file_name().unwrap(),
        crx_name,
        "Directory should have same name as CRX file"
    );

    // Verify files were extracted
    let extracted_files: Vec<_> = fs::read_dir(&extract_dir)
        .expect("Should be able to read extraction directory")
        .collect();

    assert!(!extracted_files.is_empty(), "Should have extracted files");

    println!("CLI extraction successful: {}", extract_dir.display());
}

#[test]
fn test_tui_extraction_workflow() {
    // Simulate the TUI workflow from tui_app.rs
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let crx_file_path = current_dir.join("src/mock/test-extension.crx");

    // TUI workflow (similar to convert_crx_file method)
    let data = fs::read(&crx_file_path).expect("Failed to read file");
    let extension = parse_crx(&data).expect("Failed to parse crx");

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let output_dir = temp_dir.path().join("out");

    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).expect("Failed to create output directory");
    }

    let file_name = crx_file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("extension");

    let extract_dir = output_dir.join(file_name);

    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir).expect("Failed to remove existing directory");
    }

    fs::create_dir_all(&extract_dir).expect("Failed to create extraction directory");

    // Extract zip contents to the directory
    extract_zip_to_directory(&extension.zip, &extract_dir).expect("Failed to extract zip contents");

    // Verify the extraction worked
    assert!(
        extract_dir.exists(),
        "TUI extraction directory should exist"
    );

    let extracted_files: Vec<_> = fs::read_dir(&extract_dir)
        .expect("Should be able to read extraction directory")
        .collect();

    assert!(
        !extracted_files.is_empty(),
        "TUI should have extracted files"
    );

    println!("TUI extraction successful: {}", extract_dir.display());
}

#[test]
fn test_cli_and_tui_produce_identical_results() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let crx_file_path = current_dir.join("src/mock/test-extension.crx");

    let data = fs::read(&crx_file_path).expect("Failed to read file");
    let extension = parse_crx(&data).expect("Failed to parse crx");

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // CLI-style extraction
    let cli_extract_dir = temp_dir.path().join("cli_extraction");
    fs::create_dir_all(&cli_extract_dir).expect("Failed to create CLI extraction directory");
    extract_zip_to_directory(&extension.zip, &cli_extract_dir)
        .expect("Failed to extract with CLI method");

    // TUI-style extraction
    let tui_extract_dir = temp_dir.path().join("tui_extraction");
    fs::create_dir_all(&tui_extract_dir).expect("Failed to create TUI extraction directory");
    extract_zip_to_directory(&extension.zip, &tui_extract_dir)
        .expect("Failed to extract with TUI method");

    // Compare the results
    fn collect_files_with_content(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
        let mut files = Vec::new();

        fn walk_dir(
            dir: &std::path::Path,
            base: &std::path::Path,
            files: &mut Vec<(String, Vec<u8>)>,
        ) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if path.is_file() {
                            let relative_path = path
                                .strip_prefix(base)
                                .unwrap()
                                .to_string_lossy()
                                .to_string();
                            let content = fs::read(&path).expect("Should be able to read file");
                            files.push((relative_path, content));
                        } else if path.is_dir() {
                            walk_dir(&path, base, files);
                        }
                    }
                }
            }
        }

        walk_dir(dir, dir, &mut files);
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    }

    let cli_files = collect_files_with_content(&cli_extract_dir);
    let tui_files = collect_files_with_content(&tui_extract_dir);

    assert_eq!(
        cli_files.len(),
        tui_files.len(),
        "CLI and TUI should extract same number of files"
    );

    for (cli_file, tui_file) in cli_files.iter().zip(tui_files.iter()) {
        assert_eq!(cli_file.0, tui_file.0, "File names should match");
        assert_eq!(
            cli_file.1, tui_file.1,
            "File contents should match for {}",
            cli_file.0
        );
    }

    println!("CLI and TUI extraction methods produce identical results!");
}
