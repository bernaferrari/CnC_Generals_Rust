//! Executable command and lifecycle regressions.

#[test]
fn formation_members_publish_their_own_offsets() {
    use crate::game_logic::Team;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::{Vec2, Vec3};
    // C++ loads Object INI before NewGame; units are authored content.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("FormUnit204")
            .kind_of("INFANTRY SELECTABLE")
            .health(100.0)
            .walker(),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("Form204"),
    );
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let a = logic
        .create_object("FormUnit204", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("FormUnit204", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();

    let members = [(a, Vec2::new(1.0, -2.0)), (b, Vec2::new(-1.0, 2.0))];
    for (id, offset) in members {
        logic.host_object_mut(id).unwrap().set_formation(7, offset);
    }

    let presentation = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    for (id, offset) in members {
        let owner = logic.host_object(id).unwrap();
        assert_eq!((owner.formation_id, owner.formation_offset), (7, offset));
        let row = presentation
            .objects
            .iter()
            .find(|row| row.id == id)
            .unwrap();
        assert_eq!((row.formation_id, row.formation_offset), (7, offset));
    }
}
