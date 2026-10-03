//! Callback-level scheduler contracts from GameLogic.cpp:2965–3021,3698–3736.
//! Exercise the production dispatcher with the driving world's borrowed scheduler.

use super::*;
use crate::object::Object;
use game_engine::common::thing::update_module::{
    UpdateModuleInterface, UpdateModulePtr, UpdateScheduleContext,
};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

type UpdateResult = Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>>;

struct CountUpdate {
    calls: Arc<AtomicU32>,
    observed_frame: Arc<AtomicU32>,
    phase: SleepyUpdatePhase,
    sleep: UpdateSleepTime,
    trace: Option<(Arc<AtomicU64>, u64)>,
}

impl UpdateModuleInterface for CountUpdate {
    fn update_scheduled(&mut self, context: &mut dyn UpdateScheduleContext) -> UpdateResult {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.observed_frame
            .store(context.frame(), Ordering::Relaxed);
        if let Some((trace, digit)) = &self.trace {
            trace
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |previous| {
                    Some(previous * 10 + digit)
                })
                .unwrap();
        }
        Ok(self.sleep)
    }

    fn get_update_phase(&self) -> SleepyUpdatePhase {
        self.phase
    }
}

enum CallbackAction {
    Awaken(UpdateModulePtr),
    Unregister(UpdateModulePtr),
}

struct SiblingUpdate {
    action: CallbackAction,
    calls: Arc<AtomicU32>,
}

impl UpdateModuleInterface for SiblingUpdate {
    fn update_scheduled(&mut self, context: &mut dyn UpdateScheduleContext) -> UpdateResult {
        self.calls.fetch_add(1, Ordering::Relaxed);
        match &self.action {
            CallbackAction::Awaken(target) => {
                let frame = context.frame();
                context.awaken(target, frame);
            }
            CallbackAction::Unregister(target) => context.unregister(target),
        }
        Ok(UpdateSleepTime::Forever)
    }
}

fn count_update(
    phase: SleepyUpdatePhase,
    sleep: UpdateSleepTime,
) -> (UpdateModulePtr, Arc<AtomicU32>, Arc<AtomicU32>) {
    let calls = Arc::new(AtomicU32::new(0));
    let observed_frame = Arc::new(AtomicU32::new(u32::MAX));
    let module: UpdateModulePtr = Arc::new(RwLock::new(CountUpdate {
        calls: calls.clone(),
        observed_frame: observed_frame.clone(),
        phase,
        sleep,
        trace: None,
    }));
    (module, calls, observed_frame)
}

fn sibling_update(action: CallbackAction) -> (UpdateModulePtr, Arc<AtomicU32>) {
    let calls = Arc::new(AtomicU32::new(0));
    let module: UpdateModulePtr = Arc::new(RwLock::new(SiblingUpdate {
        action,
        calls: calls.clone(),
    }));
    (module, calls)
}

fn insert_owner(world: &mut GameLogic, id: ObjectID) {
    world
        .objects
        .insert(id, Arc::new(RwLock::new(Object::new_test(id, 100.0))));
}

fn count_registration(world: &GameLogic, target: &UpdateModulePtr) -> usize {
    world
        .sleepy_updates
        .iter()
        .filter(|entry| Arc::ptr_eq(&entry.module, target))
        .count()
}

#[test]
fn sibling_awaken_of_processed_module_runs_again_without_duplicate_registration() {
    // CPP3698–3736 rebalances immediately; B can awaken the already processed A.
    let mut world = GameLogic::new();
    world.frame = 4;
    insert_owner(&mut world, 81);
    let (first, first_calls, _) =
        count_update(SleepyUpdatePhase::Initial, UpdateSleepTime::Forever);
    let (second, second_calls) = sibling_update(CallbackAction::Awaken(first.clone()));
    world.register_sleepy_update_module(81, first.clone(), 4);
    world.register_sleepy_update_module(81, second.clone(), 4);

    world.process_sleepy_updates(4);

    assert_eq!(first_calls.load(Ordering::Relaxed), 2);
    assert_eq!(second_calls.load(Ordering::Relaxed), 1);
    assert_eq!(count_registration(&world, &first), 1);
    assert_eq!(count_registration(&world, &second), 1);
}

