use std::fs;
use std::time::{Duration, Instant};
use tempfile::tempdir;

use ferrum_file_search::scanner::{run_scan, CancellationToken, ScanConfig, ScanEvent};
use ferrum_file_search::search::filter_matches;

#[test]
fn test_stress_50k_files_traversal_and_search() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let subdirs = 200;
    let files_per_dir = 250;
    let total_expected = subdirs * files_per_dir; // 50,000 files

    println!("Creating 50,000 file fixture...");
    let creation_start = Instant::now();
    for d in 0..subdirs {
        let subdir = root.join(format!("dir_{d:03}"));
        fs::create_dir(&subdir).unwrap();
        for f in 0..files_per_dir {
            let filename = format!("item_{f:04}.dat");
            fs::write(subdir.join(filename), b"ferrum").unwrap();
        }
    }
    println!("Created 50,000 files in {:.2?}", creation_start.elapsed());

    let cancel = CancellationToken::new();
    let config = ScanConfig {
        batch_size: 500,
        flush_interval: Duration::from_millis(50),
        max_files: None,
    };

    let mut all_entries = Vec::with_capacity(total_expected);
    let mut batch_count = 0usize;
    let mut finished = false;

    let scan_start = Instant::now();
    run_scan(1, root, config, cancel, |event| {
        match event {
            ScanEvent::Batch { entries, .. } => {
                batch_count += 1;
                all_entries.extend(entries);
            }
            ScanEvent::Finished {
                total_scanned,
                duration,
                ..
            } => {
                finished = true;
                println!(
                    "Scan 50k finished in {:.2?} (worker reported: {:.2?}), total: {}, batches: {}",
                    scan_start.elapsed(),
                    duration,
                    total_scanned,
                    batch_count
                );
            }
            _ => {}
        }
        true
    });

    assert!(finished);
    assert_eq!(all_entries.len(), total_expected);

    let search_start = Instant::now();
    let matches = filter_matches(&all_entries, "item_0123");
    let search_dur = search_start.elapsed();

    assert_eq!(matches.len(), 200);
    println!("Search 50k entries for 'item_0123': {:.2?}", search_dur);
    assert!(search_dur < Duration::from_millis(100));
}

#[test]
fn test_stress_cancellation_midway() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    for d in 0..50 {
        let subdir = root.join(format!("sub_{d}"));
        fs::create_dir(&subdir).unwrap();
        for f in 0..100 {
            fs::write(subdir.join(format!("test_{f}.txt")), b"x").unwrap();
        }
    }

    let cancel = CancellationToken::new();
    let cancel_trigger = cancel.clone();

    let mut scanned_batches = 0usize;
    let mut total_received = 0usize;
    let mut was_cancelled = false;

    let config = ScanConfig {
        batch_size: 100,
        flush_interval: Duration::from_millis(10),
        max_files: None,
    };

    run_scan(99, root, config, cancel, |event| {
        match event {
            ScanEvent::Batch { entries, .. } => {
                scanned_batches += 1;
                total_received += entries.len();
                if scanned_batches >= 3 {
                    cancel_trigger.cancel();
                }
            }
            ScanEvent::Cancelled { total_scanned, .. } => {
                was_cancelled = true;
                println!(
                    "Cancelled scan: total received before cancel = {total_received}, worker reported = {total_scanned}"
                );
            }
            _ => {}
        }
        true
    });

    assert!(was_cancelled, "Scan should cleanly report Cancelled");
    assert!(
        total_received < 2000,
        "Scan should have stopped promptly after cancellation (received {total_received} of 5000)"
    );
}

#[test]
#[cfg(unix)]
fn test_inaccessible_descendant_recovery() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    fs::write(root.join("accessible_1.txt"), b"ok").unwrap();
    fs::write(root.join("accessible_2.txt"), b"ok").unwrap();

    let unreadable_dir = root.join("forbidden_dir");
    fs::create_dir(&unreadable_dir).unwrap();
    fs::write(unreadable_dir.join("secret.txt"), b"hidden").unwrap();

    fs::set_permissions(&unreadable_dir, fs::Permissions::from_mode(0o000)).unwrap();

    let cancel = CancellationToken::new();
    let mut total_scanned = 0usize;
    let mut reported_skipped = 0usize;
    let mut finished = false;

    run_scan(101, root, ScanConfig::default(), cancel, |event| {
        if let ScanEvent::Finished {
            total_scanned: t,
            skipped_count: s,
            ..
        } = event
        {
            total_scanned = t;
            reported_skipped = s;
            finished = true;
        }
        true
    });

    let _ = fs::set_permissions(&unreadable_dir, fs::Permissions::from_mode(0o755));

    assert!(
        finished,
        "Scan should finish without crashing on unreadable directory"
    );
    assert!(
        reported_skipped >= 1,
        "Should report at least 1 skipped item"
    );
    assert!(
        total_scanned >= 2,
        "Should index all accessible sibling files"
    );
}
