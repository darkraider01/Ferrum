use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Cancellation token shared between UI and scanner worker.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// A scanned file entry with its path relative to the scan root.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScannedEntry {
    /// Relative path from root (for display and search).
    pub relative_path: String,
    /// Base filename (for fast search filtering).
    pub filename: String,
}

/// Events emitted by the background scanner.
#[derive(Clone, Debug)]
pub enum ScanEvent {
    /// Root directory validated, scan started.
    Started { scan_id: u64, root: PathBuf },
    /// A batch of scanned files.
    Batch {
        scan_id: u64,
        entries: Vec<ScannedEntry>,
        total_scanned: usize,
    },
    /// Status progress tick (e.g. while traversing deep directories with few matching files).
    Progress {
        scan_id: u64,
        total_scanned: usize,
        skipped_count: usize,
    },
    /// Scan completed successfully.
    Finished {
        scan_id: u64,
        total_scanned: usize,
        skipped_count: usize,
        duration: Duration,
    },
    /// Scan cancelled cooperatively.
    Cancelled {
        scan_id: u64,
        total_scanned: usize,
        skipped_count: usize,
        duration: Duration,
    },
    /// Root directory or unrecoverable error.
    Error { scan_id: u64, error: ScanError },
}

impl ScanEvent {
    pub fn scan_id(&self) -> u64 {
        match self {
            Self::Started { scan_id, .. } => *scan_id,
            Self::Batch { scan_id, .. } => *scan_id,
            Self::Progress { scan_id, .. } => *scan_id,
            Self::Finished { scan_id, .. } => *scan_id,
            Self::Cancelled { scan_id, .. } => *scan_id,
            Self::Error { scan_id, .. } => *scan_id,
        }
    }
}

/// Errors encountered while setting up or executing a scan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanError {
    EmptyPath,
    NotFound(String),
    NotADirectory(String),
    PermissionDenied(String),
    Io(String),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPath => write!(f, "Directory path cannot be empty"),
            Self::NotFound(p) => write!(f, "Directory does not exist: {p}"),
            Self::NotADirectory(p) => write!(f, "Path is not a directory: {p}"),
            Self::PermissionDenied(p) => write!(f, "Permission denied accessing directory: {p}"),
            Self::Io(msg) => write!(f, "Filesystem error: {msg}"),
        }
    }
}

/// Configuration options for the file scanner.
#[derive(Clone, Debug)]
pub struct ScanConfig {
    pub batch_size: usize,
    pub flush_interval: Duration,
    pub max_files: Option<usize>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            batch_size: 256,
            flush_interval: Duration::from_millis(50),
            max_files: None,
        }
    }
}

/// Validates the root directory before launching scan work.
pub fn validate_root(raw_path: &str) -> Result<PathBuf, ScanError> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(ScanError::EmptyPath);
    }

    let path = PathBuf::from(trimmed);
    let metadata = match fs::metadata(&path) {
        Ok(m) => m,
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => return Err(ScanError::NotFound(trimmed.to_string())),
            std::io::ErrorKind::PermissionDenied => {
                return Err(ScanError::PermissionDenied(trimmed.to_string()))
            }
            _ => return Err(ScanError::Io(err.to_string())),
        },
    };

    if !metadata.is_dir() {
        return Err(ScanError::NotADirectory(trimmed.to_string()));
    }

    Ok(path)
}

