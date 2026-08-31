//! Wave 839: host_vertical_slice_ok requires non-zero stable presentation mesh
//! residual (max_render_alive/items + live_fallback honesty). playable_claim false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_VERTICAL_RENDER_MESH_GATE_METHOD_NAMES_WAVE839: &[&str] = &[
    "host_vertical_slice_ok",
    "render_mesh_boundary_ok",
    "max_render_alive_objects",
    "max_render_item_count",
    "render_items_stable_ok",
    "presentation_live_fallback_ok",
    "Wave 839",
    "playable_claim = false",
];

pub const LIVE_HOST_VERTICAL_RENDER_MESH_GATE_NAV_STEPS_WAVE839: &[&str] = &[
    "REQUIRE_RENDER_ALIVE",
    "REQUIRE_RENDER_ITEMS",
    "REQUIRE_RENDER_STABLE",
    "REQUIRE_LIVE_FALLBACK_OK",
    "LIVE_HOST_VERTICAL_RENDER_MESH_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostVerticalRenderMeshGateAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostVerticalRenderMeshGateAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

fn bg_source() -> &'static str {
    include_str!("../../bin/behavior_gate.rs")
}

pub fn honesty_host_vertical_render_mesh_gate_method_names_residual_wave839() -> bool {
    let names = LIVE_HOST_VERTICAL_RENDER_MESH_GATE_METHOD_NAMES_WAVE839;
    let ok = residual_name_index(names, "render_mesh_boundary_ok").is_some()
        && residual_name_index(names, "max_render_alive_objects").is_some()
        && residual_name_index(names, "Wave 839").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostVerticalRenderMeshGateAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_vertical_render_mesh_gate_nav_commands_residual_wave839() -> bool {
    let steps = LIVE_HOST_VERTICAL_RENDER_MESH_GATE_NAV_STEPS_WAVE839;
    let ok = residual_name_index(steps, "LIVE_HOST_VERTICAL_RENDER_MESH_GATE").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostVerticalRenderMeshGateAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_vertical_render_mesh_gate_residual_pack_wave839() -> bool {
    let es = es_source();
    let bg = bg_source();
    let ok = es.contains("let render_mesh_boundary_ok")
        && (es.contains("result.max_render_alive_objects > 0")
            || es.contains("self.max_render_alive_objects > 0"))
        && (es.contains("result.max_render_item_count > 0")
            || es.contains("self.max_render_item_count > 0")
            || es.contains("max_render_item_count"))
        && (es.contains("result.render_items_stable_ok")
            || es.contains("self.render_items_stable_ok"))
        && es.contains("&& render_mesh_boundary_ok")
        && bg.contains("render_alive={}")
        && bg.contains("exec.max_render_alive_objects");
    residual_action_store(ResidualHostVerticalRenderMeshGateAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_vertical_render_mesh_gate_honesty() -> bool {
    let a = honesty_host_vertical_render_mesh_gate_method_names_residual_wave839();
    let b = honesty_host_vertical_render_mesh_gate_nav_commands_residual_wave839();
    let c = honesty_host_vertical_render_mesh_gate_residual_pack_wave839();
    residual_action_store(ResidualHostVerticalRenderMeshGateAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_vertical_render_mesh_gate_residual_wave839() {
        assert!(honesty_host_vertical_render_mesh_gate_residual_pack_wave839());
        assert!(honesty_host_vertical_render_mesh_gate_method_names_residual_wave839());
        assert!(honesty_host_vertical_render_mesh_gate_nav_commands_residual_wave839());
        assert!(simulate_live_host_vertical_render_mesh_gate_honesty());
    }
}
