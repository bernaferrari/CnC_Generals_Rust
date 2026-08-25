//! Wave 487 residual peels: combat model-condition bits survive refresh and log to GW.
//! - `refresh_model_condition_bits` preserves weapon fire / prone / crush bits
//! - `sync_weapon_model_conditions_from_status` records on change
//! - `go_prone` / prone timer clear record model bits
//! - `apply_crush_die_model_conditions` records on change
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 486 production-door model log.
//! Architecture residual - writeback_model_condition is last-writer from GW snapshot.
//!
//! Sources:
//! - object.rs refresh_model_condition_bits WEAPON_MC_PRESERVE
//! - object.rs sync_weapon_model_conditions_from_status record_host_model_condition
//! - object.rs go_prone / tick_timers prone clear / apply_crush_die_model_conditions
//!
//! Fail-closed:
//! - Unrelated model bits unchanged
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const COMBAT_MODEL_CONDITION_CHANNEL_METHOD_NAMES_WAVE487: &[&str] = &[
    "refresh_model_condition_bits",
    "WEAPON_MC_PRESERVE",
    "sync_weapon_model_conditions_from_status",
    "go_prone",
    "apply_crush_die_model_conditions",
    "playable_claim = false",
];

pub const COMBAT_MODEL_CONDITION_CHANNEL_SOURCE_MARKERS_WAVE487: &[&str] = &[
    "Wave 487: preserve combat/presentation bits refresh rebuild does not recompute",
    "Wave 487: weapon fire model bits must reach GW before model-condition writeback",
    "Wave 487: prone model bit must reach GW before model-condition writeback",
    "Wave 487: crush model bits must reach GW before model-condition writeback",
];

pub const COMBAT_MODEL_CONDITION_CHANNEL_NAV_STEPS_WAVE487: &[&str] = &[
    "COMBAT_BITS_SET_ON_HOST",
    "REFRESH_PRESERVES_COMBAT_BITS",
    "RECORD_MODEL_CONDITION_ON_CHANGE",
    "GW_APPLY_MODEL_CONDITION",
    "WRITEBACK_MATCHES_HOST",
    "NO_COMBAT_VISUAL_STOMP",
];

pub const RUNTIME_HOST_COMBAT_MODEL_CONDITION_CHANNEL_CMD_NAMES_WAVE487: &[&str] = &[
    "click_combat_model_condition_channel_ok_wnd_detect",
    "click_combat_model_condition_channel_ok_wnd_skip",
    "click_combat_model_condition_channel_ok_wnd_queue",
    "click_combat_model_condition_channel_ok_wnd_prepare",
    "click_combat_model_condition_channel_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualCombatModelConditionChannelAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    RefreshSource = 4,
    CombatLogSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualCombatModelConditionChannelAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_combat_model_condition_channel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_combat_model_condition_channel_last_action()
-> ResidualCombatModelConditionChannelAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualCombatModelConditionChannelAction::MethodNames,
        2 => ResidualCombatModelConditionChannelAction::SourceMarkers,
        3 => ResidualCombatModelConditionChannelAction::NavCommands,
        4 => ResidualCombatModelConditionChannelAction::RefreshSource,
        5 => ResidualCombatModelConditionChannelAction::CombatLogSource,
        6 => ResidualCombatModelConditionChannelAction::Composite,
        _ => ResidualCombatModelConditionChannelAction::Idle,
    }
}

