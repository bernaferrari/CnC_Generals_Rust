use super::*;
mod overlap_crush;
pub(in crate::game_logic) use overlap_crush::{
    OverlapCrushDamage, OverlapCrushResume, OverlapCrushStep,
};

impl Object {
    /// C++ applyGravitationalForces residual (host world Y up).
    pub fn apply_gravitational_forces(&mut self) {
        // C++ TheGlobalData->m_gravity (parseAccelerationReal, retail -0.0711).
        self.physics_accel.y += self.physics_gravity();
    }

    /// C++ AIUpdateInterface::privateMoveAwayFromUnit residual.
    ///
    /// Yield callers install a getMoveAwayFromPath result via
    /// `apply_move_away_path`. This setter keeps the 10s window for
    /// repulsor/findSafePath (crates.rs) and the already-yielding cheat.
    pub fn ai_move_away_from_unit(&mut self, threat_id: ObjectId, threat_pos: glam::Vec3) {
        if self.status.destroyed || !self.is_alive() || !self.can_move() {
            return;
        }
        if self.is_kind_of(crate::game_logic::KindOf::Immobile)
            || self.is_kind_of(crate::game_logic::KindOf::Structure)
        {
            return;
        }
        if self.move_away_frames > 0
            && (self.move_away_from == Some(threat_id) || self.move_away_from_2 == Some(threat_id))
        {
            if self.is_blocked {
                self.ignore_collisions_until_frame = self.ignore_collisions_until_frame.max(60);
                self.ignore_collisions_with = Some(threat_id);
            }
            return;
        }
        let us = self.get_position();
        let mut dx = us.x - threat_pos.x;
        let mut dz = us.z - threat_pos.z;
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1.0e-3 {
            let d = self.unit_direction_vector_2d();
            dx = d.x;
            dz = d.y;
        } else {
            dx /= len;
            dz /= len;
        }
        let step = PATHFIND_CELL_SIZE_F_RESIDUAL * 2.0;
        let dest = glam::Vec3::new(us.x + dx * step, us.y, us.z + dz * step);
        self.move_away_from_2 = self.move_away_from;
        self.move_away_from = Some(threat_id);
        self.move_away_destination = Some(dest);
        self.move_away_frames = 10 * 30;
    }

    /// C++ privateMoveAwayFromUnit after getMoveAwayFromPath succeeds.
    pub fn apply_move_away_path(&mut self, threat_id: ObjectId, path: &[glam::Vec3]) -> bool {
        if self.status.destroyed || !self.is_alive() || !self.can_move() {
            return false;
        }
        if self.is_kind_of(crate::game_logic::KindOf::Immobile)
            || self.is_kind_of(crate::game_logic::KindOf::Structure)
        {
            return false;
        }
        if self.move_away_frames > 0
            && (self.move_away_from == Some(threat_id) || self.move_away_from_2 == Some(threat_id))
        {
            if self.is_blocked {
                self.ignore_collisions_until_frame = self.ignore_collisions_until_frame.max(60);
                self.ignore_collisions_with = Some(threat_id);
            }
            return false;
        }
        if path.len() < 2 {
            return false;
        }
        // C++ AIMoveOutOfTheWayState::onEnter forces adjust true only when
        // getPath() is already non-null (AIStates.cpp:2127-2133).
        self.set_adjust_destinations(true);
        self.movement.path = path.to_vec();
        self.movement.current_path_index = 1;
        self.movement.target_position = path.last().copied();
        self.is_attack_path = false;
        self.is_exact_path = false;
        self.refresh_follow_path_extra_distance();
        self.start_move();
        self.set_status_moving(true);
        self.num_frames_blocked = 0;
        self.is_blocked_and_stuck = false;
        self.try_one_more_repath = true;
        self.locomotor_goal_type = super::LocoGoalType::PositionOnPath;
        if self.ultra_accurate {
            self.set_adjust_destinations(false);
        }
        self.move_away_from_2 = self.move_away_from;
        self.move_away_from = Some(threat_id);
        self.move_away_destination = path.last().copied();
        self.move_away_frames = 10 * 30;

        true
    }

    /// Tick move-away temporary state residual.
    pub(crate) fn tick_move_away_state(
        &mut self,
        locomotors: Option<&game_engine::common::ini::ini_locomotor::LocomotorStore>,
    ) {
        if !self.is_alive() && self.move_away_frames > 0 {
            self.move_away_frames = 1;
        }
        if self.move_away_frames > 0 {
            self.move_away_frames -= 1;
            if self.move_away_frames == 0 {
                let still_on_yield = self.move_away_destination.is_some()
                    && (self.movement.target_position == self.move_away_destination
                        || self.movement.path.last().copied() == self.move_away_destination);
                if self.ultra_accurate {
                    if let Some(goal) = self.move_away_destination {
                        if crate::game_logic::pathfinding::PathfindingGrid::is_doing_ground_movement_full(
                            self,
                        ) {
                            let us = self.get_position();
                            let dx = goal.x - us.x;
                            let dz = goal.z - us.z;
                            let cell = PATHFIND_CELL_SIZE_F_RESIDUAL;
                            if dx * dx + dz * dz < cell * cell {
                                self.final_position = goal;
                                self.do_final_position = true;
                            }
                        }
                    }
                }
                self.move_away_from = None;
                self.move_away_from_2 = None;
                self.move_away_destination = None;
                self.can_path_through_units = false;
                if still_on_yield {
                    self.movement.path.clear();
                    self.movement.target_position = None;
                    self.movement.current_path_index = 0;
                    self.waiting_for_path = false;
                    self.is_attack_path = false;
                    self.locomotor_goal_type = super::LocoGoalType::None;
                    self.set_status_moving(false);
                }
                if self.is_panicking {
                    crate::game_logic::host_upgrade_module_residuals::apply_choose_locomotor_set_in(
                        self,
                        crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Normal,
                        false,
                        locomotors,
                    );
                }
            }
        }
    }

