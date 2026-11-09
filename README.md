# Ferrum

An exploratory Rust desktop framework focused on native UI and predictable application lifecycles.

## Overview

Ferrum explores how to build cross-platform desktop applications without requiring a browser engine for their main interface. The initial focus is on Rust applications that combine interactive UI with background work, such as file search, indexing, and document processing.

The central question is whether existing native UI frameworks can provide a cohesive developer experience for task ownership, cancellation, window lifecycles, and resource control—or whether those needs justify additional infrastructure.

## Project Status

Ferrum is in the research stage. There is no framework implementation or release yet, and performance and platform compatibility have not been established.

The first planned experiment is a file-search application built with Xilem/Masonry. It will exercise background scanning, progress reporting, cancellation, search, and window shutdown alongside text input and accessibility. The results will help determine what Ferrum needs to build and what it can reuse.

## What Ferrum Is Exploring

- **Application and task lifecycles:** Clear ownership of background work, cooperative cancellation, and responsive shutdown.
- **Native UI:** A Rust-based interface without a mandatory Chromium or system WebView runtime.
- **Text and accessibility:** Keyboard navigation, text editing, input methods, and screen-reader support as early requirements.
- **Resource behavior:** Measurable startup, memory, idle activity, and responsiveness on representative workloads.
- **Cross-platform support:** Windows, macOS, and Linux, including native Wayland, as long-term targets.

These are goals for the project, not claims about implemented features. The feasibility work will evaluate existing native frameworks, including Xilem/Masonry, Floem, and GPUI, before committing to a new widget system.

## Scope and Compatibility

Ferrum is intended for new applications built with native UI components. It does not aim to run existing Electron or React DOM applications unchanged. A TypeScript interface and optional WebView integration are future ideas that require separate evaluation.

Performance comparisons need equivalent functionality and reproducible measurements. Any resource savings over Electron, or benefits over existing native frameworks, remain hypotheses until tested.

## Design

The [architecture proposal](https://hackmd.io/@darkraider01/rJvlD-kAzx) covers the research goals, candidate dependencies, feasibility roadmap, and open decisions. It is a working proposal; implementation choices may change as the prototype provides evidence.
