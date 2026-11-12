//! Host `RailedTransportAIUpdate` residual (C++ `RailedTransportAIUpdate.cpp`).
//!
//! Leftover crate `RailedTransportAIUpdate::update` / `private_execute_railed_transport`
//! already match C++ but early-out when `OBJECT_REGISTRY` is empty (host-only).
//! This is the live path: authored `PathPrefixName` StartNN/EndNN pairs, transit
//! dock close, and `aiFollowWaypointPath` along the start-waypoint link chain.
//! Docking observes the authored approach queue, DockStart and DockAction
//! before pulling, and containment callbacks share source-ordered removal and
//! FIREPOINT redeployment with generic entry, death and forced-exit paths.

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
    pub approach: Option<RailedDockApproach>,
    /// DockUpdate::xfer persists the pristine local coordinates and loaded flag.
    pub positions: Option<RailedDockPositions>,
    pub queue_moves: HashMap<ObjectId, RailedQueueMove>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct RailedDockPositions {
    enter: Vec3,
    action: Vec3,
    exit: Vec3,
    waiting: Vec<Vec3>,
    /// m_numberApproachPositionBones is not transferred by DockUpdate::xfer.
    #[serde(skip)]
    restored: bool,
}
impl RailedDockPositions {
    fn empty() -> Self { Self { enter: Vec3::ZERO, action: Vec3::ZERO, exit: Vec3::ZERO, waiting: Vec::new(), restored: false } }
}
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct RailedQueueMove {
    pub docker: RailedDockObject,
    pub goal: Vec3,
    pub started: bool,
}



#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct RailedDockApproach {
    docker: RailedDockObject,
    phase: RailedApproachPhase,
    goal: Vec3,
    enter_bone: Vec3,
    action_bone: Vec3,
    started: bool,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
enum RailedApproachPhase { Entry, Dock, Action }

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
        Self { dock_open: true, docking: None, pull_per_frame: 0.0, unloading: None, push_per_frame: 0.0, unload_count: -1, approach: None, positions: None, queue_moves: HashMap::new() }
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

    pub(crate) fn cancel_approach(&mut self, docker: ObjectId) {
        self.queue_moves.remove(&docker);
        if self.approach.is_some_and(|stage| stage.docker.id == docker) {
            self.approach = None;
        }
    }
}

impl GameLogic {
    /// Read the receiving template's parsed module, without inventing a
    /// configuration for objects that do not own this behavior.
    fn railed_dock_data<R>(&self, ferry: ObjectId, read: impl FnOnce(&gamelogic::object::production::railed_transport_dock::RailedTransportDockUpdateData) -> R) -> Option<R> {
        let template = self.objects.get(&ferry)?.get_template().receiving_client_template.as_ref()?;
        let module = template.behavior_modules().iter()
            .find(|entry| entry.name.as_str().eq_ignore_ascii_case("RailedTransportDockUpdate"))?;
        let guard = module.data.read();
        Some(read(guard.downcast_ref()?))
    }

    pub(crate) fn railed_dock_approach_settings(&self, ferry: ObjectId) -> Option<(i32, bool)> {
        self.railed_dock_data(ferry, |data| (data.base.number_approach_positions_data, data.base.is_allow_passthrough))
    }

    pub(crate) fn railed_dock_config(&self, ferry: ObjectId) -> Option<RailedDockConfig> {
        self.railed_dock_data(ferry, |data| RailedDockConfig {
            pull_frames: data.pull_inside_duration_frames,
            push_frames: data.push_outside_duration_frames,
            tolerance: data.tolerance_distance,
        })
    }
    fn railed_contain_config<R>(&self, ferry: ObjectId, read: impl FnOnce(&gamelogic::object::contain::TransportContainModuleData) -> R) -> Option<R> {
        use gamelogic::contain_module_overrides::ContainModuleDataKind;
        let template = self.objects.get(&ferry)?.get_template().receiving_client_template.as_ref()?;
        let entry = template.behavior_modules().iter()
            .find(|entry| entry.name.as_str().eq_ignore_ascii_case("RailedTransportContain"))?;
        let guard = entry.data.read();
        match ContainModuleDataKind::from_module_data(&*guard)? {
            ContainModuleDataKind::RailedTransport(data) => Some(read(&data.base)),
            _ => None,
        }
    }

    /// RailedTransportDockUpdate checks the real Contain interface before
    /// granting entry; a dock without any Contain imposes no extra restriction.
    pub(crate) fn railed_dock_accepts_rider(&self, ferry: ObjectId, docker: ObjectId) -> bool {
        let Some(container) = self.objects.get(&ferry) else { return false };
        if container.thing().template.contain_module.kind == crate::game_logic::ContainModuleKind::None {
            return true;
        }
        let Some(original) = self.objects.get(&docker) else { return false };
        let rider = if original.thing().template.contain_module.kind == crate::game_logic::ContainModuleKind::Parachute {
            original.contained_unit_ids().first().and_then(|id| self.objects.get(id)).unwrap_or(original)
        } else {
            original
        };
        if !container.thing().template.contain_module.leftover_kind_masks_admit(rider.kind_of_cpp_mask())
            || !container.allows_normal_enter_for_relationship(self.normal_enter_relationship(rider, container))
            || !self.normal_enter_controller_matches(rider, container)
        {
            return false;
        }
        let slots = rider.transport_slot_count();
        if slots == 0 { return false; }
        let Some(used) = container.contained_unit_ids().iter().try_fold(0usize, |total, &id| {
            total.checked_add(self.effective_transport_slot_count(id)?)
        }) else { return false };
        used.checked_add(slots).is_some_and(|total| total <= container.transport_capacity())
    }

