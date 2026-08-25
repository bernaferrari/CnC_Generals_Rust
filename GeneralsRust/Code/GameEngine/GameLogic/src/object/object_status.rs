//! Split-out inherent `status, script status, geometry, kind-of, selectability` methods for [`Object`].
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    pub fn is_airborne_target(&self) -> bool {
        self.test_status(ObjectStatusTypes::AirborneTarget)
    }

    pub fn get_status_bits(&self) -> ObjectStatusMaskType {
        self.status
    }

    pub fn test_status(&self, bit: ObjectStatusTypes) -> bool {
        self.status.test(bit)
    }

    /// Check for a booby trap attached to this object and detonate it if needed.
    /// Mirrors C++ Object::checkAndDetonateBoobyTrap.
    /// ID-based victim check; prefer over Arc-resolved `&Object` at call sites.
    pub fn check_and_detonate_booby_trap_for_victim_id(&self, victim_id: Option<ObjectID>) -> bool {
        // Wave 264: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        match victim_id {
            Some(id) if id != INVALID_ID => crate::object::registry::OBJECT_REGISTRY
                .with_object(id, |victim| {
                    self.check_and_detonate_booby_trap(Some(victim))
                })
                .unwrap_or_else(|| self.check_and_detonate_booby_trap(None)),
            _ => self.check_and_detonate_booby_trap(None),
        }
    }

    pub fn check_and_detonate_booby_trap(&self, victim: Option<&Object>) -> bool {
        const BOOBY_TRAP_SCAN_RANGE: Real = 25.0;

        if !self.test_status(ObjectStatusTypes::BoobyTrapped) {
            return false;
        }

        let scan_radius =
            BOOBY_TRAP_SCAN_RANGE + self.get_geometry_info().get_bounding_circle_radius();
        let pos = *self.get_position();

        let Some(partition) = crate::helpers::ThePartitionManager::get() else {
            return false;
        };

        for object_id in partition.get_objects_in_range(&pos, scan_radius) {
            let Some(booby_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };

            let update_module = {
                let Ok(booby_guard) = booby_arc.read() else {
                    continue;
                };

                if !booby_guard.is_kind_of(KindOf::BoobyTrap) {
                    continue;
                }
                if booby_guard.get_producer_id() != self.id {
                    continue;
                }

                if let Some(victim_obj) = victim {
                    if booby_guard.relationship_to(victim_obj) == Relationship::Allies {
                        return false;
                    }
                }

                booby_guard.find_update_module("StickyBombUpdate")
            };

            if let Some(module) = update_module {
                let mut detonated = false;
                module.with_module(|module| {
                    if let Some(sticky_bomb) = module.get_sticky_bomb_control_interface() {
                        sticky_bomb.detonate();
                        detonated = true;
                    }
                });
                if detonated {
                    return true;
                }
            }

            return false;
        }

        false
    }

    /// Set object status bits with proper side effects
    /// C++ Reference: Object.cpp lines 954-1039
    ///
    /// This method handles all status bit changes and their associated effects:
    /// - Repulsor status activates temporary repulsion (C++ line 965-970)
    /// - Stealth/Detected/Disguised status triggers partition updates (C++ line 972-980)
    /// - Under construction status checks for mines and updates shroud (C++ line 985-1031)
    /// - Sets/clears status bits as requested
    ///
    /// # Arguments
    /// * `object_status` - Status mask to set or clear
    /// * `set` - true to set the status, false to clear it
    ///
    /// # Behavior
    /// - Compares old status with new status
    /// - Applies special effects based on which status bits changed
    /// - Updates partition cells if visibility-related status changed
    pub fn set_status(&mut self, object_status: ObjectStatusMaskType, set: bool) {
        use crate::common::types::ObjectStatusTypes;

        let old_status = self.status;

        // Apply the status change (C++ line 958-961)
        if set {
            self.status |= object_status;
        } else {
            self.status &= !object_status;
        }

        // Only process side effects if status actually changed (C++ line 963)
        if self.status == old_status {
            return;
        }

        // Repulsor status side effect (C++ lines 965-970).
        // Only existing helpers (CAN_BE_REPULSED) sleep 2 seconds; do not create one.
        if set && object_status.test_status(ObjectStatusTypes::Repulsor) {
            self.wake_repulsor_helper_for_status();
        }

        // Stealth/Detection status side effects (C++ lines 972-980).
        // Partition vision only when shroud-reveal-to-all is actually used.
        if object_status.test_status(ObjectStatusTypes::Stealthed)
            || object_status.test_status(ObjectStatusTypes::Detected)
            || object_status.test_status(ObjectStatusTypes::Disguised)
        {
            if self.get_template().get_shroud_reveal_to_all_range() > 0.0 {
                self.handle_partition_cell_maintenance();
            }
        }

        // Under construction status side effects (C++ lines 985-1031).
        // Potential-collision iterate; enemies detonate; allies/neutrals silent destroy.
        if self
            .status
            .test_status(ObjectStatusTypes::UnderConstruction)
            != old_status.test_status(ObjectStatusTypes::UnderConstruction)
        {
            let position = *self.get_position();
            let geometry = self.get_geometry_info().clone();
            let orientation = self.get_orientation();

            if let Some(partition) = crate::helpers::ThePartitionManager::get() {
                for object_id in
                    partition.iterate_potential_collisions(&position, &geometry, orientation)
                {
                    if object_id == self.id {
                        continue;
                    }

                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id)
                    else {
                        continue;
                    };

                    let (is_mine, relationship) = {
                        let Ok(obj_guard) = obj_arc.read() else {
                            continue;
                        };
                        if obj_guard.is_destroyed() {
                            continue;
                        }
                        (
                            obj_guard.is_kind_of(KindOf::Mine),
                            self.relationship_to(&obj_guard),
                        )
                    };

                    if !is_mine {
                        continue;
                    }

                    match relationship {
                        Relationship::Enemies => {
                            if let Ok(mut obj_guard) = obj_arc.write() {
                                obj_guard
                                    .kill(Some(DamageType::LandMine), Some(DeathType::Exploded));
                            }
                        }
                        Relationship::Allies | Relationship::Neutral => {
                            let _ = crate::helpers::TheGameLogic::destroy_object_by_id(object_id);
                        }
                    }
                }
            }

            // Update partition for shroud changes (C++ line 1010-1011)
            self.handle_partition_cell_maintenance();
        }
    }

    pub(super) fn populate_radar_object_from_state(&self, radar_obj: &mut RadarObject) {
        radar_obj.is_hero = self.is_hero();
        radar_obj.is_local = self.is_locally_controlled();
        radar_obj.is_stealth =
            self.test_status(ObjectStatusTypes::Stealthed) || self.is_stealthed();
        radar_obj.is_detected = self.test_status(ObjectStatusTypes::Detected);
        radar_obj.is_disguised = self.test_status(ObjectStatusTypes::Disguised);
        radar_obj.is_enemy = self.is_enemy_to_local_player();
    }

    pub(super) fn is_enemy_to_local_player(&self) -> bool {
        let Some(team) = self.get_team() else {
            return false;
        };
        let Some(local_player) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
        else {
            return false;
        };
        let Ok(team_guard) = team.read() else {
            return false;
        };
        let Ok(local_player_guard) = local_player.read() else {
            return false;
        };

        local_player_guard.is_enemy_with_team(&team_guard)
    }

    pub(crate) fn refresh_radar_object_from_state(&self) {
        let Some(radar_data) = &self.radar_data else {
            return;
        };
        let Ok(mut radar_guard) = radar_data.lock() else {
            return;
        };

        let mut radar_obj = radar_guard.clone();
        self.populate_radar_object_from_state(&mut radar_obj);
        *radar_guard = radar_obj.clone();
        drop(radar_guard);

        let radar = game_engine::common::system::radar::get_radar_system();
        let radar_write = radar.write();
        if let Ok(mut radar_guard) = radar_write {
            radar_guard.remove_object(self.id);
            radar_guard.add_object(radar_obj);
        }
    }

    pub fn clear_status(&mut self, object_status: ObjectStatusMaskType) {
        self.set_status(object_status, false);
    }

    /// Mask/unmask an object (C++ Object::maskObject).
    ///
    /// Masking hides the object from selection/targeting and forces a deselect
    /// from currently selected groups.
    pub fn mask_object(&mut self, mask: bool) {
        self.set_status(ObjectStatusMaskType::MASKED, mask);

        if mask {
            let deselect_mask = self
                .get_controlling_player()
                .and_then(|player| player.read().ok().map(|guard| guard.get_player_mask()))
                .map(|mask| PlayerMaskType::from_bits_truncate(!mask.bits()))
                .unwrap_or(crate::common::PLAYERMASK_ALL);

            let _ = crate::helpers::TheGameLogic::deselect_object(self, deselect_mask, true);
        }
    }

    // Script status management
    pub fn test_script_status_bit(&self, bit: ObjectScriptStatusBit) -> bool {
        (self.script_status & (bit as u8)) != 0
    }

    pub fn set_script_status(&mut self, bit: ObjectScriptStatusBit, set: bool) {
        let old_script_status = self.script_status;
        if set {
            self.script_status |= bit as u8;
        } else {
            self.script_status &= !(bit as u8);
        }

        if self.script_status == old_script_status {
            return;
        }

        let disabled_changed = (self.script_status & ObjectScriptStatusBit::ScriptDisabled as u8)
            != (old_script_status & ObjectScriptStatusBit::ScriptDisabled as u8);
        if disabled_changed {
            self.handle_partition_cell_maintenance();
            if (self.script_status & ObjectScriptStatusBit::ScriptDisabled as u8) != 0 {
                self.set_disabled(DisabledType::DisabledScriptDisabled);
            } else {
                self.clear_disabled(DisabledType::DisabledScriptDisabled);
            }
        }

        let underpowered_changed = (self.script_status
            & ObjectScriptStatusBit::ScriptUnderpowered as u8)
            != (old_script_status & ObjectScriptStatusBit::ScriptUnderpowered as u8);
        if underpowered_changed {
            self.handle_partition_cell_maintenance();
            if (self.script_status & ObjectScriptStatusBit::ScriptUnderpowered as u8) != 0 {
                self.set_disabled(DisabledType::DisabledScriptUnderpowered);
            } else {
                self.clear_disabled(DisabledType::DisabledScriptUnderpowered);
            }
        }
    }

    pub fn clear_script_status(&mut self, bit: ObjectScriptStatusBit) {
        self.set_script_status(bit, false);
    }

    pub fn is_undetected_defector(&self) -> bool {
        (self.private_status & ObjectPrivateStatusBits::UndetectedDefector as u8) != 0
    }

    pub fn set_undetected_defector_flag(&mut self, value: bool) {
        if value {
            self.private_status |= ObjectPrivateStatusBits::UndetectedDefector as u8;
        } else {
            self.private_status &= !(ObjectPrivateStatusBits::UndetectedDefector as u8);
        }
    }

    pub fn set_undetected_defector(&mut self, value: bool) {
        self.set_undetected_defector_flag(value);
    }

    pub fn friend_set_undetected_defector(&mut self, value: bool) {
        self.set_undetected_defector_flag(value);
    }

    // Geometry and positioning
    pub fn get_geometry_info(&self) -> &GeometryInfo {
        &self.geometry_info
    }

    /// Mark this object as unmanned (DisabledUnmanned flag).
    pub fn set_disabled_unmanned(&mut self) {
        self.set_disabled(DisabledType::DisabledUnmanned);
    }

    /// Set team to neutral if available.
    pub fn set_team_to_neutral(&mut self) {
        if let Some(neutral_player) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_neutral_player())
        {
            if let Ok(p) = neutral_player.read() {
                let _ = self.set_team(p.get_default_team());
            }
        }
    }

    /// Clear selection for all players.
    pub fn deselect_all(&mut self) {
        let _ = crate::helpers::TheGameLogic::deselect_object(
            self,
            crate::common::PLAYERMASK_ALL,
            false,
        );
    }

    /// Convenience: is the object flagged as a vehicle?
    pub fn is_vehicle(&self) -> bool {
        self.is_kind_of(KindOf::Vehicle)
    }

    /// Convenience: is the object flagged as a structure?
    pub fn is_structure(&self) -> bool {
        self.is_kind_of(KindOf::Structure)
    }

    /// C++ Object::isFactionStructure(): any KINDOF_FS bit marks a faction structure.
    pub fn is_faction_structure(&self) -> bool {
        self.is_any_kind_of(&[
            KindOf::FSBarracks,
            KindOf::FSWarfactory,
            KindOf::FSAirfield,
            KindOf::FSInternetCenter,
            KindOf::FSPower,
            KindOf::FSBaseDefense,
            KindOf::FSSupplyDropzone,
            KindOf::FSSupplyCenter,
            KindOf::FSSuperweapon,
            KindOf::FSStrategyCenter,
            KindOf::FSFake,
            KindOf::FSTechnology,
            KindOf::FsBlackMarket,
            KindOf::FsAdvancedTech,
        ])
    }

    /// C++ Object::isNonFactionStructure().
    pub fn is_non_faction_structure(&self) -> bool {
        self.is_structure() && !self.is_faction_structure()
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.thing_template.is_kind_of(kind)
    }

    pub fn is_any_kind_of(&self, kinds: &[KindOf]) -> bool {
        kinds.iter().any(|kind| self.is_kind_of(*kind))
    }

    // Selection
    pub fn is_selectable(&self) -> bool {
        if self.is_kind_of(KindOf::AlwaysSelectable) {
            return true;
        }

        self.is_selectable
            && !self.test_status(ObjectStatusTypes::Unselectable)
            && !self.is_effectively_dead()
    }

    pub fn is_mass_selectable(&self) -> bool {
        self.is_selectable() && !self.is_kind_of(KindOf::Structure)
    }

    /// Check if this object is mobile (not immobile and not disabled).
    /// C++ Reference: Object.cpp line 2878 (Object::isMobile)
    pub fn is_mobile(&self) -> bool {
        if self.is_kind_of(KindOf::Immobile) {
            return false;
        }
        if self.is_disabled() {
            return false;
        }
        true
    }

    /// Get radar priority for this object.
    /// C++ Reference: Object.cpp line 6240 (Object::getRadarPriority)
    pub fn get_radar_priority(&self) -> crate::common::RadarPriorityType {
        use crate::common::RadarPriorityType;

        // Start with template default
        let mut priority = self.get_template().get_radar_priority();

        // If invalid, infer from object properties (C++ lines 6254-6267)
        if priority == RadarPriorityType::Invalid {
            // Garrisonable objects show as structures
            if self
                .get_contain()
                .and_then(|contain| contain.lock().ok().map(|guard| guard.is_garrisonable()))
                .unwrap_or(false)
            {
                priority = RadarPriorityType::Structure;
            }

            // Capturable objects show as structures
            if self.is_kind_of(KindOf::Capturable) {
                priority = RadarPriorityType::Structure;
            }
        }

        // Carbombs show as units (C++ line 6270)
        if self.test_status(crate::common::ObjectStatusTypes::IsCarBomb) {
            priority = RadarPriorityType::Unit;
        }

        priority
    }

    /// Check if object is effectively dead
    pub fn is_effectively_dead(&self) -> bool {
        (self.private_status & ObjectPrivateStatusBits::EffectivelyDead as u8) != 0
    }

    /// Mark object as effectively dead
    pub(crate) fn set_effectively_dead(&mut self, dead: bool) {
        if dead {
            self.private_status |= ObjectPrivateStatusBits::EffectivelyDead as u8;
        } else {
            self.private_status &= !(ObjectPrivateStatusBits::EffectivelyDead as u8);
        }
    }

    /// Get the KindOf mask for this object
    /// C++ Reference: Thing.cpp - isKindOf delegates to template
    ///
    /// # Returns
    /// A bitmask representing the kinds/types this object belongs to
    pub fn get_kind_of(&self) -> KindOfMask {
        let mut mask: KindOfMask = 0;
        for kind in crate::common::ALL_KIND_OF {
            if self.is_kind_of(*kind) {
                mask |= kind.cpp_mask();
            }
        }
        mask
    }

    pub fn is_kind_of_mask(&self, mask: u32) -> bool {
        (self.get_kind_of() & mask as u128) != 0
    }

    /// Check required/forbidden KindOf masks (C++ isKindOfMulti).
    pub fn is_kind_of_multi(&self, required: KindOfMaskType, forbidden: KindOfMaskType) -> bool {
        let kinds = self.get_kind_of();
        if required != crate::common::KIND_OF_MASK_NONE && (kinds & required) != required {
            return false;
        }
        if forbidden != crate::common::KIND_OF_MASK_NONE && (kinds & forbidden) != 0 {
            return false;
        }
        true
    }

    // ========================================================================
    // MISCELLANEOUS (5 methods)
    // ========================================================================

    pub fn is_salvage_crate(&self) -> bool {
        for behavior in &self.behaviors {
            let Ok(guard) = behavior.lock() else {
                continue;
            };
            if guard.as_any().is::<crate::object::collide::crate_collide::salvage_crate_collide::SalvageCrateCollide>() {
                return true;
            }
        }
        false
    }

    pub fn is_hero(&self) -> bool {
        // Wave 264: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if let Some(contain) = self.get_contain() {
            if let Ok(guard) = contain.lock() {
                for &contained_id in guard.get_contained_objects() {
                    if OBJECT_REGISTRY
                        .with_object(contained_id, |obj_guard| obj_guard.is_kind_of(KindOf::Hero))
                        .unwrap_or(false)
                    {
                        return true;
                    }
                }
            }
        }
        self.is_kind_of(KindOf::Hero)
    }
}
