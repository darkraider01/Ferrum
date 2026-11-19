# Ferrum Feasibility Experiment: Native File Search

## Running the application

From the repository root:

```bash
cargo run --bin ferrum-file-search
cargo test
```

The experiment uses Xilem 0.4.0 and Masonry 0.4.0 through the versions in `Cargo.lock`. It needs a working desktop session and the system libraries required by those dependencies.

## Implemented behavior

Enter a directory and start a scan. The application scans filenames on a worker thread, displays progress, and filters results by a case-insensitive substring. An empty query matches every entry; queries containing a path separator match the relative path. Results are sorted by relative path. Directory symlinks are not traversed.

Only one scan can be active. Cancellation is cooperative: it cannot interrupt a filesystem operation already in progress. Closing the window requests cancellation without joining the worker on the UI thread. Errors opening the root are reported; inaccessible descendants are counted and skipped.

The worker sends batches through a bounded channel. The forwarding task waits for the UI to acknowledge each event before forwarding another, bounding outstanding event-loop messages from this scan. Cancellation drops the receiver when the task view is removed, releasing a worker blocked on the channel. A batch filters only its new entries and merges matching indices into the existing sorted results. Changing the query still filters and sorts the full collection on the UI thread; responsiveness with large result sets has not been established.

The default result view uses Masonry's `VirtualScroll`. A portal mode renders at most 500 entries for comparison. Neither mode establishes complete accessibility for large result sets.

## Verification and limitations

The automated tests cover filtering, ordering, traversal, cancellation, and filesystem errors. Run `cargo test` for the current results. The stress fixture exercises scanning and filtering 50,000 files, but does not measure UI responsiveness or queue growth during an interactive scan.

On 20 November 2026, the 13 unit tests and three other integration tests passed. The 50,000-file test hit a quota when its fixture was created under `/tmp`; rerunning it with `TMPDIR` set to a temporary directory in the repository succeeded. In that single debug run, the scanner reported 50,000 files in 192.64 ms, and one filter took 12.36 ms. These are fixture timings, not interactive UI or comparative benchmarks. The temporary directory was removed after the test.

A brief desktop launch created a window, as recorded by the application log. The process was stopped by a 15-second timeout; this does not verify window-close behavior. Interactive scanning, responsiveness, cancellation, IME, and screen-reader interactions were not verified in that launch.

An earlier run reported window creation and an AT-SPI connection on Fedora Linux with KDE Wayland. That does not establish usable screen-reader output. IME composition and a screen reader have not been manually verified.

The earlier report recorded a 132.30 ms scan and an 8.59 ms filter for a 50,000-file fixture, a 25 MB release binary, 91 MB peak release RSS, approximately 115 ms to window creation, and idle CPU below 0.1%. These are historical single-environment observations, not measurements of the current changes or comparisons with an equivalent Electron application. The original report attributed memory measurements to `/usr/bin/time -v` and startup timing to tracing logs; the detailed measurement artifacts are not included here.

To verify accessibility, run the application in a desktop session with an active screen reader and IME. Check that the directory and query inputs have usable accessible names and values, focus and editing actions work, preedit and committed text behave as expected, and results can be navigated. Record the toolkit version, platform, assistive technology, and observed behavior.

## Toolkit findings

- Xilem 0.4.0's window-close path contains a TODO about view teardown. The application explicitly requests cancellation in its close callback. The implications for other applications and whether an upstream change would be appropriate need further investigation.
- Masonry 0.4.0's `VirtualScroll` documentation describes missing scrollbar integration and unresolved accessibility and empty-range behavior. This experiment uses a placeholder for an empty range; it does not establish that the resulting list is accessible.
- Masonry 0.4.0's text-area source submits a changed action for nonempty IME preedit. The effect on the application needs an actual IME test before drawing conclusions about search behavior or proposing a fix.
- The text inputs have adjacent visible labels. Their programmatic association with the inputs has not been verified; visual proximity alone does not establish accessible names.

## Recommendation

Continue evaluating the reference application and its lifecycle behavior on Xilem/Masonry. Compare specific unmet requirements with Floem and GPUI before choosing a toolkit or introducing a separate lifecycle layer. The current experiment does not prove that an independent Ferrum supervisor is necessary, or that alternative frameworks have the same limitations.
