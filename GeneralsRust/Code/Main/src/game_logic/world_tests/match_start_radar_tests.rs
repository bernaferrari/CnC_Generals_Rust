//! Match-start radar stays available through Player::addRadar and publication.
use crate::game_logic::{GameLogic, GameMode, Team};
use crate::presentation_frame::PresentationFrame;
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use glam::Vec3;

const CC_TEMPLATE: &str = "AmericaCommandCenter";

/// Retail AmericaCommandCenter and dozer, authored before NewGame
/// (GameEngine.cpp:428-458). Radar comes from the CC's authored RadarUpgrade.
fn admit_radar_content(logic: &mut GameLogic) {
    use crate::fixture_content::{FixtureObject, admit_fixture_objects};
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

fn update_and_publish_radar(logic: &mut GameLogic, local: u32) {
    logic.update_player_radar();
    logic.complete_presentation_observations();
    let frame = PresentationFrame::try_build_from_logic(logic, local).unwrap();
    let player = logic.get_player(local).unwrap();
    assert_eq!(player.radar_count, 1);
    assert!(player.has_radar());
    assert_eq!(frame.local_radar_count, 1);
    assert!(frame.local_radar_active());
}

#[test]
fn fresh_skirmish_publishes_match_start_radar() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MinimapSeamFresh");
    admit_radar_content(&mut logic);
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let local = local_player_id(&logic);
    let _cc = spawn_constructed_cc(&mut logic, local, Vec3::new(10.0, 0.0, 10.0));

    for _ in 0..4 {
        update_and_publish_radar(&mut logic, local);
    }
}

#[test]
fn shell_to_skirmish_publishes_match_start_radar() {
    // The same GameLogic hosts the shell and the match.
    let mut logic = GameLogic::new();
    admit_radar_content(&mut logic);
    logic.start_new_game(GameMode::Shell).unwrap();
    let _prop = logic
        .create_object("AmericaVehicleDozer", Team::USA, Vec3::new(1.0, 0.0, 1.0))
        .expect("shell prop");

    for _ in 0..2 {
        logic.update_player_radar();
        logic.complete_presentation_observations();
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

    for _ in 0..3 {
        update_and_publish_radar(&mut logic, local);
    }
}