/// Synchronous scan loop executed on a background worker thread.
///
/// Sends progress and batches over `sender`. Stops immediately if `cancel` is set
/// or if `sender` fails (indicating receiver / UI was dropped).
pub fn run_scan<F>(
    scan_id: u64,
    root: PathBuf,
    config: ScanConfig,
    cancel: CancellationToken,
    mut emit: F,
) where
    F: FnMut(ScanEvent) -> bool, // returns false if receiver disconnected
{
    let start_time = Instant::now();

    // Check cancellation before doing anything
    if cancel.is_cancelled() {
        emit(ScanEvent::Cancelled {
            scan_id,
            total_scanned: 0,
            skipped_count: 0,
            duration: Duration::ZERO,
        });
        return;
    }

    let root_meta = match fs::metadata(&root) {
        Ok(m) => m,
        Err(err) => {
            let scan_err = match err.kind() {
                std::io::ErrorKind::NotFound => {
                    ScanError::NotFound(root.to_string_lossy().to_string())
                }
                std::io::ErrorKind::PermissionDenied => {
                    ScanError::PermissionDenied(root.to_string_lossy().to_string())
                }
                _ => ScanError::Io(err.to_string()),
            };
            emit(ScanEvent::Error {
                scan_id,
                error: scan_err,
            });
            return;
        }
    };

    if !root_meta.is_dir() {
        emit(ScanEvent::Error {
            scan_id,
            error: ScanError::NotADirectory(root.to_string_lossy().to_string()),
        });
        return;
    }

    if !emit(ScanEvent::Started {
        scan_id,
        root: root.clone(),
    }) {
        return;
    }

    let mut stack: Vec<PathBuf> = vec![root.clone()];
    let mut batch: Vec<ScannedEntry> = Vec::with_capacity(config.batch_size);
    let mut total_scanned = 0usize;
    let mut skipped_count = 0usize;
    let mut last_flush = Instant::now();

    while let Some(current_dir) = stack.pop() {
        if cancel.is_cancelled() {
            emit(ScanEvent::Cancelled {
                scan_id,
                total_scanned,
                skipped_count,
                duration: start_time.elapsed(),
            });
            return;
        }

        let read_dir_result = fs::read_dir(&current_dir);
        let entries = match read_dir_result {
            Ok(entries) => entries,
            Err(_) => {
                // Inaccessible descendant: continue without failing scan
                skipped_count += 1;
                continue;
            }
        };

        for entry_res in entries {
            if cancel.is_cancelled() {
                emit(ScanEvent::Cancelled {
                    scan_id,
                    total_scanned,
                    skipped_count,
                    duration: start_time.elapsed(),
                });
                return;
            }

            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => {
                    skipped_count += 1;
                    continue;
                }
            };

            let entry_path = entry.path();

            // Check symlink metadata to enforce the rule: DO NOT follow directory symlinks
            let symlink_meta = match fs::symlink_metadata(&entry_path) {
                Ok(m) => m,
                Err(_) => {
                    skipped_count += 1;
                    continue;
                }
            };

            let is_symlink = symlink_meta.file_type().is_symlink();
            let is_dir = symlink_meta.is_dir();

            if is_dir && !is_symlink {
                // Recurse into real directory, but do not record directories in the file search results
                stack.push(entry_path);
                continue;
            }

            // Record entry
            let relative = match entry_path.strip_prefix(&root) {
                Ok(rel) => rel.to_string_lossy().to_string(),
                Err(_) => entry_path.to_string_lossy().to_string(),
            };

            let filename = entry.file_name().to_string_lossy().to_string();

            batch.push(ScannedEntry {
                relative_path: relative,
                filename,
            });
            total_scanned += 1;

            if let Some(max) = config.max_files {
                if total_scanned >= max {
                    break;
                }
            }

            // Flush batch if size reached or interval elapsed
            let now = Instant::now();
            if batch.len() >= config.batch_size
                || now.duration_since(last_flush) >= config.flush_interval
            {
                if !batch.is_empty() {
                    let entries =
                        std::mem::replace(&mut batch, Vec::with_capacity(config.batch_size));
                    if !emit(ScanEvent::Batch {
                        scan_id,
                        entries,
                        total_scanned,
                    }) {
                        return; // Receiver disconnected
                    }
                } else {
                    if !emit(ScanEvent::Progress {
                        scan_id,
                        total_scanned,
                        skipped_count,
                    }) {
                        return;
                    }
                }
                last_flush = now;
            }
        }

        if let Some(max) = config.max_files {
            if total_scanned >= max {
                break;
            }
        }
    }

    // Flush any remaining items in the final batch
    if !batch.is_empty()
        && !emit(ScanEvent::Batch {
            scan_id,
            entries: batch,
            total_scanned,
        })
    {
        return;
    }

    emit(ScanEvent::Finished {
        scan_id,
        total_scanned,
        skipped_count,
        duration: start_time.elapsed(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_validate_root_valid() {
        let dir = tempdir().unwrap();
        let res = validate_root(dir.path().to_str().unwrap());
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), dir.path());
    }

    #[test]
    fn test_validate_root_empty() {
        let res = validate_root("   ");
        assert_eq!(res, Err(ScanError::EmptyPath));
    }

    #[test]
    fn test_validate_root_not_found() {
        let res = validate_root("/path/that/definitely/does/not/exist_12345");
        match res {
            Err(ScanError::NotFound(_)) => {}
            other => panic!("Expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn test_validate_root_not_a_directory() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("a_file.txt");
        fs::write(&file_path, "contents").unwrap();

        let res = validate_root(file_path.to_str().unwrap());
        match res {
            Err(ScanError::NotADirectory(_)) => {}
            other => panic!("Expected NotADirectory, got {other:?}"),
        }
    }

    #[test]
    fn test_scan_traversal_and_relative_paths() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        // Create structure:
        // root/
        //   file1.txt
        //   sub1/
        //     file2.txt
        //     sub2/
        //       file3.txt
        fs::write(root.join("file1.txt"), "1").unwrap();
        fs::create_dir_all(root.join("sub1").join("sub2")).unwrap();
        fs::write(root.join("sub1").join("file2.txt"), "2").unwrap();
        fs::write(root.join("sub1").join("sub2").join("file3.txt"), "3").unwrap();

        let mut entries = Vec::new();
        let mut finished = false;

        let cancel = CancellationToken::new();
        let config = ScanConfig {
            batch_size: 10,
            flush_interval: Duration::from_secs(1),
            max_files: None,
        };

        run_scan(1, root, config, cancel, |event| {
            match event {
                ScanEvent::Batch { entries: b, .. } => entries.extend(b),
                ScanEvent::Finished { .. } => finished = true,
                _ => {}
            }
            true
        });

        assert!(finished);
        let mut rels: Vec<String> = entries.into_iter().map(|e| e.relative_path).collect();
        rels.sort();

        assert!(rels.contains(&"file1.txt".to_string()));
        assert!(rels.contains(&format!("sub1{}file2.txt", std::path::MAIN_SEPARATOR)));
        assert!(rels.contains(&format!(
            "sub1{}sub2{}file3.txt",
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR
        )));
    }

    #[test]
    #[cfg(unix)]
    fn test_symlink_directory_not_followed() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        // Create real directory with files:
        let real_dir = root.join("real_dir");
        fs::create_dir(&real_dir).unwrap();
        fs::write(real_dir.join("inner.txt"), "content").unwrap();

        // Create a symlink pointing to real_dir inside root:
        let symlink_dir = root.join("symlink_to_real");
        symlink(&real_dir, &symlink_dir).unwrap();

        let mut entries = Vec::new();
        let cancel = CancellationToken::new();
        let config = ScanConfig::default();

        run_scan(1, root, config, cancel, |event| {
            if let ScanEvent::Batch { entries: b, .. } = event {
                entries.extend(b);
            }
            true
        });

        // The symlink entry itself may appear, but its children through the symlink must NOT:
        let symlinked_child = format!("symlink_to_real{}inner.txt", std::path::MAIN_SEPARATOR);
        assert!(
            !entries.iter().any(|e| e.relative_path == symlinked_child),
            "Directory symlink was traversed, violating the symlink policy!"
        );
    }

    #[test]
    fn test_cancellation_stops_scan() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        for i in 0..50 {
            fs::write(root.join(format!("file_{i}.txt")), "data").unwrap();
        }

        let cancel = CancellationToken::new();
        // Cancel immediately before run
        cancel.cancel();

        let mut was_cancelled = false;
        run_scan(1, root, ScanConfig::default(), cancel, |event| {
            if let ScanEvent::Cancelled { .. } = event {
                was_cancelled = true;
            }
            true
        });

        assert!(
            was_cancelled,
            "Scan should report Cancelled when token is set"
        );
    }
}
