// C++ UpdateModule.h: UPDATE_SLEEP_FOREVER = 0x3fffffff (clamped so offsets cannot overflow).
const UPDATE_SLEEP_FOREVER_FRAMES: UnsignedInt = 0x3fff_ffff;

thread_local! {
    static CUR_UPDATE_MODULE: std::cell::RefCell<Option<UpdateModulePtr>> =
        const { std::cell::RefCell::new(None) };
}

// Temporary legacy publication must restore an outer synchronous update.
// New scheduling callbacks use their explicit context instead of this slot.
struct CurUpdateModuleGuard {
    previous: Option<UpdateModulePtr>,
}

impl Drop for CurUpdateModuleGuard {
    fn drop(&mut self) {
        CUR_UPDATE_MODULE.with(|slot| {
            *slot.borrow_mut() = self.previous.take();
        });
    }
}

fn enter_cur_update_module(module: &UpdateModulePtr) -> CurUpdateModuleGuard {
    let previous = CUR_UPDATE_MODULE.with(|slot| slot.borrow_mut().replace(Arc::clone(module)));
    CurUpdateModuleGuard { previous }
}

pub(crate) fn is_cur_update_module(module: &UpdateModulePtr) -> bool {
    CUR_UPDATE_MODULE.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|cur| Arc::ptr_eq(cur, module))
    })
}

/// C++ GameLogic.cpp:3678 / 3718 — `!dis.any() || dis.anyIntersectionWith(...)`.
fn disabled_module_should_process(
    object_disabled: DisabledMaskType,
    module_disabled_types: DisabledMaskType,
) -> bool {
    !object_disabled.any() || object_disabled.intersects(module_disabled_types)
}

/// Borrow only scheduling state, not the mutable match or an ambient singleton.
struct UpdateExecutionContext<'a> {
    queue: &'a mut SleepyUpdateQueue,
    normal: &'a mut Vec<NormalUpdateEntry>,
    lookup: &'a mut HashMap<ObjectID, Vec<ModuleIdentity>>,
    current: &'a UpdateModulePtr,
    now: UnsignedInt,
    normal_cursor: Option<&'a mut usize>,
}

fn awaken_registered_update(
    queue: &mut SleepyUpdateQueue,
    module: &UpdateModulePtr,
    now: UnsignedInt,
    wake: UnsignedInt,
) {
    let wake = wake.min(UPDATE_SLEEP_FOREVER_FRAMES);
    let Some(entry) = queue.entry_for(module) else {
        return;
    };
    // C++ GameLogic.cpp:2971–2982: already awake must still run this frame.
    if entry.wake_frame == wake
        || (now > 0 && entry.wake_frame == now && wake == now.saturating_add(1))
    {
        return;
    }
    let phase = module.read().ok().map(|update| update.get_update_phase());
    queue.reschedule(module, wake, phase);
}

impl UpdateScheduleContext for UpdateExecutionContext<'_> {
    fn frame(&self) -> u32 {
        self.now
    }

    fn awaken(&mut self, module: &UpdateModulePtr, wake_frame: u32) {
        // C++ GameLogic.cpp:2965–2969: this update's returned sleep wins.
        if !Arc::ptr_eq(self.current, module) {
            awaken_registered_update(self.queue, module, self.now, wake_frame);
        }
    }

    fn unregister(&mut self, module: &UpdateModulePtr) {
        // Removal is visible before the next callback. Keep the normal-list
        // cursor on the next surviving entry, including self/earlier removal.
        if let Some(cursor) = &mut self.normal_cursor {
            if let Some(index) = self
                .normal
                .iter()
                .position(|entry| Arc::ptr_eq(&entry.module, module))
            {
                if index < **cursor {
                    **cursor -= 1;
                }
            }
        }
        unregister_registered_update(self.queue, self.normal, self.lookup, module);
    }
}

// Registration metadata belongs to the canonical list/heap entry. Remove only
// its owner's keys; admitting a new module must not scan every match object.
fn remove_registration_identity(
    lookup: &mut HashMap<ObjectID, Vec<ModuleIdentity>>,
    owner: ObjectID,
    identity: ModuleIdentity,
) {
    if let Some(modules) = lookup.get_mut(&owner) {
        modules.retain(|entry| *entry != identity);
        if modules.is_empty() {
            lookup.remove(&owner);
        }
    }
}

