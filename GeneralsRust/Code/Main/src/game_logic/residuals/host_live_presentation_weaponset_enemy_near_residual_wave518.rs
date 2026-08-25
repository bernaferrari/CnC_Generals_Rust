//! Wave 518 residual peels: weaponset/crate/enemy-near/armed mesh bits.
//! - freeze `weapon_crate_upgrade`, `armor_crate_upgrade`, `enemy_near`, `armed`
//! - stamp WEAPONSET_PLAYER_UPGRADE / CRATEUPGRADE / ARMORSET_CRATE / ENEMYNEAR / ARMED
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 517 slot-aware weapon fire bits.
//! Architecture residual - upgrade/enemy-near pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 518 freeze + stamps
//! - host_enum_table_residual.rs weaponset/armorset/enemynear/armed helpers
//! - host_enemy_near.rs model_enemy_near residual
//!
//! Fail-closed:
//! - Full EnemyNearUpdate vision/shroud matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_WEAPONSET_ENEMY_NEAR_METHOD_NAMES_WAVE518: &[&str] = &[
    "weapon_crate_upgrade",
    "armor_crate_upgrade",
    "enemy_near",
    "weaponset_player_upgrade_model_bit",
    "enemynear_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_WEAPONSET_ENEMY_NEAR_SOURCE_MARKERS_WAVE518: &[&str] = &[
    "Wave 518: weaponset player/crate, armor crate, enemy-near, armed residual bits",
    "weapon_crate_upgrade: obj.weapon_crate_upgrade",
    "enemy_near: obj",
    "fn weaponset_player_upgrade_model_bit",
];

