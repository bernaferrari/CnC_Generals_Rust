use super::physics::{
    leftover_ground_stiffness, leftover_loco_gravity, leftover_structure_stiffness,
};
use super::*;

impl Object {
    /// C++ applyGravitationalForces residual (host world Y up).
    pub fn apply_gravitational_forces(&mut self) {
        // C++ TheGlobalData->m_gravity (parseAccelerationReal, retail -0.0711).
        self.physics_accel.y += leftover_loco_gravity();
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
        if self.move_away_from == Some(threat_id) && self.move_away_frames > 0 {
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
        self.move_away_from = Some(threat_id);
        self.move_away_destination = Some(dest);
        self.move_away_frames = 10 * 30;
    }

    /// C++ privateMoveAwayFromUnit after getMoveAwayFromPath succeeds.
    pub fn apply_move_away_path(&mut self, threat_id: ObjectId, path: &[glam::Vec3]) {
        if self.status.destroyed || !self.is_alive() || !self.can_move() {
            return;
        }
        if self.is_kind_of(crate::game_logic::KindOf::Immobile)
            || self.is_kind_of(crate::game_logic::KindOf::Structure)
        {
            return;
        }
        if self.move_away_from == Some(threat_id) && self.move_away_frames > 0 {
            if self.is_blocked {
                self.ignore_collisions_until_frame = self.ignore_collisions_until_frame.max(60);
                self.ignore_collisions_with = Some(threat_id);
            }
            return;
        }
        if path.len() < 2 {
            return;
        }
        self.movement.path = path.to_vec();
        self.movement.current_path_index = 1;
        self.movement.target_position = path.last().copied();
        self.start_move();
        self.set_status_moving(true);
        self.move_away_from = Some(threat_id);
        self.move_away_destination = path.last().copied();
        self.move_away_frames = 10 * 30;
        self.record_host_movement();
    }

    /// Tick move-away temporary state residual.
    pub fn tick_move_away_state(&mut self) {
        if self.move_away_frames > 0 {
            self.move_away_frames -= 1;
            if self.move_away_frames == 0 {
                self.move_away_from = None;
                self.move_away_destination = None;
                // C++ AIMoveAwayFromRepulsorsState::onExit clears PANICKING.
                if self.is_panicking {
                    crate::game_logic::host_upgrade_module_residuals::apply_choose_locomotor_set(
                        self,
                        crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Normal,
                        false,
                    );
                }
            }
        }
    }

    /// Leftover `desired_locomotor_set` / Chinook landed install of SET_TAXIING.
    fn apply_leftover_taxiing_locomotor_set(&mut self) {
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
            let _ = crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_kind(
                self,
                crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Taxiing,
            );
        } else {
            self.apply_taxiing_locomotor_set();
        }
    }