fn unregister_registered_update(
    queue: &mut SleepyUpdateQueue,
    normal: &mut Vec<NormalUpdateEntry>,
    lookup: &mut HashMap<ObjectID, Vec<ModuleIdentity>>,
    module: &UpdateModulePtr,
) {
    let sleepy_owner = queue.erase(module).map(|entry| entry.object_id);
    let normal_owner = normal
        .iter()
        .find(|entry| Arc::ptr_eq(&entry.module, module))
        .map(|entry| entry.object_id);
    if normal_owner.is_some() {
        normal.retain(|entry| !Arc::ptr_eq(&entry.module, module));
    }
    let identity = ModuleIdentity::of(module);
    for owner in [sleepy_owner, normal_owner].into_iter().flatten() {
        remove_registration_identity(lookup, owner, identity);
    }
}

impl GameLogic {
    fn process_sleepy_updates(&mut self, current_frame: UnsignedInt) {
        // C++ GameLogic.cpp:3698–3736 keeps a module registered during update,
        // then rebalances it immediately before executing the next callback.
        while let Some(entry) = self.sleepy_updates.peek() {
            if entry.wake_frame > current_frame {
                break;
            }
            let object_id = entry.object_id;
            let module = Arc::clone(&entry.module);
            let (module_disabled_mask, phase) = module
                .read()
                .map(|module| {
                    (
                        module.get_disabled_types_to_process(),
                        module.get_update_phase(),
                    )
                })
                .unwrap_or((DisabledMaskType::empty(), SleepyUpdatePhase::Normal));

            let object_disabled = match self.objects.get(&object_id) {
                Some(object) => object.read().ok().map(|object| object.get_disabled_flags()),
                None => {
                    self.unregister_update_module(object_id, module);
                    continue;
                }
            };
            let should_process = object_disabled
                .map(|mask| disabled_module_should_process(mask, module_disabled_mask))
                .unwrap_or(true);
            let sleep = if should_process {
                // Legacy callers still use this bridge; explicit scheduling
                // callbacks below do not discover it or lock GameLogic again.
                let _cur = enter_cur_update_module(&module);
                match module.write() {
                    Ok(mut update) => {
                        let mut context = UpdateExecutionContext {
                            queue: &mut self.sleepy_updates,
                            normal: &mut self.normal_updates,
                            lookup: &mut self.module_lookup,
                            current: &module,
                            now: current_frame,
                            normal_cursor: None,
                        };
                        match update.update_scheduled(&mut context) {
                            Ok(sleep) => sleep,
                            Err(error) => {
                                warn!(
                                    "Sleepy update module for object {} failed: {}",
                                    object_id, error
                                );
                                UpdateSleepTime::None
                            }
                        }
                    }
                    Err(_) => {
                        warn!(
                            "Sleepy update module lock poisoned for object {}",
                            object_id
                        );
                        UpdateSleepTime::None
                    }
                }
            } else {
                UpdateSleepTime::None
            };
            let next_wake = match sleep {
                UpdateSleepTime::Forever => UPDATE_SLEEP_FOREVER_FRAMES,
                UpdateSleepTime::None => current_frame
                    .saturating_add(1)
                    .min(UPDATE_SLEEP_FOREVER_FRAMES),
                UpdateSleepTime::Frames(frames) => current_frame
                    .saturating_add(frames.max(1))
                    .min(UPDATE_SLEEP_FOREVER_FRAMES),
            };
            // An explicit callback can unregister itself. Never resurrect it.
            if self.sleepy_updates.index_of(&module).is_some() {
                let phase = module
                    .read()
                    .map(|update| update.get_update_phase())
                    .unwrap_or(phase);
                self.sleepy_updates
                    .reschedule(&module, next_wake, Some(phase));
            }
        }
    }

    pub fn friend_awaken_update_module(
        &mut self,
        module: &UpdateModulePtr,
        when_to_wake_up: UnsignedInt,
    ) {
        if when_to_wake_up < self.frame {
            warn!(
                "setWakeFrame frame {} is in the past (now={})",
                when_to_wake_up, self.frame
            );
        }
        if !is_cur_update_module(module) {
            awaken_registered_update(
                &mut self.sleepy_updates,
                module,
                self.frame,
                when_to_wake_up,
            );
        }
    }