    /// The railed override tests dock openness, not TransportContain's delay.
    pub(crate) fn railed_dock_reserve_exit(&self, ferry: ObjectId) -> bool {
        self.objects.get(&ferry).is_some_and(|object| object.railed_dock.dock_open)
    }

    /// RailedTransportContain's override runs after TransportContain's
    /// onRemoving and before the passenger's onRemovedFrom callback.
    pub(crate) fn railed_on_removing_complete(&mut self, ferry: ObjectId) {
        if let Some(container) = self.objects.get_mut(&ferry) {
            if container.thing().template.contain_module.kind == crate::game_logic::ContainModuleKind::RailedTransport
                && container.contained_unit_ids().is_empty()
            {
                container.railed_dock.dock_open = true;
            }
        }
    }


    /// Rebind serialized object IDs only after every object in this world has
    /// been admitted. Runtime references never bind to a later replacement.
    pub(crate) fn railed_dock_rebind_after_load(&mut self, ferry: ObjectId) {
        let Some(dock) = self.objects.get(&ferry) else { return };
        let docking = dock.railed_dock.docking.map(|reference| self.railed_birth(reference.id).unwrap_or(RailedDockObject { id: reference.id, generation: 0 }));
        let unloading = dock.railed_dock.unloading.map(|reference| self.railed_birth(reference.id).unwrap_or(RailedDockObject { id: reference.id, generation: 0 }));
        let approach = dock.railed_dock.approach.map(|mut state| {
            state.docker = self.railed_birth(state.docker.id).unwrap_or(RailedDockObject { id: state.docker.id, generation: 0 });
            state
        });
        let queue_moves = dock.railed_dock.queue_moves.iter().map(|(&id, &state)| {
            let docker = self.railed_birth(id).unwrap_or(RailedDockObject { id, generation: 0 });
            (id, RailedQueueMove { docker, ..state })
        }).collect();
        if let Some(dock) = self.objects.get_mut(&ferry) {
            dock.railed_dock.docking = docking;
            dock.railed_dock.unloading = unloading;
            dock.railed_dock.approach = approach;
            dock.railed_dock.queue_moves = queue_moves;
            if let Some(positions) = dock.railed_dock.positions.as_mut() {
                positions.restored = true;
            }
        }
    }

    pub(crate) fn railed_birth(&self, id: ObjectId) -> Option<RailedDockObject> {
        let object = self.objects.get(&id)?;
        (object.visual_object_generation != 0).then_some(RailedDockObject {
            id, generation: object.visual_object_generation,
        })
    }

    pub(crate) fn railed_birth_alive(&self, reference: RailedDockObject) -> bool {
        reference.generation != 0 && self.objects.get(&reference.id)
            .is_some_and(|object| object.visual_object_generation == reference.generation)
    }

