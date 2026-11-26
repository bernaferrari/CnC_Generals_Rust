//! Published frames freeze the roster, player facts and body health.
use crate::fixture_content::{FixtureObject, admit_fixture_objects};
use crate::game_logic::{GameLogic, Team, host_heal_log::HostHealEvent};
use crate::presentation_frame::PresentationFrame;
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use glam::Vec3;

#[test]
fn saved_frames_keep_short_lived_object_ids_after_physical_removal() {
    let mut logic = GameLogic::new();
    admit_fixture_objects(
        &mut logic,
        &[FixtureObject::new("ObservationUnit")
            .kind_of("SELECTABLE")
            .health(100.0)],
    );
    let first = logic
        .create_object("ObservationUnit", Team::USA, Vec3::ZERO)
        .unwrap();
    let initial = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();

    let transient = logic
        .create_object("ObservationUnit", Team::USA, Vec3::X)
        .unwrap();
    logic.frame = 1;
    logic.complete_presentation_observations();
    let middle = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();

    logic.destroy_object(transient).unwrap();
    logic
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap();
    let survivor = logic
        .create_object("ObservationUnit", Team::USA, Vec3::Y)
        .unwrap();
    logic.frame = 2;
    logic.complete_presentation_observations();
    let final_frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();

    assert_eq!(initial.frame.0, 0);
    assert_eq!(initial.renderable_object_ids(), [first]);
    assert_eq!(middle.frame.0, 1);
    assert_eq!(middle.renderable_object_ids(), [first, transient]);
    assert_eq!(final_frame.frame.0, 2);
    assert_ne!(transient, survivor);
    assert_eq!(final_frame.renderable_object_ids(), [first, survivor]);
    assert_eq!(
        final_frame
            .objects
            .iter()
            .find(|row| row.id == survivor)
            .unwrap()
            .position,
        Vec3::Y
    );
}

#[test]
fn initial_skirmish_frame_publishes_its_player_roster_and_cash() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("InitialSkirmishFrame");
    logic
        .admit_object_ini_sources(std::iter::empty())
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert_eq!(frame.frame.0, logic.get_frame());
    let mut expected_players = logic.get_players().keys().copied().collect::<Vec<_>>();
    expected_players.sort_unstable();
    let mut published_players = frame.players.iter().map(|row| row.id).collect::<Vec<_>>();
    published_players.sort_unstable();
    assert_eq!(published_players, expected_players);
    assert_eq!(frame.objects.len(), logic.host_objects().registered_len());
    assert_eq!(
        frame.local_supplies,
        logic.get_player(0).unwrap().resources.supplies
    );
}

#[test]
fn frame_health_is_frozen_across_a_receiving_body_write() {
    let mut logic = GameLogic::new();
    admit_fixture_objects(
        &mut logic,
        &[FixtureObject::new("OverlayUnit")
            .kind_of("SELECTABLE")
            .health(100.0)],
    );
    let id = logic
        .create_object("OverlayUnit", Team::USA, glam::Vec3::new(4.0, 0.0, 0.0))
        .expect("u");
    let before = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let (object, events) = logic.host_object_and_health_events_mut(id).unwrap();
    assert!(!object.take_damage_from(40.0, None, before.frame.0, events));
    let after = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert_eq!(
        before
            .objects
            .iter()
            .find(|row| row.id == id)
            .unwrap()
            .health_current,
        100.0
    );
    assert_eq!(
        after
            .objects
            .iter()
            .find(|row| row.id == id)
            .unwrap()
            .health_current,
        60.0
    );
}

#[test]
fn owner_damage_and_heal_publish_current_health_without_rewriting_prior_frames() {
    let mut world = GameLogic::new();
    admit_fixture_objects(
        &mut world,
        &[FixtureObject::new("HealthPublication")
            .kind_of("SELECTABLE")
            .health(200.0)],
    );
    let id = world
        .create_object("HealthPublication", Team::USA, Vec3::ZERO)
        .unwrap();
    let full = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let before = full.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(before.health_current, 200.0);

    world.apply_host_damage(id, 35.0).unwrap();
    assert_eq!(world.host_object(id).unwrap().health.current, 165.0);
    let damaged = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let hurt = damaged.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(hurt.health_current, 165.0);

    let frame = world.get_frame();
    let (actor, events) = world.host_object_and_health_events_mut(id).unwrap();
    actor.heal(30.0, events, frame);
    assert_eq!(actor.health.current, 195.0);
    assert_eq!(
        world.health_events.drain_heal(),
        [HostHealEvent {
            target: id,
            health: 195.0
        }]
    );

    let healed = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let after = healed.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(after.health_current, 195.0);
    assert_eq!(before.health_current, 200.0);
    assert_eq!(hurt.health_current, 165.0);
}

