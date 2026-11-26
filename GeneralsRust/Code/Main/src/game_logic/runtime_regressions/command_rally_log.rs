//! Executable command and lifecycle regressions.

#[test]
fn rally_command_commits_and_publishes_factory_exit_point() {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::Team;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;
    use std::time::SystemTime;

    // C++ loads Object INI before NewGame: the factory authors a body, a
    // ProductionUpdate and the ExitInterface that owns the rally point.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("RallyFactory200")
            .kind_of("STRUCTURE FS_WARFACTORY AUTO_RALLYPOINT SELECTABLE")
            .module(
                "Body = StructureBody FixtureBody\n MaxHealth = 600.0\n InitialHealth = 600.0\nEnd",
            )
            .module("Behavior = ProductionUpdate FixtureProduction\nEnd")
            .module(
                "Behavior = DefaultProductionExitUpdate FixtureExit\n UnitCreatePoint = X: 0.0 Y: 0.0 Z: 0.0\n NaturalRallyPoint = X: 55.0 Y: 0.0 Z: 0.0\nEnd",
            ),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("RallyLog200"),
    );
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let factory = logic
        .create_object("RallyFactory200", Team::USA, Vec3::new(15.0, 0.0, 16.0))
        .unwrap();
    let dest = Vec3::new(80.0, 0.0, 81.0);
    logic.queue_command(GameCommand {
        command_type: CommandType::SetRallyPoint { location: dest },
        player_id: 0,
        command_id: 3,
        timestamp: SystemTime::now(),
        selected_units: vec![factory],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();

    let owner = logic.host_object(factory).unwrap();
    assert_eq!(
        owner.building_data.as_ref().unwrap().rally_point,
        Some(dest)
    );
    let presentation = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let row = presentation
        .objects
        .iter()
        .find(|row| row.id == factory)
        .unwrap();
    assert_eq!(row.rally_point, Some(dest));
}
