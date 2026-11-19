use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use floem::event::EventListener;
use floem::ext_event::register_ext_trigger;
use floem::prelude::*;
use floem::reactive::{create_effect, create_trigger};

fn run_work(cancel: &AtomicBool, progress: &AtomicUsize, mut notify: impl FnMut()) {
    for step in 1..=100 {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        progress.store(step, Ordering::Relaxed);
        notify();
    }
}

fn app() -> impl IntoView {
    let cancelled = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicBool::new(false));
    let progress_view = RwSignal::new(0usize);
    let running = RwSignal::new(false);
    let status = RwSignal::new(String::from("Ready"));
    let trigger = create_trigger();

    let observed_progress = Arc::clone(&progress);
    let observed_finished = Arc::clone(&finished);
    let observed_cancelled = Arc::clone(&cancelled);
    create_effect(move |_| {
        trigger.track();
        progress_view.set(observed_progress.load(Ordering::Relaxed));
        if observed_finished.load(Ordering::Acquire) {
            running.set(false);
            status.set(if observed_cancelled.load(Ordering::Relaxed) {
                "Cancelled".into()
            } else {
                "Completed".into()
            });
        }
    });

    let start_cancel = Arc::clone(&cancelled);
    let start_progress = Arc::clone(&progress);
    let start_finished = Arc::clone(&finished);
    let cancel_button = Arc::clone(&cancelled);
    let close_cancel = Arc::clone(&cancelled);

    v_stack((
        label(move || format!("Status: {}", status.get())),
        label(move || format!("Progress: {} / 100", progress_view.get())),
        h_stack((
            button("Start").action(move || {
                if running.get() {
                    return;
                }
                start_cancel.store(false, Ordering::Relaxed);
                start_progress.store(0, Ordering::Relaxed);
                start_finished.store(false, Ordering::Relaxed);
                progress_view.set(0);
                running.set(true);
                status.set("Running".into());

                let cancel = Arc::clone(&start_cancel);
                let progress = Arc::clone(&start_progress);
                let finished = Arc::clone(&start_finished);
                let worker = std::thread::Builder::new().spawn(move || {
                    run_work(&cancel, &progress, || register_ext_trigger(trigger));
                    finished.store(true, Ordering::Release);
                    register_ext_trigger(trigger);
                });
                if let Err(error) = worker {
                    running.set(false);
                    status.set(format!("Could not start worker: {error}"));
                }
            }),
            button("Cancel").action(move || {
                if running.get() {
                    cancel_button.store(true, Ordering::Relaxed);
                    status.set("Cancelling".into());
                }
            }),
        )),
    ))
    .on_event_cont(EventListener::WindowClosed, move |_| {
        close_cancel.store(true, Ordering::Relaxed);
        eprintln!("Window closed; cancellation requested");
    })
}

fn main() {
    floem::launch(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_after_progress_stops_work() {
        let cancel = AtomicBool::new(false);
        let progress = AtomicUsize::new(0);
        run_work(&cancel, &progress, || cancel.store(true, Ordering::Relaxed));
        assert_eq!(progress.load(Ordering::Relaxed), 1);
    }
}
