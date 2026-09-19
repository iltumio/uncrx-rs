//! Bounded extraction into a private staging directory before publishing.
//! The destination's parent must be trusted (not writable by hostile users).
//! A persistent `.uncrx.lock` serializes cooperating extractors in that parent.
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use anyhow::{bail, ensure, Context, Result};
use zip::ZipArchive;

use crate::uncrx::helpers::parse_crx_ref;

#[derive(Debug, Clone)]
pub struct ExtractionOptions {
    pub overwrite: bool,
    pub max_input_bytes: u64,
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    pub max_entries: usize,
    pub max_duration: Duration,
}

impl Default for ExtractionOptions {
    fn default() -> Self {
        Self {
            overwrite: false,
            max_input_bytes: 256 * 1024 * 1024,
            max_entry_bytes: 256 * 1024 * 1024,
            max_total_bytes: 1024 * 1024 * 1024,
            max_entries: 10_000,
            max_duration: Duration::from_secs(120),
        }
    }
}

pub fn extract_crx_file(
    input: &Path,
    destination: &Path,
    options: &ExtractionOptions,
) -> Result<()> {
    extract_crx_file_cancellable(input, destination, options, &AtomicBool::new(false))
}

/// Cancellation is cooperative between reads. Once publishing starts, it runs
/// to completion (or rollback) to keep the previous extraction recoverable.
pub fn extract_crx_file_cancellable(
    input: &Path,
    destination: &Path,
    options: &ExtractionOptions,
    cancelled: &AtomicBool,
) -> Result<()> {
    let started = Instant::now();
    ensure!(
        fs::metadata(input)?.is_file(),
        "Input must be a regular file"
    );
    let file = File::open(input).with_context(|| format!("Cannot open {}", input.display()))?;
    let metadata = file.metadata()?;
    ensure!(metadata.is_file(), "Input must be a regular file");
    ensure!(
        metadata.len() <= options.max_input_bytes,
        "Input exceeds size limit"
    );
    let mut data = Vec::new();
    copy_bounded(
        file,
        &mut data,
        options.max_input_bytes,
        started,
        options,
        cancelled,
    )?;
    let parsed = parse_crx_ref(&data)?;
    extract_zip(parsed.zip, destination, options, started, cancelled)
}

/// Extract ZIP bytes transactionally. Unsafe paths, links, duplicate files,
/// unsupported compression, CRC errors and resource limit violations fail.
pub fn extract_zip_to_directory(
    data: &[u8],
    destination: &Path,
    options: &ExtractionOptions,
) -> Result<()> {
    ensure!(
        data.len() as u64 <= options.max_input_bytes,
        "Input exceeds size limit"
    );
    extract_zip(
        data,
        destination,
        options,
        Instant::now(),
        &AtomicBool::new(false),
    )
}

fn check_budget(
    started: Instant,
    options: &ExtractionOptions,
    cancelled: &AtomicBool,
) -> Result<()> {
    ensure!(!cancelled.load(Ordering::Relaxed), "Extraction cancelled");
    ensure!(
        started.elapsed() < options.max_duration,
        "Extraction timed out"
    );
    Ok(())
}

fn copy_bounded(
    mut reader: impl Read,
    mut writer: impl Write,
    limit: u64,
    started: Instant,
    options: &ExtractionOptions,
    cancelled: &AtomicBool,
) -> Result<u64> {
    let mut buffer = [0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        check_budget(started, options, cancelled)?;
        // Read one byte beyond the remaining allowance to detect dishonest ZIP
        // size fields, but never write that byte to the destination.
        let capacity = (limit - written).saturating_add(1).min(buffer.len() as u64) as usize;
        let count = reader.read(&mut buffer[..capacity])?;
        if count == 0 {
            return Ok(written);
        }
        ensure!(
            count as u64 <= limit - written,
            "Extracted data exceeds size limit"
        );
        writer.write_all(&buffer[..count])?;
        written += count as u64;
    }
}

