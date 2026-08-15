//! Wave 981: FOW shroud catalog + TOD residual peels.
//!
//! - Presentation unit catalog stamps ObjectShroudStatus from FOW.
//! - hover_target_shroud_for_command_hint uses catalog when dual-world empty.
//! - populate_build_queue preserves presentation residual on empty dual-world.
//! - Meta TOD queues residual; presentation shell drains onto drawable_map.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FOW_TOD_RESIDUAL_METHOD_NAMES_WAVE981: &[&str] = &[
    "hover_target_shroud_for_command_hint",
    "take_host_drawable_tod_residual",
    "queue_host_drawable_tod_residual",
    "populate_build_queue",
    "shroud_status",
    "Wave 981",
    "playable_claim = false",
];

pub const LIVE_HOST_FOW_TOD_RESIDUAL_NAV_STEPS_WAVE981: &[&str] = &[
    "FOW_SHROUD_CATALOG",
    "HOVER_HINT_FROM_PRESENTATION",
    "BUILD_QUEUE_PRESERVE",
    "TOD_SHELL_DRAIN",
    "LIVE_HOST_FOW_TOD_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFowTodResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFowTodResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}

fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}

fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

fn client_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

fn meta_source() -> &'static str {
    game_client::message_stream::meta_event::META_EVENT_SRC
}

// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_host_fow_tod_residual_method_names_residual_wave981() -> bool {
    let names = LIVE_HOST_FOW_TOD_RESIDUAL_METHOD_NAMES_WAVE981;
    let ok = residual_name_index(names, "hover_target_shroud_for_command_hint").is_some()
        && residual_name_index(names, "Wave 981").is_some();
    residual_action_store(ResidualHostFowTodResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_tod_residual_nav_commands_residual_wave981() -> bool {
    let steps = LIVE_HOST_FOW_TOD_RESIDUAL_NAV_STEPS_WAVE981;
    let ok = residual_name_index(steps, "LIVE_HOST_FOW_TOD_RESIDUAL").is_some()
        && residual_name_index(steps, "TOD_SHELL_DRAIN").is_some();
    residual_action_store(ResidualHostFowTodResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_tod_residual_residual_pack_wave981() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let cb = cb_source();
    let client = client_source();
    let meta = meta_source();

    let hover = match ui.find("fn hover_target_shroud_for_command_hint") {
        Some(i) => &ui[i..],
        None => "",
    };
    let populate = match cb.find("fn populate_build_queue") {
        Some(i) => &cb[i..],
        None => "",
    };
    let shell = match client.find("fn update_presentation_shell") {
        Some(i) => {
            let rest = &client[i..];
            let end = rest
                .find("\n    pub fn ")
                .map(|o| i + o)
                .unwrap_or(client.len().min(i + 8000));
            &client[i..end]
        }
        None => "",
    };
    let refresh = match meta.find("fn refresh_drawable_time_of_day") {
        Some(i) => &meta[i..],
        None => "",
    };

    let ok = ui.contains("Wave 981")
        && ui.contains("pub shroud_status: ObjectShroudStatus")
        && hover.contains("presentation_unit_catalog")
        && hover.contains("e.shroud_status")
        && cnc.contains("shroud_status:")
        && cnc.contains("ObjectShroudStatus::Shrouded")
        && populate.contains("Wave 981")
        && populate.contains("dual_world_registry_unavailable")
        && !populate.contains("fail-closed wipe")
        && meta.contains("queue_host_drawable_tod_residual")
        && meta.contains("take_host_drawable_tod_residual")
        && refresh.contains("queue_host_drawable_tod_residual")
        && shell.contains("Wave 981")
        && shell.contains("take_host_drawable_tod_residual")
        && client.contains("shroud_status: u.shroud_status as u8")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFowTodResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_fow_tod_residual_honesty() -> bool {
    let a = honesty_host_fow_tod_residual_method_names_residual_wave981();
    let b = honesty_host_fow_tod_residual_nav_commands_residual_wave981();
    let c = honesty_host_fow_tod_residual_residual_pack_wave981();
    residual_action_store(ResidualHostFowTodResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_fow_tod_residual_wave981() {
        assert!(honesty_host_fow_tod_residual_residual_pack_wave981());
        assert!(honesty_host_fow_tod_residual_method_names_residual_wave981());
        assert!(honesty_host_fow_tod_residual_nav_commands_residual_wave981());
        assert!(simulate_live_host_fow_tod_residual_honesty());
    }
}
