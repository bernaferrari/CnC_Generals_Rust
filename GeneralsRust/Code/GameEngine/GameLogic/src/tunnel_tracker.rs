//! Tunnel tracker system - Rust port of C++ TunnelTracker
//!
//! Manages tunnel network systems for GLA faction.
//! Author: Graham Smallwood, March 2002 (C++ version)
//! Rust conversion: 2025
//!
//! Matches C++ TunnelTracker.cpp from GeneralsMD/Code/GameEngine/Source/Common/RTS/

use crate::common::{GameResult, KindOf, ObjectID, ObjectStatusTypes, INVALID_ID};
use crate::damage::DamageInfo;
use crate::object::drawable::DrawableArcExt;
use crate::object::Object;
use crate::system::game_logic::get_game_logic;
use game_engine::common::system::snapshot::Snapshotable;
use game_engine::common::system::xfer::{Xfer, XferMode, XferVersion};
use std::sync::{Arc, Mutex, RwLock};

/// Tracks the objects associated with a single tunnel network.
/// This system allows units to move between tunnel entrances across the map.
#[derive(Debug, Clone)]
pub struct TunnelTracker {
    /// List of tunnel entrance object IDs
    tunnel_ids: Vec<ObjectID>,
    /// Number of active tunnels
    tunnel_count: u32,
    /// Objects currently in the tunnel network (stable IDs; resolve for op duration).
    contained_ids: Vec<ObjectID>,
    /// Object IDs read from a save before load post-processing resolves them.
    xfer_contain_list: Vec<ObjectID>,
    /// Size of contained list (maintained separately for save/load)
    contain_list_size: usize,
    /// Current nemesis (enemy unit being tracked)
    cur_nemesis_id: ObjectID,
    /// Frame when nemesis was last updated (expires after 4 seconds)
    nemesis_timestamp: u32,
    /// Maximum capacity for this tunnel network
    max_capacity: i32,
}

impl TunnelTracker {
    /// Create a new tunnel tracker.
    /// Matches C++ TunnelTracker::TunnelTracker()
    pub fn new() -> Self {
        Self {
            tunnel_ids: Vec::new(),
            tunnel_count: 0,
            contained_ids: Vec::new(),
            xfer_contain_list: Vec::new(),
            contain_list_size: 0,
            cur_nemesis_id: INVALID_ID,
            nemesis_timestamp: 0,
            max_capacity: -1, // -1 means unlimited
        }
    }

    /// Set maximum capacity for this tunnel network
    pub fn set_max_capacity(&mut self, max: i32) {
        self.max_capacity = max;
    }

    /// Check if an object can be contained in the tunnel network.
    /// Matches C++ TunnelTracker::isValidContainerFor (TunnelTracker.cpp:132-150)
    pub fn is_valid_container_for(
        &self,
        object: &Object,
        check_capacity: bool,
    ) -> GameResult<bool> {
        // October 11, 2002 -- ALL units can use tunnels except aircraft
        // (Matches C++ comment lines 134-135)
        if object.is_kind_of(KindOf::Aircraft) {
            return Ok(false);
        }

        if check_capacity {
            let contain_max = self.get_contain_max()?;
            let contain_count = self.get_contain_count()?;
            Ok(contain_max <= 0 || (contain_count as i32) < contain_max)
        } else {
            Ok(true)
        }
    }

    /// Update the current nemesis (enemy unit being targeted).
    /// Matches C++ TunnelTracker::updateNemesis (TunnelTracker.cpp:87-100)
    pub fn update_nemesis(&mut self, target: Option<&Object>) -> GameResult<()> {
        let current_frame = get_current_frame()?;

        if self.get_cur_nemesis()?.is_none() {
            if let Some(target_ref) = target {
                // Only track vehicles, structures, infantry, or aircraft
                if target_ref.is_kind_of(KindOf::Vehicle)
                    || target_ref.is_kind_of(KindOf::Structure)
                    || target_ref.is_kind_of(KindOf::Infantry)
                    || target_ref.is_kind_of(KindOf::Aircraft)
                {
                    self.cur_nemesis_id = target_ref.get_id();
                    self.nemesis_timestamp = current_frame;
                }
            }
        } else if let Some(_current_nemesis) = self.get_cur_nemesis()? {
            if let Some(target_ref) = target {
                // Update timestamp if target matches our current nemesis by ID
                if target_ref.get_id() == self.cur_nemesis_id {
                    self.nemesis_timestamp = current_frame;
                }
            }
        }

        Ok(())
    }