    /// C++ GameLogic.cpp:2881: parent first, then child, with strict priorities.
    pub fn rebalance_sleepy_update(&mut self, index: usize) {
        self.sleepy_updates.rebalance(index);
    }

    pub fn rebalance_parent_sleepy_update(&mut self, index: usize) -> usize {
        self.sleepy_updates.rebalance_parent(index)
    }

    pub fn rebalance_child_sleepy_update(&mut self, index: usize) -> usize {
        self.sleepy_updates.rebalance_child(index)
    }

    pub fn validate_sleepy_update(&self) {
        self.sleepy_updates.validate();
    }

    /// Process normal (every-frame) update modules
    fn process_normal_updates(&mut self) {
        let phases = [
            SleepyUpdatePhase::Initial,
            SleepyUpdatePhase::Physics,
            SleepyUpdatePhase::Normal,
            SleepyUpdatePhase::Final,
        ];

        for phase in phases {
            let mut cursor = 0;
            while cursor < self.normal_updates.len() {
                let entry = &self.normal_updates[cursor];
                cursor += 1;
                let (module_disabled_mask, module_phase) = entry
                    .module
                    .read()
                    .map(|module| {
                        let module_phase = module.get_update_phase();
                        // Class phase is immutable. Only the selected phase
                        // needs to borrow mutable module state for its mask.
                        let disabled = if module_phase == phase {
                            module.get_disabled_types_to_process()
                        } else {
                            DisabledMaskType::empty()
                        };
                        (disabled, module_phase)
                    })
                    .unwrap_or((DisabledMaskType::empty(), SleepyUpdatePhase::Normal));
                if module_phase != phase {
                    continue;
                }

                // Scoped map borrow for the disabled-flag check — no Arc
                // handle clone per entry per phase. A missing object still
                // skips the module; a poisoned lock still reads as "not
                // disabled".
                let object_disabled = match self.objects.get(&entry.object_id) {
                    Some(obj_ref) => obj_ref.read().ok().map(|obj| obj.get_disabled_flags()),
                    None => continue,
                };
                let should_process = match object_disabled {
                    Some(mask) => disabled_module_should_process(mask, module_disabled_mask),
                    None => true,
                };
                if !should_process {
                    continue;
                }

                let object_id = entry.object_id;
                let handle = Arc::clone(&entry.module);
                let _cur = enter_cur_update_module(&handle);
                if let Ok(mut module) = handle.write() {
                    let mut context = UpdateExecutionContext {
                        queue: &mut self.sleepy_updates,
                        normal: &mut self.normal_updates,
                        lookup: &mut self.module_lookup,
                        current: &handle,
                        now: self.frame,
                        normal_cursor: Some(&mut cursor),
                    };
                    match module.update_scheduled(&mut context) {
                        Ok(UpdateSleepTime::None) => {}
                        Ok(other) => {
                            warn!(
                                "Normal update module for object {} returned sleep {:?}",
                                object_id, other
                            );
                        }
                        Err(e) => {
                            warn!(
                                "Normal update module for object {} failed: {}",
                                object_id, e
                            );
                        }
                    }
                }
            }
        }
    }

    pub fn get_number_sleepy_updates(&self) -> usize {
        self.sleepy_updates.len()
    }

    // =========================================================================
    // Update Module Registration
    // =========================================================================

    /// Register a normal (every-frame) update module
    pub fn register_normal_update_module(&mut self, object_id: ObjectID, module: UpdateModulePtr) {
        // Move one registration between categories/owners, never duplicate it.
        self.unregister_update_module(object_id, Arc::clone(&module));
        self.module_lookup
            .entry(object_id)
            .or_default()
            .push(ModuleIdentity::of(&module));

        self.normal_updates
            .push(NormalUpdateEntry { object_id, module });
    }

