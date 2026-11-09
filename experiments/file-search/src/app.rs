use std::path::PathBuf;

use xilem::core::fork;
use xilem::core::one_of::{Either, OneOf3};
use xilem::view::{
    flex_col, flex_row, label, portal, sized_box, task_raw, text_button, text_input,
    virtual_scroll, CrossAxisAlignment, MainAxisAlignment,
};
use xilem::WidgetView;

use crate::scanner::{self, CancellationToken, ScanConfig, ScanEvent, ScannedEntry};
use crate::search;

/// List rendering mode to evaluate and compare toolkit behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ListMode {
    #[default]
    VirtualScroll,
    PortalFlex,
}

pub struct AppState {
    pub root_path_input: String,
    pub query_input: String,
    pub is_scanning: bool,
    pub current_scan_id: u64,
    pub cancel_token: CancellationToken,
    pub scanned_entries: Vec<ScannedEntry>,
    pub matching_indices: Vec<usize>,
    pub total_scanned: usize,
    pub skipped_count: usize,
    pub status_message: String,
    pub error_message: Option<String>,
    pub list_mode: ListMode,
}

impl Default for AppState {
    fn default() -> Self {
        let default_path = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        Self {
            root_path_input: default_path,
            query_input: String::new(),
            is_scanning: false,
            current_scan_id: 0,
            cancel_token: CancellationToken::new(),
            scanned_entries: Vec::new(),
            matching_indices: Vec::new(),
            total_scanned: 0,
            skipped_count: 0,
            status_message: "Ready. Click 'Start Scan' to begin.".to_string(),
            error_message: None,
            list_mode: ListMode::VirtualScroll,
        }
    }
}

impl AppState {
    pub fn start_scan(&mut self) {
        // Cancel any existing active scan
        self.cancel_token.cancel();

        self.error_message = None;

        // Validate root directory
        let validated_root = match scanner::validate_root(&self.root_path_input) {
            Ok(p) => p,
            Err(err) => {
                self.error_message = Some(err.to_string());
                self.status_message = "Scan aborted: invalid root directory.".to_string();
                self.is_scanning = false;
                return;
            }
        };

        self.current_scan_id = self.current_scan_id.wrapping_add(1);
        self.cancel_token = CancellationToken::new();
        self.scanned_entries.clear();
        self.matching_indices.clear();
        self.total_scanned = 0;
        self.skipped_count = 0;
        self.is_scanning = true;
        self.status_message = format!("Scanning: {}", validated_root.display());
    }

    pub fn cancel_scan(&mut self) {
        if self.is_scanning {
            self.cancel_token.cancel();
            self.is_scanning = false;
            self.status_message = format!(
                "Scan cancelled by user. Indexed {} files (skipped {}).",
                self.total_scanned, self.skipped_count
            );
        }
    }

    pub fn update_matches(&mut self) {
        self.matching_indices = search::filter_matches(&self.scanned_entries, &self.query_input);
    }

    pub fn on_scan_event(&mut self, event: ScanEvent) {
        // Discard events from obsolete/superseded scans
        if event.scan_id() != self.current_scan_id {
            return;
        }

        match event {
            ScanEvent::Started { root, .. } => {
                self.status_message = format!("Scanning: {}", root.display());
            }
            ScanEvent::Batch {
                entries,
                total_scanned,
                ..
            } => {
                self.scanned_entries.extend(entries);
                self.total_scanned = total_scanned;
                self.update_matches();
                self.status_message = format!("Scanning... {} files found", self.total_scanned);
            }
            ScanEvent::Progress {
                total_scanned,
                skipped_count,
                ..
            } => {
                self.total_scanned = total_scanned;
                self.skipped_count = skipped_count;
                self.status_message = format!(
                    "Scanning... {} files found ({} skipped)",
                    self.total_scanned, self.skipped_count
                );
            }
            ScanEvent::Finished {
                total_scanned,
                skipped_count,
                duration,
                ..
            } => {
                self.is_scanning = false;
                self.total_scanned = total_scanned;
                self.skipped_count = skipped_count;
                self.update_matches();
                self.status_message = format!(
                    "Scan completed: {} files in {:.2?} ({} inaccessible skipped)",
                    total_scanned, duration, skipped_count
                );
            }
            ScanEvent::Cancelled {
                total_scanned,
                skipped_count,
                duration,
                ..
            } => {
                self.is_scanning = false;
                self.total_scanned = total_scanned;
                self.skipped_count = skipped_count;
                self.update_matches();
                self.status_message = format!(
                    "Scan cancelled after {:.2?}: {} files indexed",
                    duration, total_scanned
                );
            }
            ScanEvent::Error { error, .. } => {
                self.is_scanning = false;
                self.error_message = Some(error.to_string());
                self.status_message = "Scan encountered an error.".to_string();
            }
        }
    }

    pub fn toggle_list_mode(&mut self) {
        self.list_mode = match self.list_mode {
            ListMode::VirtualScroll => ListMode::PortalFlex,
            ListMode::PortalFlex => ListMode::VirtualScroll,
        };
    }
}

/// Helper to render an individual result item in virtual scroll.
fn virtual_result_item(state: &mut AppState, idx: i64) -> impl WidgetView<AppState> + use<> {
    let index = idx as usize;
    let label_text = if let Some(&entry_idx) = state.matching_indices.get(index) {
        if let Some(entry) = state.scanned_entries.get(entry_idx) {
            format!("{}. {}", index + 1, entry.relative_path)
        } else {
            format!("{}. <invalid>", index + 1)
        }
    } else {
        String::new()
    };

    sized_box(label(label_text))
}

