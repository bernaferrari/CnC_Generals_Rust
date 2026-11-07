//! Host `RailedTransportAIUpdate` residual (C++ `RailedTransportAIUpdate.cpp`).
//!
//! Leftover crate `RailedTransportAIUpdate::update` / `private_execute_railed_transport`
//! already match C++ but early-out when `OBJECT_REGISTRY` is empty (host-only).
//! This is the live path: authored `PathPrefixName` StartNN/EndNN pairs, transit
//! dock close, and `aiFollowWaypointPath` along the start-waypoint link chain.

use super::ObjectId;
use crate::game_logic::host_railroad::HostWaypointSnap;
use crate::game_logic::{AIState, DockKind, GameLogic};
use glam::Vec3;
use std::collections::{HashMap, HashSet};

/// C++ `RailedTransportAIUpdate::MAX_WAYPOINT_PATHS`.
pub const RAILED_MAX_WAYPOINT_PATHS: usize = 32;
/// C++ `INVALID_PATH`.
pub const RAILED_INVALID_PATH: i32 = -1;
/// C++ arrival threshold in `RailedTransportAIUpdate::update`.
pub const RAILED_ARRIVE_DISTANCE: f32 = 5.0;

impl GameLogic {
    pub fn railed_waypoint_overlay_reset(&mut self) {
        self.railed_waypoints.clear();
    }
    pub fn inject_railed_waypoint(&mut self, waypoint: HostWaypointSnap) {
        if let Some(existing) = self
            .railed_waypoints
            .iter_mut()
            .find(|item| item.name.eq_ignore_ascii_case(&waypoint.name))
        {
            *existing = waypoint;
        } else {
            self.railed_waypoints.push(waypoint);
        }
    }
    fn all_railed_waypoints(&self) -> Vec<HostWaypointSnap> {
        let mut waypoints = self.owned_railroad_waypoints();
        waypoints.extend(self.railed_waypoints.iter().cloned());
        waypoints
    }
}

fn waypoint_name_eq(name: &str, expected: &str) -> bool {
    name.eq_ignore_ascii_case(expected)
}

fn walk_link0_positions(snaps: &[HostWaypointSnap], start_id: u32) -> Vec<Vec3> {
    let by_id: HashMap<u32, &HostWaypointSnap> = snaps.iter().map(|wp| (wp.id, wp)).collect();
    let mut out = Vec::new();
    let mut visited = HashSet::new();
    let mut current = Some(start_id);
    while let Some(id) = current {
        if out.len() >= gamelogic::terrain::WAYPOINT_PATH_LIMIT || !visited.insert(id) {
            break;
        }
        let Some(wp) = by_id.get(&id) else {
            break;
        };
        out.push(wp.position);
        current = wp.link0;
    }
    out
}

fn load_prefix_paths(prefix: &str, snaps: &[HostWaypointSnap]) -> Vec<(u32, u32)> {
    let mut paths = vec![(0u32, 0u32); RAILED_MAX_WAYPOINT_PATHS];
    let mut num_paths = 0usize;
    for i in 0..RAILED_MAX_WAYPOINT_PATHS {
        let start_name = format!("{prefix}Start{:02}", i + 1);
        let end_name = format!("{prefix}End{:02}", i + 1);
        let start = snaps
            .iter()
            .find(|wp| waypoint_name_eq(&wp.name, &start_name));
        let end = snaps
            .iter()
            .find(|wp| waypoint_name_eq(&wp.name, &end_name));
        if let (Some(start), Some(end)) = (start, end) {
            paths[i] = (start.id, end.id);
            num_paths += 1;
        }
    }
    paths.truncate(num_paths);
    paths
}

pub fn default_railed_current_path() -> i32 {
    RAILED_INVALID_PATH
}

impl GameLogic {
    fn ensure_railed_waypoint_data(
        &self,
        prefix: &str,
        loaded: bool,
        paths: &[(u32, u32)],
    ) -> (bool, Vec<(u32, u32)>) {
        if loaded {
            (true, paths.to_vec())
        } else {
            (
                true,
                load_prefix_paths(prefix, &self.all_railed_waypoints()),
            )
        }
    }

