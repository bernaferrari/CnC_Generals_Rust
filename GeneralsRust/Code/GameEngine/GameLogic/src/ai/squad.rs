use crate::ai::group::AIGroup;
use crate::ai::object_registry::{
    get_legacy_object, register_legacy_object, unregister_legacy_object,
};
use crate::common::xfer::XferExt;
use crate::common::*;
use crate::object::*;
use crate::team::Team;
use game_engine::common::system::{Snapshotable, Xfer};

use std::sync::{Arc, RwLock, Weak};

/// Vector of object IDs
pub type VecObjectID = Vec<ObjectID>;

/// Vector of object pointers
pub type VecObjectPtr = Vec<Arc<RwLock<Object>>>;

/// Squad represents a collection of objects for AI targeting and management
///
/// Squads are different from Teams and AIGroups:
/// - Teams are for high-level organization and scripting
/// - AIGroups are for movement and pathfinding coordination
/// - Squads are for targeting and tactical operations
pub struct Squad {
    /// Object IDs in this squad
    object_ids: Vec<ObjectID>,
    /// Cached objects (updated when requested)
    objects_cached: Vec<Arc<RwLock<Object>>>,
}

impl std::fmt::Debug for Squad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Squad")
            .field("object_ids_len", &self.object_ids.len())
            .field("objects_cached_len", &self.objects_cached.len())
            .finish()
    }
}

impl Squad {
    /// Create a new empty squad
    pub fn new() -> Self {
        Self {
            object_ids: Vec::new(),
            objects_cached: Vec::new(),
        }
    }

    /// Add an object to the squad
    pub fn add_object(&mut self, object: &Arc<RwLock<Object>>) {
        if let Ok(obj_ref) = object.try_read() {
            self.add_object_id(obj_ref.get_id());
        }
        register_legacy_object(object);
    }

    /// Add an object by ID to the squad
    pub fn add_object_id(&mut self, object_id: ObjectID) {
        if self.object_ids.contains(&object_id) {
            return;
        }
        self.object_ids.push(object_id);
        // Drop stale Arc cache; rebuild on demand from IDs.
        self.objects_cached.clear();
    }

    /// Remove an object from the squad
    pub fn remove_object(&mut self, object: &Arc<RwLock<Object>>) {
        if let Ok(obj_ref) = object.try_read() {
            self.remove_object_id(obj_ref.get_id());
        }
    }

    /// Remove an object by ID from the squad
    pub fn remove_object_id(&mut self, object_id: ObjectID) {
        self.object_ids.retain(|&id| id != object_id);
        unregister_legacy_object(object_id);
        self.objects_cached.clear();
    }

    /// Clear all objects from the squad
    pub fn clear_squad(&mut self) {
        self.object_ids.clear();
        self.objects_cached.clear();
    }

    /// Get all objects in the squad that haven't been deleted.
    /// Always rebuilds from stored ObjectIDs (ID-first membership).
    pub fn get_all_objects(&mut self) -> &Vec<Arc<RwLock<Object>>> {
        self.objects_cached.clear();
        let mut valid_ids = Vec::new();

        for &obj_id in &self.object_ids {
            if let Some(obj) = self.find_object_by_id(obj_id) {
                self.objects_cached.push(obj);
                valid_ids.push(obj_id);
            }
        }

        self.object_ids = valid_ids;
        &self.objects_cached
    }

    /// Get all live objects (selectable and not effectively dead)
    pub fn get_live_objects(&mut self) -> Vec<Arc<RwLock<Object>>> {
        // First get all objects
        self.get_all_objects();

        // Filter to only live, selectable objects
        let mut live_objects = Vec::new();
        for obj in &self.objects_cached {
            if let Ok(obj_ref) = obj.try_read() {
                if obj_ref.is_selectable() && !obj_ref.is_effectively_dead() {
                    live_objects.push(obj.clone());
                }
            }
        }

        live_objects
    }

    /// Get all live object IDs (best effort when object handles are missing)
    pub fn get_live_object_ids(&mut self) -> Vec<ObjectID> {
        self.get_live_objects()
            .into_iter()
            .filter_map(|obj| obj.try_read().ok().map(|guard| guard.get_id()))
            .collect()
    }

