//! Regression: coupled shadow session must preserve host radar residual.
//!
//! hq-9udt7 minimap seam: mirrors the windowed runtime-host flow — one
//! `GameLogic` + one persistent `GameWorldShadow` (fresh and shell→skirmish
//! variants) with a match-start AmericaCommandCenter for the local player.
//! C++ oracle: Player::addRadar/radarCount persist across logic ticks
//! (GeneralsMD Player.cpp radar fields); the shadow sync mirrors host state
//! and must never zero derived radar state.

use super::*;
use crate::game_logic::GameMode;

const CC_TEMPLATE: &str = "AmericaCommandCenter";

/// Retail AmericaCommandCenter and dozer, authored before NewGame
/// (GameEngine.cpp:428-458). Radar comes from the CC's authored RadarUpgrade.
fn admit_radar_content(logic: &mut GameLogic) {
    use crate::fixture_content::{admit_fixture_objects, FixtureObject};
    admit_fixture_objects(
        logic,
        &[
            FixtureObject::new(CC_TEMPLATE)
                .kind_of("STRUCTURE COMMANDCENTER SELECTABLE MP_COUNT_FOR_VICTORY")
                .health(5000.0)
                .radar_provider(),
            FixtureObject::new("AmericaVehicleDozer")
                .kind_of("VEHICLE DOZER SELECTABLE")
                .walker()
                .dozer(),
        ],
    );
}

fn spawn_constructed_cc(logic: &mut GameLogic, owner: u32, at: Vec3) -> u32 {
    let id = logic
        .create_object_for_player(CC_TEMPLATE, owner, at)
        .expect("cc spawn");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.set_status_under_construction(false);
        obj.construction_percent = 1.0;
    }
    id.0
}

fn local_player_id(logic: &GameLogic) -> u32 {
    logic.local_player_id().expect("local player")
}

fn coupled_session(shadow: &mut GameWorldShadow, logic: &mut GameLogic) {
    observe_completed_owner(shadow, logic);
}

fn assert_radar_kept(logic: &GameLogic, local: u32, ctx: &str) {
    let p = logic.get_player(local).expect("player");
    assert_eq!(
        p.radar_count, 1,
        "{ctx}: coupled session must not zero host radar_count"
    );
    assert!(
        p.has_radar(),
        "{ctx}: coupled session must not drop has_radar"
    );
}

#[test]
fn fresh_shadow_skirmish_coupled_session_keeps_local_radar() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MinimapSeamFresh");
    admit_radar_content(&mut logic);
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let local = local_player_id(&logic);
    let _cc = spawn_constructed_cc(&mut logic, local, Vec3::new(10.0, 0.0, 10.0));

    let mut shadow = GameWorldShadow::new(4096);
    for frame in 0..4u32 {
        logic.update_player_radar();
        coupled_session(&mut shadow, &mut logic);
        assert_radar_kept(&logic, local, &format!("fresh frame {frame}"));
    }
}

#[test]
fn shell_to_skirmish_coupled_session_keeps_local_radar() {
    // Shell phase (live: GAME_SHELL with bootstrap players), then match start
    // on the SAME GameLogic + persistent GameWorldShadow.
    let mut logic = GameLogic::new();
    admit_radar_content(&mut logic);
    logic.start_new_game(GameMode::Shell).unwrap();
    let _prop = logic
        .create_object("AmericaVehicleDozer", Team::USA, Vec3::new(1.0, 0.0, 1.0))
        .expect("shell prop");

    let mut shadow = GameWorldShadow::new(4096);
    for _ in 0..2 {
        logic.update_player_radar();
        coupled_session(&mut shadow, &mut logic);
    }

    let cfg = golden_skirmish_config("MinimapSeamRepro");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let local = local_player_id(&logic);
    let _cc = spawn_constructed_cc(&mut logic, local, Vec3::new(10.0, 0.0, 10.0));
    logic.update_player_radar();
    assert_eq!(
        logic.get_player(local).expect("p").radar_count,
        1,
        "host recompute must grant radar before session"
    );

    for frame in 0..3u32 {
        logic.update_player_radar();
        coupled_session(&mut shadow, &mut logic);
        assert_radar_kept(&logic, local, &format!("shell→match frame {frame}"));
    }
}
