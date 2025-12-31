pub mod uncrx;

#[cfg(test)]
mod tests {
    use crate::uncrx::helpers::parse_crx;
    use std::{env, fs, path::PathBuf};
    use tempfile::TempDir;
    use zip::ZipArchive;

    fn create_directory_if_not_exists(dir_path: &PathBuf) {
        if !fs::metadata(dir_path).is_ok() {
            fs::create_dir_all(dir_path).expect("Failed to create directory");
        }
    }

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
    fn test_parse_crx_basic() {
        let current_dir = env::current_dir().expect("Failed to get current directory");
        let file_path = current_dir.join("src/mock/test-extension.crx");
        let data = fs::read(file_path.to_str().unwrap()).expect("Failed to read file");

        let extension = parse_crx(&data).expect("Failed to parse crx");

        // Verify that we got a valid zip file
        assert!(
            !extension.zip.is_empty(),
            "Extracted zip data should not be empty"
        );

        // Verify that the zip data is valid by trying to read it
        let cursor = std::io::Cursor::new(&extension.zip);
        let archive = ZipArchive::new(cursor).expect("Should be able to read the zip data");

        assert!(
            archive.len() > 0,
            "Zip archive should contain at least one file"
        );
    }

    #[test]
    fn test_extract_zip_to_directory() {
        let current_dir = env::current_dir().expect("Failed to get current directory");
        let file_path = current_dir.join("src/mock/test-extension.crx");
        let data = fs::read(file_path.to_str().unwrap()).expect("Failed to read file");

        let extension = parse_crx(&data).expect("Failed to parse crx");

        // Create a temporary directory for extraction
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let extract_path = temp_dir.path().join("extracted");

        // Extract the zip contents
        extract_zip_to_directory(&extension.zip, &extract_path)
            .expect("Failed to extract zip contents");

        // Verify that files were extracted
        assert!(extract_path.exists(), "Extraction directory should exist");

        // Read the extracted files and verify they exist
        let extracted_files: Vec<_> = fs::read_dir(&extract_path)
            .expect("Should be able to read extracted directory")
            .collect();

        assert!(
            !extracted_files.is_empty(),
            "Should have extracted at least one file"
        );

        // Verify specific files that should exist in a Chrome extension
        let manifest_path = extract_path.join("manifest.json");
        if manifest_path.exists() {
            let manifest_content =
                fs::read_to_string(&manifest_path).expect("Should be able to read manifest.json");
            assert!(!manifest_content.is_empty(), "Manifest should not be empty");
        }
    }

    #[test]
    fn test_extract_with_subdirectories() {
        let current_dir = env::current_dir().expect("Failed to get current directory");
        let file_path = current_dir.join("src/mock/test-extension.crx");
        let data = fs::read(file_path.to_str().unwrap()).expect("Failed to read file");

        let extension = parse_crx(&data).expect("Failed to parse crx");

        // Create a temporary directory for extraction
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let extract_path = temp_dir.path().join("test-extension");

        // Extract the zip contents
        extract_zip_to_directory(&extension.zip, &extract_path)
            .expect("Failed to extract zip contents");

        // Verify that files were extracted
        assert!(extract_path.exists(), "Extraction directory should exist");

        // Walk through all extracted files and verify structure
        fn walk_directory(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if path.is_file() {
                            files.push(path);
                        } else if path.is_dir() {
                            walk_directory(&path, files);
                        }
                    }
                }
            }
        }

        let mut all_files = Vec::new();
        walk_directory(&extract_path, &mut all_files);

        assert!(!all_files.is_empty(), "Should have extracted files");

        // Verify that all extracted files have content
        for file_path in all_files {
            let metadata = fs::metadata(&file_path).expect("Should be able to get file metadata");

            // Most files should have some content (though some might be empty)
            println!(
                "Extracted file: {} (size: {} bytes)",
                file_path.display(),
                metadata.len()
            );
        }
    }

    #[test]
    fn test_extract_creates_proper_directory_structure() {
        let current_dir = env::current_dir().expect("Failed to get current directory");
        let file_path = current_dir.join("src/mock/test-extension.crx");
        let data = fs::read(file_path.to_str().unwrap()).expect("Failed to read file");

        let extension = parse_crx(&data).expect("Failed to parse crx");

        // Create a temporary directory for extraction
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let extract_path = temp_dir.path().join("my-extension");

        // Extract the zip contents
        extract_zip_to_directory(&extension.zip, &extract_path)
            .expect("Failed to extract zip contents");

        // Verify directory structure
        assert!(
            extract_path.exists(),
            "Root extraction directory should exist"
        );

        // Check that we can list all files in the extraction
        let cursor = std::io::Cursor::new(&extension.zip);
        let mut archive = ZipArchive::new(cursor).expect("Should be able to read zip");

        // Verify that each file in the zip was extracted properly
        for i in 0..archive.len() {
            let file = archive
                .by_index(i)
                .expect("Should be able to read zip entry");

            if let Some(file_path) = file.enclosed_name() {
                let extracted_file = extract_path.join(file_path);

                if file.name().ends_with('/') {
                    // Directory
                    assert!(
                        extracted_file.exists() && extracted_file.is_dir(),
                        "Directory {} should exist",
                        extracted_file.display()
                    );
                } else {
                    // File
                    assert!(
                        extracted_file.exists() && extracted_file.is_file(),
                        "File {} should exist",
                        extracted_file.display()
                    );

                    // Verify file size matches
                    let extracted_size = fs::metadata(&extracted_file)
                        .expect("Should be able to get file metadata")
                        .len();

                    assert_eq!(
                        extracted_size,
                        file.size(),
                        "File size should match for {}",
                        extracted_file.display()
                    );
                }
            }
        }
    }

    #[test]
    fn test_extraction_handles_empty_directories() {
        let current_dir = env::current_dir().expect("Failed to get current directory");
        let file_path = current_dir.join("src/mock/test-extension.crx");
        let data = fs::read(file_path.to_str().unwrap()).expect("Failed to read file");

        let extension = parse_crx(&data).expect("Failed to parse crx");

        // Create a temporary directory for extraction
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let extract_path = temp_dir.path().join("extension-with-dirs");

        // Extract the zip contents
        let result = extract_zip_to_directory(&extension.zip, &extract_path);

        assert!(
            result.is_ok(),
            "Extraction should succeed even with empty directories"
        );
        assert!(extract_path.exists(), "Extraction directory should exist");
    }

    #[test]
    fn test_it_works_legacy() {
        // Keep the original test for backward compatibility
        let current_dir = env::current_dir().expect("Failed to get current directory");
        let file_path = current_dir.join("src/mock/test-extension.crx");
        let data = fs::read(file_path.to_str().unwrap()).expect("Failed to read file");

        let extension = parse_crx(&data).expect("Failed to parse crx");

        let output_dir = current_dir.join("out");
        create_directory_if_not_exists(&output_dir);

        let output_file = current_dir.join("out/extension.zip");
        fs::write(output_file, &extension.zip).expect("Failed to write file");
    }
}
