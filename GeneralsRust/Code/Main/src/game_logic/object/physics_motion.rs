use super::*;

impl Object {
    /// C++ applyGravitationalForces residual (host world Y up).
    pub fn apply_gravitational_forces(&mut self) {
        // C++ TheGlobalData->m_gravity residual ≈ -1.0 world units / frame²
        // Host shock gravity is -1.0 on Y.
        self.physics_accel.y += -1.0;
    }

    /// C++ AIUpdateInterface::privateMoveAwayFromUnit residual (fail-closed).
    ///
    /// No full pathfinder: push destination opposite the threat along XZ and
    /// enter move-out-of-way window. Re-request while already yielding + blocked
    /// grants ignore-collisions for 2 seconds (C++ cheat).
    pub fn ai_move_away_from_unit(&mut self, threat_id: ObjectId, threat_pos: glam::Vec3) {
        if self.status.destroyed || !self.is_alive() || !self.can_move() {
            return;
        }
        if self.is_kind_of(crate::game_logic::KindOf::Immobile)
            || self.is_kind_of(crate::game_logic::KindOf::Structure)
        {
            return;
        }
        // Already yielding for this threat.
        if self.move_away_from == Some(threat_id) && self.move_away_frames > 0 {
            if self.is_blocked {
                // C++ setIgnoreCollisionTime(2 sec)
                self.ignore_collisions_until_frame = self.ignore_collisions_until_frame.max(60); // caller should OR with current frame externally
                                                                                                 // Store relative: use flag via ignore_collisions_with as well.
                self.ignore_collisions_with = Some(threat_id);
            }
            return;
        }
        let us = self.get_position();
        let mut dx = us.x - threat_pos.x;
        let mut dz = us.z - threat_pos.z;
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1.0e-3 {
            // Coincident: push along our facing.
            let d = self.unit_direction_vector_2d();
            dx = d.x;
            dz = d.y;
        } else {
            dx /= len;
            dz /= len;
        }
        // PATHFIND_CELL_SIZE * ~2 step away residual.
        let step = PATHFIND_CELL_SIZE_F_RESIDUAL * 2.0;
        let dest = glam::Vec3::new(us.x + dx * step, us.y, us.z + dz * step);
        self.move_away_from = Some(threat_id);
        self.move_away_destination = Some(dest);
        self.move_away_frames = 10 * 30; // 10 seconds temporary state residual
                                         // Nudge velocity toward dest residual (fail-closed vs full path).
        self.movement.velocity.x += dx * 0.5;
        self.movement.velocity.z += dz * 0.5;
    }

    /// Tick move-away temporary state residual.
    pub fn tick_move_away_state(&mut self) {
        if self.move_away_frames > 0 {
            self.move_away_frames -= 1;
            if self.move_away_frames == 0 {
                self.move_away_from = None;
                self.move_away_destination = None;
            }
        }
    }