    /// Leftover `desired_locomotor_set` / Chinook landed install of SET_TAXIING.
    pub(super) fn apply_leftover_taxiing_locomotor_set(
        &mut self,
        locomotors: Option<&game_engine::common::ini::ini_locomotor::LocomotorStore>,
    ) {
        let chinook_landed = self.chinook_ai.as_ref().is_some_and(|ai| {
            ai.flight_status
                == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
        });
        let jet_ground_taxi = (self.is_kind_of(crate::game_logic::KindOf::Aircraft)
            || self.object_type == ObjectType::Aircraft)
            && !self.jet_allows_air_loco();
        if !chinook_landed && !jet_ground_taxi {
            return;
        }
        if self
            .get_cur_locomotor_set_token()
            .is_some_and(|set| set.eq_ignore_ascii_case("SET_TAXIING"))
        {
            return;
        }
        if chinook_landed {
            let _ = crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_kind_for_surfaces_in(
                self,
                crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Taxiing,
                0,
                locomotors,
            );
        } else {
            self.apply_taxiing_locomotor_set_for_surfaces_in(0, locomotors);
        }
    }

    /// Collision pass reset. Frame counters increment in doLocomotor (update_movement).
    pub fn clear_blocked_frame_state(&mut self) {
        self.is_blocked = false;
        self.cur_max_blocked_speed = 999_999.0;
        self.request_other_move_away = None;
        self.request_self_yield_from = None;
    }
    pub fn set_ignore_collisions_with(&mut self, id: Option<ObjectId>) {
        self.ignore_collisions_with = id;
    }

    /// C++ PhysicsBehavior::isIgnoringCollisionsWith residual.
    pub fn is_ignoring_collisions_with(&self, id: ObjectId) -> bool {
        self.ignore_collisions_with == Some(id)
    }

    /// C++ PhysicsBehavior::isCurrentlyOverlapped residual.
    pub fn is_currently_overlapped(&self, id: ObjectId) -> bool {
        self.physics_current_overlap == Some(id)
    }

    /// C++ PhysicsBehavior::wasPreviouslyOverlapped residual.
    pub fn was_previously_overlapped(&self, id: ObjectId) -> bool {
        self.physics_previous_overlap == Some(id)
    }

    /// C++ PhysicsBehavior::addOverlap residual.
    pub fn add_physics_overlap(&mut self, id: ObjectId) {
        if !self.is_currently_overlapped(id) {
            self.physics_current_overlap = Some(id);
        }
    }

    fn ensure_crush_levels(&mut self) {
        // C++ Object.cpp:1156-1164 reads ThingTemplate CrusherLevel/CrushableLevel.
        // Do not invent KindOf vehicle=1 / infantry=0 — that left cars/props at
        // CrushableLevel 255 and collapsed Overlord==tank. INI is stamped at
        // Object::new from the parsed template.
    }

    /// C++ Object::canCrushOrSquish TEST_CRUSH_ONLY residual.
    pub fn can_crush_only(&self, other: &Object, is_ally: bool) -> bool {
        use crate::game_logic::host_partition_collision_physics_residual::can_crush_only_residual;
        can_crush_only_residual(
            self.crusher_level,
            other.crushable_level,
            is_ally,
            self.status.disabled_unmanned,
        )
    }

    /// C++ Object::canCrushOrSquish default TEST_CRUSH_OR_SQUISH
    /// (Object.cpp:1109-1137): SquishCollide module OR crusherLevel > crushableLevel.
    pub fn can_crush_or_squish(&self, other: &Object, is_ally: bool) -> bool {
        if is_ally || self.status.disabled_unmanned || self.crusher_level == 0 {
            return false;
        }
        if other.has_squish_collide {
            return true;
        }
        self.crusher_level > other.crushable_level
    }

    /// Unit direction 2D residual from orientation (host XZ plane).
    pub fn unit_direction_xz(&self) -> (f32, f32) {
        let yaw = self.get_orientation();
        // Orientation 0 faces +X; desired heading uses (-dz).atan2(dx),
        // so +Z is yaw = -PI/2 → dir (0, +1).
        (yaw.cos(), -yaw.sin())
    }

    /// C++ PhysicsBehavior::checkForOverlapCollision residual.
    ///
    /// Returns true if this is an overlap/crush interaction (skip normal bounce).
    /// On first crush pass of target point, applies HUGE crush damage.
    #[cfg(test)]
    pub fn check_for_overlap_collision(
        &mut self,
        frame: u32,
        other: &mut Object,
        is_ally: bool,
        health_events: &mut crate::game_logic::HostHealthEvents,
    ) -> bool {
        let mut ignored = |_| {};
        let repulsors = false;
        let sources = super::PhysicsDamageSources::default();
        let mut services = super::PhysicsDamageServices {
            sources: &sources,
            frame,
            health_events,
            repulsors: &repulsors,
            player_effects: &mut ignored,
            body_fx: &mut Object::standalone_body_fx,
        };
        self.check_for_overlap_collision_with_services(other, is_ally, &mut services)
    }