    /// Get the current number of objects, including dead objects
    pub fn get_size_of_group(&self) -> usize {
        self.object_ids.len()
    }

    /// Check if an object is on this squad
    pub fn is_on_squad(&self, object: &Arc<RwLock<Object>>) -> bool {
        if let Ok(obj_ref) = object.try_read() {
            let obj_id = obj_ref.get_id();
            self.object_ids.iter().any(|&id| id == obj_id)
        } else {
            false
        }
    }

    /// Check if an object ID is on this squad
    pub fn is_on_squad_by_id(&self, object_id: ObjectID) -> bool {
        self.object_ids.iter().any(|&id| id == object_id)
    }

    /// Fill this squad with members of a team
    ///
    /// Note: There should NOT be a team_from_squad function as Teams are entirely
    /// a construct to work with the AI. Since things can only be on one Team at a time,
    /// creating a Team from an arbitrary Squad will cause weird, difficult to reproduce bugs.
    pub fn squad_from_team(&mut self, team: &Team, clear_squad_first: bool) {
        if clear_squad_first {
            self.clear_squad();
        }

        self.object_ids = team.get_members().iter().copied().collect();
        self.objects_cached.clear();
    }

    /// Fill this squad with members of an AIGroup
    pub fn squad_from_ai_group(&mut self, ai_group: &AIGroup, clear_squad_first: bool) {
        if clear_squad_first {
            self.clear_squad();
        }

        self.object_ids = ai_group.get_all_ids_snapshot();
        self.objects_cached.clear();
    }

    /// Create an AIGroup from this squad
    /// When creating the AIGroup from the Squad, the old AIGroup affiliations are broken
    pub fn ai_group_from_squad(&mut self, ai_group: &mut AIGroup) -> Result<(), String> {
        // Remove all existing members from the AI group
        // Implementation would clear the AI group first

        // Add all live squad members to the AI group
        for object_id in self.get_live_object_ids() {
            ai_group.add_by_id(object_id)?;
        }

        Ok(())
    }

    /// Get all object IDs in the squad
    pub fn get_object_ids(&self) -> &Vec<ObjectID> {
        &self.object_ids
    }

    /// Get cached objects (may be stale - use get_all_objects for fresh data)
    pub fn get_cached_objects(&self) -> &Vec<Arc<RwLock<Object>>> {
        &self.objects_cached
    }

    /// Check if squad is empty
    pub fn is_empty(&self) -> bool {
        self.object_ids.is_empty()
    }

    /// Get the center position of all objects in the squad
    pub fn get_center_position(&mut self) -> Option<Coord3D> {
        let objects = self.get_all_objects();
        if objects.is_empty() {
            return None;
        }

        let mut center = Coord3D::new(0.0, 0.0, 0.0);
        let mut count = 0;

        for obj in objects {
            if let Ok(obj_ref) = obj.try_read() {
                let pos = obj_ref.get_position();
                center.x += pos.x;
                center.y += pos.y;
                center.z += pos.z;
                count += 1;
            }
        }

        if count > 0 {
            center.x /= count as f32;
            center.y /= count as f32;
            center.z /= count as f32;
            Some(center)
        } else {
            None
        }
    }

    /// Get the bounding box of all objects in the squad
    pub fn get_bounding_box(&mut self) -> Option<(Coord3D, Coord3D)> {
        let objects = self.get_all_objects();
        if objects.is_empty() {
            return None;
        }

        let mut min_pos = Coord3D::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max_pos = Coord3D::new(f32::MIN, f32::MIN, f32::MIN);
        let mut found_any = false;

        for obj in objects {
            if let Ok(obj_ref) = obj.try_read() {
                let pos = obj_ref.get_position();
                min_pos.x = min_pos.x.min(pos.x);
                min_pos.y = min_pos.y.min(pos.y);
                min_pos.z = min_pos.z.min(pos.z);
                max_pos.x = max_pos.x.max(pos.x);
                max_pos.y = max_pos.y.max(pos.y);
                max_pos.z = max_pos.z.max(pos.z);
                found_any = true;
            }
        }

        if found_any {
            Some((min_pos, max_pos))
        } else {
            None
        }
    }

