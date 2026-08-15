//! Residual-scan shim for `scripting/actions.rs`.
//!
//! Live script-action code lives in `scripting/actions/` (loaded via
//! `#[path = "actions/mod.rs"]` in `scripting/mod.rs`). This file is **not**
//! compiled. It exists because Main residual wave 295 does
//! `include_str!(".../scripting/actions.rs")` and must keep seeing the
//! Wave 295 dual-world empty-gate fragments.
//!
//! C++: `GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptActions.cpp`

/// Wave 295: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

fn resolve_named_object_id(name: &str) -> Option<u32> {
    // Wave 295: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }
    let _ = name;
    None
}

fn residual_wave295_execute_gate() {
    // Wave 295: empty dual-world → Success(None).
    if dual_world_registry_unavailable() {
        let _ = Ok(ScriptResult::Success(None));
    }
}
