#![cfg(feature = "extract")]
mod common;
use std::{fs, sync::atomic::AtomicBool, time::Duration};
use tempfile::tempdir;
use uncrx_rs::extract::{
    extract_crx_file, extract_crx_file_cancellable, extract_zip_to_directory, ExtractionOptions,
};

#[test]
fn extracts_real_fixture_and_nested_empty_directories() {
    let root = tempdir().unwrap();
    let destination = root.path().join("extension");
    extract_crx_file(
        std::path::Path::new("src/mock/test-extension.crx"),
        &destination,
        &ExtractionOptions::default(),
    )
    .unwrap();
    assert!(destination.join("manifest.json").is_file());
    let zip = common::zip(&[("nested/empty/", b""), ("nested/file.txt", b"hello")]);
    let destination = root.path().join("synthetic");
    extract_zip_to_directory(&zip, &destination, &ExtractionOptions::default()).unwrap();
    assert!(destination.join("nested/empty").is_dir());
    assert_eq!(
        fs::read(destination.join("nested/file.txt")).unwrap(),
        b"hello"
    );
}

#[test]
fn overwrite_is_explicit_and_corruption_preserves_previous_data() {
    let root = tempdir().unwrap();
    let dest = root.path().join("extension");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("keep"), b"valuable").unwrap();
    let valid = common::zip(&[("new", b"unique-payload")]);
    assert!(extract_zip_to_directory(&valid, &dest, &ExtractionOptions::default()).is_err());
    let overwrite = ExtractionOptions {
        overwrite: true,
        ..Default::default()
    };
    let mut corrupt = valid.clone();
    let pos = corrupt
        .windows(b"unique-payload".len())
        .position(|s| s == b"unique-payload")
        .unwrap();
    corrupt[pos] ^= 1;
    for data in [b"invalid zip".as_slice(), corrupt.as_slice()] {
        assert!(extract_zip_to_directory(data, &dest, &overwrite).is_err());
        assert_eq!(fs::read(dest.join("keep")).unwrap(), b"valuable");
        assert!(!dest.join("new").exists());
    }
    extract_zip_to_directory(&valid, &dest, &overwrite).unwrap();
    assert!(!dest.join("keep").exists());
    assert_eq!(fs::read(dest.join("new")).unwrap(), b"unique-payload");
}

#[test]
fn enforces_entry_total_count_input_and_time_limits_without_publishing() {
    let root = tempdir().unwrap();
    let zip = common::zip(&[("one", b"12345"), ("two", b"67890")]);
    let limits = [
        ExtractionOptions {
            max_entry_bytes: 4,
            ..Default::default()
        },
        ExtractionOptions {
            max_total_bytes: 9,
            ..Default::default()
        },
        ExtractionOptions {
            max_entries: 1,
            ..Default::default()
        },
        ExtractionOptions {
            max_input_bytes: 1,
            ..Default::default()
        },
        ExtractionOptions {
            max_duration: Duration::ZERO,
            ..Default::default()
        },
    ];
    for options in limits {
        let dest = root.path().join("out");
        assert!(extract_zip_to_directory(&zip, &dest, &options).is_err());
        assert!(!dest.exists());
    }
    let exact = ExtractionOptions {
        max_entry_bytes: 5,
        max_total_bytes: 10,
        max_entries: 2,
        ..Default::default()
    };
    extract_zip_to_directory(&zip, &root.path().join("exact"), &exact).unwrap();
}

#[test]
fn unsafe_paths_and_file_directory_collisions_fail() {
    let root = tempdir().unwrap();
    for name in [
        "../escape",
        "/absolute",
        "a/../../escape",
        "C:/drive",
        "a\\escape",
        "NUL.txt",
        "a/../b",
        "trailing.",
    ] {
        let zip = common::zip(&[(name, b"bad")]);
        let dest = root.path().join("out");
        assert!(
            extract_zip_to_directory(&zip, &dest, &ExtractionOptions::default()).is_err(),
            "{name}"
        );
        assert!(!dest.exists());
    }
    let zip = common::zip(&[("a", b"file"), ("a/b", b"child")]);
    assert!(extract_zip_to_directory(
        &zip,
        &root.path().join("out"),
        &ExtractionOptions::default()
    )
    .is_err());
}

#[test]
fn cancellation_preserves_destination() {
    let root = tempdir().unwrap();
    let dest = root.path().join("out");
    let input = root.path().join("test.crx");
    fs::write(
        &input,
        common::crx2(b"", b"", &common::zip(&[("a", b"data")])),
    )
    .unwrap();
    assert!(extract_crx_file_cancellable(
        &input,
        &dest,
        &ExtractionOptions::default(),
        &AtomicBool::new(true)
    )
    .is_err());
    assert!(!dest.exists());
}

