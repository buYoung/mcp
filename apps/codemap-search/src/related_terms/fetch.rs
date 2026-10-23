//! Verified dataset acquisition. Call only from a background OS thread or spawn_blocking;
//! the blocking HTTP client must not run on the sequential MCP runtime thread.
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy)]
pub struct DatasetSpec<'a> {
    pub id: &'a str,
    pub url: &'a str,
    pub size: u64,
    pub sha256: &'a str,
}

pub const PINNED_DATASET: DatasetSpec<'static> = DatasetSpec {
    id: "rts5-75e02862daa29308",
    url: "https://github.com/buYoung/mcp/releases/download/related-terms-data-rts5-75e02862daa29308/rts5-75e02862daa29308.tar.zst",
    size: 31_715_328,
    sha256: "90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced",
};

pub const MAX_DOWNLOAD_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TAR_OVERHEAD_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
    Ready { path: PathBuf, user_owned: bool },
    InProgress,
    Failed { reason: String },
}

/// Resolve a user source without touching it, or fetch a verified pinned copy.
/// The caller gates this API on `related_terms.enabled`. A cached final file is never
/// rehashed: preparation may already have changed its bytes by adding SQLite indexes.
/// InProgress means another process owns download.lock; the caller may retry later.
pub fn fetch_dataset(
    global_dir: &Path,
    dataset_path: Option<&Path>,
    spec: &DatasetSpec<'_>,
) -> FetchStatus {
    if let Some(path) = dataset_path {
        return FetchStatus::Ready {
            path: path.to_path_buf(),
            user_owned: true,
        };
    }
    match fetch_pinned(global_dir, spec) {
        Ok(status) => status,
        Err(error) => FetchStatus::Failed {
            reason: format!("{error:#}"),
        },
    }
}

fn fetch_pinned(global_dir: &Path, spec: &DatasetSpec<'_>) -> anyhow::Result<FetchStatus> {
    anyhow::ensure!(
        !spec.id.is_empty()
            && spec
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "invalid dataset id"
    );
    let directory = global_dir.join("related-terms").join(spec.id);
    let final_path = directory.join("dataset.sqlite3");
    if final_path.is_file() {
        return Ok(FetchStatus::Ready {
            path: final_path,
            user_owned: false,
        });
    }
    fs::create_dir_all(&directory)?;
    let lock = private_file(&directory.join("download.lock"), false)?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Ok(FetchStatus::InProgress),
        Err(error) => return Err(anyhow::anyhow!("cannot lock dataset download: {error}")),
    }
    // The lock stays owned until cleanup and publication finish, including on errors.
    let temporary = TemporaryFiles {
        archive: directory.join("download.tar.zst.part"),
        dataset: directory.join("dataset.sqlite3.part"),
    };
    temporary.clean()?;
    if final_path.is_file() {
        return Ok(FetchStatus::Ready {
            path: final_path,
            user_owned: false,
        });
    }
    download(spec, &temporary.archive)?;
    extract(spec, &temporary.archive, &temporary.dataset)?;
    fs::rename(&temporary.dataset, &final_path)?;
    Ok(FetchStatus::Ready {
        path: final_path,
        user_owned: false,
    })
}

fn private_file(path: &Path, create_new: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    if create_new {
        options.create_new(true);
    } else {
        options.create(true).truncate(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

struct TemporaryFiles {
    archive: PathBuf,
    dataset: PathBuf,
}

impl TemporaryFiles {
    fn clean(&self) -> io::Result<()> {
        for path in [&self.archive, &self.dataset] {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

impl Drop for TemporaryFiles {
    fn drop(&mut self) {
        // A crash or a failed cleanup is recovered under download.lock on the next attempt.
        let _ = self.clean();
    }
}

fn download(spec: &DatasetSpec<'_>, destination: &Path) -> anyhow::Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    // Jev requires credentials and disallows redirects; GitHub assets need redirects.
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::limited(5))
        .retry(reqwest::retry::never())
        .user_agent(concat!("codemap-search/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let response = client.get(spec.url).send()?.error_for_status()?;
    anyhow::ensure!(
        response
            .content_length()
            .is_none_or(|size| size <= MAX_DOWNLOAD_BYTES),
        "download exceeds byte limit ({MAX_DOWNLOAD_BYTES})"
    );
    let mut source = BoundedReader {
        inner: response,
        remaining: MAX_DOWNLOAD_BYTES,
        label: "download",
    };
    let mut file = private_file(destination, true)?;
    io::copy(&mut source, &mut file)?;
    file.sync_all()?;
    Ok(())
}

fn extract(spec: &DatasetSpec<'_>, archive_path: &Path, destination: &Path) -> anyhow::Result<()> {
    let mut decoder = zstd::stream::read::Decoder::new(File::open(archive_path)?)?;
    decoder.window_log_max(26)?;
    let source = BoundedReader {
        inner: decoder,
        remaining: spec
            .size
            .checked_add(MAX_TAR_OVERHEAD_BYTES)
            .ok_or_else(|| anyhow::anyhow!("invalid size bound"))?,
        label: "expanded archive",
    };
    let mut archive = tar::Archive::new(source);
    // Continue past end markers so appended entries cannot hide behind tar padding.
    archive.set_ignore_zeros(true);
    let mut seen = false;
    // Raw mode exposes extension headers instead of silently consuming arbitrary metadata.
    for entry in archive.entries()?.raw(true) {
        let mut entry = entry?;
        anyhow::ensure!(
            !seen
                && entry.header().entry_type().is_file()
                && entry.path_bytes().as_ref() == b"dataset.sqlite3",
            "archive must contain only one regular file named dataset.sqlite3"
        );
        seen = true;
        anyhow::ensure!(
            entry.size() <= spec.size,
            "dataset exceeds size bound ({})",
            spec.size
        );
        anyhow::ensure!(entry.size() == spec.size, "dataset size mismatch");
        let mut output = private_file(destination, true)?;
        let mut hasher = Sha256::new();
        let mut total = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = entry.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            total += count as u64;
            anyhow::ensure!(
                total <= spec.size,
                "dataset exceeds size bound ({})",
                spec.size
            );
            hasher.update(&buffer[..count]);
            output.write_all(&buffer[..count])?;
        }
        anyhow::ensure!(total == spec.size, "dataset size mismatch");
        let digest: String = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        anyhow::ensure!(digest == spec.sha256, "dataset SHA-256 mismatch");
        output.sync_all()?;
    }
    anyhow::ensure!(seen, "archive has no dataset.sqlite3");
    // Validate the complete zstd frame (including its checksum), not just the tar payload.
    io::copy(&mut archive.into_inner(), &mut io::sink())?;
    Ok(())
}

struct BoundedReader<R> {
    inner: R,
    remaining: u64,
    label: &'static str,
}

impl<R: Read> Read for BoundedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let limit = (buffer.len() as u64).min(self.remaining.saturating_add(1)) as usize;
        let count = self.inner.read(&mut buffer[..limit])?;
        if count as u64 > self.remaining {
            return Err(io::Error::other(format!(
                "{} exceeds byte limit",
                self.label
            )));
        }
        self.remaining -= count as u64;
        Ok(count)
    }
}
