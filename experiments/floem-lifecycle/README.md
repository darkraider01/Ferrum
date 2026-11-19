# Floem lifecycle probe

This small experiment compares a window-owned background task with the Xilem file-search prototype. It uses the published Floem 0.2.0 release and its Vger renderer; it does not represent the current Floem development branch.

```sh
cargo run --manifest-path experiments/floem-lifecycle/Cargo.toml
cargo test --manifest-path experiments/floem-lifecycle/Cargo.toml
```

Start launches a worker that advances through 100 simulated steps, one every 100 ms. Cancel requests cooperative cancellation; Start is available again after the worker reports completion. Closing the window requests cancellation without waiting for the worker. A terminal message confirms that the close handler ran; it does not confirm the worker finished before process exit.

## Comparison

| Concern | Floem 0.2.0 probe | Xilem 0.4.0 file search |
| --- | --- | --- |
| UI updates | A reactive trigger wakes an effect that reads shared progress. | A task forwards worker events to the view. |
| Cancellation | Application-owned atomic flag, checked between steps. | Application-owned atomic flag, checked during traversal. |
| Window close | A `WindowClosed` event handler requests cancellation. | A window `on_close` callback requests cancellation. |
| Backpressure | The simulated workload emits at most 101 notifications; there is no general queue bound or acknowledgement. | A bounded channel and UI acknowledgements limit outstanding scan messages. |
| Shutdown | The close handler does not join the worker; worker termination is not guaranteed before process exit. | The close callback does not join the worker; cancellation cannot interrupt an in-progress filesystem operation. |

Both examples require application-owned cancellation. The workloads and update mechanisms differ, so this is a comparison of the APIs used, not a performance benchmark or evidence that either framework lacks a lifecycle facility. Floem's current development version may behave differently.

## Verification

The cancellation test exercises the worker, not the Floem window event loop. Interactive progress, cancellation, restarting, and close behavior must be checked in a desktop session. No screen-reader or IME behavior is evaluated here.