    /// Standalone/tool adapter drives the same caller phases, but cannot run
    /// world score/onDie. Native callers drive each yielded request on GameLogic.
    pub(in crate::game_logic) fn check_for_overlap_collision_with_services(
        &mut self,
        other: &mut Object,
        is_ally: bool,
        services: &mut super::PhysicsDamageServices<'_>,
    ) -> bool {
        let mut resume = OverlapCrushResume::Start;
        loop {
            match self.step_overlap_crush(other, is_ally, resume) {
                OverlapCrushStep::Complete(result) => return result,
                OverlapCrushStep::Damage {
                    request,
                    resume: next,
                } => {
                    let killed = services.apply(
                        other,
                        request.amount,
                        request.source,
                        request.damage_type,
                        request.death_type,
                    );
                    if killed {
                        other.finish_standalone_crush_damage(next);
                    }
                    resume = next;
                }
            }
        }
    }

    /// End-of-frame overlap residual: previous = current, clear current.
    pub fn advance_physics_overlap_frame(&mut self) {
        self.physics_previous_overlap = self.physics_current_overlap;
        self.physics_current_overlap = None;
    }

    pub fn scrub_velocity_2d(&mut self, desired_velocity: f32) {
        if desired_velocity < 0.001 {
            self.movement.velocity.x = 0.0;
            self.movement.velocity.z = 0.0;
            self.invalidate_velocity_magnitude();
            return;
        }
        let vx = self.movement.velocity.x;
        let vz = self.movement.velocity.z;
        let cur = (vx * vx + vz * vz).sqrt();
        if desired_velocity > cur || cur < 1e-6 {
            return;
        }
        let s = desired_velocity / cur;
        self.movement.velocity.x = vx * s;
        self.movement.velocity.z = vz * s;
        self.invalidate_velocity_magnitude();
    }

    /// C++ PhysicsBehavior::scrubVelocityZ residual (host Y-up vertical).
    pub fn scrub_velocity_vertical(&mut self, desired_velocity: f32) {
        if desired_velocity.abs() < 0.001 {
            self.movement.velocity.y = 0.0;
        } else {
            let vy = self.movement.velocity.y;
            if (desired_velocity < 0.0 && vy < desired_velocity)
                || (desired_velocity > 0.0 && vy > desired_velocity)
            {
                self.movement.velocity.y = desired_velocity;
            }
        }
        // C++ PhysicsUpdate.cpp:1008 always invalidates, even with no write.
        self.invalidate_velocity_magnitude();
    }

    /// C++ parachute vs building jam residual: push out + scrub lateral.
    pub fn apply_parachute_building_bounce_out(
        &mut self,
        other_center: glam::Vec3,
        us_radius: f32,
    ) {
        self.apply_parachute_building_bounce_out_with_effects(
            other_center,
            us_radius,
            0,
            &mut Default::default(),
        );
    }
    pub(crate) fn apply_parachute_building_bounce_out_with_effects(
        &mut self,
        other_center: glam::Vec3,
        us_radius: f32,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_partition_collision_physics_residual::parachute_bounce_out_distance;
        let us = self.get_position();
        let mut dx = other_center.x - us.x;
        let mut dz = other_center.z - us.z;
        let mut dist = (dx * dx + dz * dz).sqrt();
        if dist < 1.0 {
            dist = 1.0;
            dx = 1.0;
            dz = 0.0;
        }
        let bounce = parachute_bounce_out_distance(us_radius);
        let mut pos = us;
        pos.x -= bounce * dx / dist;
        pos.z -= bounce * dz / dist;
        self.set_position_with_effects(pos, frame, effects);
        self.scrub_velocity_2d(0.0);
    }

    /// C++ immobile collide stiffness bounce residual on velocity.
    ///
    /// Zeros velocity then applies bounce factor along separation (host XZ + Y).
    /// Returns applied force vector residual (for tests).
    pub fn apply_structure_stiffness_bounce(
        &mut self,
        other_center: glam::Vec3,
        mass: f32,
    ) -> glam::Vec3 {
        use crate::game_logic::host_partition_collision_physics_residual::structure_immobile_bounce_factor;
        let g = self.geometry_info();
        let lift = match g.geom_type {
            crate::game_logic::thing::HostGeometryType::Sphere => 0.0,
            crate::game_logic::thing::HostGeometryType::Box
            | crate::game_logic::thing::HostGeometryType::Cylinder => g.height * 0.5,
        };
        let us = self.get_position();
        let us_y = us.y + lift;
        let dx = other_center.x - us.x;
        let mut dy = other_center.y - us_y;
        let dz = other_center.z - us.z;
        // C++ grounded branch sets delta.z = 0 (PhysicsUpdate.cpp:1292).
        if !self.is_above_terrain() {
            dy = 0.0;
        }
        let mut dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist < 1.0 {
            dist = 1.0;
        }
        let mag = self.movement.velocity.length();
        let stiffness = self.physics_structure_stiffness();
        let factor = structure_immobile_bounce_factor(mag, mass, stiffness);
        let dir = glam::Vec3::new(dx / dist, dy / dist, dz / dist);
        let force = dir * factor;
        let mut applied = force;
        if self.is_motive() {
            let facing = self.unit_direction_vector_2d();
            let lateral_dot = applied.x * (-facing.y) + applied.z * facing.x;
            applied.x = lateral_dot * (-facing.y);
            applied.z = lateral_dot * facing.x;
        }
        self.movement.velocity = glam::Vec3::ZERO;
        let mass_inv = if mass.abs() > 1.0e-6 { 1.0 / mass } else { 0.0 };
        self.movement.velocity += applied * mass_inv;
        self.invalidate_velocity_magnitude();

        force
    }

