use crate::ai::ai_group::{
    STD_AIRCRAFT_EXTRA_MARGIN, STD_WAYPOINT_CLAMP_MARGIN, clamp_waypoint_position,
    get_helicopter_offset,
};
use crate::ai::{AiCommandParams, AiCommandType, GUICommandType};
use crate::attack::{AbleToAttackType, CanAttackResult};

use crate::action_manager::TheActionManager;
use crate::ai::the_ai;
use crate::common::Snapshot;
use crate::common::command::*;
use crate::common::coord::*;
use crate::common::science::SCIENCE_INVALID;
use crate::common::xfer::{Xfer, XferExt};
use crate::common::*;
use crate::control_bar::get_control_bar_bridge;
use crate::damage::*;
use crate::formation::{
    FormationCommand, FormationGroup, FormationManager, FormationSettings, FormationType,
};
use crate::helpers::TheGameLogic;
use crate::modules::{AIAttitudeType, AIUpdateInterfaceExt, ContainModuleInterfaceExt};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_module::SpecialPowerCommandOptions;
use crate::object::special_power_template::get_special_power_store;
use crate::object::special_power_types::SpecialPowerType;
use crate::object::*;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::path::*;
use crate::player::Player;
use crate::polygon_trigger::PolygonTrigger;
use crate::special_power::*;
use crate::team::Team;
use crate::terrain::get_terrain_logic;
use crate::upgrade::UpgradeTemplate;
use crate::upgrade::center::get_upgrade_center;
use crate::waypoint::*;
use crate::weapon::{WeaponLockType, WeaponSetType, WeaponSlotType};
use game_engine::common::system::build_assistant;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// An "AIGroup" is a simple collection of AI objects, used by the AI
/// for such things as Group Pathfinding.
pub struct AIGroup {
    /// Unique ID for this group
    id: u32,
    /// Member object IDs (stable; resolve via OBJECT_REGISTRY for the duration of an op).
    member_list: Vec<ObjectID>,
    /// Cached size of member list
    member_list_size: usize,
    /// Maximum speed of group (slowest member)
    speed: f32,
    /// "Dirty bit" - if true then group speed needs recomputation
    dirty: bool,
    /// Group ground path
    ground_path: Option<Arc<Mutex<Path>>>,
    /// Cached ID list for returning by reference
    last_requested_id_list: Vec<ObjectID>,
    /// Formation ID for this group (if in formation)
    formation_id: Option<u32>,
    /// Formation type
    formation_type: FormationType,
    /// Formation manager reference (shared across all groups)
    formation_manager: Option<Arc<Mutex<FormationManager>>>,
}

impl AIGroup {
    /// Create new AIGroup with given ID
    pub fn new(id: u32) -> Self {
        Self {
            id,
            member_list: Vec::new(),
            member_list_size: 0,
            speed: 0.0,
            dirty: false,
            ground_path: None,
            last_requested_id_list: Vec::new(),
            formation_id: None,
            formation_type: FormationType::None,
            formation_manager: None,
        }
    }

    /// Wave 253: host-only path has no dual-world factory objects.
    #[inline]
    fn dual_world_registry_unavailable() -> bool {
        OBJECT_REGISTRY.is_empty()
    }

    /// Create new AIGroup with formation manager
    pub fn new_with_formation(id: u32, formation_manager: Arc<Mutex<FormationManager>>) -> Self {
        Self {
            id,
            member_list: Vec::new(),
            member_list_size: 0,
            speed: 0.0,
            dirty: false,
            ground_path: None,
            last_requested_id_list: Vec::new(),
            formation_id: None,
            formation_type: FormationType::None,
            formation_manager: Some(formation_manager),
        }
    }

    /// Return this group's unique ID
    pub fn get_id(&self) -> u32 {
        self.id
    }

    /// Return the group IDs for every member in this group
    pub fn get_all_ids(&mut self) -> &Vec<ObjectID> {
        self.prune_dead_members();
        self.last_requested_id_list.clear();
        self.last_requested_id_list
            .extend_from_slice(&self.member_list);
        &self.last_requested_id_list
    }

    /// Return a snapshot of member IDs without mutating cached state
    pub fn get_all_ids_snapshot(&self) -> Vec<ObjectID> {
        // Wave 253: empty dual-world registry → no live members resolve.
        if Self::dual_world_registry_unavailable() {
            return Vec::new();
        }

        self.member_list
            .iter()
            .copied()
            .filter(|id| OBJECT_REGISTRY.contains(*id))
            .collect()
    }

    /// Return the speed of the group's slowest member
    pub fn get_speed(&mut self) -> f32 {
        if self.dirty {
            self.recompute();
        }
        self.speed
    }

    /// Return true if object is in this group
    pub fn is_member(&self, obj: &Arc<RwLock<Object>>) -> bool {
        obj.try_read()
            .ok()
            .map(|guard| self.is_member_id(guard.get_id()))
            .unwrap_or(false)
    }

    /// ID-first membership test.
    pub fn is_member_id(&self, object_id: ObjectID) -> bool {
        self.member_list.contains(&object_id)
    }

    fn prune_dead_members(&mut self) {
        // Wave 253: empty dual-world registry → drop all dual-world member ids.
        if Self::dual_world_registry_unavailable() {
            if !self.member_list.is_empty() {
                self.member_list.clear();
                self.member_list_size = 0;
                self.dirty = true;
            }
            return;
        }

        self.member_list.retain(|id| OBJECT_REGISTRY.contains(*id));
        self.member_list_size = self.member_list.len();
    }

    /// Add object to group
    /// Only allow AI agents into the group

    /// Borrow-first group membership: resolve `OBJECT_REGISTRY` once at the group boundary.
    /// Prefer this over cloning Arc handles at each command_processor call site.
    /// Add object to group by stable ID (primary membership API).
    pub fn add_by_id(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = OBJECT_REGISTRY
            .get_object(object_id)
            .or_else(|| crate::helpers::TheGameLogic::find_object_by_id(object_id))
            .ok_or_else(|| format!("Object {object_id} not in registry"))?;

        {
            let obj_ref = obj.try_read().map_err(|_| "Could not lock object")?;

            // Check if object has AIUpdateInterface or is a valid structure
            let has_ai = obj_ref.get_ai_update_interface().is_some();
            let is_structure = obj_ref.is_any_kind_of(&[KindOf::Structure]);
            let is_always_selectable = obj_ref.is_any_kind_of(&[KindOf::AlwaysSelectable]);

            if !has_ai && !is_structure && !is_always_selectable {
                return Err("Object is not AI-capable or valid for group".to_string());
            }
            if obj_ref.get_id() != object_id && object_id == crate::object::INVALID_ID {
                return Err("Object has invalid id".to_string());
            }
        }

        if object_id == crate::object::INVALID_ID {
            return Err("Object has invalid id".to_string());
        }
        if self.member_list.contains(&object_id) {
            return Ok(());
        }

        // Store stable ID; resolve only for the duration of an operation.
        self.member_list.push(object_id);
        self.member_list_size += 1;

        // Tell object to enter this group
        if let Ok(mut obj_ref) = obj.try_write() {
            obj_ref.enter_group(self);
        }

        // List has changed, properties need recomputation
        self.dirty = true;
        Ok(())
    }

    /// Arc convenience: extract ID and add.
    pub fn add(&mut self, obj: Arc<RwLock<Object>>) -> Result<(), String> {
        let object_id = obj
            .try_read()
            .map_err(|_| "Could not lock object")?
            .get_id();
        self.add_by_id(object_id)
    }

    /// Remove object from group
    /// Returns true if group was destroyed due to emptiness
    pub fn remove(&mut self, obj: &Arc<RwLock<Object>>) -> Result<bool, String> {
        let object_id = obj
            .try_read()
            .map_err(|_| "Could not lock object")?
            .get_id();
        self.remove_by_id(object_id)
    }

    /// Remove member by stable object id.
    pub fn remove_by_id(&mut self, object_id: ObjectID) -> Result<bool, String> {
        let index = self
            .member_list
            .iter()
            .position(|&id| id == object_id)
            .ok_or("Object not found in group")?;

        self.member_list.remove(index);
        self.member_list_size = self.member_list.len();

        let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj_ref| {
            obj_ref.leave_group();
        });

