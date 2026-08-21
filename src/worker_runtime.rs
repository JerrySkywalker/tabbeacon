//! Immutable, user-local executable images for long-lived activity workers.
//!
//! The public `tabbeacon` executable remains the one-shot entrypoint.  This
//! module only copies that already-running, local executable into a
//! content-addressed state directory before a worker is spawned.  It never
//! downloads, updates, or chooses another executable.

use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use sha2::{Digest, Sha256};

const RUNTIME_DIRECTORY: &str = "runtime";
const WORKER_IMAGES_DIRECTORY: &str = "worker-images";
const WORKER_IMAGE_FILE: &str = "tabbeacon-worker.exe";
const MAX_RUNTIME_IMAGES: usize = 128;
// Long-lived worker publication happens on the provider Hook path. A release
// executable is far smaller than this cap; rejecting an unexpectedly large
// source keeps hashing/copying bounded rather than turning a Hook into a bulk
// file-transfer mechanism.
const MAX_RUNTIME_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
static TEMPORARY_IMAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Immutable image selected for one long-lived worker invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerRuntimeImage {
    /// SHA-256 of both the source executable and the published image bytes.
    pub content_sha256: String,
    /// Exact executable path that the long-lived worker must execute.
    pub executable: PathBuf,
}

/// Bounded result of opportunistic runtime-image collection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeImageGcReport {
    /// Number of immutable images removed after ownership proof.
    pub removed: usize,
    /// Number retained because an active worker lease still names the image.
    pub retained_active: usize,
    /// Number retained because proof, contents, or deletion was unsafe.
    pub retained_unsafe: usize,
}

/// Read-only inventory used by upgrade diagnostics before any cleanup attempt.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeImageInspection {
    /// Every hash-addressed image whose directory and contents were verified.
    pub(crate) image_hashes: BTreeSet<String>,
    /// False when the image root or a hash-addressed image could not be safely
    /// inspected. Callers must retain images rather than infer ownership.
    pub(crate) healthy: bool,
}

/// State-root scoped publisher and conservative collector.
#[derive(Clone, Debug)]
pub struct WorkerRuntimeStore {
    state_root: PathBuf,
}

impl WorkerRuntimeStore {
    /// Binds image storage to the existing per-user `TabBeacon` state root.
    #[must_use]
    pub fn new(state_root: impl Into<PathBuf>) -> Self {
        Self {
            state_root: state_root.into(),
        }
    }