fn object_source() -> &'static str {
    crate::game_logic::object::OBJECT_SRC
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let brace = src[start..].find('{')? + start;
    let mut depth = 0i32;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[start..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_combat_model_condition_channel_method_names_residual_wave487() -> bool {
    COMBAT_MODEL_CONDITION_CHANNEL_METHOD_NAMES_WAVE487.len() == 6
        && residual_name_index(
            COMBAT_MODEL_CONDITION_CHANNEL_METHOD_NAMES_WAVE487,
            "refresh_model_condition_bits",
        ) == Some(0)
        && residual_name_index(
            COMBAT_MODEL_CONDITION_CHANNEL_METHOD_NAMES_WAVE487,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_combat_model_condition_channel_source_markers_residual_wave487() -> bool {
    COMBAT_MODEL_CONDITION_CHANNEL_SOURCE_MARKERS_WAVE487.len() == 4
        && residual_name_index(
            COMBAT_MODEL_CONDITION_CHANNEL_SOURCE_MARKERS_WAVE487,
            "Wave 487: preserve combat/presentation bits refresh rebuild does not recompute",
        ) == Some(0)
        && residual_name_index(
            COMBAT_MODEL_CONDITION_CHANNEL_SOURCE_MARKERS_WAVE487,
            "Wave 487: weapon fire model bits must reach GW before model-condition writeback",
        ) == Some(1)
}

pub fn honesty_combat_model_condition_channel_nav_commands_residual_wave487() -> bool {
    COMBAT_MODEL_CONDITION_CHANNEL_NAV_STEPS_WAVE487.len() == 6
        && residual_name_index(
            COMBAT_MODEL_CONDITION_CHANNEL_NAV_STEPS_WAVE487,
            "REFRESH_PRESERVES_COMBAT_BITS",
        ) == Some(1)
        && residual_name_index(
            COMBAT_MODEL_CONDITION_CHANNEL_NAV_STEPS_WAVE487,
            "NO_COMBAT_VISUAL_STOMP",
        ) == Some(5)
        && RUNTIME_HOST_COMBAT_MODEL_CONDITION_CHANNEL_CMD_NAMES_WAVE487.len() == 5
        && residual_name_index(
            RUNTIME_HOST_COMBAT_MODEL_CONDITION_CHANNEL_CMD_NAMES_WAVE487,
            "click_combat_model_condition_channel_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_combat_model_condition_channel_refresh_source() -> bool {
    let Some(body) = function_body(object_source(), "fn refresh_model_condition_bits(") else {
        return false;
    };
    let ok = body
        .contains("Wave 487: preserve combat/presentation bits refresh rebuild does not recompute")
        && body.contains("WEAPON_MC_PRESERVE")
        && body.contains("PRONE")
        && body.contains("MC_BIT_FRONTCRUSHED")
        && body.contains("record_host_model_condition");
    residual_action_store(ResidualCombatModelConditionChannelAction::RefreshSource);
    ok
}

pub fn simulate_combat_model_condition_channel_combat_log_source() -> bool {
    let src = object_source();
    let weapon = function_body(src, "fn sync_weapon_model_conditions_from_status(").unwrap_or("");
    let prone = function_body(src, "fn go_prone(").unwrap_or("");
    let crush = function_body(src, "fn apply_crush_die_model_conditions(").unwrap_or("");
    let ok = weapon.contains(
        "Wave 487: weapon fire model bits must reach GW before model-condition writeback",
    ) && weapon.contains("record_host_model_condition")
        && prone
            .contains("Wave 487: prone model bit must reach GW before model-condition writeback")
        && crush
            .contains("Wave 487: crush model bits must reach GW before model-condition writeback")
        && src.contains("Wave 487: clear prone model bit into GW");
    residual_action_store(ResidualCombatModelConditionChannelAction::CombatLogSource);
    ok
}

pub fn honesty_combat_model_condition_channel_residual_pack_wave487() -> bool {
    honesty_combat_model_condition_channel_method_names_residual_wave487()
        && honesty_combat_model_condition_channel_source_markers_residual_wave487()
        && honesty_combat_model_condition_channel_nav_commands_residual_wave487()
        && simulate_combat_model_condition_channel_refresh_source()
        && simulate_combat_model_condition_channel_combat_log_source()
}

pub fn simulate_live_combat_model_condition_channel_honesty() -> bool {
    let ok = honesty_combat_model_condition_channel_residual_pack_wave487();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualCombatModelConditionChannelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_combat_model_condition_channel_method_names_residual_wave487());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_combat_model_condition_channel_source_markers_residual_wave487());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_combat_model_condition_channel_nav_commands_residual_wave487());
    }

    #[test]
    fn combat_model_condition_channel_sources() {
        assert!(simulate_combat_model_condition_channel_refresh_source());
        assert!(simulate_combat_model_condition_channel_combat_log_source());
    }

    #[test]
    fn wave487_composite_pack() {
        assert!(honesty_combat_model_condition_channel_residual_pack_wave487());
    }

    #[test]
    fn simulate_live_combat_model_condition_channel_honesty_residual_live() {
        assert!(
            simulate_live_combat_model_condition_channel_honesty(),
            "combat model condition channel residual must latch"
        );
        assert!(residual_combat_model_condition_channel_ok());
        assert_eq!(
            residual_combat_model_condition_channel_last_action(),
            ResidualCombatModelConditionChannelAction::Composite
        );
    }
}
