# Uncrx-rs

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Description

Uncrx is a library that helps you convert a CRX Extension to a zip file that can be easily 
extracted.

## Table of Contents

- [Installation](#installation)
- [Usage](#usage)
- [Contributing](#contributing)
- [License](#license)

## Installation

```
cargo add uncrx-rs
```

## Usage

```rust
// Open the CRX extension
let current_dir = env::current_dir().expect("Failed to get current directory");
let file_path = current_dir.join("src/mock/test-extension.crx");
let data = fs::read(file_path.to_str().unwrap()).expect("Failed to read file");

// Parse the extension
let extension = parse_crx(&data).expect("Failed to parse crx");

// Eventually save the zip section into a separate file for later extraction
let output_file = current_dir.join("out/extension.zip");
fs::write(output_file, &extension.zip).expect("Failed to write file");
```

## Contributing

Feel free to open issues and send PRs. We will evaluate them together in the comment section.

## License

This project is licensed under the [MIT License](LICENSE).
