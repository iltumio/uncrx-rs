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

## Safe extraction and limits

Existing destinations are refused by default. Use `uncrx extension.crx --overwrite`
to replace one explicitly. The same option works with the TUI: `uncrx --overwrite`.
Extraction happens in a private staging directory beside the destination. Invalid
archives, CRC errors, cancellation and limit violations leave the old directory
unchanged. Publishing preserves a backup until the new directory is in place and
attempts rollback if the final rename fails. This is not a crash-durable filesystem
transaction: after a power loss or forced termination during publishing, inspect
`.uncrx-backup-*` in the output directory to recover the previous contents.

The default limits, shared by CLI and TUI, are:

| Option | Default |
| --- | --- |
| `--max-input-bytes` | 268435456 (256 MiB) |
| `--max-entry-bytes` | 268435456 (256 MiB) |
| `--max-total-bytes` | 1073741824 (1 GiB) |
| `--max-entries` | 10000 |
| `--timeout-seconds` | 120 |

Limits count actual bytes read/written as well as declared ZIP sizes. Timeout and
TUI cancellation are cooperative between reads; they do not interrupt a blocked
filesystem call. Press `q` or `Esc` during processing to cancel and quit. Once
publishing starts it completes or rolls back before exit.

Only stored and Deflate ZIP entries are enabled. Unsafe paths, Windows device
names, symlinks, special files and duplicate/conflicting file paths are rejected.
Files use normal creation permissions; archive executable/setuid bits and timestamps
are not restored. Choose an output parent not writable by untrusted users. A
persistent `.uncrx.lock` file coordinates concurrent extractions into that parent;
it is not an extracted archive entry and should not be removed while the tool runs.

CRX2 and CRX3 headers are limited to 16 MiB. Unknown versions and malformed headers
return errors. **Signatures are parsed as metadata, not cryptographically verified.**
Successful parsing/extraction does not authenticate the extension or its publisher.
For CRX3, the legacy `public_key` and `signature` fields expose the first RSA proof,
or the first ECDSA proof when there is no RSA proof. See the
[Chromium wire format](https://chromium.googlesource.com/chromium/src/+/main/components/crx_file/crx3.proto).

## Library features

Rust 1.89 or newer is required. CLI and TUI remain enabled by default. For a small
parser-only dependency without terminal or ZIP extraction dependencies:

```toml
[dependencies]
uncrx-rs = { version = "0.2.3", default-features = false }
```

Enable the `extract` feature for `uncrx_rs::extract::{extract_crx_file,
extract_zip_to_directory, ExtractionOptions}`. For a CLI without the TUI, build
with `--no-default-features --features cli`.

`parse_crx(&[u8])` retains the owned return type. `parse_crx_ref(&[u8])` borrows the
ZIP payload and avoids an archive-sized copy:

```rust
use uncrx_rs::uncrx::helpers::parse_crx_ref;
let data = std::fs::read("extension.crx")?;
let parsed = parse_crx_ref(&data)?;
std::fs::write("extension.zip", parsed.zip)?;
Ok::<(), anyhow::Error>(())
```

## Development checks

```bash
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo test --locked --no-default-features
cargo test --locked --no-default-features --features cli
cargo audit --deny warnings
cargo build --locked --all-features
python3 scripts/test_tui.py target/debug/uncrx # Unix PTY integration checks
```

CI runs tests on Linux, macOS and Windows, checks Rust 1.89, and audits dependencies
weekly as well as on changes. Dependabot proposes Cargo and GitHub Actions updates.
Release CI is generated with `dist 0.33.0`; action pins are configured in
`workspace.metadata.dist.github-action-commits` in `Cargo.toml`. Update those pins
and run `dist generate` together when updating release actions. The website uses
Oranda 0.6.5 with a checked-in archive checksum.
