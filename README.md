# Uncrx-rs

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Crates.io Version](https://img.shields.io/crates/v/uncrx-rs)](https://crates.io/crates/uncrx-rs)

A Rust library and command-line tool for converting Chrome CRX extension files to ZIP archives that can be easily extracted.

## Description

Uncrx-rs provides both a library and a CLI tool to help you convert CRX (Chrome Extension) files to ZIP format. CRX files are Chrome's packaged extension format, which contains a header with metadata and a ZIP archive. This tool extracts the ZIP portion, making it easy to inspect, modify, or extract Chrome extensions.

## Features

- 🚀 **Library API** - Use `uncrx-rs` as a library in your Rust projects
- 💻 **CLI Tool** - Command-line interface for quick conversions
- 🎨 **TUI Mode** - Interactive terminal user interface for browsing and extracting CRX files
- 📦 **Multi-platform** - Supports macOS, Linux, and Windows
- ✅ **Well-tested** - Comprehensive test suite

## Table of Contents

- [Installation](#installation)
  - [As a Library](#as-a-library)
  - [As a Binary](#as-a-binary)
- [Usage](#usage)
  - [CLI Mode](#cli-mode)
  - [TUI Mode](#tui-mode)
  - [Library Usage](#library-usage)
- [Contributing](#contributing)
- [License](#license)

## Installation

### As a Library

Add `uncrx-rs` to your `Cargo.toml`:

```toml
[dependencies]
uncrx-rs = "0.2.3"
```

Or use `cargo add`:

```bash
cargo add uncrx-rs
```

### As a Binary

#### Using Cargo

```bash
cargo install uncrx-rs
```

#### Using Homebrew (macOS)

```bash
brew install uncrx-rs
```

#### Building from Source

```bash
git clone https://github.com/iltumio/uncrx-rs.git
cd uncrx-rs
cargo build --release
```

The binary will be available at `target/release/uncrx`.

## Usage

### CLI Mode

Extract a CRX file to a directory:

```bash
uncrx extension.crx
```

This will extract the extension to `out/extension/` by default.

Specify a custom output directory:

```bash
uncrx extension.crx -o my-output-dir
# or
uncrx extension.crx --output-dir my-output-dir
```

### TUI Mode

Launch the interactive terminal user interface:

```bash
uncrx
```

The TUI allows you to:

- Browse directories and find CRX files
- Navigate with arrow keys or `j`/`k`
- Extract CRX files by pressing Enter
- Refresh the file list with `r`
- Quit with `q` or `Esc`

**TUI Controls:**

- `↑/↓` or `j/k`: Navigate files and directories
- `Enter`: Open directory or extract CRX file
- `R`: Refresh file list
- `Q` or `Esc`: Quit

### Library Usage

```rust
use std::fs;
use uncrx_rs::uncrx::helpers::parse_crx;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read the CRX file
    let data = fs::read("extension.crx")?;

    // Parse the CRX file
    let extension = parse_crx(&data)?;

    // Access the extracted ZIP data
    let zip_data = &extension.zip;

    // Access metadata
    println!("Version: {}", extension.version);
    println!("Public key length: {} bytes", extension.public_key.len());

    // Save the ZIP to a file
    fs::write("extension.zip", zip_data)?;

    Ok(())
}
```

The `parse_crx` function returns a `CrxExtension` struct containing:

- `version`: The CRX format version
- `public_key`: The extension's public key
- `signature`: The extension signature (if present)
- `zip`: The ZIP archive data

## Contributing

Contributions are welcome! Feel free to open issues and send pull requests. We'll evaluate them together in the comment section.

## License

This project is licensed under the [MIT License](LICENSE).
