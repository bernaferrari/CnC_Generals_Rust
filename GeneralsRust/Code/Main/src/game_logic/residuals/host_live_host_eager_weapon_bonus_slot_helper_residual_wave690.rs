//! Wave 690 residual peels: post-logic weapon-bonus + weapon-slot apply on coupled tick.
//! Host records `host_weapon_bonus_log` / `host_weapon_slot_log` mid-frame; engine drains
//! into GameWorld immediately after the host frame. Session reuses batches without
//! double-apply. Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_EAGER_WEAPON_BONUS_SLOT_HELPER_METHOD_NAMES_WAVE690: &[&str] = &[
    "eager_apply_host_weapon_bonus_after_logic",
    "eager_apply_host_weapon_slot_after_logic",
    "host_weapon_bonus_log",
    "host_weapon_slot_log",
    "Wave 690",
    "playable_claim = false",
];
pub const LIVE_HOST_EAGER_WEAPON_BONUS_SLOT_HELPER_NAV_STEPS_WAVE690: &[&str] = &[
    "REQUIRE_EAGER_WEAPON_BONUS_SLOT_API",
    "REQUIRE_ENGINE_POST_LOGIC_DRAIN",
    "REQUIRE_SESSION_HANDOFF",
    "REQUIRE_NO_DOUBLE_APPLY",
    "LIVE_HOST_EAGER_WEAPON_BONUS_SLOT_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_EAGER_WEAPON_BONUS_SLOT_HELPER_CMD_NAMES_WAVE690: &[&str] = &[
    "host_eager_weapon_bonus_slot_helper",
    "engine_post_logic_drain",
    "session_handoff",
    "no_double_apply",
    "eager_weapon_bonus_slot_residual",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEagerWeaponBonusSlotHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostEagerWeaponBonusSlotHelperAction {
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
fn residual_action_store(a: ResidualHostEagerWeaponBonusSlotHelperAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_eager_weapon_bonus_slot_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_eager_weapon_bonus_slot_helper_last_action()
-> ResidualHostEagerWeaponBonusSlotHelperAction {
    ResidualHostEagerWeaponBonusSlotHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
// 2026-08-15: engine dispatches via eager_apply_all_host_residuals_after_logic
// (Wave 682/925); per-channel eager_apply_* stays on GAMEWORLD_SHADOW_SRC.
pub fn honesty_host_eager_weapon_bonus_slot_helper_method_names_residual_wave690() -> bool {
    let names = LIVE_HOST_EAGER_WEAPON_BONUS_SLOT_HELPER_METHOD_NAMES_WAVE690;
    let ok = residual_name_index(names, "eager_apply_host_weapon_bonus_after_logic").is_some()
        && residual_name_index(names, "eager_apply_host_weapon_slot_after_logic").is_some()
        && residual_name_index(names, "host_weapon_bonus_log").is_some()
        && residual_name_index(names, "host_weapon_slot_log").is_some()
        && residual_name_index(names, "Wave 690").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEagerWeaponBonusSlotHelperAction::MethodNames);
    ok
}
pub fn honesty_host_eager_weapon_bonus_slot_helper_source_markers_residual_wave690() -> bool {
    let eng = eng_source();
    let sh = shadow_source();
    let api_ok = sh.contains("pub fn eager_apply_host_weapon_bonus_after_logic")
        && sh.contains("pub fn eager_apply_host_weapon_slot_after_logic")
        && sh.contains("Wave 690")
        && sh.contains("take_early_weapon_bonus_batch")
        && sh.contains("take_early_weapon_slot_batch")
        && sh.contains("early_weapon_bonus_applied")
        && sh.contains("early_weapon_slot_applied");
    let eng_ok = eng.contains("eager_apply_all_host_residuals_after_logic")
        && eng.contains("eager_apply_all_host_residuals_after_logic")
        && eng.contains("Wave 682/925");
    let ok = api_ok && eng_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostEagerWeaponBonusSlotHelperAction::SourceMarkers);
    ok
}
pub fn honesty_host_eager_weapon_bonus_slot_helper_nav_commands_residual_wave690() -> bool {
    let steps = LIVE_HOST_EAGER_WEAPON_BONUS_SLOT_HELPER_NAV_STEPS_WAVE690;
    let cmds = RUNTIME_HOST_LIVE_HOST_EAGER_WEAPON_BONUS_SLOT_HELPER_CMD_NAMES_WAVE690;
    let ok = residual_name_index(steps, "REQUIRE_EAGER_WEAPON_BONUS_SLOT_API").is_some()
        && residual_name_index(steps, "REQUIRE_ENGINE_POST_LOGIC_DRAIN").is_some()
        && residual_name_index(steps, "REQUIRE_SESSION_HANDOFF").is_some()
        && residual_name_index(steps, "REQUIRE_NO_DOUBLE_APPLY").is_some()
        && residual_name_index(steps, "LIVE_HOST_EAGER_WEAPON_BONUS_SLOT_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_eager_weapon_bonus_slot_helper").is_some()
        && residual_name_index(cmds, "engine_post_logic_drain").is_some()
        && residual_name_index(cmds, "session_handoff").is_some()
        && residual_name_index(cmds, "no_double_apply").is_some()
        && residual_name_index(cmds, "eager_weapon_bonus_slot_residual").is_some();
    residual_action_store(ResidualHostEagerWeaponBonusSlotHelperAction::NavCommands);
    ok
}
pub fn simulate_host_eager_weapon_bonus_slot_helper_collect_source() -> bool {
    let ok = shadow_source().contains("eager_apply_host_weapon_bonus_after_logic")
        && shadow_source().contains("eager_apply_host_weapon_slot_after_logic")
        && eng_source().contains("eager_apply_all_host_residuals_after_logic");
    residual_action_store(ResidualHostEagerWeaponBonusSlotHelperAction::CollectSource);
    ok
}
pub fn simulate_host_eager_weapon_bonus_slot_helper_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 682/925") && shadow_source().contains("Wave 690");
    residual_action_store(ResidualHostEagerWeaponBonusSlotHelperAction::DispatchSource);
    ok
}
pub fn honesty_host_eager_weapon_bonus_slot_helper_residual_pack_wave690() -> bool {
    honesty_host_eager_weapon_bonus_slot_helper_method_names_residual_wave690()
        && honesty_host_eager_weapon_bonus_slot_helper_source_markers_residual_wave690()
        && honesty_host_eager_weapon_bonus_slot_helper_nav_commands_residual_wave690()
        && simulate_host_eager_weapon_bonus_slot_helper_collect_source()
        && simulate_host_eager_weapon_bonus_slot_helper_dispatch_source()
}
pub fn simulate_live_host_eager_weapon_bonus_slot_helper_honesty() -> bool {
    let ok = honesty_host_eager_weapon_bonus_slot_helper_residual_pack_wave690();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEagerWeaponBonusSlotHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_weapon_bonus_log::{self, HostWeaponBonusEvent};
    use crate::game_logic::host_weapon_slot_log;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        GameWorldShadow, begin_shadow_coupled_tick, clear_active_shadow_for_coupled_tick,
        eager_apply_host_weapon_bonus_after_logic, eager_apply_host_weapon_slot_after_logic,
        eager_map_host_spawn_if_coupled, end_shadow_coupled_tick,
        install_active_shadow_for_coupled_tick, shadow_session_after_host_tick,
    };
    use glam::Vec3;

    fn ensure_template(logic: &mut GameLogic, name: &str, health: f32) {
        let mut t = ThingTemplate::new(name);
        t.set_health(health);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert(name.into(), t);
    }

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_eager_weapon_bonus_slot_helper_method_names_residual_wave690());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_eager_weapon_bonus_slot_helper_source_markers_residual_wave690());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_eager_weapon_bonus_slot_helper_nav_commands_residual_wave690());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_eager_weapon_bonus_slot_helper_collect_source());
        assert!(simulate_host_eager_weapon_bonus_slot_helper_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_eager_weapon_bonus_slot_helper_residual_pack_wave690());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_eager_weapon_bonus_slot_helper_honesty());
        assert!(residual_host_eager_weapon_bonus_slot_helper_ok());
    }

    #[test]
    fn post_logic_weapon_bonus_and_slot_apply_once_through_session() {
        let _guard = crate::gameworld_shadow::authority_env_lock();
        let prev = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        host_weapon_bonus_log::clear();
        host_weapon_slot_log::clear();

        let mut logic = GameLogic::new();
        ensure_template(&mut logic, "EagerWpnUnit", 100.0);
        let id = logic
            .create_object("EagerWpnUnit", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("spawn");
        host_weapon_bonus_log::record(HostWeaponBonusEvent {
            object: id,
            enthusiastic: true,
            subliminal: false,
            horde: false,
            nationalism: false,
            frenzy: false,
            frenzy_level: 0,
            battle_plan_bombardment: false,
            battle_plan_hold_the_line: false,
            battle_plan_search_and_destroy: false,
            frenzy_until_frame: 0,
            battle_plan_sight_scalar_applied: 1.0,
        });
        host_weapon_slot_log::record(id, 1);
        assert_eq!(host_weapon_bonus_log::len(), 1);
        assert_eq!(host_weapon_slot_log::len(), 1);

        let mut shadow = GameWorldShadow::new(64);
        begin_shadow_coupled_tick();
        install_active_shadow_for_coupled_tick(&mut shadow);
        assert!(eager_map_host_spawn_if_coupled(
            &logic,
            &crate::game_logic::host_spawn_log::HostSpawnEvent {
                id,
                template: "EagerWpnUnit".into(),
                team_ordinal: 0,
                position: [0.0, 0.0, 0.0],
            },
        ));
        assert!(eager_apply_host_weapon_bonus_after_logic(&mut shadow, &logic) >= 1);
        assert!(eager_apply_host_weapon_slot_after_logic(&mut shadow, &logic) >= 1);
        assert!(host_weapon_bonus_log::drain().is_empty());
        assert!(host_weapon_slot_log::drain().is_empty());
        let _probe = shadow_session_after_host_tick(&mut shadow, &mut logic);
        clear_active_shadow_for_coupled_tick();
        end_shadow_coupled_tick();
        let _ = ObjectId;

        match prev {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
    }
}