    /// Get the current nemesis object if still valid.
    /// Matches C++ TunnelTracker::getCurNemesis (TunnelTracker.cpp:103-129)
    pub fn get_cur_nemesis(&mut self) -> GameResult<Option<Arc<RwLock<Object>>>> {
        if self.cur_nemesis_id == INVALID_ID {
            return Ok(None);
        }

        let current_frame = get_current_frame()?;
        const LOGICFRAMES_PER_SECOND: u32 = 30; // Standard game logic update rate

        // Nemesis expires after 4 seconds (matches C++ line 108)
        if self.nemesis_timestamp + 4 * LOGICFRAMES_PER_SECOND < current_frame {
            self.cur_nemesis_id = INVALID_ID;
            return Ok(None);
        }

        // Find the target object
        if let Some(target) = find_object_by_id(self.cur_nemesis_id)? {
            let target_read = target.read().map_err(|_| "Target lock poisoned")?;

            // If the enemy unit is stealthed and not detected, can't attack it
            if target_read.test_status(ObjectStatusTypes::Stealthed)
                && !target_read.test_status(ObjectStatusTypes::Detected)
                && !target_read.test_status(ObjectStatusTypes::Disguised)
            {
                drop(target_read);
                self.cur_nemesis_id = INVALID_ID;
                return Ok(None);
            }

            // If target is effectively dead, clear it
            if target_read.is_effectively_dead() {
                drop(target_read);
                self.cur_nemesis_id = INVALID_ID;
                return Ok(None);
            }

            drop(target_read);
            Ok(Some(target))
        } else {
            self.cur_nemesis_id = INVALID_ID;
            Ok(None)
        }
    }

    /// Add an object to the contained list.
    /// Matches C++ TunnelTracker::addToContainList (TunnelTracker.cpp:153-157)
    /// ID-first contain membership.
    pub fn add_to_contain_list(&mut self, object_id: ObjectID) -> GameResult<()> {
        if object_id == INVALID_ID {
            return Ok(());
        }
        if self.contained_ids.contains(&object_id) {
            return Ok(());
        }
        if self.max_capacity > 0 && (self.contained_ids.len() as i32) >= self.max_capacity {
            return Err("TunnelTracker capacity reached".into());
        }
        self.contained_ids.push(object_id);
        self.contain_list_size = self.contained_ids.len();
        Ok(())
    }

    pub fn add_to_contain_list_id(&mut self, object_id: ObjectID) -> GameResult<()> {
        self.add_to_contain_list(object_id)
    }

    /// Remove an object from the contained list.
    /// Matches C++ TunnelTracker::removeFromContain (TunnelTracker.cpp:160-171)
    pub fn remove_from_contain(
        &mut self,
        object_id: ObjectID,
        _expose_stealth_units: bool,
    ) -> GameResult<()> {
        let initial_len = self.contained_ids.len();
        self.contained_ids.retain(|&id| id != object_id);
        if self.contained_ids.len() < initial_len {
            self.contain_list_size = self.contained_ids.len();
        }
        Ok(())
    }

    pub fn remove_from_contain_id(&mut self, object_id: ObjectID) -> GameResult<()> {
        self.remove_from_contain(object_id, false)
    }

    /// Check whether the specified object is contained.
    /// Matches C++ TunnelTracker::isInContainer (TunnelTracker.cpp:174-177)
    pub fn is_in_container(&self, object_id: ObjectID) -> bool {
        self.contained_ids.contains(&object_id)
    }

    pub fn is_in_container_id(&self, object_id: ObjectID) -> bool {
        self.is_in_container(object_id)
    }

    /// Register that a tunnel object has been created.
    /// Matches C++ TunnelTracker::onTunnelCreated (TunnelTracker.cpp:180-184)
    pub fn on_tunnel_created(&mut self, new_tunnel: &Object) -> GameResult<()> {
        self.on_tunnel_created_id(new_tunnel.get_id())
    }

    /// Register that a tunnel object ID has been created.
    pub fn on_tunnel_created_id(&mut self, tunnel_id: ObjectID) -> GameResult<()> {
        self.tunnel_count += 1;
        if !self.tunnel_ids.contains(&tunnel_id) {
            self.tunnel_ids.push(tunnel_id);
        }
        Ok(())
    }