    /// Publishes a verified immutable image or reuses an identical one.
    ///
    /// The source must be a regular local file.  Every destination component
    /// is checked for symbolic-link redirection; readers see the executable
    /// only after the temporary image has been completely copied, synced, and
    /// renamed into its hash directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the source, destination chain, existing image,
    /// or copied bytes cannot be verified as the exact local executable.
    pub fn publish(&self, source: &Path) -> io::Result<WorkerRuntimeImage> {
        let source = regular_file(source)?;
        let source_hash = file_sha256(&source)?;
        let images = self.images_root()?;
        let images_canonical = fs::canonicalize(&images)?;
        if source.starts_with(&images_canonical) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "worker runtime source cannot be a runtime image",
            ));
        }

        let image_directory = images.join(&source_hash);
        ensure_directory(&image_directory)?;
        let image = image_directory.join(WORKER_IMAGE_FILE);
        if image.exists() {
            verify_image(&image, &source_hash)?;
            return Ok(WorkerRuntimeImage {
                content_sha256: source_hash,
                executable: image,
            });
        }

        let temporary = image_directory.join(format!(
            ".{WORKER_IMAGE_FILE}.{}-{}.tmp",
            std::process::id(),
            TEMPORARY_IMAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut input = File::open(&source)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            io::copy(&mut input, &mut output)?;
            output.flush()?;
            output.sync_all()?;
            if file_sha256(&temporary)? != source_hash || file_sha256(&source)? != source_hash {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "worker runtime source changed during publication",
                ));
            }
            match fs::rename(&temporary, &image) {
                Ok(()) => verify_image(&image, &source_hash),
                Err(error) if image.exists() => {
                    verify_image(&image, &source_hash).map_err(|_| error)
                }
                Err(error) => Err(error),
            }
        })();
        let _ = fs::remove_file(&temporary);
        result?;
        Ok(WorkerRuntimeImage {
            content_sha256: source_hash,
            executable: image,
        })
    }

    /// Removes only hash-verified images not named by a proven active lease.
    ///
    /// When lease inspection is incomplete, collection is a no-op.  Failed
    /// deletion is deliberately retention, never an error for a Hook path.
    #[must_use]
    pub fn collect_unused(
        &self,
        active_image_hashes: &BTreeSet<String>,
        ownership_proven: bool,
    ) -> RuntimeImageGcReport {
        let mut report = RuntimeImageGcReport::default();
        if !ownership_proven {
            report.retained_unsafe = self.image_directories().map_or(0, |images| images.len());
            return report;
        }
        let Ok(images) = self.image_directories() else {
            return report;
        };
        for (hash, directory) in images {
            if active_image_hashes.contains(&hash) {
                report.retained_active = report.retained_active.saturating_add(1);
                continue;
            }
            let image = directory.join(WORKER_IMAGE_FILE);
            if verify_image(&image, &hash).is_err() || fs::remove_dir_all(&directory).is_err() {
                report.retained_unsafe = report.retained_unsafe.saturating_add(1);
            } else {
                report.removed = report.removed.saturating_add(1);
            }
        }
        report
    }

    /// Inspects images without creating directories or changing runtime state.
    #[must_use]
    pub(crate) fn inspect_read_only(&self) -> RuntimeImageInspection {
        let images = match self.images_root_read_only() {
            Ok(Some(images)) => images,
            Ok(None) => {
                return RuntimeImageInspection {
                    image_hashes: BTreeSet::new(),
                    healthy: true,
                };
            }
            Err(_) => {
                return RuntimeImageInspection {
                    image_hashes: BTreeSet::new(),
                    healthy: false,
                };
            }
        };
        let mut inspection = RuntimeImageInspection {
            image_hashes: BTreeSet::new(),
            healthy: true,
        };
        let Ok(entries) = fs::read_dir(images) else {
            inspection.healthy = false;
            return inspection;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                inspection.healthy = false;
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_sha256(&name) {
                continue;
            }
            if inspection.image_hashes.len() >= MAX_RUNTIME_IMAGES {
                inspection.healthy = false;
                break;
            }
            let directory = entry.path();
            let valid_directory = fs::symlink_metadata(&directory)
                .ok()
                .is_some_and(|metadata| !metadata.file_type().is_symlink() && metadata.is_dir());
            if !valid_directory || verify_image(&directory.join(WORKER_IMAGE_FILE), &name).is_err()
            {
                inspection.healthy = false;
                continue;
            }
            inspection.image_hashes.insert(name);
        }
        inspection
    }

    fn images_root(&self) -> io::Result<PathBuf> {
        ensure_directory(&self.state_root)?;
        let runtime = self.state_root.join(RUNTIME_DIRECTORY);
        ensure_directory(&runtime)?;
        let images = runtime.join(WORKER_IMAGES_DIRECTORY);
        ensure_directory(&images)?;
        Ok(images)
    }

    fn images_root_read_only(&self) -> io::Result<Option<PathBuf>> {
        let runtime = self.state_root.join(RUNTIME_DIRECTORY);
        let images = runtime.join(WORKER_IMAGES_DIRECTORY);
        for path in [&self.state_root, &runtime, &images] {
            match fs::symlink_metadata(path) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "worker runtime directory is redirected or not a directory",
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error),
            }
        }
        Ok(Some(images))
    }

    fn image_directories(&self) -> io::Result<Vec<(String, PathBuf)>> {
        let images = self.images_root()?;
        let mut result = Vec::new();
        for entry in fs::read_dir(images)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_sha256(&name) || result.len() >= MAX_RUNTIME_IMAGES {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                continue;
            }
            result.push((name, entry.path()));
        }
        result.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(result)
    }
}

