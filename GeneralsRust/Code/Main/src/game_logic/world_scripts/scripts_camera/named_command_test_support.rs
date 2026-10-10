//! Owned fixtures and actual script walks shared by named command regressions.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

pub(super) fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.world_min = Vec3::ZERO;
    world.world_max = Vec3::splat(1000.0);
    world.add_player(Player::new(1, Team::USA, "NamedOwner", true));
    world.add_player(Player::new(2, Team::China, "NamedVictim", false));
    let mut template = ThingTemplate::new("OwnedNamedInfantry");
    template
        .add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_primary_weapon(Weapon::default());
    use crate::game_logic::host_upgrade_module_residuals::{
        AuthoredLocomotorSet, HostLocomotorSetKind,
    };
    template.set_authored_ai_update_interface(Some(true));
    template.authored_locomotor_sets = Some(vec![
        AuthoredLocomotorSet {
            kind: HostLocomotorSetKind::Normal,
            members: vec!["BasicHumanLocomotor".into()],
        },
        AuthoredLocomotorSet {
            kind: HostLocomotorSetKind::Panic,
            members: vec!["RedguardLocomotor".into()],
        },
    ]);
    world.templates.insert(template.name.clone(), template);
    let unit = world
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let target = world
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(70.0, 0.0, 70.0))
        .unwrap();
    world.host_object_mut(unit).unwrap().name = "NamedUnit".into();
    world.host_object_mut(target).unwrap().name = "NamedTarget".into();
    let object = world.host_object_mut(unit).unwrap();
    object.set_formation(19, glam::Vec2::new(4.0, 6.0));
    object.max_shots_to_fire = 3;
    assert!(world.apply_unit_locomotor_set(unit, "panic"));
    assert_eq!(
        world
            .host_object(unit)
            .unwrap()
            .cur_locomotor_name
            .as_deref(),
        Some("RedguardLocomotor")
    );
    assert_eq!(
        world
            .host_object(unit)
            .unwrap()
            .jet_ai
            .cur_locomotor_set
            .as_deref(),
        Some("SET_PANIC")
    );
    (world, unit, target)
}

pub(super) fn execute(world: &mut GameLogic, actions: &[(ScriptActionType, &str, &str)]) -> i32 {
    let mut next = None;
    for (kind, unit, target) in actions.iter().rev() {
        let mut action = ScriptAction::new(*kind);
        for name in [unit, target] {
            action
                .add_parameter(Parameter::with_string(ParameterType::Unit, (*name).into()))
                .unwrap();
        }
        action.next_action = next;
        next = Some(Box::new(action));
    }
    // A real counter after the commands calibrates the production action walk.
    let mut counter = ScriptAction::new(ScriptActionType::IncrementCounter);
    counter
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    counter
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "NamedObserved".into(),
        ))
        .unwrap();
    if let Some(head) = next.as_mut() {
        let mut tail = head.as_mut();
        while let Some(ref mut following) = tail.next_action {
            tail = following.as_mut();
        }
        tail.next_action = Some(Box::new(counter));
    } else {
        next = Some(Box::new(counter));
    }
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OwnedNamedCommand".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = next;
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("NamedObserved", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::new(world),
        )
        .unwrap();
    engine.get_counter("NamedObserved").unwrap().value
}