    fn set_railed_in_transit(&mut self, id: ObjectId, in_transit: bool) {
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.railed_in_transit = in_transit;
            obj.railed_dock.dock_open = !in_transit;
        }
    }

    fn follow_railed_waypoint_id(&mut self, id: ObjectId, waypoint_id: u32) -> bool {
        let snaps = self.all_railed_waypoints();
        let points = walk_link0_positions(&snaps, waypoint_id);
        if points.is_empty() {
            return false;
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.waiting_for_path = false;
        unit.movement.current_path_index = 0;
        unit.movement.path = points;
        unit.movement.target_position = unit.movement.path.first().copied();
        unit.is_exact_path = true;
        unit.is_attack_path = false;
        unit.path_extra_distance = unit.waypoint_link_extra_distance();
        unit.set_ultra_accurate(true);
        unit.set_ai_state(
            AIState::Moving,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        unit.set_status_moving(true);
        true
    }

    fn pick_and_move_to_initial_railed_location(&mut self, id: ObjectId) -> bool {
        let (pos, paths) = {
            let Some(obj) = self.objects.get(&id) else {
                return false;
            };
            (obj.get_position(), obj.railed_paths.clone())
        };
        if paths.is_empty() {
            return false;
        }
        let snaps = self.all_railed_waypoints();
        let mut closest_path = RAILED_INVALID_PATH;
        let mut closest_dist = f32::MAX;
        let mut closest_end_id = None;
        for (i, (_start, end_id)) in paths.iter().enumerate() {
            let Some(end) = snaps.iter().find(|wp| wp.id == *end_id) else {
                continue;
            };
            let dist = (end.position - pos).length();
            if dist < closest_dist {
                closest_dist = dist;
                closest_path = i as i32;
                closest_end_id = Some(*end_id);
            }
        }
        let Some(end_id) = closest_end_id else {
            return false;
        };
        if !self.follow_railed_waypoint_id(id, end_id) {
            return false;
        }
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.railed_current_path = closest_path;
        }
        self.set_railed_in_transit(id, true);
        true
    }

    /// C++ `RailedTransportAIUpdate::privateExecuteRailedTransport`.
    pub fn execute_railed_transport_for(&mut self, id: ObjectId) -> bool {
        let (prefix, dock_kind, loading, loaded, current, paths) = {
            let Some(obj) = self.objects.get(&id) else {
                return false;
            };
            if !obj.is_alive() {
                return false;
            }
            (
                obj.thing().template.railed_path_prefix_name.clone(),
                obj.thing().template.dock_kind,
                obj.railed_dock.busy(),
                obj.railed_waypoint_data_loaded,
                obj.railed_current_path,
                obj.railed_paths.clone(),
            )
        };
        if prefix.is_empty() || dock_kind != DockKind::RailedTransport {
            return false;
        }
        if loading {
            return false;
        }
        let (loaded, paths) = self.ensure_railed_waypoint_data(&prefix, loaded, &paths);
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.railed_waypoint_data_loaded = loaded;
            obj.railed_paths = paths.clone();
        }
        if paths.is_empty() {
            return false;
        }
        let mut next = current + 1;
        if next >= paths.len() as i32 {
            next = 0;
        }
        let start_id = paths[next as usize].0;
        if !self.follow_railed_waypoint_id(id, start_id) {
            return false;
        }
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.railed_current_path = next;
        }
        self.set_railed_in_transit(id, true);
        true
    }

    /// C++ `RailedTransportAIUpdate::update`.
    pub fn update_railed_transports(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                if obj.thing().template.railed_path_prefix_name.is_empty() {
                    return None;
                }
                if obj.thing().template.dock_kind != DockKind::RailedTransport {
                    return None;
                }
                Some(*id)
            })
            .collect();

        for id in ids {
            let (prefix, loaded, paths, current, in_transit, pos, idle) = {
                let Some(obj) = self.objects.get_mut(&id) else {
                    continue;
                };
                obj.set_ultra_accurate(true);
                (
                    obj.thing().template.railed_path_prefix_name.clone(),
                    obj.railed_waypoint_data_loaded,
                    obj.railed_paths.clone(),
                    obj.railed_current_path,
                    obj.railed_in_transit,
                    obj.get_position(),
                    matches!(obj.ai_state(), AIState::Idle)
                        || obj.movement.path.is_empty()
                        || obj.movement.current_path_index >= obj.movement.path.len(),
                )
            };
            let (loaded, paths) = self.ensure_railed_waypoint_data(&prefix, loaded, &paths);
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.railed_waypoint_data_loaded = loaded;
                obj.railed_paths = paths.clone();
            }
            if current == RAILED_INVALID_PATH && !paths.is_empty() {
                let _ = self.pick_and_move_to_initial_railed_location(id);
                continue;
            }
            if !in_transit || current < 0 {
                continue;
            }
            let Some((_, end_id)) = paths.get(current as usize) else {
                continue;
            };
            let snaps = self.all_railed_waypoints();
            let Some(end) = snaps.iter().find(|wp| wp.id == *end_id) else {
                continue;
            };
            let dist = (end.position - pos).length();
            if dist <= RAILED_ARRIVE_DISTANCE || idle {
                self.set_railed_in_transit(id, false);
            }
        }
    }
}

/// State of one `RailedTransportDockUpdate`. Store this on the ferry, not on
/// the world scheduler: the two object references are independent of the base
/// dock's approach queue.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct RailedDockState {
    /// Base DockUpdate's `m_dockOpen`.
    pub dock_open: bool,
    pub docking: Option<RailedDockObject>,
    pub pull_per_frame: f32,
    pub unloading: Option<RailedDockObject>,
    pub push_per_frame: f32,
    pub unload_count: i32,
}