fn ensure_directory(path: &Path) -> io::Result<()> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "worker runtime directory is redirected or not a directory",
            ));
        }
        return Ok(());
    }
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "worker runtime directory has no parent",
        )
    })?;
    if parent != path {
        ensure_directory(parent)?;
    }
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if path.exists() => ensure_directory(path).map_err(|_| error),
        Err(error) => Err(error),
    }
}

fn regular_file(path: &Path) -> io::Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_RUNTIME_IMAGE_BYTES
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "worker runtime source is redirected or not a file",
        ));
    }
    fs::canonicalize(path)
}

fn verify_image(path: &Path, expected_hash: &str) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_RUNTIME_IMAGE_BYTES
        || file_sha256(path)? != expected_hash
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "worker runtime image is missing, redirected, or mismatched",
        ));
    }
    Ok(())
}

fn file_sha256(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 32 * 1024].into_boxed_slice();
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use std::{fs, thread};

    use super::{File, MAX_RUNTIME_IMAGE_BYTES, WorkerRuntimeStore};

    fn root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tabbeacon-worker-runtime-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock is available")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root is created");
        root
    }

    #[test]
    fn publishes_atomically_reuses_verified_content_and_collects_only_unowned_images() {
        let root = root("publish");
        let source = root.join("installed.exe");
        fs::write(&source, b"release-one").expect("source is written");
        let store = WorkerRuntimeStore::new(root.join("state"));
        let image = store.publish(&source).expect("image publishes");
        assert!(image.executable.is_file());
        assert_eq!(store.publish(&source).expect("image reuses"), image);
        let withheld = store.collect_unused(&std::collections::BTreeSet::new(), false);
        assert_eq!(withheld.removed, 0);
        assert_eq!(withheld.retained_unsafe, 1);
        assert!(image.executable.is_file());
        let protected = std::collections::BTreeSet::from([image.content_sha256.clone()]);
        assert_eq!(store.collect_unused(&protected, true).retained_active, 1);
        let report = store.collect_unused(&std::collections::BTreeSet::new(), true);
        assert_eq!(report.removed, 1);
        assert!(!image.executable.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_mismatched_existing_image_and_concurrent_publication_is_safe() {
        let root = root("concurrent");
        let source = root.join("installed.exe");
        fs::write(&source, b"release-two").expect("source is written");
        let store = WorkerRuntimeStore::new(root.join("state"));
        let first = store.publish(&source).expect("initial image publishes");
        fs::write(&first.executable, b"tampered").expect("image is tampered for test");
        assert!(store.publish(&source).is_err());
        let _ = fs::remove_dir_all(root.join("state"));
        let store = WorkerRuntimeStore::new(root.join("state"));
        let mut workers = Vec::new();
        for _ in 0..4 {
            let store = store.clone();
            let source = source.clone();
            workers.push(thread::spawn(move || store.publish(&source)));
        }
        let images = workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .expect("publisher joins")
                    .expect("publisher succeeds")
            })
            .collect::<Vec<_>>();
        assert!(images.windows(2).all(|pair| pair[0] == pair[1]));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_an_unbounded_source_before_copying_or_hashing() {
        let root = root("oversized");
        let source = root.join("unexpectedly-large.exe");
        File::create(&source)
            .expect("source creates")
            .set_len(MAX_RUNTIME_IMAGE_BYTES.saturating_add(1))
            .expect("sparse source grows for the bounded test");
        let store = WorkerRuntimeStore::new(root.join("state"));
        assert!(store.publish(&source).is_err());
        assert!(!root.join("state").exists());
        let _ = fs::remove_dir_all(root);
    }
}