    /// Count live objects in the squad
    pub fn count_live_objects(&mut self) -> usize {
        self.get_live_objects().len()
    }

    /// Get objects of a specific type from the squad
    pub fn get_objects_of_type(&mut self, object_type: &str) -> Vec<Arc<RwLock<Object>>> {
        let mut matching_objects = Vec::new();

        for obj in self.get_all_objects() {
            if let Ok(obj_ref) = obj.try_read() {
                if obj_ref.get_template_name() == object_type {
                    matching_objects.push(obj.clone());
                }
            }
        }

        matching_objects
    }

    /// Check if squad contains any objects of a specific type
    pub fn contains_type(&mut self, object_type: &str) -> bool {
        for obj in self.get_all_objects() {
            if let Ok(obj_ref) = obj.try_read() {
                if obj_ref.get_template_name() == object_type {
                    return true;
                }
            }
        }
        false
    }

    /// Get the strongest object in the squad (by health and damage potential)
    pub fn get_strongest_object(&mut self) -> Option<Arc<RwLock<Object>>> {
        let objects = self.get_live_objects();
        if objects.is_empty() {
            return None;
        }

        let mut strongest: Option<Arc<RwLock<Object>>> = None;
        let mut best_score = 0.0f32;

        for obj in objects {
            if let Ok(obj_ref) = obj.try_read() {
                // Simple scoring: health + damage potential
                let health = obj_ref.get_health_percentage();
                let damage = obj_ref.get_max_damage_potential();
                let score = health * 0.5 + damage * 0.5;

                if strongest.is_none() || score > best_score {
                    best_score = score;
                    strongest = Some(obj.clone());
                }
            }
        }

        strongest
    }

    /// Get the weakest object in the squad (by health)
    pub fn get_weakest_object(&mut self) -> Option<Arc<RwLock<Object>>> {
        let objects = self.get_live_objects();
        if objects.is_empty() {
            return None;
        }

        let mut weakest: Option<Arc<RwLock<Object>>> = None;
        let mut lowest_health = f32::MAX;

        for obj in objects {
            if let Ok(obj_ref) = obj.try_read() {
                let health = obj_ref.get_health_percentage();

                if weakest.is_none() || health < lowest_health {
                    lowest_health = health;
                    weakest = Some(obj.clone());
                }
            }
        }

        weakest
    }

    // Private helper methods

    /// Find object by ID
    fn find_object_by_id(&self, obj_id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        get_legacy_object(obj_id)
    }
}

impl Default for Squad {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Squad {
    fn clone(&self) -> Self {
        Self {
            object_ids: self.object_ids.clone(),
            objects_cached: Vec::new(), // Don't clone cached objects, they'll be rebuilt
        }
    }
}

impl Snapshotable for Squad {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc Squad version: {:?}", e))?;
        let mut object_count = self.object_ids.len() as u16;
        xfer.xfer_unsigned_short(&mut object_count)
            .map_err(|e| format!("Failed to crc Squad object count: {:?}", e))?;
        for &object_id in &self.object_ids {
            let mut id = object_id;
            xfer.xfer_object_id(&mut id)
                .map_err(|e| format!("Failed to crc Squad object id: {:?}", e))?;
        }
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer Squad version: {:?}", e))?;

        let mut object_count = self.object_ids.len() as u16;
        xfer.xfer_unsigned_short(&mut object_count)
            .map_err(|e| format!("Failed to xfer Squad object count: {:?}", e))?;

        if xfer.is_loading() {
            if !self.objects_cached.is_empty() {
                return Err("Squad::xfer - objects_cached should be empty on load".to_string());
            }

            self.object_ids.clear();
            for _ in 0..object_count {
                let mut object_id = crate::common::INVALID_ID;
                xfer.xfer_object_id(&mut object_id)
                    .map_err(|e| format!("Failed to xfer Squad object id: {:?}", e))?;
                self.object_ids.push(object_id);
            }
        } else {
            for &object_id in &self.object_ids {
                let mut id = object_id;
                xfer.xfer_object_id(&mut id)
                    .map_err(|e| format!("Failed to xfer Squad object id: {:?}", e))?;
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