    pub(in crate::game_logic) fn containment_pristine_bone_local_optional(
        &self, ferry: ObjectId, name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<Option<Vec3>, String> {
        if self.objects.get(&ferry).is_some_and(|object| !object.has_drawable) {
            return Ok(None);
        }
        let source = self.current_native_slave_source(ferry, self.local_player_id().unwrap_or(0))
            .map_err(|error| format!("rail dock source: {error}"))?;
        // Drawable::getPristineBonePositions over a drawable whose Draw
        // modules author no model finds no bones; DockUpdate then keeps its
        // zero positions. No model assets are needed to learn that.
        if source.receiving_draw_models.select(source.condition_bits).is_empty() {
            return Ok(None);
        }
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
        Ok(self.containment_pristine_bone_local_optional(ferry, name, effects)?.unwrap_or(Vec3::ZERO))
    }

    /// DockUpdate loads all three coordinates together once per drawable
    /// lifetime. With no drawable the source leaves positionsLoaded false.
    pub(crate) fn railed_dock_positions(
        &mut self, ferry: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<RailedDockPositions, String> {
        let dock = self.objects.get(&ferry).ok_or("rail dock disappeared")?;
        if let Some(positions) = dock.railed_dock.positions.clone() {
            return Ok(positions);
        }
        if !dock.has_drawable {
            return Ok(RailedDockPositions::empty());
        }
        let ignore = dock.kind_of_cpp_mask() & (1u128 << crate::game_logic::host_enum_table_residual::KINDOF_IGNORE_DOCKING_BONES) != 0;
        let positions = if ignore {
            RailedDockPositions::empty()
        } else {
            RailedDockPositions {
                enter: self.railed_bone_local(ferry, "DockStart", effects)?,
                action: self.railed_bone_local(ferry, "DockAction", effects)?,
                exit: self.railed_bone_local(ferry, "DockEnd", effects)?,
                waiting: {
                    let count = self.railed_dock_approach_settings(ferry).map_or(0, |settings| settings.0);
                    if count > 0 {
                        self.railed_pristine_bones(ferry, "DockWaiting", 1, count as usize, effects)?
                            .into_iter().map(|matrix| {
                                let p = matrix.w_axis;
                                Vec3::new(p.x, p.z, p.y)
                            }).collect()
                    } else {
                        Vec::new()
                    }
                },
                restored: false,
            }
        };
        self.objects.get_mut(&ferry).ok_or("rail dock disappeared")?.railed_dock.positions = Some(positions.clone());
        Ok(positions)
    }

    pub(crate) fn railed_dock_waiting_world(
        &mut self, ferry: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<Vec<Vec3>, String> {
        let positions = self.railed_dock_positions(ferry, effects)?;
        let dock = self.objects.get(&ferry).ok_or("rail dock disappeared")?;
        let transform = dock.get_transform_matrix();
        if positions.restored && positions.waiting.is_empty() {
            // The constructor's -1 bone count survives Xfer, so unlike the
            // runtime zero count the restored dock does not apply boneless bias.
            return Ok(vec![dock.get_position()]);
        }
        Ok(positions.waiting.into_iter().map(|point| transform.transform_point3(point)).collect())
    }

    fn railed_exit_position(
        &mut self, ferry: ObjectId, passenger: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<Vec3, String> {
        let positions = self.railed_dock_positions(ferry, effects)?;
        let ferry = self.objects.get(&ferry).ok_or("rail dock disappeared")?;
        if positions.enter == Vec3::ZERO {
            return self.objects.get(&passenger).map(|p| p.get_position())
                .ok_or_else(|| "rail passenger disappeared".to_owned());
        }
        Ok(ferry.get_transform_matrix().transform_point3(positions.exit))
    }

    /// Clearance leads to DockStart and then DockAction, not directly to
    /// RailedTransportDockUpdate::action.
    pub(crate) fn railed_dock_approach(
        &mut self, ferry: ObjectId, docker: ObjectId, config: RailedDockConfig,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        let Some(birth) = self.railed_birth(docker) else { return Ok(()) };
        let prior = self.objects.get(&ferry).and_then(|dock| dock.railed_dock.approach)
            .filter(|stage| stage.docker.id == docker && stage.docker.generation == birth.generation);
        if prior.is_none() {
            let positions = self.railed_dock_positions(ferry, effects)?;
            let start = positions.enter;
            let action = positions.action;
            let dock = self.objects.get(&ferry).ok_or("rail dock disappeared")?;
            let rider = self.objects.get(&docker).ok_or("rail docker disappeared")?;
            let goal = if start == Vec3::ZERO {
                if rider.selected_locomotor_is_airborne() { dock.get_position() } else { rider.get_position() }
            } else {
                dock.get_transform_matrix().transform_point3(start)
            };
            self.objects.get_mut(&ferry).unwrap().railed_dock.approach =
                Some(RailedDockApproach { docker: birth, phase: RailedApproachPhase::Entry, goal, enter_bone: start, action_bone: action, started: false });
        }
        loop {
            let Some(stage) = self.objects.get(&ferry).and_then(|dock| dock.railed_dock.approach) else { return Ok(()) };
            if !self.railed_birth_alive(stage.docker) { return Ok(()); }
            if stage.phase == RailedApproachPhase::Action {
                self.railed_dock_action(ferry, docker, config);
                return Ok(());
            }
            let rider = self.objects.get(&docker).unwrap();
            let reached = rider.get_position() == stage.goal
                || (stage.started && Self::host_internal_move_reached_goal(rider));
            if !reached {
                if !stage.started {
                    let state = rider.ai_state().clone();
                    let allows_passthrough = self.railed_dock_approach_settings(ferry)
                        .ok_or("rail dock approach settings unavailable")?.1;
                    let requested = if allows_passthrough {
                        self.path_approach_with_state_ignoring(docker, stage.goal, state, Some(ferry))
                    } else {
                        self.path_approach_with_state(docker, stage.goal, state)
                    };
                    if requested {
                        if let Some(stage) = self.objects.get_mut(&ferry).and_then(|dock| dock.railed_dock.approach.as_mut()) {
                            stage.started = true;
                        }
                    }
                }
                return Ok(());
            }
            if stage.phase == RailedApproachPhase::Entry {
                self.host_dock_enter_reached(ferry, docker);
                let beginning = crate::game_logic::host_enum_table_residual::docking_beginning_model_bit();
                let ending = crate::game_logic::host_enum_table_residual::docking_ending_model_bit();
                let docking = crate::game_logic::host_enum_table_residual::model_condition_bit_name_index("DOCKING").expect("DOCKING model condition");
                for id in [ferry, docker] {
                    if let Some(object) = self.objects.get_mut(&id) {
                        object.model_condition_bits &= !(1u128 << ending);
                        object.model_condition_bits |= (1u128 << beginning) | (1u128 << docking);
                    }
                }
                let dock = self.objects.get(&ferry).ok_or("rail dock disappeared")?;
                let goal = if stage.enter_bone == Vec3::ZERO {
                    self.objects.get(&docker).ok_or("rail docker disappeared")?.get_position()
                } else {
                    dock.get_transform_matrix().transform_point3(stage.action_bone)
                };
                self.objects.get_mut(&ferry).unwrap().railed_dock.approach =
                    Some(RailedDockApproach { phase: RailedApproachPhase::Dock, goal, started: false, ..stage });
            } else {
                let beginning = crate::game_logic::host_enum_table_residual::docking_beginning_model_bit();
                let active = crate::game_logic::host_enum_table_residual::docking_active_model_bit();
                for id in [ferry, docker] {
                    if let Some(object) = self.objects.get_mut(&id) {
                        object.model_condition_bits &= !(1u128 << beginning);
                        object.model_condition_bits |= 1u128 << active;
                    }
                }
                self.objects.get_mut(&ferry).unwrap().railed_dock.approach =
                    Some(RailedDockApproach { phase: RailedApproachPhase::Action, ..stage });
            }
        }
    }

    fn railed_pristine_bones(
        &self, ferry: ObjectId, name: &str, start: i32, limit: usize,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<Vec<glam::Mat4>, String> {
        if self.objects.get(&ferry).is_some_and(|object| !object.has_drawable) {
            return Ok(Vec::new());
        }
        let source = self.current_native_slave_source(ferry, self.local_player_id().unwrap_or(0))
            .map_err(|error| format!("rail containment source: {error}"))?;
        if let Some(assets) = effects.pristine_bones {
            return assets.pristine_bone_transforms(&source, name, start, limit);
        }
        #[cfg(feature = "game_client")]
        {
            effects.view.as_deref_mut()
                .ok_or_else(|| "rail containment requires receiving pristine model assets".to_owned())?
                .pristine_client_bone_transforms(&source, name, start, limit)
        }
        #[cfg(not(feature = "game_client"))]
        Err("rail containment requires receiving pristine model assets".to_owned())
    }

    /// Object::getSingleLogicalBonePositionOnTurret: rotate the pristine
    /// FIREPOINT about the primary turret's projectile-launch rotation pivot.
    fn railed_turret_firepoint(
        &self, ferry: ObjectId, index: usize,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<Vec3, String> {
        let source = self.current_native_slave_source(ferry, self.local_player_id().unwrap_or(0))
            .map_err(|error| format!("rail turret source: {error}"))?;
        let selected = source.receiving_draw_models.object_draws(source.condition_bits)
            .map_err(|_| "rail turret Draw state unavailable".to_owned())?;
        // Drawable::getProjectileLaunchOffset visits each ObjectDraw when
        // launchPos is null; each selected state writes the turret pivot.
        let draw = selected.last().ok_or("rail turret has no receiving ObjectDraw")?;
        let assets = effects.pristine_bones
            .ok_or("rail turret requires selected-Draw pristine asset access")?;
        let pivot = if let Some(name) = draw.primary_turret.yaw_bone.as_deref() {
            assets.pristine_bones_for_selected_draw(draw, source.mesh_scale, name, 0, 1)?
                .first().map(|matrix| matrix.w_axis.truncate()).unwrap_or(glam::Vec3::ZERO)
        } else {
            glam::Vec3::ZERO
        };
        let attachment = if let Some(name) = draw.attach_to_drawable_bone.as_deref() {
            self.railed_pristine_bones(ferry, name, 0, 1, effects)?
                .first().map(|matrix| matrix.w_axis.truncate()).unwrap_or(glam::Vec3::ZERO)
        } else {
            glam::Vec3::ZERO
        };
        let name = format!("FIREPOINT{:02}", index + 1);
        let bone = self.railed_pristine_bones(ferry, &name, 0, 1, effects)?
            .first().map(|matrix| matrix.w_axis.truncate())
            .ok_or_else(|| format!("rail turret logical bone {name} unavailable"))?;
        let container = self.objects.get(&ferry).ok_or("rail dock disappeared")?;
        if !container.has_ai_update_interface() || !container.turret_ai.turret_enabled {
            return Err("rail turret logical bone requires a primary TurretAI".into());
        }
        let pivot = pivot + attachment;
        let local = pivot + glam::Quat::from_rotation_z(container.turret_ai.turret_angle_deg.to_radians())
            * (bone - pivot);
        Ok(container.get_transform_matrix().transform_point3(Vec3::new(local.x, local.z, local.y)))
    }

    /// OpenContain's post-insertion placement uses reverse list order and
    /// cycles through the receiving model's logical FIREPOINT bones.
    fn railed_redeploy_occupants(
        &mut self,
        ferry: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        let Some(ferry_birth) = self.railed_birth(ferry) else { return Ok(()) };
        let container = self.objects.get(&ferry).unwrap();
        let riders: Vec<_> = container.contained_unit_ids().iter().rev()
            .filter_map(|&id| self.railed_birth(id)).collect();
        let transform = container.get_transform_matrix();
        let turret = self.railed_contain_config(ferry, |data| data.base.passengers_in_turret)
            .unwrap_or(false);
        let points = self.railed_pristine_bones(ferry, "FIREPOINT", 1, 32, effects)?;
        for (index, rider) in riders.into_iter().enumerate() {
            if !self.railed_birth_alive(ferry_birth) { break; }
            if !self.railed_birth_alive(rider) { continue; }
            if !self.objects[&ferry].contained_unit_ids().contains(&rider.id) { continue; }
            let position = if points.is_empty() {
                self.objects.get(&ferry).unwrap().get_position()
            } else if turret {
                self.railed_turret_firepoint(ferry, index % points.len(), effects)?
            } else {
                let p = points[index % points.len()].w_axis;
                transform.transform_point3(Vec3::new(p.x, p.z, p.y))
            };
            self.set_owned_object_position_with_effects(rider.id, position, self.frame, effects);
        }
        Ok(())
    }

    pub(crate) fn railed_add_contained_with_effects(
        &mut self, ferry: ObjectId, passenger: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<bool, String> {
        let Some(ferry_birth) = self.railed_birth(ferry) else { return Ok(false) };
        let Some(rider_birth) = self.railed_birth(passenger) else { return Ok(false) };
        if self.should_cancel_containment_after_booby_trap_with_effects(ferry, passenger, effects)
            || !self.railed_birth_alive(ferry_birth) || !self.railed_birth_alive(rider_birth)
            || self.objects[&passenger].contained_by.is_some()
        {
            return Ok(false);
        }
        if !self.objects.get_mut(&ferry).unwrap().push_railed_contained_rider(passenger) { return Ok(false); }
        self.stamp_player_who_entered(ferry, passenger);
        self.change_owned_containment_world_membership(passenger, false);
        self.railed_redeploy_occupants(ferry, effects)?;
        if !self.railed_birth_alive(ferry_birth) || !self.railed_birth_alive(rider_birth) { return Ok(true); }
        if !self.objects[&ferry].contained_unit_ids().contains(&passenger)
            || self.objects[&passenger].contained_by.is_some() { return Ok(true); }
        self.play_container_on_containing_sound(ferry, effects);
        if !self.railed_birth_alive(ferry_birth) || !self.railed_birth_alive(rider_birth) { return Ok(true); }
        if !self.objects[&ferry].contained_unit_ids().contains(&passenger)
            || self.objects[&passenger].contained_by.is_some() { return Ok(true); }
        self.objects.get_mut(&passenger).unwrap().set_status_disabled_held(true);
        if let Some(container) = self.objects.get_mut(&ferry) {
            if container.contained_unit_ids().len() == 1 && container.has_drawable {
                container.model_condition_bits |= 1u128 << crate::game_logic::host_enum_table_residual::loaded_model_bit();
            }
        }
        if self.railed_contain_config(ferry, |data| data.armed_riders_upgrade_weapon_set).unwrap_or(false) {
            self.refresh_battle_bus_armed_riders_weapon_set(ferry);
        }
        if !self.railed_birth_alive(ferry_birth) || !self.railed_birth_alive(rider_birth) { return Ok(true); }
        if !self.objects[&ferry].contained_unit_ids().contains(&passenger)
            || self.objects[&passenger].contained_by.is_some() { return Ok(true); }
        self.objects.get_mut(&passenger).unwrap()
            .finish_containment_reference_with_partition_owner(Some(ferry), true, &mut self.partition_manager);
        self.play_container_load_sound(ferry, effects);
        Ok(true)
    }

    /// `action`: begin pulling only after the docker enters tolerance.
    pub(crate) fn railed_dock_action(
        &mut self, ferry: ObjectId, docker: ObjectId,
        config: RailedDockConfig,
    ) -> bool {
        let Some(birth) = self.railed_birth(docker) else { return false };
        let Some(ferry_birth) = self.railed_birth(ferry) else { return false };
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
            self.objects.get_mut(&docker).unwrap().status.selected = false;
            self.admit_object_drawable_deselection(birth.id);
            if !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return true; }
            let docker_object = self.objects.get_mut(&docker).unwrap();
            docker_object.set_status_unselectable(true);
            docker_object.set_status_disabled_held(true);
            self.objects.get_mut(&ferry).unwrap().railed_dock.pull_per_frame =
                distance / config.pull_frames as f32;
            self.objects.get_mut(&docker).unwrap()
                .set_orientation_with_partition_owner(delta.z.atan2(delta.x), &mut self.partition_manager);
        }
        true
    }

    /// OpenContain::removeFromContain with the rail override at its original
    /// callback boundary. Also used by forced and destruction removals.
    pub(crate) fn railed_remove_contained_with_effects(
        &mut self, ferry: ObjectId, passenger: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<bool, String> {
        self.railed_remove_contained_inner(ferry, passenger, false, effects)
    }

    /// killAllContained skips world re-admission and the unload cue.
    pub(crate) fn railed_kill_contained_with_effects(
        &mut self, ferry: ObjectId, passenger: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<bool, String> {
        self.railed_remove_contained_inner(ferry, passenger, true, effects)
    }

    fn railed_remove_contained_inner(
        &mut self, ferry: ObjectId, passenger: ObjectId, killing: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<bool, String> {
        let Some(ferry_birth) = self.railed_birth(ferry) else { return Ok(false) };
        let Some(birth) = self.railed_birth(passenger) else { return Ok(false) };
        if !self.objects.get(&passenger).is_some_and(|rider| rider.contained_by == Some(ferry)) {
            return Ok(false);
        }
        let container = self.objects.get(&ferry).unwrap();
        if !container.contained_unit_ids().contains(&passenger) { return Ok(false); }
        let position = container.get_position();
        let yaw = container.get_orientation();
        let layer = container.pathfind_layer;
        let airborne = container.is_above_terrain_for_exit();
        let (aggressive, velocity, exit_delay, reset_mood, armed_riders, orient_on_exit, pitch_rate, exit_bone_position) = self.railed_contain_config(ferry, |contain| {
            let exit_bone_position = if contain.exit_bone.is_empty() {
                Ok(None)
            } else {
                self.containment_pristine_bone_local_optional(ferry, &contain.exit_bone, effects)
                    .map(|position| position.map(|local| container.get_transform_matrix().transform_point3(local)))
            };
            (contain.go_aggressive_on_exit,
             contain.keep_container_velocity_on_exit.then_some(container.movement.velocity),
             contain.exit_delay, contain.reset_mood_check_time_on_exit,
             contain.armed_riders_upgrade_weapon_set, contain.orient_like_container_on_exit,
             contain.exit_pitch_rate, exit_bone_position)
        }).ok_or_else(|| "rail containment requires receiving RailedTransportContain module data".to_owned())?;
        let exit_bone_position = exit_bone_position?;
        if !self.objects.get_mut(&ferry).unwrap().remove_occupant_with_partition_owner(
            passenger, &mut self.ownership_events, &mut self.partition_manager,
        ) { return Ok(false); }
        if killing {
            self.play_container_on_removing_template_sounds(ferry, passenger, effects);
        } else {
            self.change_owned_containment_world_membership(passenger, true);
            if !self.set_owned_object_position_with_effects(passenger, position, self.frame, effects)
                || !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(true); }
            self.set_owned_pathfinder_layer(passenger, layer);
            self.play_container_removing_template_sounds(ferry, passenger, effects);
        }
        if !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(true); }
        self.objects.get_mut(&passenger).unwrap().set_status_disabled_held(false);
        if let Some(exit_position) = exit_bone_position {
            if !self.set_owned_object_position_with_effects(passenger, exit_position, self.frame, effects)
                || !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(true); }
        }
        if let Some(object) = self.objects.get_mut(&passenger) {
            if orient_on_exit { object.set_orientation_with_partition_owner(yaw, &mut self.partition_manager); }
            if let Some(velocity) = velocity {
                object.apply_motive_force(velocity * object.physics_get_mass(), self.frame);
                object.shock_pitch_rate = object.center_of_mass_offset * pitch_rate;
            }
        }
        if let Some(container) = self.objects.get_mut(&ferry) {
            if container.contained_unit_ids().is_empty() {
                container.model_condition_bits &= !(1u128 << crate::game_logic::host_enum_table_residual::loaded_model_bit());
            }
        }
        if let Some(object) = self.objects.get_mut(&passenger) {
            if airborne { object.allow_to_fall = true; }
            if aggressive && object.has_ai_update_interface() {
                object.set_ai_attitude(crate::game_logic::host_strategy_center::HostAiAttitude::Aggressive);
            }
        }
        self.scatter_dying_transport_rider_with_effects(ferry, passenger, effects);
        if !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(true); }
        if reset_mood { self.reset_rider_mood_check_on_exit(ferry, passenger); }
        if let Some(container) = self.objects.get_mut(&ferry) {
            container.frame_exit_not_busy = self.frame.wrapping_add(exit_delay);
        }
        if armed_riders { self.refresh_battle_bus_armed_riders_weapon_set(ferry); }
        self.transfer_kell_snipe_reload_from_bike(ferry, passenger);
        if self.railed_birth_alive(ferry_birth) { self.railed_on_removing_complete(ferry); }
        if !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(true); }
        self.objects.get_mut(&passenger).unwrap()
            .finish_containment_reference_with_partition_owner(None, true, &mut self.partition_manager);
        Ok(true)
    }

    /// `unloadNext` ignores the selected argument and exits the first rider.
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
        if !self.railed_remove_contained_with_effects(ferry, passenger, effects)?
            || !self.railed_birth_alive(birth) || !self.railed_birth_alive(ferry_birth) { return Ok(()); }
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
                        self.railed_add_contained_with_effects(ferry, docker.id, effects)?;
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
                    self.unit_command_ai_idle_owned_with_effects(unloader.id, effects)
                        .map_err(|error| error.to_string())?;
                    if !self.railed_birth_alive(unloader) || !self.railed_birth_alive(ferry_birth) { return Ok(()); }
                }
                let object = self.objects.get_mut(&unloader.id).unwrap();
                object.set_status_disabled_held(false);
                object.set_status_unselectable(false);
                if has_ai && self.objects.get(&ferry).is_some_and(|object| object.has_drawable) {
                    let waiting = self.containment_pristine_bone_local_optional(ferry, "DOCKWAITING07", effects)?
                        .ok_or("rail dock exit requires DOCKWAITING07")?;
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
    fn authored_supply_rail_dock_uses_its_own_queue_and_rider_slots() {
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("supply-rail.ini".into(), "
Object SupplyRail
 KindOf = VEHICLE TRANSPORT SUPPLY_SOURCE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = RailedTransportContain Contain
  Slots = 3
 End
 Behavior = RailedTransportDockUpdate Dock
  NumberApproachPositions = 2
  AllowsPassthrough = No
 End
End
Object HeavyDocker
 KindOf = VEHICLE SELECTABLE
 TransportSlotCount = 2
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
End
".into())]).unwrap();
        world.add_player(crate::game_logic::Player::new(0, Team::USA, "Owner", true));
        let ferry = world.create_object_for_player("SupplyRail", 0, Vec3::ZERO).unwrap();
        let first = world.create_object_for_player("HeavyDocker", 0, Vec3::ZERO).unwrap();
        let second = world.create_object_for_player("HeavyDocker", 0, Vec3::ZERO).unwrap();
        assert_eq!(world.railed_dock_approach_settings(ferry), Some((2, false)));
        assert!(world.railed_dock_accepts_rider(ferry, first));
        assert!(world.objects.get_mut(&ferry).unwrap().add_occupant(first));
        assert!(!world.railed_dock_accepts_rider(ferry, second));
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
    fn rail_boarding_redeploys_all_riders_before_containment_callback() {
        let mut world = GameLogic::new();
        world.templates.insert("ferry".into(), ferry_template("ferry", "f"));
        world.templates.insert("rider".into(), ThingTemplate::new("rider"));
        let ferry = world.create_object("ferry", Team::USA, Vec3::ZERO).unwrap();
        let first = world.create_object("rider", Team::USA, Vec3::new(30.0, 0.0, 0.0)).unwrap();
        let second = world.create_object("rider", Team::USA, Vec3::new(6.0, 0.0, 0.0)).unwrap();
        world.objects.get_mut(&ferry).unwrap().has_drawable = false;
        assert!(world.objects.get_mut(&ferry).unwrap().add_occupant(first));
        GameLogic::set_owned_containment_in_parts(&mut world.objects, &mut world.partition_manager,
            &mut world.ai_groups, &mut world.pathfinding_system, first, Some(ferry), true);
        let config = RailedDockConfig { pull_frames: 4, push_frames: 4, tolerance: 50.0 };
        assert!(world.railed_dock_action(ferry, second, config));
        world.railed_dock_update(ferry, config, &mut Default::default()).unwrap();
        assert_eq!(world.objects[&ferry].contained_unit_ids(), &[first, second]);
        assert_eq!(world.objects[&first].get_position(), Vec3::ZERO);
        assert_eq!(world.objects[&second].get_position(), Vec3::ZERO);
        assert_eq!(world.objects[&second].contained_by, Some(ferry));
    }

    #[test]
    fn boarding_detonates_real_booby_trap_before_adding_the_rider() {
        let mut world = GameLogic::new();
        world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
            "rail-trap.ini".into(),
            "Object BoobyTrap\nKindOf = IMMOBILE BOOBY_TRAP\nBody = ActiveBody Body\nMaxHealth = 1\nInitialHealth = 1\nEnd\nBehavior = StickyBombUpdate Sticky\nEnd\nEnd\n\
Object ferry\nKindOf = VEHICLE TRANSPORT SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 400\nInitialHealth = 400\nEnd\nBehavior = RailedTransportContain Contain\nSlots = 2\nEnd\nBehavior = RailedTransportDockUpdate Dock\nEnd\nEnd\n\
Object rider\nKindOf = INFANTRY SELECTABLE\nTransportSlotCount = 1\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into(),
        )]).unwrap()).unwrap();
        let mut planter_player = crate::game_logic::Player::new(0, Team::GLA, "Planter", true);
        planter_player.set_map_relationship(1, gamelogic::common::Relationship::Enemies);
        world.add_player(planter_player);
        world.add_player(crate::game_logic::Player::new(1, Team::USA, "Docker", true));
        let ferry = world.create_object_for_player("ferry", 1, Vec3::ZERO).unwrap();
        let planter = world.create_object_for_player("rider", 0, Vec3::new(30., 0., 0.)).unwrap();
        let rider = world.create_object_for_player("rider", 1, Vec3::new(6., 0., 0.)).unwrap();
        world.objects.get_mut(&ferry).unwrap().has_drawable = false;
        let charge = world.spawn_booby_trap_special_object(planter, Team::GLA, ferry).unwrap();
        assert_eq!(world.objects[&charge].producer_id, Some(ferry));
        assert_eq!(world.objects[&charge].owner_player_id, Some(0));
        assert_eq!(world.objects[&rider].owner_player_id, Some(1));
        assert_eq!(world.player_relationship(0, 1), gamelogic::common::Relationship::Enemies);
        assert!(world.objects[&ferry].status.booby_trapped);
        let mut plants = world.booby_trap_residual().clone();
        plants.install(ferry, planter, Team::GLA, world.frame, 8.0, None);
        plants.set_charge_object(ferry, charge);
        world.restore_booby_traps(plants, world.booby_trap_objects_spawned());
        // ThingFactory attaches PartitionData immediately; its dirty cells
        // become visible to Object::checkAndDetonateBoobyTrap only at the
        // original PartitionManager update phase, before a later dock tick.
        world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
        assert!(world.object_ids_near(Vec3::ZERO, 25.0).contains(&charge));
        let config = RailedDockConfig { pull_frames: 4, push_frames: 4, tolerance: 50.0 };
        assert!(world.railed_dock_action(ferry, rider, config));
        assert_eq!(world.objects[&ferry].railed_dock.docking.unwrap().id, rider);
        assert_eq!(world.objects[&rider].contained_by, None);
        world.railed_dock_update(ferry, config, &mut Default::default()).unwrap();
        assert!(!world.booby_trap_residual().is_booby_trapped(ferry));
        assert!(world.objects.get(&charge).is_none_or(|charge| !charge.is_alive() || charge.status.destroyed));
        assert_eq!(world.objects[&rider].contained_by, Some(ferry));
    }

    #[test]
    fn direct_removal_reopens_only_after_last_rider() {
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("rail-removal.ini".into(), "
Object ferry
 KindOf = VEHICLE TRANSPORT SELECTABLE
 SoundExit = RailExit
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = RailedTransportContain Contain
  Slots = 2
  ExitSound = RailUnload
 End
 Behavior = RailedTransportDockUpdate Dock
 End
End
Object rider
 KindOf = VEHICLE SELECTABLE
 SoundFallingFromPlane = RailFalling
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
End
".into())]).unwrap();
        let ferry = world.create_object("ferry", Team::USA, Vec3::ZERO).unwrap();
        let first = world.create_object("rider", Team::USA, Vec3::ZERO).unwrap();
        let second = world.create_object("rider", Team::USA, Vec3::ZERO).unwrap();
        for rider in [first, second] {
            assert!(world.objects.get_mut(&ferry).unwrap().add_occupant(rider));
            GameLogic::set_owned_containment_in_parts(&mut world.objects, &mut world.partition_manager,
                &mut world.ai_groups, &mut world.pathfinding_system, rider, Some(ferry), true);
        }
        world.set_railed_in_transit(ferry, true);
        world.frame = 1;
        let mut audio = crate::game_logic::containment_audio_test_support::manager(&["RailUnload", "RailExit", "RailFalling"]);
        world.remove_direct_contained_child_with_effects(ferry, first, &mut crate::game_logic::ScriptPresentationEffects {
            audio: Some(&mut audio), ..Default::default()
        }).unwrap();
        crate::game_logic::containment_audio_test_support::assert_events(&audio, &[
            ("RailUnload", ferry), ("RailExit", ferry), ("RailFalling", first),
        ]);
        assert!(!world.railed_dock_reserve_exit(ferry));
        assert_eq!(world.objects[&first].contained_by, None);
        world.remove_direct_contained_child(ferry, second).unwrap();
        assert!(world.railed_dock_reserve_exit(ferry));
        assert_eq!(world.objects[&second].contained_by, None);
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
    fn dock_approaches_start_then_action_and_redeploys_on_attached_turret() {
        use crate::assets::{W3dHierarchy, W3dHlod, W3dHlodLod, W3dPivot};
        let make_model = |name: &str, bones: &[(&str, [f32; 3])]| {
            let mut model = crate::assets::W3DModel::new(name.into());
            let mut pivots = vec![W3dPivot { name: "ROOT".into(), parent_idx: u32::MAX,
                translation: [0.; 3], euler_angles: [0.; 3], rotation: [0., 0., 0., 1.] }];
            pivots.extend(bones.iter().map(|(name, translation)| W3dPivot {
                name: (*name).into(), parent_idx: 0, translation: *translation,
                euler_angles: [0.; 3], rotation: [0., 0., 0., 1.],
            }));
            model.hierarchy = Some(W3dHierarchy { name: name.into(), pivots, pivot_fixups: Vec::new() });
            model.hlods = vec![W3dHlod {
                version: 0, name: name.into(), hierarchy_name: name.into(),
                lods: vec![W3dHlodLod { max_screen_size: 1., subobjects: Vec::new() }],
                aggregates: None, proxies: None, has_unrendered_aggregates: false,
                has_invalid_trailing_records: false,
            }];
            model
        };
        let mut assets = crate::assets::AssetManager::new().unwrap();
        assets.admit_test_w3d_source("RailHull", make_model("RailHull", &[
            ("DockStart", [20., 0., 0.]), ("DockAction", [10., 0., 0.]),
            ("DockWaiting01", [50., 0., 0.]),
            ("FIREPOINT01", [10., 0., 0.]), ("attach", [2., 0., 0.]),
            ("turret", [90., 0., 0.]),
        ]));
        assets.admit_test_w3d_source("RailTurret", make_model("RailTurret", &[
            ("turret", [4., 0., 0.]),
        ]));
        let mut world = GameLogic::new();
        let ini = "
Object TurretRailFerry
 KindOf = VEHICLE TRANSPORT SELECTABLE
 Draw = W3DModelDraw Hull
  DefaultConditionState
   Model = RailHull
  End
 End
 Draw = W3DModelDraw TurretDraw
  AttachToBoneInAnotherModule = attach
  DefaultConditionState
   Model = RailTurret
   Turret = turret
  End
 End
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = AIUpdateInterface Ai
 End
 Behavior = RailedTransportContain Contain
  Slots = 2
  PassengersInTurret = Yes
 End
 Behavior = RailedTransportDockUpdate Dock
  NumberApproachPositions = 1
 End
End
Object TurretRailRider
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
End
";
        world.admit_object_ini_sources([("rail-turret.ini".into(), ini.into())]).unwrap();
        let ferry = world.create_object("TurretRailFerry", Team::USA, Vec3::ZERO).unwrap();
        let rider = world.create_object("TurretRailRider", Team::USA, Vec3::new(40., 0., 0.)).unwrap();
        let hull = world.objects.get_mut(&ferry).unwrap();
        hull.turret_ai.turret_enabled = true;
        hull.turret_ai.turret_angle_deg = 90.;
        let config = RailedDockConfig { pull_frames: 4, push_frames: 4, tolerance: 50. };
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(&assets), ..Default::default()
        };
        assert!(!world.try_claim_railed_dock(ferry, rider, &mut effects).unwrap());
        assert_eq!(world.objects[&ferry].dock_active_docker, None,
            "being within a pathfinding cell must not complete the approach");
        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let queued = builder.create_world_snapshot(&world).unwrap();
        let queued_bytes = bincode_legacy::serialize(&queued).unwrap();
        let queued: crate::save_load::snapshot::WorldSnapshot = bincode_legacy::deserialize(&queued_bytes).unwrap();
        let mut queued_world = GameLogic::new();
        queued_world.admit_object_ini_sources([("rail-turret.ini".into(), ini.into())]).unwrap();
        builder.restore_from_snapshot(&queued, &mut queued_world).unwrap();
        assert!(!queued_world.try_claim_railed_dock(ferry, rider, &mut Default::default()).unwrap());
        assert_eq!(queued_world.objects[&ferry].dock_active_docker, None);
        assert_eq!(queued_world.objects[&ferry].railed_dock.queue_moves[&rider].goal, Vec3::new(50., 0., 0.));
        world.cancel_dock_reservation(rider);
        world.railed_dock_approach(ferry, rider, config, &mut effects).unwrap();
        // DockUpdate::xfer owns the loaded coordinates; the receiving world
        // must not consult a later model state when continuing the approach.
        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let snapshot: crate::save_load::snapshot::WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
        let mut restored = GameLogic::new();
        restored.admit_object_ini_sources([("rail-turret.ini".into(), ini.into())]).unwrap();
        builder.restore_from_snapshot(&snapshot, &mut restored).unwrap();
        let saved = restored.railed_dock_positions(ferry, &mut Default::default()).unwrap();
        assert_eq!(saved.enter, Vec3::new(20., 0., 0.));
        assert_eq!(saved.action, Vec3::new(10., 0., 0.));
        assert!(world.objects[&ferry].railed_dock.docking.is_none(), "DockStart must precede action");
        world.set_fixture_object_position(rider, Vec3::new(20., 0., 0.));
        world.railed_dock_approach(ferry, rider, config, &mut effects).unwrap();
        assert!(world.objects[&ferry].railed_dock.docking.is_none(), "DockAction must precede action");
        world.set_fixture_object_position(rider, Vec3::new(10., 0., 0.));
        world.railed_dock_approach(ferry, rider, config, &mut effects).unwrap();
        assert_eq!(world.objects[&ferry].railed_dock.docking.unwrap().id, rider);
        world.railed_dock_update(ferry, config, &mut effects).unwrap();
        world.railed_dock_update(ferry, config, &mut effects).unwrap();
        let pos = world.objects[&rider].get_position();
        assert!((pos - Vec3::new(6., 0., 4.)).length() < 0.001, "{pos:?}");
        assert_eq!(world.objects[&rider].contained_by, Some(ferry));
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
 Behavior = AIUpdateInterface Ai
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
        assert_eq!(world.objects[&first].last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI);
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