    /// Register a sleepy (delayed) update module
    pub fn register_sleepy_update_module(
        &mut self,
        object_id: ObjectID,
        module: UpdateModulePtr,
        wake_frame: UnsignedInt,
    ) {
        // Move one registration between categories/owners, never duplicate it.
        self.unregister_update_module(object_id, Arc::clone(&module));
        self.module_lookup
            .entry(object_id)
            .or_default()
            .push(ModuleIdentity::of(&module));

        // C++ GameLogic.cpp:3872-3899 — wake 0 (ctor never called setWakeFrame) becomes
        // the current frame, or 1 when the world is still on frame 0, so the module
        // can tick in the same frame it was registered.
        let wake = if wake_frame == 0 {
            self.frame.max(1)
        } else {
            wake_frame.min(UPDATE_SLEEP_FOREVER_FRAMES)
        };

        let phase = module
            .read()
            .map(|module| module.get_update_phase())
            .unwrap_or(SleepyUpdatePhase::Normal);

        self.sleepy_updates.push(SleepyUpdateEntry {
            wake_frame: wake,
            phase,
            module,
            object_id,
        });
    }
    /// True when an AI update is already on the normal list or due at `now + 1` or sooner.
    pub fn ai_update_already_due(&self, object_id: ObjectID, now: UnsignedInt) -> bool {
        let due = now.saturating_add(1);
        for entry in &self.normal_updates {
            if entry.object_id != object_id {
                continue;
            }
            let is_ai = entry
                .module
                .read()
                .ok()
                .map(|proxy| proxy.module_name().contains("AIUpdate"))
                .unwrap_or(false);
            if is_ai {
                return true;
            }
        }
        for entry in self.sleepy_updates.iter() {
            if entry.object_id != object_id || entry.wake_frame > due {
                continue;
            }
            let is_ai = entry
                .module
                .read()
                .ok()
                .map(|proxy| proxy.module_name().contains("AIUpdate"))
                .unwrap_or(false);
            if is_ai {
                return true;
            }
        }
        false
    }

    /// Unregister an update module
    pub fn unregister_update_module(&mut self, _object_id: ObjectID, module: UpdateModulePtr) {
        unregister_registered_update(
            &mut self.sleepy_updates,
            &mut self.normal_updates,
            &mut self.module_lookup,
            &module,
        );
    }

    /// Remove all update modules for an object
    fn remove_updates_for_object(&mut self, object_id: ObjectID) {
        if let Some(entries) = self.module_lookup.remove(&object_id) {
            for module in entries {
                self.sleepy_updates.erase_identity(module);
            }
        }
        self.normal_updates
            .retain(|entry| entry.object_id != object_id);
    }

    /// PARITY_NOTE: GameLogic::pushSleepyUpdate(UpdateModulePtr) C++ line 2907.
    /// Adds a module to the sleepy heap. Wake 0 is current frame (min 1).
    pub fn push_sleepy_update(&mut self, object_id: ObjectID, module: UpdateModulePtr) {
        // C++ pushSleepyUpdate uses the module's next-call frame; wake 0 is "now".
        let wake_frame = self.frame.max(1);
        self.register_sleepy_update_module(object_id, module, wake_frame);
    }

    /// PARITY_NOTE: GameLogic::popSleepyUpdate() C++ line 2930.
    pub fn pop_sleepy_update(&mut self) -> Option<SleepyUpdateEntry> {
        let entry = self.sleepy_updates.pop()?;
        remove_registration_identity(
            &mut self.module_lookup,
            entry.object_id,
            ModuleIdentity::of(&entry.module),
        );
        Some(entry)
    }

    /// PARITY_NOTE: GameLogic::peekSleepyUpdate() C++ line 2920.
    pub fn peek_sleepy_update(&self) -> Option<&SleepyUpdateEntry> {
        self.sleepy_updates.peek()
    }

    /// PARITY_NOTE: GameLogic::eraseSleepyUpdate(Int i) C++ line 2737.
    pub fn erase_sleepy_update(&mut self, target_module: &UpdateModulePtr) {
        if let Some(entry) = self.sleepy_updates.erase(target_module) {
            remove_registration_identity(
                &mut self.module_lookup,
                entry.object_id,
                ModuleIdentity::of(target_module),
            );
        }
    }

    /// C++ GameLogic.cpp:2890: bottom-up child rebalancing, including equal ties.
    pub fn remake_sleepy_update(&mut self) {
        self.sleepy_updates.remake();
    }

