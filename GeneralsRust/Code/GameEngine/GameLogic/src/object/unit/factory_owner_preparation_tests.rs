//! The factory already owns the Object; a same-ID legacy Unit cannot select it.
use super::*;
use crate::ai::CommandSourceType;
use crate::common::Coord3D;
use crate::common::{DefaultThingTemplate, TemplateModuleInfo};
use crate::modules::AIAttitudeType;
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use game_engine::common::thing::module::ModuleInterfaceType;
use std::sync::{Arc, RwLock};

struct PreparedTemplate {
    inner: DefaultThingTemplate,
    modules: Vec<TemplateModuleInfo>,
}
impl std::fmt::Debug for PreparedTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedTemplate").finish_non_exhaustive()
    }
}
impl crate::common::ThingTemplate for PreparedTemplate {
    fn is_kind_of(&self, kind: crate::common::KindOf) -> bool {
        self.inner.is_kind_of(kind)
    }
    fn get_name(&self) -> &crate::common::AsciiString {
        self.inner.get_name()
    }
    fn get_template_geometry_info(&self) -> crate::common::GeometryInfo {
        self.inner.get_template_geometry_info()
    }
    fn calc_vision_range(&self) -> f32 {
        self.inner.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> f32 {
        self.inner.calc_shroud_clearing_range()
    }
    fn get_behavior_module_info(&self) -> &[TemplateModuleInfo] {
        &self.modules
    }
}
fn template() -> PreparedTemplate {
    let mut data = AIUpdateModuleData::default();
    data.set_auto_acquire_enemies_when_idle(crate::object::update::AUTO_ACQUIRE_IDLE);
    PreparedTemplate {
        inner: DefaultThingTemplate::new("PreparedAiOwner".into()),
        modules: vec![TemplateModuleInfo {
            name: "AIUpdateInterface".into(),
            module_tag: "PreparedAi".into(),
            data: Arc::new(data),
            interface_mask: ModuleInterfaceType::UPDATE,
        }],
    }
}
struct Unregister(u32);
impl Drop for Unregister {
    fn drop(&mut self) {
        super::unregister_unit(self.0);
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.0);
        crate::ai::object_registry::unregister_legacy_object(self.0);
    }
}
fn prepare(owner: &Arc<RwLock<crate::object::Object>>) -> Arc<std::sync::Mutex<UnitAIUpdate>> {
    let id = owner.read().unwrap().get_id();
    crate::object::object_factory::factory_ai::prepare_unit_ai(owner, &template(), id)
}
#[test]
fn actual_factory_preparation_binds_exact_owner_without_legacy_unit_admission() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_23;
    assert!(super::registry::get_unit_arc(id).is_none());
    let first = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let second = Arc::new(RwLock::new(crate::object::Object::new_test(id, 200.0)));
    let rng = game_engine::common::random_value::get_game_logic_random_seed_state();
    let foreign = crate::system::game_logic::get_game_logic().lock().unwrap();
    let first_ai = prepare(&first);
    let second_ai = prepare(&second);
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        rng
    );
    for (owner, ai) in [(&first, &first_ai), (&second, &second_ai)] {
        let ai = ai.lock().unwrap();
        let machine = ai
            .ai_state_machine
            .as_ref()
            .expect("factory FSM has an exact owner")
            .lock()
            .unwrap();
        assert!(Arc::ptr_eq(&machine.base.get_owner().unwrap(), owner));
        assert_eq!(
            machine.get_current_state_id(),
            None,
            "preparation does not initialize the machine"
        );
        assert_eq!(ai.get_next_mood_check_time(), 0);
    }
    assert!(super::registry::get_unit_arc(id).is_none());
    drop(foreign);
}
#[test]
fn foreign_same_id_unit_cannot_select_factory_owner_or_receive_its_module_flags() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_24;
    let actual = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let decoy = Arc::new(RwLock::new(crate::object::Object::new_test(id, 200.0)));
    let legacy = Arc::new(RwLock::new(
        Unit::new(
            decoy.clone(),
            &DefaultThingTemplate::new("LegacyDecoy".into()),
        )
        .unwrap(),
    ));
    super::register_unit(id, &legacy);
    let _unregister = Unregister(id);
    legacy.write().unwrap().auto_acquire_enemies = false;
    let ai = prepare(&actual);
    let ai = ai.lock().unwrap();
    let machine = ai.ai_state_machine.as_ref().unwrap().lock().unwrap();
    assert!(Arc::ptr_eq(&machine.base.get_owner().unwrap(), &actual));
    assert!(
        !legacy.read().unwrap().auto_acquire_enemies,
        "foreign Unit mirrors are untouched"
    );
    assert_eq!(
        ai.data.auto_acquire_enemies_when_idle,
        crate::object::update::AUTO_ACQUIRE_IDLE
    );
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(id)
            .is_some_and(|published| Arc::ptr_eq(&published, &decoy))
    );
}