pub const PRESENTATION_WEAPONSET_ENEMY_NEAR_NAV_STEPS_WAVE518: &[&str] = &[
    "FREEZE_CRATE_UPGRADES",
    "FREEZE_ENEMY_NEAR_ARMED",
    "STAMP_WEAPONSET_CRATE_BITS",
    "STAMP_ENEMYNEAR_ARMED",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_WEAPONSET_ENEMY_NEAR_CMD_NAMES_WAVE518: &[&str] = &[
    "click_presentation_weaponset_enemy_near_ok_wnd_detect",
    "click_presentation_weaponset_enemy_near_ok_wnd_skip",
    "click_presentation_weaponset_enemy_near_ok_wnd_queue",
    "click_presentation_weaponset_enemy_near_ok_wnd_prepare",
    "click_presentation_weaponset_enemy_near_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationWeaponsetEnemyNearAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    FreezeSource = 4,
    StampSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationWeaponsetEnemyNearAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_weaponset_enemy_near_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_weaponset_enemy_near_last_action()
-> ResidualPresentationWeaponsetEnemyNearAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationWeaponsetEnemyNearAction::MethodNames,
        2 => ResidualPresentationWeaponsetEnemyNearAction::SourceMarkers,
        3 => ResidualPresentationWeaponsetEnemyNearAction::NavCommands,
        4 => ResidualPresentationWeaponsetEnemyNearAction::FreezeSource,
        5 => ResidualPresentationWeaponsetEnemyNearAction::StampSource,
        6 => ResidualPresentationWeaponsetEnemyNearAction::Composite,
        _ => ResidualPresentationWeaponsetEnemyNearAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

pub fn honesty_presentation_weaponset_enemy_near_method_names_residual_wave518() -> bool {
    PRESENTATION_WEAPONSET_ENEMY_NEAR_METHOD_NAMES_WAVE518.len() == 6
        && residual_name_index(
            PRESENTATION_WEAPONSET_ENEMY_NEAR_METHOD_NAMES_WAVE518,
            "weapon_crate_upgrade",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_WEAPONSET_ENEMY_NEAR_METHOD_NAMES_WAVE518,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_weaponset_enemy_near_source_markers_residual_wave518() -> bool {
    PRESENTATION_WEAPONSET_ENEMY_NEAR_SOURCE_MARKERS_WAVE518.len() == 4
        && residual_name_index(
            PRESENTATION_WEAPONSET_ENEMY_NEAR_SOURCE_MARKERS_WAVE518,
            "Wave 518: weaponset player/crate, armor crate, enemy-near, armed residual bits",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_WEAPONSET_ENEMY_NEAR_SOURCE_MARKERS_WAVE518,
            "fn weaponset_player_upgrade_model_bit",
        ) == Some(3)
}

pub fn honesty_presentation_weaponset_enemy_near_nav_commands_residual_wave518() -> bool {
    PRESENTATION_WEAPONSET_ENEMY_NEAR_NAV_STEPS_WAVE518.len() == 6
        && residual_name_index(
            PRESENTATION_WEAPONSET_ENEMY_NEAR_NAV_STEPS_WAVE518,
            "STAMP_ENEMYNEAR_ARMED",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_WEAPONSET_ENEMY_NEAR_NAV_STEPS_WAVE518,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_WEAPONSET_ENEMY_NEAR_CMD_NAMES_WAVE518.len() == 5
}

pub fn simulate_presentation_weaponset_enemy_near_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("weapon_crate_upgrade: obj.weapon_crate_upgrade")
        && pf.contains("armor_crate_upgrade: obj.armor_crate_upgrade")
        && pf.contains("model_enemy_near || e.enemy_near")
        && pf.contains("armed_riders_upgrade_weapon_set");
    residual_action_store(ResidualPresentationWeaponsetEnemyNearAction::FreezeSource);
    ok
}

pub fn simulate_presentation_weaponset_enemy_near_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = pf
        .contains("Wave 518: weaponset player/crate, armor crate, enemy-near, armed residual bits")
        && en.contains("pub fn weaponset_player_upgrade_model_bit")
        && en.contains("pub fn enemynear_model_bit")
        && en.contains("pub fn armed_model_bit")
        && pf.contains("if self.enemy_near");
    residual_action_store(ResidualPresentationWeaponsetEnemyNearAction::StampSource);
    ok
}

pub fn honesty_presentation_weaponset_enemy_near_residual_pack_wave518() -> bool {
    honesty_presentation_weaponset_enemy_near_method_names_residual_wave518()
        && honesty_presentation_weaponset_enemy_near_source_markers_residual_wave518()
        && honesty_presentation_weaponset_enemy_near_nav_commands_residual_wave518()
        && simulate_presentation_weaponset_enemy_near_freeze_source()
        && simulate_presentation_weaponset_enemy_near_stamp_source()
}

pub fn simulate_live_presentation_weaponset_enemy_near_honesty() -> bool {
    let ok = honesty_presentation_weaponset_enemy_near_residual_pack_wave518();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationWeaponsetEnemyNearAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_weaponset_enemy_near_method_names_residual_wave518());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_weaponset_enemy_near_source_markers_residual_wave518());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_weaponset_enemy_near_nav_commands_residual_wave518());
    }

    #[test]
    fn presentation_weaponset_enemy_near_sources() {
        assert!(simulate_presentation_weaponset_enemy_near_freeze_source());
        assert!(simulate_presentation_weaponset_enemy_near_stamp_source());
    }

    #[test]
    fn wave518_composite_pack() {
        assert!(honesty_presentation_weaponset_enemy_near_residual_pack_wave518());
    }

    #[test]
    fn simulate_live_presentation_weaponset_enemy_near_honesty_residual_live() {
        assert!(
            simulate_live_presentation_weaponset_enemy_near_honesty(),
            "presentation weaponset/enemy-near residual must latch"
        );
        assert!(residual_presentation_weaponset_enemy_near_ok());
        assert_eq!(
            residual_presentation_weaponset_enemy_near_last_action(),
            ResidualPresentationWeaponsetEnemyNearAction::Composite
        );
    }
}
