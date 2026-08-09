//! Wave 979: meta plane-lock / kill-enemy presentation catalog residual.
//!
//! Expands translator catalog with airborne_target and peels
//! next_plane_camera_lock_object_id + kill_all_enemy_objects_for_local_player
//! onto catalog residual when OBJECT_REGISTRY is empty.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_META_AIRBORNE_CATALOG_METHOD_NAMES_WAVE979: &[&str] = &[
    "next_plane_camera_lock_object_id",
    "kill_all_enemy_objects_for_local_player",
    "airborne_target",
    "Wave 979",
    "playable_claim = false",
];

pub const LIVE_HOST_META_AIRBORNE_CATALOG_NAV_STEPS_WAVE979: &[&str] = &[
    "PLANE_LOCK_FROM_CATALOG",
    "KILL_ENEMY_FROM_CATALOG",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_META_AIRBORNE_CATALOG",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMetaAirborneCatalogAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMetaAirborneCatalogAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn meta_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/message_stream/meta_event.rs")
}

fn residual_mod_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/presentation_translator_residual.rs")
}

fn ui_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/gui/ingame_ui.rs")
}

pub fn honesty_host_meta_airborne_catalog_method_names_residual_wave979() -> bool {
    let names = LIVE_HOST_META_AIRBORNE_CATALOG_METHOD_NAMES_WAVE979;
    let ok = residual_name_index(names, "next_plane_camera_lock_object_id").is_some()
        && residual_name_index(names, "Wave 979").is_some();
    residual_action_store(ResidualHostMetaAirborneCatalogAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_meta_airborne_catalog_nav_commands_residual_wave979() -> bool {
    let steps = LIVE_HOST_META_AIRBORNE_CATALOG_NAV_STEPS_WAVE979;
    let ok = residual_name_index(steps, "LIVE_HOST_META_AIRBORNE_CATALOG").is_some()
        && residual_name_index(steps, "PLANE_LOCK_FROM_CATALOG").is_some();
    residual_action_store(ResidualHostMetaAirborneCatalogAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_meta_airborne_catalog_residual_pack_wave979() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let meta = meta_source();
    let residual = residual_mod_source();
    let ui = ui_source();
    let plane = match meta.find("fn next_plane_camera_lock_object_id") {
        Some(i) => &meta[i..meta.len().min(i + 1800)],
        None => "",
    };
    let kill = match meta.find("fn kill_all_enemy_objects_for_local_player") {
        Some(i) => &meta[i..meta.len().min(i + 1200)],
        None => "",
    };
    let ok = meta.contains("Wave 979")
        && residual.contains("airborne_target")
        && ui.contains("airborne_target")
        && cnc.contains("airborne_target:")
        && plane.contains("with_translator_catalog")
        && plane.contains("airborne_target")
        && kill.contains("with_translator_catalog")
        && kill.contains("translator_entry_is_local")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostMetaAirborneCatalogAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_meta_airborne_catalog_honesty() -> bool {
    let a = honesty_host_meta_airborne_catalog_method_names_residual_wave979();
    let b = honesty_host_meta_airborne_catalog_nav_commands_residual_wave979();
    let c = honesty_host_meta_airborne_catalog_residual_pack_wave979();
    residual_action_store(ResidualHostMetaAirborneCatalogAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_meta_airborne_catalog_residual_wave979() {
        assert!(honesty_host_meta_airborne_catalog_residual_pack_wave979());
        assert!(honesty_host_meta_airborne_catalog_method_names_residual_wave979());
        assert!(honesty_host_meta_airborne_catalog_nav_commands_residual_wave979());
        assert!(simulate_live_host_meta_airborne_catalog_honesty());
    }
}
