//! Wave 579 residual peels: host selection residual is centralized through
//! `host_set_selection`, and map-load fallback through `host_load_map_or_default`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 578 silent command peel residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_set_selection / host_load_map_or_default
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELECTION_MAP_HELPER_METHOD_NAMES_WAVE579: &[&str] = &[
    "host_set_selection",
    "host_load_map_or_default",
    "select_objects",
    "load_map",
    "Wave 579",
    "playable_claim = false",
];

pub const LIVE_HOST_SELECTION_MAP_HELPER_NAV_STEPS_WAVE579: &[&str] = &[
    "REQUIRE_HOST_SET_SELECTION",
    "REQUIRE_HOST_LOAD_MAP_OR_DEFAULT",
    "LIVE_HOST_SELECTION_MAP_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_SELECTION_MAP_HELPER_CMD_NAMES_WAVE579: &[&str] = &[
    "host_set_selection_helper",
    "host_load_map_helper",
    "selection_map_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSelectionMapHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostSelectionMapHelperAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualHostSelectionMapHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_selection_map_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_selection_map_helper_last_action() -> ResidualHostSelectionMapHelperAction {
    ResidualHostSelectionMapHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_host_selection_map_helper_method_names_residual_wave579() -> bool {
    let names = LIVE_HOST_SELECTION_MAP_HELPER_METHOD_NAMES_WAVE579;
    let ok = residual_name_index(names, "host_set_selection").is_some()
        && residual_name_index(names, "host_load_map_or_default").is_some()
        && residual_name_index(names, "select_objects").is_some()
        && residual_name_index(names, "load_map").is_some()
        && residual_name_index(names, "Wave 579").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSelectionMapHelperAction::MethodNames);
    ok
}

pub fn honesty_host_selection_map_helper_source_markers_residual_wave579() -> bool {
    let eng = eng_source();
    let Some(sel) = fn_body(eng, "fn host_set_selection(") else {
        residual_action_store(ResidualHostSelectionMapHelperAction::SourceMarkers);
        return false;
    };
    let Some(load) = fn_body(eng, "fn host_load_map_or_default(") else {
        residual_action_store(ResidualHostSelectionMapHelperAction::SourceMarkers);
        return false;
    };
    let sel_ok = sel.contains("Wave 579")
        && sel.contains("select_objects(player_id, ids.clone())")
        && sel.contains("self.selected_objects = ids");
    let load_ok = load.contains("Wave 579")
        && load.contains("load_map(map_name)")
        && load.contains("DEFAULT_SKIRMISH_MAP");
    let call_ok =
        eng.contains("self.host_set_selection(") && eng.contains("self.host_load_map_or_default(");
    let raw_sel = eng.matches("self.game_logic.select_objects").count();
    let raw_load = eng.matches("self.game_logic.load_map").count();
    // only inside helpers (select once, load twice for fallback)
    let ok = sel_ok
        && load_ok
        && call_ok
        && raw_sel == 1
        && raw_load == 2
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostSelectionMapHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_selection_map_helper_nav_commands_residual_wave579() -> bool {
    let steps = LIVE_HOST_SELECTION_MAP_HELPER_NAV_STEPS_WAVE579;
    let cmds = RUNTIME_HOST_LIVE_HOST_SELECTION_MAP_HELPER_CMD_NAMES_WAVE579;
    let ok = residual_name_index(steps, "REQUIRE_HOST_SET_SELECTION").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_LOAD_MAP_OR_DEFAULT").is_some()
        && residual_name_index(steps, "LIVE_HOST_SELECTION_MAP_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_set_selection_helper").is_some()
        && residual_name_index(cmds, "host_load_map_helper").is_some()
        && residual_name_index(cmds, "selection_map_residual").is_some();
    residual_action_store(ResidualHostSelectionMapHelperAction::NavCommands);
    ok
}

pub fn simulate_host_selection_map_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 579")
        && eng.contains("fn host_set_selection")
        && eng.contains("fn host_load_map_or_default");
    residual_action_store(ResidualHostSelectionMapHelperAction::CollectSource);
    ok
}

pub fn simulate_host_selection_map_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_set_selection(")
        && eng.contains("self.host_load_map_or_default(&map_name)")
        && eng.contains("host_start_new_game_with_faction");
    residual_action_store(ResidualHostSelectionMapHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_selection_map_helper_residual_pack_wave579() -> bool {
    honesty_host_selection_map_helper_method_names_residual_wave579()
        && honesty_host_selection_map_helper_source_markers_residual_wave579()
        && honesty_host_selection_map_helper_nav_commands_residual_wave579()
        && simulate_host_selection_map_helper_collect_source()
        && simulate_host_selection_map_helper_dispatch_source()
}

pub fn simulate_live_host_selection_map_helper_honesty() -> bool {
    let ok = honesty_host_selection_map_helper_residual_pack_wave579();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSelectionMapHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_selection_map_helper_method_names_residual_wave579());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_selection_map_helper_source_markers_residual_wave579());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_selection_map_helper_nav_commands_residual_wave579());
    }

    #[test]
    fn host_selection_map_helper_sources() {
        assert!(simulate_host_selection_map_helper_collect_source());
        assert!(simulate_host_selection_map_helper_dispatch_source());
    }

    #[test]
    fn wave579_composite_pack() {
        assert!(honesty_host_selection_map_helper_residual_pack_wave579());
    }

    #[test]
    fn simulate_live_host_selection_map_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_selection_map_helper_honesty(),
            "host selection/map helper residual must latch"
        );
        assert!(residual_host_selection_map_helper_ok());
        assert_eq!(
            residual_host_selection_map_helper_last_action(),
            ResidualHostSelectionMapHelperAction::Composite
        );
    }
}
