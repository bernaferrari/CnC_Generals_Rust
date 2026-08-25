//! Split-out inherent `disabled state, upgrades, production, construction` methods for [`Object`].
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// Set the disabled/held state for this object
    /// Used by containment modules to disable contained units
    pub fn set_disabled_held(
        &mut self,
        held: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let holder_id = self.contained_by_id;

        if self.held_helper.is_none() {
            self.held_helper = Some(Arc::new(Mutex::new(ObjectHeldHelper::new())));
        }

        if let Some(helper) = &self.held_helper {
            if let Ok(mut guard) = helper.lock() {
                guard.set_held(held, holder_id);
            }
        }

        if held {
            self.set_disabled(DisabledType::Held);
        } else {
            self.clear_disabled(DisabledType::Held);
        }

        if held {
            self.set_status(ObjectStatusMaskType::UNSELECTABLE, true);
        } else {
            self.clear_status(ObjectStatusMaskType::UNSELECTABLE);
        }
        Ok(())
    }

    // Disabled state management
    pub fn get_disabled_flags(&self) -> DisabledMaskType {
        self.disabled_mask
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled_mask.any()
    }

    pub fn is_disabled_by_type(&self, disabled_type: DisabledType) -> bool {
        self.disabled_mask.test(disabled_type)
    }

    pub fn set_disabled(&mut self, disabled_type: DisabledType) {
        let was_disabled = self.is_disabled();
        self.disabled_mask.set_disabled(disabled_type);
        if !was_disabled && self.is_disabled() {
            self.on_disabled_edge(true);
        }
    }

    /// Whether the object is currently invulnerable.
    pub fn is_invulnerable(&mut self) -> bool {
        if self.invulnerable_until_frame == 0 {
            return false;
        }
        let now = crate::helpers::TheGameLogic::get_frame();
        if now >= self.invulnerable_until_frame {
            self.invulnerable_until_frame = 0;
            return false;
        }
        true
    }

    pub fn clear_disabled(&mut self, disabled_type: DisabledType) -> bool {
        if !self.is_disabled_by_type(disabled_type) {
            return false;
        }

        // C++ Object.cpp lines 2203-2239: Play audio feedback for re-enabled structures/vehicles
        // Only play audio for specific disable types that affect power/EMP/subdued/hacked
        if matches!(
            disabled_type,
            DisabledType::DisabledUnderpowered
                | DisabledType::DisabledEmp
                | DisabledType::DisabledSubdued
                | DisabledType::DisabledHacked
        ) {
            let any_power_disable_remaining = [
                DisabledType::DisabledUnderpowered,
                DisabledType::DisabledEmp,
                DisabledType::DisabledSubdued,
                DisabledType::DisabledHacked,
            ]
            .into_iter()
            .any(|other_type| other_type != disabled_type && self.is_disabled_by_type(other_type));

            if !any_power_disable_remaining {
                // Play appropriate audio event for re-enabled object
                if let Some(audio) = crate::helpers::TheAudio::get() {
                    if let Some(misc_audio) =
                        game_engine::common::ini::ini_misc_audio::get_misc_audio()
                    {
                        let misc_audio = misc_audio.read();
                        let sound_name = if self.is_kind_of(KindOf::Structure) {
                            misc_audio
                                .building_reenabled
                                .playable_event_name()
                                .to_string()
                        } else if self.is_kind_of(KindOf::Vehicle) {
                            misc_audio
                                .vehicle_reenabled
                                .playable_event_name()
                                .to_string()
                        } else {
                            String::new()
                        };

                        if !sound_name.is_empty() {
                            let mut event =
                                crate::object::special_power_template::AudioEventRts::new(
                                    sound_name,
                                );
                            let pos = self.get_position();
                            event.set_position(&(pos.x, pos.y, pos.z));
                            audio.add_audio_event(&event);
                        }
                    }
                }
            }
        }

        // C++ Object.cpp line 2253-2257: HELD never pauses special powers, other types do
        if disabled_type != DisabledType::Held && self.is_disabled_by_type(disabled_type) {
            self.pause_all_special_powers(false); // unpause = false means decrement pause count
        }

        // Handle contained rider disable state propagation (C++ lines 2259-2268)
        if let Some(contain) = &self.contain {
            if let Ok(contain_guard) = contain.lock() {
                if let Some(rider_id) = contain_guard.get_rider_id() {
                    if let Some(rider) = crate::helpers::TheGameLogic::find_object_by_id(rider_id) {
                        if let Ok(mut rider_guard) = rider.write() {
                            // If this was a FOREVER disable, clear the rider's matching disable
                            if let Some(index) = self.get_disabled_type_index(disabled_type) {
                                if self.disabled_till_frame[index] == FOREVER {
                                    let _ = rider_guard.clear_disabled(disabled_type);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Handle spawns-as-weapons objects (C++ lines 2270-2280)
        if self.is_kind_of(KindOf::SpawnsAreTheWeapons) {
            self.order_spawn_slaves_to_clear_disabled(disabled_type);
        }

        let was_disabled_type = self.is_disabled_by_type(disabled_type);
        let was_disabled = self.is_disabled();
        self.disabled_mask.clear(disabled_type);
        if let Some(index) = self.get_disabled_type_index(disabled_type) {
            self.disabled_till_frame[index] = NEVER;
        }

        // C++ lines 2288-2296: Clear tint status if no longer disabled by non-exception types
        let flags_minus_exceptions = Self::flags_requiring_disabled_tint(self.disabled_mask);
        if flags_minus_exceptions.is_empty() {
            if let Some(drawable) = &self.drawable {
                if let Ok(mut draw_guard) = drawable.write() {
                    draw_guard.clear_tint_status(crate::object::drawable::TintStatus::DISABLED);
                }
            }
        }

        // C++ line 2299: check disabled status for edge detection
        self.check_disabled_status();

        // C++ lines 2302-2304: if no longer disabled at all, call edge function
        if was_disabled && !self.is_disabled() {
            self.on_disabled_edge(false);
        }

        was_disabled_type
    }

    /// Friend access to a typed module by NameKeyType.
    /// Mirrors C++ `Object::findModule(key)` followed by a static_cast to the
    /// requested type. Searches behaviors first (matching C++ order), then the
    /// module entries list.
    ///
    /// Uses a closure because the underlying module is behind a `Mutex<Box<dyn Module>>`
    /// and a direct reference cannot outlive the guard.
    ///
    /// # Type Parameters
    /// * `T` - The concrete module type to retrieve
    ///
    /// # Example
    /// ```ignore
    /// // C++: auto* topple = (ToppleUpdate*)findModule(key_ToppleUpdate);
    /// obj.with_friend_module::<ToppleUpdateModule, _, _>(key_ToppleUpdate, |t| {
    ///     t.apply_toppling_force(...);
    /// });
    /// ```
    pub fn with_friend_module<T: 'static, F, R>(&self, key: NameKeyType, func: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut func = Some(func);

        for behavior_arc in &self.behaviors {
            let Ok(mut guard) = behavior_arc.lock() else {
                continue;
            };
            if guard.get_module_name_key() == key {
                if let Some(f) = func.take() {
                    return (&mut *guard as &mut dyn std::any::Any)
                        .downcast_mut::<T>()
                        .map(f);
                }
            }
        }

        for entry in &self.modules {
            let result = entry.with_module(|module| {
                if module.get_module_name_key() == key {
                    if let Some(f) = func.take() {
                        return (module as &mut dyn std::any::Any)
                            .downcast_mut::<T>()
                            .map(f);
                    }
                }
                None
            });
            if result.is_some() {
                return result;
            }
        }

        None
    }

    /// Friend access to a typed module by name string.
    /// Convenience wrapper that resolves the NameKeyType internally.
    pub fn with_friend_module_by_name<T: 'static, F, R>(
        &self,
        module_name: &str,
        func: F,
    ) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let key = crate::common::name_key_generate(module_name);
        self.with_friend_module(key, func)
    }

    /// Get the spawn behavior interface if this object has one.
    /// Used for handling spawns-as-weapons disable propagation.
    #[allow(dead_code)]
    pub(super) fn get_spawn_behavior_interface(
        &self,
    ) -> Option<Arc<Mutex<dyn SpawnBehaviorInterface>>> {
        None // Placeholder - spawn behavior is accessed directly through behaviors
    }

    /// Call order_slaves_to_clear_disabled on any spawn behavior modules
    pub(super) fn order_spawn_slaves_to_clear_disabled(&mut self, disabled_type: DisabledType) {
        for behavior in &self.behaviors {
            if let Ok(mut guard) = behavior.lock() {
                if let Some(spawn) = guard.get_spawn_behavior_interface() {
                    let _ = spawn.order_slaves_to_clear_disabled(disabled_type);
                    return;
                }
            }
        }
    }

    pub(super) fn on_disabled_edge(&mut self, becoming_disabled: bool) {
        self.cancel_dozer_task_on_disabled_edge(becoming_disabled);
        for behavior in &self.behaviors {
            if let Ok(mut guard) = behavior.lock() {
                guard.on_disabled_edge(becoming_disabled);
            }
        }

        let Some(player) = self.get_controlling_player() else {
            return;
        };

        let mut radar_disable_proof: Option<bool> = None;
        let mut power_bonus_applied = false;
        for entry in &self.upgrade_module_handles {
            entry.with_module(|module| {
                if let Some(radar) = module
                    .as_any()
                    .downcast_ref::<crate::object::upgrade::radar_upgrade::RadarUpgrade>()
                {
                    if radar.is_applied() {
                        radar_disable_proof = Some(radar.is_disable_proof());
                    }
                } else if let Some(power_plant) = module
                    .as_any()
                    .downcast_ref::<crate::object::upgrade::power_plant_upgrade::PowerPlantUpgrade>()
                {
                    if power_plant.is_applied() {
                        power_bonus_applied = true;
                    }
                }
            });
        }

        if let Some(disable_proof) = radar_disable_proof {
            if let Ok(mut player_guard) = player.write() {
                if becoming_disabled {
                    player_guard.remove_radar(disable_proof);
                } else {
                    player_guard.add_radar(disable_proof);
                }
            }
        }

        let mut power_to_adjust = self.get_template().get_energy_production();
        if power_to_adjust > 0 {
            let energy_bonus = self.get_template().get_energy_bonus();
            if energy_bonus != 0 {
                if power_bonus_applied {
                    power_to_adjust += energy_bonus;
                }
                for entry in &self.modules {
                    let is_overcharge_active = entry.with_module(|module| {
                        module_behavior_utility_kind(module)
                            .and_then(BehaviorUtilityModuleKindMut::overcharge_active)
                            .unwrap_or(false)
                    });
                    if is_overcharge_active {
                        power_to_adjust += energy_bonus;
                        break;
                    }
                }
                for behavior in &self.behaviors {
                    if let Ok(guard) = behavior.lock() {
                        if let Some(overcharge) = guard
                            .as_any()
                            .downcast_ref::<crate::object::behavior::overcharge_behavior::OverchargeBehavior>()
                        {
                            if overcharge.is_overcharge_active() {
                                power_to_adjust += energy_bonus;
                            }
                            break;
                        }
                    }
                }
            }
            if let Ok(mut player_guard) = player.write() {
                player_guard.adjust_power(power_to_adjust, !becoming_disabled);
            }
        }
    }

    /// Adjust power influence for the controlling player.
    /// Mirrors C++ Object::friend_adjustPowerForPlayer.
    pub fn adjust_power_for_player(&self, enable: bool) {
        let power = self.get_template().get_energy_production();
        if power == 0 {
            return;
        }
        let Some(player) = self.get_controlling_player() else {
            return;
        };
        let Ok(mut player_guard) = player.write() else {
            return;
        };

        if power > 0 {
            if self.is_disabled() {
                return;
            }
            if enable {
                player_guard.object_entering_influence(self);
            } else {
                player_guard.object_leaving_influence(self);
            }
        } else {
            let delta = power.abs();
            if enable {
                player_guard.add_power_consumption(delta);
            } else {
                player_guard.add_power_consumption(-delta);
            }
        }
    }

    pub fn check_disabled_status(&mut self) {
        // Check timers and clear expired disabled states
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        for i in 0..DISABLED_COUNT {
            if self.disabled_till_frame[i] != NEVER {
                if current_frame >= self.disabled_till_frame[i] {
                    if let Some(disabled_type) = disabled_type_from_index(i) {
                        self.clear_disabled(disabled_type);
                    } else {
                        self.disabled_till_frame[i] = NEVER;
                    }
                }
            }
        }
    }

    // Upgrade management
    pub fn has_upgrade(&self, upgrade_template: &UpgradeTemplate) -> bool {
        let mask = upgrade_template.mask();
        if mask.is_empty() {
            return false;
        }
        // Convert UpgradeMask to UpgradeMaskType
        let mask_bits = UpgradeMaskType::from_bits_retain(mask.bits());
        self.object_upgrades_completed.contains(mask_bits)
    }

    /// C++ `Object::affectedByUpgrade` (`Object.cpp:4444-4469`).
    /// Combine player|object completed masks with this upgrade, then ask
    /// each upgrade module `wouldUpgrade` (`can_upgrade`).
    pub fn affected_by_upgrade(&self, upgrade_template: &UpgradeTemplate) -> bool {
        let mut mask_to_check = UpgradeMaskType::none();
        if let Some(player) = self.get_controlling_player() {
            if let Ok(player) = player.read() {
                mask_to_check = player.get_completed_upgrade_mask();
            }
        }
        mask_to_check = mask_to_check | self.object_upgrades_completed;
        let upgrade_bits = UpgradeMaskType::from_bits_retain(upgrade_template.mask().bits());
        mask_to_check = mask_to_check | upgrade_bits;
        if mask_to_check.is_empty() {
            return false;
        }

        let mut would = false;
        for entry in &self.upgrade_module_handles {
            entry.with_module(|module| {
                if let Some(upgrade) = module_upgrade_kind(module) {
                    if upgrade.into_interface().can_upgrade(mask_to_check) {
                        would = true;
                    }
                }
            });
            if would {
                return true;
            }
        }

        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if let Some(upgrade) = guard.get_upgrade() {
                if upgrade.can_upgrade(mask_to_check) {
                    return true;
                }
            }
        }
        false
    }

    pub fn completed_upgrades(&self) -> UpgradeMaskType {
        self.object_upgrades_completed
    }

    pub fn give_upgrade(&mut self, upgrade_template: &UpgradeTemplate) {
        let mask = upgrade_template.mask();
        if mask.is_empty() {
            return;
        }

        // Convert UpgradeMask to UpgradeMaskType
        let mask_bits = UpgradeMaskType::from_bits_retain(mask.bits());

        if !upgrade_template.is_stackable() && self.object_upgrades_completed.contains(mask_bits) {
            return;
        }

        self.object_upgrades_completed.insert(mask_bits);
        self.apply_upgrade_modules(mask_bits);
        crate::object::upgrade::status_bits_upgrade::apply_registered_status_upgrades(self);
    }

    /// Apply any active player upgrades that should affect this object.
    /// Mirrors C++ Object::updateUpgradeModules() after construction finishes.
    pub fn update_upgrade_modules_from_player(&mut self) {
        if self.is_under_construction() {
            return;
        }
        if self.is_destroyed() {
            return;
        }
        let Some(player) = self.get_controlling_player() else {
            return;
        };
        let Ok(player_guard) = player.read() else {
            return;
        };
        let Some(manager) = player_guard.get_upgrade_manager() else {
            return;
        };
        let active_mask = manager.get_active_upgrades();
        let active_bits = UpgradeMaskType::from_bits_retain(active_mask.bits());
        // C++ Object.cpp:2421-2436 — `maskToCheck = player | object` is only
        // the argument to `attemptUpgrade`. Never write player bits into
        // `m_objectUpgradesCompleted`.
        let combined_bits = active_bits | self.object_upgrades_completed;
        self.apply_upgrade_modules(combined_bits);
        crate::object::upgrade::status_bits_upgrade::apply_registered_status_upgrades(self);
    }

    pub fn remove_upgrade(&mut self, upgrade_template: &UpgradeTemplate) {
        let mask = upgrade_template.mask();
        // Convert UpgradeMask to UpgradeMaskType
        let mask_bits = UpgradeMaskType::from_bits_retain(mask.bits());
        self.remove_upgrade_mask(mask_bits);
    }

    pub fn remove_upgrade_mask(&mut self, mask: UpgradeMaskType) {
        if mask.is_empty() {
            return;
        }
        // C++ Object.cpp:4491-4503 always clears the object bit (no-op if
        // unset) then `resetUpgrade` on every upgrade module.
        self.object_upgrades_completed.remove(mask);

        let mut matched_any = false;
        for entry in &self.upgrade_module_handles {
            let matched_any_ref = &mut matched_any;
            entry.with_module(|module| {
                if let Some(upgrade) = module_upgrade_kind(module) {
                    *matched_any_ref = true;
                    upgrade.into_interface().remove_upgrade(mask);
                }
            });
        }

        if !matched_any {
            // Convert UpgradeMaskType to UpgradeMask for notify
            let upgrade_mask = crate::upgrade::UpgradeMask::from_bits_retain(mask.bits());
            self.notify_upgrade_removed_internal(upgrade_mask);
        }
        crate::object::upgrade::status_bits_upgrade::apply_registered_status_upgrades(self);
    }

    pub(super) fn collect_upgrade_modules(&self) -> Vec<UpgradeModuleHandle> {
        let mut modules = Vec::new();
        if self.id != INVALID_ID {
            for handle in StatusBitsUpgradeHandle::for_object(self.id) {
                modules.push(UpgradeModuleHandle::StatusBits(handle));
            }
            for handle in PassengersFireUpgradeHandle::for_object(self.id) {
                modules.push(UpgradeModuleHandle::PassengersFire(handle));
            }
            for handle in SubObjectsUpgradeHandle::for_object(self.id) {
                modules.push(UpgradeModuleHandle::SubObjects(handle));
            }
        }
        modules
    }

    pub(super) fn apply_upgrade_modules(&mut self, mask: UpgradeMaskType) {
        if mask.is_empty() {
            return;
        }
        let mut matched_any = false;
        for entry in &self.upgrade_module_handles {
            let matched_any_ref = &mut matched_any;
            entry.with_module(|module| {
                if let Some(upgrade) = module_upgrade_kind(module) {
                    let upgrade = upgrade.into_interface();
                    *matched_any_ref = true;
                    if upgrade.can_upgrade(mask) {
                        let _ = upgrade.apply_upgrade(mask);
                    }
                }
            });
        }

        if !matched_any {
            let modules = self.collect_upgrade_modules();
            for module in modules {
                match module {
                    UpgradeModuleHandle::StatusBits(handle) => {
                        let _ = handle.apply(mask);
                    }
                    UpgradeModuleHandle::PassengersFire(handle) => {
                        let _ = handle.apply(mask);
                    }
                    UpgradeModuleHandle::SubObjects(handle) => {
                        let _ = handle.apply(mask);
                    }
                }
            }
        }
    }

    pub(super) fn notify_upgrade_removed_internal(&mut self, mask: crate::upgrade::UpgradeMask) {
        if mask.is_empty() {
            return;
        }

        // Convert UpgradeMask to UpgradeMaskType for module operations
        let mask_bits = UpgradeMaskType::from_bits_retain(mask.bits());
        for module in self.collect_upgrade_modules() {
            match module {
                UpgradeModuleHandle::StatusBits(handle) => handle.remove(mask_bits),
                UpgradeModuleHandle::PassengersFire(handle) => handle.remove(mask_bits),
                UpgradeModuleHandle::SubObjects(handle) => handle.remove(mask_bits),
            }
        }
    }

    pub(super) fn get_disabled_type_index(&self, _disabled_type: DisabledType) -> Option<usize> {
        // Convert disabled type to an index in `disabled_till_frame`.
        //
        // The C++ engine stores per-disabled-type expiration frames (see Object.cpp).
        // We keep a fixed array for parity; the mapping needs to remain stable.
        let index = match _disabled_type {
            DisabledType::DisabledDefault => 0,
            DisabledType::DisabledHacked => 1,
            DisabledType::DisabledEmp => 2,
            DisabledType::Held => 3,
            DisabledType::Paralyzed => 4,
            DisabledType::DisabledUnmanned | DisabledType::Unmanned => 5,
            DisabledType::DisabledUnderpowered => 6,
            DisabledType::DisabledFreefall => 7,
            DisabledType::DisabledAwestruck => 8,
            DisabledType::DisabledBrainwashed => 9,
            DisabledType::DisabledSubdued => 10,
            DisabledType::DisabledScriptDisabled => 11,
            DisabledType::DisabledScriptUnderpowered => 12,
            DisabledType::DisabledAny => return None,
        };

        if index < DISABLED_COUNT {
            Some(index)
        } else {
            None
        }
    }

    /// C++ `Object::getConstructionPercent()` — raw `Real`, unclamped.
    /// Completed buildings store `CONSTRUCTION_COMPLETE` (`-1`); selling can go to `-50`.
    pub fn get_construction_percent(&self) -> Real {
        self.construction_percent
    }

    /// C++ `Object::setConstructionPercent(Real)` — stores the raw value with no 0..100 clamp.
    pub fn set_construction_percent(&mut self, percent: f32) {
        self.construction_percent = percent;
        // C++ completed = -1; selling drives below 0. `pct >= 0` is still under construction.
        let under_construction = self.construction_percent >= 0.0;
        self.set_status(
            ObjectStatusMaskType::from(ObjectStatusTypes::UnderConstruction),
            under_construction,
        );

        let mut clear_flags = ModelConditionFlags::AWAITING_CONSTRUCTION
            | ModelConditionFlags::PARTIALLY_CONSTRUCTED
            | ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED
            | ModelConditionFlags::CONSTRUCTION_COMPLETE;

        let mut set_flags = ModelConditionFlags::empty();
        if !under_construction {
            set_flags |= ModelConditionFlags::CONSTRUCTION_COMPLETE;
        } else if self.construction_percent <= 0.0 {
            set_flags |= ModelConditionFlags::AWAITING_CONSTRUCTION;
        } else {
            let mut active_builder = false;
            if self.builder_id != INVALID_ID {
                if let Some(builder) =
                    crate::helpers::TheGameLogic::find_object_by_id(self.builder_id)
                {
                    active_builder = builder
                        .read()
                        .map(|guard| guard.is_alive())
                        .unwrap_or(false);
                }
            }
            if active_builder {
                set_flags |= ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED;
            } else {
                set_flags |= ModelConditionFlags::PARTIALLY_CONSTRUCTED;
            }
        }

        clear_flags.remove(set_flags);
        if let Err(err) = self.clear_and_set_model_condition_flags(clear_flags, set_flags) {
            log::debug!("Object::update_construction_model_condition_flags failed: {err}");
        }
    }

    /// True while construction percent is `>= 0`. Completed is `-1`; selling is negative.
    pub fn is_under_construction(&self) -> bool {
        self.construction_percent >= 0.0
    }

    /// C++ parity: Object::hasProductionInQueue()
    pub fn has_production_in_queue(&self) -> bool {
        self.get_contain()
            .map(|c| {
                c.lock()
                    .map(|guard| guard.get_contain_count() > 0)
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    pub(super) fn queue_unit_via_production(
        &self,
        template: &Arc<dyn crate::common::ThingTemplate>,
    ) -> bool {
        let template_name = template.get_name().to_string();
        let player_id = self.get_controlling_player_id().unwrap_or(0) as ObjectID;
        let build_cost = template.calc_cost_to_build(None);
        let build_time = template.calc_time_to_build(None).max(0) as u32;

        for entry in &self.modules {
            let queued = entry.with_module(|module| {
                module_production_queue_kind(module).map(|kind| {
                    kind.queue_unit(template_name.clone(), build_cost, build_time, player_id)
                })
            });

            if let Some(result) = queued {
                return result;
            }
        }

        for behavior in &self.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };

            if let Some(kind) = behavior_production_queue_kind(&mut *behavior_guard) {
                return kind.queue_unit(template_name.clone(), build_cost, build_time, player_id);
            }

            if let Some(prod) = behavior_guard.get_production_update_interface() {
                return prod
                    .start_production(template_name.clone(), player_id)
                    .is_ok();
            }
        }

        false
    }

    pub(super) fn queue_unit_via_production_id(
        &self,
        template: &Arc<dyn crate::common::ThingTemplate>,
        production_id: u32,
    ) -> bool {
        let template_name = template.get_name().to_string();
        let player_id = self.get_controlling_player_id().unwrap_or(0) as ObjectID;
        let build_cost = template.calc_cost_to_build(None);
        let build_time = template.calc_time_to_build(None).max(0) as u32;

        for entry in &self.modules {
            let queued = entry.with_module(|module| {
                module_production_queue_kind(module).map(|kind| {
                    kind.queue_unit_with_production_id(
                        template_name.clone(),
                        build_cost,
                        build_time,
                        player_id,
                        production_id,
                    )
                })
            });

            if let Some(result) = queued {
                return result;
            }
        }

        for behavior in &self.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };

            if let Some(kind) = behavior_production_queue_kind(&mut *behavior_guard) {
                return kind.queue_unit_with_production_id(
                    template_name.clone(),
                    build_cost,
                    build_time,
                    player_id,
                    production_id,
                );
            }
        }

        self.queue_unit_via_production(template)
    }

    pub(super) fn queue_upgrade_via_production(&self, upgrade: &Arc<UpgradeTemplate>) -> bool {
        let upgrade_name = upgrade.get_name().to_string();
        let player_id = self.get_controlling_player_id().unwrap_or(0) as ObjectID;
        let (build_cost, build_time) = if let Some(player_arc) = self.get_controlling_player() {
            if let Ok(player_guard) = player_arc.read() {
                (
                    upgrade.calc_cost_to_build(&player_guard),
                    upgrade.calc_time_to_build(&player_guard).max(0) as u32,
                )
            } else {
                (
                    upgrade.get_cost(),
                    (upgrade.get_build_time() * LOGICFRAMES_PER_SECOND as f32).max(0.0) as u32,
                )
            }
        } else {
            (
                upgrade.get_cost(),
                (upgrade.get_build_time() * LOGICFRAMES_PER_SECOND as f32).max(0.0) as u32,
            )
        };

        for entry in &self.modules {
            let queued = entry.with_module(|module| {
                module_production_queue_kind(module).map(|kind| {
                    kind.queue_upgrade(upgrade_name.clone(), build_cost, build_time, player_id)
                })
            });

            if let Some(result) = queued {
                return result;
            }
        }

        for behavior in &self.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };

            if let Some(kind) = behavior_production_queue_kind(&mut *behavior_guard) {
                return kind.queue_upgrade(upgrade_name.clone(), build_cost, build_time, player_id);
            }

            if let Some(prod) = behavior_guard.get_production_update_interface() {
                return prod
                    .start_production(upgrade_name.clone(), player_id)
                    .is_ok();
            }
        }

        false
    }

    pub(super) fn cancel_upgrade_via_production(&self, upgrade: &Arc<UpgradeTemplate>) -> bool {
        let upgrade_name = upgrade.get_name().to_string();

        for entry in &self.modules {
            let canceled = entry.with_module(|module| {
                module_production_queue_kind(module).and_then(|kind| {
                    if kind.cancel_upgrade(&upgrade_name) {
                        Some(())
                    } else {
                        None
                    }
                })
            });

            if canceled.is_some() {
                return true;
            }
        }

        for behavior in &self.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };

            if let Some(kind) = behavior_production_queue_kind(&mut *behavior_guard) {
                return kind.cancel_upgrade(&upgrade_name);
            }

            if let Some(prod) = behavior_guard.get_production_update_interface() {
                if prod.cancel_production(0).is_ok() {
                    return true;
                }
            }
        }

        false
    }

    pub(super) fn cancel_unit_via_production_id(&self, production_id: u32) -> bool {
        for entry in &self.modules {
            let canceled = entry.with_module(|module| {
                module_production_queue_kind(module).and_then(|kind| {
                    if kind.cancel_unit_by_production_id(production_id) {
                        Some(())
                    } else {
                        None
                    }
                })
            });

            if canceled.is_some() {
                return true;
            }
        }

        for behavior in &self.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };

            if let Some(kind) = behavior_production_queue_kind(&mut *behavior_guard) {
                return kind.cancel_unit_by_production_id(production_id);
            }
        }

        false
    }

    pub(super) fn cancel_unit_via_template(
        &self,
        template: &Arc<dyn crate::common::ThingTemplate>,
    ) -> bool {
        let template_name = template.get_name().to_string();

        for entry in &self.modules {
            let canceled = entry.with_module(|module| {
                module_production_queue_kind(module).and_then(|kind| {
                    if kind.cancel_unit_by_template_name(&template_name) {
                        Some(())
                    } else {
                        None
                    }
                })
            });

            if canceled.is_some() {
                return true;
            }
        }

        for behavior in &self.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };

            if let Some(kind) = behavior_production_queue_kind(&mut *behavior_guard) {
                return kind.cancel_unit_by_template_name(&template_name);
            }

            if let Some(prod) = behavior_guard.get_production_update_interface() {
                if prod.cancel_production(0).is_ok() {
                    return true;
                }
            }
        }

        false
    }

    pub fn queue_upgrade(&self, upgrade: &Arc<UpgradeTemplate>) -> bool {
        self.queue_upgrade_via_production(upgrade)
    }

    pub fn queue_unit(&self, template: &Arc<dyn crate::common::ThingTemplate>) -> bool {
        self.queue_unit_via_production(template)
    }

    pub fn queue_unit_with_production_id(
        &self,
        template: &Arc<dyn crate::common::ThingTemplate>,
        production_id: u32,
    ) -> bool {
        if production_id == 0 {
            return self.queue_unit_via_production(template);
        }
        self.queue_unit_via_production_id(template, production_id)
    }

    pub fn request_unique_unit_production_id(&mut self) -> Option<u32> {
        for entry in &mut self.modules {
            let id = entry.with_module_mut(|module| {
                module_production_queue_kind(module).and_then(|kind| kind.request_unique_unit_id())
            });

            if id.is_some() {
                return id;
            }
        }

        for behavior in &self.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };

            if let Some(kind) = behavior_production_queue_kind(&mut *behavior_guard) {
                if let Some(id) = kind.request_unique_unit_id() {
                    return Some(id);
                }
            }
        }

        None
    }

    pub fn cancel_upgrade(&self, upgrade: &Arc<UpgradeTemplate>) -> bool {
        self.cancel_upgrade_via_production(upgrade)
    }

    pub fn cancel_unit_by_template(
        &self,
        template: &Arc<dyn crate::common::ThingTemplate>,
    ) -> bool {
        self.cancel_unit_via_template(template)
    }

    pub fn cancel_unit_by_production_id(&self, production_id: u32) -> bool {
        self.cancel_unit_via_production_id(production_id)
    }

    pub(super) fn cancel_and_refund_all_production_for_capture_or_defection(&mut self) {
        for entry in &self.modules {
            entry.with_module(|module| {
                if let Some(kind) = module_production_queue_kind(module) {
                    kind.cancel_and_refund_all();
                }
            });
        }

        for behavior in &self.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };
            if let Some(prod) = behavior_guard.get_production_update_interface() {
                Self::cancel_production_queue_entries(prod);
            }
        }
    }

    pub(super) fn cancel_production_queue_entries(prod: &mut dyn ProductionUpdateInterface) {
        let mut safety = 0usize;
        let mut previous_size = usize::MAX;

        while safety < 128 {
            let queue_size = prod.get_queue_size();
            if queue_size == 0 || queue_size == previous_size {
                break;
            }
            previous_size = queue_size;

            if prod.cancel_production(0).is_err() && prod.cancel_production(1).is_err() {
                break;
            }
            safety += 1;
        }
    }

    /// C++ `Object::canProduceUpgrade` — CommandSet walk, not production-module presence.
    ///
    /// `TheControlBar->findCommandSet(getCommandSetString())`, then each of
    /// `MAX_COMMANDS_PER_SET` slots via `getCommandButton(i)` (which consults
    /// `GameLogic::findControlBarOverride`). True only if that button's
    /// `getUpgradeTemplate()` matches the requested upgrade.
    pub fn can_produce_upgrade(&self, upgrade: &crate::upgrade::template::UpgradeTemplate) -> bool {
        let Some(control_bar) = crate::control_bar::get_control_bar_bridge() else {
            return false;
        };
        let Some(command_set) = control_bar.find_command_set_by_name(self.get_command_set_string())
        else {
            return false;
        };
        for index in 0..crate::command_button::MAX_COMMANDS_PER_SET {
            let Some(button) = command_set.get_command_button(index) else {
                continue;
            };
            if let Some(template) = button.get_upgrade_template() {
                if template.get_name() == upgrade.get_name() {
                    return true;
                }
            }
        }
        false
    }

    /// Enable or disable production for this object (matches C++ ProductionUpdate::setEnabled).
    pub fn set_production_enabled(&mut self, enabled: bool) {
        for entry in &self.modules {
            let handled = entry.with_module(|module| {
                module_production_queue_kind(module).map(|kind| kind.set_enabled(enabled))
            });
            if handled.is_some() {
                return;
            }
        }

        for behavior in &self.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };

            if let Some(kind) = behavior_production_queue_kind(&mut *behavior_guard) {
                if kind.apply_production_enabled(enabled) {
                    continue;
                }
            }

            if let Some(prod) = behavior_guard.get_production_update_interface() {
                if enabled {
                    prod.resume_production();
                } else {
                    prod.pause_production();
                }
                continue;
            }
        }
    }

    pub fn get_disabled_until(&self, disabled_type: DisabledType) -> UnsignedInt {
        if disabled_type == DisabledType::DisabledAny {
            let mut highest_frame: UnsignedInt = 0;
            for i in 0..DISABLED_COUNT {
                if let Some(dt) = disabled_type_from_index(i) {
                    if self.disabled_mask.test(dt) && self.disabled_till_frame[i] > highest_frame {
                        highest_frame = self.disabled_till_frame[i];
                    }
                }
            }
            highest_frame
        } else if let Some(index) = self.get_disabled_type_index(disabled_type) {
            if self.disabled_mask.test(disabled_type) {
                return self.disabled_till_frame[index];
            }
            0
        } else {
            0
        }
    }
}
