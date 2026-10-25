//! Executable command and lifecycle regressions.

fn command_attack_log() -> bool {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;
    use std::time::SystemTime;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AtkLog197");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    for name in ["AtkLogUnit197", "AtkLogTarget197"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.set_health(120.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Vehicle);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let Some(attacker) =
        logic.create_object("AtkLogUnit197", Team::USA, Vec3::new(10.0, 0.0, 10.0))
    else {
        return false;
    };
    let Some(target) =
        logic.create_object("AtkLogTarget197", Team::China, Vec3::new(30.0, 0.0, 30.0))
    else {
        return false;
    };

    logic.queue_command(GameCommand {
        command_type: CommandType::AttackObject { target_id: target },
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![attacker],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();

    // The command has already committed its target to the canonical Main object.
    if logic.host_object(attacker).and_then(|object| object.target) != Some(target) {
        return false;
    }
    let mut observer = crate::gameworld_shadow::GameWorldShadow::new(64);
    observer.sync_from_host(&logic);
    let Some(entity) = observer.entity_for_host(attacker) else {
        return false;
    };
    let Some(victim) = observer.entity_for_host(target) else {
        return false;
    };
    if observer
        .world()
        .entity(entity)
        .and_then(|object| object.attack_target)
        != Some(victim)
    {
        return false;
    }

    true
}

#[test]
fn command_attack_log_runs() {
    assert!(command_attack_log());
}