    pub fn evaluate_vehicle_crash_into(
        &self,
        other: &Object,
    ) -> crate::game_logic::host_partition_collision_physics_residual::VehicleCrashImmobileOutcome
    {
        use crate::game_logic::host_partition_collision_physics_residual::{
            PHYSICS_DEFAULT_STRUCTURE_RUBBLE_HEIGHT_RESIDUAL, vehicle_crash_into_immobile_outcome,
        };
        let is_vehicle = self.is_kind_of(KindOf::Vehicle);
        let other_structure = other.is_kind_of(KindOf::Structure);
        // C++ otherImmobile = isKindOf(KINDOF_IMMOBILE); crash is inside that gate.
        let other_immobile = other.is_kind_of(KindOf::Immobile);
        let center_y = |obj: &Object| {
            let g = obj.geometry_info();
            let lift = match g.geom_type {
                crate::game_logic::thing::HostGeometryType::Sphere => 0.0,
                crate::game_logic::thing::HostGeometryType::Box
                | crate::game_logic::thing::HostGeometryType::Cylinder => g.height * 0.5,
            };
            obj.get_position().y + lift
        };
        let above = self.is_above_terrain();
        let falling = above && (center_y(other) - center_y(self)) < 0.0;
        vehicle_crash_into_immobile_outcome(
            is_vehicle,
            other_structure,
            other_immobile,
            falling,
            self.get_position().y,
            PHYSICS_DEFAULT_STRUCTURE_RUBBLE_HEIGHT_RESIDUAL,
        )
    }

    pub fn record_bounce_land(&mut self, prev_y: f32) {
        let dy = (prev_y - self.get_position().y).abs();
        self.last_bounce_fall_dy = dy;
        self.last_bounce_volume = bounce_sound_volume_residual(dy, Self::SHOCK_MASS);
        self.bounce_land_events = self.bounce_land_events.saturating_add(1);
        // C++ doBounceSound returns immediately unless m_bounceSound was authored.
        if !self.bounce_sound_name.is_empty() {
            self.bounce_audio_pending = self.bounce_audio_pending.saturating_add(1);
        }
    }

    /// Drain one pending bounce audio emit for GameLogic → TheAudio queue.
    pub fn take_bounce_audio_pending(&mut self) -> Option<(String, f32)> {
        if self.bounce_audio_pending == 0 || self.bounce_sound_name.is_empty() {
            self.bounce_audio_pending = 0;
            return None;
        }
        self.bounce_audio_pending = self.bounce_audio_pending.saturating_sub(1);
        Some((self.bounce_sound_name.clone(), self.last_bounce_volume))
    }

    /// C++ Get_Z_Vector().Z remapped to host Y-up: world-Y of local up.
    pub fn physics_transform_up_y(&self) -> f32 {
        self.get_transform_matrix().y_axis.y
    }

    pub(super) fn sync_shock_up_from_transform(&mut self) {
        self.shock_up_z = self.physics_transform_up_y().clamp(-1.0, 1.0);
    }

    /// Keep translation; preserve accumulated pitch/roll (C++ setPosition).
    pub(super) fn set_position_keep_rotation(&mut self, pos: glam::Vec3) {
        self.set_position_keep_rotation_with_effects(pos, 0, &mut Default::default());
    }
    pub(super) fn set_position_keep_rotation_with_effects(
        &mut self,
        pos: glam::Vec3,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let mut mtx = self.get_transform_matrix();
        mtx.w_axis = pos.extend(1.0);
        self.set_transform_matrix_with_effects(mtx, frame, effects);
        self.sync_shock_up_from_transform();
    }

    /// C++ PhysicsBehavior::update CenterOfMassOffset * sin(remainingAngle).
    pub(super) fn center_of_mass_pitch_scale(&self) -> f32 {
        if self.center_of_mass_offset == 0.0 {
            return 1.0;
        }
        let fwd = self.get_transform_matrix().x_axis;
        let xz = (fwd.x * fwd.x + fwd.z * fwd.z).sqrt();
        let pitch_angle = fwd.y.atan2(xz);
        let remaining = if self.center_of_mass_offset > 0.0 {
            std::f32::consts::FRAC_PI_2 - pitch_angle
        } else {
            -std::f32::consts::FRAC_PI_2 + pitch_angle
        };
        remaining.sin()
    }

    /// C++ HAS_PITCHROLLYAW incremental Rotate_X/Y/Z remapped to host Y-up.
    /// C++ X=forward, Y=right, Z=up → host X=forward, Z=right, Y=up.
    pub fn apply_physics_ypr(&mut self, yaw_rate: f32, pitch_rate: f32, roll_rate: f32) {
        self.apply_physics_ypr_with_effects(
            yaw_rate,
            pitch_rate,
            roll_rate,
            0,
            &mut Default::default(),
        );
    }

    pub(crate) fn apply_physics_ypr_with_effects(
        &mut self,
        yaw_rate: f32,
        pitch_rate: f32,
        roll_rate: f32,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if yaw_rate.abs() <= 1e-8 && pitch_rate.abs() <= 1e-8 && roll_rate.abs() <= 1e-8 {
            self.sync_shock_up_from_transform();
            return;
        }
        let mut mtx = self.get_transform_matrix();
        mtx *= glam::Mat4::from_rotation_x(roll_rate);
        mtx *= glam::Mat4::from_rotation_z(pitch_rate);
        mtx *= glam::Mat4::from_rotation_y(yaw_rate);
        self.set_transform_matrix_with_effects(mtx, frame, effects);
        self.sync_shock_up_from_transform();
    }

