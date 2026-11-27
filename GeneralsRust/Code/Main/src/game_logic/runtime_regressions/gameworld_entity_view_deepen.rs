//! Executable command and lifecycle regressions.

#[test]
fn frame_keeps_receiving_actor_max_health_and_motion() {
    use crate::game_logic::Team;
    use generals_sim::object::BodyDamageType;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ loads Object INI before NewGame; units are authored content.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("ViewDeep191")
            .kind_of("VEHICLE SELECTABLE")
            .health(180.0)
            .walker(),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("GwEntityView191"),
    );
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let id = logic
        .create_object("ViewDeep191", Team::USA, Vec3::new(14.0, 0.0, 15.0))
        .unwrap();
    let owner = logic.host_object(id).unwrap().owner_player_id.unwrap();
    let player = logic.get_player_mut(owner).unwrap();
    player.color_rgb = (51, 102, 153);
    player.color_night_rgb = player.color_rgb;
    let actor = logic.host_object_mut(id).unwrap();
    actor.movement.velocity = Vec3::new(1.5, 0.0, 2.5);
    actor.movement.max_speed = 22.0;
    actor.selected = true;
    actor.body_damage_state = BodyDamageType::Damaged;
    actor.health.current = 99.0;
    actor.health.maximum = 180.0;
    actor.max_health = 180.0;

    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let row = frame.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(row.velocity, Vec3::new(1.5, 0.0, 2.5));
    assert_eq!(row.move_max_speed, 22.0);
    assert!(row.selected);
    assert_eq!(row.team_color, [0.2, 0.4, 0.6, 1.0]);
    assert_eq!(row.body_damage_state, BodyDamageType::Damaged.ordinal());
    assert_eq!((row.health_current, row.health_max), (99.0, 180.0));
}