fn player_world() -> (GameLogic, u32) {
    let mut world = GameLogic::new();
    world.admit_object_ini_sources(std::iter::empty()).unwrap();
    apply_skirmish_config(&mut world, &golden_skirmish_config("PlayerPublication")).unwrap();
    let local = world.local_player_id().unwrap();
    (world, local)
}

#[test]
fn player_frame_freezes_liveness_bounty_and_color_for_the_receiving_slot() {
    let (mut world, local) = player_world();
    let player = world.get_player_mut(local).unwrap();
    player.cash_bounty_percent = 0.2;
    player.color_rgb = (12, 34, 56);
    assert!(player.is_alive);
    let alive = PresentationFrame::try_build_from_logic(&mut world, local).unwrap();
    let player_row = alive.players.iter().find(|row| row.id == local).unwrap();
    assert!(player_row.is_alive);
    assert_eq!(player_row.color_rgb, (12, 34, 56));
    assert_eq!(alive.local_cash_bounty_percent, 0.2);
    // C++ Player.cpp starts ReplayObserver dead.
    assert!(
        alive
            .players
            .iter()
            .any(|row| row.name == "ReplayObserver" && !row.is_alive)
    );

    world.get_player_mut(local).unwrap().is_alive = false;
    let defeated = PresentationFrame::try_build_from_logic(&mut world, local).unwrap();
    assert_eq!(
        defeated.players.iter().filter(|row| row.is_alive).count() + 1,
        alive.players.iter().filter(|row| row.is_alive).count()
    );
    assert!(
        !defeated
            .players
            .iter()
            .find(|row| row.id == local)
            .unwrap()
            .is_alive
    );
    assert!(player_row.is_alive);
}

#[test]
fn disabled_radar_keeps_its_published_provider_count() {
    let (mut world, local) = player_world();
    let player = world.get_player_mut(local).unwrap();
    player.radar_count = 2;
    player.radar_disabled = false;
    let enabled = PresentationFrame::try_build_from_logic(&mut world, local).unwrap();
    assert_eq!(enabled.local_radar_count, 2);
    assert!(enabled.local_radar_active());

    world.get_player_mut(local).unwrap().radar_disabled = true;
    let disabled = PresentationFrame::try_build_from_logic(&mut world, local).unwrap();
    assert_eq!(disabled.local_radar_count, 2);
    assert!(disabled.local_radar_disabled);
    assert!(!disabled.local_radar_active());
    assert!(enabled.local_radar_active());
}

#[test]
fn player_frame_publishes_power_produced_consumed_and_available() {
    let (mut world, local) = player_world();
    let player = world.get_player_mut(local).unwrap();
    player.power_produced = 120;
    player.power_consumed = 45;
    player.power_available = 75;
    let frame = PresentationFrame::try_build_from_logic(&mut world, local).unwrap();
    assert_eq!(
        (
            frame.local_power_produced,
            frame.local_power_consumed,
            frame.local_power
        ),
        (120, 45, 75)
    );
}

#[test]
fn player_frame_publishes_receiving_science_names() {
    let (mut world, local) = player_world();
    let baseline = PresentationFrame::try_build_from_logic(&mut world, local).unwrap();
    assert!(!baseline.local_has_science("SCIENCE_PaladinTank"));
    assert!(!baseline.local_has_science("SCIENCE_Pathfinder"));
    let player = world.get_player_mut(local).unwrap();
    player
        .unlocked_sciences
        .insert("SCIENCE_PaladinTank".into());
    player.unlocked_sciences.insert("SCIENCE_Pathfinder".into());
    let frame = PresentationFrame::try_build_from_logic(&mut world, local).unwrap();
    assert!(frame.local_has_science("SCIENCE_PaladinTank"));
    assert!(frame.local_has_science("SCIENCE_Pathfinder"));
    assert!(!frame.local_has_science("SCIENCE_UnknownFixture"));
    assert!(!baseline.local_has_science("SCIENCE_PaladinTank"));
}
