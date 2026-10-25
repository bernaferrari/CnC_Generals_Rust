//! Executable command and lifecycle regressions.

fn command_rally_log() -> bool {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;
    use std::time::SystemTime;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RallyLog200");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("RallyFactory200") {
        let mut t = ThingTemplate::new("RallyFactory200");
        t.set_health(600.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSWarFactory);
        t.add_kind_of(KindOf::AutoRallypoint);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RallyFactory200".into(), t);
    }
    let Some(factory) =
        logic.create_object("RallyFactory200", Team::USA, Vec3::new(15.0, 0.0, 16.0))
    else {
        return false;
    };
    // Ensure building_data present for rally.
    if let Some(obj) = logic.host_object_mut(factory) {
        if obj.building_data.is_none() {
            obj.building_data = Some(crate::game_logic::buildings::BuildingData::new(
                crate::game_logic::buildings::BuildingType::WarFactory,
            ));
        }
    }

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
