//! Legacy module preserving the original file layout.
//!
//! The previous port sprinkled hundreds of modules across the crate.  They are
//! still available in the repository history and will be reintroduced behind
//! well-defined APIs as we progress with the rewrite.  Until then enabling the
//! `legacy_port` feature is a deliberate, opt-in action that immediately fails
//!
//! with a descriptive message.  This prevents accidental builds from silently
//! pulling in the half-ported code.

#[cfg(any(feature = "legacy_port"))]
compile_error!(
    "The `legacy_port` feature is intentionally disabled while the modern \
     game-logic core is under construction.  Track progress in \
     docs/game-engine.md and re-enable once the new systems are in place."
);
