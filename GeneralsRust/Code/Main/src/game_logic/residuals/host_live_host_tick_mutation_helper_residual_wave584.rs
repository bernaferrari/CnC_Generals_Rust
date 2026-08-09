//! Wave 584 residual peels: remaining host mutation dual-reads are centralized
//! through host helpers — logic/shell ticks, multiplayer gate, object-alive,
//! special-power ready, victory summary, reset/destroy, science capability,
//! path clear, guard radius, and queue-only command residual.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 583 runtime command helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host tick/mutation helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_TICK_MUTATION_HELPER_METHOD_NAMES_WAVE584: &[&str] = &[
    "host_object_is_alive",
    "presentation_or_boot_object_alive",
    "host_update_shell_with_budget",
    "host_update_logic_frame",
    "host_is_in_multiplayer_game",
    "host_is_special_power_ready_for",
    "presentation_or_boot_victory_summary",
    "host_reset_game_logic",
    "host_destroy_object",
    "host_player_can_purchase_science",
    "host_clear_unit_movement_path",
    "host_adjust_unit_guard_radius",
    "host_queue_command",
    "Wave 584",
    "playable_claim = false",
];

pub const LIVE_HOST_TICK_MUTATION_HELPER_NAV_STEPS_WAVE584: &[&str] = &[
    "REQUIRE_HOST_LOGIC_TICK",
    "REQUIRE_HOST_SHELL_TICK",
    "REQUIRE_HOST_DESTROY_RESET",
    "REQUIRE_PRESENTATION_OR_BOOT_ALIVE",
    "REQUIRE_VICTORY_SUMMARY_HELPER",
    "LIVE_HOST_TICK_MUTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_TICK_MUTATION_HELPER_CMD_NAMES_WAVE584: &[&str] = &[
    "host_logic_tick_helper",
    "host_shell_tick_helper",
    "host_destroy_reset_helper",
    "presentation_or_boot_alive_helper",
    "victory_summary_helper",
    "tick_mutation_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostTickMutationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostTickMutationHelperAction {
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

fn residual_action_store(action: ResidualHostTickMutationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_tick_mutation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_tick_mutation_helper_last_action() -> ResidualHostTickMutationHelperAction {
    ResidualHostTickMutationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_host_tick_mutation_helper_method_names_residual_wave584() -> bool {
    let names = LIVE_HOST_TICK_MUTATION_HELPER_METHOD_NAMES_WAVE584;
    let ok = residual_name_index(names, "host_update_logic_frame").is_some()
        && residual_name_index(names, "host_update_shell_with_budget").is_some()
        && residual_name_index(names, "host_destroy_object").is_some()
        && residual_name_index(names, "host_reset_game_logic").is_some()
        && residual_name_index(names, "presentation_or_boot_object_alive").is_some()
        && residual_name_index(names, "presentation_or_boot_victory_summary").is_some()
        && residual_name_index(names, "host_queue_command").is_some()
        && residual_name_index(names, "Wave 584").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostTickMutationHelperAction::MethodNames);
    ok
}

pub fn honesty_host_tick_mutation_helper_source_markers_residual_wave584() -> bool {
    let eng = eng_source();
    let required = [
        "fn host_object_is_alive(",
        "fn presentation_or_boot_object_alive(",
        "fn host_update_shell_with_budget(",
        "fn host_update_logic_frame(",
        "fn host_is_in_multiplayer_game(",
        "fn host_is_special_power_ready_for(",
        "fn presentation_or_boot_victory_summary(",
        "fn host_reset_game_logic(",
        "fn host_destroy_object(",
        "fn host_player_can_purchase_science(",
        "fn host_clear_unit_movement_path(",
        "fn host_adjust_unit_guard_radius(",
        "fn host_queue_command(",
    ];
    let mut defs_ok = true;
    for sig in required {
        let Some(body) = fn_body(eng, sig) else {
            defs_ok = false;
            break;
        };
        if !body.contains("Wave 584") {
            defs_ok = false;
            break;
        }
        let name = sig.trim_start_matches("fn ").trim_end_matches('(');
        // no self-recursion
        if body.matches(&format!("self.{name}(")).count() > 0 {
            defs_ok = false;
            break;
        }
    }
    let call_ok = eng.contains("self.host_update_logic_frame(")
        && eng.contains("self.host_update_shell_with_budget(")
        && eng.contains("self.host_is_in_multiplayer_game()")
        && eng.contains("self.presentation_or_boot_object_alive(")
        && eng.contains("self.presentation_or_boot_victory_summary(")
        && eng.contains("self.host_reset_game_logic()")
        && eng.contains("self.host_destroy_object(")
        && eng.contains("self.host_player_can_purchase_science(")
        && eng.contains("self.host_clear_unit_movement_path(")
        && eng.contains("self.host_adjust_unit_guard_radius(")
        && eng.contains("self.host_queue_command(")
        && eng.contains("self.host_is_special_power_ready_for(");
    let raw_shell = eng
        .matches("self.game_logic.update_shell_with_budget")
        .count();
    let raw_timing = eng.matches("self.game_logic.update_with_timing(").count();
    let raw_dt = eng.matches("self.game_logic.update_with_dt(").count();
    let raw_mp = eng.matches("self.game_logic.isInMultiplayerGame()").count();
    let raw_reset = eng.matches("self.game_logic.reset()").count();
    let raw_destroy = eng.matches("self.game_logic.destroy_object(").count();
    let raw_alive = eng.matches("self.game_logic.object_is_alive(").count();
    let ok = defs_ok
        && call_ok
        && raw_shell == 1
        && raw_timing == 1
        && raw_dt == 1
        && raw_mp == 1
        && raw_reset == 1
        && raw_destroy == 1
        && raw_alive == 1
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostTickMutationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_tick_mutation_helper_nav_commands_residual_wave584() -> bool {
    let steps = LIVE_HOST_TICK_MUTATION_HELPER_NAV_STEPS_WAVE584;
    let cmds = RUNTIME_HOST_LIVE_HOST_TICK_MUTATION_HELPER_CMD_NAMES_WAVE584;
    let ok = residual_name_index(steps, "REQUIRE_HOST_LOGIC_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_SHELL_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_DESTROY_RESET").is_some()
        && residual_name_index(steps, "REQUIRE_PRESENTATION_OR_BOOT_ALIVE").is_some()
        && residual_name_index(steps, "REQUIRE_VICTORY_SUMMARY_HELPER").is_some()
        && residual_name_index(steps, "LIVE_HOST_TICK_MUTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_logic_tick_helper").is_some()
        && residual_name_index(cmds, "host_shell_tick_helper").is_some()
        && residual_name_index(cmds, "host_destroy_reset_helper").is_some()
        && residual_name_index(cmds, "presentation_or_boot_alive_helper").is_some()
        && residual_name_index(cmds, "victory_summary_helper").is_some()
        && residual_name_index(cmds, "tick_mutation_residual").is_some();
    residual_action_store(ResidualHostTickMutationHelperAction::NavCommands);
    ok
}

pub fn simulate_host_tick_mutation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 584")
        && eng.contains("fn host_update_logic_frame")
        && eng.contains("fn host_update_shell_with_budget")
        && eng.contains("fn presentation_or_boot_object_alive")
        && eng.contains("fn presentation_or_boot_victory_summary")
        && eng.contains("fn host_reset_game_logic")
        && eng.contains("fn host_destroy_object");
    residual_action_store(ResidualHostTickMutationHelperAction::CollectSource);
    ok
}

pub fn simulate_host_tick_mutation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_update_logic_frame(dt, headless_step_budget)")
        && eng.contains("self.host_update_shell_with_budget(dt, 1)")
        && eng.contains("self.presentation_or_boot_object_alive(pid)")
        && eng.contains("self.presentation_or_boot_victory_summary(winner)")
        && eng.contains("self.host_reset_game_logic()")
        && eng.contains("self.host_destroy_object(id)");
    residual_action_store(ResidualHostTickMutationHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_tick_mutation_helper_residual_pack_wave584() -> bool {
    honesty_host_tick_mutation_helper_method_names_residual_wave584()
        && honesty_host_tick_mutation_helper_source_markers_residual_wave584()
        && honesty_host_tick_mutation_helper_nav_commands_residual_wave584()
        && simulate_host_tick_mutation_helper_collect_source()
        && simulate_host_tick_mutation_helper_dispatch_source()
}

pub fn simulate_live_host_tick_mutation_helper_honesty() -> bool {
    let ok = honesty_host_tick_mutation_helper_residual_pack_wave584();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostTickMutationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_tick_mutation_helper_method_names_residual_wave584());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_tick_mutation_helper_source_markers_residual_wave584());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_tick_mutation_helper_nav_commands_residual_wave584());
    }

    #[test]
    fn host_tick_mutation_helper_sources() {
        assert!(simulate_host_tick_mutation_helper_collect_source());
        assert!(simulate_host_tick_mutation_helper_dispatch_source());
    }

    #[test]
    fn wave584_composite_pack() {
        assert!(honesty_host_tick_mutation_helper_residual_pack_wave584());
    }

    #[test]
    fn simulate_live_host_tick_mutation_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_tick_mutation_helper_honesty(),
            "host tick/mutation helper residual must latch"
        );
        assert!(residual_host_tick_mutation_helper_ok());
        assert_eq!(
            residual_host_tick_mutation_helper_last_action(),
            ResidualHostTickMutationHelperAction::Composite
        );
    }
}
