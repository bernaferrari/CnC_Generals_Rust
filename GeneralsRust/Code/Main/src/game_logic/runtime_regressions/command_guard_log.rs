//! Executable command and lifecycle regressions.

fn command_guard_log() -> bool {
    use crate::command_system::{CommandType, GameCommand, GuardTarget, ModifierKeys};
    use crate::game_logic::{GameLogic, GuardMode, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;
    use std::time::SystemTime;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GuardLog198");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("GuardLogUnit198") {
        let mut t = ThingTemplate::new("GuardLogUnit198");
        t.set_health(140.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("GuardLogUnit198".into(), t);
    }
    let Some(unit) = logic.create_object("GuardLogUnit198", Team::USA, Vec3::new(12.0, 0.0, 13.0))
    else {
        return false;
    };

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

    let Some(owner) = logic.host_object(unit) else {
        return false;
    };
    if owner.guard_position != Some(dest)
        || owner.guard_target.is_some()
        || owner.guard_radius <= 0.0
    {
        return false;
    }
    let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(entity) = shadow.entity_for_host(unit) else {
        return false;
    };
    #[cfg(test)]
    {
        let copy = shadow.world_mut().world_mut().entity_mut(entity).unwrap();
        copy.guard_position = Some([900.0, 0.0, 900.0]);
        copy.guard_radius = 1.0;
    }
    if logic.host_object(unit).unwrap().guard_position != Some(dest) {
        return false;
    }
    shadow.sync_from_host(&logic);
    if shadow.world().entity(entity).unwrap().guard_position != Some([dest.x, dest.y, dest.z]) {
        return false;
    }

    true
}

#[test]
fn command_guard_log_runs() {
    assert!(command_guard_log());
}