    /// Clear per-frame blocked residual at start of AI/physics tick.
    pub fn clear_blocked_frame_state(&mut self) {
        if self.is_blocked {
            self.num_frames_blocked = self.num_frames_blocked.saturating_add(1);
            // Stuck residual: blocked for > 1 second (30 frames).
            if self.num_frames_blocked > 30 {
                self.is_blocked_and_stuck = true;
            }
        } else {
            self.num_frames_blocked = 0;
            self.is_blocked_and_stuck = false;
        }
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
        // Host residual defaults when unset: vehicles crush infantry.
        if self.crusher_level == 0 && self.is_kind_of(KindOf::Vehicle) {
            self.crusher_level = 1;
        }
        if self.crushable_level == 255 && self.is_kind_of(KindOf::Infantry) {
            self.crushable_level = 0;
        }
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
            past_crush_point_residual, CrushTarget, PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL,
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
        if !self_crushing_other {
            return false;
        }
        // C++ SquishCollide residual: infantry/crushable under tank with velocity
        // toward victim takes immediate HUGE crush damage (tight radius).
        // Physics front/back crush points still run below for vehicles/props.
        if other.is_kind_of(crate::game_logic::KindOf::Infantry)
            || other.crushable_level < self.crusher_level
        {
            use crate::game_logic::host_squish_collide::{
                should_skip_squish_for_goal_ability, velocity_toward_victim, within_squish_radius,
                SQUISH_HUGE_DAMAGE,
            };
            if !is_ally && !should_skip_squish_for_goal_ability(&other.template_name) {
                let us = self.get_position();
                let them = other.get_position();
                let vel = self.movement.velocity;
                let toward = velocity_toward_victim((us.x, us.z), (them.x, them.z), (vel.x, vel.z));
                let crusher_r = self.selection_radius.max(5.0);
                if toward && within_squish_radius((us.x, us.z), (them.x, them.z), crusher_r) {
                    other.front_crushed = true;
                    other.back_crushed = true;
                    other.apply_crush_die_model_conditions();
                    let _ = other.take_damage_from_typed_death(
                        SQUISH_HUGE_DAMAGE,
                        Some(self.id),
                        crate::game_logic::combat::DamageType::Crush,
                        crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
                    );
                    self.add_physics_overlap(other.id);
                    return true;
                }
            }
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
        let (dx_f, dz_f) = self.unit_direction_xz();
        // major radius residual ≈ selection_radius
        let major = other.selection_radius.max(5.0);
        let offset = major / 2.0;
        let crushee_facing = {
            let y = other.get_orientation();
            (y.cos(), y.sin())
        };
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
            match target {
                CrushTarget::FrontEndCrush => {
                    other.front_crushed = true;
                    other.record_host_crush_vision();
                }
                CrushTarget::BackEndCrush => {
                    other.back_crushed = true;
                    other.record_host_crush_vision();
                }
                CrushTarget::TotalCrush => {
                    other.front_crushed = true;
                    other.back_crushed = true;
                    other.record_host_crush_vision();
                }
                CrushTarget::NoCrush => {}
            }
            // C++ CrushDie::onDie model condition residual.
            other.apply_crush_die_model_conditions();
            let _ = other.take_damage_from_typed_death(
                PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL,
                Some(self.id),
                crate::game_logic::combat::DamageType::Crush,
                crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
            );
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
        stiffness: f32,
        mass: f32,
    ) -> glam::Vec3 {
        use crate::game_logic::host_partition_collision_physics_residual::structure_immobile_bounce_factor;
        let us = self.get_position();
        let mut dx = other_center.x - us.x;
        let mut dy = other_center.y - us.y;
        let mut dz = other_center.z - us.z;
        let mut dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist < 1.0 {
            dist = 1.0;
        }
        let mag = self.movement.velocity.length();
        let factor = structure_immobile_bounce_factor(mag, mass, stiffness);
        // C++ cheats: nuke velocity then apply force direction from delta.
        self.movement.velocity = glam::Vec3::ZERO;
        let dir = glam::Vec3::new(dx / dist, dy / dist, dz / dist);
        // Force on us is opposite separation (push away from other): -delta direction * |factor|
        // factor is already negative; force = factor * unit(delta) pushes us away when factor<0?
        // C++: force = factor * (delta/dist) with factor negative → toward -delta = away from other. Good.
        let force = dir * factor;
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
            vehicle_crash_into_immobile_outcome, PHYSICS_DEFAULT_STRUCTURE_RUBBLE_HEIGHT_RESIDUAL,
        };
        let is_vehicle = self.is_kind_of(KindOf::Vehicle);
        let other_structure = other.is_kind_of(KindOf::Structure);
        let other_immobile =
            other_structure || other.is_kind_of(KindOf::Immobile) || !other.can_move();
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
        self.bounce_audio_pending = self.bounce_audio_pending.saturating_add(1);
        if self.bounce_sound_name.is_empty() {
            self.bounce_sound_name = BOUNCE_SOUND_DEFAULT.to_string();
        }
        self.record_host_bounce_land();
    }

    /// Drain one pending bounce audio emit for GameLogic → TheAudio queue.
    pub fn take_bounce_audio_pending(&mut self) -> Option<(String, f32)> {
        if self.bounce_audio_pending == 0 {
            return None;
        }
        self.bounce_audio_pending = self.bounce_audio_pending.saturating_sub(1);
        self.record_host_bounce_land();
        Some((self.bounce_sound_name.clone(), self.last_bounce_volume))
    }

    /// C++ killWhenRestingOnGround residual.
    ///
    /// When settled on ground with near-zero velocity, kill non-drone (or
    /// unmanned/dead drones).
    pub fn maybe_kill_when_resting_on_ground(&mut self) -> bool {
        if !self.kill_when_resting_on_ground || self.status.destroyed {
            return false;
        }
        if self.get_position().y > 0.05 {
            return false;
        }
        if !self.velocity_is_very_small() {
            return false;
        }
        let is_drone = self.template_name.to_ascii_lowercase().contains("drone");
        // C++: kill if !drone OR dead OR unmanned.
        if is_drone && self.is_alive() && !self.status.disabled_unmanned {
            return false;
        }
        self.kill_from_stun_destruction()
    }

