// ContainModuleInterface and Arc extension
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

/// Contain module interface for garrison/transport (matching C++ ContainModuleInterface)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainWant {
    WantsToEnter,
    WantsToExit,
    WantsNeither,
}

pub trait ContainModuleInterface: Send + Sync + std::fmt::Debug {
    fn can_contain(&self, object_id: ObjectID) -> bool;
    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String>;
    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String>;
    fn get_contained_objects(&self) -> &[ObjectID];
    fn get_contained_count(&self) -> usize;
    fn get_max_capacity(&self) -> usize;

    /// C++ `ContainModuleInterface::isSpecialZeroSlotContainer` — parachute-style
    /// containers whose riders do not consume the holder's transport slots.
    fn is_special_zero_slot_container(&self) -> bool {
        false
    }

    fn snapshot_crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        Ok(())
    }

    fn snapshot_xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        Ok(())
    }

    fn snapshot_load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// Per-frame containment update.
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        Ok(UpdateSleepTime::Forever)
    }

    /// Called after the owning object finishes module construction.
    fn on_owner_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Containment reaction to owner damage.
    fn on_damage(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Containment reaction to owner death.
    fn on_die(
        &mut self,
        _damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Containment reaction to owner body damage state changes.
    fn on_body_damage_state_change(
        &mut self,
        _damage_info: &DamageInfo,
        _old_state: BodyDamageType,
        _new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Containment reaction to owner deletion.
    fn on_delete(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Whether this container encloses (hides/masks) the given contained object.
    ///
    /// C++ reference: `ContainModuleInterface::isEnclosingContainerFor`.
    ///
    /// Most containers enclose their passengers; specialized riders (e.g. Overlord/Helix payloads)
    /// are expected to override this to return `false` for visible riders.
    fn is_enclosing_container_for(&self, _obj: &Object) -> bool {
        true
    }

    /// Whether this container is a heal-only container (matches C++ isHealContain).
    fn is_heal_contain(&self) -> bool {
        false
    }

    /// Whether this container can be busted by bunker buster weapons.
    fn is_bustable(&self) -> bool {
        false
    }

    /// Record the owning object's pre-capture team for distributed garrisons.
    fn set_original_team(&mut self, _old_team: Option<Weak<RwLock<Team>>>) {}

    /// Check if this container is valid for the given object
    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        // Default implementation - always allow
        let _ = (obj, check_capacity);
        true
    }

    /// Add object to containment
    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_id = obj.get_id();
        if !self.can_contain(object_id) {
            return Err("Container cannot accept object".into());
        }
        self.contain_object(object_id).map_err(|err| err.into())
    }

    /// Add object to the literal contain list without full enter callbacks.
    fn add_to_contain_list(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.add_to_contain(obj)
    }

    /// Enable or disable load sounds for this container
    fn enable_load_sounds(
        &mut self,
        _enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Default implementation does nothing
        Ok(())
    }

    /// Notify container that an object wants to enter/exit.
    fn on_object_wants_to_enter_or_exit(
        &mut self,
        _obj: &Object,
        _want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Whether this container can be garrisoned (default: false).
    fn is_garrisonable(&self) -> bool {
        false
    }

    /// C++ ContainModuleInterface::isRiderChangeContain (combat-bike / rider swap).
    fn is_rider_change_contain(&self) -> bool {
        false
    }


    /// Whether clear-building attacks should spare passengers.
    ///
    /// C++ parity: OpenContain defaults this to true; GarrisonContain overrides from INI.
    fn is_immune_to_clear_building_attacks(&self) -> bool {
        true
    }

    /// Attempt to position a garrisoned unit at the best fire point for a target object.
    /// Matches C++ ContainModuleInterface::attemptBestFirePointPosition.
    fn attempt_best_fire_point_position(
        &mut self,
        _source_id: ObjectID,
        _weapon: &crate::weapon::Weapon,
        _victim_id: ObjectID,
    ) -> bool {
        false
    }

    /// Attempt to position a garrisoned unit at the best fire point for a target position.
    /// Matches C++ ContainModuleInterface::attemptBestFirePointPosition (position overload).
    fn attempt_best_fire_point_position_coord(
        &mut self,
        _source_id: ObjectID,
        _weapon: &crate::weapon::Weapon,
        _target_pos: &Coord3D,
    ) -> bool {
        false
    }

    /// C++ ContainModuleInterface::calcBestGarrisonPosition (WeaponSet.cpp:638).
    fn calc_best_garrison_position(
        &self,
        _source_pos: &mut Coord3D,
        _target_pos: &Coord3D,
    ) -> bool {
        false
    }


    /// Returns the apparent controlling player when the container is garrisoned/stealth-contained.
    fn get_apparent_controlling_player(
        &self,
        _observing_player: Option<&Player>,
    ) -> Option<Arc<RwLock<Player>>> {
        None
    }

    /// Override drop destination for contained objects (default: no-op).
    fn set_override_destination(&mut self, _pos: &Coord3D) {}

    /// Set a rally point for contained units that exit this container.
    fn set_rally_point(&mut self, _pos: Coord3D) {}

    /// Return the rally point for contained units that exit this container.
    fn get_rally_point(&self) -> Option<Coord3D> {
        None
    }

    /// Whether the specified contained object can exit through this container.
    fn can_exit(&self, object_id: ObjectID) -> bool {
        self.get_contained_objects().contains(&object_id)
    }

    /// Reserve an exit door/path for a contained object.
    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&crate::object::Object>,
        _spawn: Option<&crate::object::Object>,
    ) -> ExitDoorType {
        DOOR_NONE_AVAILABLE
    }

    /// Release a reserved exit door/path.
    fn unreserve_door_for_exit(&mut self, _door: ExitDoorType) {}

    /// Exit a contained object via a reserved door/path.
    fn exit_object_via_door(
        &mut self,
        _obj_id: ObjectID,
        _door: ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Special tunnel-network style exit that preserves the passenger's current AI state.
    fn exit_object_in_a_hurry(
        &mut self,
        _obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Whether a passenger is allowed to fire (default: false).
    fn is_passenger_allowed_to_fire(&self, _id: Option<ObjectID>) -> bool {
        false
    }

    /// Whether the container passes weapon bonus flags to passengers.
    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        false
    }

    /// Toggle whether passengers may fire from this container (default: no-op).
    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        let _ = allowed;
    }

    /// Script hook for cave/tunnel containers (default: no-op).
    fn try_to_set_cave_index(&mut self, _new_index: Int) {}

    /// Script hook for garrison evac disposition (default: no-op).
    fn set_evac_disposition(&mut self, _disposition: UnsignedInt) {}
    /// C++ `ContainModuleInterface::onSelling`. OpenContain orders passengers out;
    /// Garrison/Tunnel override to force-empty first.
    fn on_selling(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// C++ CreateModuleInterface::onCreate (CaveContain copies CaveIndex).
    fn on_create(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// C++ CreateModuleInterface::onBuildComplete (CaveContain registers the network).
    fn on_build_complete(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// C++ CreateModule::shouldDoOnBuildComplete.
    fn should_do_on_build_complete(&self) -> bool {
        false
    }

    /// C++ `OpenContain::markAllPassengersDetected`. Reveal stealth-garrison riders
    /// immediately before an evac order so they do not stay cloaked on exit.
    fn mark_all_passengers_detected(&mut self) {
        if dual_world_registry_unavailable() {
            return;
        }
        for object_id in self.get_contained_objects() {
            let Some(obj) = TheGameLogic::find_object_by_id(*object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj.read() else {
                continue;
            };
            if !obj_guard.is_kind_of(KindOf::StealthGarrison) {
                continue;
            }
            if let Some(stealth) = obj_guard.get_stealth() {
                if let Ok(mut stealth_guard) = stealth.lock() {
                    stealth_guard.mark_as_detected();
                }
            }
        }
    }


    /// Order all passengers to exit (matches C++ OpenContain::orderAllPassengersToExit).
    fn order_all_passengers_to_exit(
        &mut self,
        command_source: CommandSourceType,
        instantly: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 340: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        self.mark_all_passengers_detected();

        let cmd = if instantly {
            AiCommandType::ExitInstantly
        } else {
            AiCommandType::Exit
        };

        for object_id in self.get_contained_objects() {
            if let Some(obj) = TheGameLogic::find_object_by_id(*object_id) {
                let container_id = obj.read().ok().and_then(|guard| guard.get_contained_by());
                if let Ok(obj_guard) = obj.read() {
                    if let Some(ai) = obj_guard.get_ai() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            let mut params = AiCommandParams::new(cmd, command_source);
                            params.obj = container_id;
                            let _ = ai_guard.execute_command(&params);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Order all passengers to idle (matches C++ OpenContain::orderAllPassengersToIdle).
    fn order_all_passengers_to_idle(
        &mut self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 340: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        for object_id in self.get_contained_objects() {
            if let Some(obj) = TheGameLogic::find_object_by_id(*object_id) {
                if let Ok(obj_guard) = obj.read() {
                    if let Some(ai) = obj_guard.get_ai() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            let params = AiCommandParams::new(AiCommandType::Idle, command_source);
                            let _ = ai_guard.execute_command(&params);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Order money hackers to begin hacking (matches C++ OpenContain::orderAllPassengersToHackInternet).
    fn order_all_passengers_to_hack_internet(
        &mut self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 340: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        for object_id in self.get_contained_objects() {
            if let Some(obj) = TheGameLogic::find_object_by_id(*object_id) {
                if let Ok(obj_guard) = obj.read() {
                    if !obj_guard.is_kind_of(KindOf::Hacker) {
                        continue;
                    }
                    if let Some(ai) = obj_guard.get_ai() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            let params =
                                AiCommandParams::new(AiCommandType::HackInternet, command_source);
                            let _ = ai_guard.execute_command(&params);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Notify container that an object started being contained.
    fn on_containing(
        &mut self,
        _obj_id: ObjectID,
        _was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Notify container that an object is being removed.
    fn on_removing(
        &mut self,
        _obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Notify container that its owning object has been captured.
    /// Matches C++ `ContainModuleInterface::onCapture`.
    fn on_capture(
        &mut self,
        _owner: &Object,
        _old_owner: Option<&Arc<RwLock<Player>>>,
        _new_owner: Option<&Arc<RwLock<Player>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Remove all contained objects.
    fn remove_all_contained(
        &mut self,
        _expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Whether this contain should be displayed on the control bar.
    fn is_displayed_on_control_bar(&self) -> bool {
        true
    }

    /// Whether passengers should be kicked out on capture.
    fn is_kick_out_on_capture(&self) -> bool {
        true
    }

    /// Force all contained objects to exit, and damage them.
    /// Matches C++ OpenContain::harmAndForceExitAllContained.
    fn harm_and_force_exit_all_contained(
        &mut self,
        _damage_info: &mut crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Kill all contained objects.
    /// Matches C++ OpenContain::killAllContained.
    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Flash visible contained units as selected.
    fn client_visible_contained_flash_as_selected(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Get contain count as u32 (matches legacy API).
    fn get_contain_count(&self) -> u32 {
        self.get_contained_count() as u32
    }

    /// Number of passengers currently hidden by stealth containment.
    fn get_stealth_units_contained(&self) -> UnsignedInt {
        // Wave 340: empty dual-world → 0.
        if dual_world_registry_unavailable() {
            return 0;
        }

        self.get_contained_objects()
            .iter()
            .filter_map(|id| TheGameLogic::find_object_by_id(*id))
            .filter(|obj| {
                obj.read()
                    .ok()
                    .map(|guard| guard.test_status(ObjectStatusTypes::Stealthed))
                    .unwrap_or(false)
            })
            .count() as UnsignedInt
    }

    /// Get contain max as i32 (matches legacy API).
    fn get_contain_max(&self) -> i32 {
        let max = self.get_max_capacity();
        if max == usize::MAX {
            -1
        } else {
            max as i32
        }
    }

    /// Return container pip display data for selected-object UI.
    ///
    /// Matches C++ `ContainModuleInterface::getContainerPipsToShow`, including the ability for
    /// specialized containers to suppress pip drawing.
    fn get_container_pips_to_show(&self) -> (i32, i32, bool) {
        (
            self.get_contain_max(),
            self.get_contained_count() as i32,
            true,
        )
    }

    /// Return the player mask for the last player who entered this container.
    fn get_player_who_entered(&self) -> PlayerMaskType {
        PlayerMaskType::none()
    }

    /// Return the special rider object for Overlord-style containers.
    fn friend_get_rider(&self) -> Option<ObjectID> {
        None
    }

    /// Whether any contained object wants to enter/exit (C++ hasObjectsWantingToEnterOrExit).
    fn has_objects_wanting_to_enter_or_exit(&self) -> bool {
        false
    }

    /// Whether this container is a special Overlord-style container.
    fn is_special_overlord_style_container(&self) -> bool {
        false
    }

    /// Get the rider ID for Overlord-style containers (alias for friend_get_rider).
    fn get_rider_id(&self) -> Option<ObjectID> {
        self.friend_get_rider()
    }

    /// C++ ContainModuleInterface::processDamageToContained.
    /// OpenContain/TransportContain override with BURNED vs NORMAL + flame-proof kill.
    fn process_damage_to_contained(&mut self, _percent_damage: f32) {}
}

/// Extension trait for Arc<Mutex<dyn ContainModuleInterface>> to provide convenient methods
pub trait ContainModuleInterfaceExt {
    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool;
    fn add_to_contain(&self, obj: &Object);
    fn get_contained_objects(&self) -> Vec<ObjectID>;
    fn get_contained_count(&self) -> usize;
    fn enable_load_sounds(
        &self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn on_removing(&self, obj: &Object);
    fn on_object_wants_to_enter_or_exit(&self, obj: &Object, want: ContainWant);
    fn is_enclosing_container_for(&self, obj: &Object) -> bool;
    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool;
    fn set_override_destination(&self, pos: &Coord3D);
    fn set_rally_point(&self, pos: Coord3D);
    fn get_rally_point(&self) -> Option<Coord3D>;
    fn has_objects_wanting_to_enter_or_exit(&self) -> bool;
    fn is_special_overlord_style_container(&self) -> bool;
    fn get_rider_id(&self) -> Option<ObjectID>;
    fn friend_get_rider(&self) -> Option<ObjectID>;
    fn order_all_passengers_to_exit(
        &self,
        command_source: CommandSourceType,
        instantly: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn order_all_passengers_to_idle(
        &self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn order_all_passengers_to_hack_internet(
        &self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn harm_and_force_exit_all_contained(
        &self,
        damage_info: &mut crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn kill_all_contained(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn process_damage_to_contained(&self, percent_damage: f32);
    fn on_selling(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn mark_all_passengers_detected(&self);
}

impl ContainModuleInterfaceExt for Arc<Mutex<dyn ContainModuleInterface>> {
    // C++ ContainModuleInterface methods never silently no-op on a "busy" container.
    // `try_lock()` returned false / skipped add under contention, which breaks OCL
    // `containInsideSourceObject` (treats a valid container as invalid and destroys
    // the spawn) and UI/script enter checks. Critical paths therefore block on
    // `lock()` like C++ direct calls.
    //
    // Poison is fail-closed: validity/reads report empty/false, mutations do not
    // panic and do not pretend success.
    //
    // Deadlock note: std::sync::Mutex is not reentrant. A same-thread re-lock
    // exists in OverlordContain::create_payload, which calls
    // `owner.get_contain()` + Ext (`is_valid_container_for` / `add_to_contain`)
    // while the same mutex is already held via `on_owner_created` /
    // `Object::update` → `contain.lock()`. TransportContain / HelixContain call
    // `self` methods instead and are safe. That Overlord path is a Rust-only
    // re-entry (C++ called `this->addToContain`); it currently no-ops with
    // try_lock and would block forever with lock(). Fix belongs in
    // overlord_contain.rs (call `self` like Transport/Helix). OCL/UI callers
    // do not hold this mutex, so blocking lock() is the C++-closer behavior.

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        match self.lock() {
            Ok(guard) => guard.is_valid_container_for(obj, check_capacity),
            Err(_) => false,
        }
    }

    fn add_to_contain(&self, obj: &Object) {
        // Poison / failed lock: fail-closed. Do not add and do not panic;
        // callers observe the object remaining uncontained.
        if let Ok(mut guard) = self.lock() {
            let _ = guard.add_to_contain(obj);
        }
    }

    fn get_contained_objects(&self) -> Vec<ObjectID> {
        match self.lock() {
            Ok(guard) => guard.get_contained_objects().to_vec(),
            Err(_) => Vec::new(),
        }
    }

    fn get_contained_count(&self) -> usize {
        match self.lock() {
            Ok(guard) => guard.get_contained_count(),
            Err(_) => 0,
        }
    }

    fn enable_load_sounds(
        &self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.enable_load_sounds(enabled)
        } else {
            Err("Failed to lock ContainModuleInterface".into())
        }
    }

    fn on_removing(&self, _obj: &Object) {
        // Wave 340: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        if let Some(obj_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(_obj.get_id()) {
            if let Ok(mut guard) = self.try_lock() {
                let _ = guard.on_removing(obj_arc.read().map(|g| g.get_id()).unwrap_or(0));
            }
        }
    }

    fn on_object_wants_to_enter_or_exit(&self, obj: &Object, want: ContainWant) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.on_object_wants_to_enter_or_exit(obj, want);
        }
    }

    fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_enclosing_container_for(obj)
        } else {
            true
        }
    }

    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_passenger_allowed_to_fire(id)
        } else {
            false
        }
    }

    fn set_override_destination(&self, pos: &Coord3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_override_destination(pos);
        }
    }

    fn set_rally_point(&self, pos: Coord3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_rally_point(pos);
        }
    }

    fn get_rally_point(&self) -> Option<Coord3D> {
        self.try_lock()
            .ok()
            .and_then(|guard| guard.get_rally_point())
    }

    fn has_objects_wanting_to_enter_or_exit(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.has_objects_wanting_to_enter_or_exit()
        } else {
            false
        }
    }

    fn is_special_overlord_style_container(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_special_overlord_style_container()
        } else {
            false
        }
    }

    fn get_rider_id(&self) -> Option<ObjectID> {
        if let Ok(guard) = self.try_lock() {
            guard.get_rider_id()
        } else {
            None
        }
    }

    fn friend_get_rider(&self) -> Option<ObjectID> {
        if let Ok(guard) = self.try_lock() {
            guard.friend_get_rider()
        } else {
            None
        }
    }

    fn order_all_passengers_to_exit(
        &self,
        command_source: CommandSourceType,
        instantly: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.order_all_passengers_to_exit(command_source, instantly)
        } else {
            Ok(())
        }
    }

    fn order_all_passengers_to_idle(
        &self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.order_all_passengers_to_idle(command_source)
        } else {
            Ok(())
        }
    }

    fn order_all_passengers_to_hack_internet(
        &self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.order_all_passengers_to_hack_internet(command_source)
        } else {
            Ok(())
        }
    }

    fn on_selling(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.lock() {
            guard.on_selling()
        } else {
            Ok(())
        }
    }

    fn mark_all_passengers_detected(&self) {
        if let Ok(mut guard) = self.lock() {
            guard.mark_all_passengers_detected();
        }
    }

    fn harm_and_force_exit_all_contained(
        &self,
        damage_info: &mut crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.harm_and_force_exit_all_contained(damage_info)
        } else {
            Ok(())
        }
    }

    fn kill_all_contained(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.kill_all_contained()
        } else {
            Ok(())
        }
    }

    fn process_damage_to_contained(&self, percent_damage: f32) {
        if let Ok(mut guard) = self.lock() {
            guard.process_damage_to_contained(percent_damage);
        }
    }
}
