//! Executable command and lifecycle regressions.

#[test]
fn receiving_frame_freezes_movement_color_selection_and_body_state() {
    use crate::game_logic::Team;
    use generals_sim::object::BodyDamageType;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ GameEngine.cpp loads PlayerTemplate and Object INI before NewGame.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("OverlayDeep189")
            .kind_of("SELECTABLE VEHICLE")
            .health(200.0),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("PresOverlayDeep189"),
    );
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let position = Vec3::new(11.0, 0.0, 12.0);
    let id = logic
        .create_object("OverlayDeep189", Team::USA, position)
        .unwrap();

    let owner = logic.host_object(id).unwrap().owner_player_id.unwrap();
    let player = logic.get_player_mut(owner).unwrap();
    player.color_rgb = (25, 51, 76);
    player.color_night_rgb = player.color_rgb;
    let object = logic.host_object_mut(id).unwrap();
    object.movement.velocity = Vec3::new(3.5, 0.0, -2.25);
    object.movement.max_speed = 18.5;
    object.selected = true;
    object.body_damage_state = BodyDamageType::ReallyDamaged;
    object.health.current = 88.0;

    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let row = frame.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(row.velocity, Vec3::new(3.5, 0.0, -2.25));
    assert_eq!(row.move_max_speed, 18.5);
    assert!(row.selected);
    assert_eq!(
        row.team_color,
        [25.0 / 255.0, 51.0 / 255.0, 76.0 / 255.0, 1.0]
    );
    assert_eq!(
        row.body_damage_state,
        BodyDamageType::ReallyDamaged.ordinal()
    );
    assert_eq!(row.health_current, 88.0);
}
