use crate::ai::{AiCommandParams, AiCommandType, GUICommandType};
use crate::common::command::*;
use crate::common::coord::*;
use crate::common::*;
use crate::control_bar::get_control_bar_bridge;
use crate::damage::*;
use crate::formation::{
    FormationCommand, FormationGroup, FormationManager, FormationSettings, FormationType,
};
use crate::helpers::TheGameLogic;
use crate::modules::{AIAttitudeType, AIUpdateInterface, AIUpdateInterfaceExt};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::get_special_power_store;
use crate::object::*;
use crate::path::*;
use crate::player::Player;
use crate::polygon_trigger::PolygonTrigger;
use crate::special_power::*;
use crate::team::Team;
use crate::upgrade::center::THE_UPGRADE_CENTER;
use crate::upgrade::UpgradeTemplate;
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
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_move_to_position(pos, add_waypoint, cmd_source);
                }
            });
        }
    }

    pub fn group_move_to_and_evacuate(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_move_to_and_evacuate(pos, cmd_source);
                }
            });
        }
    }

    /// Start following the path from the given waypoint (matches C++ AIGroup::groupFollowWaypointPath).
    pub fn group_follow_waypoint_path(&self, way: &Waypoint, cmd_source: CommandSourceType) {
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
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_follow_waypoint_path_exact_as_team(way, cmd_source);
                }
            });
        }
    }

    pub fn group_idle(&self, cmd_source: CommandSourceType) {
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_idle(cmd_source);
                }
            });
        }
    }

    /// Tell all things in the group to toggle overcharge (matches C++ AIGroup::groupToggleOvercharge).
    pub fn group_toggle_overcharge(&self, _cmd_source: CommandSourceType) {
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
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
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
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
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
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(target_id, |target_ref| {
                let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
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
        self.group_attack_object_private(false, victim_id, max_shots_to_fire, cmd_source);
    }

    pub fn group_force_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        self.group_attack_object_private(true, victim_id, max_shots_to_fire, cmd_source);
    }

    fn group_attack_object_private(
        &self,
        forced: bool,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
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

    pub fn group_attack_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    ai.ai_attack_position(pos, max_shots_to_fire, cmd_source);
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
        let upgrade_center = THE_UPGRADE_CENTER.clone();

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

    /// Group attack-move: Move to position and engage enemies along the way
    pub fn group_attack_move_to_position(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        for &member_id in &self.member_list {
            let _ = OBJECT_REGISTRY.with_object(member_id, |obj_ref| {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    // Attack-move is a special AI state that moves to position
                    // while automatically engaging enemies
                    ai.ai_attack_move_to_position(
                        pos,
                        crate::weapon::NO_MAX_SHOTS_LIMIT,
                        cmd_source,
                    );
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