    /// Collision pass reset. Frame counters increment in doLocomotor (update_movement).
    pub fn clear_blocked_frame_state(&mut self) {
        self.is_blocked = false;
        self.cur_max_blocked_speed = f32::MAX;
        self.request_other_move_away = None;
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
        self.record_host_crush_vision();
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
    pub fn check_for_overlap_collision(&mut self, other: &mut Object, is_ally: bool) -> bool {
        use crate::game_logic::host_partition_collision_physics_residual::{
            CrushTarget, PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL, past_crush_point_residual,
        };
        self.ensure_crush_levels();
        other.ensure_crush_levels();
        if self.velocity_is_very_small() {
            return false;
        }
        let self_crushing_other = self.can_crush_only(other, is_ally);
        let self_being_crushed = other.can_crush_only(self, is_ally);
        if self_crushing_other && self_being_crushed {
            return false;
        }
        if self_being_crushed {
            return true; // passive overlap
        }
        // C++ SquishCollide::onCollide is independent of TEST_CRUSH_ONLY.
        // Instant-squish when the victim has the authored module.
        if other.has_squish_collide {
            use crate::game_logic::host_squish_collide::{
                SQUISH_HUGE_DAMAGE, authored_crusher_geometry, should_skip_squish_for_goal_ability,
                squish_geom_collides_with, template_has_hijacker_update, velocity_toward_victim,
            };
            let has_hijacker = template_has_hijacker_update(&other.template_name);
            let tnt_active =
                crate::game_logic::host_tank_hunter::is_tank_hunter_template(&other.template_name)
                    && other.ai_state == crate::game_logic::AIState::SpecialAbility
                    && other.target == Some(self.id);
            if !is_ally
                && self.crusher_level > 0
                && !should_skip_squish_for_goal_ability(
                    other.target,
                    self.id,
                    has_hijacker,
                    tnt_active,
                )
            {
                let us = self.get_position();
                let them = other.get_position();
                let vel = self.movement.velocity;
                let toward = velocity_toward_victim((us.x, us.z), (them.x, them.z), (vel.x, vel.z));
                let crusher_g = &self.thing.geometry;
                let crusher_half_x =
                    ((crusher_g.bounds_max.x - crusher_g.bounds_min.x).abs() * 0.5).max(0.0);
                let fallback_major = crusher_g
                    .radius
                    .max(self.selection_radius)
                    .max(crusher_half_x)
                    .max(1.0);
                let fallback_h = (crusher_g.bounds_max.y - crusher_g.bounds_min.y)
                    .abs()
                    .max(crusher_g.radius)
                    .max(1.0);
                let crusher_geom = authored_crusher_geometry(
                    &self.thing.template.geometry_info,
                    fallback_major,
                    fallback_h,
                );
                let victim_g = &other.thing.geometry;
                let victim_h = if other.thing.template.geometry_info.authored {
                    other.thing.template.geometry_info.height.max(0.01)
                } else {
                    (victim_g.bounds_max.y - victim_g.bounds_min.y)
                        .abs()
                        .max(victim_g.radius)
                        .max(1.0)
                };
                if toward
                    && squish_geom_collides_with(
                        (us.x, us.y, us.z),
                        self.get_orientation(),
                        crusher_geom,
                        (them.x, them.y, them.z),
                        other.get_orientation(),
                        victim_h,
                    )
                {
                    // C++ SquishCollide.cpp:88-93: HUGE_DAMAGE + DEATH_CRUSHED.
                    // The victim's die pipeline then runs CrushDie::onDie
                    // (CrushDie.cpp:137-180), which stamps FRONT/BACK crushed
                    // body flags from the dealer-relative crushLocationCheck.
                    let _ = other.take_damage_from_typed_death(
                        SQUISH_HUGE_DAMAGE,
                        Some(self.id),
                        crate::game_logic::combat::DamageType::Crush,
                        crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
                    );
                    if matches!(
                        other.status.death_type,
                        crate::game_logic::host_usa_pilot::HostDeathType::Crushed
                    ) {
                        other.fire_crush_die_from_crusher(Some((us.x, us.z)));
                    }
                    self.add_physics_overlap(other.id);
                    return true;
                }
            }
        }
        if !self_crushing_other {
            return false;
        }
        // add overlap
        let oid = other.id;
        let first =
            self.physics_previous_overlap != Some(oid) && self.physics_current_overlap != Some(oid);
        self.add_physics_overlap(oid);
        if first {
            // 0-amount crush damage residual (DamageFX trigger only).
            let _ = other.take_damage_from_typed_death(
                0.0,
                Some(self.id),
                crate::game_logic::combat::DamageType::Crush,
                crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
            );
        }
        if other.front_crushed && other.back_crushed {
            return true;
        }
        let us = self.get_position();
        let them = other.get_position();
        // C++ PhysicsUpdate.cpp:1488 — crush ray is unit facing, not velocity.
        let (dx_f, dz_f) = self.unit_direction_xz();
        // C++ getGeometryInfo().getMajorRadius() / 2 (PhysicsUpdate.cpp:1490).
        let geom = &other.thing.template.geometry_info;
        let major = if geom.authored {
            geom.major_radius
        } else {
            other.selection_radius.max(1.0)
        };
        let offset = major / 2.0;
        let crushee_facing = other.unit_direction_xz();
        let target = {
            use crate::game_logic::host_partition_collision_physics_residual::select_crush_target_by_perp_residual;
            select_crush_target_by_perp_residual(
                other.front_crushed,
                other.back_crushed,
                (us.x, us.z),
                (them.x, them.z),
                (dx_f, dz_f),
                crushee_facing,
                offset,
            )
        };
        if target == CrushTarget::NoCrush {
            return true;
        }
        let point = match target {
            CrushTarget::FrontEndCrush => (
                them.x + crushee_facing.0 * offset,
                them.z + crushee_facing.1 * offset,
            ),
            CrushTarget::BackEndCrush => (
                them.x - crushee_facing.0 * offset,
                them.z - crushee_facing.1 * offset,
            ),
            CrushTarget::TotalCrush | CrushTarget::NoCrush => (them.x, them.z),
        };
        if past_crush_point_residual((us.x, us.z), point, (dx_f, dz_f), offset) {
            // C++ applies HUGE crush damage only. CrushDie::onDie writes
            // FRONT/BACK/TOTAL from the previous (usually unset) flags.
            let crusher_xz = (us.x, us.z);
            let _ = other.take_damage_from_typed_death(
                PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL,
                Some(self.id),
                crate::game_logic::combat::DamageType::Crush,
                crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
            );
            if matches!(
                other.status.death_type,
                crate::game_logic::host_usa_pilot::HostDeathType::Crushed
            ) {
                other.fire_crush_die_from_crusher(Some(crusher_xz));
            }
        }
        true
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
    }

    /// C++ PhysicsBehavior::scrubVelocityZ residual (host Y-up vertical).
    pub fn scrub_velocity_vertical(&mut self, desired_velocity: f32) {
        if desired_velocity.abs() < 0.001 {
            self.movement.velocity.y = 0.0;
            return;
        }
        let vy = self.movement.velocity.y;
        if (desired_velocity < 0.0 && vy < desired_velocity)
            || (desired_velocity > 0.0 && vy > desired_velocity)
        {
            self.movement.velocity.y = desired_velocity;
        }
    }

    /// C++ parachute vs building jam residual: push out + scrub lateral.
    pub fn apply_parachute_building_bounce_out(
        &mut self,
        other_center: glam::Vec3,
        us_radius: f32,
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
        self.set_position(pos);
        self.scrub_velocity_2d(0.0);
    }

    /// C++ immobile collide stiffness bounce residual on velocity.
    ///
    /// Zeros velocity then applies bounce factor along separation (host XZ + Y).
    /// Returns applied force vector residual (for tests).
    pub fn apply_structure_stiffness_bounce(
        &mut self,
        other_center: glam::Vec3,
        _stiffness: f32,
        mass: f32,
    ) -> glam::Vec3 {
        use crate::game_logic::host_partition_collision_physics_residual::structure_immobile_bounce_factor;
        let us = self.get_position();
        let dx = other_center.x - us.x;
        let dy = other_center.y - us.y;
        let dz = other_center.z - us.z;
        let mut dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist < 1.0 {
            dist = 1.0;
        }
        let mag = self.movement.velocity.length();
        let stiffness = leftover_structure_stiffness();
        let factor = structure_immobile_bounce_factor(mag, mass, stiffness);
        let dir = glam::Vec3::new(dx / dist, dy / dist, dz / dist);
        // C++: force = factor * (delta/dist) with factor negative → away from other.
        let force = dir * factor;
        // C++ PhysicsUpdate.cpp:1377-1384 — nuke vel first so the graze
        // becomes a rebound instead of slide-through (hq-yunv0).
        self.movement.velocity = glam::Vec3::ZERO;
        self.invalidate_velocity_magnitude();
        // mass≈1 → velocity += force (host residual, no separate accel integrate).
        self.movement.velocity += force;
        self.record_host_movement();
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
        // C++ delta.z < 0 → host Y-up falling.
        let falling = self.movement.velocity.y < 0.0;
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
        self.record_host_bounce_land();
    }

    /// Drain one pending bounce audio emit for GameLogic → TheAudio queue.
    pub fn take_bounce_audio_pending(&mut self) -> Option<(String, f32)> {
        if self.bounce_audio_pending == 0 || self.bounce_sound_name.is_empty() {
            self.bounce_audio_pending = 0;
            return None;
        }
        self.bounce_audio_pending = self.bounce_audio_pending.saturating_sub(1);
        self.record_host_bounce_land();
        Some((self.bounce_sound_name.clone(), self.last_bounce_volume))
    }

    /// C++ Get_Z_Vector().Z remapped to host Y-up: world-Y of local up.
    pub fn physics_transform_up_y(&self) -> f32 {
        self.get_transform_matrix().y_axis.y
    }

    fn sync_shock_up_from_transform(&mut self) {
        self.shock_up_z = self.physics_transform_up_y().clamp(-1.0, 1.0);
    }

    /// Keep translation; preserve accumulated pitch/roll (C++ setPosition).
    fn set_position_keep_rotation(&mut self, pos: glam::Vec3) {
        let mut mtx = self.get_transform_matrix();
        mtx.w_axis = pos.extend(1.0);
        self.thing.set_transform_matrix(mtx);
        self.position = pos;
        self.sync_shock_up_from_transform();
    }

    /// C++ PhysicsBehavior::update CenterOfMassOffset * sin(remainingAngle).
    fn center_of_mass_pitch_scale(&self) -> f32 {
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
        if yaw_rate.abs() <= 1e-8 && pitch_rate.abs() <= 1e-8 && roll_rate.abs() <= 1e-8 {
            self.sync_shock_up_from_transform();
            return;
        }
        let mut mtx = self.get_transform_matrix();
        mtx *= glam::Mat4::from_rotation_x(roll_rate);
        mtx *= glam::Mat4::from_rotation_z(pitch_rate);
        mtx *= glam::Mat4::from_rotation_y(yaw_rate);
        self.thing.set_transform_matrix(mtx);
        self.position = mtx.w_axis.truncate();
        self.sync_shock_up_from_transform();
    }

    /// Leftover handle_bounce first-righting (PhysicsUpdate.cpp:505-512):
    /// pitch = 0, roll = 0 or PI from current up so stun-kill still sees a flip.
    fn right_physics_pitch_keep_flip(&mut self) {
        let pos = self.get_position();
        let yaw = self.get_orientation();
        let roll = if self.physics_transform_up_y() > 0.0 {
            0.0
        } else {
            std::f32::consts::PI
        };
        self.thing.set_transform_matrix(
            glam::Mat4::from_translation(pos)
                * glam::Mat4::from_rotation_y(yaw)
                * glam::Mat4::from_rotation_x(roll),
        );
        self.sync_shock_up_from_transform();
    }

    /// C++ killWhenRestingOnGround residual.
    ///
    /// When settled on ground with near-zero velocity, kill non-drone (or
    /// unmanned/dead drones). Airborne is C++ isAboveTerrain (height > 0).
    pub fn maybe_kill_when_resting_on_ground(&mut self) -> bool {
        if !self.kill_when_resting_on_ground || self.status.destroyed {
            return false;
        }
        if self.is_above_terrain() {
            return false;
        }
        if !self.velocity_is_very_small() {
            return false;
        }
        let is_drone = self.is_kind_of(KindOf::Drone);
        // C++: kill if !KINDOF_DRONE OR dead OR unmanned.
        if is_drone && self.is_alive() && !self.status.disabled_unmanned {
            return false;
        }
        self.kill()
    }

    /// C++ Object::kill — lethal UNRESISTABLE so Body/Die modules (FX, OCL) run.
    pub fn kill(&mut self) -> bool {
        if self.status.destroyed {
            return false;
        }
        let max_h = self.health.maximum.max(self.max_health).max(1.0);
        self.take_damage_from_typed_death(
            max_h,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        )
    }

    pub fn apply_shock_fall_damage(&mut self, impact_vy: f32) -> f32 {
        if self.immune_to_falling_damage || self.is_kind_of(KindOf::Projectile) {
            return 0.0;
        }
        // Leftover/C++: netSpeed = -activeVelZ - height_to_speed(minFallHeight).
        // Host Y-up. Compare leftover default ~2.385, not live sqrt(80).
        let min_fall = Self::leftover_compare_min_fall_speed(self.min_fall_speed_for_damage);
        let net_speed = (-impact_vy) - min_fall;
        if net_speed <= 0.0 {
            return 0.0;
        }
        let vx = self.movement.velocity.x;
        let vz = self.movement.velocity.z;
        // Steep-fall gate residual.
        let steep_x =
            vx.abs() <= Self::FALL_TINY_DELTA || (impact_vy / vx).abs() >= Self::FALL_MIN_ANGLE_TAN;
        let steep_z =
            vz.abs() <= Self::FALL_TINY_DELTA || (impact_vy / vz).abs() >= Self::FALL_MIN_ANGLE_TAN;
        if !(steep_x && steep_z) {
            return 0.0;
        }
        let damage_amt = net_speed * self.physics_get_mass() * self.fall_height_damage_factor;
        if damage_amt <= 0.0 {
            return 0.0;
        }
        let killed = self.take_damage_from_typed_death(
            damage_amt,
            Some(self.id),
            crate::game_logic::combat::DamageType::Falling,
            crate::game_logic::host_usa_pilot::HostDeathType::Splatted,
        );
        if killed {
            use crate::game_logic::host_enum_table_residual::MC_BIT_SPLATTED;
            self.model_condition_bits |= 1u128 << MC_BIT_SPLATTED;
            self.refresh_model_condition_bits();
            // refresh may clear SPLATTED if not wired — re-set after.
            self.model_condition_bits |= 1u128 << MC_BIT_SPLATTED;
        }
        damage_amt
    }

    /// C++ PhysicsBehavior::applyYPRDamping residual.
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
        if !self.shock_allow_bounce || new_y > ground_y {
            return None;
        }
        let vy = self.movement.velocity.y;
        let mut desired_accel_y = 0.0;
        if old_y > ground_y && vy < 0.0 {
            let stiffness = leftover_ground_stiffness();
            desired_accel_y = vy.abs() * stiffness;
        }
        self.apply_ypr_damping(Self::BOUNCE_YPR_DAMPING);
        if vy < 0.0 {
            // Leftover handle_bounce vz<0: pitch=0, roll=0 or PI from up.
            // Rates stay 0.7-damped (C++ applyYPRDamping only).
            self.right_physics_pitch_keep_flip();
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
        self.status.airborne_target = height > self.current_airborne_targeting_height() as f32;
    }

    fn current_airborne_targeting_height(&self) -> i32 {
        if self.airborne_targeting_height != 0 && self.airborne_targeting_height != i32::MAX {
            return self.airborne_targeting_height;
        }
        if let Some(name) = self.cur_locomotor_name.as_deref() {
            if !name.is_empty() {
                if let Some(tmpl) = gamelogic::locomotor::ini_bridge::convert_named(name) {
                    return tmpl.airborne_targeting_height;
                }
            }
        }
        if self.airborne_targeting_height != 0 {
            self.airborne_targeting_height
        } else {
            i32::MAX
        }
    }

    /// C++ PhysicsBehavior position integrate + ground clamp residual (one frame).
    /// `ground_y` is terrain height at object XZ. Returns true if a bounce force was applied.
    pub fn tick_physics_motion_step(&mut self, ground_y: f32) -> bool {
        self.apply_leftover_taxiing_locomotor_set();

        self.ground_height = ground_y;

        // C++ PhysicsUpdate.cpp:626 — DISABLED_HELD skips gravity/friction/Euler.
        // Landing onCollide / bounce still run outside that gate.
        if self.is_physics_held() {
            let old_y = self.get_position().y;
            let impact_vy = self.movement.velocity.y;
            self.finish_physics_landing_bookkeeping(old_y, ground_y, impact_vy);
            return false;
        }

        // C++ does not bail on isEffectivelyDead — wrecks keep Euler.
        if self.is_kind_of(crate::game_logic::KindOf::Structure) && !self.allow_to_fall {
            return false;
        }

        self.ground_height = ground_y;

        // C++ PhysicsUpdate.cpp:626-636 applyGravitationalForces every update
        // unless DISABLED_HELD (gated above). No aircraft exception.
        // Stun tick already applies leftover gravity while IS_STUNNED.
        // Living Z-motive units already Euler'd lift+gravity in
        // apply_live_handle_behavior_z when host march ran (hq-g8oig).
        // Dead / disabled skip doLocomotor, so they still need gravity here.
        if self.shock_stun_frames == 0 {
            let host_live_y_euler_already =
                !crate::gameworld_shadow::gameworld_movement_authority_live()
                    && !self.is_disabled()
                    && !self.host_skip_dead_locomotor()
                    && matches!(
                        self.loco_behavior_z,
                        LocomotorBehaviorZ::SurfaceRelativeHeight
                            | LocomotorBehaviorZ::SmoothRelativeToHighestLayer
                            | LocomotorBehaviorZ::AbsoluteHeight
                    );
            if !host_live_y_euler_already {
                self.apply_gravitational_forces();
                self.movement.velocity.y += self.physics_accel.y;
                self.physics_accel.y = 0.0;
                self.invalidate_velocity_magnitude();
            }
        }

        let old_pos = self.get_position();
        let old_y = old_pos.y;
        let airborne_start = old_y > ground_y + 0.05;

        // C++ PhysicsBehavior::update is the sole Euler step. Live march already
        // integrated pos += v*dt in update_movement (v is units/second).
        // Applying `old_pos + v` here treats per-second velocity as per-frame
        // and ~25x retail speed. Only integrate leftover Y / shock here when
        // the unit is not already path-marching this frame.
        let v = self.movement.velocity;
        let marching = self.movement.target_position.is_some() || !self.movement.path.is_empty();
        let mut new_pos = if marching { old_pos } else { old_pos + v };
        // YPR: stun tick already integrates while stunned (avoid double apply).
        if self.shock_stun_frames == 0 {
            let pryf = self.pitch_roll_yaw_factor;
            let yaw_rate = self.shock_yaw_rate * pryf;
            let pitch_rate = self.shock_pitch_rate * pryf * self.center_of_mass_pitch_scale();
            let roll_rate = self.shock_roll_rate * pryf;
            self.apply_physics_ypr(yaw_rate, pitch_rate, roll_rate);
        }

        let bounce_force = self.compute_ground_bounce_force(old_y, new_pos.y, ground_y);
        let mut bounced = false;

        // Remember z-vel prior to ground-slam (host Y).
        if new_pos.y <= ground_y {
            let dy = ground_y - new_pos.y;
            self.movement.velocity.y += dy;
            if self.movement.velocity.y > 0.0 {
                self.movement.velocity.y = 0.0;
            } else if dy.abs() < 1e-6 && self.movement.velocity.y < 0.0 {
                // Marching / no Y integrate: do not accumulate gravity into vy.
                self.movement.velocity.y = 0.0;
            }
            self.invalidate_velocity_magnitude();
            new_pos.y = ground_y;
            self.allow_to_fall = false;
            // Stunned flailing → stunned residual on first ground hit.
            if self.shock_stun_frames > 0 && !self.shock_grounded_once {
                self.shock_grounded_once = true;
            }
        } else if new_pos.y > ground_y {
            if self.stick_to_ground && !self.allow_to_fall {
                new_pos.y = ground_y;
            }
            // C++ getIsDownhillOnly: refuse uphill motive (Locomotor.cpp:1596-1598).
            if self.downhill_only && new_pos.y > old_pos.y + 0.05 {
                new_pos.y = old_pos.y;
                self.movement.velocity.y = 0.0;
                self.invalidate_velocity_magnitude();
            }
            // Climber slope: while FLAG_CLIMBING, scale leftover XY when slope>1
            // (Locomotor.cpp:1734-1739 desiredSpeed /= groundSlope*4).
            if self.is_climbing && matches!(self.loco_appearance, LocomotorAppearance::Climber) {
                let slope = (new_pos.y - old_pos.y).abs().max(1.0);
                if slope > 1.0 {
                    let scale = 1.0 / (slope * 4.0);
                    self.movement.velocity.x *= scale;
                    self.movement.velocity.z *= scale;
                    self.invalidate_velocity_magnitude();
                }
            }
        }

        // C++ PhysicsUpdate.cpp:665-669 — NaN translation destroyObject.
        if !new_pos.x.is_finite() || !new_pos.y.is_finite() || !new_pos.z.is_finite() {
            let hp = self.health.current.max(1.0);
            if crate::gameworld_shadow::gameworld_damage_authority_live() {
                crate::game_logic::host_damage_log::record(self.id, hp, Some(self.id), true);
            } else {
                self.health.current = 0.0;
            }
            self.status.destroyed = true;
            self.movement.velocity = glam::Vec3::ZERO;
            self.invalidate_velocity_magnitude();
            return false;
        }

        self.set_position_keep_rotation(new_pos);

        if let Some(force) = bounce_force {
            if self.shock_allow_bounce {
                self.apply_physics_force(force);
                // Immediate integrate of bounce accel residual (C++ applies same frame).
                self.integrate_physics_accel();
                bounced = true;
                // C++ handleBounce (PhysicsUpdate.cpp:505-517) rights 0-or-PI
                // then testStunned. Do not slam yaw-only here — leftover
                // handle_bounce roll=PI must survive so stun-kill sees a flip
                // and inverted wreck poses stick (hq-p6amn).
                if !self.test_stunned_unit_for_destruction() && !self.status.destroyed {
                    self.right_physics_pitch_keep_flip();
                }
            }
        }

        let airborne_end = new_pos.y > ground_y + 0.05;
        // C++ WAS_AIRBORNE_LAST_FRAME && !airborneAtEnd && !IMMUNE
        if self.was_airborne_last_frame && !airborne_end && !self.immune_to_falling_damage {
            self.record_bounce_land(old_y);
            self.pending_ground_collide = true;
            let impact_vy = v.y;
            let _ = self.apply_shock_fall_damage(impact_vy);
        }
        self.was_airborne_last_frame = airborne_end;
        self.record_host_locomotor();
        self.stamp_airborne_target_from_locomotor();
        let _ = airborne_start; // reserved for future free-fall start residual
        // C++ killWhenRestingOnGround residual after landing.
        if !airborne_end {
            let _ = self.maybe_kill_when_resting_on_ground();
        }
        bounced
    }

    /// C++ landing peel that still runs when HELD skips Euler.
    fn finish_physics_landing_bookkeeping(&mut self, old_y: f32, ground_y: f32, impact_vy: f32) {
        let airborne_end = self.get_position().y > ground_y + 0.05;
        if self.was_airborne_last_frame && !airborne_end && !self.immune_to_falling_damage {
            self.record_bounce_land(old_y);
            self.pending_ground_collide = true;
            let _ = self.apply_shock_fall_damage(impact_vy);
        }
        self.was_airborne_last_frame = airborne_end;
        self.stamp_airborne_target_from_locomotor();
        if !airborne_end {
            let _ = self.maybe_kill_when_resting_on_ground();
        }
    }

    /// C++ PhysicsBehavior::handleBounce residual (world-Y = C++ Z).
    ///
    /// Returns upward bounce velocity applied (0 if no bounce).
    pub fn handle_shock_ground_bounce(&mut self, old_y: f32, new_y: f32, ground_y: f32) -> f32 {
        if !self.shock_allow_bounce || new_y > ground_y {
            return 0.0;
        }
        let mut bounce_vy = 0.0;
        let vy = self.movement.velocity.y;
        if old_y > ground_y && vy < 0.0 {
            let stiffness = leftover_ground_stiffness();
            // C++ desiredAccelZ = fabs(vz)*stiffness; mass≈1 → velocity kick.
            bounce_vy = vy.abs() * stiffness;
        }
        // Damp tumble rates on bounce.
        self.shock_yaw_rate *= Self::BOUNCE_YPR_DAMPING;
        self.shock_pitch_rate *= Self::BOUNCE_YPR_DAMPING;
        self.shock_roll_rate *= Self::BOUNCE_YPR_DAMPING;
        // C++ handleBounce vz<0: setAngles(yaw, 0, up>0 ? 0 : PI) before
        // testStunnedUnitForDestruction. Yaw-only overwrite is leftover
        // update_simple gotBounceForce — it must not run before the stun
        // test, and leftover handle_bounce pose is the 0-or-PI roll.
        if vy < 0.0 {
            self.right_physics_pitch_keep_flip();
        }
        if bounce_vy > 0.0 {
            self.movement.velocity.y = bounce_vy;
            // C++ testStunnedUnitForDestruction on successful bounce force.
            if self.test_stunned_unit_for_destruction() {
                return 0.0;
            }
            return bounce_vy;
        }
        // C++ handleBounce bounceForce.z==0: setAllowBouncing(m_originalAllowBounce).
        self.shock_allow_bounce = self.original_allow_bounce;
        self.record_host_bounce_land();
        0.0
    }

    /// Default locomotor surfaces residual from KindOf (fail-closed ground units).
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

    /// C++ `Object::getAI() != NULL` residual.
    ///
    /// Leftover thing-factory AIUpdate module when the template is loaded;
    /// else combat/mobile heuristic so infantry/vehicles still gate without INI.
    pub fn has_ai_update_interface(&self) -> bool {
        match leftover_stun_template_has_ai_update(&self.template_name) {
            Some(has) => has,
            None => {
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
        }
    }

    /// C++ PhysicsBehavior::testStunnedUnitForDestruction residual.
    ///
    /// Called on bounce. Kills when upside-down, off-map, cliff without cliff
    /// locomotor, or underwater without water locomotor. Cliff/water kills
    /// require AIUpdateInterface (C++ PhysicsUpdate.cpp:1777-1779).
    pub fn test_stunned_unit_for_destruction(&mut self) -> bool {
        if !self.is_shock_stunned() || self.status.destroyed {
            return false;
        }
        self.ensure_locomotor_surfaces();
        // Upside down when integrated transform up-Y < 0 (C++ Get_Z_Vector().Z).
        if self.physics_transform_up_y() < 0.0 {
            return self.kill_from_stun_destruction();
        }
        // C++ obj->isOffMap residual.
        let pos = self.get_position();
        if crate::game_logic::host_deliver_payload::is_off_map_default_residual(pos) {
            return self.kill_from_stun_destruction();
        }
        // C++ AIUpdateInterface *aiInt = obj->getAI(); if (!aiInt) return;
        if !self.has_ai_update_interface() {
            return false;
        }
        // C++ isCliffCell && !hasLocomotorForSurface(CLIFF).
        if self.cell_is_cliff && !self.has_locomotor_for_surface(LOCO_SURFACE_CLIFF) {
            return self.kill_from_stun_destruction();
        }
        // C++ isUnderwater && !hasLocomotorForSurface(WATER).
        if self.cell_is_underwater && !self.has_locomotor_for_surface(LOCO_SURFACE_WATER) {
            return self.kill_from_stun_destruction();
        }
        false
    }

    fn kill_from_stun_destruction(&mut self) -> bool {
        if self.status.destroyed {
            return false;
        }
        // C++ PhysicsBehavior::testStunnedUnitForDestruction → Object::kill().
        let killed = self.kill();
        if killed {
            self.set_ai_state(AIState::Idle);
            self.target = None;
            self.shock_stun_frames = 0;
            self.set_status_disabled_freefall(false);
            self.refresh_model_condition_bits();
        }
        killed
    }

    /// Tick shock stun residual (once per logic frame).
    pub fn tick_shock_stun(&mut self) {
        self.tick_shock_stun_with_countdown(true);
    }

    /// Wave 764: under coupled dual-tick, GW sole-decrements `shock_stun_frames`;
    /// host still integrates tumble/bounce physics without dual-countdown.
    pub fn tick_shock_stun_physics_only(&mut self) {
        self.tick_shock_stun_with_countdown(false);
    }

    fn tick_shock_stun_with_countdown(&mut self, countdown: bool) {
        if self.shock_stun_frames == 0 {
            // Damp residual rates when fully settled.
            self.shock_yaw_rate *= 0.85;
            self.shock_pitch_rate *= 0.85;
            self.shock_roll_rate *= 0.85;
            if self.shock_yaw_rate.abs() < 1e-4 {
                self.shock_yaw_rate = 0.0;
            }
            // Grounded settle: clear freefall leftovers.
            if self.movement.velocity.y.abs() < 0.25 {
                self.movement.velocity.y = 0.0;
                self.shock_was_airborne = false;
                self.shock_allow_bounce = false;
                self.set_status_disabled_freefall(false);
                let _ = self.maybe_kill_when_resting_on_ground();
            }
            // Settled stun ticks must still re-derive model conditions, or
            // stale STUNNED/FREEFALL bits from the last stunned refresh stick
            // after relief (C++ PhysicsBehavior::update re-evaluates
            // IS_STUNNED / IS_IN_FREEFALL every frame, PhysicsUpdate.cpp:671-702).
            self.refresh_model_condition_bits();
            return;
        }
        // Capture before this frame's land so first ground contact keeps STUNNED
        // one frame (C++ ≈1 frame grounded stun; FLAILING → STUNNED visible).
        // Also record pre-tick height band so relief defers on the landing frame.
        let already_on_ground =
            self.shock_grounded_once || self.get_position().y <= self.ground_height + 0.05;
        let was_significantly_airborne = self.is_significantly_above_terrain();
        // C++ has no stun timer. Keep IS_STUNNED until settle relief.
        let _ = countdown;
        // Integrate YPR while stunned (tumble settle). Motion step skips YPR
        // when stunned so this is the sole live HAS_PITCHROLLYAW pass.
        // C++ PhysicsUpdate.cpp:715-727 COM sine applies every YPR frame,
        // including stunned (shocked units keep pitch/roll rates).
        let pryf = self.pitch_roll_yaw_factor;
        self.apply_physics_ypr(
            self.shock_yaw_rate * pryf,
            self.shock_pitch_rate * pryf * self.center_of_mass_pitch_scale(),
            self.shock_roll_rate * pryf,
        );
        self.shock_yaw_rate *= 0.92;
        self.shock_pitch_rate *= 0.92;
        self.shock_roll_rate *= 0.92;

        // Vertical freefall / bounce residual (host Y-up == C++ Z).
        let ground_y = self.ground_height;
        let old_y = self.get_position().y;
        // Gravity while airborne or still carrying vertical velocity.
        if old_y > ground_y + 0.01 || self.movement.velocity.y.abs() > 0.01 {
            self.movement.velocity.y += leftover_loco_gravity();
            let mut pos = self.get_position();
            let new_y = pos.y + self.movement.velocity.y;
            if new_y <= ground_y {
                // Capture impact velocity before bounce/slam (C++ activeVelZ residual).
                let impact_vy = self.movement.velocity.y;
                let was_air = self.shock_was_airborne || old_y > ground_y + 0.01;
                let bounced = self.handle_shock_ground_bounce(old_y, new_y, ground_y);
                pos.y = ground_y;
                self.set_position_keep_rotation(pos);
                // C++ first ground hit while stunned: FLAILING → STUNNED.
                if !self.shock_grounded_once {
                    self.shock_grounded_once = true;
                }
                // C++ WAS_AIRBORNE_LAST_FRAME && !airborneAtEnd → bounce sound + fall damage.
                if was_air {
                    if self.bounce_audio_pending == 0 {
                        self.record_bounce_land(old_y);
                    }
                    self.pending_ground_collide = true;
                    let _ = self.apply_shock_fall_damage(impact_vy);
                }
                if bounced <= 0.0 {
                    // Slam residual: clamp downward vel at ground.
                    if self.movement.velocity.y < 0.0 {
                        self.movement.velocity.y = 0.0;
                    }
                    self.shock_was_airborne = false;
                    // C++ clear IS_IN_FREEFALL / DISABLED_FREEFALL when grounded.
                    self.set_status_disabled_freefall(false);
                } else {
                    // Bounce still airborne residual.
                    self.shock_was_airborne = true;
                    self.set_status_disabled_freefall(true);
                }
            } else {
                pos.y = new_y;
                self.set_position_keep_rotation(pos);
                self.shock_was_airborne = true;
                // C++ IS_IN_FREEFALL → DISABLED_FREEFALL + MODELCONDITION_FREEFALL.
                self.set_status_disabled_freefall(true);
            }
        } else {
            // Lateral bleed only when grounded.
            if self.movement.velocity.y.abs() < 0.5 {
                self.movement.velocity.y = 0.0;
            }
            self.set_status_disabled_freefall(false);
            self.shock_was_airborne = false;
        }
        // Lateral friction residual while stunned on ground.
        if self.get_position().y <= ground_y + 0.01 {
            self.movement.velocity.x *= 0.92;
            self.movement.velocity.z *= 0.92;
            // Ground contact residual: freefall disable only while airborne.
            if self.movement.velocity.y <= 0.01 {
                self.set_status_disabled_freefall(false);
            }
            // C++ killWhenRestingOnGround after settle.
            let _ = self.maybe_kill_when_resting_on_ground();
        }
        // C++ PhysicsUpdate.cpp:672-682: clear stun when |vel|<0.5 or not significantly airborne.
        self.maybe_clear_shock_stun_relief(already_on_ground, was_significantly_airborne);
        self.refresh_model_condition_bits();
    }

    /// C++ STUN_RELIEF_EPSILON / !isSignificantlyAboveTerrain residual.
    fn maybe_clear_shock_stun_relief(
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
            self.record_host_shock_stun();
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
    /// - Legs/climber/other/hover/thrust: `calcSlowDownDist` → minSpeed, no IS_BRAKING
    ///   (`Locomotor.cpp:1648-1653`, `:2368-2374`). Only treads/wheels set the pose cheat.
    /// - Treads: `(actual/1.5)*(actual/braking)` + squared `braking_factor`.
    /// - Wheels: +1 frame, donut timer (40 units / 2.5s), then `braking_factor=1`.
    pub fn apply_cpp_approach_brake(
        &mut self,
        on_path_dist: f32,
        actual_speed: f32,
        desired_speed: f32,
        logic_frame: u32,
    ) -> f32 {
        const MAX_BRAKING_FACTOR: f32 = 5.0;
        const DONUT_DISTANCE: f32 = 4.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
        const DONUT_TIME_FRAMES: u32 = 75; // 2.5s * 30
        let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;

        if matches!(self.loco_appearance, LocomotorAppearance::Wings) {
            // C++ Locomotor.cpp:1046-1050 clears IS_BRAKING for LOCO_WINGS,
            // then moveTowardsPositionWings delegates to Other which still
            // floors to minSpeed (Locomotor.cpp:2368-2374).
            self.is_braking = false;
        }
        if self.no_slow_down_as_approaching_dest {
            return desired_speed;
        }

        let braking = self.braking.max(1.0e-3);
        // Far-from-goal IS_BRAKING clear (Locomotor.cpp:941-946) lives in
        // locoUpdate *before* the 2× path-raise. Do not repeat it here on the
        // already-raised on_path or the latch is killed the same frame.
        let mut goal_speed = desired_speed;

        match self.loco_appearance {
            LocomotorAppearance::Treads => {
                let slow_down_time = actual_speed / braking;
                let slow_down_dist = (actual_speed / 1.5) * slow_down_time;
                if on_path_dist < slow_down_dist && !self.is_braking {
                    self.is_braking = true;
                    self.braking_factor = 1.1;
                }
                if on_path_dist > cell && on_path_dist > 2.0 * slow_down_dist {
                    self.is_braking = false;
                }
                if self.is_braking {
                    if on_path_dist > 0.0 {
                        self.braking_factor = slow_down_dist / on_path_dist;
                    }
                    self.braking_factor *= self.braking_factor;
                    if self.braking_factor > MAX_BRAKING_FACTOR {
                        self.braking_factor = MAX_BRAKING_FACTOR;
                    }
                    if slow_down_dist > on_path_dist {
                        goal_speed = (actual_speed - braking).max(0.0);
                    } else if slow_down_dist > on_path_dist * 0.75 {
                        goal_speed = (actual_speed - braking / 2.0).max(0.0);
                    } else {
                        goal_speed = actual_speed;
                    }
                }
            }
            LocomotorAppearance::WheelsFour | LocomotorAppearance::Motorcycle => {
                let slow_down_time = actual_speed / braking + 1.0;
                let slow_down_dist = (actual_speed / 1.5) * slow_down_time + actual_speed;
                let mut effective = slow_down_dist;
                if effective < cell {
                    effective = cell;
                }
                if on_path_dist < effective && !self.is_braking {
                    self.is_braking = true;
                    self.braking_factor = 1.1;
                }
                if on_path_dist > cell && on_path_dist > 2.0 * slow_down_dist {
                    self.is_braking = false;
                }
                if self.donut_timer == u32::MAX {
                    self.donut_timer = logic_frame.saturating_add(DONUT_TIME_FRAMES);
                }
                if on_path_dist > DONUT_DISTANCE {
                    self.donut_timer = logic_frame.saturating_add(DONUT_TIME_FRAMES);
                } else if self.donut_timer < logic_frame {
                    self.is_braking = true;
                }
                if self.is_braking {
                    if on_path_dist > 0.0 {
                        self.braking_factor = slow_down_dist / on_path_dist;
                    }
                    self.braking_factor *= self.braking_factor;
                    if self.braking_factor > MAX_BRAKING_FACTOR {
                        self.braking_factor = MAX_BRAKING_FACTOR;
                    }
                    // C++ Locomotor.cpp:1420 overwrites braking_factor back to 1.0.
                    self.braking_factor = 1.0;
                    if slow_down_dist > on_path_dist {
                        goal_speed = (actual_speed - braking).max(0.0);
                    } else if slow_down_dist > on_path_dist * 0.75 {
                        goal_speed = (actual_speed - braking / 2.0).max(0.0);
                    } else {
                        goal_speed = actual_speed;
                    }
                }
            }
            _ => {
                // Legs / climber / hover / other / thrust / wings: desired = minSpeed.
                // C++ never sets IS_BRAKING here (Locomotor.cpp:1648-1653, 2368-2374).
                let floor = self.min_speed.max(0.0);
                let slow = crate::game_logic::calc_slow_down_dist(actual_speed, floor, braking);
                if on_path_dist < slow {
                    goal_speed = floor;
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
        if !self.is_braking {
            return current;
        }
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
        let min_vel = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL / 30.0;
        let mut vel = self.movement.velocity.length() * dt.max(1.0e-6);
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

/// C++ leftover `get_ai_update_interface().is_some()` via thing-factory modules.
fn leftover_stun_template_has_ai_update(template_name: &str) -> Option<bool> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    Some(
        tmpl.get_behavior_module_info()
            .iter()
            .any(|entry| leftover_stun_module_is_ai_update(entry.name.as_str())),
    )
}

fn leftover_stun_module_is_ai_update(name: &str) -> bool {
    name.eq_ignore_ascii_case("AIUpdateInterface")
        || name.eq_ignore_ascii_case("AIUpdate")
        || name.to_ascii_lowercase().ends_with("aiupdate")
}