#[test]
fn sibling_unregister_of_processed_module_cannot_resurrect_it() {
    let mut world = GameLogic::new();
    world.frame = 4;
    insert_owner(&mut world, 82);
    let (first, first_calls, _) =
        count_update(SleepyUpdatePhase::Initial, UpdateSleepTime::Forever);
    let (second, second_calls) = sibling_update(CallbackAction::Unregister(first.clone()));
    world.register_sleepy_update_module(82, first.clone(), 4);
    world.register_sleepy_update_module(82, second, 4);

    world.process_sleepy_updates(4);

    assert_eq!(first_calls.load(Ordering::Relaxed), 1);
    assert_eq!(second_calls.load(Ordering::Relaxed), 1);
    assert_eq!(count_registration(&world, &first), 0);
    assert!(
        world.module_lookup[&82]
            .iter()
            .all(|identity| *identity != ModuleIdentity::of(&first))
    );
}

struct SelfAwakeningUpdate {
    self_handle: Option<UpdateModulePtr>,
    calls: Arc<AtomicU32>,
}

impl UpdateModuleInterface for SelfAwakeningUpdate {
    fn update_scheduled(&mut self, context: &mut dyn UpdateScheduleContext) -> UpdateResult {
        self.calls.fetch_add(1, Ordering::Relaxed);
        // Consume the fixture's handle so it retains no self-ownership cycle.
        let own = self.self_handle.take().expect("first scheduled callback");
        let frame = context.frame();
        context.awaken(&own, frame);
        Ok(UpdateSleepTime::Frames(3))
    }
}

#[test]
fn current_callback_awaken_is_ignored_and_returned_sleep_wins() {
    // CPP2965–2969 ignores setWakeFrame on the current update; its return wins.
    let mut world = GameLogic::new();
    world.frame = 6;
    insert_owner(&mut world, 83);
    let calls = Arc::new(AtomicU32::new(0));
    let concrete = Arc::new(RwLock::new(SelfAwakeningUpdate {
        self_handle: None,
        calls: calls.clone(),
    }));
    let module: UpdateModulePtr = concrete.clone();
    concrete.write().unwrap().self_handle = Some(module.clone());
    world.register_sleepy_update_module(83, module.clone(), 6);

    world.process_sleepy_updates(6);

    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(count_registration(&world, &module), 1);
    let entry = world
        .sleepy_updates
        .iter()
        .find(|entry| Arc::ptr_eq(&entry.module, &module))
        .unwrap();
    assert_eq!(entry.wake_frame, 9);
}

#[test]
fn interleaved_same_id_world_callbacks_use_their_driving_scheduler() {
    let mut first_world = GameLogic::new();
    let mut second_world = GameLogic::new();
    first_world.frame = 4;
    second_world.frame = 9;
    insert_owner(&mut first_world, 84);
    insert_owner(&mut second_world, 84);
    let (first, first_calls, first_frame) =
        count_update(SleepyUpdatePhase::Initial, UpdateSleepTime::Forever);
    let (second, second_calls, second_frame) =
        count_update(SleepyUpdatePhase::Initial, UpdateSleepTime::Forever);
    let (wake_first, _) = sibling_update(CallbackAction::Awaken(first.clone()));
    let (remove_second, _) = sibling_update(CallbackAction::Unregister(second.clone()));
    first_world.register_sleepy_update_module(84, first.clone(), 4);
    first_world.register_sleepy_update_module(84, wake_first, 4);
    second_world.register_sleepy_update_module(84, second.clone(), 9);
    second_world.register_sleepy_update_module(84, remove_second, 9);

    first_world.process_sleepy_updates(4);
    assert_eq!(second_calls.load(Ordering::Relaxed), 0);
    assert_eq!(count_registration(&second_world, &second), 1);
    second_world.process_sleepy_updates(9);

    assert_eq!(first_calls.load(Ordering::Relaxed), 2);
    assert_eq!(second_calls.load(Ordering::Relaxed), 1);
    assert_eq!(first_frame.load(Ordering::Relaxed), 4);
    assert_eq!(second_frame.load(Ordering::Relaxed), 9);
    assert_eq!(count_registration(&first_world, &first), 1);
    assert_eq!(count_registration(&second_world, &second), 0);
}