/// A reference to an admitted object birth. Restore must rebind `generation`
/// after all snapshot objects have been admitted to the receiving world.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct RailedDockObject {
    pub id: ObjectId,
    #[serde(skip)]
    pub generation: u64,
}

impl Default for RailedDockState {
    fn default() -> Self {
        Self { dock_open: true, docking: None, pull_per_frame: 0.0, unloading: None, push_per_frame: 0.0, unload_count: -1 }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RailedDockConfig {
    pub pull_frames: u32,
    pub push_frames: u32,
    pub tolerance: f32,
}

impl RailedDockState {
    pub(crate) fn busy(&self) -> bool {
        self.docking.is_some() || self.unloading.is_some()
    }
}

impl GameLogic {
    /// Read the receiving template's parsed module, without inventing a
    /// configuration for objects that do not own this behavior.
    pub(crate) fn railed_dock_config(&self, ferry: ObjectId) -> Option<RailedDockConfig> {
        let template = self.objects.get(&ferry)?.get_template().receiving_client_template.as_ref()?;
        let module = template.behavior_modules().iter()
            .find(|entry| entry.name.as_str().eq_ignore_ascii_case("RailedTransportDockUpdate"))?;
        let data = module.data.as_any().downcast_ref::<
            gamelogic::object::production::railed_transport_dock::RailedTransportDockUpdateData
        >()?;
        Some(RailedDockConfig {
            pull_frames: data.pull_inside_duration_frames,
            push_frames: data.push_outside_duration_frames,
            tolerance: data.tolerance_distance,
        })
    }
    fn railed_contain_config(&self, ferry: ObjectId) -> Option<&gamelogic::object::contain::TransportContainModuleData> {
        use gamelogic::contain_module_overrides::ContainModuleDataKind;
        let template = self.objects.get(&ferry)?.get_template().receiving_client_template.as_ref()?;
        let entry = template.behavior_modules().iter()
            .find(|entry| entry.name.as_str().eq_ignore_ascii_case("RailedTransportContain"))?;
        match ContainModuleDataKind::from_module_data(entry.data.as_ref())? {
            ContainModuleDataKind::RailedTransport(data) => Some(&data.base),
            _ => None,
        }
    }

    /// The railed override tests dock openness, not TransportContain's delay.
    pub(crate) fn railed_dock_reserve_exit(&self, ferry: ObjectId) -> bool {
        self.objects.get(&ferry).is_some_and(|object| object.railed_dock.dock_open)
    }


    /// Rebind serialized object IDs only after every object in this world has
    /// been admitted. Runtime references never bind to a later replacement.
    pub(crate) fn railed_dock_rebind_after_load(&mut self, ferry: ObjectId) {
        let Some(dock) = self.objects.get(&ferry) else { return };
        let docking = dock.railed_dock.docking.map(|reference| self.railed_birth(reference.id).unwrap_or(RailedDockObject { id: reference.id, generation: 0 }));
        let unloading = dock.railed_dock.unloading.map(|reference| self.railed_birth(reference.id).unwrap_or(RailedDockObject { id: reference.id, generation: 0 }));
        if let Some(dock) = self.objects.get_mut(&ferry) {
            dock.railed_dock.docking = docking;
            dock.railed_dock.unloading = unloading;
        }
    }

    fn railed_birth(&self, id: ObjectId) -> Option<RailedDockObject> {
        let object = self.objects.get(&id)?;
        (object.visual_object_generation != 0).then_some(RailedDockObject {
            id, generation: object.visual_object_generation,
        })
    }

    fn railed_birth_alive(&self, reference: RailedDockObject) -> bool {
        reference.generation != 0 && self.objects.get(&reference.id)
            .is_some_and(|object| object.visual_object_generation == reference.generation)
    }

    fn railed_bone_local_optional(
        &self, ferry: ObjectId, name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<Option<Vec3>, String> {
        let source = self.current_native_slave_source(ferry, self.local_player_id().unwrap_or(0))
            .map_err(|error| format!("rail dock source: {error}"))?;
        let transforms = if let Some(assets) = effects.pristine_bones {
            assets.pristine_bone_transforms(&source, name, 0, 1)
        } else {
            #[cfg(feature = "game_client")]
            {
                effects.view.as_deref_mut()
                    .ok_or_else(|| "rail dock requires receiving pristine model assets".to_owned())?
                    .pristine_client_bone_transforms(&source, name, 0, 1)
            }
            #[cfg(not(feature = "game_client"))]
            {
                return Err("rail dock requires receiving pristine model assets".to_owned());
            }
        }?;
        Ok(transforms.first().map(|matrix| {
            let p = matrix.w_axis.truncate();
            Vec3::new(p.x, p.z, p.y)
        }))
    }

    fn railed_bone_local(
        &self, ferry: ObjectId, name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<Vec3, String> {
        // DockUpdate initializes missing pristine positions to zero.
        Ok(self.railed_bone_local_optional(ferry, name, effects)?.unwrap_or(Vec3::ZERO))
    }

    fn railed_exit_position(
        &self, ferry: ObjectId, passenger: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<Vec3, String> {
        let start = self.railed_bone_local(ferry, "DockStart", effects)?;
        let end = self.railed_bone_local(ferry, "DockEnd", effects)?;
        let ferry = self.objects.get(&ferry).ok_or("rail dock disappeared")?;
        if start == Vec3::ZERO {
            return self.objects.get(&passenger).map(|p| p.get_position())
                .ok_or_else(|| "rail passenger disappeared".to_owned());
        }
        Ok(ferry.get_transform_matrix().transform_point3(end))
    }

    /// `action`: begin pulling only after the docker enters tolerance.
    pub(crate) fn railed_dock_action(
        &mut self, ferry: ObjectId, docker: ObjectId,
        config: RailedDockConfig,
    ) -> bool {
        let Some(birth) = self.railed_birth(docker) else { return false };
        if self.objects.get(&ferry).is_some_and(|object| object.railed_dock.docking.is_some_and(|current| current.id == docker && current.generation == birth.generation)) {
            return true;
        }
        let Some(dock) = self.objects.get(&ferry) else { return false };
        let dock_pos = dock.get_position();
        let docker_pos = self.objects.get(&docker).unwrap().get_position();
        let delta = dock_pos - docker_pos;
        let distance = delta.length();
        if distance <= config.tolerance {
            let Some(dock) = self.objects.get_mut(&ferry) else { return false };
            dock.railed_dock.docking = Some(birth);
            dock.railed_dock.pull_per_frame = distance / config.pull_frames as f32;
            self.objects.get_mut(&docker).unwrap().status.selected = false;
            self.admit_object_drawable_deselection(birth.id);
            if !self.railed_birth_alive(birth) { return true; }
            let docker = self.objects.get_mut(&docker).unwrap();
            docker.set_status_unselectable(true);
            docker.set_status_disabled_held(true);
            docker.set_orientation_with_partition_owner(delta.z.atan2(delta.x), &mut self.partition_manager);
        }
        true
    }

    /// `unloadNext`: the selected argument to `unloadSingleObject` is ignored
    /// in the original; the first member of the ordered OpenContain list exits.
    fn railed_dock_unload_next(
        &mut self, ferry: ObjectId,
        config: RailedDockConfig, effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        let Some(ferry_birth) = self.railed_birth(ferry) else { return Ok(()) };
        let Some(container) = self.objects.get_mut(&ferry) else { return Ok(()) };
        container.railed_dock.unloading = None;
        if container.railed_dock.unload_count == 0 { return Ok(()); }
        let Some(container) = self.objects.get(&ferry) else { return Ok(()) };
        let Some(&passenger) = container.contained_unit_ids().first() else { return Ok(()) };
        let Some(birth) = self.railed_birth(passenger) else { return Ok(()) };
        let position = container.get_position();
        let yaw = container.get_orientation();
        let layer = container.pathfind_layer;
        let airborne = container.is_above_terrain_for_exit();
        let contain = self.railed_contain_config(ferry)
            .ok_or_else(|| "rail dock requires receiving RailedTransportContain module data".to_owned())?;
        let aggressive = contain.go_aggressive_on_exit;
        let velocity = contain.keep_container_velocity_on_exit.then_some(container.movement.velocity);
        let exit_delay = contain.exit_delay;
        let reset_mood = contain.reset_mood_check_time_on_exit;
        let armed_riders = contain.armed_riders_upgrade_weapon_set;
        let orient_on_exit = contain.orient_like_container_on_exit;
        let pitch_rate = contain.exit_pitch_rate;
        let exit_bone_position = if contain.exit_bone.is_empty() {
            None
        } else {
            self.railed_bone_local_optional(ferry, &contain.exit_bone, effects)?
                .map(|local| container.get_transform_matrix().transform_point3(local))
        };
        // Resolve before changing containment so unavailable receiving assets
        // cannot leave the passenger half removed.
        let _ = self.railed_exit_position(ferry, passenger, effects)?;
        let container = self.objects.get_mut(&ferry).unwrap();
        if !container.remove_occupant_with_partition_owner(passenger, &mut self.ownership_events, &mut self.partition_manager) {
            return Ok(());
        }
        if self.objects.get(&passenger).is_some_and(|object| object.contained_by.is_some() && object.status.masked) {
            self.change_owned_containment_world_membership(passenger, true);
        }
        self.set_owned_pathfinder_layer(passenger, layer);
        if !self.set_owned_object_position_with_effects(passenger, position, self.frame, effects)
            || !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(()); }
        self.play_container_removing_template_sounds(ferry, passenger, effects);
        if !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(()); }
        if let Some(object) = self.objects.get_mut(&passenger) {
            object.set_status_disabled_held(false);
        }
        if let Some(exit_position) = exit_bone_position {
            if !self.set_owned_object_position_with_effects(passenger, exit_position, self.frame, effects)
                || !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(()); }
        }
        if let Some(object) = self.objects.get_mut(&passenger) {
            if orient_on_exit {
                object.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
            }
            if airborne { object.allow_to_fall = true; }
            if aggressive && object.has_ai_update_interface() {
                object.set_ai_attitude(crate::game_logic::host_strategy_center::HostAiAttitude::Aggressive);
            }
            if let Some(velocity) = velocity {
                object.apply_motive_force(velocity * object.physics_get_mass(), self.frame);
                object.shock_pitch_rate = object.center_of_mass_offset * pitch_rate;
            }
        }
        if let Some(container) = self.objects.get_mut(&ferry) {
            container.frame_exit_not_busy = self.frame.saturating_add(exit_delay);
            if container.contained_unit_ids().is_empty() {
                container.model_condition_bits &= !(1u128 << crate::game_logic::host_enum_table_residual::loaded_model_bit());
            }
        }
        if reset_mood { self.reset_rider_mood_check_on_exit(ferry, passenger); }
        if armed_riders { self.refresh_battle_bus_armed_riders_weapon_set(ferry); }
        if self.railed_birth_alive(ferry_birth) {
            let container = self.objects.get_mut(&ferry).unwrap();
            if container.contained_unit_ids().is_empty() {
                container.railed_dock.dock_open = true;
            }
        }
        if !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(()); }
        if let Some(object) = self.objects.get_mut(&passenger) {
            object.finish_containment_reference_with_partition_owner(None, true, &mut self.partition_manager);
        }
        if !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(()); }
        self.set_owned_pathfinder_layer(passenger, layer);
        if !self.set_owned_object_position_with_effects(passenger, position, self.frame, effects) { return Ok(()); }
        if !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(()); }
        let object = self.objects.get_mut(&passenger).unwrap();
        object.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
        object.set_status_disabled_held(true);
        let destination = self.railed_exit_position(ferry, passenger, effects)?;
        if let Some(container) = self.objects.get_mut(&ferry) {
            let state = &mut container.railed_dock;
            state.push_per_frame = (destination - position).length() / config.push_frames as f32;
            state.unloading = Some(birth);
            if state.unload_count != -1 { state.unload_count -= 1; }
        }
        Ok(())
    }

    pub(crate) fn railed_dock_unload(
        &mut self, ferry: ObjectId, config: RailedDockConfig,
        all: bool, effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        let Some(container) = self.objects.get_mut(&ferry) else { return Ok(()) };
        if all && container.railed_dock.unloading.is_some() { return Ok(()); }
        container.railed_dock.unload_count = if all { -1 } else { 1 };
        self.railed_dock_unload_next(ferry, config, effects)
    }
    /// Run after the base DockUpdate each logic frame.
    pub(crate) fn railed_dock_update(
        &mut self, ferry: ObjectId,
        config: RailedDockConfig, effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        let Some(ferry_birth) = self.railed_birth(ferry) else { return Ok(()) };
        let Some(dock) = self.objects.get(&ferry) else { return Ok(()) };
        let docking = dock.railed_dock.docking;
        let pull_per_frame = dock.railed_dock.pull_per_frame;
        if let Some(docker) = docking {
            if !self.railed_birth_alive(docker) {
                if let Some(dock) = self.objects.get_mut(&ferry) { dock.railed_dock.docking = None; }
            } else if let Some(dock) = self.objects.get(&ferry) {
                let dock_pos = dock.get_position();
                let pos = self.objects.get(&docker.id).unwrap().get_position();
                let delta = (dock_pos - pos).normalize_or_zero() * pull_per_frame;
                let next = Vec3::new(pos.x + delta.x, pos.y, pos.z + delta.z);
                if self.set_owned_object_position_with_effects(docker.id, next, self.frame, effects)
                    && self.railed_birth_alive(docker) && self.railed_birth_alive(ferry_birth)
                {
                    let object = self.objects.get_mut(&docker.id).unwrap();
                    object.model_condition_bits |= 1u128 << crate::game_logic::host_enum_table_residual::moving_model_bit();
                    let dx = dock_pos.x - next.x;
                    let dz = dock_pos.z - next.z;
                    if dx * dx + dz * dz <= 36.0 {
                        object.clear_moving_model_bits();
                        self.cancel_dock_reservation(docker.id);
                        if let Some(object) = self.objects.get_mut(&docker.id) {
                            if object.has_ai_update_interface() {
                                object.set_ai_state(AIState::Idle, self.frame,
                                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events));
                            }
                        }
                        let added = self.objects.get_mut(&ferry)
                            .is_some_and(|object| object.add_occupant(docker.id));
                        if added && self.railed_birth_alive(docker) {
                            self.change_owned_containment_world_membership(docker.id, false);
                            self.stamp_player_who_entered(ferry, docker.id);
                            self.play_container_on_containing_sound(ferry, effects);
                            if self.railed_birth_alive(docker) && self.railed_birth_alive(ferry_birth) {
                                self.objects.get_mut(&docker.id).unwrap()
                                    .finish_containment_reference_with_partition_owner(Some(ferry), true, &mut self.partition_manager);
                                self.play_container_load_sound(ferry, effects);
                            }
                        }
                        if self.railed_birth_alive(ferry_birth) {
                            self.objects.get_mut(&ferry).unwrap().railed_dock.docking = None;
                        }
                    }
                }
            }
        }
        if !self.railed_birth_alive(ferry_birth) { return Ok(()); }
        let dock = self.objects.get(&ferry).unwrap();
        let unloading = dock.railed_dock.unloading;
        let push_per_frame = dock.railed_dock.push_per_frame;
        if let Some(unloader) = unloading {
            if !self.railed_birth_alive(unloader) {
                return self.railed_dock_unload_next(ferry, config, effects);
            }
            let pos = self.objects.get(&unloader.id).unwrap().get_position();
            let mut destination = self.railed_exit_position(ferry, unloader.id, effects)?;
            destination.y = self.terrain_ground_height_at(destination)
                .ok_or_else(|| "rail dock requires receiving terrain height".to_owned())?;
            let delta = (destination - pos).normalize_or_zero() * push_per_frame;
            let next = Vec3::new(pos.x + delta.x, destination.y, pos.z + delta.z);
            if !self.set_owned_object_position_with_effects(unloader.id, next, self.frame, effects)
                || !self.railed_birth_alive(unloader) || !self.railed_birth_alive(ferry_birth) { return Ok(()); }
            let object = self.objects.get_mut(&unloader.id).unwrap();
            object.model_condition_bits |= 1u128 << crate::game_logic::host_enum_table_residual::moving_model_bit();
            if destination.distance_squared(next) <= 9.0 {
                object.clear_moving_model_bits();
                let has_ai = object.has_ai_update_interface();
                if has_ai {
                    object.set_ai_state(AIState::Idle, self.frame,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events));
                }
                object.set_status_disabled_held(false);
                object.set_status_unselectable(false);
                if has_ai && self.objects.get(&ferry).is_some_and(|object| object.has_drawable) {
                    let waiting = self.railed_bone_local(ferry, "DOCKWAITING07", effects)?;
                    if let Some(ferry_object) = self.objects.get(&ferry) {
                        let waiting_world = ferry_object.get_transform_matrix().transform_point3(waiting);
                        self.unit_command_ai_move_to(unloader.id, waiting_world);
                    }
                }
                if self.railed_birth_alive(ferry_birth) {
                    self.railed_dock_unload_next(ferry, config, effects)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{
        ContainModuleKind, ContainModuleMetadata, KindOf, Team, ThingTemplate,
    };

    fn ferry_template(name: &str, prefix: &str) -> ThingTemplate {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Transport)
            .set_health(400.0);
        t.dock_kind = DockKind::RailedTransport;
        t.railed_path_prefix_name = prefix.to_string();
        t.railed_transport_slots = Some(2);
        t.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::RailedTransport,
            slots: Some(2),
            ..Default::default()
        };
        t
    }

    #[test]
    fn rail_pull_advances_only_the_receiving_world_and_tracks_object_birth() {
        fn world() -> (GameLogic, ObjectId, ObjectId) {
            let mut world = GameLogic::new();
            world.templates.insert("ferry".into(), ferry_template("ferry", "f"));
            world.templates.insert("rider".into(), ThingTemplate::new("rider"));
            let ferry = world.create_object("ferry", Team::USA, Vec3::ZERO).unwrap();
            let rider = world.create_object("rider", Team::USA, Vec3::new(20.0, 0.0, 0.0)).unwrap();
            (world, ferry, rider)
        }
        let (mut first, ferry, rider) = world();
        let (mut second, other_ferry, other_rider) = world();
        let config = RailedDockConfig { pull_frames: 4, push_frames: 4, tolerance: 50.0 };
        assert!(first.railed_dock_action(ferry, rider, config));
        assert!(second.railed_dock_action(other_ferry, other_rider, config));
        first.railed_dock_update(ferry, config, &mut Default::default()).unwrap();
        assert_eq!(first.objects[&rider].get_position(), Vec3::new(15.0, 0.0, 0.0));
        assert_eq!(second.objects[&other_rider].get_position(), Vec3::new(20.0, 0.0, 0.0));
        assert!(first.objects[&rider].status.disabled_held);
        let old = first.objects[&ferry].railed_dock.docking.unwrap();
        let replacement = first.objects[&rider].clone();
        first.destroy_object(rider).unwrap();
        first.process_destroy_list();
        first.add_object(replacement);
        assert!(!first.railed_birth_alive(old));
        first.railed_dock_update(ferry, config, &mut Default::default()).unwrap();
        assert!(!first.objects[&ferry].railed_dock.busy());
    }

    #[test]
    fn railed_exit_reservation_uses_dock_open_not_transport_exit_delay() {
        let mut world = GameLogic::new();
        world.templates.insert("ferry".into(), ferry_template("ferry", "f"));
        let ferry = world.create_object("ferry", Team::USA, Vec3::ZERO).unwrap();
        world.objects.get_mut(&ferry).unwrap().frame_exit_not_busy = 100;
        assert!(world.railed_dock_reserve_exit(ferry));
        world.set_railed_in_transit(ferry, true);
        assert!(!world.railed_dock_reserve_exit(ferry));
        world.set_railed_in_transit(ferry, false);
        assert!(world.railed_dock_reserve_exit(ferry));
    }

    #[test]
    fn in_flight_pull_resumes_after_world_snapshot() {
        use crate::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};
        fn install(world: &mut GameLogic) {
            world.admit_object_ini_sources([("rail-snapshot.ini".into(), "
Object ferry
 KindOf = VEHICLE TRANSPORT SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 400
  InitialHealth = 400
 End
 Behavior = RailedTransportContain Contain
  Slots = 2
 End
 Behavior = RailedTransportDockUpdate Dock
  PullInsideDuration = 133ms
  PushOutsideDuration = 133ms
  ToleranceDistance = 50
 End
End
Object rider
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
End
".into())]).unwrap();
        }
        let config = RailedDockConfig { pull_frames: 4, push_frames: 4, tolerance: 50.0 };
        let mut source = GameLogic::new();
        install(&mut source);
        let ferry = source.create_object("ferry", Team::USA, Vec3::ZERO).unwrap();
        let rider = source.create_object("rider", Team::USA, Vec3::new(20.0, 0.0, 0.0)).unwrap();
        assert!(source.railed_dock_action(ferry, rider, config));
        source.railed_dock_update(ferry, config, &mut Default::default()).unwrap();
        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let snapshot: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
        let mut restored = GameLogic::new();
        install(&mut restored);
        builder.restore_from_snapshot(&snapshot, &mut restored).unwrap();
        restored.railed_dock_update(ferry, config, &mut Default::default()).unwrap();
        assert_eq!(restored.objects[&rider].get_position(), Vec3::new(10.0, 0.0, 0.0));
        assert!(restored.objects[&ferry].railed_dock.busy());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn single_exit_uses_first_contained_passenger_and_real_pristine_dock_end() {
        use crate::assets::{W3dHierarchy, W3dHlod, W3dHlodLod, W3dPivot};
        let pivot = |name: &str, parent_idx, translation| W3dPivot {
            name: name.into(), parent_idx, translation,
            euler_angles: [0.; 3], rotation: [0., 0., 0., 1.],
        };
        let mut model = crate::assets::W3DModel::new("RailTestModel".into());
        model.hierarchy = Some(W3dHierarchy {
            name: "RailHierarchy".into(),
            pivots: vec![
                pivot("ROOT", u32::MAX, [0., 0., 0.]),
                pivot("DockStart", 0, [1., 0., 0.]),
                pivot("DockEnd", 0, [20., 0., 0.]),
                pivot("DOCKWAITING07", 0, [30., 0., 0.]),
            ],
            pivot_fixups: Vec::new(),
        });
        model.hlods = vec![W3dHlod {
            version: 0, name: "RailTestModel".into(), hierarchy_name: "RailHierarchy".into(),
            lods: vec![W3dHlodLod { max_screen_size: 1., subobjects: Vec::new() }],
            aggregates: None, proxies: None, has_unrendered_aggregates: false,
            has_invalid_trailing_records: false,
        }];
        let mut assets = crate::assets::AssetManager::new().unwrap();
        assets.admit_test_w3d_source("RailTestModel", model);
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("rail-test.ini".into(), "
Object RailFerry
 KindOf = VEHICLE TRANSPORT SELECTABLE
 Draw = W3DModelDraw Draw
  DefaultConditionState
   Model = RailTestModel
  End
 End
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = RailedTransportContain Contain
  Slots = 2
 End
 Behavior = RailedTransportDockUpdate Dock
 End
End
Object RailPassenger
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
End
".into())]).unwrap();
        let ferry = world.create_object("RailFerry", Team::USA, Vec3::ZERO).unwrap();
        let first = world.create_object("RailPassenger", Team::USA, Vec3::ZERO).unwrap();
        let second = world.create_object("RailPassenger", Team::USA, Vec3::ZERO).unwrap();
        for rider in [first, second] {
            assert!(world.objects.get_mut(&ferry).unwrap().add_occupant(rider));
            GameLogic::set_owned_containment_in_parts(&mut world.objects, &mut world.partition_manager,
                &mut world.ai_groups, &mut world.pathfinding_system, rider, Some(ferry), true);
        }
        let config = RailedDockConfig { pull_frames: 4, push_frames: 4, tolerance: 50.0 };
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(&assets), ..Default::default()
        };
        world.railed_dock_unload(ferry, config, false, &mut effects).unwrap();
        assert_eq!(world.objects[&ferry].contained_unit_ids(), &[second]);
        assert_eq!(world.objects[&ferry].railed_dock.unloading.unwrap().id, first);
        assert_eq!(world.objects[&ferry].railed_dock.unload_count, 0);
        assert_eq!(world.objects[&ferry].railed_dock.push_per_frame, 5.0);
        assert_eq!(world.objects[&first].contained_by, None);
        assert!(world.objects[&first].status.disabled_held);
        assert_eq!(world.objects[&second].contained_by, Some(ferry));
        let width = world.pathfinding_system.grid.width() as u32;
        let height = world.pathfinding_system.grid.height() as u32;
        assert!(world.restore_terrain_heights_from_grid(width, height, &vec![0.0; (width * height) as usize]));
        world.railed_dock_update(ferry, config, &mut effects).unwrap();
        assert_eq!(world.objects[&first].get_position(), Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(world.objects[&ferry].contained_unit_ids(), &[second]);
        for _ in 0..3 {
            world.railed_dock_update(ferry, config, &mut effects).unwrap();
        }
        assert!(!world.objects[&ferry].railed_dock.busy());
        assert!(!world.objects[&first].status.disabled_held);
        assert_eq!(world.objects[&ferry].contained_unit_ids(), &[second]);
    }

    #[test]
    fn execute_railed_transport_follows_start_end_and_closes_dock() {
        let mut logic = GameLogic::new();
        logic.inject_railed_waypoint(HostWaypointSnap {
            id: 101,
            name: "ProbeFerryStart01".into(),
            position: Vec3::new(100.0, 0.0, 0.0),
            link0: Some(102),
        });
        logic.inject_railed_waypoint(HostWaypointSnap {
            id: 102,
            name: "ProbeFerryEnd01".into(),
            position: Vec3::new(200.0, 0.0, 0.0),
            link0: None,
        });

        logic.templates.insert(
            "ProbeFerry".into(),
            ferry_template("ProbeFerry", "ProbeFerry"),
        );
        let id = logic
            .create_object("ProbeFerry", Team::USA, Vec3::ZERO)
            .expect("ferry");

        assert!(logic.execute_railed_transport_for(id));

        let ferry = logic.host_object(id).expect("ferry after execute");
        assert!(ferry.railed_in_transit);
        assert_eq!(ferry.railed_current_path, 0);
        assert_eq!(ferry.ai_state(), AIState::Moving);
        assert_eq!(
            ferry.movement.path,
            vec![Vec3::new(100.0, 0.0, 0.0), Vec3::new(200.0, 0.0, 0.0)]
        );
        assert!(ferry.ultra_accurate);
    }

    #[test]
    fn railed_update_parks_at_nearest_end_then_opens_on_arrival() {
        let mut logic = GameLogic::new();
        logic.inject_railed_waypoint(HostWaypointSnap {
            id: 201,
            name: "ParkFerryStart01".into(),
            position: Vec3::new(0.0, 0.0, 0.0),
            link0: Some(202),
        });
        logic.inject_railed_waypoint(HostWaypointSnap {
            id: 202,
            name: "ParkFerryEnd01".into(),
            position: Vec3::new(10.0, 0.0, 0.0),
            link0: None,
        });

        logic
            .templates
            .insert("ParkFerry".into(), ferry_template("ParkFerry", "ParkFerry"));
        let id = logic
            .create_object("ParkFerry", Team::USA, Vec3::new(11.0, 0.0, 0.0))
            .expect("ferry");

        logic.update_railed_transports();
        {
            let ferry = logic.host_object(id).expect("parked");
            assert!(ferry.railed_in_transit);
            assert_eq!(ferry.railed_current_path, 0);
            assert_eq!(ferry.movement.path, vec![Vec3::new(10.0, 0.0, 0.0)]);
        }

        let frame = logic.frame;
        assert!(logic.set_owned_object_position_with_effects(
            id, Vec3::new(10.0, 0.0, 0.0), frame, &mut Default::default(),
        ));
        {
            let ferry = logic.host_object_mut(id).expect("arrive");
            ferry.movement.path.clear();
            ferry.set_ai_state(
                AIState::Idle,
                frame,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
        }
        logic.update_railed_transports();
        let ferry = logic.host_object(id).expect("dock open");
        assert!(!ferry.railed_in_transit);
    }
}
