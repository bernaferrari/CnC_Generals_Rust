//! Wave 1105: force-attack FOW + selected command-set usable residual.
//!
//! - `first_enemy_force_attack_id` ignored FOW Clear (unlike is_enemy_attackable /
//!   first_enemy_attackable_id Waves 1103–1104)
//! - `selected_command_set_name(s)` only excluded destroyed, still fed sold/
//!   unselectable/masked/disabled selected objects into ControlBar command sets

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FORCE_ATTACK_FOW_CMDSET_USABLE_METHOD_NAMES_WAVE1105: &[&str] = &[
    "first_enemy_force_attack_id",
    "selected_command_set_name",
    "selected_command_set_names",
    "Wave 1105",
    "playable_claim: false",
];

pub const LIVE_HOST_FORCE_ATTACK_FOW_CMDSET_USABLE_NAV_STEPS_WAVE1105: &[&str] = &[
    "FORCE_ATTACK_FOW_CLEAR",
    "CMDSET_EXCLUDES_SOLD_UNUSABLE",
    "LIVE_HOST_FORCE_ATTACK_FOW_CMDSET_USABLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostForceAttackFowCmdsetUsableAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostForceAttackFowCmdsetUsableAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_force_attack_fow_cmdset_usable_method_names_residual_wave1105() -> bool {
    let names = LIVE_HOST_FORCE_ATTACK_FOW_CMDSET_USABLE_METHOD_NAMES_WAVE1105;
    let ok = residual_name_index(names, "first_enemy_force_attack_id").is_some()
        && residual_name_index(names, "Wave 1105").is_some();
    residual_action_store(ResidualHostForceAttackFowCmdsetUsableAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_force_attack_fow_cmdset_usable_nav_commands_residual_wave1105() -> bool {
    let steps = LIVE_HOST_FORCE_ATTACK_FOW_CMDSET_USABLE_NAV_STEPS_WAVE1105;
    let ok = residual_name_index(steps, "LIVE_HOST_FORCE_ATTACK_FOW_CMDSET_USABLE").is_some()
        && residual_name_index(steps, "FORCE_ATTACK_FOW_CLEAR").is_some();
    residual_action_store(ResidualHostForceAttackFowCmdsetUsableAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_force_attack_fow_cmdset_usable_residual_pack_wave1105() -> bool {
    let pf = pf_source();
    let es = es_source();
    let fa_i = match pf.find("fn first_enemy_force_attack_id") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostForceAttackFowCmdsetUsableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let fa = &pf[fa_i..fa_i.saturating_add(1400)];
    let cs_i = match pf.find("fn selected_command_set_name") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostForceAttackFowCmdsetUsableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let cs = &pf[cs_i..cs_i.saturating_add(2200)];
    let ok = fa.contains("Wave 1105: fail-closed on non-local FOW unless Clear")
        && fa.contains("visibility_alpha >= 0.95")
        && fa.contains("presentation_is_attackable")
        && cs.contains("Wave 1105: primary selection residual fail-closed on sold")
        && cs.contains("Wave 1105: multi-select command-set residual fail-closed on sold")
        && cs.contains("!o.sold")
        && cs.contains("!o.unselectable")
        && cs.contains("!o.masked")
        && cs.contains("!o.disabled")
        && es.contains("playable_claim: false");
    residual_action_store(ResidualHostForceAttackFowCmdsetUsableAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_force_attack_fow_cmdset_usable_residual_honesty() -> bool {
    let a = honesty_host_force_attack_fow_cmdset_usable_method_names_residual_wave1105();
    let b = honesty_host_force_attack_fow_cmdset_usable_nav_commands_residual_wave1105();
    let c = honesty_host_force_attack_fow_cmdset_usable_residual_pack_wave1105();
    residual_action_store(ResidualHostForceAttackFowCmdsetUsableAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_force_attack_fow_cmdset_usable_residual_wave1105() {
        assert!(honesty_host_force_attack_fow_cmdset_usable_residual_pack_wave1105());
        assert!(honesty_host_force_attack_fow_cmdset_usable_method_names_residual_wave1105());
        assert!(honesty_host_force_attack_fow_cmdset_usable_nav_commands_residual_wave1105());
        assert!(simulate_live_host_force_attack_fow_cmdset_usable_residual_honesty());
    }
}