#[test]
fn equal_priority_callbacks_follow_cpp_in_place_heap_rebalancing() {
    // CPP2761–2887: strict priority comparison, left child on ties, no FIFO key.
    // Three equal entries start [1,2,3]; after each frame the root is retained
    // when both children tie, giving callback orders 123,312,231.
    let mut world = GameLogic::new();
    world.frame = 21;
    insert_owner(&mut world, 85);
    let trace = Arc::new(AtomicU64::new(0));
    for digit in 1..=3 {
        let module: UpdateModulePtr = Arc::new(RwLock::new(CountUpdate {
            calls: Arc::new(AtomicU32::new(0)),
            observed_frame: Arc::new(AtomicU32::new(0)),
            phase: SleepyUpdatePhase::Normal,
            sleep: UpdateSleepTime::None,
            trace: Some((trace.clone(), digit)),
        }));
        world.register_sleepy_update_module(85, module, 21);
    }

    for frame in 21..=23 {
        world.frame = frame;
        world.process_sleepy_updates(frame);
    }

    assert_eq!(trace.load(Ordering::Relaxed), 123_312_231);
    assert_eq!(world.sleepy_updates.len(), 3);
}

#[test]
fn registration_has_one_shared_handle_and_category_changes_retire_old_entries() {
    let mut world = GameLogic::new();
    let (module, _, _) = count_update(SleepyUpdatePhase::Normal, UpdateSleepTime::None);
    world.register_sleepy_update_module(91, module.clone(), 10);
    assert_eq!(
        Arc::strong_count(&module),
        2,
        "caller and canonical schedule only"
    );
    world.register_normal_update_module(92, module.clone());
    assert_eq!(world.sleepy_update_count(), 0);
    assert_eq!(world.normal_updates.len(), 1);
    assert!(!world.module_lookup.contains_key(&91));
    assert_eq!(Arc::strong_count(&module), 2);
    world.remove_updates_for_object(91);
    assert_eq!(
        world.normal_updates.len(),
        1,
        "old owner cannot remove new registration"
    );
    world.register_sleepy_update_module(93, module.clone(), 12);
    assert!(world.normal_updates.is_empty());
    assert!(!world.module_lookup.contains_key(&92));
    let entry = world.pop_sleepy_update().unwrap();
    assert!(world.module_lookup.is_empty());
    drop(entry);
    assert_eq!(Arc::strong_count(&module), 1);
}

#[test]
fn retired_handle_cannot_awaken_equal_id_replacement_or_another_match() {
    let mut world = GameLogic::new();
    let (old, _, _) = count_update(SleepyUpdatePhase::Normal, UpdateSleepTime::None);
    world.register_sleepy_update_module(94, old.clone(), 10);
    world.unregister_update_module(94, old.clone());
    let (replacement, _, _) = count_update(SleepyUpdatePhase::Normal, UpdateSleepTime::None);
    world.register_sleepy_update_module(94, replacement.clone(), 20);
    world.friend_awaken_update_module(&old, 1);
    assert_eq!(world.sleepy_entry_for(&replacement), Some((94, 20)));
    let mut other = GameLogic::new();
    let (other_module, _, _) = count_update(SleepyUpdatePhase::Normal, UpdateSleepTime::None);
    other.register_sleepy_update_module(94, other_module.clone(), 30);
    other.friend_awaken_update_module(&replacement, 1);
    assert_eq!(other.sleepy_entry_for(&other_module), Some((94, 30)));
    assert_eq!(Arc::strong_count(&old), 1);
}

use game_engine::common::system::{Snapshotable, Xfer as ModuleXfer};
use game_engine::common::thing::module::{BaseModuleData, Module, ModuleData};