#[test]
fn prepared_same_id_data_operates_beside_held_parent_and_object_guards() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_25;
    assert!(super::registry::get_unit_arc(id).is_none());
    let first = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let second = Arc::new(RwLock::new(crate::object::Object::new_test(id, 200.0)));
    let first_ai = prepare(&first);
    let second_ai = prepare(&second);
    let _held_first_owner = first.write().unwrap();
    let _held_second_owner = second.write().unwrap();
    let _held_ambient = crate::system::game_logic::get_game_logic().lock().unwrap();
    for (ai, frame, index, source, goal) in [
        (
            &first_ai,
            101,
            7,
            CommandSourceType::FromPlayer,
            Coord3D::new(1.0, 2.0, 3.0),
        ),
        (
            &second_ai,
            909,
            11,
            CommandSourceType::FromScript,
            Coord3D::new(9.0, 8.0, 7.0),
        ),
    ] {
        let mut ai = ai.lock().unwrap();
        let UnitAIUpdate {
            ai_state_machine,
            data,
            ..
        } = &mut *ai;
        let mut parent = ai_state_machine.as_ref().unwrap().lock().unwrap();
        parent.base.set_goal_position(goal);
        parent.base.lock();
        // The actual factory preparation runtime loans its disjoint data.
        // No installed AI, parent mutex, Object or frame lookup is needed.
        data.set_next_mood_check_time(frame);
        data.set_current_goal_path_index(index).unwrap();
        data.set_last_command_source(source);
        data.set_can_path_through_units(true).unwrap();
        data.set_attitude(AIAttitudeType::Aggressive).unwrap();
        assert_eq!(data.get_next_mood_check_time(), frame);
        assert_eq!(data.get_current_goal_path_index(), index);
        assert_eq!(data.get_last_command_source(), source);
        assert!(data.get_can_path_through_units());
        assert_eq!(data.get_attitude(), AIAttitudeType::Aggressive);
        assert_eq!(parent.base.get_goal_position(), goal);
        assert!(parent.base.is_locked());
        assert_eq!(parent.get_current_state_id(), None);
    }
    assert_eq!(
        first_ai.lock().unwrap().data.get_next_mood_check_time(),
        101
    );
    assert_eq!(
        second_ai.lock().unwrap().data.get_next_mood_check_time(),
        909
    );
    assert!(super::registry::get_unit_arc(id).is_none());
}

#[test]
fn prepared_data_bump_speed_preserves_cpp_arithmetic_with_parent_held() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_26;
    let owner = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let ai = prepare(&owner);
    let mut ai = ai.lock().unwrap();
    let UnitAIUpdate {
        ai_state_machine,
        data,
        ..
    } = &mut *ai;
    let _parent = ai_state_machine.as_ref().unwrap().lock().unwrap();
    let _owner = owner.write().unwrap();
    // CPP AIUpdate.cpp:2197-2218/2270-2273: the blocked branch applies 0.95;
    // recovery applies 1.05 and reduces blocked frames to one.
    data.set_cur_max_blocked_speed(40.0);
    data.blocked_frames = 7;
    assert_eq!(data.apply_bump_speed_limit(80.0, true), 40.0 * 0.95);
    assert_eq!(data.blocked_frames, 7);
    assert_eq!(data.apply_bump_speed_limit(60.0, false), 40.0 * 0.95 * 1.05);
    assert_eq!(data.blocked_frames, 1);
}

#[test]
fn prepared_data_loans_the_actual_locomotor_member_with_parent_held() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_27;
    let owner = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let name = "PreparedDataBorrowLoco";
    let mut loco = crate::locomotor::LocomotorTemplate::new(name.into());
    loco.preferred_height = 10.0;
    crate::locomotor::LOCOMOTOR_STORE.register_template(loco);
    let mut module = AIUpdateModuleData::default();
    module.set_locomotor_set_entries(crate::common::LocomotorSetType::Normal, vec![name.into()]);
    let mut authored = template();
    authored.modules[0].data = Arc::new(module);
    let ai = crate::object::object_factory::factory_ai::prepare_unit_ai(&owner, &authored, id);
    let mut ai = ai.lock().unwrap();
    let UnitAIUpdate {
        ai_state_machine,
        data,
        ..
    } = &mut *ai;
    let _parent = ai_state_machine.as_ref().unwrap().lock().unwrap();
    let _owner = owner.write().unwrap();
    let _ambient = crate::system::game_logic::get_game_logic().lock().unwrap();
    let member = std::ptr::from_ref(data.locomotor_set.get_active().unwrap());
    data.with_cur_locomotor_mut(&mut |loco| {
        assert!(std::ptr::eq(std::ptr::from_ref(loco), member));
        loco.preferred_height = 73.0;
    });
    data.set_ultra_accurate(true).unwrap();
    data.with_cur_locomotor(&mut |loco| {
        assert!(std::ptr::eq(std::ptr::from_ref(loco), member));
    });
    assert_eq!(data.get_preferred_height(), Some(73.0));
    assert!(data.current_locomotor_is_ultra_accurate());
    assert_eq!(data.locomotor_set.active_name(), Some(name));
    assert!(super::registry::get_unit_arc(id).is_none());
}

