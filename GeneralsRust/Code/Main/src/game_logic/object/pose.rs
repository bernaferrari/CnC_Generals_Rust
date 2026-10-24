use super::*;

impl Object {
    pub(crate) fn attach_host_trigger_world(
        &mut self,
        world: &std::sync::Arc<std::sync::Mutex<gamelogic::scripting::HostTriggerWorld>>,
    ) {
        self.host_trigger_world.bind(world);
    }

    pub(crate) fn validate_partition_namespace(&self, namespace: &std::sync::Arc<()>) {
        assert!(
            !self.partition_namespace.is_detached_clone(),
            "Object observation clone requires actual admission before partition attachment"
        );
        assert!(
            self.partition_namespace.upgrade().is_none()
                || self
                    .partition_namespace
                    .ptr_eq(&std::sync::Arc::downgrade(namespace)),
            "Object partition attachment belongs to a different world"
        );
    }

    /// Lifecycle restore changes continuation values on the same installed
    /// lifetime. Transfer its leases only after the candidate has been checked.
    pub(crate) fn take_restored_runtime_attachments_from(
        &mut self,
        original: &mut Self,
    ) -> Result<(), &'static str> {
        if self.id != original.id
            || self.visual_object_generation != original.visual_object_generation
            || !std::sync::Arc::ptr_eq(
                &self.visual_draw_identity_token,
                &original.visual_draw_identity_token,
            )
        {
            return Err("lifecycle attachment transfer changed Object lifetime");
        }
        self.partition_namespace = std::mem::take(&mut original.partition_namespace);
        self.host_trigger_world = std::mem::take(&mut original.host_trigger_world);
        Ok(())
    }

    pub(crate) fn bind_partition_namespace(&mut self, namespace: &std::sync::Arc<()>) {
        self.validate_partition_namespace(namespace);
        self.partition_namespace.bind(namespace);
    }

    pub(in crate::game_logic) fn require_partition_owner(
        &self,
        partition: Option<&crate::game_logic::partition_manager::PartitionManager>,
    ) {
        assert!(
            !self.partition_namespace.is_detached_clone() || partition.is_none(),
            "detached Object observation clone has no partition mutation capability"
        );
        if let Some(owner) = self.partition_namespace.upgrade() {
            assert!(
                partition
                    .is_some_and(|partition| std::sync::Arc::ptr_eq(&owner, partition.namespace())),
                "registered Object transform requires its explicit receiving partition owner"
            );
        }
    }

    pub fn get_position(&self) -> Vec3 {
        self.thing.get_position()
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.set_position_with_effects(position, 0, &mut Default::default());
    }

    pub(crate) fn set_position_with_effects(
        &mut self,
        position: Vec3,
        frame: u32,
        effects: &mut crate::game_logic::script_presentation::ScriptPresentationEffects<'_>,
    ) {
        self.set_position_with_partition_context(position, frame, effects, None);
    }

    pub(crate) fn set_position_with_partition_owner(
        &mut self,
        position: Vec3,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        partition: &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        self.set_position_with_effects(
            position,
            frame,
            &mut effects.reborrow_for_object(partition),
        );
    }

    fn set_position_with_partition_context(
        &mut self,
        position: Vec3,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        partition: Option<&mut crate::game_logic::partition_manager::PartitionManager>,
    ) {
        self.require_partition_owner(partition.as_deref().or(effects.partition.as_deref()));
        let old_position = self.get_position();
        let old_angle = self.get_orientation();
        self.thing.set_position(position);
        self.notify_native_pose_installed();
        if let Some(partition) = partition {
            partition.receiving_transform_changed(self, old_position, old_angle);
        } else if let Some(partition) = effects.partition.as_deref_mut() {
            partition.receiving_transform_changed(self, old_position, old_angle);
        }
        self.react_to_position_change(old_position, frame, effects);
    }

    /// C++ Thing::setTransformMatrix reaches Object::reactToTransformChange.
    /// Physics may retain pitch/roll while changing translation.
    pub fn set_transform_matrix(&mut self, transform: Mat4) {
        self.set_transform_matrix_with_effects(transform, 0, &mut Default::default());
    }

    pub(crate) fn set_transform_matrix_with_effects(
        &mut self,
        transform: Mat4,
        frame: u32,
        effects: &mut crate::game_logic::script_presentation::ScriptPresentationEffects<'_>,
    ) {
        self.set_transform_matrix_with_partition_context(transform, frame, effects, None);
    }

    pub(crate) fn set_transform_matrix_with_partition_owner(
        &mut self,
        transform: Mat4,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        partition: &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        self.set_transform_matrix_with_effects(
            transform,
            frame,
            &mut effects.reborrow_for_object(partition),
        );
    }

    fn set_transform_matrix_with_partition_context(
        &mut self,
        transform: Mat4,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        partition: Option<&mut crate::game_logic::partition_manager::PartitionManager>,
    ) {
        self.require_partition_owner(partition.as_deref().or(effects.partition.as_deref()));
        let old_position = self.get_position();
        let old_angle = self.get_orientation();
        self.thing.set_transform_matrix(transform);
        self.notify_native_pose_installed();
        if let Some(partition) = partition {
            partition.receiving_transform_changed(self, old_position, old_angle);
        } else if let Some(partition) = effects.partition.as_deref_mut() {
            partition.receiving_transform_changed(self, old_position, old_angle);
        }
        self.react_to_position_change(old_position, frame, effects);
    }

    fn react_to_position_change(
        &mut self,
        old_position: Vec3,
        frame: u32,
        effects: &mut crate::game_logic::script_presentation::ScriptPresentationEffects<'_>,
    ) {
        let position = self.get_position();
        // CPP Object.cpp:2563–2587 uses integer horizontal coordinates.
        if old_position.x as i32 != position.x as i32 || old_position.z as i32 != position.z as i32
        {
            // Object.cpp2564-2569 returns before the terrain/trigger phase.
            let skip = self.is_kind_of(KindOf::Projectile) || self.is_kind_of(KindOf::Inert);
            if skip {
                return;
            }
            self.notify_terrain_trees_on_unit_move_with_effects(frame, effects);
            let team = if self.team_instance_name.is_empty() {
                None
            } else {
                Some(self.team_instance_name.as_str())
            };
            if let Some(world) = self.host_trigger_world.upgrade() {
                let mut world = world.lock().unwrap_or_else(|e| e.into_inner());
                let frame = world.current_frame();
                world.update_object_flags(self.id.0, position.x, position.z, frame, skip, team);
            }
        }
    }

    /// Immutable authored and derived Thing facts; cannot mutate this Object's pose.
    pub fn thing(&self) -> &Thing {
        &self.thing
    }

    /// Existing authored rule edits are separate from the owned pose domain.
    pub(crate) fn template_mut(&mut self) -> &mut ThingTemplate {
        &mut self.thing.template
    }

    /// Live shape follows Thing lifetime; the authored template is only its
    /// construction seed. Copying geometry to a rebuild hole copies this value.
    pub(crate) fn geometry_info(&self) -> &HostGeometryInfo {
        self.thing
            .geometry
            .shape
            .as_ref()
            .expect("live Thing geometry has admitted shape")
    }

    pub(crate) fn set_geometry_shape(&mut self, shape: HostGeometryInfo) {
        self.set_geometry_info(shape.to_host_geometry());
    }

    pub fn set_geometry_radius(&mut self, radius: f32) {
        self.thing.geometry.radius = radius;
    }

    pub fn set_geometry_bounds_min(&mut self, bounds: Vec3) {
        self.thing.geometry.bounds_min = bounds;
    }

    pub fn set_geometry_bounds_max(&mut self, bounds: Vec3) {
        self.thing.geometry.bounds_max = bounds;
    }

    /// C++ setGeometryInfo changes shape, never the object's pose.
    pub(crate) fn set_geometry_info_with_partition_owner(
        &mut self,
        geometry: GeometryInfo,
        partition: &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        let attached =
            partition.receiving_object_is_attached(self.id.0, &self.visual_draw_identity_token);
        self.require_partition_owner(Some(partition));
        if attached {
            partition.unregister_object(self.id.0);
            self.partition_namespace = Default::default();
        }
        self.set_geometry_info(geometry);
        if attached {
            partition.attach_receiving_object(self);
        }
        // Original Drawable dispatch follows membership replacement. All shipped
        // concrete reactToGeometryChange overrides are empty inline functions.
    }

    pub(crate) fn set_geometry_info_z(&mut self, height: f32) {
        let mut shape = *self.geometry_info();
        match shape.geom_type {
            HostGeometryType::Sphere => shape.major_radius = height,
            HostGeometryType::Box | HostGeometryType::Cylinder => shape.height = height,
        }
        let mut geometry = shape.to_host_geometry();
        geometry.position = self.get_position();
        geometry.rotation = self.get_orientation();
        self.thing.geometry = geometry;
        // The shipped concrete Drawable::reactToGeometryChange overrides are empty.
    }

    pub fn set_geometry_info(&mut self, mut geometry: GeometryInfo) {
        self.require_partition_owner(None);
        geometry.position = self.get_position();
        geometry.rotation = self.get_orientation();
        self.thing.geometry = geometry;
    }

    /// C++ Object.cpp2564-2587 -> W3DGameClient.cpp202 -> TreeBuffer1125.
    /// The immediate borrowed receiver is called before host trigger updates.
    pub(crate) fn notify_terrain_trees_on_unit_move_with_effects(
        &self,
        frame: u32,
        effects: &mut crate::game_logic::script_presentation::ScriptPresentationEffects<'_>,
    ) {
        if self.is_kind_of(KindOf::Projectile)
            || self.is_kind_of(KindOf::Inert)
            || self.is_kind_of(KindOf::Immobile)
        {
            return;
        }
        if !self.is_kind_of(KindOf::Infantry) && !self.is_kind_of(KindOf::Vehicle) {
            return;
        }
        if let Some(terrain) = effects.terrain.as_deref_mut() {
            let pos = self.get_position();
            let angle = self.get_orientation();
            // Existing host geometry approximation is preserved in this owner migration.
            let radius = self.selection_radius.max(1.0);
            terrain.unit_moved(
                crate::game_logic::script_presentation::NativeTerrainUnitMove {
                    object_id: self.id.0,
                    position: Vec3::new(pos.x, pos.z, pos.y),
                    direction_2d: glam::Vec2::new(angle.cos(), -angle.sin()),
                    major_radius: radius,
                    minor_radius: radius,
                    is_box: false,
                    crusher_level: i32::from(self.crusher_level),
                },
                frame,
            );
        }
    }

    pub fn get_orientation(&self) -> f32 {
        self.thing.get_orientation()
    }

    pub fn set_orientation(&mut self, angle: f32) {
        self.require_partition_owner(None);
        self.thing.set_orientation(angle);
        self.notify_native_pose_installed();
    }

    pub(crate) fn set_orientation_with_partition_owner(
        &mut self,
        angle: f32,
        partition: &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        let old_position = self.get_position();
        let old_angle = self.get_orientation();
        self.require_partition_owner(Some(partition));
        self.thing.set_orientation(angle);
        self.notify_native_pose_installed();
        partition.receiving_transform_changed(self, old_position, old_angle);
    }

    pub(crate) fn set_orientation_with_effects(
        &mut self,
        angle: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if let Some(partition) = effects.partition.as_deref_mut() {
            self.set_orientation_with_partition_owner(angle, partition);
        } else {
            self.set_orientation(angle);
        }
    }

    pub(crate) fn native_pose_revision(&self) -> u64 {
        self.native_pose_revision
    }

    pub(super) fn notify_native_pose_installed(&mut self) {
        self.native_pose_revision = self
            .native_pose_revision
            .checked_add(1)
            .expect("Object native pose notification sequence exhausted");
    }

    pub fn get_transform_matrix(&self) -> Mat4 {
        self.thing.get_transform_matrix()
    }

    /// C++ `Object::getHealthBoxPosition` (`Object.cpp:3346-3356`).
    /// Host pose is Y-up: C++ world Z maps to `position.y`.
    pub fn get_health_box_position(&self) -> Vec3 {
        let pos = self.get_position();
        let geom = self.geometry_info();
        let max_h = if geom.authored {
            geom.max_height_above_position()
        } else {
            self.selection_radius.max(1.0)
        };
        let mut y = pos.y + max_h + 10.0 + self.health_box_offset[1];
        if crate::game_logic::host_angry_mob::is_angry_mob_nexus_template(&self.template_name) {
            y += 20.0;
        }
        Vec3::new(
            pos.x + self.health_box_offset[0],
            y,
            pos.z + self.health_box_offset[2],
        )
    }

    /// C++ `Object::getHealthBoxDimensions` (`Object.cpp:3402-3413`).
    /// Returns `(height, width)`. IGNORED_IN_GUI → `(0, 0)`.
    pub fn get_health_box_dimensions(&self) -> (f32, f32) {
        if self.is_kind_of(KindOf::IgnoredInGui) {
            return (0.0, 0.0);
        }
        let geom = self.geometry_info();
        let (major, minor) = if geom.authored {
            (geom.major_radius, geom.minor_radius)
        } else {
            let r = self.selection_radius.max(1.0);
            (r, r)
        };
        let size = (major + minor).min(150.0).max(20.0);
        (3.0, (size * 2.0).max(20.0))
    }

    /// Presentation overlay: C++ Z-up height of the health bar above ground pose.
    pub fn health_box_world_z_offset(&self) -> f32 {
        let geom = self.geometry_info();
        let max_h = if geom.authored {
            geom.max_height_above_position()
        } else {
            self.selection_radius.max(1.0)
        };
        let mut z = max_h + 10.0 + self.health_box_offset[1];
        if crate::game_logic::host_angry_mob::is_angry_mob_nexus_template(&self.template_name) {
            z += 20.0;
        }
        z
    }

    /// C++ ActiveBody visual condition + Drawable::reactToBodyDamageStateChange residual.

    /// C++ ProductionUpdate MODELCONDITION_CONSTRUCTION_COMPLETE residual.

    /// C++ RadarUpdate::extendRadar residual.

    /// C++ ProductionUpdate door residual:
    /// OPENING → WAITING_OPEN → CLOSING → idle.
    /// `theWaitingToCloseFlags` is never set (ProductionUpdate.cpp:513-582).
    ///
    /// Phase durations come from INI DoorOpeningTime / DoorWaitOpenTime /
    /// DoorCloseTime (ProductionUpdate.cpp:113-115, updateDoors:528/548/568).
    /// Completing a unit opens only the reserved ExitDoorType (DOOR_1..4).
    pub fn start_production_door_cycle(&mut self, now: u32) {
        // C++ ProductionUpdate.cpp:746-754: start OPENING only when the reserved
        // door is fully closed. pick_idle must not open a sibling hangar while
        // the reserved door is already OPENING (that would reset spawn gating).
        if self.production_door_spawn_phase() != 0 {
            return;
        }
        let door = self
            .pick_idle_production_door()
            .unwrap_or(self.production_door_active_index as usize);
        self.start_production_door_cycle_at(now, door);
    }

    /// Start OPENING on a specific hangar / factory door (0=DOOR_1).
    ///
    /// C++ ProductionUpdate.cpp:746-773: a fully-closed door starts OPENING
    /// once. Already-OPENING leaves `m_doorOpenedFrame` (DoorOpeningTime)
    /// intact so the door can reach WAITING_OPEN and release the unit.
    pub fn start_production_door_cycle_at(&mut self, now: u32, door: usize) {
        use crate::game_logic::host_production_buildable_command_residual::producer_door_phase_duration;
        let door = door.min(3);
        let current = if self.production_door_phases[door] != 0 {
            self.production_door_phases[door]
        } else if door == 0 {
            self.production_door_phase
        } else {
            0
        };
        if current != 0 {
            return;
        }
        self.clear_production_door_bits(door);
        self.set_production_door_phase_bits(door, 1);
        self.production_door_active_index = door as u8;
        self.production_door_phases[door] = 1;
        self.production_door_phase_end_frames[door] =
            now.saturating_add(producer_door_phase_duration(&self.template_name, 1));
        self.sync_primary_production_door();
        self.refresh_model_condition_bits();
    }

    /// C++ ProductionUpdate.cpp:762-776: a completed unit whose reserved door
    /// is already CLOSING (`m_doorClosedFrame != 0`) pops immediately to
    /// WAITING_OPEN so spawn proceeds this frame.
    pub fn pop_closing_production_door_to_waiting_open(&mut self, now: u32) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::producer_door_phase_duration;
        let door = (self.production_door_active_index as usize).min(3);
        let phase = self.production_door_spawn_phase();
        if phase != 4 && phase != 3 {
            return false;
        }
        self.clear_production_door_bits(door);
        self.set_production_door_phase_bits(door, 2);
        self.production_door_phases[door] = 2;
        self.production_door_phase_end_frames[door] =
            now.saturating_add(producer_door_phase_duration(&self.template_name, 2));
        self.sync_primary_production_door();
        self.refresh_model_condition_bits();

        true
    }

    /// C++ ProductionUpdate::setHoldDoorOpen residual.
    ///
    /// Door 0 wrapper for factory / save callers. Hangar stalls use
    /// `set_production_door_hold_open_at`.
    pub fn set_production_door_hold_open(&mut self, hold: bool, now: u32) {
        self.set_production_door_hold_open_at(0, hold, now);
    }

    /// C++ `ProductionUpdate::setHoldDoorOpen(exitDoor, holdIt)` — one stall.
    /// Starts OPENING on that door only when it is fully closed.
    pub fn set_production_door_hold_open_at(&mut self, door: usize, hold: bool, now: u32) {
        let door = door.min(3);
        self.production_door_hold_opens[door] = hold;
        if door == 0 {
            self.production_door_hold_open = hold;
        }
        if hold {
            let phase = if self.production_door_phases[door] != 0 {
                self.production_door_phases[door]
            } else if door == 0 {
                self.production_door_phase
            } else {
                0
            };
            if phase == 0 {
                self.start_production_door_cycle_at(now, door);
            }
        }
        self.sync_primary_production_door();
    }

    /// C++ DoorInfo::m_holdOpen for this ExitDoorType index.
    pub fn production_door_is_held(&self, door: usize) -> bool {
        let door = door.min(3);
        self.production_door_hold_opens[door] || (door == 0 && self.production_door_hold_open)
    }

    /// Advance production door residual; returns true when every door fully closed.
    /// Wave 627: apply production-door model bits after GW phase writeback.
    ///
    /// Does not advance phase schedules (GameWorld is last-writer for phase/end).
    /// Phase 3 (legacy WAITING_TO_CLOSE writeback) maps to CLOSING — C++ never plays it.
    pub(crate) fn apply_production_door_phase_residual(&mut self, phase: u8) -> bool {
        let door = (self.production_door_active_index as usize).min(3);
        let phase = if phase == 3 { 4 } else { phase };
        self.clear_production_door_bits(door);
        self.set_production_door_phase_bits(door, phase);
        self.production_door_phases[door] = phase;
        if phase == 0 {
            self.production_door_phase_end_frames[door] = 0;
        }
        self.sync_primary_production_door();

        self.refresh_model_condition_bits();
        true
    }

    pub fn tick_production_door(&mut self, now: u32) -> bool {
        self.ensure_primary_door_array_sync();
        let count = self.production_door_count();
        let mut closed_any = false;
        for i in 0..count {
            if self.production_door_phases[i] == 0 {
                continue;
            }
            if now <= self.production_door_phase_end_frames[i] {
                continue;
            }
            if self.tick_one_production_door(i, now) {
                closed_any = true;
            }
        }
        self.sync_primary_production_door();
        closed_any && self.production_door_phases.iter().all(|&p| p == 0)
    }

    /// Door count from INI `NumDoorAnimations`, at least 1 so existing
    /// single-door producers keep the DOOR_1 cycle.
    pub fn production_door_count(&self) -> usize {
        use crate::game_logic::host_production_buildable_command_residual::producer_num_door_animations;
        let n = producer_num_door_animations(&self.template_name);
        if n <= 0 { 1 } else { (n as usize).min(4) }
    }

    /// First idle hangar among authored doors.
    pub fn pick_idle_production_door(&self) -> Option<usize> {
        let count = self.production_door_count();
        (0..count).find(|&i| self.production_door_phases[i] == 0)
    }

    /// Phase used for spawn gating: the reserved/active door, not a shared bank.
    pub fn production_door_spawn_phase(&self) -> u8 {
        let i = (self.production_door_active_index as usize).min(3);
        let p = self.production_door_phases[i];
        if p != 0 {
            p
        } else {
            self.production_door_phase
        }
    }

    fn ensure_primary_door_array_sync(&mut self) {
        if self.production_door_phases[0] == 0 && self.production_door_phase != 0 {
            self.production_door_phases[0] = self.production_door_phase;
            self.production_door_phase_end_frames[0] = self.production_door_phase_end_frame;
        }
        if !self.production_door_hold_opens[0] && self.production_door_hold_open {
            self.production_door_hold_opens[0] = true;
        }
    }

    fn sync_primary_production_door(&mut self) {
        self.production_door_phase = self.production_door_phases[0];
        self.production_door_phase_end_frame = self.production_door_phase_end_frames[0];
        self.production_door_hold_open = self.production_door_hold_opens[0];
    }

    fn tick_one_production_door(&mut self, door: usize, now: u32) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::producer_door_phase_duration;
        let name = self.template_name.clone();
        match self.production_door_phases[door] {
            1 => {
                self.clear_production_door_bits(door);
                self.set_production_door_phase_bits(door, 2);
                self.production_door_phases[door] = 2;
                self.production_door_phase_end_frames[door] =
                    now.saturating_add(producer_door_phase_duration(name.as_str(), 2));

                self.refresh_model_condition_bits();
                false
            }
            2 => {
                if self.production_door_is_held(door) {
                    // C++ retains m_doorWaitOpenFrame while held. Release
                    // must use that original deadline, not restart the wait.
                    return false;
                }
                // C++ updateDoors: WAITING_OPEN → CLOSING. Never WAITING_TO_CLOSE.
                self.clear_production_door_bits(door);
                self.set_production_door_phase_bits(door, 4);
                self.production_door_phases[door] = 4;
                self.production_door_phase_end_frames[door] =
                    now.saturating_add(producer_door_phase_duration(name.as_str(), 4));

                self.refresh_model_condition_bits();
                false
            }
            3 | 4 => {
                if self.production_door_is_held(door) {
                    self.clear_production_door_bits(door);
                    self.set_production_door_phase_bits(door, 2);
                    self.production_door_phases[door] = 2;
                    self.production_door_phase_end_frames[door] =
                        now.saturating_add(producer_door_phase_duration(name.as_str(), 2));

                    self.refresh_model_condition_bits();
                    return false;
                }
                self.clear_production_door_bits(door);
                self.production_door_phases[door] = 0;
                self.production_door_phase_end_frames[door] = 0;

                self.refresh_model_condition_bits();
                true
            }
            _ => {
                self.production_door_phases[door] = 0;
                self.production_door_phase_end_frames[door] = 0;

                false
            }
        }
    }

    fn production_door_bank_bits(door: usize) -> (u32, u32, u32, u32) {
        use crate::game_logic::host_enum_table_residual::{
            door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
            door_1_waiting_to_close_model_bit, door_2_closing_model_bit, door_2_opening_model_bit,
            door_2_waiting_open_model_bit, door_2_waiting_to_close_model_bit,
            door_3_closing_model_bit, door_3_opening_model_bit, door_3_waiting_open_model_bit,
            door_3_waiting_to_close_model_bit, door_4_closing_model_bit, door_4_opening_model_bit,
            door_4_waiting_open_model_bit, door_4_waiting_to_close_model_bit,
        };
        match door {
            1 => (
                door_2_opening_model_bit(),
                door_2_waiting_open_model_bit(),
                door_2_waiting_to_close_model_bit(),
                door_2_closing_model_bit(),
            ),
            2 => (
                door_3_opening_model_bit(),
                door_3_waiting_open_model_bit(),
                door_3_waiting_to_close_model_bit(),
                door_3_closing_model_bit(),
            ),
            3 => (
                door_4_opening_model_bit(),
                door_4_waiting_open_model_bit(),
                door_4_waiting_to_close_model_bit(),
                door_4_closing_model_bit(),
            ),
            _ => (
                door_1_opening_model_bit(),
                door_1_waiting_open_model_bit(),
                door_1_waiting_to_close_model_bit(),
                door_1_closing_model_bit(),
            ),
        }
    }

    fn clear_production_door_bits(&mut self, door: usize) {
        let (open_b, wait_b, wait_close_b, close_b) = Self::production_door_bank_bits(door);
        self.model_condition_bits &= !(1u128 << open_b);
        self.model_condition_bits &= !(1u128 << wait_b);
        self.model_condition_bits &= !(1u128 << wait_close_b);
        self.model_condition_bits &= !(1u128 << close_b);
    }

    fn set_production_door_phase_bits(&mut self, door: usize, phase: u8) {
        let (open_b, wait_b, _wait_close_b, close_b) = Self::production_door_bank_bits(door);
        match phase {
            1 => self.model_condition_bits |= 1u128 << open_b,
            2 => self.model_condition_bits |= 1u128 << wait_b,
            3 | 4 => self.model_condition_bits |= 1u128 << close_b,
            _ => {}
        }
    }

    pub fn extend_radar(&mut self, done_frame: u32) {
        use crate::game_logic::host_enum_table_residual::radar_extending_model_bit;
        let bit = radar_extending_model_bit();
        self.model_condition_bits |= 1u128 << bit;
        // Clear upgraded while extending.
        use crate::game_logic::host_enum_table_residual::radar_upgraded_model_bit;
        self.model_condition_bits &= !(1u128 << radar_upgraded_model_bit());
        self.radar_extend_done_frame = done_frame;
        self.radar_extend_complete = false;
        self.radar_active = true;

        self.refresh_model_condition_bits();
    }

    /// C++ RadarUpdate::update extend completion residual.
    /// Returns true when extension just completed this tick.
    /// Wave 625: apply radar-extend complete residual after GW writeback.
    ///
    /// Does not re-check done_frame (GameWorld is last-writer for complete flag).
    pub(crate) fn apply_radar_extend_complete_residual(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            radar_extending_model_bit, radar_upgraded_model_bit,
        };
        self.radar_extend_complete = true;
        self.radar_extend_done_frame = 0;
        self.model_condition_bits &= !(1u128 << radar_extending_model_bit());
        self.model_condition_bits |= 1u128 << radar_upgraded_model_bit();
        self.refresh_model_condition_bits();
    }

    pub fn tick_radar_extend(&mut self, current_frame: u32) -> bool {
        if self.radar_extend_done_frame == 0 || self.radar_extend_complete {
            return false;
        }
        if current_frame <= self.radar_extend_done_frame {
            return false;
        }
        use crate::game_logic::host_enum_table_residual::{
            radar_extending_model_bit, radar_upgraded_model_bit,
        };
        self.radar_extend_complete = true;
        self.radar_extend_done_frame = 0;
        self.model_condition_bits &= !(1u128 << radar_extending_model_bit());
        self.model_condition_bits |= 1u128 << radar_upgraded_model_bit();
        self.refresh_model_condition_bits();
        true
    }

    /// C++ BuildAssistant/Dozer construction model-condition residual.
    ///
    /// - With active dozer nearby: PARTIALLY_CONSTRUCTED + ACTIVELY_BEING_CONSTRUCTED
    /// - Without dozer (waiting): AWAITING_CONSTRUCTION + PARTIALLY_CONSTRUCTED
    /// - Clears ACTIVELY_BEING when dozer leaves.

    /// C++ MODELCONDITION_ACTIVELY_CONSTRUCTING residual (dozer or factory).

    /// C++ BuildAssistant sell scaffold model residual (start of sell).
    /// C++ TechBuildingBehavior MODELCONDITION_CAPTURED residual.
    pub fn set_captured_model_condition(&mut self, captured: bool) {
        use crate::game_logic::host_enum_table_residual::captured_model_bit;
        let bit = captured_model_bit();
        if bit == 0 {
            return;
        }
        if captured {
            self.model_condition_bits |= 1u128 << bit;
        } else {
            self.model_condition_bits &= !(1u128 << bit);
        }
        self.refresh_model_condition_bits();
    }

    pub fn has_captured_model_condition(&self) -> bool {
        use crate::game_logic::host_enum_table_residual::captured_model_bit;
        let bit = captured_model_bit();
        bit != 0 && (self.model_condition_bits & (1u128 << bit)) != 0
    }

    pub fn apply_sell_scaffold_model_conditions(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, construction_complete_model_bit,
            partially_constructed_model_bit, sold_model_bit,
        };
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        let sold_b = sold_model_bit();
        let complete_b = construction_complete_model_bit();
        self.model_condition_bits &= !(1u128 << sold_b);
        self.model_condition_bits &= !(1u128 << complete_b);
        self.model_condition_bits |= 1u128 << part_b;
        self.model_condition_bits |= 1u128 << active_b;
        self.refresh_model_condition_bits();
    }

    /// C++ BuildAssistant: when construction percent crosses to <= 0 during sell.
    pub fn apply_sold_model_condition(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            partially_constructed_model_bit, sold_model_bit,
        };
        let await_b = awaiting_construction_model_bit();
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        let sold_b = sold_model_bit();
        self.model_condition_bits &= !(1u128 << await_b);
        self.model_condition_bits &= !(1u128 << part_b);
        self.model_condition_bits &= !(1u128 << active_b);
        self.model_condition_bits |= 1u128 << sold_b;
        self.refresh_model_condition_bits();
    }

    /// C++ Object::attemptHealingFromSoleBenefactor residual.
    ///
    /// Non-stacking healers (dozer repair, ambulance, propaganda) claim exclusive
    /// heal rights for `duration` frames. Returns false if another benefactor still
    /// owns the claim.
    pub fn attempt_healing_from_sole_benefactor(
        &mut self,
        amount: f32,
        source_id: ObjectId,
        duration_frames: u32,
        now: u32,
        health_events: &mut crate::game_logic::HostHealthEvents,
    ) -> bool {
        if amount <= 0.0 {
            return false;
        }
        if !self.claim_healing_benefactor(source_id, duration_frames, now) {
            return false;
        }
        let before = self.health.current;
        self.heal(amount, health_events, now);
        self.health.current > before + 0.0001 || self.health.current >= self.health.maximum - 0.01
    }

    /// Claim admission is independent of clipped HP gain (Object.cpp1883).
    pub(in crate::game_logic) fn claim_healing_benefactor(
        &mut self,
        source_id: ObjectId,
        duration_frames: u32,
        now: u32,
    ) -> bool {
        let claim_open = now > self.sole_healing_benefactor_expiration_frame
            || self.sole_healing_benefactor == Some(source_id);

        if !claim_open {
            return false;
        }
        self.sole_healing_benefactor = Some(source_id);
        self.sole_healing_benefactor_expiration_frame = now.wrapping_add(duration_frames);
        true
    }

    pub fn set_actively_constructing(&mut self, active: bool) {
        use crate::game_logic::host_enum_table_residual::actively_constructing_model_bit;
        let bit = actively_constructing_model_bit();
        if active {
            self.model_condition_bits |= 1u128 << bit;
        } else {
            self.model_condition_bits &= !(1u128 << bit);
        }
        self.refresh_model_condition_bits();
    }

    pub fn set_under_construction_model_conditions(&mut self, actively_built: bool) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            construction_complete_model_bit, partially_constructed_model_bit,
        };
        let await_b = awaiting_construction_model_bit();
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        let complete_b = construction_complete_model_bit();
        // Clear all construction-related bits first.
        self.model_condition_bits &= !(1u128 << await_b);
        self.model_condition_bits &= !(1u128 << part_b);
        self.model_condition_bits &= !(1u128 << active_b);
        self.model_condition_bits &= !(1u128 << complete_b);
        self.model_condition_bits |= 1u128 << part_b;
        if actively_built {
            self.model_condition_bits |= 1u128 << active_b;
        } else {
            self.model_condition_bits |= 1u128 << await_b;
        }
        self.refresh_model_condition_bits();
    }

    /// C++ clear construction model conditions on finish (before CONSTRUCTION_COMPLETE).
    pub fn clear_under_construction_model_conditions(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            partially_constructed_model_bit,
        };
        let await_b = awaiting_construction_model_bit();
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        self.model_condition_bits &= !(1u128 << await_b);
        self.model_condition_bits &= !(1u128 << part_b);
        self.model_condition_bits &= !(1u128 << active_b);
        self.refresh_model_condition_bits();
    }

    /// Legacy 45f residual name. Live deadline uses INI ConstructionCompleteDuration
    /// (C++ module default 0 → one-frame flash).
    pub const CONSTRUCTION_COMPLETE_DURATION_FRAMES_RESIDUAL: u32 = 45;

    fn construction_complete_duration_frames(&self) -> u32 {
        use crate::game_logic::host_production_buildable_command_residual::producer_construction_complete_duration_frames;
        let authored = producer_construction_complete_duration_frames(&self.template_name);
        // Duration 0 still flashes one frame (C++ now - start > 0 on the next update).
        authored.max(1)
    }

    pub fn set_construction_complete_condition(&mut self) {
        self.set_construction_complete_condition_at(0);
    }

    /// Set CONSTRUCTION_COMPLETE and schedule clear after INI duration.
    /// `now==0` keeps bit sticky (legacy structure-complete path until tick).
    pub fn set_construction_complete_condition_at(&mut self, now: u32) {
        use crate::game_logic::host_enum_table_residual::construction_complete_model_bit;
        let bit = construction_complete_model_bit();
        self.model_condition_bits |= 1u128 << bit;
        if now > 0 {
            self.construction_complete_clear_frame =
                now.saturating_add(self.construction_complete_duration_frames());
        } else {
            // Structure build-complete path: clear after residual duration from next tick
            // when caller supplies frame via arm helper.
            self.construction_complete_clear_frame = 0;
        }
        self.refresh_model_condition_bits();
    }

    /// Arm clear deadline for CONSTRUCTION_COMPLETE (C++ m_constructionCompleteFrame).
    pub fn arm_construction_complete_clear(&mut self, now: u32) {
        if now == 0 {
            return;
        }
        self.construction_complete_clear_frame =
            now.saturating_add(self.construction_complete_duration_frames());
    }

    /// C++ ProductionUpdate clear CONSTRUCTION_COMPLETE after duration.
    /// Returns true when the bit was cleared this tick.
    /// Wave 626: clear CONSTRUCTION_COMPLETE model bit after GW deadline writeback.
    ///
    /// Returns true when the bit was cleared (or deadline consumed).
    pub(crate) fn apply_construction_complete_clear_residual(&mut self) -> bool {
        use crate::game_logic::host_enum_table_residual::construction_complete_model_bit;
        let bit = construction_complete_model_bit();
        let had_bit = (self.model_condition_bits & (1u128 << bit)) != 0;
        let had_deadline = self.construction_complete_clear_frame > 0;
        if !had_bit && !had_deadline {
            return false;
        }
        self.model_condition_bits &= !(1u128 << bit);
        self.construction_complete_clear_frame = 0;

        self.refresh_model_condition_bits();
        true
    }

    pub fn tick_construction_complete_clear(&mut self, now: u32) -> bool {
        if self.construction_complete_clear_frame == 0 {
            return false;
        }
        if now < self.construction_complete_clear_frame {
            return false;
        }
        use crate::game_logic::host_enum_table_residual::construction_complete_model_bit;
        let bit = construction_complete_model_bit();
        self.model_condition_bits &= !(1u128 << bit);
        self.construction_complete_clear_frame = 0;

        self.refresh_model_condition_bits();
        true
    }

    fn queue_transition_damage_fx_event(
        &mut self,
        ev: crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxEvent,
    ) {
        self.queue_transition_damage_fx_event_with_source(ev, None);
    }

    pub(super) fn queue_transition_damage_fx_event_with_source(
        &mut self,
        mut ev: crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxEvent,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
    ) {
        crate::game_logic::host_transition_damage_fx::take_played_transition_event_fx_ocl(
            &mut ev,
            self.id.0,
            self.get_position(),
            self.get_orientation(),
            &self
                .thing
                .template
                .model_name_for_conditions(self.model_condition_bits),
            self.thing.template.asset_scale,
            source,
        );
        self.pending_transition_damage_fx.push(ev);
    }

    /// Native playback borrows the receiving session's view at the original
    /// callback point, before fear RNG, killed rereads and score/onDie.
    #[cfg(feature = "game_client")]
    pub(in crate::game_logic) fn queue_transition_damage_fx_event_with_view(
        &mut self,
        mut event: crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxEvent,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        view: &mut game_client::fx_list::FxViewServices<'_>,
    ) {
        crate::game_logic::host_transition_damage_fx::take_played_transition_event_fx_ocl_with_view(
            view,
            &mut event,
            self.id.0,
            self.get_position(),
            self.get_orientation(),
            &self
                .thing
                .template
                .model_name_for_conditions(self.model_condition_bits),
            self.thing.template.asset_scale,
            source,
        );
        self.pending_transition_damage_fx.push(event);
    }

    /// Adapter for isolated Object calls without a driving session. Native
    /// callers pass an explicit callback to the matching `_with_fx` operation.
    pub(super) fn standalone_object_fx(name: &str, object: &Object, at_object: bool) {
        if at_object {
            crate::game_logic::publish_host_fx_object(
                object.id.0,
                object.get_position(),
                object.get_orientation(),
                object.owner_player_id.map(|p| p as i32).unwrap_or(-1),
            );
            let _ = crate::game_logic::dispatch_fx_list_at_object(name, object.id.0, None);
        } else {
            let _ = crate::game_logic::dispatch_fx_list_at_pos(name, object.get_position());
        }
    }

    /// Model flags observe canonical Body state; pose and AI setters cannot
    /// reclassify health or invoke damage modules and their clocks.
    pub fn refresh_model_condition_bits(&mut self) {
        self.rebuild_model_condition_bits();
    }

    /// Rebuild presentation flags from already canonical Body and status state.
    /// This operation cannot change HP, reclassify Body state, or retime damage FX.
    pub(super) fn rebuild_model_condition_bits(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            HostBodyDamageType, MC_BIT_ATTACKING, MC_BIT_DISGUISED, MC_BIT_DYING, MC_BIT_MOVING,
            host_apply_body_damage_model_bits,
        };
        let visual_state = if !self.status.under_construction || self.status.destroyed {
            self.body_damage_state
        } else {
            HostBodyDamageType::Pristine
        };
        let mut bits = host_apply_body_damage_model_bits(self.model_condition_bits, visual_state);
        // CPP MinefieldBehavior sets RUBBLE art for an empty regenerating pad,
        // even while its canonical Body retains positive MIN_HEALTH.
        if self
            .mine_data
            .as_ref()
            .is_some_and(|mine| mine.regenerates && mine.virtual_mines_remaining == 0)
        {
            bits |= 1u128 << crate::game_logic::host_enum_table_residual::rubble_model_bit();
        }
        // Motion / combat residual bits from ObjectStatus.
        if self.status.moving {
            bits |= 1u128 << MC_BIT_MOVING;
        } else {
            bits &= !(1u128 << MC_BIT_MOVING);
        }
        if self.status.attacking {
            bits |= 1u128 << MC_BIT_ATTACKING;
        } else {
            bits &= !(1u128 << MC_BIT_ATTACKING);
        }
        if self.status.destroyed {
            bits |= 1u128 << MC_BIT_DYING;
        } else {
            bits &= !(1u128 << MC_BIT_DYING);
        }
        if self.status.disguised {
            bits |= 1u128 << MC_BIT_DISGUISED;
        } else {
            bits &= !(1u128 << MC_BIT_DISGUISED);
        }
        // C++ Physics stun model conditions residual.
        use crate::game_logic::host_enum_table_residual::{
            MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
        };
        bits &= !(1u128 << MC_BIT_STUNNED_FLAILING);
        bits &= !(1u128 << MC_BIT_STUNNED);
        if self.shock_stun_frames > 15 {
            bits |= 1u128 << MC_BIT_STUNNED_FLAILING;
        } else if self.shock_stun_frames > 0 {
            bits |= 1u128 << MC_BIT_STUNNED;
        }
        // FREEFALL residual: airborne while stunned (C++ IS_IN_FREEFALL path).
        use crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL;
        bits &= !(1u128 << MC_BIT_FREEFALL);
        let airborne = self.get_position().y > 0.05 || self.movement.velocity.y > 1.0;
        if airborne && self.shock_stun_frames > 0 && self.shock_was_airborne {
            bits |= 1u128 << MC_BIT_FREEFALL;
        }
        // After first ground contact, prefer STUNNED over FLAILING even if frames high.
        if self.shock_grounded_once && self.shock_stun_frames > 0 {
            use crate::game_logic::host_enum_table_residual::{
                MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
            };
            bits &= !(1u128 << MC_BIT_STUNNED_FLAILING);
            bits |= 1u128 << MC_BIT_STUNNED;
        }
        // Radar extend residual sticks across body/motion refresh.
        use crate::game_logic::host_enum_table_residual::{
            radar_extending_model_bit, radar_upgraded_model_bit,
        };
        let had_radar_ext =
            (self.model_condition_bits & (1u128 << radar_extending_model_bit())) != 0;
        let had_radar_upg =
            (self.model_condition_bits & (1u128 << radar_upgraded_model_bit())) != 0;
        for door in 0..4 {
            let (open_b, wait_b, wait_close_b, close_b) = Self::production_door_bank_bits(door);
            let had_open = (self.model_condition_bits & (1u128 << open_b)) != 0;
            let had_wait = (self.model_condition_bits & (1u128 << wait_b)) != 0;
            let had_close = (self.model_condition_bits & (1u128 << close_b)) != 0;
            // C++ ProductionUpdate never sets WAITING_TO_CLOSE; do not restore it.
            let _ = wait_close_b;
            if had_open {
                bits |= 1u128 << open_b;
            }
            if had_wait {
                bits |= 1u128 << wait_b;
            }
            if had_close {
                bits |= 1u128 << close_b;
            }
        }
        // Construction residual bits stick across body/motion refresh.
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            construction_complete_model_bit, partially_constructed_model_bit,
        };
        let had_cc =
            (self.model_condition_bits & (1u128 << construction_complete_model_bit())) != 0;
        let had_await =
            (self.model_condition_bits & (1u128 << awaiting_construction_model_bit())) != 0;
        let had_partial =
            (self.model_condition_bits & (1u128 << partially_constructed_model_bit())) != 0;
        let had_active =
            (self.model_condition_bits & (1u128 << actively_being_constructed_model_bit())) != 0;
        if had_cc {
            bits |= 1u128 << construction_complete_model_bit();
        }
        if had_await {
            bits |= 1u128 << awaiting_construction_model_bit();
        }
        if had_partial {
            bits |= 1u128 << partially_constructed_model_bit();
        }
        if had_active {
            bits |= 1u128 << actively_being_constructed_model_bit();
        }
        use crate::game_logic::host_enum_table_residual::actively_constructing_model_bit;
        let had_ac =
            (self.model_condition_bits & (1u128 << actively_constructing_model_bit())) != 0;
        if had_ac {
            bits |= 1u128 << actively_constructing_model_bit();
        }
        use crate::game_logic::host_enum_table_residual::sold_model_bit;
        let had_sold_mc = (self.model_condition_bits & (1u128 << sold_model_bit())) != 0;
        if had_sold_mc {
            bits |= 1u128 << sold_model_bit();
        }
        // Wave 487: preserve combat/presentation bits refresh rebuild does not recompute.
        use crate::game_logic::host_enum_table_residual::{
            MC_BIT_BETWEEN_FIRING_SHOTS_A, MC_BIT_BETWEEN_FIRING_SHOTS_B,
            MC_BIT_BETWEEN_FIRING_SHOTS_C, MC_BIT_FIRING_A, MC_BIT_FIRING_B, MC_BIT_FIRING_C,
            MC_BIT_PREATTACK_A, MC_BIT_PREATTACK_B, MC_BIT_PREATTACK_C, MC_BIT_RELOADING_A,
            MC_BIT_RELOADING_B, MC_BIT_RELOADING_C,
        };
        const WEAPON_MC_PRESERVE: [u32; 12] = [
            MC_BIT_PREATTACK_A,
            MC_BIT_FIRING_A,
            MC_BIT_BETWEEN_FIRING_SHOTS_A,
            MC_BIT_RELOADING_A,
            MC_BIT_PREATTACK_B,
            MC_BIT_FIRING_B,
            MC_BIT_BETWEEN_FIRING_SHOTS_B,
            MC_BIT_RELOADING_B,
            MC_BIT_PREATTACK_C,
            MC_BIT_FIRING_C,
            MC_BIT_BETWEEN_FIRING_SHOTS_C,
            MC_BIT_RELOADING_C,
        ];
        for b in WEAPON_MC_PRESERVE {
            if (self.model_condition_bits & (1u128 << b)) != 0 {
                bits |= 1u128 << b;
            }
        }
        if let Some(bit) =
            crate::game_logic::host_enum_table_residual::model_condition_bit_name_index("PRONE")
        {
            if (self.model_condition_bits & (1u128 << bit)) != 0 {
                bits |= 1u128 << bit;
            }
        }
        {
            use crate::game_logic::host_neutron_missile_slow_death::{
                MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
            };
            if (self.model_condition_bits & (1u128 << MC_BIT_FRONTCRUSHED)) != 0 {
                bits |= 1u128 << MC_BIT_FRONTCRUSHED;
            }
            if (self.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED)) != 0 {
                bits |= 1u128 << MC_BIT_BACKCRUSHED;
            }
        }
        {
            use crate::game_logic::host_enum_table_residual::{
                armorset_crateupgrade_one_model_bit, armorset_crateupgrade_two_model_bit,
            };
            let one = armorset_crateupgrade_one_model_bit();
            let two = armorset_crateupgrade_two_model_bit();
            bits &= !(1u128 << one);
            bits &= !(1u128 << two);
            match self.armor_crate_upgrade {
                1 => bits |= 1u128 << one,
                n if n >= 2 => bits |= 1u128 << two,
                _ => {}
            }
        }
        // C++ FlammableUpdate tryToIgnite / update: MODELCONDITION_AFLAME / SMOLDERING.
        {
            use crate::game_logic::host_enum_table_residual::{
                aflame_model_bit, smoldering_model_bit,
            };
            let aflame_b = aflame_model_bit();
            let smolder_b = smoldering_model_bit();
            bits &= !(1u128 << aflame_b);
            bits &= !(1u128 << smolder_b);
            let live_aflame = self.has_object_status_bit("AFLAME")
                || self
                    .fire_spread
                    .as_ref()
                    .map(|f| f.is_aflame())
                    .unwrap_or(false);
            let live_smolder = self
                .fire_spread
                .as_ref()
                .map(|f| f.smoldering)
                .unwrap_or(false)
                || (self.has_object_status_bit("BURNED") && live_aflame)
                || (self.has_object_status_bit("BURNED") && !live_aflame);
            if live_aflame {
                bits |= 1u128 << aflame_b;
            }
            if live_smolder {
                bits |= 1u128 << smolder_b;
            }
        }
        // C++ W3DModelDraw::updateDrawModuleSupplyStatus — CARRYING while boxes > 0.
        {
            use crate::game_logic::host_enum_table_residual::carrying_model_bit;
            let carry_b = carrying_model_bit();
            bits &= !(1u128 << carry_b);
            if self.stored_resources.supplies > 0 {
                bits |= 1u128 << carry_b;
            }
        }
        // C++ MODELCONDITION_PANICKING from chooseLocomotorSet(PANIC) / AIPanicState.
        // C++ MODELCONDITION_OVER_WATER from Locomotor hover flag.
        {
            use crate::game_logic::host_enum_table_residual::over_water_model_bit;
            let bit = over_water_model_bit();
            bits &= !(1u128 << bit);
            if self.over_water {
                bits |= 1u128 << bit;
            }
        }
        {
            use crate::game_logic::host_enum_table_residual::panicking_model_bit;
            let panic_b = panicking_model_bit();
            bits &= !(1u128 << panic_b);
            if self.is_panicking {
                bits |= 1u128 << panic_b;
            }
        }
        if self.model_condition_bits != bits {
            self.visual_draw_state_revision =
                self.visual_draw_state_revision.saturating_add(1).max(1);
        }
        self.model_condition_bits = bits;
        self.sync_overlord_addon_body_damage();
    }
}