struct ScheduledOnlyModule {
    data: Arc<BaseModuleData>,
    calls: Arc<AtomicU32>,
    observed_frame: Arc<AtomicU32>,
}

struct SimpleOnlyModule {
    data: Arc<BaseModuleData>,
    calls: Arc<AtomicU32>,
}

// Identical Module wrapper contracts; callback implementations below remain
// separate so each test actually checks the other callback's default bridge.
macro_rules! impl_test_update_module {
    ($module:ty) => {
        impl Snapshotable for $module {
            fn crc(&self, _xfer: &mut dyn ModuleXfer) -> Result<(), String> {
                Ok(())
            }
            fn xfer(&mut self, _xfer: &mut dyn ModuleXfer) -> Result<(), String> {
                Ok(())
            }
            fn load_post_process(&mut self) -> Result<(), String> {
                Ok(())
            }
        }
        impl Module for $module {
            fn get_module_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
                0x5052_4f58
            }
            fn get_module_data(&self) -> &dyn ModuleData {
                self.data.as_ref()
            }
            fn get_update_module_interface(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
                Some(self)
            }
            fn get_sleepy_update_interface(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
                Some(self)
            }
        }
    };
}

impl_test_update_module!(ScheduledOnlyModule);
impl_test_update_module!(SimpleOnlyModule);

impl UpdateModuleInterface for ScheduledOnlyModule {
    // Intentionally no update() or update_simple(): the real proxy must forward
    // this interface's scheduled hook, not silently use either default.
    fn update_scheduled(&mut self, context: &mut dyn UpdateScheduleContext) -> UpdateResult {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.observed_frame
            .store(context.frame(), Ordering::Relaxed);
        Ok(UpdateSleepTime::Frames(3))
    }
}

impl UpdateModuleInterface for SimpleOnlyModule {
    // Intentionally no update_scheduled(): existing simple-only modules must
    // retain their callback through the new default scheduled bridge.
    fn update_simple(&mut self) -> UpdateSleepTime {
        self.calls.fetch_add(1, Ordering::Relaxed);
        UpdateSleepTime::Frames(2)
    }
}

fn installed_scheduled_only_module(
    world: &mut GameLogic,
    object_id: ObjectID,
    wake: u32,
) -> (UpdateModulePtr, Arc<AtomicU32>, Arc<AtomicU32>) {
    let data = Arc::new(BaseModuleData::new());
    let calls = Arc::new(AtomicU32::new(0));
    let observed_frame = Arc::new(AtomicU32::new(u32::MAX));
    let behavior = ScheduledOnlyModule {
        data: Arc::clone(&data),
        calls: Arc::clone(&calls),
        observed_frame: Arc::clone(&observed_frame),
    };
    let (object, proxy) = Object::installed_update_proxy_for_test(
        object_id,
        "ScheduledOnlyModule",
        Box::new(behavior),
        data,
    );
    world
        .objects
        .insert(object_id, Arc::new(RwLock::new(object)));
    world.register_sleepy_update_module(object_id, Arc::clone(&proxy), wake);
    (proxy, calls, observed_frame)
}

