//! Split-out inherent `vision, shroud, partition, visibility` methods for [`Object`].
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    pub fn get_shroud_range(&self) -> Real {
        self.shroud_range
    }

    pub fn set_shroud_range(&mut self, range: Real) {
        self.shroud_range = range.max(0.0);
    }

    /// Get object vision range (sight distance in game units)
    pub fn get_vision_range(&self) -> f32 {
        self.vision_range as f32
    }

    /// Update vision range; matches the C++ Object API used by radar upgrades.
    pub fn set_vision_range(&mut self, range: f32) {
        self.vision_range = range.max(0.0);
    }

    /// Mark this object as having its vision "spied" by another player.
    ///
    /// C++ reference: `Object::setVisionSpiedByPlayer` (ref-counted per spying player).
    pub fn set_vision_spied_by_player(&mut self, spying_player_index: Int, on: Bool) {
        if spying_player_index < 0 {
            return;
        }
        let idx = spying_player_index as usize;
        if idx >= MAX_PLAYER_COUNT {
            return;
        }

        let was_spied = self.vision_spied_by[idx] > 0;
        if on {
            self.vision_spied_by[idx] = self.vision_spied_by[idx].saturating_add(1);
        } else {
            self.vision_spied_by[idx] = self.vision_spied_by[idx].saturating_sub(1);
        }
        let is_spied = self.vision_spied_by[idx] > 0;

        if was_spied != is_spied {
            let mut working_mask = PlayerMaskType::none();
            for i in 0..MAX_PLAYER_COUNT {
                if self.vision_spied_by[i] > 0 {
                    working_mask |= PlayerMaskType::from_bits_truncate(1u32 << i);
                }
            }
            self.vision_spied_mask = working_mask;
            self.handle_partition_cell_maintenance();
        }
    }

    pub fn set_vision_spied(&mut self, setting: Bool, by_whom: Int) {
        self.set_vision_spied_by_player(by_whom, setting);
    }

    /// Returns true if this object's vision is currently spied by `player_index`.
    pub fn is_vision_spied_by_player(&self, player_index: UnsignedInt) -> bool {
        let idx = player_index as usize;
        if idx >= MAX_PLAYER_COUNT {
            return false;
        }
        self.vision_spied_by[idx] > 0
    }

    /// Check if this object is visible to a specific player (for rendering)
    /// Used by renderer to determine if object should be rendered
    pub fn is_visible_to_player(&self, player_id: UnsignedInt) -> bool {
        if player_id >= MAX_PLAYER_COUNT as UnsignedInt {
            return false;
        }
        self.visibility_flags[player_id as usize]
    }

    /// Get visibility alpha for a specific player (for rendering fade-in/out)
    /// Returns 0.0 (fully invisible) to 1.0 (fully visible)
    pub fn get_visibility_alpha(&self, player_id: UnsignedInt) -> f32 {
        if player_id >= MAX_PLAYER_COUNT as UnsignedInt {
            return 0.0;
        }
        self.visibility_alpha[player_id as usize]
    }

    /// Get safe occlusion frame
    /// Returns the frame number when this object can be safely occluded
    pub fn get_safe_occlusion_frame(&self) -> UnsignedInt {
        self.safe_occlusion_frame
    }

    /// Set safe occlusion frame
    /// Sets the frame number when this object can be safely occluded
    /// Used by contain modules when showing/hiding contained objects
    pub fn set_safe_occlusion_frame(&mut self, frame: UnsignedInt) {
        self.safe_occlusion_frame = frame;
    }

    pub(super) fn update_partition_object_position(&self) {
        if crate::object_manager::is_resetting() {
            return;
        }

        if let Ok(mut manager) = crate::object_manager::get_object_manager().try_write() {
            manager.update_object_position(self.id, *self.get_position());
        }
    }

    pub(super) fn handle_shroud(&mut self) {
        self.unlook();
        self.unshroud();
        self.shroud();
        self.look();
    }

    pub(super) fn handle_value_map(&mut self) {
        self.remove_value();
        self.add_value();
    }

    pub(super) fn handle_threat_map(&mut self) {
        self.remove_threat();
        self.add_threat();
    }

    pub(super) fn look(&mut self) {
        // Wave 264: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        if !self.partition_last_look.is_invalid() {
            warn!("Object {} look called without unlook", self.id);
            return;
        }

        let Some(controller) = self.get_controlling_player() else {
            return;
        };
        if self.is_destroyed() || self.is_effectively_dead() {
            return;
        }

        if let Some(container_id) = self.get_container_id() {
            let not_garrisonable = crate::object::registry::OBJECT_REGISTRY
                .with_object(container_id, |container_guard| {
                    let Some(contain) = container_guard.get_contain() else {
                        return false;
                    };
                    // Rust-only re-entrancy guard: look() can run while the
                    // caller already holds this contain mutex (attach paths);
                    // a blocking lock would self-deadlock. Under contention
                    // the caller is mid-attach, so let the reveal proceed.
                    contain
                        .try_lock()
                        .ok()
                        .map(|contain_guard| !contain_guard.is_garrisonable())
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if not_garrisonable {
                return;
            }
        }

        let shroud_clearing_range = self.get_shroud_clearing_range();
        if shroud_clearing_range > 0.0 {
            let mut looking_mask = PlayerMaskType::none();

            // C++ Object::look: KINDOF_REVEAL_TO_ALL uses PLAYERMASK_ALL so every
            // player gets the unit's normal shroud-clearing range.
            if self.is_kind_of(KindOf::RevealToAll) {
                looking_mask = crate::common::PLAYERMASK_ALL;
            } else if let (Ok(controller_guard), Ok(list)) =
                (controller.read(), player_list().read())
            {
                for current_player_arc in list.iter() {
                    let Ok(current_player) = current_player_arc.read() else {
                        continue;
                    };

                    let is_allied = Self::controller_relationship_to_player_default_team(
                        &controller_guard,
                        &current_player,
                    ) == Relationship::Allies;

                    if is_allied {
                        looking_mask |= current_player.get_player_mask();
                    }
                }
                looking_mask |= self.vision_spied_mask;
            } else {
                looking_mask |= self.vision_spied_mask;
            }

            if let Some(partition) = crate::helpers::ThePartitionManager::get() {
                let pos = *self.get_position();
                partition.do_shroud_reveal(&pos, shroud_clearing_range, looking_mask);
                self.partition_last_look.where_pos = pos;
                self.partition_last_look.for_whom = looking_mask;
                self.partition_last_look.how_far = shroud_clearing_range;
            }
        }

        let shroud_reveal_to_all_range = self.get_template().get_shroud_reveal_to_all_range();
        if shroud_reveal_to_all_range > 0.0
            && !self.test_status(ObjectStatusTypes::UnderConstruction)
        {
            let stealthed_and_not_detected = self.test_status(ObjectStatusTypes::Stealthed)
                && !self.test_status(ObjectStatusTypes::Detected)
                && !self.test_status(ObjectStatusTypes::Disguised);
            if !stealthed_and_not_detected {
                let mut players_mask = PlayerMaskType::none();
                if let (Ok(controller_guard), Ok(list)) = (controller.read(), player_list().read())
                {
                    for current_player_arc in list.iter() {
                        let Ok(current_player) = current_player_arc.read() else {
                            continue;
                        };
                        let relationship = Self::controller_relationship_to_player_default_team(
                            &controller_guard,
                            &current_player,
                        );
                        if matches!(relationship, Relationship::Enemies | Relationship::Neutral) {
                            players_mask |= current_player.get_player_mask();
                        }
                    }
                }

                if let Some(partition) = crate::helpers::ThePartitionManager::get() {
                    let pos = *self.get_position();
                    partition.do_shroud_reveal(&pos, shroud_reveal_to_all_range, players_mask);
                    self.partition_reveal_all_last_look.where_pos = pos;
                    self.partition_reveal_all_last_look.for_whom = players_mask;
                    self.partition_reveal_all_last_look.how_far = shroud_reveal_to_all_range;
                }
            }
        }
    }

    pub(super) fn unlook(&mut self) {
        if self.partition_last_look.is_invalid() {
            return;
        }

        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            partition.queue_undo_shroud_reveal(
                &self.partition_last_look.where_pos,
                self.partition_last_look.how_far,
                self.partition_last_look.for_whom,
            );
        }
        self.partition_last_look.reset();

        if !self.partition_reveal_all_last_look.is_invalid() {
            if let Some(partition) = crate::helpers::ThePartitionManager::get() {
                partition.queue_undo_shroud_reveal(
                    &self.partition_reveal_all_last_look.where_pos,
                    self.partition_reveal_all_last_look.how_far,
                    self.partition_reveal_all_last_look.for_whom,
                );
            }
            self.partition_reveal_all_last_look.reset();
        }
    }

    pub(super) fn shroud(&mut self) {
        if !self.partition_last_shroud.is_invalid() {
            warn!("Object {} shroud called without unshroud", self.id);
            return;
        }

        let Some(controller) = self.get_controlling_player() else {
            return;
        };

        if self.test_status(ObjectStatusTypes::UnderConstruction)
            || self.is_effectively_dead()
            || self.get_shroud_range() <= 0.0
        {
            return;
        }

        let mut shrouding_mask = PlayerMaskType::none();
        if let (Ok(controller_guard), Ok(list)) = (controller.read(), player_list().read()) {
            for current_player_arc in list.iter() {
                let Ok(current_player) = current_player_arc.read() else {
                    continue;
                };
                let relationship = Self::controller_relationship_to_player_default_team(
                    &controller_guard,
                    &current_player,
                );
                if relationship != Relationship::Allies {
                    shrouding_mask |= current_player.get_player_mask();
                }
            }
        }

        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            let pos = *self.get_position();
            partition.do_shroud_cover(&pos, self.get_shroud_range(), shrouding_mask);
            self.partition_last_shroud.where_pos = pos;
            self.partition_last_shroud.for_whom = shrouding_mask;
            self.partition_last_shroud.how_far = self.get_shroud_range();
        }
    }

    pub(super) fn unshroud(&mut self) {
        if self.partition_last_shroud.is_invalid() {
            return;
        }

        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            partition.undo_shroud_cover(
                &self.partition_last_shroud.where_pos,
                self.partition_last_shroud.how_far,
                self.partition_last_shroud.for_whom,
            );
        }
        self.partition_last_shroud.reset();
    }

    pub(super) fn add_value(&mut self) {
        if !self.partition_last_value.is_invalid() {
            warn!("Object {} add_value called without remove_value", self.id);
            return;
        }
        let Some(controller) = self.get_controlling_player() else {
            return;
        };
        if self.test_status(ObjectStatusTypes::UnderConstruction)
            || self.is_effectively_dead()
            || self.get_shroud_clearing_range() <= 0.0
        {
            return;
        }

        let Ok(controller_guard) = controller.read() else {
            return;
        };
        let pos = *self.get_position();
        let value = self.get_template().get_build_cost().max(0) as u32;
        self.partition_last_value.where_pos = pos;
        self.partition_last_value.data = value;
        self.partition_last_value.for_whom = controller_guard.get_player_mask();
        self.partition_last_value.how_far = self.get_vision_range();

        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            partition.do_value_affect(
                &self.partition_last_value.where_pos,
                self.partition_last_value.how_far,
                self.partition_last_value.data,
                self.partition_last_value.for_whom,
            );
        }
    }

    pub(super) fn remove_value(&mut self) {
        if self.partition_last_value.is_invalid() {
            return;
        }
        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            partition.undo_value_affect(
                &self.partition_last_value.where_pos,
                self.partition_last_value.how_far,
                self.partition_last_value.data,
                self.partition_last_value.for_whom,
            );
        }
        self.partition_last_value.reset();
    }

    pub(super) fn add_threat(&mut self) {
        if !self.partition_last_threat.is_invalid() {
            warn!("Object {} add_threat called without remove_threat", self.id);
            return;
        }
        let Some(controller) = self.get_controlling_player() else {
            return;
        };
        if self.test_status(ObjectStatusTypes::UnderConstruction)
            || self.is_effectively_dead()
            || self.get_shroud_clearing_range() <= 0.0
        {
            return;
        }

        let Ok(controller_guard) = controller.read() else {
            return;
        };
        let pos = *self.get_position();
        self.partition_last_threat.where_pos = pos;
        self.partition_last_threat.data = self.get_template().get_threat_value();
        self.partition_last_threat.for_whom = controller_guard.get_player_mask();
        self.partition_last_threat.how_far = self.get_vision_range();

        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            partition.do_threat_affect(
                &self.partition_last_threat.where_pos,
                self.partition_last_threat.how_far,
                self.partition_last_threat.data,
                self.partition_last_threat.for_whom,
            );
        }
    }

    pub(super) fn remove_threat(&mut self) {
        if self.partition_last_threat.is_invalid() {
            return;
        }
        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            partition.undo_threat_affect(
                &self.partition_last_threat.where_pos,
                self.partition_last_threat.how_far,
                self.partition_last_threat.data,
                self.partition_last_threat.for_whom,
            );
        }
        self.partition_last_threat.reset();
    }

    /// Handle partition cell maintenance
    /// Called when object position/visibility changes to refresh shroud and influence maps.
    pub fn handle_partition_cell_maintenance(&mut self) {
        self.update_partition_object_position();
        self.handle_shroud();
        self.handle_value_map();
        self.handle_threat_map();
    }

    /// Set visibility flag for a specific player
    /// Called by rendering system to update visibility based on ShroudManager
    pub fn set_visibility_for_player(&mut self, player_id: UnsignedInt, visible: bool) {
        if player_id < MAX_PLAYER_COUNT as UnsignedInt {
            self.visibility_flags[player_id as usize] = visible;
        }
    }

    /// Set visibility alpha for a specific player (for smooth transitions)
    /// Called by rendering system for fading effects
    pub fn set_visibility_alpha_for_player(&mut self, player_id: UnsignedInt, alpha: f32) {
        if player_id < MAX_PLAYER_COUNT as UnsignedInt {
            // Clamp alpha to 0.0-1.0 range
            let idx = player_id as usize;
            let clamped = alpha.max(0.0).min(1.0);
            self.visibility_alpha[idx] = clamped;
            if clamped <= 0.0 {
                self.visibility_flags[idx] = false;
            } else if clamped >= 1.0 {
                self.visibility_flags[idx] = true;
            }
        }
    }

    /// Update visibility flags for all players based on current ShroudManager state
    /// Called periodically by rendering system for efficiency
    pub fn update_visibility_for_all_players(&mut self, frame: UnsignedInt) -> Result<(), String> {
        use crate::object_manager::get_object_manager;
        use crate::system::shroud_manager::get_shroud_manager;

        // Skip if already updated this frame
        if self.last_visibility_update_frame == frame {
            return Ok(());
        }

        let shroud = get_shroud_manager();
        let shroud_mgr = shroud
            .lock()
            .map_err(|_| "ShroudManager poisoned".to_string())?;

        // Update visibility for all players
        for player_id in 0..MAX_PLAYER_COUNT {
            let visible = shroud_mgr.can_see_object(player_id as UnsignedInt, self.id);
            self.visibility_flags[player_id] = visible;
            // Default alpha: fully visible if seen, invisible otherwise
            self.visibility_alpha[player_id] = if visible { 1.0 } else { 0.0 };
        }

        self.last_visibility_update_frame = frame;
        Ok(())
    }

    /// Smoothly interpolate visibility alpha for fade-in/out effects
    /// Used for gradient fog-of-war transitions between visibility states
    ///
    /// # Arguments
    /// * `player_id` - Which player's visibility to update
    /// * `target_alpha` - Target alpha value (0.0-1.0)
    /// * `transition_speed` - Speed of transition (0.0-1.0), higher = faster
    pub fn interpolate_visibility_alpha(
        &mut self,
        player_id: UnsignedInt,
        target_alpha: f32,
        transition_speed: f32,
    ) {
        if player_id >= MAX_PLAYER_COUNT as UnsignedInt {
            return;
        }

        let idx = player_id as usize;
        let target = target_alpha.max(0.0).min(1.0);
        let speed = transition_speed.max(0.0).min(1.0);

        let current = self.visibility_alpha[idx];
        let delta = target - current;
        if delta.abs() <= speed {
            self.visibility_alpha[idx] = target;
        } else {
            self.visibility_alpha[idx] = current + delta.signum() * speed;
        }

        let alpha = self.visibility_alpha[idx];
        if alpha <= 0.0 {
            self.visibility_flags[idx] = false;
        } else if alpha >= 1.0 {
            self.visibility_flags[idx] = true;
        }
    }

    /// Set gradient falloff strength for this object
    /// Higher values create sharper visibility transitions (like distance-based fade)
    /// Lower values create smoother transitions (like gradual fog-of-war)
    pub fn set_visibility_falloff(&mut self, falloff: f32) {
        // Falloff clamped to reasonable range [0.5 - 3.0]
        // 0.5 = very smooth gradient
        // 1.0 = linear gradient (default)
        // 3.0 = very sharp edge
        // Stored for shader use
        let falloff_clamped = falloff.max(0.5).min(3.0);
        // Would be stored in shader uniform if we had object-specific uniform tracking
        // For now, documented for future shader integration
        let _ = falloff_clamped;
    }

    /// Check if object is in transition between visibility states
    /// Used for rendering to determine if fade effects should be applied
    pub fn is_visibility_transitioning(&self, player_id: UnsignedInt) -> bool {
        if player_id >= MAX_PLAYER_COUNT as UnsignedInt {
            return false;
        }
        let idx = player_id as usize;
        let alpha = self.visibility_alpha[idx];
        // Transitioning if not fully visible (1.0) and not fully hidden (0.0)
        alpha > 0.0 && alpha < 1.0
    }

    /// Register this object in the partition manager for spatial queries
    /// C++ Reference: Object.cpp - Partition manager registration
    ///
    /// # Returns
    /// * `Ok(())` - Registered successfully
    /// * `Err(ObjectError)` - Failed to register
    pub fn register_in_partition_manager(&mut self) -> Result<(), ObjectError> {
        // Register in the object manager's spatial partition.
        let manager = crate::object_manager::get_object_manager();
        if let Ok(mut guard) = manager.write() {
            guard.update_object_position(self.id, self.geometry_info.position);
        }

        Ok(())
    }
}
