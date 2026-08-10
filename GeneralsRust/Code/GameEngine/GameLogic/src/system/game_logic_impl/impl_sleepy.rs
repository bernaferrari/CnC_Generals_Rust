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
                Some(mask) if mask.any() => {
                    let disallowed = mask & !module_disabled_mask;
                    !disallowed.any()
                }
                _ => true,
            };

            // Update the module and get next wake time
            // C++ lines 3717-3732: Check disabled flags and call update()
            let next_wake;
            if should_process {
                match entry.module.write() {
                    Ok(mut module) => match module.update() {
                        Ok(sleep_time) => {
                            processed += 1;
                            match sleep_time {
                                UpdateSleepTime::Forever => {
                                    next_wake = None;
                                }
                                UpdateSleepTime::None => {
                                    next_wake = Some(current_frame.saturating_add(1));
                                }
                                UpdateSleepTime::Frames(frames) => {
                                    let sleep_frames = frames.max(1);
                                    let wake = current_frame.saturating_add(sleep_frames);
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

        // Check if already at this wake frame
        let current_wake = module
            .read()
            .map(|m| m.get_update_phase())
            .unwrap_or(SleepyUpdatePhase::Normal);

        // Find the entry in the sleepy heap
        let idx = self
            .sleepy_updates
            .iter()
            .position(|e| Arc::ptr_eq(&e.module, module) && e.wake_frame == when_to_wake_up);

        if let Some(_) = idx {
            return;
        }

        // Remove old entry if present, then insert with new wake frame
        let old_idx = self
            .sleepy_updates
            .iter()
            .position(|e| Arc::ptr_eq(&e.module, module));

        if let Some(_remove_idx) = old_idx {
            let mut entries: Vec<_> = self.sleepy_updates.drain().collect();
            entries.retain(|e| !Arc::ptr_eq(&e.module, module));

            let object_id = entries.first().map(|e| e.object_id).unwrap_or(0);

            entries.push(SleepyUpdateEntry {
                wake_frame: when_to_wake_up,
                phase: current_wake,
                object_id,
                module: Arc::clone(module),
            });

            for entry in entries {
                self.sleepy_updates.push(entry);
            }
        } else {
            // Not in heap yet - check if we know the object
            let obj_id = self
                .module_lookup
                .iter()
                .find(|(_, mods)| mods.iter().any(|m| Arc::ptr_eq(m, module)))
                .map(|(id, _)| *id)
                .unwrap_or(0);

            if obj_id != 0 {
                self.sleepy_updates.push(SleepyUpdateEntry {
                    wake_frame: when_to_wake_up,
                    phase: current_wake,
                    object_id: obj_id,
                    module: Arc::clone(module),
                });
            }
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
                    Some(mask) if mask.any() => {
                        let disallowed = mask & !module_disabled_mask;
                        !disallowed.any()
                    }
                    _ => true,
                };
                if !should_process {
                    continue;
                }

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

        let wake = if wake_frame == 0 {
            self.frame.saturating_add(1)
        } else {
            wake_frame
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
    /// Adds a module to the sleepy heap. Wake frame defaults to next frame.
    pub fn push_sleepy_update(&mut self, object_id: ObjectID, module: UpdateModulePtr) {
        let wake_frame = self.frame.saturating_add(1);
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