    /// Leftover handle_bounce first-righting (PhysicsUpdate.cpp:505-512):
    /// pitch = 0, roll = 0 or PI from current up so stun-kill still sees a flip.
    pub(super) fn right_physics_pitch_keep_flip(&mut self) {
        self.right_physics_pitch_keep_flip_with_operation(0, &mut Default::default())
    }

    pub(super) fn right_physics_pitch_keep_flip_with_operation(
        &mut self,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let pos = self.get_position();
        let yaw = self.get_orientation();
        let roll = if self.physics_transform_up_y() > 0.0 {
            0.0
        } else {
            std::f32::consts::PI
        };
        self.set_transform_matrix_with_effects(
            glam::Mat4::from_translation(pos)
                * glam::Mat4::from_rotation_y(yaw)
                * glam::Mat4::from_rotation_x(roll),
            operation_frame,
            operation_effects,
        );
        self.sync_shock_up_from_transform();
    }

    /// C++ killWhenRestingOnGround residual.
    ///
    /// When settled on ground with near-zero velocity, kill non-drone (or
    /// unmanned/dead drones). Airborne is C++ isAboveTerrain (height > 0).
    pub fn apply_ypr_damping(&mut self, factor: f32) {
        self.shock_yaw_rate *= factor;
        self.shock_pitch_rate *= factor;
        self.shock_roll_rate *= factor;
    }

    /// C++ setAllowBouncing residual.
    pub fn set_allow_bouncing(&mut self, allow: bool) {
        self.shock_allow_bounce = allow;
    }

    /// C++ handleBounce force residual (does not mutate velocity; returns force).
    ///
    /// Callers apply via `apply_physics_force` when ALLOW_BOUNCE remains set.
    pub fn compute_ground_bounce_force(
        &mut self,
        old_y: f32,
        new_y: f32,
        ground_y: f32,
    ) -> Option<glam::Vec3> {
        self.compute_ground_bounce_force_with_operation(
            old_y,
            new_y,
            ground_y,
            0,
            &mut Default::default(),
        )
    }

