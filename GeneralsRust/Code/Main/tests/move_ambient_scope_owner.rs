use generals_main::game_logic::{GameLogic, GameMode, KindOf, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn starting_a_move_does_not_retry_another_objects_ambient_sound() {
    // A process-wide audio manager exists in the real application. A move
    // must not consult it to restart unrelated Drawable ambient loops.
    game_engine::common::audio::game_audio::initialize_global_audio_manager();

    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    let mut template = ThingTemplate::new("OwnedMoveProbe");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert("OwnedMoveProbe".into(), template);

    let ambient = world
        .create_object("OwnedMoveProbe", Team::USA, Vec3::ZERO)
        .expect("ambient object");
    let moving = world
        .create_object("OwnedMoveProbe", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("moving object");
    let other = world
        .host_object_mut(ambient)
        .expect("ambient object remains");
    other.ambient_audio = Some("MissingOwnedAmbientLoop".into());
    other.ambient_sound_enabled_from_script = true;
    let before = world.queued_audio_event_count_for_test();

    let destination = Vec3::new(20.0, 0.0, 0.0);
    assert!(world.assign_unit_path_exact(moving, destination, &[destination]));
    assert_eq!(
        world.queued_audio_event_count_for_test(),
        before,
        "C++ startMoveSound does not retry a different Drawable's ambient loop"
    );
}