#[test]
fn prepared_native_driver_changes_real_goals_and_state_before_idle_sleep_finishes() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_28;
    let first = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let second = Arc::new(RwLock::new(crate::object::Object::new_test(id, 200.0)));
    let first_ai = prepare(&first);
    let second_ai = prepare(&second);
    for (owner, handle, target, goal) in [
        (&first, &first_ai, 0xA1_F0_30, Coord3D::new(3.0, 5.0, 7.0)),
        (
            &second,
            &second_ai,
            0xA1_F0_31,
            Coord3D::new(11.0, 13.0, 17.0),
        ),
    ] {
        let mut ai = handle.lock().unwrap();
        let parent = ai.ai_state_machine.as_ref().unwrap().clone();
        let mut machine = parent.lock().unwrap();
        assert!(Arc::ptr_eq(&machine.base.get_owner().unwrap(), owner));
        assert_eq!(
            machine.set_state_with_ai(crate::ai::states::AIStateType::Idle as u32, &mut *ai),
            crate::state_machine::StateReturnType::Continue
        );
        ai.set_queue_for_path_time(123);
        let mut called = false;
        let result = machine.update_state_machine(&mut *ai, |driver, ai, _owner| {
            called = true;
            // Hold the exact Object only while the post-body operations run.
            // Completion may read it once this callback has returned.
            let _held_owner = owner.write().unwrap();
            let mut command = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::Busy,
                CommandSourceType::FromPlayer,
            );
            command.obj = Some(target);
            command.pos = goal;
            driver.ai_do_command_with_ai(&command, ai).unwrap();
            driver.set_goal_path(&[goal]);
            driver.add_to_goal_path(&goal);
            let end = Coord3D::new(goal.x + 1.0, goal.y, goal.z);
            driver.add_to_goal_path(&end);
            assert_eq!(driver.get_goal_path_size(), 2);
            assert_eq!(driver.get_goal_path_position(0), Some(&goal));
            assert_eq!(driver.get_goal_path_position(1), Some(&end));
            assert_eq!(driver.get_goal_object_id(), target);
            assert_eq!(driver.get_goal_position().unwrap(), goal);
            assert!(driver.is_busy());
            assert!(!driver.is_idle());
            assert_eq!(ai.data.queue_for_path_frame, 0);
        });
        assert!(
            called,
            "the real native body reached its operation-local driver"
        );
        // StateMachine.cpp:413-435: changed state overrides outgoing Idle Sleep.
        assert_eq!(result, crate::state_machine::StateReturnType::Continue);
        assert_eq!(
            machine.get_current_state_id(),
            Some(crate::ai::states::AIStateType::Busy as u32)
        );
        assert_eq!(machine.get_goal_object_id(), target);
        assert_eq!(machine.get_goal_position(), Some(goal));
        assert_eq!(machine.get_goal_path_position(0), Some(&goal));
        assert_eq!(machine.get_goal_path_size(), 2);
        let mut bytes = std::io::Cursor::new(Vec::new());
        machine
            .base
            .xfer(&mut game_engine::common::system::xfer_save::XferSave::new(
                &mut bytes, 1,
            ))
            .unwrap();
        let wire = bytes.into_inner();
        assert_eq!(wire[0], 1);
        assert_eq!(
            u32::from_le_bytes(wire[1..5].try_into().unwrap()),
            0,
            "outgoing Idle did not leave a sleep deadline on Busy"
        );
        assert!(Arc::ptr_eq(ai.ai_state_machine.as_ref().unwrap(), &parent));
    }
    assert_eq!(
        first_ai
            .lock()
            .unwrap()
            .ai_state_machine
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .get_goal_object_id(),
        0xA1_F0_30
    );
    assert_eq!(
        second_ai
            .lock()
            .unwrap()
            .ai_state_machine
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .get_goal_object_id(),
        0xA1_F0_31
    );
    assert!(super::registry::get_unit_arc(id).is_none());
}

