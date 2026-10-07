//! AIUpdate.cpp:3111-3116 forwards the explicit busy-state classifier.
use super::*;
use crate::ai::states::AIStateType;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};

#[test]
fn factory_attack_with_live_victim_is_not_busy() {
    if !child(concat!(
        module_path!(),
        "::factory_attack_with_live_victim_is_not_busy"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    let target_id = actual.id + 1000;
    let target = Arc::new(RwLock::new(Object::new_test(target_id, 100.0)));
    let target_pos = Coord3D::new(50.0, 60.0, 0.0);
    target.write().unwrap().set_position(&target_pos).unwrap();
    crate::ai::object_registry::register_legacy_object(&target);
    crate::object::registry::OBJECT_REGISTRY.register_object(target_id, &target);
    {
        let mut owner = actual.owner.write().unwrap();
        let mut template = crate::weapon::WeaponTemplate::new("FactoryBusyClassifierWeapon".into());
        template.attack_range = 100.0;
        template.primary_damage = 10.0;
        let mut weapons = crate::weapon::WeaponTemplateSet::new();
        weapons.set_weapon_template(crate::weapon::WeaponSlotType::Primary, Arc::new(template));
        owner.weapon_set.add_weapon_template_set(weapons);
        owner.refresh_weapon_set().unwrap();
        owner.reload_all_ammo(true).unwrap();
    }
    let mut command =
        AiCommandParams::new(AiCommandType::AttackObject, CommandSourceType::FromPlayer);
    command.obj = Some(target_id);
    let mut ai = actual.ai.lock().unwrap();
    ai.execute_command(&command).unwrap();
    // Positive controls: the actual installed native machine entered attack
    // and recorded a real victim. Neither a default Idle nor failed entry can
    // satisfy the classification proof.
    assert_eq!(
        ai.get_current_state_id(),
        Some(AIStateType::AttackObject as u32)
    );
    assert_eq!(ai.get_goal_object_id(), target_id);
    assert_eq!(ai.get_original_victim_pos(), Some(target_pos));
    assert!(!ai.is_busy());
    assert!(super::super::registry::get_unit_arc(actual.id).is_none());
    assert!(Arc::ptr_eq(
        &actual.ai,
        &actual
            .owner
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap()
    ));
}

#[test]
fn factory_busy_and_idle_keep_cpp_classification() {
    if !child(concat!(
        module_path!(),
        "::factory_busy_and_idle_keep_cpp_classification"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    let mut ai = actual.ai.lock().unwrap();
    ai.execute_command(&AiCommandParams::new(
        AiCommandType::Busy,
        CommandSourceType::FromPlayer,
    ))
    .unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Busy as u32));
    assert!(ai.is_busy());
    ai.execute_command(&AiCommandParams::new(
        AiCommandType::Idle,
        CommandSourceType::FromPlayer,
    ))
    .unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Idle as u32));
    assert!(!ai.is_busy());
    assert!(ai.is_idle());
    assert!(super::super::registry::get_unit_arc(actual.id).is_none());
    assert!(Arc::ptr_eq(
        &actual.ai,
        &actual
            .owner
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap()
    ));
}
