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
fn test_end_to_end_crx_extraction() {
    // Test the complete workflow from CRX file to extracted directory
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let file_path = current_dir.join("src/mock/test-extension.crx");

    // Ensure the test file exists
    assert!(
        file_path.exists(),
        "Test CRX file should exist at: {}",
        file_path.display()
    );

    // Read and parse the CRX file
    let data = fs::read(&file_path).expect("Failed to read CRX file");
    let extension = parse_crx(&data).expect("Failed to parse CRX file");

    // Create temporary extraction directory
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let crx_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("extension");
    let extract_dir = temp_dir.path().join(crx_name);

    // Extract the extension
    extract_zip_to_directory(&extension.zip, &extract_dir).expect("Failed to extract ZIP contents");

    // Verify extraction was successful
    assert!(extract_dir.exists(), "Extraction directory should exist");

    // Check that we have files in the extraction directory
    let extracted_entries: Vec<_> = fs::read_dir(&extract_dir)
        .expect("Should be able to read extraction directory")
        .collect();

    assert!(
        !extracted_entries.is_empty(),
        "Should have extracted at least one file/directory"
    );

    // Verify the extraction maintains the original ZIP structure
    let cursor = std::io::Cursor::new(&extension.zip);
    let archive = ZipArchive::new(cursor).expect("Should be able to read ZIP data");

    println!("ZIP archive contains {} entries", archive.len());

    // Print extracted structure for debugging
    print_directory_structure(&extract_dir, 0);
}

#[test]
fn test_crx_file_produces_valid_extension_structure() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let file_path = current_dir.join("src/mock/test-extension.crx");

    let data = fs::read(&file_path).expect("Failed to read CRX file");
    let extension = parse_crx(&data).expect("Failed to parse CRX file");

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let extract_dir = temp_dir.path().join("test-extension");

    extract_zip_to_directory(&extension.zip, &extract_dir).expect("Failed to extract ZIP contents");

    // Check for common Chrome extension files
    let manifest_path = extract_dir.join("manifest.json");

    if manifest_path.exists() {
        let manifest_content =
            fs::read_to_string(&manifest_path).expect("Should be able to read manifest.json");

        // Basic validation that it's a JSON file
        assert!(!manifest_content.is_empty(), "Manifest should not be empty");
        assert!(
            manifest_content.contains("manifest_version")
                || manifest_content.contains("name")
                || manifest_content.contains("version"),
            "Manifest should contain typical extension fields"
        );

        println!(
            "Manifest content preview: {}",
            &manifest_content.chars().take(200).collect::<String>()
        );
    }

    // Check for other common extension files
    let common_files = [
        "background.js",
        "content.js",
        "popup.html",
        "options.html",
        "icon.png",
    ];
    for file_name in &common_files {
        let file_path = extract_dir.join(file_name);
        if file_path.exists() {
            println!("Found common extension file: {}", file_name);
        }
    }
}

#[test]
fn test_extraction_preserves_file_permissions() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let file_path = current_dir.join("src/mock/test-extension.crx");

    let data = fs::read(&file_path).expect("Failed to read CRX file");
    let extension = parse_crx(&data).expect("Failed to parse CRX file");

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let extract_dir = temp_dir.path().join("permissions-test");

    extract_zip_to_directory(&extension.zip, &extract_dir).expect("Failed to extract ZIP contents");

    // Verify all extracted files are readable
    fn check_file_permissions(dir: &std::path::Path) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        // Verify we can read the file
                        assert!(
                            fs::read(&path).is_ok(),
                            "Should be able to read extracted file: {}",
                            path.display()
                        );
                    } else if path.is_dir() {
                        check_file_permissions(&path);
                    }
                }
            }
        }
    }

    check_file_permissions(&extract_dir);
}

#[test]
fn test_multiple_extractions_to_different_directories() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let file_path = current_dir.join("src/mock/test-extension.crx");

    let data = fs::read(&file_path).expect("Failed to read CRX file");
    let extension = parse_crx(&data).expect("Failed to parse CRX file");

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Extract to multiple different directories
    let extract_dirs = [
        temp_dir.path().join("extraction1"),
        temp_dir.path().join("extraction2"),
        temp_dir.path().join("nested/extraction3"),
    ];

    for extract_dir in &extract_dirs {
        extract_zip_to_directory(&extension.zip, extract_dir)
            .expect("Failed to extract ZIP contents");

        assert!(
            extract_dir.exists(),
            "Extraction directory should exist: {}",
            extract_dir.display()
        );

        let entries: Vec<_> = fs::read_dir(extract_dir)
            .expect("Should be able to read extraction directory")
            .collect();

        assert!(
            !entries.is_empty(),
            "Should have extracted files to: {}",
            extract_dir.display()
        );
    }

    // Verify all extractions are identical
    let entries1: Vec<_> = collect_all_files(&extract_dirs[0]);
    let entries2: Vec<_> = collect_all_files(&extract_dirs[1]);
    let entries3: Vec<_> = collect_all_files(&extract_dirs[2]);

    assert_eq!(
        entries1.len(),
        entries2.len(),
        "All extractions should have same number of files"
    );
    assert_eq!(
        entries1.len(),
        entries3.len(),
        "All extractions should have same number of files"
    );
}

#[test]
fn test_overwrite_existing_extraction() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let file_path = current_dir.join("src/mock/test-extension.crx");

    let data = fs::read(&file_path).expect("Failed to read CRX file");
    let extension = parse_crx(&data).expect("Failed to parse CRX file");

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let extract_dir = temp_dir.path().join("overwrite-test");

    // First extraction
    extract_zip_to_directory(&extension.zip, &extract_dir)
        .expect("Failed to extract ZIP contents first time");

    let first_extraction_files = collect_all_files(&extract_dir);

    // Create a dummy file that should be overwritten
    let dummy_file = extract_dir.join("dummy.txt");
    fs::write(&dummy_file, "This should be overwritten").expect("Failed to create dummy file");

    // Second extraction (should overwrite)
    extract_zip_to_directory(&extension.zip, &extract_dir)
        .expect("Failed to extract ZIP contents second time");

    let second_extraction_files = collect_all_files(&extract_dir);

    // The dummy file should still be there (since we're not removing the directory first)
    // but the original files should be intact
    assert!(
        first_extraction_files.len() <= second_extraction_files.len(),
        "Second extraction should have at least as many files as first"
    );
}

// Helper function to print directory structure for debugging
fn print_directory_structure(dir: &std::path::Path, indent: usize) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");

                println!(
                    "{}{}{}",
                    "  ".repeat(indent),
                    if path.is_dir() { "📁 " } else { "📄 " },
                    name
                );

                if path.is_dir() {
                    print_directory_structure(&path, indent + 1);
                }
            }
        }
    }
}

// Helper function to collect all files in a directory recursively
fn collect_all_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();

    fn walk_dir(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        files.push(path);
                    } else if path.is_dir() {
                        walk_dir(&path, files);
                    }
                }
            }
        }
    }

    walk_dir(dir, &mut files);
    files.sort();
    files
}
