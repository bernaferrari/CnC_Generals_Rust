//! Concatenated `executable_smoke` sources for residual source-text honesty
//! assertions.
//!
//! `executable_smoke.rs` was split by lifecycle phase into an ordered
//! `include!` hub plus `executable_smoke/` fragments (result → status →
//! process → bootstrap → frame_loop → gameplay_chain → presentation →
//! shutdown → report → tests). Residual packs and tests that previously read
//! the single-file monolith via `include_str!("executable_smoke.rs")` should
//! read [`EXECUTABLE_SMOKE_SRC`] instead: the identical text stream, hub
//! first, then fragments in `include!` order, then the test mods in their
//! original tail position (which keeps self-referencing assertion text such
//! as `pause_ok:paused` / `pause_ok:resumed` visible).
//!
//! Private module so the split adds no crate-public API.

/// Full `executable_smoke` source text in pre-split monolith order.
pub const EXECUTABLE_SMOKE_SRC: &str = concat!(
    include_str!("executable_smoke.rs"),
    include_str!("executable_smoke/result.rs"),
    include_str!("executable_smoke/status.rs"),
    include_str!("executable_smoke/process.rs"),
    include_str!("executable_smoke/bootstrap.rs"),
    include_str!("executable_smoke/frame_loop.rs"),
    include_str!("executable_smoke/gameplay_chain.rs"),
    include_str!("executable_smoke/presentation.rs"),
    include_str!("executable_smoke/shutdown.rs"),
    include_str!("executable_smoke/report.rs"),
    include_str!("executable_smoke/tests.rs"),
);