        self.dirty = true;
        Ok(self.is_empty())
    }

    /// Check if group contains any objects not owned by the specified player
    pub fn contains_any_objects_not_owned_by_player(&self, owner_player: &Player) -> bool {
        // Wave 253: empty dual-world → nothing foreign.
        if self.member_list_size == 0 || Self::dual_world_registry_unavailable() {
            return false;
        }

        let owner_id = owner_player.get_player_index() as UnsignedInt;
        for &member_id in &self.member_list {
            let foreign = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                obj_ref.get_controlling_player_id() != Some(owner_id)
            });
            if foreign == Some(true) {
                return true;
            }
        }
        false
    }

    /// Remove any objects that aren't owned by the player
    /// Returns true if the group was destroyed due to emptiness
    pub fn remove_any_objects_not_owned_by_player(&mut self, owner_player: &Player) -> bool {
        // Wave 253: empty dual-world → nothing to remove.
        if self.member_list_size == 0 || Self::dual_world_registry_unavailable() {
            return false;
        }

        let mut ids_to_remove = Vec::new();
        let owner_id = owner_player.get_player_index() as UnsignedInt;

        for &member_id in &self.member_list {
            let foreign = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                obj_ref.get_controlling_player_id() != Some(owner_id)
            });
            if foreign == Some(true) {
                ids_to_remove.push(member_id);
            }
        }

        for member_id in ids_to_remove {
            if self.remove_by_id(member_id).unwrap_or(false) {
                return true;
            }
        }

        false
    }

    /// Compute the centroid of the group
    pub fn get_center(&self) -> Option<Coord3D> {
        // Wave 253: empty dual-world → no member positions.
        if self.member_list_size == 0 || Self::dual_world_registry_unavailable() {
            return None;
        }

        let mut count = 0;
        let mut center = Coord3D::new(0.0, 0.0, 0.0);

        // First pass - try to use only AI objects
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if obj_ref.is_disabled_by_type(DisabledType::Held) {
                    return; // Don't count riders in center calculation
                }

                if obj_ref.get_ai_update_interface().is_some() {
                    let pos = obj_ref.get_position();
                    center.x += pos.x;
                    center.y += pos.y;
                    center.z += pos.z;
                    count += 1;
                }
            });
        }

        // If no AI objects found, use all objects
        if count == 0 && !self.member_list.is_empty() {
            for &member_id in &self.member_list {
                let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                    if obj_ref.is_disabled_by_type(DisabledType::Held) {
                        return; // Don't count riders in center calculation
                    }

                    let pos = obj_ref.get_position();
                    center.x += pos.x;
                    center.y += pos.y;
                    center.z += pos.z;
                    count += 1;
                });
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

    /// Get min/max bounds and center, returns true if group is in formation
    pub fn get_min_max_and_center(&self) -> Option<(Coord2D, Coord2D, Coord3D, bool)> {
        // Wave 253: empty dual-world → empty extents.
        if self.member_list_size == 0 || Self::dual_world_registry_unavailable() {
            return None;
        }

        let mut count = 0;
        let mut min = Coord2D::new(f32::MAX, f32::MAX);
        let mut max = Coord2D::new(f32::MIN, f32::MIN);
        let mut center = Coord3D::new(0.0, 0.0, 0.0);
        let mut formation_id: Option<FormationID> = None;

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if obj_ref.is_disabled_by_type(DisabledType::Held) {
                    return; // Don't count riders in center calculation
                }

                if obj_ref.get_ai_update_interface().is_some() {
                    let pos = obj_ref.get_position();
                    center.x += pos.x;
                    center.y += pos.y;
                    center.z += pos.z;

                    // Calculate bounding coordinates
                    min.x = min.x.min(pos.x);
                    max.x = max.x.max(pos.x);
                    min.y = min.y.min(pos.y);
                    max.y = max.y.max(pos.y);

                    let cur_id = obj_ref.get_formation_id();
                    if count == 0 {
                        formation_id = Some(cur_id);
                    } else if formation_id.map_or(false, |id| id != cur_id) {
                        formation_id = None;
                    }

                    count += 1;
                }
            });
        }

        if count > 0 {
            center.x /= count as f32;
            center.y /= count as f32;
            center.z /= count as f32;

            let is_formation = formation_id.map(|id| !id.is_none()).unwrap_or(false) && count >= 2;
            Some((min, max, center, is_formation))
        } else {
            None
        }
    }

    /// Return the number of objects in the group
    pub fn get_count(&self) -> usize {
        self.member_list_size
    }

    /// Returns true if the group has no members
    pub fn is_empty(&self) -> bool {
        self.member_list_size == 0
    }

    /// Given a destination location, compute the destination position for
    /// this object such that it keeps its relative position with the group.
    /// Matches C++ AIGroup::computeIndividualDestination
    pub fn compute_individual_destination(
        &self,
        object_id: ObjectID,
        group_dest: &Coord3D,
        center: &Coord3D,
        is_formation: bool,
    ) -> Option<Coord3D> {
        // Wave 253: empty dual-world → passthrough destination.
        if Self::dual_world_registry_unavailable() {
            let _ = (object_id, center, is_formation);
            return Some(*group_dest);
        }

        let obj = OBJECT_REGISTRY
            .get_object(object_id)
            .or_else(|| crate::helpers::TheGameLogic::find_object_by_id(object_id))?;
        let obj_guard = obj.try_read().ok()?;

        // Compute vector from "group center" to self
        let pos = obj_guard.get_position();
        let mut v = if is_formation {
            obj_guard.get_formation_offset()
        } else {
            Coord2D::new(pos.x - center.x, pos.y - center.y)
        };

        let mut length = (v.x * v.x + v.y * v.y).sqrt();
        let max_length = 6.0 * obj_guard.get_geometry_info().get_bounding_circle_radius();
        if length > max_length {
            length = max_length;
        }

        // Normalize and scale
        if length > 0.001 {
            v.x /= length;
            v.y /= length;
            v.x *= length;
            v.y *= length;
        }

        // Move to same offset at destination
        let mut dest = Coord3D::new(group_dest.x + v.x, group_dest.y + v.y, 0.0);

        // Get terrain layer for destination
        let layer = crate::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(|t| t.get_layer_for_destination(group_dest))
            .unwrap_or(crate::path::PathfindLayerEnum::Ground);

        // Set Z coordinate based on layer
        if let Ok(terrain) = crate::terrain::get_terrain_logic().read() {
            dest.z = terrain.get_layer_height(dest.x, dest.y, layer, None, true);
        }

        // Adjust destination for ground movement if object has AI
        // Note: The full adjustment requires mutable access to AI which we can't get while holding obj_guard
        // The pathfinder adjustment is a best-effort simplification here
        drop(obj_guard);

        Some(dest)
    }

    /// Recompute group speed and other properties
    fn recompute(&mut self) {
        // Wave 253: empty dual-world → no speeds to sample.
        if Self::dual_world_registry_unavailable() {
            self.member_list.clear();
            self.member_list_size = 0;
            self.speed = 0.0;
            self.dirty = false;
            return;
        }

        self.speed = f32::MAX;
        let mut found_any = false;

        // Drop destroyed members while computing speed
        self.prune_dead_members();

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    let obj_speed = ai.get_speed();
                    if obj_speed < self.speed {
                        self.speed = obj_speed;
                    }
                    found_any = true;
                }
            });
        }

        if !found_any {
            self.speed = 0.0;
        }

        self.dirty = false;
    }

    /// Mark group for recomputation
    pub fn recompute_group_speed(&mut self) {
        self.dirty = true;
    }

    // Group movement commands
    pub fn group_move_to_position(
        &self,
        pos: &Coord3D,
        add_waypoint: bool,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        // C++ AIGroup::groupMoveToPosition — centroid/formation, column paths,
        // click-to-gather, held/immobile filters, near-to-far sort.
        let mut goal = *pos;
        let (min, max, mut center, mut is_formation) = match self.get_min_max_and_center() {
            Some(v) => v,
            None => return,
        };

        if add_waypoint {
            is_formation = false;
        }

        let mut did_infantry = false;
        let mut did_vehicles = false;
        if !add_waypoint && !is_formation {
            if let Some(path) = self.friend_compute_ground_path(&goal, cmd_source) {
                did_infantry = self.friend_move_infantry_to_pos(&goal, cmd_source, &path);
                did_vehicles = self.friend_move_vehicle_to_pos(&goal, cmd_source, &path);
            }
        }

        // Click-to-gather: player click inside scaled group rect → tighten.
        let mut tighten_group = false;
        if !is_formation && matches!(cmd_source, CommandSourceType::FromPlayer) {
            let gather_factor = game_engine::common::ini::get_global_data()
                .map(|d| d.read().group_move_click_to_gather_factor)
                .unwrap_or(0.0);
            if gather_factor > 0.0 {
                let mut smin = min;
                let mut smax = max;
                let cx = (smin.x + smax.x) * 0.5;
                let cy = (smin.y + smax.y) * 0.5;
                let hx = (smax.x - smin.x) * 0.5 * gather_factor;
                let hy = (smax.y - smin.y) * 0.5 * gather_factor;
                smin.x = cx - hx;
                smax.x = cx + hx;
                smin.y = cy - hy;
                smax.y = cy + hy;
                if goal.x >= smin.x && goal.x <= smax.x && goal.y >= smin.y && goal.y <= smax.y {
                    tighten_group = true;
                }
            }
        }

        let mut extra_margin = 0.0_f32;
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj| {
                if obj.is_kind_of(KindOf::ProducedAtHelipad) {
                    is_formation = false;
                    extra_margin = extra_margin.max(obj.get_geometry_info().get_major_radius());
                } else if obj.is_kind_of(KindOf::Aircraft) {
                    if let Some(ai) = obj.get_ai_update_interface() {
                        if let Ok(ai_guard) = ai.lock() {
                            if !ai_guard.is_doing_ground_movement() {
                                tighten_group = false;
                                is_formation = false;
                            }
                        }
                    }
                    extra_margin = extra_margin.max(STD_AIRCRAFT_EXTRA_MARGIN);
                }
            });
        }
        // C++ AIGroup.cpp:1592-1593 — clamp click onto playable map.
        clamp_waypoint_position(&mut goal, STD_WAYPOINT_CLAMP_MARGIN + extra_margin);

        if tighten_group {
            is_formation = false;
            if !add_waypoint {
                let cell = PATHFIND_CELL_SIZE_F;
                let dx = ((max.x - min.x) / cell) as i32;
                let dy = ((max.x - min.x) / cell) as i32;
                let cells = dx * dy;
                if cells < 2000 {
                    self.group_tighten_to_position(&goal, false, cmd_source);
                    return;
                }
            }
        }

        if is_formation {
            let path = self.friend_compute_ground_path(&goal, cmd_source);
            self.friend_move_formation_to_pos(&goal, cmd_source, path.as_deref());
            return;
        }

        let mut movers: Vec<(ObjectID, f32)> = Vec::new();
        for &member_id in &self.member_list {
            let Some(key) = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    if obj.is_disabled_by_type(DisabledType::Held) {
                        return None;
                    }
                    if obj.is_kind_of(KindOf::Immobile) {
                        return None;
                    }
                    if obj.get_ai_update_interface().is_none() {
                        return None;
                    }
                    if did_infantry && obj.is_kind_of(KindOf::Infantry) {
                        return None;
                    }
                    if did_vehicles && obj.is_kind_of(KindOf::Vehicle) {
                        if let Some(ai) = obj.get_ai_update_interface() {
                            if let Ok(ai_guard) = ai.lock() {
                                if ai_guard.is_doing_ground_movement()
                                    && !obj.is_kind_of(KindOf::CliffJumper)
                                {
                                    return None;
                                }
                            }
                        }
                    }
                    let unit_pos = obj.get_position();
                    let dx = unit_pos.x - goal.x;
                    let dy = unit_pos.y - goal.y;
                    Some(dx * dx + dy * dy)
                })
                .flatten()
            else {
                continue;
            };
            movers.push((member_id, key));
        }
        movers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut first_unit = true;
        let mut goal_pos = goal;
        for (member_id, _) in movers {
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj| {
                obj.set_formation_id(FormationID::NONE);
            });

            if first_unit {
                if is_formation {
                    if let Some(v) =
                        OBJECT_REGISTRY.with_object(member_id, |obj| obj.get_formation_offset())
                    {
                        goal_pos.x -= v.x;
                        goal_pos.y -= v.y;
                    }
                } else if let Some(p) =
                    OBJECT_REGISTRY.with_object(member_id, |obj| *obj.get_position())
                {
                    center = p;
                }
                first_unit = false;
            }

            let Some(dest) =
                self.compute_individual_destination(member_id, &goal_pos, &center, is_formation)
            else {
                continue;
            };

            let _ = OBJECT_REGISTRY.with_object(member_id, |obj| {
                let Some(ai) = obj.get_ai_update_interface() else {
                    return;
                };
                if !add_waypoint {
                    ai.ai_move_to_position(&dest, false, cmd_source);
                } else {
                    ai.ai_follow_path_append(&dest, cmd_source);
                }
            });
        }
    }

    /// C++ `AIGroup::groupTightenToPosition` — near-to-far, helipad ring offsets.
    pub fn group_tighten_to_position(
        &self,
        pos: &Coord3D,
        add_waypoint: bool,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        let mut movers: Vec<(ObjectID, f32)> = Vec::new();
        for &member_id in &self.member_list {
            let Some(key) = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    if obj.is_disabled_by_type(DisabledType::Held)
                        || obj.is_kind_of(KindOf::Immobile)
                        || obj.get_ai_update_interface().is_none()
                    {
                        return None;
                    }
                    let p = obj.get_position();
                    let dx = p.x - pos.x;
                    let dy = p.y - pos.y;
                    Some(dx * dx + dy * dy)
                })
                .flatten()
            else {
                continue;
            };
            movers.push((member_id, key));
        }
        movers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut heli_idx = 0i32;
        for (member_id, _) in movers {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj| {
                let Some(ai) = obj.get_ai_update_interface() else {
                    return;
                };
                if add_waypoint {
                    ai.ai_follow_path_append(pos, cmd_source);
                    return;
                }
                if obj.is_kind_of(KindOf::ProducedAtHelipad) {
                    let mut heli_offs = *pos;
                    get_helicopter_offset(&mut heli_offs, heli_idx);
                    heli_idx += 1;
                    ai.ai_tighten_to_position(&heli_offs, CommandSourceType::FromAi);
                } else {
                    ai.ai_tighten_to_position(pos, cmd_source);
                }
            });
        }
    }

    pub fn group_move_to_and_evacuate(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_move_to_and_evacuate(pos, cmd_source);
                }
            });
        }
    }

    /// C++ `AIGroup::groupMoveToAndEvacuateAndExit`.
    pub fn group_move_to_and_evacuate_and_exit(
        &self,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
    ) {
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_move_to_and_evacuate_and_exit(pos, cmd_source);
                }
            });
        }
    }

    /// C++ `AIGroup::groupHunt`.
    pub fn group_hunt(&self, cmd_source: CommandSourceType) {
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_hunt(cmd_source);
                }
            });
        }
    }

    /// C++ `AIGroup::groupEnter`.
    pub fn group_enter(&self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_enter(obj_id, cmd_source);
                }
            });
        }
    }

    /// C++ `AIGroup::groupDock`.
    pub fn group_dock(&self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_dock(obj_id, cmd_source);
                }
            });
        }
    }

    /// C++ `AIGroup::groupExit`.
    pub fn group_exit(&self, object_to_exit: ObjectID, cmd_source: CommandSourceType) {
        let mut params = AiCommandParams::new(AiCommandType::Exit, cmd_source);
        params.obj = Some(object_to_exit);
        self.fan_ai_command(&params);
    }

    /// C++ `AIGroup::groupEvacuate` — airborne aircraft move-to-ground then unload.
    pub fn group_evacuate(&self, cmd_source: CommandSourceType) {
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    if obj_ref.is_kind_of(KindOf::Aircraft) && obj_ref.is_airborne_target() {
                        let mut pos = *obj_ref.get_position();
                        if let Ok(terrain) = get_terrain_logic().read() {
                            pos.z = terrain.get_ground_height(pos.x, pos.y, None);
                        }
                        ai.ai_move_to_and_evacuate(&pos, cmd_source);
                    } else {
                        let params = AiCommandParams::new(AiCommandType::Evacuate, cmd_source);
                        let _ = ai.execute_command(&params);
                    }
                } else if obj_ref.is_kind_of(KindOf::Structure) {
                    if let Some(contain) = obj_ref.get_contain() {
                        let _ = contain.order_all_passengers_to_exit(cmd_source, false);
                    }
                }
            });
        }
    }

    /// C++ `AIGroup::groupScatter` — spread members away from centroid.
    pub fn group_scatter(&self, cmd_source: CommandSourceType) {
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }
        let Some((_, _, center, _)) = self.get_min_max_and_center() else {
            return;
        };
        let mut movers: Vec<(ObjectID, f32)> = Vec::new();
        for &member_id in &self.member_list {
            let Some(key) = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    if obj.is_disabled_by_type(DisabledType::Held)
                        || obj.is_kind_of(KindOf::Immobile)
                        || obj.get_ai_update_interface().is_none()
                    {
                        return None;
                    }
                    let p = obj.get_position();
                    let dx = p.x - center.x;
                    let dy = p.y - center.y;
                    Some(dx * dx + dy * dy)
                })
                .flatten()
            else {
                continue;
            };
            movers.push((member_id, key));
        }
        movers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut scatter_center = center;
        for (member_id, _) in movers {
            scatter_center.x -= 0.01;
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj| {
                let Some(ai) = obj.get_ai_update_interface() else {
                    return;
                };
                let unit_pos = *obj.get_position();
                let mut dx = unit_pos.x - scatter_center.x;
                let mut dy = unit_pos.y - scatter_center.y;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.0001 {
                    dx /= len;
                    dy /= len;
                } else {
                    dx = 1.0;
                    dy = 0.0;
                }
                let radius = obj.get_geometry_info().get_bounding_circle_radius();
                let dest = Coord3D::new(
                    unit_pos.x + dx * 4.0 * radius,
                    unit_pos.y + dy * 4.0 * radius,
                    unit_pos.z,
                );
                ai.ai_move_to_position(&dest, false, cmd_source);
            });
        }
    }

    /// C++ `AIGroup::groupRepair`.
    pub fn group_repair(&self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        let mut params = AiCommandParams::new(AiCommandType::Repair, cmd_source);
        params.obj = Some(obj_id);
        self.fan_ai_command(&params);
    }

    /// C++ `AIGroup::groupResumeConstruction`.
    pub fn group_resume_construction(&self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        let mut params = AiCommandParams::new(AiCommandType::ResumeConstruction, cmd_source);
        params.obj = Some(obj_id);
        self.fan_ai_command(&params);
    }

    /// C++ `AIGroup::groupGetHealed`.
    pub fn group_get_healed(&self, heal_depot: ObjectID, cmd_source: CommandSourceType) {
        let mut params = AiCommandParams::new(AiCommandType::GetHealed, cmd_source);
        params.obj = Some(heal_depot);
        self.fan_ai_command(&params);
    }

    /// C++ `AIGroup::groupGetRepaired`.
    pub fn group_get_repaired(&self, repair_depot: ObjectID, cmd_source: CommandSourceType) {
        let mut params = AiCommandParams::new(AiCommandType::GetRepaired, cmd_source);
        params.obj = Some(repair_depot);
        self.fan_ai_command(&params);
    }

    /// C++ `AIGroup::groupGuardArea`.
    pub fn group_guard_area(
        &self,
        area: &PolygonTrigger,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    ) {
        let mut params = AiCommandParams::new(AiCommandType::GuardArea, cmd_source);
        params.polygon = Some(area.get_id());
        params.int_value = guard_mode.as_i32();
        self.fan_ai_command(&params);
    }

    /// C++ `AIGroup::groupAttackArea`.
    pub fn group_attack_area(&self, area: &PolygonTrigger, cmd_source: CommandSourceType) {
        let mut params = AiCommandParams::new(AiCommandType::AttackArea, cmd_source);
        params.polygon = Some(area.get_id());
        self.fan_ai_command(&params);
    }

    /// C++ `AIGroup::groupHackInternet`.
    pub fn group_hack_internet(&self, cmd_source: CommandSourceType) {
        let params = AiCommandParams::new(AiCommandType::HackInternet, cmd_source);
        self.fan_ai_command(&params);
    }

    /// C++ `AIGroup::groupDoSpecialPower`.
    pub fn group_do_special_power(&self, special_power_id: u32, command_options: u32) {
        self.do_special_power_common(special_power_id, command_options, None, None, 0.0);
    }

    /// C++ `AIGroup::groupDoSpecialPowerAtLocation`.
    pub fn group_do_special_power_at_location(
        &self,
        special_power_id: u32,
        location: &Coord3D,
        angle: f32,
        _object_in_way: Option<ObjectID>,
        command_options: u32,
    ) {
        self.do_special_power_common(
            special_power_id,
            command_options,
            Some(*location),
            None,
            angle,
        );
    }

    /// C++ `AIGroup::groupDoSpecialPowerAtObject`.
    pub fn group_do_special_power_at_object(
        &self,
        special_power_id: u32,
        target_id: ObjectID,
        command_options: u32,
    ) {
        self.do_special_power_common(
            special_power_id,
            command_options,
            None,
            Some(target_id),
            0.0,
        );
    }

    /// C++ `AIGroup::groupOverrideSpecialPowerDestination`.
    pub fn group_override_special_power_destination(
        &self,
        sp_type: SpecialPowerType,
        loc: &Coord3D,
        _cmd_source: CommandSourceType,
    ) {
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(module) =
                    obj_ref.find_special_power_with_overridable_destination_active(sp_type)
                {
                    if let Ok(mut guard) = module.lock() {
                        if let Some(sp) = guard.get_special_power_update_interface() {
                            sp.set_special_power_overridable_destination(loc);
                        }
                    }
                }
            });
        }
    }

    fn do_special_power_common(
        &self,
        special_power_id: u32,
        command_options: u32,
        location: Option<Coord3D>,
        target: Option<ObjectID>,
        _angle: f32,
    ) {
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }
        let Some(store) = get_special_power_store() else {
            return;
        };
        let Some(template) = store.find_special_power_template_by_id(special_power_id) else {
            return;
        };
        let template_name = template.get_name().to_string();
        let required_science = template.get_required_science();
        let options = SpecialPowerCommandOptions::from_bits_truncate(command_options);
        let member_ids: Vec<ObjectID> = self.member_list.clone();
        for member_id in member_ids {
            let allowed = OBJECT_REGISTRY
                .with_object(member_id, |obj_ref| {
                    if required_science != SCIENCE_INVALID {
                        let has_science = obj_ref
                            .get_controlling_player()
                            .and_then(|player| {
                                player.read().ok().map(|p| p.has_science(required_science))
                            })
                            .unwrap_or(false);
                        if !has_science {
                            return false;
                        }
                    }
                    if obj_ref
                        .get_special_power_module(template.get_id())
                        .is_none()
                    {
                        return false;
                    }
                    match (location, target) {
                        (Some(loc), _) => TheActionManager::can_do_special_power_at_location(
                            obj_ref,
                            &loc,
                            CommandSourceType::FromPlayer,
                            template,
                            None,
                            command_options,
                            true,
                        ),
                        (None, Some(tid)) => OBJECT_REGISTRY
                            .with_object(tid, |target_obj| {
                                TheActionManager::can_do_special_power_at_object(
                                    obj_ref,
                                    target_obj,
                                    CommandSourceType::FromPlayer,
                                    template,
                                    command_options,
                                    true,
                                )
                            })
                            .unwrap_or(false),
                        (None, None) => TheActionManager::can_do_special_power(
                            obj_ref,
                            template,
                            CommandSourceType::FromPlayer,
                            command_options,
                            true,
                        ),
                    }
                })
                .unwrap_or(false);
            if !allowed {
                continue;
            }
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| match (location, target) {
                (Some(loc), _) => obj_ref.do_special_power_at_location(
                    &template_name,
                    &loc,
                    crate::object_creation_list::nuggets::INVALID_ANGLE,
                    options,
                    false,
                ),
                (None, Some(tid)) => {
                    obj_ref.do_special_power_at_object(&template_name, tid, options, false)
                }
                (None, None) => obj_ref.do_special_power(&template_name, options, false),
            });
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj_ref| {
                obj_ref.friend_set_undetected_defector(false);
            });
        }
    }

    fn fan_ai_command(&self, params: &AiCommandParams) {
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    let _ = ai.execute_command(params);
                }
            });
        }
    }

    /// Start following the path from the given waypoint (matches C++ AIGroup::groupFollowWaypointPath).
    pub fn group_follow_waypoint_path(&self, way: &Waypoint, cmd_source: CommandSourceType) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_follow_waypoint_path(way, cmd_source);
                }
            });
        }
    }

    /// Start following the path exactly from the given waypoint (matches C++ AIGroup::groupFollowWaypointPathExact).
    pub fn group_follow_waypoint_path_exact(&self, way: &Waypoint, cmd_source: CommandSourceType) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_follow_waypoint_path_exact(way, cmd_source);
                }
            });
        }
    }

    /// Start following the path as a team (matches C++ AIGroup::groupFollowWaypointPathAsTeam).
    pub fn group_follow_waypoint_path_as_team(
        &self,
        way: &Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_follow_waypoint_path_as_team(way, cmd_source);
                }
            });
        }
    }

    /// Start following the path exactly as a team (matches C++ AIGroup::groupFollowWaypointPathAsTeamExact).
    pub fn group_follow_waypoint_path_as_team_exact(
        &self,
        way: &Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_follow_waypoint_path_exact_as_team(way, cmd_source);
                }
            });
        }
    }

    /// C++ `AIGroup::groupIdle` — AI idle + stealth mood delay, garrison stop, slaves.
    pub fn group_idle(&self, cmd_source: CommandSourceType) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_idle(cmd_source);
                    if matches!(cmd_source, CommandSourceType::FromPlayer)
                        && obj_ref.test_status(ObjectStatusTypes::CanStealth)
                        && !obj_ref.test_status(ObjectStatusTypes::Stealthed)
                        && !obj_ref.test_status(ObjectStatusTypes::Detected)
                    {
                        if let Some(stealth) = obj_ref.get_stealth() {
                            if let (Ok(stealth_guard), Ok(mut ai_guard)) =
                                (stealth.lock(), ai.lock())
                            {
                                let stealth_frames = stealth_guard.get_stealth_delay();
                                let random_frames =
                                    GameLogicRandomValue(0, LOGICFRAMES_PER_SECOND as i32) as u32;
                                ai_guard.set_next_mood_check_time(
                                    TheGameLogic::get_frame() + stealth_frames + random_frames,
                                );
                            }
                        }
                    }
                } else if let Some(contain) = obj_ref.get_contain() {
                    for passenger_id in contain.get_contained_objects() {
                        let _ = OBJECT_REGISTRY.with_object(passenger_id, |passenger| {
                            if let Some(pai) = passenger.get_ai_update_interface() {
                                pai.ai_idle(cmd_source);
                            }
                        });
                    }
                }
                if let Some(behavior) = obj_ref.get_spawn_behavior_interface_public() {
                    if let Ok(mut guard) = behavior.lock() {
                        if let Some(spawn) = guard.get_spawn_behavior_full_interface() {
                            let _ = spawn.order_slaves_to_go_idle(cmd_source);
                        }
                    }
                }
            });
        }
    }

    /// Tell all things in the group to toggle overcharge (matches C++ AIGroup::groupToggleOvercharge).
    pub fn group_toggle_overcharge(&self, _cmd_source: CommandSourceType) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                let _ = obj_ref.with_overcharge_behavior_interface(|overcharge| {
                    let _ = overcharge.toggle();
                });
            });
        }
    }

    /// Set surrender state for all members (matches C++ AIGroup::groupSurrender).
    #[cfg(feature = "allow_surrender")]
    pub fn group_surrender(
        &self,
        obj_we_surrendered_to: Option<ObjectID>,
        surrender: bool,
        _cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.try_lock() {
                        ai_guard.set_surrendered(obj_we_surrendered_to, surrender);
                    }
                }
            });
        }
    }

    /// Trigger a group cheer (matches C++ AIGroup::groupCheer).
    pub fn group_cheer(&self, _cmd_source: CommandSourceType) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj_ref| {
                obj_ref.set_special_model_condition_state(
                    MODELCONDITION_SPECIAL_CHEERING,
                    LOGICFRAMES_PER_SECOND * 3,
                );
            });
        }
    }

    /// Pick up a prisoner (matches C++ AIGroup::groupPickUpPrisoner).
    #[cfg(feature = "allow_surrender")]
    pub fn group_pick_up_prisoner(&self, prisoner_id: ObjectID, cmd_source: CommandSourceType) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        let prisoner_id = Some(prisoner_id);
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.try_lock() {
                        let mut params =
                            AiCommandParams::new(AiCommandType::PickUpPrisoner, cmd_source);
                        params.obj = prisoner_id;
                        let _ = ai_guard.execute_command(&params);
                    }
                }
            });
        }
    }

    /// Return prisoners to a prison (matches C++ AIGroup::groupReturnToPrison).
    #[cfg(feature = "allow_surrender")]
    pub fn group_return_to_prison(&self, prison_id: ObjectID, cmd_source: CommandSourceType) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        let prison_id = Some(prison_id);
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.try_lock() {
                        let mut params =
                            AiCommandParams::new(AiCommandType::ReturnPrisoners, cmd_source);
                        params.obj = prison_id;
                        let _ = ai_guard.execute_command(&params);
                    }
                }
            });
        }
    }

    /// Combat drop (matches C++ AIGroup::groupCombatDrop).
    pub fn group_combat_drop(
        &self,
        target_id: Option<ObjectID>,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.try_lock() {
                        let mut params =
                            AiCommandParams::new(AiCommandType::CombatDrop, cmd_source);
                        params.obj = target_id;
                        params.pos = *pos;
                        let _ = ai_guard.execute_command(&params);
                    }
                }
            });
        }
    }

    /// Issue a command button (matches C++ AIGroup::groupDoCommandButton).
    pub fn group_do_command_button(&self, button_id: u32, cmd_source: CommandSourceType) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj_ref| {
                let _ = obj_ref.do_command_button(button_id, cmd_source);
            });
        }
    }

    /// Issue a command button at a position (matches C++ AIGroup::groupDoCommandButtonAtPosition).
    pub fn group_do_command_button_at_position(
        &self,
        button_id: u32,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj_ref| {
                let _ = obj_ref.do_command_button_at_position(button_id, pos, cmd_source);
            });
        }
    }

    /// Issue a command button using waypoints (matches C++ AIGroup::groupDoCommandButtonUsingWaypoints).
    pub fn group_do_command_button_using_waypoints(
        &self,
        button_id: u32,
        way: &Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                let _ = obj_ref.do_command_button_using_waypoints(button_id, way, cmd_source);
            });
        }
    }

    /// Issue a command button at a target object (matches C++ AIGroup::groupDoCommandButtonAtObject).
    pub fn group_do_command_button_at_object(
        &self,
        button_id: u32,
        target_id: ObjectID,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(target_id, |target_ref| {
                let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj_ref| {
                    let _ = obj_ref.do_command_button_at_object(button_id, target_ref, cmd_source);
                });
            });
        }
    }

    pub fn group_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        self.group_attack_object_private(false, victim_id, max_shots_to_fire, cmd_source);
    }

    pub fn group_force_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        self.group_attack_object_private(true, victim_id, max_shots_to_fire, cmd_source);
    }

    fn group_attack_object_private(
        &self,
        forced: bool,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ returns immediately when the victim is already gone.
        if OBJECT_REGISTRY.with_object(victim_id, |_| ()).is_none() {
            return;
        }
        let Some(victim_pos) = OBJECT_REGISTRY.with_object(victim_id, |v| *v.get_position()) else {
            return;
        };
        let attack_type = if forced {
            AbleToAttackType::NewTargetForced
        } else {
            AbleToAttackType::NewTarget
        };
        let mut movers: Vec<(ObjectID, f32)> = Vec::new();
        for &member_id in &self.member_list {
            let Some(key) = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    if obj.is_disabled_by_type(DisabledType::Held) {
                        return None;
                    }
                    let p = obj.get_position();
                    let dx = p.x - victim_pos.x;
                    let dy = p.y - victim_pos.y;
                    Some(dx * dx + dy * dy)
                })
                .flatten()
            else {
                continue;
            };
            movers.push((member_id, key));
        }
        movers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        for (member_id, _) in movers {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(contain) = obj_ref.get_contain() {
                    if contain.is_passenger_allowed_to_fire(None) {
                        for passenger_id in contain.get_contained_objects() {
                            if passenger_id == victim_id {
                                continue;
                            }
                            let can = OBJECT_REGISTRY
                                .with_object(passenger_id, |passenger| {
                                    OBJECT_REGISTRY
                                        .with_object(victim_id, |victim| {
                                            passenger.get_able_to_attack_specific_object(
                                                attack_type,
                                                victim,
                                                cmd_source,
                                            )
                                        })
                                        .unwrap_or(CanAttackResult::NotPossible)
                                })
                                .unwrap_or(CanAttackResult::NotPossible);
                            if matches!(
                                can,
                                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                            ) {
                                let _ = OBJECT_REGISTRY.with_object(passenger_id, |passenger| {
                                    if let Some(pai) = passenger.get_ai_update_interface() {
                                        if forced {
                                            pai.ai_force_attack_object(
                                                victim_id,
                                                max_shots_to_fire,
                                                cmd_source,
                                            );
                                        } else {
                                            pai.ai_attack_object(
                                                victim_id,
                                                max_shots_to_fire,
                                                cmd_source,
                                            );
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
                if let Some(behavior) = obj_ref.get_spawn_behavior_interface_public() {
                    if let Ok(mut guard) = behavior.lock() {
                        if let Some(spawn) = guard.get_spawn_behavior_full_interface() {
                            if !spawn.do_slaves_have_freedom() {
                                let _ = OBJECT_REGISTRY.with_object(victim_id, |victim| {
                                    let _ = spawn.order_slaves_to_attack_target(
                                        victim,
                                        max_shots_to_fire,
                                        cmd_source,
                                    );
                                });
                            }
                        }
                    }
                }
                if member_id == victim_id {
                    return;
                }
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    if forced {
                        ai.ai_force_attack_object(victim_id, max_shots_to_fire, cmd_source);
                    } else {
                        ai.ai_attack_object(victim_id, max_shots_to_fire, cmd_source);
                    }
                }
            });
        }
    }

    /// C++ `AIGroup::groupAttackTeam` — persistent attack-team state per member.
    pub fn group_attack_team(
        &self,
        team: &Arc<RwLock<Team>>,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_attack_team(team, max_shots_to_fire, cmd_source);
                }
            });
        }
    }

    /// C++ `AIGroup::groupAttackPosition` — passengers and slaves fire the same point.
    pub fn group_attack_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                let attack_pos = *pos;
                if let Some(contain) = obj_ref.get_contain() {
                    if contain.is_passenger_allowed_to_fire(None) {
                        for passenger_id in contain.get_contained_objects() {
                            let can = OBJECT_REGISTRY
                                .with_object(passenger_id, |passenger| {
                                    passenger.get_able_to_use_weapon_against_position(
                                        AbleToAttackType::NewTarget,
                                        &attack_pos,
                                        cmd_source,
                                    )
                                })
                                .unwrap_or(CanAttackResult::NotPossible);
                            if matches!(
                                can,
                                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                            ) {
                                let _ = OBJECT_REGISTRY.with_object(passenger_id, |passenger| {
                                    if let Some(pai) = passenger.get_ai_update_interface() {
                                        pai.ai_attack_position(
                                            &attack_pos,
                                            max_shots_to_fire,
                                            cmd_source,
                                        );
                                    }
                                });
                            }
                        }
                    }
                }
                if let Some(behavior) = obj_ref.get_spawn_behavior_interface_public() {
                    if let Ok(mut guard) = behavior.lock() {
                        if let Some(spawn) = guard.get_spawn_behavior_full_interface() {
                            if !spawn.do_slaves_have_freedom() {
                                let _ = spawn.order_slaves_to_attack_position(
                                    &attack_pos,
                                    max_shots_to_fire,
                                    cmd_source,
                                );
                            }
                        }
                    }
                }
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_attack_position(&attack_pos, max_shots_to_fire, cmd_source);
                }
            });
        }
    }

    pub fn group_guard_position(
        &self,
        pos: &Coord3D,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_guard_position(pos, guard_mode, cmd_source);
                }
            });
        }
    }

    /// Try to sell all objects in the group (matches C++ AIGroup::groupSell).
    pub fn group_sell(&self, _cmd_source: CommandSourceType) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        let current_frame = TheGameLogic::get_frame();
        for &member_id in &self.member_list {
            let Some(sell_obj) =
                OBJECT_REGISTRY.with_object(member_id, |obj_ref| build_assistant::Object {
                    id: obj_ref.get_id(),
                    position: build_assistant::Coord3D {
                        x: obj_ref.get_position().x,
                        y: obj_ref.get_position().y,
                        z: obj_ref.get_position().z,
                    },
                    orientation: obj_ref.get_orientation(),
                    command_set: None,
                })
            else {
                continue;
            };
            let Some(mut assistant) = build_assistant::get_build_assistant() else {
                return;
            };
            assistant.sell_object(&sell_obj, current_frame);
        }
    }

    pub fn group_guard_object(
        &self,
        obj_to_guard_id: ObjectID,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_guard_object(obj_to_guard_id, guard_mode, cmd_source);
                }
            });
        }
    }

    /// Set mine clearing detail weapon set flag for all members (matches C++ AIGroup::setMineClearingDetail)
    pub fn set_mine_clearing_detail(&self, set: bool) {
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj_ref| {
                if set {
                    obj_ref.set_weapon_set_flag(WeaponSetType::MineClearingDetail);
                } else {
                    obj_ref.clear_weapon_set_flag(WeaponSetType::MineClearingDetail);
                }
            });
        }
    }

    /// Set weapon lock for group (matches C++ AIGroup::setWeaponLockForGroup)
    pub fn set_weapon_lock_for_group(
        &self,
        weapon_slot: WeaponSlotType,
        lock_type: WeaponLockType,
    ) -> bool {
        let mut any = false;
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj_ref| {
                obj_ref.set_weapon_lock(weapon_slot, lock_type);
                any = true;
            });
        }
        any
    }

    /// Release weapon lock for all members (matches C++ AIGroup::releaseWeaponLockForGroup)
    pub fn release_weapon_lock_for_group(&self, lock_type: WeaponLockType) {
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj_ref| {
                obj_ref.release_weapon_lock(lock_type);
            });
        }
    }

    /// Set a weapon set flag for members that support it (matches C++ AIGroup::setWeaponSetFlag)
    pub fn set_weapon_set_flag(&self, wst: WeaponSetType) {
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj_ref| {
                if obj_ref.has_weapon_set_template(wst) {
                    obj_ref.set_weapon_set_flag(wst);
                }
            });
        }
    }

    /// Queue an upgrade for all capable members (matches C++ AIGroup::queueUpgrade)
    pub fn queue_upgrade(&self, upgrade: &Arc<UpgradeTemplate>) {
        let upgrade_center = get_upgrade_center().clone();

        for &member_id in &self.member_list {
            let can_queue = OBJECT_REGISTRY
                .with_object(member_id, |obj_ref| {
                    if !obj_ref.can_produce_upgrade(upgrade.as_ref()) {
                        return false;
                    }
                    if upgrade.get_upgrade_type() == crate::upgrade::UpgradeType::Object {
                        if obj_ref.has_upgrade(upgrade.as_ref())
                            || !obj_ref.affected_by_upgrade(upgrade.as_ref())
                        {
                            return false;
                        }
                    }
                    let Some(player) = obj_ref.get_controlling_player() else {
                        return false;
                    };
                    let Ok(player_guard) = player.read() else {
                        return false;
                    };
                    upgrade_center
                        .read()
                        .ok()
                        .map(|center| {
                            center.can_afford_upgrade(&player_guard, upgrade.as_ref(), false)
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if !can_queue {
                continue;
            }
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj_ref| {
                let _ = obj_ref.queue_upgrade(upgrade);
            });
        }
    }

    /// Find an object in the group that can execute a special power (matches C++ AIGroup::getSpecialPowerSourceObject)
    pub fn get_special_power_source_object(
        &self,
        special_power_id: UnsignedInt,
    ) -> Option<Arc<RwLock<Object>>> {
        let store = get_special_power_store()?;
        let template = store.find_special_power_template_by_id(special_power_id as u32)?;

        for &member_id in &self.member_list {
            let has_special_power = OBJECT_REGISTRY
                .with_object(member_id, |obj_ref| {
                    obj_ref
                        .get_special_power_module(template.get_id())
                        .is_some()
                })
                .unwrap_or(false);
            if has_special_power {
                return OBJECT_REGISTRY.get_object(member_id);
            }
        }

        None
    }

    /// Find an object in the group that has a command button (matches C++ AIGroup::getCommandButtonSourceObject)
    pub fn get_command_button_source_object(
        &self,
        command_type: GUICommandType,
    ) -> Option<Arc<RwLock<Object>>> {
        let control_bar = get_control_bar_bridge()?;
        for &member_id in &self.member_list {
            let has_command_button = OBJECT_REGISTRY
                .with_object(member_id, |obj_ref| {
                    let command_set_name = obj_ref.get_command_set_string();
                    let Some(command_set) = control_bar.find_command_set_by_name(command_set_name)
                    else {
                        return false;
                    };
                    command_set
                        .buttons
                        .iter()
                        .flatten()
                        .any(|button| button.id == command_type)
                })
                .unwrap_or(false);
            if has_command_button {
                return OBJECT_REGISTRY.get_object(member_id);
            }
        }

        None
    }

    /// Check if the group is idle
    pub fn is_idle(&self) -> bool {
        // Wave 253 / C++ empty → true; skip registry walks when dual-world empty.
        if self.member_list_size == 0 || Self::dual_world_registry_unavailable() {
            return true;
        }

        // C++ AIGroup::isIdle — all AI members idle or effectively dead; empty → true.
        let mut is_idle = true;
        for &member_id in &self.member_list {
            let member = OBJECT_REGISTRY.with_object(member_id, |obj| {
                obj.get_ai_update_interface()
                    .map(|ai| ai.is_idle() || obj.is_effectively_dead())
            });
            let Some(Some(member_idle)) = member else {
                // missing object or no AI → C++ continue
                continue;
            };
            is_idle = member_idle;
            if !is_idle {
                return false;
            }
        }
        is_idle
    }

    /// Check if the group is busy (explicitly in busy state)
    pub fn is_busy(&self) -> bool {
        // Wave 253 / C++ empty → true; skip registry walks when dual-world empty.
        if self.member_list_size == 0 || Self::dual_world_registry_unavailable() {
            return true;
        }

        // C++ AIGroup::isBusy — all AI members busy and alive; empty → true.
        let mut is_busy = true;
        for &member_id in &self.member_list {
            let member = OBJECT_REGISTRY.with_object(member_id, |obj| {
                obj.get_ai_update_interface()
                    .map(|ai| ai.is_busy() && !obj.is_effectively_dead())
            });
            let Some(Some(member_busy)) = member else {
                // missing object or no AI → C++ continue
                continue;
            };
            is_busy = member_busy;
            if !is_busy {
                return false;
            }
        }
        is_busy
    }

    /// Check if the group AI is dead
    pub fn is_group_ai_dead(&self) -> bool {
        // C++: group AI is dead when every member is effectively dead (or missing).
        for &member_id in &self.member_list {
            let alive = OBJECT_REGISTRY
                .with_object(member_id, |obj| !obj.is_effectively_dead())
                .unwrap_or(false);
            if alive {
                return false;
            }
        }
        true
    }

    /// Set attitude for all group members
    pub fn set_attitude(&self, attitude: AttitudeType) {
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.set_attitude(to_module_attitude(attitude));
                }
            });
        }
    }

    /// Get attitude from first group member (they should all be the same)
    pub fn get_attitude(&self) -> AttitudeType {
        for &member_id in &self.member_list {
            if let Some(Some(v)) = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    return Some(from_module_attitude(ai.get_attitude()));
                }

                None
            }) {
                return v;
            }
        }
        AttitudeType::Normal
    }

    // Formation commands

    /// Set formation type for the group
    pub fn set_formation(&mut self, formation_type: FormationType, player_id: i32) {
        self.formation_type = formation_type;

        // Create or update formation if we have a formation manager
        if let Some(ref manager_arc) = self.formation_manager {
            if let Ok(mut manager) = manager_arc.try_lock() {
                if let Some(formation_id) = self.formation_id {
                    // Update existing formation
                    if let Some(formation) = manager.get_formation_mut(formation_id) {
                        let _ = formation
                            .execute_command(FormationCommand::SetFormation(formation_type));
                    }
                } else if self.member_list_size >= 2 {
                    // Create new formation
                    let settings = FormationSettings::default();
                    let formation_id =
                        manager.create_formation(formation_type, settings, player_id);
                    self.formation_id = Some(formation_id);

                    // Add all members to the formation
                    for &member_id in &self.member_list {
                        let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                            let unit_id = obj_ref.get_id();
                            let position = *obj_ref.get_position();
                            let speed = if let Some(ai) = obj_ref.get_ai_update_interface() {
                                ai.get_speed()
                            } else {
                                100.0
                            };
                            // Get actual health percentage from object
                            let health = obj_ref.get_health_percentage();
                            // Get actual veterancy rank (0=Regular, 1=Veteran, 2=Elite, 3=Heroic)
                            let rank = obj_ref.get_veterancy_level() as u32;

                            if let Some(formation) = manager.get_formation_mut(formation_id) {
                                let _ = formation.add_unit(unit_id, position, speed, health, rank);
                            }
                        });
                    }
                }
            }
        }
    }

    /// Get current formation type
    pub fn get_formation_type(&self) -> FormationType {
        self.formation_type
    }

    /// Move group in formation to position
    pub fn group_move_in_formation(
        &mut self,
        pos: &Coord3D,
        add_waypoint: bool,
        cmd_source: CommandSourceType,
    ) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        if let Some(formation_id) = self.formation_id {
            if let Some(ref manager_arc) = self.formation_manager {
                if let Ok(mut manager) = manager_arc.try_lock() {
                    if let Some(formation) = manager.get_formation_mut(formation_id) {
                        // Issue formation move command
                        let _ = formation.execute_command(FormationCommand::MoveTo(*pos));
                    }
                }
            }
        } else {
            // Fall back to regular group move
            self.group_move_to_position(pos, add_waypoint, cmd_source);
        }
    }

    /// C++ `AIGroup::groupAttackMoveToPosition` — every member uses the same `pos`.
    pub fn group_attack_move_to_position(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        // Wave 253: empty dual-world / empty group short-circuit.
        if self.is_empty() || Self::dual_world_registry_unavailable() {
            return;
        }

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                let Some(ai) = obj_ref.get_ai_update_interface() else {
                    return;
                };
                if obj_ref.is_able_to_attack() {
                    ai.ai_attack_move_to_position(
                        pos,
                        crate::weapon::NO_MAX_SHOTS_LIMIT,
                        cmd_source,
                    );
                } else {
                    ai.ai_move_to_position(pos, false, cmd_source);
                }
            });
        }
    }

    /// Break formation (units move independently)
    pub fn break_formation(&mut self) {
        if let Some(formation_id) = self.formation_id {
            if let Some(ref manager_arc) = self.formation_manager {
                if let Ok(mut manager) = manager_arc.try_lock() {
                    if let Some(formation) = manager.get_formation_mut(formation_id) {
                        let _ = formation.execute_command(FormationCommand::Break);
                    }
                }
            }
        }
        self.formation_type = FormationType::None;
    }

    /// Reform formation
    pub fn reform_formation(&mut self) {
        if let Some(formation_id) = self.formation_id {
            if let Some(ref manager_arc) = self.formation_manager {
                if let Ok(mut manager) = manager_arc.try_lock() {
                    if let Some(formation) = manager.get_formation_mut(formation_id) {
                        let _ = formation.execute_command(FormationCommand::Reform);
                    }
                }
            }
        }
    }

    /// Check if group is in formation
    pub fn is_in_formation(&self) -> bool {
        self.formation_type != FormationType::None
    }

    /// Update formation positions (should be called regularly)
    pub fn update_formation(&mut self, _frame: u32) {
        if let Some(formation_id) = self.formation_id {
            if let Some(ref manager_arc) = self.formation_manager {
                if let Ok(mut manager) = manager_arc.try_lock() {
                    // Update member positions in formation
                    if let Some(formation) = manager.get_formation_mut(formation_id) {
                        for &member_id in &self.member_list {
                            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                                let unit_id = obj_ref.get_id();
                                let position = *obj_ref.get_position();
                                // Get actual health percentage from object
                                let health = obj_ref.get_health_percentage();
                                // Check if object is in combat
                                let in_combat = obj_ref.is_in_combat();

                                let _ = formation
                                    .update_unit_status(unit_id, position, health, in_combat);
                            });
                        }
                    }
                }
            }
        }
    }

    /// Set formation manager (for integration with global formation system)
    pub fn set_formation_manager(&mut self, manager: Arc<Mutex<FormationManager>>) {
        self.formation_manager = Some(manager);
    }

    /// C++ `AIGroup::friend_computeGroundPath`.
    fn friend_compute_ground_path(
        &self,
        pos: &Coord3D,
        _cmd_source: CommandSourceType,
    ) -> Option<Vec<Coord3D>> {
        let Some((min, max, mut center, _)) = self.get_min_max_and_center() else {
            return None;
        };
        let ai_store = the_ai();let (min_dist, require_dist, _min_inf, _min_veh) = ai_store
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|d| {
                    (
                        d.min_distance_for_group,
                        d.distance_requires_group,
                        d.min_infantry_for_group,
                        d.min_vehicles_for_group,
                    )
                })
            })
            .unwrap_or((100.0, 400.0, 3, 4));

        let mut dist_sqr = 4.0 * require_dist * require_dist;
        let mut num_infantry = 0i32;
        let mut num_vehicles = 0i32;
        let mut center_id = None;
        let mut dist_sqr_center = dist_sqr * 10.0;

        for &member_id in &self.member_list {
            let Some((kind_inf, kind_veh, kind_air, unit_pos)) = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    if obj.is_disabled_by_type(DisabledType::Held)
                        || obj.get_ai_update_interface().is_none()
                    {
                        return None;
                    }
                    Some((
                        obj.is_kind_of(KindOf::Infantry),
                        obj.is_kind_of(KindOf::Vehicle),
                        obj.is_kind_of(KindOf::Aircraft),
                        *obj.get_position(),
                    ))
                })
                .flatten()
            else {
                continue;
            };
            if kind_inf {
                num_infantry += 1;
            } else if kind_veh {
                if kind_air {
                    continue;
                }
                num_vehicles += 1;
            } else {
                continue;
            }
            let dx = unit_pos.x - pos.x;
            let dy = unit_pos.y - pos.y;
            let to_goal = dx * dx + dy * dy;
            if to_goal < dist_sqr {
                dist_sqr = to_goal;
            }
            let cx = unit_pos.x - center.x;
            let cy = unit_pos.y - center.y;
            let to_center = cx * cx + cy * cy;
            if center_id.is_none() || to_center < dist_sqr_center {
                center_id = Some(member_id);
                dist_sqr_center = to_center;
            }
        }
        let center_id = center_id?;
        if let Some(p) = OBJECT_REGISTRY.with_object(center_id, |obj| *obj.get_position()) {
            center = p;
        }

        let span_x = max.x - min.x;
        let span_y = max.y - min.y;
        if span_x * span_x + span_y * span_y > require_dist * require_dist {
            dist_sqr = span_x * span_x + span_y * span_y;
        }
        if dist_sqr < min_dist * min_dist {
            return None;
        }
        let mut close_enough =
            dist_sqr > require_dist * require_dist || num_infantry > 6 || num_vehicles > 4;
        if !close_enough {
            let mut passable = true;
            for &member_id in &self.member_list {
                let ok = OBJECT_REGISTRY
                    .with_object(member_id, |obj| {
                        if !obj.is_kind_of(KindOf::Infantry) {
                            return true;
                        }
                        let Some(ai) = obj.get_ai_update_interface() else {
                            return true;
                        };
                        let Ok(ai_guard) = ai.lock() else {
                            return true;
                        };
                        let Some(set) = ai_guard.get_locomotor_set_clone() else {
                            return true;
                        };
                        the_ai()
                            .read()
                            .ok()
                            .and_then(|ai| ai.pathfinder())
                            .and_then(|pf| {
                                pf.read().ok().map(|p| {
                                    p.is_line_passable_for_surfaces(
                                        obj.get_position(),
                                        &center,
                                        set.get_valid_surfaces(),
                                        None,
                                    )
                                })
                            })
                            .unwrap_or(true)
                    })
                    .unwrap_or(true);
                if !ok {
                    passable = false;
                    break;
                }
            }
            if passable {
                close_enough = true;
            }
        }
        if !close_enough {
            return None;
        }

        const PATH_DIAMETER_IN_CELLS: i32 = 6;
        the_ai().read().ok().and_then(|ai| {
            ai.pathfinder().and_then(|pf| {
                pf.read()
                    .ok()
                    .and_then(|p| p.find_group_ground_path(&center, pos, PATH_DIAMETER_IN_CELLS))
            })
        })
    }

    /// C++ `AIGroup::friend_moveInfantryToPos` — 3-column infantry packing.
    fn friend_move_infantry_to_pos(
        &self,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
        path: &[Coord3D],
    ) -> bool {
        self.friend_move_column_to_pos(pos, cmd_source, path, true)
    }

    /// C++ `AIGroup::friend_moveVehicleToPos` — 2-column vehicle packing.
    fn friend_move_vehicle_to_pos(
        &self,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
        path: &[Coord3D],
    ) -> bool {
        self.friend_move_column_to_pos(pos, cmd_source, path, false)
    }

    fn friend_move_column_to_pos(
        &self,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
        path: &[Coord3D],
        infantry: bool,
    ) -> bool {
        if path.len() < 2 {
            return false;
        }
        let Some(center) = self.get_center() else {
            return false;
        };
        let ai_store = the_ai();let min_count = ai_store
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|d| {
                    if infantry {
                        d.min_infantry_for_group
                    } else {
                        d.min_vehicles_for_group
                    }
                })
            })
            .unwrap_or(if infantry { 3 } else { 4 });

        const PATH_DIAMETER_IN_CELLS: f32 = 6.0;
        let far_enough_sqr = (PATH_DIAMETER_IN_CELLS * PATHFIND_CELL_SIZE_F)
            * (PATH_DIAMETER_IN_CELLS * PATHFIND_CELL_SIZE_F);
        let start_point = path[0];
        let end_point = *path.last().unwrap();
        let Some(start_node) = path.iter().find(|n| {
            let dx = n.x - start_point.x;
            let dy = n.y - start_point.y;
            dx * dx + dy * dy > far_enough_sqr
        }) else {
            return false;
        };
        let Some(end_node) = path.iter().rev().find(|n| {
            let dx = n.x - end_point.x;
            let dy = n.y - end_point.y;
            dx * dx + dy * dy > far_enough_sqr
        }) else {
            return false;
        };

        let mut start_vector =
            Coord2D::new(start_node.x - start_point.x, start_node.y - start_point.y);
        normalize_coord2(&mut start_vector);
        let mut end_vector = Coord2D::new(end_point.x - end_node.x, end_point.y - end_node.y);
        normalize_coord2(&mut end_vector);
        let mut start_normal = Coord2D::new(-start_vector.y, start_vector.x);
        normalize_coord2(&mut start_normal);
        let mut end_normal = Coord2D::new(-end_vector.y, end_vector.x);
        normalize_coord2(&mut end_normal);

        let mut units: Vec<(ObjectID, f32, Coord3D)> = Vec::new();
        let mut use_end = false;
        for &member_id in &self.member_list {
            let Some((unit_pos, ok)) = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    if obj.is_disabled_by_type(DisabledType::Held)
                        || obj.get_ai_update_interface().is_none()
                    {
                        return None;
                    }
                    if infantry {
                        if !obj.is_kind_of(KindOf::Infantry) {
                            return None;
                        }
                        if obj.is_kind_of(KindOf::MobNexus) {
                            return Some((*obj.get_position(), false));
                        }
                    } else {
                        if !obj.is_kind_of(KindOf::Vehicle) {
                            return None;
                        }
                        if let Some(ai) = obj.get_ai_update_interface() {
                            if let Ok(ai_guard) = ai.lock() {
                                if !ai_guard.is_doing_ground_movement() {
                                    return None;
                                }
                            }
                        }
                    }
                    Some((*obj.get_position(), true))
                })
                .flatten()
            else {
                continue;
            };
            if !ok {
                return false;
            }
            let dx = unit_pos.x - center.x;
            let dy = unit_pos.y - center.y;
            let key = dx * start_normal.x + dy * start_normal.y;
            let to_end = {
                let ex = unit_pos.x - end_point.x;
                let ey = unit_pos.y - end_point.y;
                ex * ex + ey * ey
            };
            let to_start = {
                let sx = unit_pos.x - start_point.x;
                let sy = unit_pos.y - start_point.y;
                sx * sx + sy * sy
            };
            if to_start > to_end {
                use_end = true;
            }
            units.push((member_id, key, unit_pos));
        }
        if (units.len() as i32) < min_count {
            return false;
        }
        if use_end {
            start_vector = end_vector;
            start_normal = end_normal;
            for unit in &mut units {
                let dx = unit.2.x - center.x;
                let dy = unit.2.y - center.y;
                unit.1 = dx * start_normal.x + dy * start_normal.y;
            }
        }
        units.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let num_columns = if infantry { 3 } else { 2 };
        let half = num_columns / 2;
        let units_to_path = units.len() as i32;
        for (i, (member_id, _, _)) in units.iter().enumerate() {
            let divisor = ((units_to_path + 1) / num_columns).max(1);
            let mut column_delta = half - (i as i32 / divisor);
            if column_delta < -half {
                column_delta = -half;
            }
            let mut dests: Vec<Coord3D> = Vec::new();
            let mut prev = OBJECT_REGISTRY
                .with_object(*member_id, |obj| *obj.get_position())
                .unwrap_or(center);
            for window in path.windows(2) {
                let mut dest = window[0];
                let next = window[1];
                let mut corner = Coord2D::new(next.x - dest.x, next.y - dest.y);
                let mut corner_n = Coord2D::new(-corner.y, corner.x);
                normalize_coord2(&mut corner);
                normalize_coord2(&mut corner_n);
                let offset = PATHFIND_CELL_SIZE_F * 2.1 / half.max(1) as f32;
                dest.x += offset * column_delta as f32 * corner_n.x;
                dest.y += offset * column_delta as f32 * corner_n.y;
                let cur = Coord2D::new(dest.x - prev.x, dest.y - prev.y);
                if corner.x * cur.x + corner.y * cur.y > 0.0 {
                    dests.push(dest);
                    prev = dest;
                }
            }
            let mut dest = *pos;
            let offset = PATHFIND_CELL_SIZE_F * 2.2;
            dest.x += offset * column_delta as f32 * end_normal.x;
            dest.y += offset * column_delta as f32 * end_normal.y;
            dests.push(dest);
            let _ = OBJECT_REGISTRY.with_object(*member_id, |obj| {
                if let Some(ai) = obj.get_ai_update_interface() {
                    ai.ai_follow_path(&dests, None, cmd_source);
                }
            });
        }
        true
    }

    /// C++ `AIGroup::friend_moveFormationToPos`.
    fn friend_move_formation_to_pos(
        &self,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
        path: Option<&[Coord3D]>,
    ) {
        let Some(center) = self.get_center() else {
            return;
        };
        const PATH_DIAMETER_IN_CELLS: f32 = 6.0;
        let far_enough_sqr = (PATH_DIAMETER_IN_CELLS * PATHFIND_CELL_SIZE_F)
            * (PATH_DIAMETER_IN_CELLS * PATHFIND_CELL_SIZE_F);
        let (start_node, end_point) = if let Some(path) = path.filter(|p| p.len() >= 2) {
            let start_point = path[0];
            let end_point = *path.last().unwrap();
            let start = path.iter().find(|n| {
                let dx = n.x - start_point.x;
                let dy = n.y - start_point.y;
                dx * dx + dy * dy > far_enough_sqr
            });
            (start.copied(), end_point)
        } else {
            (None, *pos)
        };

        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj| {
                if obj.is_disabled_by_type(DisabledType::Held) {
                    return;
                }
                let Some(ai) = obj.get_ai_update_interface() else {
                    return;
                };
                let offset = obj.get_formation_offset();
                if let Some(start) = start_node {
                    let mut dests: Vec<Coord3D> = Vec::new();
                    if let Some(path) = path {
                        let mut started = false;
                        for node in path {
                            if !started {
                                if (node.x - start.x).abs() < 0.01
                                    && (node.y - start.y).abs() < 0.01
                                {
                                    started = true;
                                } else {
                                    continue;
                                }
                            }
                            dests.push(Coord3D::new(node.x + offset.x, node.y + offset.y, node.z));
                        }
                    }
                    dests.push(Coord3D::new(
                        end_point.x + offset.x,
                        end_point.y + offset.y,
                        end_point.z,
                    ));
                    ai.ai_follow_path(&dests, None, cmd_source);
                } else {
                    let dest =
                        Coord3D::new(end_point.x + offset.x, end_point.y + offset.y, end_point.z);
                    ai.ai_move_to_position(&dest, false, cmd_source);
                }
                let _ = center;
            });
        }
    }

    /// C++ `AIGroup::crc`.
    pub fn crc(&self, xfer: &mut dyn Xfer) {
        for &id in &self.member_list {
            let mut member_id = id;
            let _ = xfer.xfer_object_id(&mut member_id);
        }
        let mut size = self.member_list_size as u32;
        let _ = xfer.xfer_unsigned_int(&mut size);
        let mut leader = crate::common::INVALID_ID;
        let _ = xfer.xfer_object_id(&mut leader);
        let mut speed = self.speed;
        let _ = xfer.xfer_real(&mut speed);
        let mut dirty = self.dirty;
        let _ = xfer.xfer_bool(&mut dirty);
        let mut id = self.id;
        let _ = xfer.xfer_unsigned_int(&mut id);
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);
    }

    pub fn load_post_process(&mut self) {}
}

