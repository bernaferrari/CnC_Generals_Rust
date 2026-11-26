//! Executable command and lifecycle regressions.

#[test]
fn receiving_frame_updates_body_and_authoritative_transform_together() {
    use crate::game_logic::Team;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ GameEngine.cpp loads PlayerTemplate and Object INI before NewGame.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("OverlayU187")
            .kind_of("SELECTABLE")
            .health(200.0),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("PresGwOverlay187"),
    );
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let position = Vec3::new(10.0, 0.0, 10.0);
    let id = logic
        .create_object("OverlayU187", Team::USA, position)
        .unwrap();

    let before = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let (object, partition) = logic.host_object_and_partition_mut(id).unwrap();
    object.health.current = 77.0;
    let destination = Vec3::new(42.0, 0.0, 43.0);
    object.set_position_with_partition_owner(destination, 0, &mut Default::default(), partition);
    object.set_orientation_with_partition_owner(1.25, partition);

    let after = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let a = before.objects.iter().find(|row| row.id == id).unwrap();
    let b = after.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(a.health_current, 200.0);
    assert_eq!(a.position, position);
    assert_eq!(b.health_current, 77.0);
    assert_eq!(b.position, destination);
    assert!((b.orientation - 1.25).abs() < 1e-4);
}
