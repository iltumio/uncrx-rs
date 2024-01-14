pub mod uncrx;

#[cfg(test)]
mod tests {
    use crate::uncrx::helpers::parse_crx;
    use std::{env, fs, path::PathBuf};

    fn create_directory_if_not_exists(dir_path: &PathBuf) {
        if !fs::metadata(dir_path).is_ok() {
            fs::create_dir_all(dir_path).expect("Failed to create directory");
        }
    }

    #[test]
    fn it_works() {
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