#[test]
fn installed_module_proxy_forwards_scheduled_only_hook_to_driving_world() {
    // CPP GameLogic.cpp:3698-3736 executes the admitted update interface and
    // computes its next wake from the driving frame and returned sleep.
    let mut first_world = GameLogic::new();
    let mut second_world = GameLogic::new();
    let (first, first_calls, first_frame) =
        installed_scheduled_only_module(&mut first_world, 0x5052, 4);
    let (second, second_calls, second_frame) =
        installed_scheduled_only_module(&mut second_world, 0x5052, 13);

    first_world.process_sleepy_updates(4);
    assert_eq!(first_calls.load(Ordering::Relaxed), 1);
    assert_eq!(first_frame.load(Ordering::Relaxed), 4);
    assert_eq!(second_calls.load(Ordering::Relaxed), 0);
    assert_eq!(first_world.sleepy_entry_for(&first), Some((0x5052, 7)));

    second_world.process_sleepy_updates(13);
    assert_eq!(second_calls.load(Ordering::Relaxed), 1);
    assert_eq!(second_frame.load(Ordering::Relaxed), 13);
    assert_eq!(second_world.sleepy_entry_for(&second), Some((0x5052, 16)));
    assert_eq!(first_frame.load(Ordering::Relaxed), 4);

    first_world.process_sleepy_updates(6);
    assert_eq!(first_calls.load(Ordering::Relaxed), 1);
    first_world.process_sleepy_updates(7);
    assert_eq!(first_calls.load(Ordering::Relaxed), 2);
    assert_eq!(first_frame.load(Ordering::Relaxed), 7);
    assert_eq!(first_world.sleepy_entry_for(&first), Some((0x5052, 10)));
    assert_eq!(count_registration(&first_world, &first), 1);
    assert_eq!(count_registration(&second_world, &second), 1);
}

#[test]
fn installed_module_proxy_preserves_simple_only_callback_and_sleep() {
    let mut world = GameLogic::new();
    let data = Arc::new(BaseModuleData::new());
    let calls = Arc::new(AtomicU32::new(0));
    let behavior = SimpleOnlyModule {
        data: Arc::clone(&data),
        calls: Arc::clone(&calls),
    };
    let (object, proxy) = Object::installed_update_proxy_for_test(
        0x5053,
        "SimpleOnlyModule",
        Box::new(behavior),
        data,
    );
    world.objects.insert(0x5053, Arc::new(RwLock::new(object)));
    world.register_sleepy_update_module(0x5053, Arc::clone(&proxy), 4);

    world.process_sleepy_updates(4);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(world.sleepy_entry_for(&proxy), Some((0x5053, 6)));
    world.process_sleepy_updates(5);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    world.process_sleepy_updates(6);
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert_eq!(world.sleepy_entry_for(&proxy), Some((0x5053, 8)));
    assert_eq!(count_registration(&world, &proxy), 1);
}

#[test]
fn set_defaults_retires_all_registration_keys_before_equal_id_readmission() {
    let mut world = GameLogic::new();
    let (old, _, _) = count_update(SleepyUpdatePhase::Normal, UpdateSleepTime::None);
    world.register_sleepy_update_module(95, old.clone(), 10);
    world.set_defaults(false);
    assert!(world.module_lookup.is_empty());
    assert!(world.normal_updates.is_empty());
    assert_eq!(world.sleepy_update_count(), 0);
    assert_eq!(Arc::strong_count(&old), 1);
    let (replacement, _, _) = count_update(SleepyUpdatePhase::Normal, UpdateSleepTime::None);
    world.register_sleepy_update_module(95, replacement.clone(), 20);
    world.friend_awaken_update_module(&old, 1);
    assert_eq!(world.sleepy_entry_for(&replacement), Some((95, 20)));
}

#[test]
fn sleepy_legacy_current_guard_restores_outer_update_after_nesting_and_unwind() {
    let (outer, _, _) = count_update(SleepyUpdatePhase::Normal, UpdateSleepTime::None);
    let (inner, _, _) = count_update(SleepyUpdatePhase::Normal, UpdateSleepTime::None);
    assert!(!is_cur_update_module(&outer));
    let outer_guard = enter_cur_update_module(&outer);
    assert!(is_cur_update_module(&outer));
    {
        let _inner_guard = enter_cur_update_module(&inner);
        assert!(is_cur_update_module(&inner));
        assert!(!is_cur_update_module(&outer));
    }
    assert!(is_cur_update_module(&outer));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _inner_guard = enter_cur_update_module(&inner);
        assert!(is_cur_update_module(&inner));
        panic!("nested update fixture");
    }));
    assert!(panic.is_err());
    assert!(is_cur_update_module(&outer));
    assert!(!is_cur_update_module(&inner));
    drop(outer_guard);
    assert!(!is_cur_update_module(&outer));
    assert!(!is_cur_update_module(&inner));
}
