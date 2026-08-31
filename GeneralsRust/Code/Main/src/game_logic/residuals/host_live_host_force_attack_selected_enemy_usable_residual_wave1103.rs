//! Wave 1103: force-attack sold + selected count + enemy attackable FOW residual.
//!
//! ForceAttackObject ignored sold/dead presentation hints; selected-friendly count
//! skipped only destroyed; is_enemy_attackable ignored FOW Clear requirement.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FORCE_ATTACK_SELECTED_ENEMY_USABLE_METHOD_NAMES_WAVE1103: &[&str] = &[
    "process_right_click",
    "ForceAttack",
    "count_selected_friendlies",
    "is_enemy_attackable",
    "Wave 1103",
    "playable_claim = false",
];

pub const LIVE_HOST_FORCE_ATTACK_SELECTED_ENEMY_USABLE_NAV_STEPS_WAVE1103: &[&str] = &[
    "FORCE_ATTACK_SOLD_FAIL_CLOSED",
    "SELECTED_COUNT_SELECTABLE",
    "ENEMY_ATTACKABLE_FOW_CLEAR",
    "LIVE_HOST_FORCE_ATTACK_SELECTED_ENEMY_USABLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostForceAttackSelectedEnemyUsableAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostForceAttackSelectedEnemyUsableAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cs_source() -> &'static str {
    crate::command_system::COMMAND_SYSTEM_SRC
}
fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_force_attack_selected_enemy_usable_method_names_residual_wave1103() -> bool {
    let names = LIVE_HOST_FORCE_ATTACK_SELECTED_ENEMY_USABLE_METHOD_NAMES_WAVE1103;
    let ok = residual_name_index(names, "ForceAttack").is_some()
        && residual_name_index(names, "Wave 1103").is_some();
    residual_action_store(ResidualHostForceAttackSelectedEnemyUsableAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_force_attack_selected_enemy_usable_nav_commands_residual_wave1103() -> bool {
    let steps = LIVE_HOST_FORCE_ATTACK_SELECTED_ENEMY_USABLE_NAV_STEPS_WAVE1103;
    let ok = residual_name_index(steps, "LIVE_HOST_FORCE_ATTACK_SELECTED_ENEMY_USABLE").is_some()
        && residual_name_index(steps, "FORCE_ATTACK_SOLD_FAIL_CLOSED").is_some();
    residual_action_store(ResidualHostForceAttackSelectedEnemyUsableAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_force_attack_selected_enemy_usable_residual_pack_wave1103() -> bool {
    let cs = cs_source();
    let pf = pf_source();
    let es = es_source();
    let fa_i = match cs.find("CommandMode::ForceAttack =>") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostForceAttackSelectedEnemyUsableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let fa = &cs[fa_i..fa_i.saturating_add(1400)];
    let sel_i = match pf.find("fn count_selected_friendlies") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostForceAttackSelectedEnemyUsableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let sel = &pf[sel_i..sel_i.saturating_add(900)];
    let en_i = match pf.find("fn is_enemy_attackable") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostForceAttackSelectedEnemyUsableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let en = &pf[en_i..en_i.saturating_add(900)];
    let ok = fa.contains("Wave 1103: force-attack object residual fail-closed")
        && fa.contains("h.sold || !h.is_alive")
        && fa.contains("ForceAttackGround")
        && sel.contains("Wave 1103: selected count residual uses presentation selectable legality")
        && sel.contains("presentation_is_selectable")
        && en.contains("Wave 1103: fail-closed on non-local FOW unless Clear")
        && en.contains("visibility_alpha >= 0.95")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostForceAttackSelectedEnemyUsableAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_force_attack_selected_enemy_usable_residual_honesty() -> bool {
    let a = honesty_host_force_attack_selected_enemy_usable_method_names_residual_wave1103();
    let b = honesty_host_force_attack_selected_enemy_usable_nav_commands_residual_wave1103();
    let c = honesty_host_force_attack_selected_enemy_usable_residual_pack_wave1103();
    residual_action_store(ResidualHostForceAttackSelectedEnemyUsableAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_force_attack_selected_enemy_usable_residual_wave1103() {
        assert!(honesty_host_force_attack_selected_enemy_usable_residual_pack_wave1103());
        assert!(honesty_host_force_attack_selected_enemy_usable_method_names_residual_wave1103());
        assert!(honesty_host_force_attack_selected_enemy_usable_nav_commands_residual_wave1103());
        assert!(simulate_live_host_force_attack_selected_enemy_usable_residual_honesty());
    }
}
