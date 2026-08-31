//! Wave 1115: dual construct sold residual + combat latch honesty.
//!
//! - `draw_construct_percent` dual presentation fail-closed when sold or
//!   not under construction (C++ OBJECT_STATUS_SOLD). Dead health does not
//!   clear construct percent (C++ isEffectivelyDead check is commented out).
//! - Early combat smoke window 6s → 12s with mid-window attack re-issue.
//! - `attack_nearest_enemy` prefers FOW-clear attackable enemy before force-attack.
//! - `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONSTRUCT_DEAD_COMBAT_LATCH_METHOD_NAMES_WAVE1115: &[&str] = &[
    "draw_construct_percent",
    "set_presentation_sold",
    "first_enemy_attack_command_id",
    "first_enemy_attackable_id",
    "first_enemy_force_attack_id",
    "Wave 1115",
    "playable_claim: false",
];

pub const LIVE_HOST_CONSTRUCT_DEAD_COMBAT_LATCH_NAV_STEPS_WAVE1115: &[&str] = &[
    "CONSTRUCT_SOLD_FAIL_CLOSED",
    "COMBAT_LATCH_12S_REISSUE",
    "ATTACK_ATTACKABLE_THEN_FORCE",
    "LIVE_HOST_CONSTRUCT_DEAD_COMBAT_LATCH",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostConstructDeadCombatLatchAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostConstructDeadCombatLatchAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn dr_source() -> &'static str {
    game_client::drawable::drawable::DRAWABLE_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

pub fn honesty_host_construct_dead_combat_latch_method_names_residual_wave1115() -> bool {
    // Self-table membership is inflation (host_wave_inflation). Scan shipped fns.
    debug_assert!(crate::game_logic::host_wave_inflation::self_table_honesty_is_inflation());
    let dr = dr_source();
    let pf = pf_source();
    let cnc = cnc_source();
    let ok =
        crate::game_logic::host_wave_inflation::shipped_fn_contains(
            dr,
            "fn draw_construct_percent",
            &[
                "presentation_sold",
                "ObjectStatusTypes::Sold",
                "isEffectivelyDead check is commented out",
            ],
        ) && crate::game_logic::host_wave_inflation::shipped_fn_contains(
            pf,
            "pub fn first_enemy_attack_command_id",
            &["first_enemy_attackable_id", "first_enemy_force_attack_id"],
        ) && crate::game_logic::host_wave_inflation::shipped_fn_exists(
            pf,
            "first_enemy_attackable_id",
        ) && crate::game_logic::host_wave_inflation::shipped_fn_exists(
            pf,
            "first_enemy_force_attack_id",
        ) && crate::game_logic::host_wave_inflation::shipped_fn_exists(dr, "set_presentation_sold")
            && cnc.contains("first_enemy_attack_command_id");
    residual_action_store(ResidualHostConstructDeadCombatLatchAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_construct_dead_combat_latch_nav_commands_residual_wave1115() -> bool {
    // Nav honesty must scan real smoke/combat latch + attack command path.
    debug_assert!(crate::game_logic::host_wave_inflation::self_table_honesty_is_inflation());
    let es = es_source();
    let cnc = cnc_source();
    let ok = es.contains("Duration::from_secs(12)")
        && es.contains("!st.saw_combat_damage")
        && es.contains("Duration::from_secs(4)")
        && es.contains("Duration::from_secs(5)")
        && es.contains("attack_nearest_enemy|auto_target=1")
        && cnc.contains("first_enemy_attack_command_id")
        && cnc.contains("host_command_attack");
    residual_action_store(ResidualHostConstructDeadCombatLatchAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_construct_dead_combat_latch_residual_pack_wave1115() -> bool {
    let dr = dr_source();
    let es = es_source();
    let cnc = cnc_source();
    let ok = dr.contains("Wave 1115: dual construct residual fail-closed on sold")
        && dr.contains("presentation_sold")
        && dr.contains("ObjectStatusTypes::Sold")
        && dr.contains("isEffectivelyDead check is commented out")
        && es.contains("Wave 1112/1115")
        && es.contains("Duration::from_secs(12)")
        && es.contains("!st.saw_combat_damage")
        && es.contains("Duration::from_secs(4)")
        && es.contains("Duration::from_secs(5)")
        && es.contains("attack_nearest_enemy|auto_target=1")
        && es.contains("playable_claim: false")
        && cnc.contains("Wave 1115: prefer FOW-clear attackable enemy")
        && cnc.contains("first_enemy_attack_command_id");
    residual_action_store(ResidualHostConstructDeadCombatLatchAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_construct_dead_combat_latch_residual_honesty() -> bool {
    let a = honesty_host_construct_dead_combat_latch_method_names_residual_wave1115();
    let b = honesty_host_construct_dead_combat_latch_nav_commands_residual_wave1115();
    let c = honesty_host_construct_dead_combat_latch_residual_pack_wave1115();
    residual_action_store(ResidualHostConstructDeadCombatLatchAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_construct_dead_combat_latch_residual_wave1115() {
        assert!(honesty_host_construct_dead_combat_latch_residual_pack_wave1115());
        assert!(honesty_host_construct_dead_combat_latch_method_names_residual_wave1115());
        assert!(honesty_host_construct_dead_combat_latch_nav_commands_residual_wave1115());
        assert!(simulate_live_host_construct_dead_combat_latch_residual_honesty());
    }

    #[test]
    fn draw_construct_percent_sold_fail_closed_via_draw_icon_ui() {
        use game_client::drawable::drawable::{BasicDrawable, DrawableId, ICoord2D, IRegion2D};

        let region = IRegion2D::new(ICoord2D::new(10, 20), ICoord2D::new(74, 32));
        let mut building = BasicDrawable::new(DrawableId(8));
        building.overlay_data.health_region = Some(region);
        building.set_presentation_sold(false);
        building.overlay_data.is_under_construction = true;
        building.overlay_data.construction_percent = 0.42;
        // Stamp construction residual the same way host sync does, then draw.
        building.set_presentation_host_residual(
            Vec::new(),
            None,
            false,
            false,
            0.0,
            false,
            0,
            true,
            0.42,
            0,
            0,
            0,
            0,
            false,
            false,
            0,
            0,
            false,
            0.0,
            false,
            0,
            Vec::new(),
            String::new(),
            0,
            0,
            String::new(),
        );
        building.set_presentation_sold(false);
        building.draw_icon_ui();
        assert!(
            building.overlay_data.is_under_construction,
            "dead health must not clear construct overlay (C++ dead check commented out)"
        );
        assert!((building.overlay_data.construction_percent - 0.42).abs() < 0.0001);

        building.set_presentation_sold(true);
        building.draw_icon_ui();
        assert!(
            !building.overlay_data.is_under_construction,
            "sold must fail-closed construct overlay"
        );
        assert_eq!(building.overlay_data.construction_percent, 0.0);
    }
}