#[test]
fn prepared_native_driver_preserves_cpp_logical_lock_during_commands() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_29;
    let owner = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let handle = prepare(&owner);
    let mut ai = handle.lock().unwrap();
    let parent = ai.ai_state_machine.as_ref().unwrap().clone();
    let mut machine = parent.lock().unwrap();
    machine.set_state_with_ai(crate::ai::states::AIStateType::Wait as u32, &mut *ai);
    let result = machine.update_state_machine(&mut *ai, |driver, ai, _owner| {
        let _held_owner = owner.write().unwrap();
        let command = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::Busy,
            CommandSourceType::FromAI,
        );
        driver.lock();
        driver.ai_do_command_with_ai(&command, ai).unwrap();
        assert!(driver.is_locked());
        assert_eq!(
            driver.get_current_state_id(),
            Some(crate::ai::states::AIStateType::Wait as u32)
        );
        assert!(!driver.is_busy());
        driver.unlock();
        driver.ai_do_command_with_ai(&command, ai).unwrap();
        assert!(!driver.is_locked());
        assert_eq!(
            driver.get_current_state_id(),
            Some(crate::ai::states::AIStateType::Busy as u32)
        );
        assert!(driver.is_busy());
    });
    assert_eq!(result, crate::state_machine::StateReturnType::Continue);
    assert_eq!(
        machine.get_current_state_id(),
        Some(crate::ai::states::AIStateType::Busy as u32)
    );
    assert!(!machine.is_locked());
    assert!(Arc::ptr_eq(ai.ai_state_machine.as_ref().unwrap(), &parent));
}

#[test]
fn prepared_cpp_blocked_speed_default_is_zero_and_inert() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_32;
    let first = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let second = Arc::new(RwLock::new(crate::object::Object::new_test(id, 200.0)));
    let rng = game_engine::common::random_value::get_game_logic_random_seed_state();
    let _held_ambient = crate::system::game_logic::get_game_logic().lock().unwrap();
    let first_ai = prepare(&first);
    let second_ai = prepare(&second);
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        rng
    );
    drop(_held_ambient);
    for (owner, handle) in [(&first, &first_ai), (&second, &second_ai)] {
        let ai = handle.lock().unwrap();
        let _held_owner = owner.write().unwrap();
        let machine = ai.ai_state_machine.as_ref().unwrap().lock().unwrap();
        // AIUpdate.cpp:218-220: only the blocked cap starts at zero.
        assert_eq!(ai.get_cur_max_blocked_speed(), 0.0);
        assert_eq!(ai.data.blocked_frames, 0);
        assert_eq!(ai.data.bump_speed_limit, crate::modules::FAST_AS_POSSIBLE);
        assert_eq!(ai.get_desired_speed(), crate::modules::FAST_AS_POSSIBLE);
        assert_eq!(machine.get_current_state_id(), None);
        assert!(Arc::ptr_eq(&machine.base.get_owner().unwrap(), owner));
    }
    assert!(super::registry::get_unit_arc(id).is_none());
}

#[test]
fn prepared_cpp_blocked_speed_initial_cap_limits_then_recovers() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_33;
    let owner = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let handle = prepare(&owner);
    let mut ai = handle.lock().unwrap();
    let parent = ai.ai_state_machine.as_ref().unwrap().clone();
    let _held_parent = parent.lock().unwrap();
    let _held_owner = owner.write().unwrap();
    ai.data.blocked_frames = 7;
    // AIUpdate.cpp:2197-2218/2270-2273, evaluated against the constructor cap.
    assert_eq!(ai.apply_bump_speed_limit(80.0, true), 0.0);
    assert_eq!(ai.data.bump_speed_limit, 0.0);
    assert_eq!(ai.data.blocked_frames, 7);
    assert_eq!(ai.apply_bump_speed_limit(80.0, false), 80.0 * 0.2 * 1.05);
    assert_eq!(ai.data.blocked_frames, 1);
}

#[test]
fn prepared_cpp_blocked_speed_snapshot_keeps_fresh_recomputed_cap() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_34;
    let owner = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let source = prepare(&owner);
    let loaded = prepare(&owner);
    let mut source = source.lock().unwrap();
    let mut loaded = loaded.lock().unwrap();
    source.set_cur_max_blocked_speed(27.0);
    let mut bytes = std::io::Cursor::new(Vec::new());
    assert!(
        source
            .xfer_ai_update_state(&mut game_engine::common::system::xfer_save::XferSave::new(
                &mut bytes, 1
            ))
            .unwrap()
    );
    assert!(
        loaded
            .xfer_ai_update_state(&mut game_engine::common::system::xfer_load::XferLoad::new(
                std::io::Cursor::new(bytes.into_inner()),
                1
            ))
            .unwrap()
    );
    // AIUpdate.cpp:5087-5090 explicitly omits this recomputed cap from Xfer.
    assert_eq!(source.get_cur_max_blocked_speed(), 27.0);
    assert_eq!(loaded.get_cur_max_blocked_speed(), 0.0);
    assert!(Arc::ptr_eq(
        &loaded
            .ai_state_machine
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .base
            .get_owner()
            .unwrap(),
        &owner
    ));
}