    pub fn sleepy_update_count(&self) -> usize {
        self.sleepy_updates.len()
    }

    fn refresh_global_weapon_bonuses(&mut self) {
        self.global_weapon_bonus_set = build_global_weapon_bonus_set();
    }
}

#[cfg(test)]
impl GameLogic {
    pub(crate) fn sleepy_entry_for(
        &self,
        module: &UpdateModulePtr,
    ) -> Option<(ObjectID, UnsignedInt)> {
        self.sleepy_updates
            .iter()
            .find(|entry| Arc::ptr_eq(&entry.module, module))
            .map(|entry| (entry.object_id, entry.wake_frame))
    }
}

#[cfg(test)]
mod sleepy_parity_tests {
    use super::*;
    use crate::common::DisabledType;
    use crate::modules::UpdateModuleInterface;
    use crate::object::Object;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct CountingUpdate {
        count: Arc<AtomicU32>,
        disabled: DisabledMaskType,
        sleep: UpdateSleepTime,
    }

    impl UpdateModuleInterface for CountingUpdate {
        fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(self.sleep)
        }

        fn get_disabled_types_to_process(&self) -> DisabledMaskType {
            self.disabled
        }
    }

    fn counting_ptr(
        disabled: DisabledMaskType,
        sleep: UpdateSleepTime,
    ) -> (UpdateModulePtr, Arc<AtomicU32>) {
        let count = Arc::new(AtomicU32::new(0));
        let module: UpdateModulePtr = Arc::new(RwLock::new(CountingUpdate {
            count: Arc::clone(&count),
            disabled,
            sleep,
        }));
        (module, count)
    }

    fn insert_test_object(logic: &mut GameLogic, id: ObjectID) {
        let object = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
        logic.objects.insert(id, object);
    }

    #[test]
    fn friend_awaken_ignores_call_from_inside_current_module_update() {
        // C++ GameLogic.cpp:2965-2969 — setWakeFrame from inside update() is ignored.
        let mut logic = GameLogic::new();
        logic.frame = 4;
        let (module, _ticks) = counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::None);
        logic.register_sleepy_update_module(11, Arc::clone(&module), 10);

        let _guard = enter_cur_update_module(&module);
        logic.friend_awaken_update_module(&module, 4);
        drop(_guard);

        let (object_id, wake) = logic
            .sleepy_entry_for(&module)
            .expect("module stays on heap");
        assert_eq!(object_id, 11);
        assert_eq!(
            wake, 10,
            "inside-update awaken must not rewrite the wake frame"
        );
    }

    #[test]
    fn friend_awaken_keeps_already_awake_now_when_asked_now_plus_one() {
        // C++ GameLogic.cpp:2974-2982 — already awake at `now`; UPDATE_SLEEP_NONE
        // (now+1) must not defer this frame.
        let mut logic = GameLogic::new();
        logic.frame = 5;
        let (module, _ticks) = counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::None);
        logic.register_sleepy_update_module(22, Arc::clone(&module), 5);

        logic.friend_awaken_update_module(&module, 6);

        let (object_id, wake) = logic
            .sleepy_entry_for(&module)
            .expect("module stays on heap");
        assert_eq!(object_id, 22);
        assert_eq!(wake, 5);
        assert_eq!(logic.sleepy_update_count(), 1, "must not duplicate-push");
    }

    #[test]
    fn friend_awaken_same_frame_is_idempotent_and_keeps_object_id() {
        // C++ GameLogic.cpp:2971-2972 — already scheduled for this frame.
        // Pre-fix used entries.first() after retain, stealing another object's id.
        let mut logic = GameLogic::new();
        logic.frame = 3;
        let (first, _first_ticks) = counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::None);
        let (second, _second_ticks) =
            counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::None);
        logic.register_sleepy_update_module(100, Arc::clone(&first), 20);
        logic.register_sleepy_update_module(200, Arc::clone(&second), 30);

        logic.friend_awaken_update_module(&first, 20);
        logic.friend_awaken_update_module(&first, 8);

        assert_eq!(logic.sleepy_update_count(), 2, "no duplicate heap push");
        let (object_id, wake) = logic
            .sleepy_entry_for(&first)
            .expect("first module remains");
        assert_eq!(object_id, 100, "awaken must keep the owning object_id");
        assert_eq!(wake, 8);
        let (other_id, other_wake) = logic
            .sleepy_entry_for(&second)
            .expect("second module remains");
        assert_eq!(other_id, 200);
        assert_eq!(other_wake, 30);
    }

    #[test]
    fn friend_awaken_clamps_past_forever_and_keeps_module_in_heap() {
        // C++ UpdateModule.h friend_setNextCallFrame clamps to UPDATE_SLEEP_FOREVER.
        let mut logic = GameLogic::new();
        logic.frame = 2;
        let (module, _ticks) = counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::Forever);
        logic.register_sleepy_update_module(33, Arc::clone(&module), 9);
        logic.friend_awaken_update_module(&module, u32::MAX);
        let (_, wake) = logic.sleepy_entry_for(&module).expect("kept in heap");
        assert_eq!(wake, UPDATE_SLEEP_FOREVER_FRAMES);
    }

    #[test]
    fn disabled_gate_uses_any_intersection_not_subset() {
        // C++ GameLogic.cpp:3718-3720 — process if !dis.any() || any intersection.
        // Object EMP|HELD, module HELD: C++ runs; old Rust subset skipped EMP leftover.
        let mut logic = GameLogic::new();
        logic.frame = 4;
        insert_test_object(&mut logic, 44);
        {
            let mut guard = logic
                .objects
                .get(&44)
                .expect("test object inserted")
                .write()
                .expect("object lock");
            guard.set_disabled(DisabledType::DisabledEmp);
            guard.set_disabled(DisabledType::Held);
        }

        let (intersection, intersection_ticks) =
            counting_ptr(DisabledMaskType::HELD, UpdateSleepTime::Forever);
        logic.register_sleepy_update_module(44, Arc::clone(&intersection), 4);
        logic.process_sleepy_updates(4);
        assert_eq!(
            intersection_ticks.load(Ordering::SeqCst),
            1,
            "any intersection with disabledTypesToProcess must run the module"
        );

        let (no_overlap, no_overlap_ticks) = counting_ptr(
            DisabledMaskType::DISABLED_UNDERPOWERED,
            UpdateSleepTime::Forever,
        );
        logic.register_sleepy_update_module(44, Arc::clone(&no_overlap), 4);
        logic.process_sleepy_updates(4);
        assert_eq!(
            no_overlap_ticks.load(Ordering::SeqCst),
            0,
            "no intersection must skip the module"
        );

        insert_test_object(&mut logic, 45);
        let (always, always_ticks) =
            counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::Forever);
        logic.register_sleepy_update_module(45, Arc::clone(&always), 4);
        logic.process_sleepy_updates(4);
        assert_eq!(
            always_ticks.load(Ordering::SeqCst),
            1,
            "undisabled objects always process"
        );
    }

    #[test]
    fn register_wake_zero_is_now_and_ticks_same_frame() {
        // C++ GameLogic.cpp:3872-3899 — wake 0 → now (min 1), can tick this frame.
        let mut logic = GameLogic::new();
        logic.frame = 7;
        insert_test_object(&mut logic, 55);
        let (module, ticks) = counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::Forever);
        logic.register_sleepy_update_module(55, Arc::clone(&module), 0);

        let (_, wake) = logic.sleepy_entry_for(&module).expect("registered");
        assert_eq!(wake, 7, "wake 0 must become current frame, not now+1");

        logic.process_sleepy_updates(7);
        assert_eq!(
            ticks.load(Ordering::SeqCst),
            1,
            "new object module must be eligible on the registration frame"
        );
    }

    #[test]
    fn register_wake_zero_on_frame_zero_becomes_one() {
        let mut logic = GameLogic::new();
        assert_eq!(logic.frame, 0);
        let (module, _ticks) = counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::None);
        logic.register_sleepy_update_module(66, Arc::clone(&module), 0);
        let (_, wake) = logic.sleepy_entry_for(&module).expect("registered");
        assert_eq!(wake, 1);
    }
}

#[cfg(test)]
mod sleepy_context_tests;
