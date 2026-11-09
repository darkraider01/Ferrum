use ferrum_file_search::app::{app_logic, AppState};
use xilem::dpi::LogicalSize;
use xilem::{EventLoop, WindowOptions, Xilem};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Ferrum File Search (Xilem/Masonry Feasibility Experiment)...");

    let event_loop = EventLoop::with_user_event();

    let window_options = WindowOptions::new("Ferrum File Search - Feasibility Spike")
        .with_min_inner_size(LogicalSize::new(700.0, 500.0))
        .on_close(|state: &mut AppState| {
            println!("Window close requested. Triggering cooperative cancellation...");
            state.cancel_scan();
        });

    let app = Xilem::new_simple(AppState::default(), app_logic, window_options);

    app.run_in(event_loop)?;
    Ok(())
}