fn safe_path(name: &str) -> Result<PathBuf> {
    ensure!(
        !name.contains(['\\', ':']) && !name.chars().any(char::is_control),
        "Unsafe ZIP path: {name:?}"
    );
    let name = name.strip_suffix('/').unwrap_or(name);
    ensure!(!name.is_empty(), "Empty ZIP path");
    for part in name.split('/') {
        ensure!(
            !part.is_empty() && part != "." && part != ".." && !part.ends_with(['.', ' ']),
            "Unsafe ZIP path: {name:?}"
        );
        let base = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let device = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ["COM", "LPT"].iter().any(|prefix| {
                base.strip_prefix(prefix).is_some_and(|n| {
                    matches!(
                        n,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            });
        ensure!(!device, "Reserved device name in ZIP: {name:?}");
    }
    Ok(PathBuf::from(name))
}

fn destination_exists(destination: &Path, overwrite: bool) -> Result<bool> {
    match fs::symlink_metadata(destination) {
        Ok(meta) => {
            ensure!(
                overwrite,
                "Destination already exists; use --overwrite to replace it"
            );
            ensure!(
                meta.is_dir() && !meta.file_type().is_symlink(),
                "Destination must be a real directory"
            );
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

// ZipArchive indexes entries by name, so len() alone would hide duplicate
// names and check the count only after metadata allocations. Read the bounded
// end record first; the zip crate still validates the archive itself.
fn zip_entry_count(data: &[u8]) -> Result<u64> {
    fn bytes<const N: usize>(data: &[u8], offset: usize) -> Result<[u8; N]> {
        data.get(offset..)
            .and_then(|tail| tail.get(..N))
            .context("Truncated ZIP end record")?
            .try_into()
            .map_err(Into::into)
    }
    let search_start = data.len().saturating_sub(22 + u16::MAX as usize);
    let end = (search_start..data.len().saturating_sub(21))
        .rev()
        .find(|&i| {
            data.get(i..i + 4) == Some(b"PK\x05\x06")
                && bytes::<2>(data, i + 20)
                    .is_ok_and(|n| i + 22 + u16::from_le_bytes(n) as usize == data.len())
        })
        .context("Missing ZIP end record")?;
    ensure!(
        bytes::<4>(data, end + 4)? == [0; 4],
        "Multi-disk ZIP is not supported"
    );
    let count = u16::from_le_bytes(bytes(data, end + 10)?);
    ensure!(
        u16::from_le_bytes(bytes(data, end + 8)?) == count,
        "Inconsistent ZIP entry counts"
    );
    let zip64 = count == u16::MAX
        || u32::from_le_bytes(bytes(data, end + 12)?) == u32::MAX
        || u32::from_le_bytes(bytes(data, end + 16)?) == u32::MAX;
    if !zip64 {
        return Ok(u64::from(count));
    }
    let locator = end.checked_sub(20).context("Missing ZIP64 locator")?;
    ensure!(
        bytes::<4>(data, locator)? == *b"PK\x06\x07",
        "Invalid ZIP64 locator"
    );
    ensure!(
        u32::from_le_bytes(bytes(data, locator + 4)?) == 0
            && u32::from_le_bytes(bytes(data, locator + 16)?) == 1,
        "Multi-disk ZIP64 is not supported"
    );
    let start = usize::try_from(u64::from_le_bytes(bytes(data, locator + 8)?))?;
    let record = data.get(start..locator).context("Invalid ZIP64 offset")?;
    ensure!(
        record.len() >= 56 && bytes::<4>(record, 0)? == *b"PK\x06\x06",
        "Invalid ZIP64 end record"
    );
    ensure!(
        u64::from_le_bytes(bytes(record, 4)?) == (record.len() - 12) as u64,
        "Invalid ZIP64 record length"
    );
    ensure!(
        bytes::<8>(record, 16)? == [0; 8],
        "Multi-disk ZIP64 is not supported"
    );
    let count64 = u64::from_le_bytes(bytes(record, 32)?);
    ensure!(
        u64::from_le_bytes(bytes(record, 24)?) == count64,
        "Inconsistent ZIP64 entry counts"
    );
    Ok(count64)
}

fn extract_zip(
    data: &[u8],
    destination: &Path,
    options: &ExtractionOptions,
    started: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    check_budget(started, options, cancelled)?;
    ensure!(
        destination.file_name().is_some(),
        "Destination must name a directory"
    );
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    // Keep the lock inode after closing it: deleting lockfiles creates a race
    // between old and new handles held by concurrent processes.
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(parent.join(".uncrx.lock"))?;
    lock.try_lock()
        .context("Another extraction is using this output directory")?;
    destination_exists(destination, options.overwrite)?;
    let stage = tempfile::Builder::new()
        .prefix(".uncrx-stage-")
        .tempdir_in(parent)?;
    let entries = zip_entry_count(data)?;
    ensure!(
        entries <= options.max_entries as u64,
        "ZIP entry count exceeds limit"
    );
    let mut archive = ZipArchive::new(std::io::Cursor::new(data))?;
    ensure!(
        archive.len() as u64 == entries,
        "Duplicate ZIP names or inconsistent entry count"
    );
    ensure!(
        archive.len() <= options.max_entries,
        "ZIP entry count exceeds limit"
    );
    let mut total = 0u64;
    for i in 0..archive.len() {
        check_budget(started, options, cancelled)?;
        let mut entry = archive.by_index(i)?;
        ensure!(entry.enclosed_name().is_some(), "Unsafe ZIP path");
        let relative = safe_path(entry.name())?;
        ensure!(!entry.is_symlink(), "ZIP symlinks are not supported");
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            ensure!(
                matches!(kind, 0 | 0o040000 | 0o100000),
                "Special ZIP files are not supported"
            );
        }
        let limit = options.max_entry_bytes.min(options.max_total_bytes - total);
        ensure!(entry.size() <= limit, "ZIP entry exceeds size limit");
        let path = stage.path().join(relative);
        if entry.is_dir() {
            ensure!(entry.size() == 0, "Directory contains unexpected data");
            fs::create_dir_all(path)?;
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .context("Cannot create ZIP entry (duplicate or conflicting path)")?;
            let expected = entry.size();
            let copied = copy_bounded(&mut entry, &mut output, limit, started, options, cancelled)?;
            ensure!(copied == expected, "ZIP entry size mismatch");
            total += copied;
        }
    }
    check_budget(started, options, cancelled)?;
    publish(stage, destination, options.overwrite)
}

fn publish(stage: tempfile::TempDir, destination: &Path, overwrite: bool) -> Result<()> {
    if !destination_exists(destination, overwrite)? {
        fs::rename(stage.path(), destination).context("Cannot publish extracted directory")?;
        return Ok(());
    }
    let parent = stage.path().parent().context("Missing staging parent")?;
    let backup = tempfile::Builder::new()
        .prefix(".uncrx-backup-")
        .tempdir_in(parent)?;
    let old = backup.path().join("previous");
    fs::rename(destination, &old).context("Cannot preserve previous destination")?;
    if let Err(error) = fs::rename(stage.path(), destination) {
        if let Err(rollback) = fs::rename(&old, destination) {
            let saved = backup.keep();
            bail!("Publish failed: {error}; rollback failed: {rollback}. Previous data retained at {}", saved.join("previous").display());
        }
        return Err(error).context("Cannot publish extraction; previous destination restored");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_stream_bytes_are_limited_independently_of_metadata() {
        let mut output = Vec::new();
        let result = copy_bounded(
            &b"123456"[..],
            &mut output,
            5,
            Instant::now(),
            &ExtractionOptions::default(),
            &AtomicBool::new(false),
        );
        assert!(result.is_err());
        assert!(output.len() <= 5);
    }

    #[test]
    fn failed_publish_restores_previous_directory() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("existing");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep"), b"precious").unwrap();
        let stage = tempfile::tempdir_in(root.path()).unwrap();
        // Simulate the staging directory disappearing immediately before commit.
        fs::remove_dir(stage.path()).unwrap();
        assert!(publish(stage, &destination, true).is_err());
        assert_eq!(fs::read(destination.join("keep")).unwrap(), b"precious");
    }
}
