//! Executable command and lifecycle regressions.

#[test]
fn receiving_frame_refreshes_on_capture_and_keeps_the_previous_selection() {
    use crate::game_logic::Team;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ GameEngine.cpp loads PlayerTemplate and Object INI before NewGame.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("StampU190")
            .kind_of("SELECTABLE")
            .health(150.0),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("PresOverlayStamp190"),
    );
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let position = Vec3::new(9.0, 0.0, 9.0);
    let id = logic
        .create_object("StampU190", Team::USA, position)
        .unwrap();

    let before = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let object = logic.host_object_mut(id).unwrap();
    object.health.current = 55.0;
    object.selected = true;

    let after = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let a = before.objects.iter().find(|row| row.id == id).unwrap();
    let b = after.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(before.frame, after.frame);
    assert_eq!((a.health_current, a.selected), (150.0, false));
    assert_eq!((b.health_current, b.selected), (55.0, true));
}
