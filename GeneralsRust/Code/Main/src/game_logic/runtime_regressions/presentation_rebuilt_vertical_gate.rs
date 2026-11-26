//! Executable command and lifecycle regressions.

#[test]
fn newly_admitted_actor_reaches_native_presentation_at_its_world_pose() {
    use crate::game_logic::Team;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ GameEngine.cpp loads PlayerTemplate and Object INI before NewGame.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("RebuiltVert196")
            .kind_of("SELECTABLE VEHICLE")
            .health(100.0),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("RebuiltVert196"),
    );
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let before = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let position = Vec3::new(5.0, 0.0, 6.0);
    let id = logic
        .create_object("RebuiltVert196", Team::USA, position)
        .unwrap();
    let published =
        PresentationFrame::try_publish(&mut logic, 0, None, true)
            .unwrap();
    assert!(before.objects.iter().all(|row| row.id != id));
    let row = published.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(row.position, position);
    assert_eq!(row.health_current, 100.0);
    assert!(
        published
            .direct_host_drawables
            .iter()
            .any(|draw| draw.object.id == id && draw.resident)
    );
}
