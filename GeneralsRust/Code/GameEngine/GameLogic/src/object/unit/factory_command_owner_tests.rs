//! Real factory command entry must use its admitted Object, with no Unit mirror.
use super::*;
use crate::ai::states::AIStateType;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};

#[test]
fn factory_busy_enters_actual_machine_and_clears_owned_path_timer() {
    if !child(concat!(
        module_path!(),
        "::factory_busy_enters_actual_machine_and_clears_owned_path_timer"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = FactoryRuntime::new();
    let mut ai = actual.ai.lock().unwrap();
    ai.set_queue_for_path_time(123);
    // CPP AIUpdate.cpp4002-4007: clear, set source, enter AI_BUSY. The
    // machine-change callback clears the owned path timer synchronously.
    ai.execute_command(&AiCommandParams::new(
        AiCommandType::Busy,
        CommandSourceType::FromPlayer,
    ))
    .unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Busy as u32));
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromPlayer);
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Busy));
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
    let mut bytes = Cursor::new(Vec::new());
    assert!(
        ai.xfer_ai_update_state(&mut XferSave::new(&mut bytes, 1))
            .unwrap()
    );
    drop(ai);
    // Decode through the same production runtime topology; this only reads
    // the captured wire. It neither installs a substitute nor advances AI.
    let loaded = crate::object::object_factory::factory_ai::prepare_unit_ai(
        &actual.owner,
        &crate::common::DefaultThingTemplate::new("WireReader".into()),
        actual.id,
    );
    let mut loaded = loaded.lock().unwrap();
    assert!(
        loaded
            .xfer_ai_update_state(&mut XferLoad::new(Cursor::new(bytes.into_inner()), 1))
            .unwrap()
    );
    assert_eq!(loaded.data.queue_for_path_frame, 0);
}

#[test]
fn factory_command_ignores_locked_foreign_same_id_unit() {
    if !child(concat!(
        module_path!(),
        "::factory_command_ignores_locked_foreign_same_id_unit"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    let foreign_owner = Arc::new(RwLock::new(Object::new_test(actual.id, 200.0)));
    let foreign = Arc::new(RwLock::new(
        super::super::Unit::new(
            foreign_owner.clone(),
            &crate::common::DefaultThingTemplate::new("ForeignCommandOwner".into()),
        )
        .unwrap(),
    ));
    super::super::register_unit(actual.id, &foreign);
    let _held_foreign = foreign.write().unwrap();
    let mut ai = actual.ai.lock().unwrap();
    ai.execute_command(&AiCommandParams::new(
        AiCommandType::Busy,
        CommandSourceType::FromPlayer,
    ))
    .unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Busy as u32));
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromPlayer);
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Busy));
    assert!(Arc::ptr_eq(
        &foreign_owner,
        &crate::object::registry::OBJECT_REGISTRY
            .get_object(actual.id)
            .unwrap(),
    ));
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
fn factory_attack_sets_live_victim_position_and_weapon_limit_before_update() {
    if !child(concat!(
        module_path!(),
        "::factory_attack_sets_live_victim_position_and_weapon_limit_before_update"
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
        let mut template = crate::weapon::WeaponTemplate::new("FactoryCommandWeapon".into());
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
    command.int_value = 3;
    let mut ai = actual.ai.lock().unwrap();
    ai.execute_command(&command).unwrap();
    // CPP AIStates.cpp5516 and AIUpdate.cpp3418-3421: enter records the victim
    // directly on AI; command's limit applies after onEnter resets the weapon.
    assert_eq!(ai.get_original_victim_pos(), Some(target_pos));
    assert_eq!(ai.get_goal_object_id(), target_id);
    assert_eq!(
        ai.get_current_state_id(),
        Some(AIStateType::AttackObject as u32)
    );
    assert_eq!(
        actual
            .owner
            .read()
            .unwrap()
            .get_current_weapon()
            .unwrap()
            .0
            .max_shot_count,
        3
    );
    assert!(
        actual
            .owner
            .read()
            .unwrap()
            .ai_pending_original_victim_pos
            .is_none()
    );
}

#[test]
fn stationary_factory_move_to_object_does_not_enter_movement_state() {
    if !child(concat!(
        module_path!(),
        "::stationary_factory_move_to_object_does_not_enter_movement_state"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    let mut command =
        AiCommandParams::new(AiCommandType::MoveToObject, CommandSourceType::FromPlayer);
    command.obj = Some(actual.id + 1000);
    let mut ai = actual.ai.lock().unwrap();
    // No authored locomotor: CPP privateMoveToObject returns before clearing.
    let mut has_active = false;
    ai.with_cur_locomotor(&mut |_| has_active = true);
    assert!(!has_active);
    ai.execute_command(&command).unwrap();
    assert_eq!(ai.get_current_state_id(), None);
    assert_eq!(ai.get_goal_object_id(), crate::common::INVALID_ID);
}
