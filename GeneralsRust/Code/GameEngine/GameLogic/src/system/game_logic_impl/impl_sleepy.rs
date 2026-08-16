// C++ UpdateModule.h: UPDATE_SLEEP_FOREVER = 0x3fffffff (clamped so offsets cannot overflow).
const UPDATE_SLEEP_FOREVER_FRAMES: UnsignedInt = 0x3fff_ffff;

thread_local! {
    static CUR_UPDATE_MODULE: std::cell::RefCell<Option<UpdateModulePtr>> =
        const { std::cell::RefCell::new(None) };
}

struct CurUpdateModuleGuard;

impl Drop for CurUpdateModuleGuard {
    fn drop(&mut self) {
        CUR_UPDATE_MODULE.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

fn enter_cur_update_module(module: &UpdateModulePtr) -> CurUpdateModuleGuard {
    CUR_UPDATE_MODULE.with(|slot| {
        *slot.borrow_mut() = Some(Arc::clone(module));
    });
    CurUpdateModuleGuard
}

fn is_cur_update_module(module: &UpdateModulePtr) -> bool {
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

impl GameLogic {
    fn process_sleepy_updates(&mut self, current_frame: UnsignedInt) {
        let mut processed = 0usize;
        let mut requeue: Vec<SleepyUpdateEntry> = Vec::new();

        // C++ lines 3698-3713: While loop processes all ready updates
        while let Some(entry) = self.sleepy_updates.peek() {
            // C++ line 3710: Check if wake frame has arrived
            if entry.wake_frame > current_frame {
                // No more entries ready to wake
                break;
            }

            let mut entry = self
                .sleepy_updates
                .pop()
                .expect("Heap became empty after peek");

            let object_ref = match self.objects.get(&entry.object_id) {
                Some(obj) => obj.clone(),
                None => {
                    continue;
                }
            };

            let (module_disabled_mask, phase) = entry
                .module
                .read()
                .map(|module| {
                    (
                        module.get_disabled_types_to_process(),
                        module.get_update_phase(),
                    )
                })
                .unwrap_or((DisabledMaskType::empty(), SleepyUpdatePhase::Normal));

            let object_disabled = object_ref.read().ok().map(|obj| obj.get_disabled_flags());
            let should_process = match object_disabled {
                Some(mask) => disabled_module_should_process(mask, module_disabled_mask),
                None => true,
            };

            // Update the module and get next wake time
            // C++ lines 3717-3732: Check disabled flags and call update()
            let next_wake;
            if should_process {
                let _cur = enter_cur_update_module(&entry.module);
                match entry.module.write() {
                    Ok(mut module) => match module.update() {
                        Ok(sleep_time) => {
                            processed += 1;
                            match sleep_time {
                                UpdateSleepTime::Forever => {
                                    // C++ friend_setNextCallFrame(now + FOREVER) then clamp.
                                    next_wake = Some(UPDATE_SLEEP_FOREVER_FRAMES);
                                }
                                UpdateSleepTime::None => {
                                    next_wake = Some(current_frame.saturating_add(1));
                                }
                                UpdateSleepTime::Frames(frames) => {
                                    let sleep_frames = frames.max(1);
                                    let wake = current_frame
                                        .saturating_add(sleep_frames)
                                        .min(UPDATE_SLEEP_FOREVER_FRAMES);
                                    next_wake = Some(wake);
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Sleepy update module for object {} failed: {}",
                                entry.object_id, e
                            );
                            // Retry next frame
                            next_wake = Some(current_frame.saturating_add(1));
                        }
                    },
                    Err(_) => {
                        warn!(
                            "Sleepy update module lock poisoned for object {}",
                            entry.object_id
                        );
                        next_wake = Some(current_frame.saturating_add(1));
                    }
                }
            } else {
                next_wake = Some(current_frame.saturating_add(1));
            }

            // Requeue for next wake (C++ line 3735-3736)
            if let Some(wake_frame) = next_wake {
                entry.phase = entry
                    .module
                    .read()
                    .map(|module| module.get_update_phase())
                    .unwrap_or(phase);
                entry.wake_frame = wake_frame;
                requeue.push(entry);
            }
        }

        // Re-add entries back to heap (C++ line 3737: rebalanceSleepyUpdate)
        // BinaryHeap automatically maintains heap property on push
        for entry in requeue {
            self.sleepy_updates.push(entry);
        }
    }

    pub fn friend_awaken_update_module(
        &mut self,
        module: &UpdateModulePtr,
        when_to_wake_up: UnsignedInt,
    ) {
        let now = self.frame;
        if when_to_wake_up < now {
            warn!(
                "setWakeFrame frame {} is in the past (now={})",
                when_to_wake_up, now
            );
        }

        // C++ GameLogic.cpp:2965-2969 — ignore awaken from inside the current module update.
        if is_cur_update_module(module) {
            return;
        }

        // C++ UpdateModule.h friend_setNextCallFrame: anything > FOREVER is still FOREVER.
        let when_to_wake_up = when_to_wake_up.min(UPDATE_SLEEP_FOREVER_FRAMES);

        let existing = self
            .sleepy_updates
            .iter()
            .find(|entry| Arc::ptr_eq(&entry.module, module))
            .map(|entry| (entry.object_id, entry.wake_frame, entry.phase));

        if let Some((object_id, current_wake, phase)) = existing {
            // C++ GameLogic.cpp:2971-2972 — already scheduled for this frame.
            if current_wake == when_to_wake_up {
                return;
            }
            // C++ GameLogic.cpp:2974-2982 — already awake at `now`; UPDATE_SLEEP_NONE
            // (now+1) must not defer this frame.
            if now > 0 && current_wake == now && when_to_wake_up == now.saturating_add(1) {
                return;
            }

            let mut heap = BinaryHeap::new();
            while let Some(entry) = self.sleepy_updates.pop() {
                if !Arc::ptr_eq(&entry.module, module) {
                    heap.push(entry);
                }
            }
            heap.push(SleepyUpdateEntry {
                wake_frame: when_to_wake_up,
                phase,
                object_id,
                module: Arc::clone(module),
            });
            self.sleepy_updates = heap;
            return;
        }

        // C++ GameLogic.cpp:3018-3021: not yet in the heap (ctor / init). Remember the
        // requested wake only if we already know the owning object; never invent id 0.
        let obj_id = self
            .module_lookup
            .iter()
            .find(|(_, mods)| mods.iter().any(|m| Arc::ptr_eq(m, module)))
            .map(|(id, _)| *id);

        if let Some(object_id) = obj_id {
            let phase = module
                .read()
                .map(|m| m.get_update_phase())
                .unwrap_or(SleepyUpdatePhase::Normal);
            self.sleepy_updates.push(SleepyUpdateEntry {
                wake_frame: when_to_wake_up,
                phase,
                object_id,
                module: Arc::clone(module),
            });
        }
    }

    /// C++ parity: GameLogic::rebalanceSleepyUpdate() (GameLogic.cpp line 2881)
    ///
    /// The Rust BinaryHeap auto-rebalances on push/pop, so this is a no-op
    /// that exists for API parity with C++.
    pub fn rebalance_sleepy_update(&mut self, _index: usize) {
        // BinaryHeap auto-rebalances; no manual work needed
    }

    /// C++ parity: GameLogic::rebalanceParentSleepyUpdate() (GameLogic.cpp line 2773)
    ///
    /// In C++, this bubbles an element up the heap. BinaryHeap handles this
    /// automatically, so this is a no-op for parity.
    pub fn rebalance_parent_sleepy_update(&mut self, _index: usize) -> usize {
        0
    }

    /// C++ parity: GameLogic::rebalanceChildSleepyUpdate() (GameLogic.cpp line 2799)
    ///
    /// In C++, this sifts an element down the heap. BinaryHeap handles this
    /// automatically, so this is a no-op for parity.
    pub fn rebalance_child_sleepy_update(&mut self, _index: usize) -> usize {
        0
    }

    /// C++ parity: GameLogic::validateSleepyUpdate() (GameLogic.cpp line 2693)
    ///
    /// Debug validation of the sleepy update heap. In C++ this checks parent/child
    /// priority ordering and index consistency. In Rust, BinaryHeap maintains
    /// invariants automatically.
    #[cfg(debug_assertions)]
    pub fn validate_sleepy_update(&self) {
        // BinaryHeap maintains its own invariants
    }

    #[cfg(not(debug_assertions))]
    pub fn validate_sleepy_update(&self) {}

    /// Process normal (every-frame) update modules
    fn process_normal_updates(&mut self) {
        let phases = [
            SleepyUpdatePhase::Initial,
            SleepyUpdatePhase::Physics,
            SleepyUpdatePhase::Normal,
            SleepyUpdatePhase::Final,
        ];

        for phase in phases {
            for entry in &self.normal_updates {
                let object_ref = match self.objects.get(&entry.object_id) {
                    Some(obj) => obj.clone(),
                    None => continue,
                };

                let (module_disabled_mask, module_phase) = entry
                    .module
                    .read()
                    .map(|module| {
                        (
                            module.get_disabled_types_to_process(),
                            module.get_update_phase(),
                        )
                    })
                    .unwrap_or((DisabledMaskType::empty(), SleepyUpdatePhase::Normal));
                if module_phase != phase {
                    continue;
                }

                let object_disabled = object_ref.read().ok().map(|obj| obj.get_disabled_flags());
                let should_process = match object_disabled {
                    Some(mask) => disabled_module_should_process(mask, module_disabled_mask),
                    None => true,
                };
                if !should_process {
                    continue;
                }

                let _cur = enter_cur_update_module(&entry.module);
                if let Ok(mut module) = entry.module.write() {
                    match module.update() {
                        Ok(UpdateSleepTime::None) => {}
                        Ok(other) => {
                            warn!(
                                "Normal update module for object {} returned sleep {:?}",
                                entry.object_id, other
                            );
                        }
                        Err(e) => {
                            warn!(
                                "Normal update module for object {} failed: {}",
                                entry.object_id, e
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
        let entry = self.module_lookup.entry(object_id).or_insert_with(Vec::new);
        entry.retain(|existing| !Arc::ptr_eq(existing, &module));
        entry.push(module.clone());

        self.normal_updates
            .retain(|tracked| !Arc::ptr_eq(&tracked.module, &module));
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
        let entry = self.module_lookup.entry(object_id).or_insert_with(Vec::new);
        entry.retain(|existing| !Arc::ptr_eq(existing, &module));
        entry.push(module.clone());

        // C++ GameLogic.cpp:3872-3899 — wake 0 (ctor never called setWakeFrame) becomes
        // the current frame, or 1 when the world is still on frame 0, so the module
        // can tick in the same frame it was registered.
        let wake = if wake_frame == 0 {
            self.frame.max(1)
        } else {
            wake_frame.min(UPDATE_SLEEP_FOREVER_FRAMES)
        };

        // Remove existing entry if present
        if !self.sleepy_updates.is_empty() {
            let mut heap = BinaryHeap::new();
            while let Some(entry) = self.sleepy_updates.pop() {
                if !Arc::ptr_eq(&entry.module, &module) {
                    heap.push(entry);
                }
            }
            self.sleepy_updates = heap;
        }

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

    /// Unregister an update module
    pub fn unregister_update_module(&mut self, object_id: ObjectID, module: UpdateModulePtr) {
        self.normal_updates
            .retain(|entry| !Arc::ptr_eq(&entry.module, &module));

        if !self.sleepy_updates.is_empty() {
            let mut heap = BinaryHeap::new();
            while let Some(entry) = self.sleepy_updates.pop() {
                if !Arc::ptr_eq(&entry.module, &module) {
                    heap.push(entry);
                }
            }
            self.sleepy_updates = heap;
        }

        if let Some(list) = self.module_lookup.get_mut(&object_id) {
            list.retain(|existing| !Arc::ptr_eq(existing, &module));
            if list.is_empty() {
                self.module_lookup.remove(&object_id);
            }
        }
    }

    /// Remove all update modules for an object
    fn remove_updates_for_object(&mut self, object_id: ObjectID) {
        if let Some(entries) = self.module_lookup.remove(&object_id) {
            self.normal_updates.retain(|tracked| {
                !entries
                    .iter()
                    .any(|registered| Arc::ptr_eq(registered, &tracked.module))
            });
        }

        if !self.sleepy_updates.is_empty() {
            let mut heap = BinaryHeap::new();
            while let Some(entry) = self.sleepy_updates.pop() {
                if entry.object_id != object_id {
                    heap.push(entry);
                }
            }
            self.sleepy_updates = heap;
        }
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
        self.sleepy_updates.pop()
    }

    /// PARITY_NOTE: GameLogic::peekSleepyUpdate() C++ line 2920.
    pub fn peek_sleepy_update(&self) -> Option<&SleepyUpdateEntry> {
        self.sleepy_updates.peek()
    }

    /// PARITY_NOTE: GameLogic::eraseSleepyUpdate(Int i) C++ line 2737.
    pub fn erase_sleepy_update(&mut self, target_module: &UpdateModulePtr) {
        let mut heap = BinaryHeap::new();
        while let Some(entry) = self.sleepy_updates.pop() {
            if !Arc::ptr_eq(&entry.module, target_module) {
                heap.push(entry);
            }
        }
        self.sleepy_updates = heap;
    }

    /// PARITY_NOTE: GameLogic::remakeSleepyUpdate() C++ line 2890.
    pub fn remake_sleepy_update(&mut self) {
        let entries: Vec<SleepyUpdateEntry> = self.sleepy_updates.drain().collect();
        for entry in entries {
            self.sleepy_updates.push(entry);
        }
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
    fn sleepy_entry_for(
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
        fn update(
            &mut self,
        ) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
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

    fn insert_test_object(logic: &mut GameLogic, id: ObjectID) -> Arc<RwLock<Object>> {
        let object = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
        logic.objects.insert(id, Arc::clone(&object));
        object
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
        assert_eq!(wake, 10, "inside-update awaken must not rewrite the wake frame");
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
        let (second, _second_ticks) = counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::None);
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
        let object = insert_test_object(&mut logic, 44);
        {
            let mut guard = object.write().expect("object lock");
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

        let enabled = insert_test_object(&mut logic, 45);
        let _ = enabled;
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