pub fn app_logic(state: &mut AppState) -> impl WidgetView<AppState> + use<> {
    // 1. Directory Input and Controls Row
    let dir_label = label("Scan Directory:");
    let dir_input = text_input(
        state.root_path_input.clone(),
        |state: &mut AppState, val| {
            state.root_path_input = val;
        },
    )
    .placeholder("Enter directory path...");

    let scan_button = if state.is_scanning {
        Either::A(text_button("Scanning...", |_| {}))
    } else {
        Either::B(text_button("Start Scan", |state: &mut AppState| {
            state.start_scan();
        }))
    };

    let cancel_button = text_button("Cancel", |state: &mut AppState| {
        state.cancel_scan();
    });

    let current_dir_button = text_button("Use Current (.)", |state: &mut AppState| {
        if let Ok(cwd) = std::env::current_dir() {
            state.root_path_input = cwd.to_string_lossy().to_string();
        } else {
            state.root_path_input = ".".to_string();
        }
        state.start_scan();
    });

    let controls_row = flex_row((
        dir_label,
        dir_input,
        scan_button,
        cancel_button,
        current_dir_button,
    ));

    // 2. Search Query Row
    let query_label = label("Search Query:");
    let query_input = text_input(state.query_input.clone(), |state: &mut AppState, val| {
        state.query_input = val;
        state.update_matches();
    })
    .placeholder("Filter filenames (case-insensitive substring)...");

    let clear_query_button = text_button("Clear Query", |state: &mut AppState| {
        state.query_input.clear();
        state.update_matches();
    });

    let mode_toggle_button = text_button(
        match state.list_mode {
            ListMode::VirtualScroll => "Mode: VirtualScroll (Fast)",
            ListMode::PortalFlex => "Mode: Portal (Standard)",
        },
        |state: &mut AppState| {
            state.toggle_list_mode();
        },
    );

    let query_row = flex_row((
        query_label,
        query_input,
        clear_query_button,
        mode_toggle_button,
    ));

    // 3. Status and Error Section
    let status_line = label(format!("Status: {}", state.status_message));
    let counts_line = label(format!(
        "Matching: {}  |  Total Scanned: {}  |  Skipped Inaccessible: {}",
        state.matching_indices.len(),
        state.total_scanned,
        state.skipped_count
    ));

    let error_line = state
        .error_message
        .as_ref()
        .map(|err| label(format!("ERROR: {err}")));

    // 4. Results List View
    let results_count = state.matching_indices.len();
    let results_view = if results_count == 0 {
        OneOf3::A(if state.total_scanned == 0 {
            label("No files scanned yet. Select a directory and click 'Start Scan'.")
        } else {
            label("No matching files found for the current query.")
        })
    } else {
        match state.list_mode {
            ListMode::VirtualScroll => {
                OneOf3::B(virtual_scroll(0..results_count as i64, virtual_result_item))
            }
            ListMode::PortalFlex => {
                // Show up to 500 items in standard portal mode to prevent GUI freeze
                const MAX_PORTAL_ITEMS: usize = 500;
                let displayed_count = results_count.min(MAX_PORTAL_ITEMS);
                let items: Vec<_> = state.matching_indices[0..displayed_count]
                    .iter()
                    .enumerate()
                    .map(|(i, &entry_idx)| {
                        let path = &state.scanned_entries[entry_idx].relative_path;
                        label(format!("{}. {}", i + 1, path))
                    })
                    .collect();

                let footer = (results_count > MAX_PORTAL_ITEMS).then(|| {
                    label(format!(
                        "... (showing first {MAX_PORTAL_ITEMS} of {results_count} items in Portal mode; switch to VirtualScroll for full virtualization)"
                    ))
                });

                OneOf3::C(portal(flex_col((items, footer))))
            }
        }
    };

    let main_layout = flex_col((
        controls_row,
        query_row,
        status_line,
        counts_line,
        error_line,
        results_view,
    ))
    .main_axis_alignment(MainAxisAlignment::Start)
    .cross_axis_alignment(CrossAxisAlignment::Start);

    // 5. Connect Background Work with `fork` and `task_raw`
    let task_view = if state.is_scanning {
        let scan_id = state.current_scan_id;
        let cancel = state.cancel_token.clone();
        let root = PathBuf::from(&state.root_path_input);
        let config = ScanConfig::default();

        Some(task_raw(
            move |proxy| {
                let cancel_for_worker = cancel.clone();
                let cancel_for_cleanup = cancel.clone();
                let root_for_worker = root.clone();
                let config_for_worker = config.clone();

                async move {
                    // Deliberate capacity: 32 batches of backpressure
                    let (tx, mut rx) = tokio::sync::mpsc::channel::<ScanEvent>(32);

                    let _worker_handle = std::thread::Builder::new()
                        .name(format!("ferrum-scanner-{scan_id}"))
                        .spawn(move || {
                            scanner::run_scan(
                                scan_id,
                                root_for_worker,
                                config_for_worker,
                                cancel_for_worker,
                                |event| {
                                    // Bounded blocking send enforces backpressure
                                    tx.blocking_send(event).is_ok()
                                },
                            );
                        });

                    // Forward events from bounded channel to Xilem event loop
                    while let Some(event) = rx.recv().await {
                        if proxy.message(event).is_err() {
                            // UI event loop disconnected
                            cancel_for_cleanup.cancel();
                            break;
                        }
                    }

                    // On cancellation or drop, ensure token is cancelled
                    cancel_for_cleanup.cancel();
                }
            },
            |state: &mut AppState, event: ScanEvent| {
                state.on_scan_event(event);
            },
        ))
    } else {
        None
    };

    fork(main_layout, task_view)
}