    pub fn compute_ground_bounce_force_with_operation(
        &mut self,
        old_y: f32,
        new_y: f32,
        ground_y: f32,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<glam::Vec3> {
        if !self.shock_allow_bounce || new_y > ground_y {
            return None;
        }
        let vy = self.movement.velocity.y;
        let mut desired_accel_y = 0.0;
        if old_y > ground_y && vy < 0.0 {
            let stiffness = self.physics_ground_stiffness();
            desired_accel_y = vy.abs() * stiffness;
        }
        self.apply_ypr_damping(Self::BOUNCE_YPR_DAMPING);
        if vy < 0.0 {
            // Leftover handle_bounce vz<0: pitch=0, roll=0 or PI from up.
            // Rates stay 0.7-damped (C++ applyYPRDamping only).
            self.right_physics_pitch_keep_flip_with_operation(operation_frame, operation_effects);
        }
        if desired_accel_y > 0.0 {
            // C++ bounceForce.z = mass * desiredAccelZ
            let force_y = self.physics_get_mass() * desired_accel_y;
            Some(glam::Vec3::new(0.0, force_y, 0.0))
        } else {
            // Restore original allow bounce residual.
            self.shock_allow_bounce = self.original_allow_bounce;
            None
        }
    }

    /// C++ `AIUpdate.cpp:2276-2279` / leftover `ai_update_interface.rs:1848-1853`.
    /// `AirborneTargetingHeight` defaults to `INT_MAX`, so tossed ground locos
    /// stay `ANTI_GROUND`. Authored INI heights still flag when above threshold.
    pub fn stamp_airborne_target_from_locomotor(&mut self) {
        let height = self.get_position().y - self.ground_height;
        self.status.airborne_target = height > self.airborne_targeting_height as f32;
    }

    /// C++ PhysicsBehavior position integrate + ground clamp residual (one frame).
    /// `ground_y` is terrain height at object XZ. Returns true if a bounce force was applied.
    #[cfg(test)]
    pub fn tick_physics_motion_step(
        &mut self,
        frame: u32,
        ground_y: f32,
        health_events: &mut crate::game_logic::HostHealthEvents,
    ) -> bool {
        let mut ignored = |_| {};
        let repulsors = false;
        let sources = super::PhysicsDamageSources::default();
        let mut services = super::PhysicsDamageServices {
            sources: &sources,
            frame,
            health_events,
            repulsors: &repulsors,
            player_effects: &mut ignored,
            body_fx: &mut Object::standalone_body_fx,
        };
        self.tick_physics_motion_step_with_services(ground_y, &mut services)
    }

    pub(in crate::game_logic) fn tick_physics_motion_step_with_services(
        &mut self,
        ground_y: f32,
        services: &mut super::PhysicsDamageServices<'_>,
    ) -> bool {
        self.integrate_physics_force_phase();
        self.drive_standalone_contact(
            super::MotionContact::Start(ground_y),
            services,
            Object::step_motion_contact,
        )
    }

    /// C++ landing peel that still runs when HELD skips Euler.
    pub fn default_locomotor_surfaces_for_template(template: &ThingTemplate) -> u32 {
        if template.is_kind_of(KindOf::Aircraft) {
            LOCO_SURFACE_AIR | LOCO_SURFACE_GROUND
        } else if template.name.to_ascii_lowercase().contains("hover")
            || template.name.to_ascii_lowercase().contains("amphib")
            || template.name.to_ascii_lowercase().contains("ship")
        {
            LOCO_SURFACE_GROUND | LOCO_SURFACE_WATER
        } else if template.is_kind_of(KindOf::Structure) {
            LOCO_SURFACE_GROUND
        } else {
            LOCO_SURFACE_GROUND
        }
    }

    pub(super) fn ensure_locomotor_surfaces(&mut self) {
        if self.locomotor_surfaces == 0 {
            self.locomotor_surfaces =
                Self::default_locomotor_surfaces_for_template(&self.thing.template);
        }
    }

    pub fn has_locomotor_for_surface(&self, surface: u32) -> bool {
        (self.locomotor_surfaces & surface) != 0
    }

    /// CPP Object.cpp3336 reads the installed AI's actual selected template,
    /// independently of altitude/targeting flags and object KindOf.
    pub(crate) fn selected_locomotor_is_airborne(&self) -> bool {
        self.thing.template.authored_ai_update_interface() == Some(true)
            && self
                .cur_locomotor_name
                .as_deref()
                .is_some_and(|name| !name.is_empty())
            && self
                .selected_locomotor_definition
                .as_ref()
                .is_some_and(|definition| (definition.locomotor_surfaces & LOCO_SURFACE_AIR) != 0)
    }

    /// C++ `Object::getAI() != NULL` residual.
    ///
    /// Leftover thing-factory AIUpdate module when the template is loaded;
    /// else combat/mobile heuristic so infantry/vehicles still gate without INI.
    pub fn has_ai_update_interface(&self) -> bool {
        // Object.cpp419 caches the admitted module interface. Another
        // world's factory must not override a proven template definition.
        if let Some(presence) = self.thing.template.authored_ai_update_interface() {
            return presence;
        }
        // Synthesized test/tool templates have no authored module inventory.
        // Their local fallback must not discover another session's definitions.
        if self.is_kind_of(KindOf::Mine)
            || self.is_kind_of(KindOf::Projectile)
            || self.is_kind_of(KindOf::Crate)
        {
            return false;
        }
        if self.is_kind_of(KindOf::Structure) || self.is_kind_of(KindOf::Immobile) {
            return self.can_attack() || self.weapon.is_some();
        }
        self.is_mobile()
    }

    /// C++ PhysicsBehavior::testStunnedUnitForDestruction residual.
    ///
    /// Called on bounce. Kills when upside-down, off-map, cliff without cliff
    /// locomotor, or underwater without water locomotor. Cliff/water kills
    /// require AIUpdateInterface (C++ PhysicsUpdate.cpp:1777-1779).
    #[cfg(test)]
    pub fn tick_shock_stun(
        &mut self,
        frame: u32,
        health_events: &mut crate::game_logic::HostHealthEvents,
    ) {
        let mut ignored = |_| {};
        let repulsors = false;
        let sources = super::PhysicsDamageSources::default();
        let mut services = super::PhysicsDamageServices {
            sources: &sources,
            frame,
            health_events,
            repulsors: &repulsors,
            player_effects: &mut ignored,
            body_fx: &mut Object::standalone_body_fx,
        };
        self.tick_shock_stun_with_services(&mut services);
    }

    pub(in crate::game_logic) fn tick_shock_stun_with_services(
        &mut self,
        services: &mut super::PhysicsDamageServices<'_>,
    ) {
        self.tick_shock_stun_body(services);
    }

    fn tick_shock_stun_body(&mut self, services: &mut super::PhysicsDamageServices<'_>) {
        self.drive_standalone_contact(
            super::StunContact::Start,
            services,
            Object::step_stun_contact,
        );
    }

    /// C++ STUN_RELIEF_EPSILON / !isSignificantlyAboveTerrain residual.
    pub(super) fn maybe_clear_shock_stun_relief(
        &mut self,
        already_on_ground: bool,
        was_significantly_airborne: bool,
    ) {
        if self.shock_stun_frames == 0 {
            return;
        }
        let v = self.movement.velocity;
        const EPS: f32 = 0.5; // C++ STUN_RELIEF_EPSILON
        let vel_settled = v.x.abs() < EPS && v.y.abs() < EPS && v.z.abs() < EPS;
        // C++ PhysicsUpdate.cpp:671-682 — the landing frame and the grounded
        // sliding tumble keep IS_STUNNED (FLAILING → STUNNED flip observable,
        // C++ ≈1+ frame grounded stun) until |vel| drops under
        // STUN_RELIEF_EPSILON; relief then applies on the settled tick.
        if self.shock_grounded_once
            && !vel_settled
            && (already_on_ground || was_significantly_airborne)
        {
            return;
        }
        // C++: |vel|<0.5 OR !isSignificantlyAboveTerrain (3-frame fall height,
        // Thing.cpp:308-311). A fast tumbling body steps over the 0.64wu band
        // between frames, so the terrain clause also requires a settled
        // vertical speed — otherwise a mid-fall frame inside the band would
        // clear IS_STUNNED before first ground contact (PhysicsUpdate.cpp:671-682).
        if vel_settled || (!self.is_significantly_above_terrain() && v.y.abs() < EPS) {
            self.shock_stun_frames = 0;
            self.set_status_disabled_freefall(false);
        }
    }

    /// C++ PhysicsBehavior::getIsStunned residual.
    pub fn is_shock_stunned(&self) -> bool {
        self.shock_stun_frames > 0
    }

    /// C++ `getIsDownhillOnly` legs/other refuse (Locomotor.cpp:1596-1598).
    /// Host Y is C++ Z.
    pub fn downhill_only_blocks_goal(&self, current_y: f32, goal_y: f32) -> bool {
        self.downhill_only && current_y < goal_y - 0.05
    }

    /// C++ `moveTowardsPositionClimb` FLAG_CLIMBING + backward-on-descent.
    /// `ground_ahead_y` is terrain height 1 unit toward the goal.
    /// Returns whether the unit should walk backwards (downhill climb).
    pub fn update_climber_flags(
        &mut self,
        current: glam::Vec3,
        goal: glam::Vec3,
        ground_ahead_y: f32,
    ) -> bool {
        let dz = current.y - goal.y;
        let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
        if dz * dz > cell * cell {
            self.is_climbing = true;
        }
        if dz.abs() < 1.0 {
            self.is_climbing = false;
        }
        let mut move_backwards = false;
        if self.is_climbing && ground_ahead_y < current.y - 0.1 {
            move_backwards = true;
        }
        self.moving_backwards = move_backwards;
        move_backwards
    }

    /// C++ climb slope divisor: `desiredSpeed /= groundSlope*4` when slope>1.
    pub fn climber_slope_speed_scale(&self, current_y: f32, ground_ahead_y: f32) -> f32 {
        if !self.is_climbing {
            return 1.0;
        }
        let mut ground_slope = (ground_ahead_y - current_y).abs();
        if ground_slope < 1.0 {
            ground_slope = 1.0;
        }
        if ground_slope > 1.0 {
            1.0 / (ground_slope * 4.0)
        } else {
            1.0
        }
    }

    /// C++ appearance-specific approach brake (not dest=0 `dist/dt` snap).
    ///
    /// - Wings: clear IS_BRAKING then floor to minSpeed via Other
    ///   (`Locomotor.cpp:1046-1050`, `:1859-1860`, `:2368-2374`).
    /// - Legs/climber/other/hover: `calcSlowDownDist` → minSpeed, no new IS_BRAKING
    ///   (`Locomotor.cpp:1648-1653`, `:2368-2374`). Only treads/wheels set the pose cheat.
    /// - Treads: `(actual/1.5)*(actual/braking)` + squared `braking_factor`.
    /// - Wheels: +1 frame, donut timer (40 units / 2.5s), then `braking_factor=1`.
    ///
    /// Inputs/outputs use host seconds; the original arithmetic uses frames.
    /// Reconstruct frame inputs in the order below, retaining host scalars on
    /// copy branches. This cannot recover rounding lost in the host binding.
    /// The live caller owns the 2D-motive gate and dispatches Thrust separately.
    pub fn apply_cpp_approach_brake(
        &mut self,
        on_path_dist: f32,
        actual_speed: f32,
        desired_speed: f32,
        logic_frame: u32,
    ) -> f32 {
        const MAX_BRAKING_FACTOR: f32 = 5.0;
        const DONUT_DISTANCE: f32 = 4.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
        const FRAMES: f32 = game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32;
        const DONUT_TIME_FRAMES: f32 = 2.5 * FRAMES;
        let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;

        if matches!(self.loco_appearance, LocomotorAppearance::Wings) {
            // C++ Locomotor.cpp:1046-1050 clears IS_BRAKING for LOCO_WINGS,
            // then moveTowardsPositionWings delegates to Other which still
            // floors to minSpeed (Locomotor.cpp:2368-2374).
            self.is_braking = false;
        }
        let actual_frame_speed = actual_speed / FRAMES;
        let braking = self.braking / FRAMES / FRAMES;
        // Far-from-goal IS_BRAKING clear (Locomotor.cpp:941-946) lives in
        // locoUpdate *before* the 2× path-raise. Do not repeat it here on the
        // already-raised on_path or the latch is killed the same frame.
        let mut goal_speed = desired_speed;

        match self.loco_appearance {
            LocomotorAppearance::Treads => {
                let slow_down_time = actual_frame_speed / braking;
                let slow_down_dist = (actual_frame_speed / 1.5) * slow_down_time;
                if on_path_dist < slow_down_dist
                    && !self.is_braking
                    && !self.no_slow_down_as_approaching_dest
                {
                    self.is_braking = true;
                    self.braking_factor = 1.1;
                }
                // Original 2.0 is a double literal, including at overflow edges.
                if on_path_dist > cell && f64::from(on_path_dist) > 2.0 * f64::from(slow_down_dist)
                {
                    self.is_braking = false;
                }
                if self.is_braking {
                    self.braking_factor = slow_down_dist / on_path_dist;
                    self.braking_factor *= self.braking_factor;
                    if self.braking_factor > MAX_BRAKING_FACTOR {
                        self.braking_factor = MAX_BRAKING_FACTOR;
                    }
                    if slow_down_dist > on_path_dist {
                        let mut frame_goal = actual_frame_speed - braking;
                        if frame_goal < 0.0 {
                            frame_goal = 0.0;
                        }
                        goal_speed = frame_goal * FRAMES;
                    } else if slow_down_dist > on_path_dist * 0.75 {
                        let mut frame_goal = actual_frame_speed - braking / 2.0;
                        if frame_goal < 0.0 {
                            frame_goal = 0.0;
                        }
                        goal_speed = frame_goal * FRAMES;
                    } else {
                        goal_speed = actual_speed;
                    }
                }
            }
            LocomotorAppearance::WheelsFour | LocomotorAppearance::Motorcycle => {
                let slow_down_time = actual_frame_speed / braking + 1.0;
                let slow_down_dist =
                    (actual_frame_speed / 1.5) * slow_down_time + actual_frame_speed;
                let mut effective = slow_down_dist;
                if effective < cell {
                    effective = cell;
                }
                if on_path_dist < effective
                    && !self.is_braking
                    && !self.no_slow_down_as_approaching_dest
                {
                    self.is_braking = true;
                    self.braking_factor = 1.1;
                }
                if on_path_dist > cell && f64::from(on_path_dist) > 2.0 * f64::from(slow_down_dist)
                {
                    self.is_braking = false;
                }
                if self.donut_timer == u32::MAX {
                    // Retained host lifecycle adaptation: C++ initializes at
                    // construction/startMove, not at the first approach call.
                    self.donut_timer = (logic_frame as f32 + DONUT_TIME_FRAMES) as u32;
                }
                if on_path_dist > DONUT_DISTANCE {
                    // C++ adds in float before conversion to UnsignedInt.
                    // Rust saturates outside that conversion's defined C++
                    // range; that fallback is not a portable parity claim.
                    self.donut_timer = (logic_frame as f32 + DONUT_TIME_FRAMES) as u32;
                } else if self.donut_timer < logic_frame {
                    self.is_braking = true;
                }
                if self.is_braking {
                    self.braking_factor = slow_down_dist / on_path_dist;
                    self.braking_factor *= self.braking_factor;
                    if self.braking_factor > MAX_BRAKING_FACTOR {
                        self.braking_factor = MAX_BRAKING_FACTOR;
                    }
                    // C++ Locomotor.cpp:1420 overwrites braking_factor back to 1.0.
                    self.braking_factor = 1.0;
                    if slow_down_dist > on_path_dist {
                        let mut frame_goal = actual_frame_speed - braking;
                        if frame_goal < 0.0 {
                            frame_goal = 0.0;
                        }
                        goal_speed = frame_goal * FRAMES;
                    } else if slow_down_dist > on_path_dist * 0.75 {
                        let mut frame_goal = actual_frame_speed - braking / 2.0;
                        if frame_goal < 0.0 {
                            frame_goal = 0.0;
                        }
                        goal_speed = frame_goal * FRAMES;
                    } else {
                        goal_speed = actual_speed;
                    }
                }
            }
            _ => {
                // Legs / climber / hover / other / wings: desired = minSpeed.
                // C++ never sets IS_BRAKING here (Locomotor.cpp:1648-1653, 2368-2374).
                if !self.no_slow_down_as_approaching_dest {
                    let slow = crate::game_logic::calc_slow_down_dist(
                        actual_frame_speed,
                        self.min_speed / FRAMES,
                        braking,
                    );
                    if on_path_dist < slow {
                        goal_speed = self.min_speed;
                    }
                }
            }
        }
        goal_speed
    }

    /// C++ `locoUpdate_moveTowardsPosition` (Locomotor.cpp:980-992): one raise.
    /// Latch `IS_BRAKING` when Euclidean 2D exceeds 2× remaining path, then raise.
    pub fn raise_on_path_dist_to_goal(&mut self, dist_2d: f32, on_path_dist: f32) -> f32 {
        if dist_2d > on_path_dist {
            let projectile =
                self.is_kind_of(KindOf::Projectile) || self.object_type == ObjectType::Projectile;
            if !projectile && dist_2d > 2.0 * on_path_dist {
                self.is_braking = true;
            }
            dist_2d
        } else {
            on_path_dist
        }
    }

    /// C++ `moveTowardsPositionWheels` (Locomotor.cpp:1283-1286): floor is
    /// damage-condition `maxSpeed/4`, not the already-reduced desiredSpeed.
    pub fn wheeled_turn_speed_floor(&self) -> f32 {
        let mut turn_speed = self.min_turn_speed;
        let max_speed = self.effective_max_speed();
        if turn_speed < max_speed / 4.0 {
            turn_speed = max_speed / 4.0;
        }
        turn_speed
    }

    /// C++ braking pose cheat (`Locomotor.cpp:1092-1138`): snap XY (3D for
    /// projectiles) while OBJECT_STATUS_BRAKING. Host vel is units/sec.
    pub fn braking_cheat_step(
        &self,
        current: glam::Vec3,
        target: glam::Vec3,
        dt: f32,
    ) -> glam::Vec3 {
        let projectile =
            self.is_kind_of(KindOf::Projectile) || self.object_type == ObjectType::Projectile;
        let dx = target.x - current.x;
        let dy = target.y - current.y;
        let dz = target.z - current.z;
        let dist = if projectile {
            (dx * dx + dy * dy + dz * dz).sqrt()
        } else {
            (dx * dx + dz * dz).sqrt()
        };
        if dist <= 0.001 {
            return if projectile {
                target
            } else {
                glam::Vec3::new(target.x, current.y, target.z)
            };
        }
        // C++ getForwardSpeed2D is distance per logic frame. Rust velocity is
        // per second, so one cheat step is speed * dt. MIN_VEL stays one cell
        // per logic frame (PATHFIND_CELL_SIZE / 30).
        let min_vel = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL / 30.0;
        let per_second = if projectile {
            self.movement.velocity.length()
        } else {
            self.forward_speed_2d().abs()
        };
        let mut vel = per_second * dt.max(0.0);
        if vel < min_vel {
            vel = min_vel;
        }
        if vel > dist {
            vel = dist;
        }
        let inv = 1.0 / dist;
        if projectile {
            glam::Vec3::new(
                current.x + dx * inv * vel,
                current.y + dy * inv * vel,
                current.z + dz * inv * vel,
            )
        } else {
            glam::Vec3::new(
                current.x + dx * inv * vel,
                current.y,
                current.z + dz * inv * vel,
            )
        }
    }
}