    /// Register that a tunnel object has been destroyed.
    /// Handles critical tunnel network destruction logic.
    /// Matches C++ TunnelTracker::onTunnelDestroyed (TunnelTracker.cpp:187-212)
    pub fn on_tunnel_destroyed(&mut self, dead_tunnel: &Object) -> GameResult<()> {
        self.on_tunnel_destroyed_id(dead_tunnel.get_id())
    }

    /// Register that a tunnel object ID has been destroyed.
    pub fn on_tunnel_destroyed_id(&mut self, dead_tunnel_id: ObjectID) -> GameResult<()> {
        self.tunnel_count = self.tunnel_count.saturating_sub(1);
        self.tunnel_ids.retain(|&id| id != dead_tunnel_id);

        if self.tunnel_count == 0 {
            // Kill everyone in the contain list - cave in! (Matches C++ lines 192-198)
            // Snapshot IDs to avoid iterator invalidation
            let objects_to_destroy: Vec<ObjectID> = self.contained_ids.clone();

            for object_id in objects_to_destroy {
                // C++ lines 217-220: Notify object before destruction
                // obj->onRemovedFrom(obj->getContainedBy())
                if let Some(obj) = find_object_by_id(object_id)? {
                    if let Ok(mut obj_guard) = obj.write() {
                        if let Some(container_id) = obj_guard.get_contained_by() {
                            if let Some(container) = find_object_by_id(container_id)? {
                                let _ = obj_guard.on_removed_from(container);
                            }
                        }
                    }
                    destroy_object(obj)?;
                }
            }

            self.contained_ids.clear();
            self.contain_list_size = 0;
        } else {
            // C++ lines 200-211: Make sure nobody inside remembers the dead tunnel as the one they entered
            // (scripts need to use so there must be something valid here)
            if let Some(&valid_tunnel_id) = self.tunnel_ids.first() {
                if let Some(valid_tunnel) = find_object_by_id(valid_tunnel_id)? {
                    // C++ lines 204-210: Update contained objects to point to valid tunnel
                    for &object_id in &self.contained_ids {
                        if let Some(obj) = find_object_by_id(object_id)? {
                            if let Ok(mut obj_guard) = obj.write() {
                                // C++ line 208-209: if(obj->getContainedBy() == deadTunnel) obj->onContainedBy(validTunnel)
                                if let Some(container_id) = obj_guard.get_contained_by() {
                                    if container_id == dead_tunnel_id {
                                        let _ = obj_guard.on_contained_by(valid_tunnel.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Heal all objects within the tunnel network.
    /// Matches C++ TunnelTracker::healObjects (TunnelTracker.cpp:224-228)
    pub fn heal_objects(&mut self, frames: f32) -> GameResult<()> {
        let ids = self.contained_ids.clone();
        for object_id in ids {
            self.heal_object(object_id, frames)?;
        }
        Ok(())
    }

    /// Heal one object within the tunnel network.
    /// Matches C++ TunnelTracker::healObject (TunnelTracker.cpp:231-271)
    fn heal_object(&self, object_id: ObjectID, frames_for_full_heal: f32) -> GameResult<()> {
        let Some(obj) = find_object_by_id(object_id)? else {
            return Ok(());
        };

        let obj_read = obj.read().map_err(|_| "Object lock poisoned")?;

        let body_module = match obj_read.get_body_module() {
            Some(body) => body,
            None => return Ok(()), // No body module, nothing to heal
        };

        // C++ line 248: TheGameLogic->getFrame() - obj->getContainedByFrame()
        let current_frame = get_current_frame()?;
        let contained_by_frame = obj_read.get_contained_by_frame();
        let frames_contained = current_frame.saturating_sub(contained_by_frame);

        let body_guard = body_module
            .lock()
            .map_err(|_| "Body module lock poisoned")?;
        let max_health = body_guard.get_max_health();
        drop(body_guard);
        drop(obj_read);

        // Prepare healing damage info
        let mut heal_info = DamageInfo::new();
        heal_info.input.damage_type = crate::damage::DamageType::Healing;
        heal_info.input.death_type = crate::damage::DeathType::None;

        if frames_contained as f32 >= frames_for_full_heal {
            // Been in long enough - set to max health (matches C++ lines 248-256)
            heal_info.input.amount = max_health;
        } else {
            // Gradual healing based on time contained (matches C++ lines 258-269)
            heal_info.input.amount = max_health / frames_for_full_heal;
        }
        heal_info.sync_from_input();

        // Apply healing
        if let Some(body_module) = obj
            .read()
            .map_err(|_| "Object lock poisoned")?
            .get_body_module()
        {
            let mut body_guard = body_module
                .lock()
                .map_err(|_| "Body module lock poisoned")?;
            body_guard.attempt_healing(&mut heal_info)?;
        }

        Ok(())
    }

    /// Iterate over contained objects.
    /// Matches C++ TunnelTracker::iterateContained (TunnelTracker.cpp:42-78)
    pub fn iterate_contained<F>(&self, mut func: F, reverse: bool) -> GameResult<()>
    where
        F: FnMut(Arc<RwLock<Object>>) -> GameResult<()>,
    {
        // Snapshot IDs to handle iterator invalidation during callback
        // (matches C++ note about handling deletion via callback, lines 46-47)
        let mut ids = self.contained_ids.clone();
        if reverse {
            ids.reverse();
        }

        for object_id in ids {
            if let Some(object) = find_object_by_id(object_id)? {
                func(object)?;
            }
        }

        Ok(())
    }

    /// ID-first contained iteration.
    pub fn iterate_contained_ids<F>(&self, mut func: F, reverse: bool) -> GameResult<()>
    where
        F: FnMut(ObjectID) -> GameResult<()>,
    {
        let mut ids = self.contained_ids.clone();
        if reverse {
            ids.reverse();
        }
        for object_id in ids {
            func(object_id)?;
        }
        Ok(())
    }

    /// Number of contained objects.
    pub fn get_contain_count(&self) -> GameResult<u32> {
        Ok(self.contained_ids.len() as u32)
    }

    /// Maximum capacity allowed for this tracker.
    /// Matches C++ TunnelTracker::getContainMax (TunnelTracker.cpp:81-84)
    pub fn get_contain_max(&self) -> GameResult<i32> {
        // C++ line 83: return TheGlobalData->m_maxTunnelCapacity
        if let Some(global_data) = crate::helpers::TheGlobalData::get() {
            return Ok(global_data.get_max_tunnel_capacity());
        }
        // Fallback to configured max_capacity
        Ok(self.max_capacity)
    }

    /// Retrieve a reference to the contained objects list.
    /// Resolve contained members for the duration of the caller (owned Arc vec, not stored).
    pub fn get_contained_items_list(&self) -> Vec<Arc<RwLock<Object>>> {
        self.contained_ids
            .iter()
            .filter_map(|&id| find_object_by_id(id).ok().flatten())
            .collect()
    }

    pub fn get_contained_item_ids(&self) -> &[ObjectID] {
        &self.contained_ids
    }

    /// Obtain the list of tunnel container IDs.
    pub fn get_container_list(&self) -> GameResult<Vec<ObjectID>> {
        Ok(self.tunnel_ids.clone())
    }

    /// Get number of active tunnels in the network.
    pub fn get_tunnel_count(&self) -> u32 {
        self.tunnel_count
    }

    /// Check if this tracker contains a container (tunnel entrance) with the given ID.
    pub fn contains_container(&self, object_id: ObjectID) -> bool {
        self.tunnel_ids.contains(&object_id)
    }
}

impl Default for TunnelTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshotable for TunnelTracker {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ TunnelTracker::crc is intentionally empty.
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |res: std::io::Result<()>| res.map_err(|err| err.to_string());

        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer_io(xfer.xfer_version(&mut version, current_version))?;

        if xfer.is_reading() {
            self.tunnel_ids.clear();
        }
        xfer_io(xfer.xfer_stl_object_id_list(&mut self.tunnel_ids))?;

        let mut contain_list_size = self.contained_ids.len() as i32;
        xfer_io(xfer.xfer_int(&mut contain_list_size))?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.contain_list_size = contain_list_size.max(0) as usize;
            self.xfer_contain_list.clear();
            self.contained_ids.clear();
        } else {
            self.contain_list_size = self.contained_ids.len();
        }

        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for &object_id in &self.contained_ids {
                    let mut object_id = object_id;
                    xfer_io(xfer.xfer_object_id(&mut object_id))?;
                }
            }
            XferMode::Load => {
                for _ in 0..self.contain_list_size {
                    let mut object_id: ObjectID = INVALID_ID;
                    xfer_io(xfer.xfer_object_id(&mut object_id))?;
                    self.xfer_contain_list.push(object_id);
                }
            }
            XferMode::Invalid => {
                return Err("TunnelTracker::xfer - invalid xfer mode".to_string());
            }
        }

        xfer_io(xfer.xfer_unsigned_int(&mut self.tunnel_count))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if !self.contained_ids.is_empty() {
            return Err(
                "TunnelTracker::loadPostProcess - contain list should be empty but is not"
                    .to_string(),
            );
        }

        for object_id in self.xfer_contain_list.drain(..) {
            let object = find_object_by_id(object_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| {
                    format!(
                        "TunnelTracker::loadPostProcess - unable to find object ID '{}'",
                        object_id
                    )
                })?;

            {
                let mut guard = object
                    .write()
                    .map_err(|_| "TunnelTracker::loadPostProcess object lock poisoned")?;
                guard.leave_group();
                let pos = *guard.get_position();
                let _ = crate::ai::integration::with_ai_integration_mut(|manager| {
                    manager.remove_pathfinding_obstacle(object_id, &[pos])
                });
            }

            if let Ok(guard) = object.read() {
                if let Some(drawable) = guard.get_drawable() {
                    drawable.set_drawable_hidden(true);
                }
            }

            self.contained_ids.push(object_id);
        }

        self.contain_list_size = self.contained_ids.len();
        Ok(())
    }
}

/// Helper function to get current game frame
fn get_current_frame() -> GameResult<u32> {
    if let Ok(logic) = get_game_logic().lock() {
        Ok(logic.get_frame())
    } else {
        Err("Failed to lock game logic".into())
    }
}

/// Helper function to find object by ID
fn find_object_by_id(id: ObjectID) -> GameResult<Option<Arc<RwLock<Object>>>> {
    if id == INVALID_ID {
        return Ok(None);
    }

    if let Ok(logic) = get_game_logic().lock() {
        Ok(logic.find_object_by_id(id))
    } else {
        Err("Failed to lock game logic".into())
    }
}

/// Helper function to destroy an object
/// Matches C++ TunnelTracker::destroyObject (TunnelTracker.cpp:215-221)
fn destroy_object(obj: Arc<RwLock<Object>>) -> GameResult<()> {
    if let Ok(mut logic_mutex) = get_game_logic().lock() {
        let object_id = obj.read().map_err(|_| "Object lock poisoned")?.get_id();

        if object_id != INVALID_ID {
            logic_mutex.destroy_object(object_id);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::system::xfer_load::XferLoad;
    use game_engine::system::xfer_save::XferSave;
    use std::io::Cursor;

    #[test]
    fn test_tunnel_tracker_creation() {
        let tracker = TunnelTracker::new();
        assert_eq!(tracker.tunnel_count, 0);
        assert_eq!(tracker.contain_list_size, 0);
        assert_eq!(tracker.cur_nemesis_id, INVALID_ID);
    }

    #[test]
    fn test_tunnel_capacity() {
        let mut tracker = TunnelTracker::new();
        tracker.set_max_capacity(10);
        let expected_capacity = crate::helpers::TheGlobalData::get()
            .map(|global| global.get_max_tunnel_capacity())
            .unwrap_or(10);
        assert_eq!(tracker.get_contain_max().unwrap(), expected_capacity);
    }

    #[test]
    fn test_tunnel_destruction_clears_network() {
        let mut tracker = TunnelTracker::new();
        tracker.tunnel_count = 1;
        tracker.contain_list_size = 5;

        // Simulating destruction of last tunnel would clear contained objects
        // (actual test would require mock objects)
    }

    #[test]
    fn tunnel_tracker_xfer_preserves_tunnel_ids_and_count() {
        let mut original = TunnelTracker::new();
        original.tunnel_ids = vec![11, 22];
        original.tunnel_count = 2;

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            original.xfer(&mut save).unwrap();
        }

        let mut loaded = TunnelTracker::new();
        {
            let cursor = Cursor::new(bytes.as_slice());
            let mut load = XferLoad::new(cursor, 1);
            loaded.xfer(&mut load).unwrap();
        }

        assert_eq!(loaded.tunnel_ids, vec![11, 22]);
        assert_eq!(loaded.tunnel_count, 2);
        assert!(loaded.xfer_contain_list.is_empty());
    }
}
