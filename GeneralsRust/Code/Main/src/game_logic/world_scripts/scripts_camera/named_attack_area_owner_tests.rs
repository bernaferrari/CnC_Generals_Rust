//! CPP ScriptActions1336–1359 validates the polygon before leave/Normal/AttackArea.
use super::named_command_test_support::{execute, world};
use super::*;
use gamelogic::scripting::core::ScriptActionType;

fn polygon(owner: &GameLogic, min: i32, max: i32) {
    owner
        .host_trigger_world
        .lock()
        .unwrap()
        .set_trigger_areas(&[gamelogic::polygon_trigger::PolygonTrigger::new(
            8470,
            "OwnedAttackArea".into(),
            vec![
                gamelogic::common::ICoord3D::new(min, min, 0),
                gamelogic::common::ICoord3D::new(max, min, 0),
                gamelogic::common::ICoord3D::new(max, max, 0),
                gamelogic::common::ICoord3D::new(min, max, 0),
            ],
        )]);
}

#[test]
fn named_attack_area_uses_the_driving_polygon_and_persistent_target_selection() {
    let (mut owner, unit, target) = world();
    let (foreign, peer, _) = world();
    assert_eq!(unit, peer);
    polygon(&owner, 60, 90);
    polygon(&foreign, 500, 600);
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::NamedAttackArea,
                "NamedUnit",
                "MissingArea"
            )]
        ),
        1
    );
    assert_eq!(owner.host_object(unit).unwrap().formation_id, 19);
    assert_eq!(
        owner
            .host_object(unit)
            .unwrap()
            .jet_ai
            .cur_locomotor_set
            .as_deref(),
        Some("SET_PANIC")
    );
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::NamedAttackArea,
                "NamedUnit",
                "OwnedAttackArea"
            )]
        ),
        1
    );
    assert_eq!(owner.host_object(unit).unwrap().formation_id, 19);
    assert_eq!(
        owner
            .host_object(unit)
            .unwrap()
            .jet_ai
            .cur_locomotor_set
            .as_deref(),
        Some("SET_NORMAL")
    );
    assert_eq!(owner.host_object(unit).unwrap().target, Some(target));
    assert_eq!(
        owner.host_object(unit).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    let next = owner
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(85.0, 0.0, 85.0))
        .unwrap();
    owner.host_object_mut(target).unwrap().health.current = 0.0;
    owner.tick_attack_area_persist(&[unit]);
    assert_eq!(owner.host_object(unit).unwrap().target, Some(next));
    assert_eq!(
        owner.host_object(unit).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(foreign.host_object(peer).unwrap().formation_id, 19);
    assert_eq!(foreign.host_object(peer).unwrap().target, None);
    assert_eq!(
        foreign
            .host_object(peer)
            .unwrap()
            .jet_ai
            .cur_locomotor_set
            .as_deref(),
        Some("SET_PANIC")
    );
}

#[test]
fn named_attack_area_move_without_a_victim_retains_script_source() {
    let (mut owner, unit, _) = world();
    polygon(&owner, 200, 300);
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::NamedAttackArea,
                "NamedUnit",
                "OwnedAttackArea"
            )]
        ),
        1
    );
    let object = owner.host_object(unit).unwrap();
    assert_eq!(object.target, None);
    assert_eq!(
        object.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(object.formation_id, 19);
    assert_eq!(
        object.jet_ai.cur_locomotor_set.as_deref(),
        Some("SET_NORMAL")
    );
}

#[test]
fn player_attack_area_resolves_owned_polygon_and_retains_player_source_on_rescan() {
    let (mut owner, unit, target) = world();
    let (foreign, peer, _) = world();
    polygon(&owner, 60, 90);
    polygon(&foreign, 500, 600);
    {
        let mut executor = crate::command_executor::CommandExecutor::new(&mut owner, 1);
        assert_eq!(
            executor
                .execute_attack_area(&[unit], Vec3::ZERO, 1.0, Some("OwnedAttackArea"))
                .expect("valid owned attack-area fixture"),
            crate::command_system::CommandResult::Success
        );
    }
    assert_eq!(owner.host_object(unit).unwrap().target, Some(target));
    assert_eq!(
        owner.host_object(unit).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_PLAYER
    );
    let next = owner
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(85.0, 0.0, 85.0))
        .unwrap();
    owner.host_object_mut(target).unwrap().health.current = 0.0;
    owner.tick_attack_area_persist(&[unit]);
    assert_eq!(owner.host_object(unit).unwrap().target, Some(next));
    assert_eq!(
        owner.host_object(unit).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_PLAYER
    );
    assert_eq!(foreign.host_object(peer).unwrap().target, None);
}