#[cfg(test)]
mod native_pose_notification_tests {
    use super::*;

    #[test]
    fn standalone_orientation_effects_fallback_notifies_once() {
        let mut object =
            Object::new_simple(ObjectId(1), ObjectType::Vehicle, "PoseFallback".into());
        let before = object.native_pose_revision();
        object.set_orientation_with_effects(0.7, &mut Default::default());
        assert_eq!(object.get_orientation(), 0.7);
        assert_eq!(object.native_pose_revision(), before + 1);
    }

    #[test]
    fn equal_setters_notify_and_queries_do_not() {
        let mut object = Object::new_simple(ObjectId(1), ObjectType::Vehicle, "PoseFixture".into());
        let initial = object.native_pose_revision();
        let position = object.get_position();
        object.set_position(position);
        assert_eq!(object.native_pose_revision(), initial + 1);
        object.set_orientation(object.get_orientation());
        assert_eq!(object.native_pose_revision(), initial + 2);
        let matrix = object.get_transform_matrix();
        object.set_transform_matrix(matrix);
        assert_eq!(object.native_pose_revision(), initial + 3);
        assert_eq!(object.get_transform_matrix(), matrix);
        assert_eq!(object.native_pose_revision(), initial + 3);
        let clone = object.clone();
        assert_eq!(clone.native_pose_revision(), object.native_pose_revision());
    }
}

#[cfg(test)]
#[path = "pose/native_tree_owner_tests.rs"]
mod native_tree_owner_tests;
