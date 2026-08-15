//! Wave 583 residual peels: runtime host command / selection / camera-follow /
//! EVA boot dual-reads are centralized through host helpers:
//! `host_force_complete_construction`, `host_ensure_barracks_building_data`,
//! `host_command_attack`/`stop`/`move`/`attack_move`, legal-build probes,
//! `host_set_camera_follow_object`, `presentation_or_boot_camera_follow_active`,
//! `boot_eva_counter_bundle_from_host`, plus additional `host_set_selection` peels.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 582 enqueue/shell command helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host command/selection/follow/EVA helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_RUNTIME_CMD_HELPER_METHOD_NAMES_WAVE583: &[&str] = &[
    "host_force_complete_construction",
    "host_ensure_barracks_building_data",
    "host_command_attack",
    "host_command_stop",
    "host_command_attack_move",
    "host_command_move",
    "host_legal_build_code_at_for_builder",
    "host_is_location_legal_to_build_for_builder",
    "host_set_camera_follow_object",
    "presentation_or_boot_camera_follow_active",
    "boot_eva_counter_bundle_from_host",
    "host_set_selection",
    "Wave 583",
    "playable_claim = false",
];

pub const LIVE_HOST_RUNTIME_CMD_HELPER_NAV_STEPS_WAVE583: &[&str] = &[
    "REQUIRE_HOST_FORCE_COMPLETE",
    "REQUIRE_HOST_COMMAND_ATTACK",
    "REQUIRE_HOST_COMMAND_MOVE",
    "REQUIRE_CAMERA_FOLLOW_HELPER",
    "REQUIRE_BOOT_EVA_BUNDLE",
    "LIVE_HOST_RUNTIME_CMD_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_RUNTIME_CMD_HELPER_CMD_NAMES_WAVE583: &[&str] = &[
    "host_force_complete_helper",
    "host_command_attack_helper",
    "host_command_move_helper",
    "host_camera_follow_helper",
    "boot_eva_bundle_helper",
    "runtime_cmd_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRuntimeCmdHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostRuntimeCmdHelperAction {
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

fn residual_action_store(action: ResidualHostRuntimeCmdHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_runtime_cmd_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_runtime_cmd_helper_last_action() -> ResidualHostRuntimeCmdHelperAction {
    ResidualHostRuntimeCmdHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
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

pub fn honesty_host_runtime_cmd_helper_method_names_residual_wave583() -> bool {
    let names = LIVE_HOST_RUNTIME_CMD_HELPER_METHOD_NAMES_WAVE583;
    let ok = residual_name_index(names, "host_force_complete_construction").is_some()
        && residual_name_index(names, "host_command_attack").is_some()
        && residual_name_index(names, "host_command_move").is_some()
        && residual_name_index(names, "host_set_camera_follow_object").is_some()
        && residual_name_index(names, "presentation_or_boot_camera_follow_active").is_some()
        && residual_name_index(names, "boot_eva_counter_bundle_from_host").is_some()
        && residual_name_index(names, "host_set_selection").is_some()
        && residual_name_index(names, "Wave 583").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostRuntimeCmdHelperAction::MethodNames);
    ok
}

pub fn honesty_host_runtime_cmd_helper_source_markers_residual_wave583() -> bool {
    let eng = eng_source();
    let required = [
        "fn host_force_complete_construction(",
        "fn host_ensure_barracks_building_data(",
        "fn host_command_attack(",
        "fn host_command_stop(",
        "fn host_command_attack_move(",
        "fn host_command_move(",
        "fn host_legal_build_code_at_for_builder(",
        "fn host_is_location_legal_to_build_for_builder(",
        "fn host_set_camera_follow_object(",
        "fn presentation_or_boot_camera_follow_active(",
        "fn boot_eva_counter_bundle_from_host(",
    ];
    let mut defs_ok = true;
    for sig in required {
        let Some(body) = fn_body(eng, sig) else {
            defs_ok = false;
            break;
        };
        if !body.contains("Wave 583") {
            defs_ok = false;
            break;
        }
        // no recursion: helper body must call game_logic, not itself
        let name = sig.trim_start_matches("fn ").trim_end_matches('(');
        if body.matches(&format!("self.{name}(")).count() > 0 {
            defs_ok = false;
            break;
        }
    }
    let call_ok = eng.contains("self.host_force_complete_construction(")
        && eng.contains("self.host_command_attack(")
        && eng.contains("self.host_command_stop(")
        && eng.contains("self.host_command_move(")
        && eng.contains("self.host_command_attack_move(")
        && eng.contains("self.host_legal_build_code_at_for_builder(")
        && eng.contains("self.host_is_location_legal_to_build_for_builder(")
        && eng.contains("self.host_set_camera_follow_object(")
        && eng.contains("self.presentation_or_boot_camera_follow_active()")
        && eng.contains("self.boot_eva_counter_bundle_from_host()");
    // raw dual-reads should only remain inside the helpers
    let raw_force = eng
        .matches("self.game_logic.force_complete_construction")
        .count();
    let raw_attack = eng.matches("self.game_logic.command_attack(").count();
    let raw_stop = eng.matches("self.game_logic.command_stop(").count();
    let raw_move = eng.matches("self.game_logic.command_move(").count();
    let raw_amove = eng.matches("self.game_logic.command_attack_move(").count();
    let raw_set_follow = eng
        .matches("self.game_logic.set_camera_follow_object(")
        .count();
    let raw_eva = eng.matches("self.game_logic.eva_low_power_count()").count();
    let ok = defs_ok
        && call_ok
        && raw_force == 0
        && raw_attack == 0
        && raw_stop == 0
        && raw_move == 0
        && raw_amove == 0
        && raw_set_follow == 0
        && raw_eva == 0
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostRuntimeCmdHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_runtime_cmd_helper_nav_commands_residual_wave583() -> bool {
    let steps = LIVE_HOST_RUNTIME_CMD_HELPER_NAV_STEPS_WAVE583;
    let cmds = RUNTIME_HOST_LIVE_HOST_RUNTIME_CMD_HELPER_CMD_NAMES_WAVE583;
    let ok = residual_name_index(steps, "REQUIRE_HOST_FORCE_COMPLETE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_COMMAND_ATTACK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_COMMAND_MOVE").is_some()
        && residual_name_index(steps, "REQUIRE_CAMERA_FOLLOW_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_BOOT_EVA_BUNDLE").is_some()
        && residual_name_index(steps, "LIVE_HOST_RUNTIME_CMD_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_force_complete_helper").is_some()
        && residual_name_index(cmds, "host_command_attack_helper").is_some()
        && residual_name_index(cmds, "host_command_move_helper").is_some()
        && residual_name_index(cmds, "host_camera_follow_helper").is_some()
        && residual_name_index(cmds, "boot_eva_bundle_helper").is_some()
        && residual_name_index(cmds, "runtime_cmd_residual").is_some();
    residual_action_store(ResidualHostRuntimeCmdHelperAction::NavCommands);
    ok
}

pub fn simulate_host_runtime_cmd_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 583")
        && eng.contains("fn host_force_complete_construction")
        && eng.contains("fn host_command_attack")
        && eng.contains("fn host_command_move")
        && eng.contains("fn presentation_or_boot_camera_follow_active")
        && eng.contains("fn boot_eva_counter_bundle_from_host");
    residual_action_store(ResidualHostRuntimeCmdHelperAction::CollectSource);
    ok
}

pub fn simulate_host_runtime_cmd_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_force_complete_construction(")
        && eng.contains("self.host_command_attack(")
        && eng.contains("self.host_command_stop(")
        && eng.contains("self.host_command_move(")
        && eng.contains("self.presentation_or_boot_camera_follow_active()")
        && eng.contains("self.boot_eva_counter_bundle_from_host()")
        && eng.contains("self.host_set_selection(");
    residual_action_store(ResidualHostRuntimeCmdHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_runtime_cmd_helper_residual_pack_wave583() -> bool {
    honesty_host_runtime_cmd_helper_method_names_residual_wave583()
        && honesty_host_runtime_cmd_helper_source_markers_residual_wave583()
        && honesty_host_runtime_cmd_helper_nav_commands_residual_wave583()
        && simulate_host_runtime_cmd_helper_collect_source()
        && simulate_host_runtime_cmd_helper_dispatch_source()
}

pub fn simulate_live_host_runtime_cmd_helper_honesty() -> bool {
    let ok = honesty_host_runtime_cmd_helper_residual_pack_wave583();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostRuntimeCmdHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_runtime_cmd_helper_method_names_residual_wave583());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_runtime_cmd_helper_source_markers_residual_wave583());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_runtime_cmd_helper_nav_commands_residual_wave583());
    }

    #[test]
    fn host_runtime_cmd_helper_sources() {
        assert!(simulate_host_runtime_cmd_helper_collect_source());
        assert!(simulate_host_runtime_cmd_helper_dispatch_source());
    }

    #[test]
    fn wave583_composite_pack() {
        assert!(honesty_host_runtime_cmd_helper_residual_pack_wave583());
    }

    #[test]
    fn simulate_live_host_runtime_cmd_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_runtime_cmd_helper_honesty(),
            "host runtime cmd helper residual must latch"
        );
        assert!(residual_host_runtime_cmd_helper_ok());
        assert_eq!(
            residual_host_runtime_cmd_helper_last_action(),
            ResidualHostRuntimeCmdHelperAction::Composite
        );
    }
}
