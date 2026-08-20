//! Shared installer for the large model files Eavesdrop fetches on demand.
//!
//! The Whisper and summary catalogs both download one big file over HTTPS,
//! stream progress to the webview, and refuse to install anything whose digest
//! does not match the catalog. Only the digest algorithm and the progress event
//! differ between them, so the transfer itself lives here.

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use sha1::{Sha1, digest::Digest};
use sha2::Sha256;

use crate::error::{AppError, AppResult};

/// Headroom kept free on top of the download so installing a model cannot fill
/// the disk out from under an in-flight recording.
const SPARE_BYTES: u64 = 100 * 1024 * 1024;

/// The published checksum for a catalog entry. whisper.cpp publishes SHA-1;
/// Hugging Face exposes the LFS SHA-256 for GGUF files.
#[derive(Debug, Clone, Copy)]
pub enum Checksum<'a> {
    Sha1(&'a str),
    Sha256(&'a str),
}

impl Checksum<'_> {
    fn expected(&self) -> &str {
        match self {
            Self::Sha1(value) | Self::Sha256(value) => value,
        }
    }

    fn matches(&self, path: &Path) -> AppResult<bool> {
        let actual = match self {
            Self::Sha1(_) => file_digest::<Sha1>(path)?,
            Self::Sha256(_) => file_digest::<Sha256>(path)?,
        };
        Ok(actual == self.expected())
    }
}

/// Downloads `url` into `destination`, verifying it before it replaces any
/// installed copy. An already-installed file with the right digest is left
/// alone. `on_progress` receives `(downloaded, total)` byte counts.
pub fn install(
    url: &str,
    destination: &Path,
    size_bytes: u64,
    checksum: Checksum<'_>,
    what: &str,
    mut on_progress: impl FnMut(u64, u64),
) -> AppResult<PathBuf> {
    let directory = destination
        .parent()
        .ok_or_else(|| AppError::Storage(format!("{what} has no install directory")))?;
    fs::create_dir_all(directory)?;
    if destination.is_file() && checksum.matches(destination)? {
        on_progress(size_bytes, size_bytes);
        return Ok(destination.to_path_buf());
    }
    if fs2::available_space(directory).unwrap_or(u64::MAX) < size_bytes + SPARE_BYTES {
        return Err(AppError::Storage(format!(
            "not enough free space to install this {what}"
        )));
    }

    let pending = destination.with_extension("part");
    let result = download(url, &pending, size_bytes, what, &mut on_progress).and_then(|()| {
        if !checksum.matches(&pending)? {
            return Err(AppError::Other(format!(
                "the downloaded {what} failed its integrity check"
            )));
        }
        if destination.exists() {
            fs::remove_file(destination)?;
        }
        fs::rename(&pending, destination)?;
        Ok(destination.to_path_buf())
    });
    if result.is_err() {
        let _ = fs::remove_file(pending);
    }
    result
}

fn download(
    url: &str,
    path: &Path,
    size_hint: u64,
    what: &str,
    on_progress: &mut impl FnMut(u64, u64),
) -> AppResult<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60 * 60))
        .user_agent("Eavesdrop model installer")
        .build()
        .map_err(|error| AppError::Other(format!("could not start the download: {error}")))?;
    let mut response = client
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| AppError::Other(format!("could not download the {what}: {error}")))?;
    let expected_bytes = response.content_length();
    let total_bytes = expected_bytes.unwrap_or(size_hint);
    let mut file = File::create(path)?;
    let mut downloaded_bytes = 0u64;
    let mut last_reported_bytes = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    on_progress(0, total_bytes);
    loop {
        let count = response
            .read(&mut buffer)
            .map_err(|error| AppError::Other(format!("the {what} download stopped: {error}")))?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])?;
        downloaded_bytes += count as u64;
        if downloaded_bytes.saturating_sub(last_reported_bytes) >= 1024 * 1024
            || expected_bytes == Some(downloaded_bytes)
        {
            on_progress(downloaded_bytes, total_bytes);
            last_reported_bytes = downloaded_bytes;
        }
    }
    file.sync_all()?;
    if let Some(expected_bytes) = expected_bytes
        && downloaded_bytes != expected_bytes
    {
        return Err(AppError::Other(format!(
            "the {what} download was incomplete ({downloaded_bytes} of {expected_bytes} bytes)"
        )));
    }
    on_progress(downloaded_bytes, downloaded_bytes);
    Ok(())
}

fn file_digest<D: Digest>(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = D::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digests_use_the_published_hex_format() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("model.bin");
        fs::write(&path, b"abc").unwrap();

        assert!(
            Checksum::Sha1("a9993e364706816aba3e25717850c26c9cd0d89d")
                .matches(&path)
                .unwrap()
        );
        assert!(
            Checksum::Sha256("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
                .matches(&path)
                .unwrap()
        );
        assert!(!Checksum::Sha1("0".repeat(40).as_str()).matches(&path).unwrap());
    }

    #[test]
    fn an_installed_file_with_the_right_digest_is_left_alone() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("model.bin");
        fs::write(&path, b"abc").unwrap();
        let mut reported = Vec::new();

        // A bogus URL proves nothing was fetched.
        let installed = install(
            "https://example.invalid/model.bin",
            &path,
            3,
            Checksum::Sha1("a9993e364706816aba3e25717850c26c9cd0d89d"),
            "test model",
            |downloaded, total| reported.push((downloaded, total)),
        )
        .unwrap();

        assert_eq!(installed, path);
        assert_eq!(reported, vec![(3, 3)]);
    }
}