    pub fn apply_shock_fall_damage(&mut self, impact_vy: f32) -> f32 {
        if self.immune_to_falling_damage || self.is_kind_of(KindOf::Projectile) {
            return 0.0;
        }
        // netSpeed = -activeVelZ - minFall (C++ Z-up); host Y-up equivalent.
        let net_speed = (-impact_vy) - Self::min_fall_speed_for_damage();
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
        let damage_amt = net_speed * Self::SHOCK_MASS * Self::FALL_HEIGHT_DAMAGE_FACTOR;
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
            let stiffness = Self::GROUND_STIFFNESS.clamp(0.01, 0.99);
            desired_accel_y = vy.abs() * stiffness;
        }
        self.apply_ypr_damping(Self::BOUNCE_YPR_DAMPING);
        if desired_accel_y > 0.0 {
            // C++ bounceForce.z = mass * desiredAccelZ
            let force_y = self.physics_get_mass() * desired_accel_y;
            // Right orientation residual when inverted.
            if self.shock_up_z < 0.0 {
                self.shock_up_z = 1.0;
            }
            self.shock_pitch_rate = 0.0;
            self.shock_roll_rate = 0.0;
            Some(glam::Vec3::new(0.0, force_y, 0.0))
        } else {
            // Restore original allow bounce residual.
            self.shock_allow_bounce = self.original_allow_bounce;
            None
        }
    }

    /// C++ PhysicsBehavior position integrate + ground clamp residual (one frame).
    ///
    /// `ground_y` is terrain height at object XZ. Returns true if a bounce force was applied.
    pub fn tick_physics_motion_step(&mut self, ground_y: f32) -> bool {
        if self.status.destroyed || !self.is_alive() {
            return false;
        }
        // Held residual not fully ported — skip if explicitly non-mobile structure without fall.
        if self.is_kind_of(crate::game_logic::KindOf::Structure) && !self.allow_to_fall {
            return false;
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
        let mut new_pos = if marching {
            old_pos
        } else {
            old_pos + v
        };
        // YPR rate integrate residual (orientation presentation).
        let pryf = self.pitch_roll_yaw_factor;
        let mut yaw_rate = self.shock_yaw_rate * pryf;
        let mut pitch_rate = self.shock_pitch_rate * pryf;
        let roll_rate = self.shock_roll_rate * pryf;
        // C++ centerOfMassOffset damps pitch toward straight up/down residual.
        if self.center_of_mass_offset != 0.0 {
            // Host residual: approximate pitch angle from shock_up_z.
            let pitch_angle = (1.0 - self.shock_up_z.clamp(-1.0, 1.0))
                .asin()
                .copysign(self.shock_up_z);
            let remaining = if self.center_of_mass_offset > 0.0 {
                std::f32::consts::FRAC_PI_2 - pitch_angle
            } else {
                -std::f32::consts::FRAC_PI_2 + pitch_angle
            };
            pitch_rate *= remaining.sin();
        }
        let _ = roll_rate; // roll applied via shock rates presentation residual
        if yaw_rate.abs() > 1e-8 {
            let yaw = self.get_orientation() + yaw_rate;
            self.set_orientation(yaw);
        }
        let _ = pitch_rate;

        let bounce_force = self.compute_ground_bounce_force(old_y, new_pos.y, ground_y);
        let mut bounced = false;

        // Remember z-vel prior to ground-slam (host Y).
        if new_pos.y <= ground_y {
            let dy = ground_y - new_pos.y;
            self.movement.velocity.y += dy;
            if self.movement.velocity.y > 0.0 {
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
        }

        self.set_position(new_pos);

        if let Some(force) = bounce_force {
            if self.shock_allow_bounce {
                self.apply_physics_force(force);
                // Immediate integrate of bounce accel residual (C++ applies same frame).
                self.integrate_physics_accel();
                bounced = true;
                let _ = self.test_stunned_unit_for_destruction();
            }
        }

        let airborne_end = new_pos.y > ground_y + 0.05;
        // Landing damage residual when was airborne last frame.
        if self.was_airborne_last_frame && !airborne_end && !self.immune_to_falling_damage {
            // doBounceSound residual already exists elsewhere; falling damage peel.
            let impact_vy = v.y;
            let _ = self.apply_shock_fall_damage(impact_vy);
        }
        self.was_airborne_last_frame = airborne_end;
        self.record_host_locomotor();
        self.status.airborne_target = airborne_end;
        let _ = airborne_start; // reserved for future free-fall start residual
                                // C++ killWhenRestingOnGround residual after landing.
        if !airborne_end {
            let _ = self.maybe_kill_when_resting_on_ground();
        }
        bounced
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
            let stiffness = Self::GROUND_STIFFNESS.clamp(0.01, 0.99);
            // C++ desiredAccelZ = fabs(vz)*stiffness; mass≈1 → velocity kick.
            bounce_vy = vy.abs() * stiffness;
        }
        // Damp tumble rates on bounce.
        self.shock_yaw_rate *= Self::BOUNCE_YPR_DAMPING;
        self.shock_pitch_rate *= Self::BOUNCE_YPR_DAMPING;
        self.shock_roll_rate *= Self::BOUNCE_YPR_DAMPING;
        if bounce_vy > 0.0 {
            self.movement.velocity.y = bounce_vy;
            // C++ testStunnedUnitForDestruction on successful bounce force.
            if self.test_stunned_unit_for_destruction() {
                return 0.0;
            }
            // Right the object residual: keep yaw, zero pitch/roll presentation rates.
            self.shock_pitch_rate = 0.0;
            self.shock_roll_rate = 0.0;
            // C++ setAngles after bounce rights pitch/roll when not killed.
            if self.shock_up_z < 0.0 {
                // Already handled by kill path; keep.
            } else {
                self.shock_up_z = 1.0;
            }
            return bounce_vy;
        }
        // Bounce complete — restore original allow (host: off).
        self.shock_allow_bounce = false;
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

    /// C++ PhysicsBehavior::testStunnedUnitForDestruction residual.
    ///
    /// Called on bounce. Kills when upside-down, off-map, cliff without cliff
    /// locomotor, or underwater without water locomotor.
    pub fn test_stunned_unit_for_destruction(&mut self) -> bool {
        if !self.is_shock_stunned() || self.status.destroyed {
            return false;
        }
        self.ensure_locomotor_surfaces();
        // Upside down when transform Z-up residual is negative.
        if self.shock_up_z < 0.0 {
            return self.kill_from_stun_destruction();
        }
        // C++ obj->isOffMap residual.
        let pos = self.get_position();
        if crate::game_logic::host_deliver_payload::is_off_map_default_residual(pos) {
            return self.kill_from_stun_destruction();
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
        self.health.current = 0.0;
        self.status.destroyed = true;
        self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Normal;
        crate::game_logic::host_death_type_log::record(self.id, self.status.death_type.ordinal());
        self.set_ai_state(AIState::Idle);
        self.target = None;
        self.shock_stun_frames = 0;
        self.set_status_disabled_freefall(false);
        self.refresh_model_condition_bits();
        true
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
            return;
        }
        if countdown {
            self.shock_stun_frames = self.shock_stun_frames.saturating_sub(1);
            self.record_host_shock_stun();
        }
        // Integrate yaw rate residual while stunned (tumble settle).
        if self.shock_yaw_rate.abs() > 1e-5 {
            let ori = self.get_orientation() + self.shock_yaw_rate;
            self.set_orientation(ori);
            self.shock_yaw_rate *= 0.92; // friction residual
        }
        self.shock_pitch_rate *= 0.92;
        self.shock_roll_rate *= 0.92;

        // Vertical freefall / bounce residual (host Y-up == C++ Z).
        let ground_y = 0.0;
        let old_y = self.get_position().y;
        // Gravity while airborne or still carrying vertical velocity.
        if old_y > ground_y + 0.01 || self.movement.velocity.y.abs() > 0.01 {
            self.movement.velocity.y += Self::SHOCK_GRAVITY;
            let mut pos = self.get_position();
            let new_y = pos.y + self.movement.velocity.y;
            if new_y <= ground_y {
                // Capture impact velocity before bounce/slam (C++ activeVelZ residual).
                let impact_vy = self.movement.velocity.y;
                let was_air = self.shock_was_airborne || old_y > ground_y + 0.01;
                let bounced = self.handle_shock_ground_bounce(old_y, new_y, ground_y);
                pos.y = ground_y;
                self.set_position(pos);
                // C++ first ground hit while stunned: FLAILING → STUNNED.
                if !self.shock_grounded_once {
                    self.shock_grounded_once = true;
                    // Force model into STUNNED band (frames 1..=15) if still flailing.
                    if self.shock_stun_frames > 15 {
                        self.shock_stun_frames = 15;
                    }
                }
                // C++ WAS_AIRBORNE_LAST_FRAME && !airborneAtEnd → bounce sound + fall damage.
                if was_air {
                    self.record_bounce_land(old_y);
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
                self.set_position(pos);
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
        self.refresh_model_condition_bits();
    }

    /// C++ PhysicsBehavior::getIsStunned residual.
    pub fn is_shock_stunned(&self) -> bool {
        self.shock_stun_frames > 0
    }
}