fn normalize_coord2(v: &mut Coord2D) {
    let len = (v.x * v.x + v.y * v.y).sqrt();
    if len > 0.0001 {
        v.x /= len;
        v.y /= len;
    } else {
        v.x = 1.0;
        v.y = 0.0;
    }
}

impl Snapshot for AIGroup {
    fn crc(&self, xfer: &mut dyn Xfer) {
        AIGroup::crc(self, xfer);
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        AIGroup::xfer(self, xfer);
    }

    fn load_post_process(&mut self) {
        AIGroup::load_post_process(self);
    }
}

impl Drop for AIGroup {
    fn drop(&mut self) {
        // Disassociate each member from the group
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj_ref| {
                obj_ref.leave_group();
            });
        }
    }
}

fn to_module_attitude(attitude: AttitudeType) -> AIAttitudeType {
    match attitude {
        AttitudeType::Normal | AttitudeType::Sleep | AttitudeType::Passive => {
            AIAttitudeType::Normal
        }
        AttitudeType::Aggressive | AttitudeType::Alert => AIAttitudeType::Aggressive,
        AttitudeType::Invalid => AIAttitudeType::Normal,
    }
}

fn from_module_attitude(attitude: AIAttitudeType) -> AttitudeType {
    match attitude {
        AIAttitudeType::Aggressive => AttitudeType::Aggressive,
        AIAttitudeType::Defensive => AttitudeType::Normal,
        AIAttitudeType::Passive => AttitudeType::Passive,
        AIAttitudeType::Sleep => AttitudeType::Sleep,
        _ => AttitudeType::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formation_creation() {
        let manager = Arc::new(Mutex::new(FormationManager::new()));
        let mut group = AIGroup::new_with_formation(1, manager.clone());

        // Initially no formation
        assert_eq!(group.get_formation_type(), FormationType::None);
        assert!(!group.is_in_formation());

        // Set formation type
        group.set_formation(FormationType::Line, 0);
        assert_eq!(group.get_formation_type(), FormationType::Line);
    }

    #[test]
    fn test_formation_break_and_reform() {
        let manager = Arc::new(Mutex::new(FormationManager::new()));
        let mut group = AIGroup::new_with_formation(1, manager.clone());

        group.set_formation(FormationType::Wedge, 0);
        assert!(group.is_in_formation());

        group.break_formation();
        assert!(!group.is_in_formation());
        assert_eq!(group.get_formation_type(), FormationType::None);

        group.reform_formation();
        // Note: reform won't work without units, this is just testing the API
    }

    #[test]
    fn test_group_speed_calculation() {
        let mut group = AIGroup::new(1);

        // Empty group should have 0 speed
        assert_eq!(group.get_speed(), 0.0);
    }

    #[test]
    fn test_formation_manager_reference() {
        let manager = Arc::new(Mutex::new(FormationManager::new()));
        let mut group = AIGroup::new(1);

        // Initially no manager
        assert!(group.formation_manager.is_none());

        // Set manager
        group.set_formation_manager(manager.clone());
        assert!(group.formation_manager.is_some());
    }
}

/// Guard mode enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
pub enum GuardMode {
    Normal,
    /// No pursuit out of guard area.
    GuardWithoutPursuit,
    /// Ignore non-flying units.
    GuardFlyingUnitsOnly,
    /// Preserve raw mode values coming from the network/replay/message stream.
    Other(i32),
}

impl From<i32> for GuardMode {
    fn from(value: i32) -> Self {
        GuardMode::from_i32(value)
    }
}

impl From<GuardMode> for i32 {
    fn from(value: GuardMode) -> Self {
        value.as_i32()
    }
}

impl Default for GuardMode {
    fn default() -> Self {
        GuardMode::Normal
    }
}

impl GuardMode {
    /// Convert from the raw C++ integer guard mode without losing information.
    pub const fn from_i32(mode: i32) -> Self {
        match mode {
            0 => GuardMode::Normal,
            1 => GuardMode::GuardWithoutPursuit,
            2 => GuardMode::GuardFlyingUnitsOnly,
            other => GuardMode::Other(other),
        }
    }

    /// Convert back to the raw C++ integer guard mode.
    pub const fn as_i32(self) -> i32 {
        match self {
            GuardMode::Normal => 0,
            GuardMode::GuardWithoutPursuit => 1,
            GuardMode::GuardFlyingUnitsOnly => 2,
            GuardMode::Other(v) => v,
        }
    }
}

// Re-export canonical AttitudeType from ai/mod.rs (Sleep=-2, Passive=-1, Normal=0, Alert=1, Aggressive=2, Invalid=3)
pub use super::AttitudeType;
