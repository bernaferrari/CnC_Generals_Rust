//! Executable command and lifecycle regressions.

fn command_rally_log() -> bool {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::Team;
    use crate::gameworld_shadow::GameWorldShadow;
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
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    let Some(factory) =
        logic.create_object("RallyFactory200", Team::USA, Vec3::new(15.0, 0.0, 16.0))
    else {
        return false;
    };
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

    if logic.host_object(factory).and_then(|object| {
        object
            .building_data
            .as_ref()
            .and_then(|building| building.rally_point)
    }) != Some(dest)
    {
        return false;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Entity should hold rally after apply.
    let Some(eid) = shadow.entity_for_host(factory) else {
        return false;
    };
    let Some(ent) = shadow.world().entity(eid) else {
        return false;
    };
    ent.rally_point
        .map(|p| (p[0] - 80.0).abs() < 1e-3 && (p[2] - 81.0).abs() < 1e-3)
        .unwrap_or(false)
}

#[test]
fn command_rally_log_runs() {
    assert!(command_rally_log());
}
