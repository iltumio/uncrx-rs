#![cfg(feature = "cli")]
mod common;
use std::{
    fs,
    process::{Command, Output},
};
use tempfile::tempdir;

fn run(root: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_uncrx"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn real_cli_extracts_and_requires_explicit_overwrite() {
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("test.crx"),
        common::crx2(b"", b"", &common::zip(&[("nested/file", b"hello")])),
    )
    .unwrap();
    let first = run(root.path(), &["test.crx", "-o", "custom"]);
    assert!(first.status.success(), "{:?}", first);
    assert_eq!(
        fs::read(root.path().join("custom/test/nested/file")).unwrap(),
        b"hello"
    );
    fs::write(root.path().join("custom/test/keep"), b"old").unwrap();
    assert!(!run(root.path(), &["test.crx", "-o", "custom"])
        .status
        .success());
    assert!(root.path().join("custom/test/keep").exists());
    assert!(
        run(root.path(), &["test.crx", "-o", "custom", "--overwrite"])
            .status
            .success()
    );
    assert!(!root.path().join("custom/test/keep").exists());
}

#[test]
fn real_cli_reports_malformed_input_without_panicking_or_losing_data() {
    let root = tempdir().unwrap();
    fs::create_dir_all(root.path().join("out/test")).unwrap();
    fs::write(root.path().join("out/test/keep"), b"old").unwrap();
    for bytes in [
        b"Cr24\x02\0\0\0\xff\xff\xff\xff".to_vec(),
        common::crx2(b"", b"", b"broken ZIP"),
    ] {
        fs::write(root.path().join("test.crx"), bytes).unwrap();
        let result = run(root.path(), &["test.crx", "--overwrite"]);
        assert_eq!(result.status.code(), Some(1));
        assert!(!String::from_utf8_lossy(&result.stderr).contains("panicked"));
        assert_eq!(fs::read(root.path().join("out/test/keep")).unwrap(), b"old");
    }
}

#[test]
fn real_cli_applies_limits_and_reports_package_version() {
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("test.crx"),
        common::crx2(b"", b"", &common::zip(&[("file", b"hello")])),
    )
    .unwrap();
    for args in [
        vec!["--max-entry-bytes", "4"],
        vec!["--max-total-bytes", "4"],
        vec!["--max-entries", "0"],
        vec!["--max-input-bytes", "1"],
        vec!["--timeout-seconds", "0"],
    ] {
        let mut cmd = vec!["test.crx"];
        cmd.extend(args);
        assert_eq!(run(root.path(), &cmd).status.code(), Some(1));
        assert!(!root.path().join("out/test").exists());
    }
    let version = run(root.path(), &["--version"]);
    assert!(String::from_utf8_lossy(&version.stdout).contains(env!("CARGO_PKG_VERSION")));
}