#[test]
fn rejects_zip_symlinks() {
    let root = tempdir().unwrap();
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .add_symlink(
            "link",
            "../outside",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    let data = writer.finish().unwrap().into_inner();
    assert!(extract_zip_to_directory(
        &data,
        &root.path().join("out"),
        &ExtractionOptions::default()
    )
    .is_err());
}

#[cfg(unix)]
#[test]
fn rejects_existing_destination_symlink_even_with_overwrite() {
    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("keep"), b"safe").unwrap();
    let dest = root.path().join("link");
    std::os::unix::fs::symlink(outside.path(), &dest).unwrap();
    let options = ExtractionOptions {
        overwrite: true,
        ..Default::default()
    };
    assert!(extract_zip_to_directory(&common::zip(&[("a", b"bad")]), &dest, &options).is_err());
    assert_eq!(fs::read(outside.path().join("keep")).unwrap(), b"safe");
}

#[test]
fn compressed_expansion_is_rejected_before_publication() {
    use std::io::Write;
    let root = tempdir().unwrap();
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file(
            "large",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(&vec![0; 1024 * 1024]).unwrap();
    let data = writer.finish().unwrap().into_inner();
    assert!(data.len() < 10_000);
    let options = ExtractionOptions {
        max_total_bytes: 4096,
        ..Default::default()
    };
    let destination = root.path().join("out");
    assert!(extract_zip_to_directory(&data, &destination, &options).is_err());
    assert!(!destination.exists());
}

#[test]
fn concurrent_extraction_is_refused_while_output_parent_is_locked() {
    use std::fs::OpenOptions;
    let root = tempdir().unwrap();
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(root.path().join(".uncrx.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let result = extract_zip_to_directory(
        &common::zip(&[("a", b"b")]),
        &root.path().join("out"),
        &ExtractionOptions::default(),
    );
    assert!(format!("{:#}", result.unwrap_err()).contains("Another extraction"));
    assert!(!root.path().join("out").exists());
}

#[test]
fn rejects_duplicate_names_even_when_zip_reader_deduplicates_them() {
    let mut data = common::zip(&[("a", b"first"), ("b", b"second")]);
    // Rename b to a in its local header and central directory. Sizes and CRCs
    // stay valid; only duplicate naming makes this archive unacceptable.
    for i in 0..data.len().saturating_sub(47) {
        if data.get(i..i + 4) == Some(b"PK\x03\x04") && data[i + 30] == b'b' {
            data[i + 30] = b'a';
        }
        if data.get(i..i + 4) == Some(b"PK\x01\x02") && data[i + 46] == b'b' {
            data[i + 46] = b'a';
        }
    }
    let root = tempdir().unwrap();
    let error = extract_zip_to_directory(
        &data,
        &root.path().join("out"),
        &ExtractionOptions::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("Duplicate ZIP"));
    assert!(!root.path().join("out").exists());
}

#[test]
fn zip64_end_records_are_supported_and_count_limited() {
    let mut data = common::zip(&[("file", b"hello")]);
    let end = data.len() - 22;
    let mut legacy = data.split_off(end);
    let size = u32::from_le_bytes(legacy[12..16].try_into().unwrap());
    let offset = u32::from_le_bytes(legacy[16..20].try_into().unwrap());
    data.extend(b"PK\x06\x06");
    data.extend(44u64.to_le_bytes());
    data.extend(45u16.to_le_bytes());
    data.extend(45u16.to_le_bytes());
    data.extend([0; 8]);
    for n in [1u64, 1, u64::from(size), u64::from(offset)] {
        data.extend(n.to_le_bytes());
    }
    data.extend(b"PK\x06\x07");
    data.extend([0; 4]);
    data.extend((end as u64).to_le_bytes());
    data.extend(1u32.to_le_bytes());
    legacy[8..20].fill(255);
    data.extend(legacy);
    let root = tempdir().unwrap();
    extract_zip_to_directory(
        &data,
        &root.path().join("ok"),
        &ExtractionOptions::default(),
    )
    .unwrap();
    assert_eq!(fs::read(root.path().join("ok/file")).unwrap(), b"hello");
    for position in [end + 24, end + 32] {
        data[position..position + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    }
    let error = extract_zip_to_directory(
        &data,
        &root.path().join("bad"),
        &ExtractionOptions::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("entry count exceeds limit"));
}
