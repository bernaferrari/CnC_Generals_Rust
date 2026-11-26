//! Executable command and lifecycle regressions.

#[test]
fn guard_command_publishes_the_receiving_units_guard_position() {
    use crate::command_system::{CommandType, GameCommand, GuardTarget, ModifierKeys};
    use crate::game_logic::{GuardMode, Team};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;
    use std::time::SystemTime;

    // C++ loads Object INI before NewGame; a Guard order reaches the Object's
    // AIUpdateInterface (GameLogicDispatch.cpp), so the unit authors one.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("GuardLogUnit198")
            .kind_of("VEHICLE SELECTABLE")
            .health(140.0)
            .walker(),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("GuardLog198"),
    );
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let unit = logic
        .create_object("GuardLogUnit198", Team::USA, Vec3::new(12.0, 0.0, 13.0))
        .unwrap();

    let dest = Vec3::new(40.0, 0.0, 41.0);
    logic.queue_command(GameCommand {
        command_type: CommandType::Guard {
            target: GuardTarget::Position(dest),
            mode: GuardMode::Normal,
        },
        player_id: 0,
        command_id: 2,
        timestamp: SystemTime::now(),
        selected_units: vec![unit],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();

    let owner = logic.host_object(unit).unwrap();
    assert_eq!(owner.active_guard_position(), Some(dest));
    assert_eq!(owner.active_guard_target(), None);
    assert!(owner.guard_radius > 0.0);

    let presentation = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let row = presentation
        .objects
        .iter()
        .find(|row| row.id == unit)
        .unwrap();
    assert_eq!(row.guard_position, Some(dest));
    assert_eq!(row.guard_target, None);
}
