use super::*;

pub(in crate::game_logic) struct ZBehaviorStep {
    pub requires_constant_calling: bool,
    pub position: Option<glam::Vec3>,
}

/// Source locals preserved across the synchronous orientation callback.
/// Velocity and mass deliberately are not captured: C++ rereads both afterward.
pub(in crate::game_logic) struct ThrustStep {
    pub reaction: Option<super::PositionReaction>,
    force: Option<(glam::Vec3, f32, f32)>,
}
impl Object {
    pub fn take_damage(
        &mut self,
        damage: f32,
        frame: u32,
        health_events: &mut crate::game_logic::HostHealthEvents,
    ) -> bool {
        self.take_damage_from(damage, None, frame, health_events)
    }

    /// Apply damage with optional C++ BodyModule last-damage-source residual.
    ///
    /// Passive AI mood (WaitForAttack) uses `last_damage_source` for idle
    /// mood-target retaliate residual.

    /// C++ PhysicsBehavior::applyShock residual (ground units only).
    ///
    /// Adds lateral+up velocity impulse and a short stun residual. Airborne
    /// targets and projectiles are immune (C++ Object.cpp:1808 isAirborneTarget
    /// / KINDOF_PROJECTILE). Parked aircraft are *not* immune.

    /// C++ PhysicsBehavior defaults for shock random rotation residual.
    pub const SHOCK_MAX_YAW: f32 = 0.05;
    pub const SHOCK_MAX_PITCH: f32 = 0.025;
    pub const SHOCK_MAX_ROLL: f32 = 0.025;

    /// C++ PhysicsBehavior::applyRandomRotation residual.
    ///
    /// Adds random yaw/pitch/roll rates and immediately kicks orientation yaw
    /// so the tumble is observable without a full rigid-body integrator.
    /// C++ PhysicsUpdate.cpp:357-358: STICK_TO_GROUND early-return only.
    /// Infantry/Structure still tumble unless that flag is set.
    pub fn apply_shock_random_rotation(&mut self, seed: u32) {
        self.apply_shock_random_rotation_with_operation(seed, 0, &mut Default::default())
    }

    pub fn apply_shock_random_rotation_with_operation(
        &mut self,
        seed: u32,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if self.stick_to_ground {
            return;
        }
        use crate::game_logic::host_rng_residual::pure_logic_random_real;
        // GameLogicRandomValue(-1, 1) residual via pure stream.
        let yaw_m = pure_logic_random_real(seed, 10, -1.0, 1.0);
        let pitch_m = pure_logic_random_real(seed, 11, -1.0, 1.0);
        let roll_m = pure_logic_random_real(seed, 12, -1.0, 1.0);
        let dyaw = Self::SHOCK_MAX_YAW * yaw_m;
        let dpitch = Self::SHOCK_MAX_PITCH * pitch_m;
        let droll = Self::SHOCK_MAX_ROLL * roll_m;
        self.shock_yaw_rate += dyaw;
        self.shock_pitch_rate += dpitch;
        self.shock_roll_rate += droll;
        // Immediate yaw kick first (set_orientation rebuilds yaw-only).
        let ori = self.get_orientation() + self.shock_yaw_rate;
        self.set_orientation_with_effects(ori, operation_effects);
        // Then apply this increment of pitch/roll onto the transform so
        // upside-down kill can see integrated up-Y, not a synthetic scalar.
        let pryf = self.pitch_roll_yaw_factor;
        self.apply_physics_ypr_with_effects(
            0.0,
            dpitch * pryf,
            droll * pryf,
            operation_frame,
            operation_effects,
        );
        // C++ applyRandomRotation: setAllowBouncing(true) after stick gate.
        self.shock_allow_bounce = true;
    }

    pub fn apply_shock_wave_impulse(&mut self, force: glam::Vec3) -> bool {
        self.apply_shock_wave_impulse_with_operation(force, 0, &mut Default::default())
    }

    pub fn apply_shock_wave_impulse_with_operation(
        &mut self,
        force: glam::Vec3,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        if !self.is_alive() {
            return false;
        }
        // C++ Object.cpp:1808: only airborne-target + projectiles skip toss.
        if self.status.airborne_target {
            return false;
        }
        if self.is_kind_of(KindOf::Projectile) || self.object_type == ObjectType::Projectile {
            return false;
        }
        // C++ Object.cpp:1806-1832: no Infantry/Structure skip before applyShock.
        // C++ applyShock: scale by (1 - clamp(shockResistance, 0, 1)), then
        // applyForce divides by getMass(). Resistance >= 1 yields zero toss.
        let resisted = force * (1.0 - self.shock_resistance.clamp(0.0, 1.0));
        if resisted.length_squared() > 0.0 {
            // C++ applyShock: resist then applyForce (a = F/m). No post-impulse speed cap.
            self.movement.velocity += resisted / self.physics_get_mass();
            self.invalidate_velocity_magnitude();
        }
        // C++ applyRandomRotation residual (deterministic seed from id + force).
        let seed = self
            .id
            .0
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add((force.x.to_bits()).wrapping_mul(0x85EB_CA6B))
            .wrapping_add(force.z.to_bits());
        self.apply_shock_random_rotation_with_operation(seed, operation_frame, operation_effects);
        self.shock_grounded_once = false;
        self.ensure_locomotor_surfaces();
        // Strong upward impulse residual: freefall model bit while airborne from shock.
        if self.movement.velocity.y > 8.0 {
            use crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL;
            self.model_condition_bits |= 1u128 << MC_BIT_FREEFALL;
            self.shock_was_airborne = true;
        }
        // C++ setStunned(true) — no duration. Clear when |vel|<0.5 or
        // !isSignificantlyAboveTerrain (PhysicsUpdate.cpp:671-682).
        self.shock_stun_frames = u32::MAX;
        self.refresh_model_condition_bits();
        if matches!(
            self.ai_state(),
            AIState::Attacking | AIState::AttackMoving | AIState::Moving
        ) {
            self.set_status_moving(true);
        }
        true
    }

    /// Retail GameData.ini GroundStiffness (C++ ctor 0.5 is unused after parse).
    pub const GROUND_STIFFNESS: f32 = 0.8;
    /// Retail GameData.ini StructureStiffness.
    pub const STRUCTURE_STIFFNESS: f32 = 0.3;
    /// Retail GameData.ini Gravity -64 dist/sec² → parseAccelerationReal / 900.
    pub const SHOCK_GRAVITY: f32 = -64.0 / 900.0;
    /// C++ handleBounce YPR damping residual.
    pub const BOUNCE_YPR_DAMPING: f32 = 0.7;
    /// C++ PhysicsBehavior mass default residual.
    pub const SHOCK_MASS: f32 = 1.0;
    /// C++ FallHeightDamageFactor default residual.
    pub const FALL_HEIGHT_DAMAGE_FACTOR: f32 = 1.0;
    /// C++ min fall angle tan residual (~71 degrees).
    pub const FALL_MIN_ANGLE_TAN: f32 = 3.0;
    pub const FALL_TINY_DELTA: f32 = 0.01;

    /// C++ heightToSpeed(height) = sqrt(|2*g*h|) with parsed GlobalData gravity.
    pub fn height_to_fall_speed(height: f32) -> f32 {
        (2.0 * Self::SHOCK_GRAVITY.abs() * height.abs()).sqrt()
    }

    /// C++ PhysicsBehaviorModuleData::m_minFallSpeedForDamage default (height 40).
    pub fn min_fall_speed_for_damage() -> f32 {
        Self::height_to_fall_speed(40.0)
    }

    /// Already-resolved module data stays exact; no numeric default guessing.
    pub(super) fn fall_damage_speed_threshold(&self, stored: f32) -> f32 {
        if stored.is_finite() && stored >= 0.0 {
            stored
        } else {
            (2.0 * self.physics_gravity().abs() * 40.0).sqrt()
        }
    }

    /// C++ falling-damage residual when leaving airborne for ground.
    ///
    /// `impact_vy` is world-Y velocity at impact (negative when falling).
    /// Returns damage applied (0 if none).

    /// C++ isVerySmall3D residual on velocity.
    pub fn velocity_is_very_small(&self) -> bool {
        let v = self.movement.velocity;
        v.x.abs() < VERY_SMALL_VEL && v.y.abs() < VERY_SMALL_VEL && v.z.abs() < VERY_SMALL_VEL
    }

    /// C++ PhysicsBehavior::doBounceSound residual (event count + fall dy + volume).

    /// C++ PhysicsBehavior onCollide vehicle-into-immobile crash residual.

    /// C++ PhysicsBehavior::scrubVelocity2D residual (host XZ ground plane).
    ///
    /// If desired < 0.001, zero lateral velocity. Else scale down if faster than desired.

    /// C++ PhysicsBehavior::setIgnoreCollisionsWith residual.

    /// C++ Object::getUnitDirectionVector2D residual (XZ ground, glam x/z).
    pub fn unit_direction_vector_2d(&self) -> glam::Vec2 {
        // Match unit_direction_xz: orientation 0 faces +X (host XZ plane).
        let (x, z) = self.unit_direction_xz();
        glam::Vec2::new(x, z)
    }

    /// C++ AIUpdateInterface::blockedBy (AIUpdate.cpp:1272-1376).
    ///
    /// Near-goal, reverse, path-priority, and off-angle yield match leftover
    /// `collision_system` / C++. Infantry-infantry still uses C++ `dot<=0.25`
    /// (not leftover always-false). `is_ally` is the crusher's
    /// `getRelationship == ALLIES` (Object.cpp:1096).
    pub fn ai_blocked_by(&self, other: &Object, is_ally: bool) -> bool {
        let cell = self.pathfind_goal_cell;
        if cell.0 > 0 && cell.1 > 0 {
            if let Some(goal) = self.host_blocked_by_goal() {
                let us = self.get_position();
                let dx = (goal.x - us.x).abs();
                let dz = (goal.z - us.z).abs();
                if dx < PATHFIND_CELL_SIZE_F_RESIDUAL && dz < PATHFIND_CELL_SIZE_F_RESIDUAL {
                    return false;
                }
            }
        }

        if self.can_crush_or_squish(other, is_ally) {
            return false;
        }
        if !crate::game_logic::pathfinding::PathfindingGrid::is_doing_ground_movement_full(other) {
            return false;
        }
        if self.moving_backwards
            && self
                .cur_locomotor_name
                .as_ref()
                .is_some_and(|name| !name.is_empty())
        {
            return false;
        }

        let us = self.get_position();
        let them = other.get_position();
        let dx = them.x - us.x;
        let dz = them.z - us.z; // host XZ ground plane (C++ XY)
        let dsqr = dx * dx + dz * dz;

        let our_dir = self.unit_direction_vector_2d();
        let their_dir = other.unit_direction_vector_2d();
        let dir_dot = our_dir.x * their_dir.x + our_dir.y * their_dir.y;

        // Infantry vs infantry: only block if same-ish heading.
        if self.is_kind_of(crate::game_logic::KindOf::Infantry)
            && other.is_kind_of(crate::game_logic::KindOf::Infantry)
            && dir_dot <= 0.25
        {
            return false;
        }

        // Same-cell: C++ hasHigherPathPriority (dozer > vehicle > infantry,
        // then heading, then lower ObjectId).
        if dsqr < PATHFIND_CELL_SIZE_F_RESIDUAL * PATHFIND_CELL_SIZE_F_RESIDUAL * 0.0001 {
            return self.has_higher_path_priority(other);
        }

        // C++ blocked > 1s and not the same heading: pass through before angles.
        if self.num_frames_blocked > 30 && dir_dot <= 0.0 {
            return false;
        }
        // Relative angle of other from us along our facing.
        let collision_angle = self.relative_angle_2d_to(them);
        let other_angle = other.relative_angle_2d_to(us);
        let mut angle_limit = std::f32::consts::FRAC_PI_4; // 45 deg
        // C++ otherMoving is locomotorGoalType != NONE, not current velocity.
        let other_moving = other.locomotor_goal_type != super::LocoGoalType::None;
        if !other_moving {
            angle_limit *= 0.75;
        }
        if collision_angle > std::f32::consts::FRAC_PI_2
            || collision_angle < -std::f32::consts::FRAC_PI_2
        {
            return false; // moving away
        }
        if collision_angle > angle_limit || collision_angle < -angle_limit {
            if dir_dot <= 0.0 {
                return false;
            }
            if other_moving && (other_angle > angle_limit || other_angle < -angle_limit) {
                // C++ uses pos-otherPos (self-other) plus heading delta.
                let sep_x = us.x - them.x;
                let sep_z = us.z - them.z;
                let adjust_dx = sep_x + our_dir.x - their_dir.x;
                let adjust_dz = sep_z + our_dir.y - their_dir.y;
                if dsqr > adjust_dx * adjust_dx + adjust_dz * adjust_dz {
                    if self.has_higher_path_priority(other) {
                        return false;
                    }
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
        !other.status.destroyed && other.is_alive()
    }

    /// C++ AIUpdateInterface::hasHigherPathPriority (AIUpdate.cpp:1191-1228).
    fn has_higher_path_priority(&self, other: &Object) -> bool {
        let self_dozer = self.is_kind_of(crate::game_logic::KindOf::Dozer);
        let other_dozer = other.is_kind_of(crate::game_logic::KindOf::Dozer);
        if self_dozer && !other_dozer {
            return true;
        }
        if !self_dozer && other_dozer {
            return false;
        }
        let self_vehicle = self.is_kind_of(crate::game_logic::KindOf::Vehicle);
        let other_inf = other.is_kind_of(crate::game_logic::KindOf::Infantry);
        if self_vehicle && other_inf {
            return true;
        }
        if self.is_kind_of(crate::game_logic::KindOf::Infantry)
            && other.is_kind_of(crate::game_logic::KindOf::Vehicle)
        {
            return false;
        }

        let our_dir = self.unit_direction_vector_2d();
        let their_dir = other.unit_direction_vector_2d();
        if our_dir.x * their_dir.x + our_dir.y * their_dir.y <= 0.0 {
            return self.id.0 < other.id.0;
        }
        let us = self.get_position();
        let them = other.get_position();
        let combined_x = our_dir.x + their_dir.x;
        let combined_z = our_dir.y + their_dir.y;
        let vx = them.x - us.x;
        let vz = them.z - us.z;
        let dot_product = combined_x * vx + combined_z * vz;
        if dot_product > 0.0 {
            return false;
        }
        if dot_product < 0.0 {
            return true;
        }
        self.id.0 < other.id.0
    }

    /// Leftover / C++ path destination used by the near-goal ignore.
    fn host_blocked_by_goal(&self) -> Option<glam::Vec3> {
        self.movement
            .path
            .last()
            .copied()
            .or(self.path_goal_position)
            .or(self.requested_destination)
    }

    /// C++ AIUpdateInterface::calculateMaxBlockedSpeed residual.
    pub fn calculate_max_blocked_speed(&self, other: &Object) -> f32 {
        let us = self.get_position();
        let them = other.get_position();
        let mut vx = them.x - us.x;
        let mut vz = them.z - us.z;
        let len = (vx * vx + vz * vz).sqrt();
        if len < 1.0e-4 {
            return self.cur_max_blocked_speed;
        }
        vx /= len;
        vz /= len;
        let other_dir = other.unit_direction_vector_2d();
        let speed_factor = vx * other_dir.x + vz * other_dir.y;
        if speed_factor < 0.0 {
            return 0.0; // they run into us
        }
        let other_vel = other.movement.velocity;
        let other_speed_2d = (other_vel.x * other_vel.x + other_vel.z * other_vel.z).sqrt();
        let away_speed = other_speed_2d * speed_factor;
        let our_dir = self.unit_direction_vector_2d();
        let toward = vx * our_dir.x + vz * our_dir.y;
        if toward <= 0.0 {
            return self.cur_max_blocked_speed;
        }
        let mut max_speed = away_speed / toward;
        // C++ AIUpdate.cpp:1262-1265: formation members do not crowd.
        if self.formation_id != 0 && self.formation_id == other.formation_id {
            max_speed *= 0.55;
        }
        if max_speed > self.cur_max_blocked_speed {
            return self.cur_max_blocked_speed;
        }
        max_speed
    }

    /// C++ `AIUpdateInterface::isMoving` (AIUpdate.cpp:3169-3180).
    pub fn host_ai_is_moving(&self) -> bool {
        if self.ai_state() == AIState::Idle {
            return false;
        }
        self.locomotor_goal_type != super::LocoGoalType::None || self.status.moving
    }

    /// C++ AIUpdateInterface::processCollision residual (force-apply gate + blocked).
    ///
    /// Returns true if physics should apply bounce force. Sets is_blocked /
    /// cur_max_blocked_speed when self is moving into other.
    pub fn ai_process_collision(
        &mut self,
        other: &Object,
        current_frame: u32,
        is_ally: bool,
        other_sees_us_as_ally: bool,
    ) -> bool {
        if !self.allow_collide_force {
            return false;
        }
        if self.can_path_through_units {
            return false;
        }
        if self.ignore_collisions_until_frame > 0
            && current_frame < self.ignore_collisions_until_frame
        {
            return false;
        }
        // C++ AIUpdate.cpp:1423-1425: aiOther==NULL → FALSE (buildings/props).
        // is_mobile is the live stand-in for getAI() (infantry/vehicle/aircraft).
        if !other.is_mobile() {
            return false;
        }
        if !crate::game_logic::pathfinding::PathfindingGrid::is_doing_ground_movement_full(self)
            || !crate::game_logic::pathfinding::PathfindingGrid::is_doing_ground_movement_full(
                other,
            )
        {
            return false;
        }

        let self_moving = self.host_ai_is_moving();
        if self_moving {
            let blocked = self.ai_blocked_by(other, is_ally);
            if blocked {
                // C++ AIUpdate::processCollision (AIUpdate.cpp:1410-1444):
                // a panicking infantry unit bounces before m_isBlocked is set.
                if self.ai_state() == super::AIState::Panic
                    && self.is_kind_of(crate::game_logic::KindOf::Infantry)
                {
                    return true;
                }
                self.is_blocked = true;
                if other.host_ai_is_moving() && other.waiting_for_path {
                    return false;
                }
                let max_speed = self.calculate_max_blocked_speed(other);
                if max_speed < self.cur_max_blocked_speed {
                    self.cur_max_blocked_speed = max_speed;
                }
                let already_yielding = other.move_away_frames > 0
                    && (other.move_away_from == Some(self.id)
                        || other.move_away_from_2 == Some(self.id));
                if !already_yielding
                    && other.is_kind_of(crate::game_logic::KindOf::Infantry)
                    && !self.is_kind_of(crate::game_logic::KindOf::Infantry)
                {
                    let other_busy = matches!(
                        other.ai_state(),
                        super::AIState::Constructing
                            | super::AIState::Repairing
                            | super::AIState::SpecialAbility
                            | super::AIState::Capturing
                    );
                    if !other.status.using_ability && !other_busy {
                        self.request_other_move_away = Some(other.id);
                    }
                    return false;
                }
                if self.num_frames_blocked == 0 {
                    self.num_frames_blocked = 1;
                }
                if !self.need_to_rotate() {
                    // C++ isMoving: idle is not moving; else goal or m_isMoving.
                    let other_moving = other.host_ai_is_moving();
                    if !other_moving {
                        self.is_blocked_and_stuck = true;
                    } else if other.ai_blocked_by(self, other_sees_us_as_ally)
                        && !other.need_to_rotate()
                        && !self.has_higher_path_priority(other)
                    {
                        self.request_self_yield_from = Some(other.id);
                    }
                } else {
                    self.num_frames_blocked = 1;
                }
                return false;
            }
        } else if !self.is_alive()
            && self.is_kind_of(crate::game_logic::KindOf::Infantry)
            && !other_sees_us_as_ally
            && !other.status.disabled_unmanned
            && other.crusher_level > 0
            && other.has_squish_collide
        {
            return true;
        } else if !other.host_ai_is_moving() {
            let us = self.get_position();
            let them = other.get_position();
            let dx = us.x - them.x;
            let dz = us.z - them.z;
            let cell = PATHFIND_CELL_SIZE_F_RESIDUAL;
            if dx * dx + dz * dz < cell * cell * 0.25 {
                let busy = matches!(
                    self.ai_state(),
                    super::AIState::Constructing
                        | super::AIState::Repairing
                        | super::AIState::SpecialAbility
                        | super::AIState::Capturing
                );
                if busy || self.status.using_ability {
                    return false;
                }
                if self.ai_state() == super::AIState::Idle
                    || other.ai_state() == super::AIState::Idle
                {
                    self.unstack_partner = Some(other.id);
                }
            }
        }
        false
    }

    /// Apply cur_max_blocked_speed cap residual (2D XZ).
    pub fn apply_blocked_speed_cap(&mut self) {
        if !self.is_blocked || !self.cur_max_blocked_speed.is_finite() {
            return;
        }
        let v = self.movement.velocity;
        let speed_2d = (v.x * v.x + v.z * v.z).sqrt();
        if speed_2d > self.cur_max_blocked_speed && speed_2d > 1.0e-4 {
            let s = self.cur_max_blocked_speed / speed_2d;
            self.movement.velocity.x *= s;
            self.movement.velocity.z *= s;
        }
    }

    /// C++ PhysicsBehavior::getMass residual: hull + contained riders.
    pub fn physics_get_mass(&self) -> f32 {
        (self.physics_mass + self.contained_items_mass).max(1.0e-4)
    }

    /// C++ PhysicsBehavior::isMotive residual.
    pub fn is_motive(&self) -> bool {
        self.motive_frames_remaining > 0
    }

    /// C++ PhysicsBehavior::applyForce residual.
    ///
    /// When motive, only lateral component (perp to unit facing) is accepted.
    /// Host XZ ground plane maps C++ XY; world Y is vertical.
    pub fn apply_physics_force(&mut self, force: glam::Vec3) {
        if !force.x.is_finite() || !force.y.is_finite() || !force.z.is_finite() {
            return;
        }
        let mut mod_force = force;
        if self.is_motive() {
            let dir = self.unit_direction_vector_2d(); // (x,z)
            // C++ lateralDot = force.x * (-dir.y) + force.y * dir.x
            // Host: force.x * (-dir.z_comp) + force.z * dir.x where dir=(x,z)
            let lateral_dot = force.x * (-dir.y) + force.z * dir.x;
            mod_force.x = lateral_dot * (-dir.y);
            mod_force.z = lateral_dot * dir.x;
            // vertical unchanged
        }
        let inv = 1.0 / self.physics_get_mass();
        self.physics_accel += mod_force * inv;
    }

    /// C++ rotateObjAroundLocoPivot / rotateTowardsPosition residual.
    pub fn rotate_towards_position(
        &mut self,
        goal: glam::Vec3,
        dt: f32,
    ) -> (PhysicsTurningType, f32) {
        self.rotate_towards_position_with_effects(goal, dt, 0, &mut Default::default())
    }
    pub(crate) fn rotate_towards_position_with_effects(
        &mut self,
        goal: glam::Vec3,
        dt: f32,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (PhysicsTurningType, f32) {
        self.rotate_obj_around_loco_pivot_with_effects(
            goal,
            self.maximum_towards_turn(dt),
            frame,
            effects,
        )
    }
    fn maximum_towards_turn(&self, dt: f32) -> f32 {
        let mut max_turn = self.effective_turn_rate() * dt;
        if matches!(
            self.loco_appearance,
            LocomotorAppearance::WheelsFour | LocomotorAppearance::Motorcycle
        ) {
            max_turn *= self.wheeled_turn_factor();
        }
        max_turn
    }

    /// C++ `Locomotor::setUltraAccurate` + `setPhysicsOptions`.
    pub fn set_ultra_accurate(&mut self, ultra: bool) {
        if self.ultra_accurate == ultra {
            return;
        }
        self.ultra_accurate = ultra;
        self.set_locomotor_physics_options();
    }

    /// C++ `Locomotor::setUsePreciseZPos` (PRECISE_Z_POS).
    pub fn set_precise_z_pos(&mut self, precise: bool) {
        if self.precise_z_pos == precise {
            return;
        }
        self.precise_z_pos = precise;
    }

    /// C++ `Locomotor::setAllowInvalidPosition` (ALLOW_INVALID_POSITION).
    pub fn set_allow_invalid_position(&mut self, allow: bool) {
        if self.allow_invalid_position == allow {
            return;
        }
        self.allow_invalid_position = allow;
    }

    /// C++ JetTaxi / JetTakeoffOrLanding / HeliTakeoffOrLanding /
    /// ChinookTakeoffOrLanding pair: `setUsePreciseZPos` + `setUltraAccurate`.
    pub fn set_precise_z_and_ultra_accurate(&mut self, enable: bool) {
        self.set_precise_z_pos(enable);
        self.set_ultra_accurate(enable);
    }

    /// C++ `moveTowardsPositionWheels` turnFactor = |actualSpeed|/minTurnSpeed.
    pub fn wheeled_turn_factor(&self) -> f32 {
        let mut turn_speed = self.min_turn_speed;
        let max_speed = self.effective_max_speed();
        if turn_speed < max_speed / 4.0 {
            turn_speed = max_speed / 4.0;
        }
        if turn_speed > 0.0 {
            (self.forward_speed_2d().abs() / turn_speed).min(1.0)
        } else {
            0.0
        }
    }

    /// C++ Locomotor::rotateObjAroundLocoPivot residual.
    pub fn rotate_obj_around_loco_pivot(
        &mut self,
        goal: glam::Vec3,
        max_turn_rate: f32,
    ) -> (PhysicsTurningType, f32) {
        self.rotate_obj_around_loco_pivot_with_effects(
            goal,
            max_turn_rate,
            0,
            &mut Default::default(),
        )
    }
    pub(crate) fn rotate_obj_around_loco_pivot_with_effects(
        &mut self,
        goal: glam::Vec3,
        max_turn_rate: f32,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (PhysicsTurningType, f32) {
        let plan = self.plan_loco_pivot(goal, max_turn_rate);
        if let Some(transform) = plan.transform {
            self.set_transform_matrix_with_effects(transform, frame, effects);
        }
        if let Some(angle) = plan.orientation {
            self.set_orientation_with_effects(angle, effects);
        }
        self.physics_turning = plan.turning;
        (plan.turning, plan.relative)
    }
    fn plan_loco_pivot(&self, goal: glam::Vec3, max_turn_rate: f32) -> PivotTurn {
        let angle = self.get_orientation();
        let mut offset = self.turn_pivot_offset;
        if self.is_braking {
            offset = 0.0;
        }
        let us = self.get_position();
        let (dx, dz, turn_pos) = if offset != 0.0 {
            let g = self.geometry_info();
            let radius = if g.authored {
                g.bounding_circle_radius()
            } else {
                self.selection_radius.max(1.0)
            };
            let turn_point = offset * radius;
            let dir = self.unit_direction_vector_2d();
            let turn_pos =
                glam::Vec3::new(us.x + dir.x * turn_point, us.y, us.z + dir.y * turn_point);
            let dx = goal.x - turn_pos.x;
            let dz = goal.z - turn_pos.z;
            if dx.abs() < 0.1 && dz.abs() < 0.1 {
                return PivotTurn::unchanged();
            }
            (dx, dz, Some(turn_pos))
        } else {
            let dx = goal.x - us.x;
            let dz = goal.z - us.z;
            (dx, dz, None)
        };
        let desired = (-dz).atan2(dx);
        let mut amount = desired - angle;
        while amount > std::f32::consts::PI {
            amount -= std::f32::consts::TAU;
        }
        while amount < -std::f32::consts::PI {
            amount += std::f32::consts::TAU;
        }
        let rel = amount;
        let (amount, turning) = if amount > max_turn_rate {
            (max_turn_rate, PhysicsTurningType::TurnPositive)
        } else if amount < -max_turn_rate {
            (-max_turn_rate, PhysicsTurningType::TurnNegative)
        } else {
            (amount, PhysicsTurningType::TurnNone)
        };
        let (transform, orientation) = if let Some(tp) = turn_pos {
            // Preserve the existing validated Main Y-up handedness. C++
            // T(pivot)*Rz(amount)*T(-pivot)*oldMatrix maps to this Ry product.
            (
                Some(
                    glam::Mat4::from_translation(tp)
                        * glam::Mat4::from_rotation_y(amount)
                        * glam::Mat4::from_translation(-tp)
                        * self.get_transform_matrix(),
                ),
                None,
            )
        } else {
            (None, Some(angle + amount))
        };
        PivotTurn {
            transform,
            orientation,
            turning,
            relative: rel,
        }
    }

    /// Advance wander angle offset residual (legs).
    pub fn tick_wander_angle_offset(&mut self, actual_speed: f32) -> f32 {
        if self.wander_width_factor == 0.0 {
            return 0.0;
        }
        if self.wander_offset_increment == 0.0 {
            self.wander_offset_increment = std::f32::consts::PI / 40.0;
        }
        let angle_limit = std::f32::consts::PI / 8.0 * self.wander_width_factor;
        if self.wander_offset_increasing {
            self.wander_angle_offset += self.wander_offset_increment * actual_speed;
            if self.wander_angle_offset > angle_limit {
                self.wander_offset_increasing = false;
            }
        } else {
            self.wander_angle_offset -= self.wander_offset_increment * actual_speed;
            if self.wander_angle_offset < -angle_limit {
                self.wander_offset_increasing = true;
            }
        }
        self.wander_angle_offset
    }

    /// C++ `Locomotor::handleBehaviorZ` (`Locomotor.cpp:2196-2323`).
    ///
    /// `ground_y` is surface height at object XZ (water when underwater).
    /// Returns true if needs constant calling.
    /// Explicit component surface input. Live controllers use the receiving
    /// World observation/continuation below; this method never selects terrain.
    pub fn handle_behavior_z(&mut self, surface_y: f32, goal_y: Option<f32>) -> bool {
        self.handle_behavior_z_with_highest_surface(surface_y, goal_y, surface_y)
    }
    pub(crate) fn handle_behavior_z_with_highest_surface(
        &mut self,
        surface_y: f32,
        goal_y: Option<f32>,
        highest_surface: f32,
    ) -> bool {
        self.handle_behavior_z_with_highest_surface_and_effects(
            surface_y,
            goal_y,
            highest_surface,
            0,
            &mut Default::default(),
        )
    }
    pub(crate) fn handle_behavior_z_with_highest_surface_and_effects(
        &mut self,
        surface_y: f32,
        goal_y: Option<f32>,
        highest_surface: f32,
        frame: u32,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> bool {
        let step = self.prepare_behavior_z(surface_y, goal_y, highest_surface);
        if let Some(position) = step.position {
            self.set_position_with_effects(position, frame, effects)
        }
        step.requires_constant_calling
    }
    /// Original Locomotor::handleBehaviorZ. Kinematic branches return one
    /// owned position continuation; motive branches only admit source force.
    pub(in crate::game_logic) fn prepare_behavior_z(
        &mut self,
        surface_y: f32,
        goal_y: Option<f32>,
        highest_surface: f32,
    ) -> ZBehaviorStep {
        let mut position = None;
        let requires_constant_calling = match self.loco_behavior_z {
            LocomotorBehaviorZ::NoZMotiveForce => false,
            LocomotorBehaviorZ::SeaLevel => {
                if !self.is_held_disabled() {
                    let mut p = self.get_position();
                    p.y = surface_y;
                    position = Some(p)
                }
                true
            }
            LocomotorBehaviorZ::FixedSurfaceRelativeHeight
            | LocomotorBehaviorZ::RelativeToGroundAndBuildings => {
                let mut p = self.get_position();
                p.y = self.loco_preferred_height + surface_y;
                position = Some(p);
                true
            }
            LocomotorBehaviorZ::FixedAbsoluteHeight => {
                let mut p = self.get_position();
                p.y = self.loco_preferred_height;
                position = Some(p);
                true
            }
            LocomotorBehaviorZ::SurfaceRelativeHeight => {
                self.apply_surface_relative_lift(surface_y, goal_y)
            }
            LocomotorBehaviorZ::SmoothRelativeToHighestLayer => {
                self.apply_surface_relative_lift(highest_surface, goal_y)
            }
            LocomotorBehaviorZ::AbsoluteHeight => self.apply_surface_relative_lift(0., goal_y),
        };
        ZBehaviorStep {
            requires_constant_calling,
            position,
        }
    }

    /// C++ `Z_SURFACE_RELATIVE_HEIGHT` / `Z_SMOOTH_RELATIVE_TO_HIGHEST_LAYER`
    /// lift residual (`Locomotor.cpp:2288-2317` / `:2248-2285`).
    fn apply_surface_relative_lift(&mut self, surface_y: f32, goal_y: Option<f32>) -> bool {
        // A source AI with no current locomotor has no motive controller.
        // Physics still admits gravity through its ordinary receiving owner.
        if self.selected_locomotor_definition.is_none() {
            return false;
        }

        if self.loco_preferred_height == 0.0 && !self.precise_z_pos {
            return true;
        }
        let p = self.get_position();
        // C++ Locomotor.cpp:2296-2300 / :2265-2267 — goal Z only when PRECISE_Z_POS.
        let mut preferred_raw = self.loco_preferred_height + surface_y;
        if self.precise_z_pos {
            if let Some(gy) = goal_y {
                preferred_raw = gy;
            }
        }
        let mut delta = preferred_raw - p.y;
        delta *= self.loco_preferred_height_damping;
        let preferred = p.y + delta;
        let lift = self.calc_lift_to_use_at_pt(p.y, preferred);
        if lift != 0.0 {
            let force_y = lift
                * self.physics_get_mass()
                * if self.loco_appearance == LocomotorAppearance::Thrust {
                    30.0
                } else {
                    1.0
                };
            self.apply_motive_force(glam::Vec3::new(0.0, force_y, 0.0));
        }
        true
    }

    /// Appearance half of C++ maintainCurrentPosition. The driving locomotor
    /// applies altitude once, after this operation, using its current goal.
    pub(crate) fn loco_maintain_appearance(&mut self, dt: f32) -> bool {
        self.loco_maintain_appearance_with_effects(
            dt,
            0,
            self.ground_height,
            &mut Default::default(),
        )
    }
    pub(in crate::game_logic) fn begin_loco_maintain_appearance(&mut self) {
        if !self.maintain_pos_valid {
            self.maintain_pos = Some(self.get_position());

            self.maintain_pos_valid = true;
        }
        // C++ Locomotor.cpp:2420-2421 — reset donut window and clear IS_BRAKING.
        self.start_move();
        self.is_braking = false;
        self.physics_turning = PhysicsTurningType::TurnNone;
    }
    pub(crate) fn loco_maintain_appearance_with_effects(
        &mut self,
        dt: f32,
        frame: u32,
        surface_y: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        self.begin_loco_maintain_appearance();

        // Appearance-specific maintain residual.
        match self.loco_appearance {
            LocomotorAppearance::Wings => {
                self.motive_frames_remaining = MOTIVE_FRAMES_RESIDUAL.max(1);
                // 2D circle only. C++ handleBehaviorZ runs after with
                // getSurfaceHtAtPt (hq-xlays) — not own altitude.
                self.maintain_position_wings_with_effects(dt.max(1.0 / 30.0), frame, effects);
                return true;
            }
            LocomotorAppearance::Thrust => {
                if let Some(m) = self.maintain_pos {
                    let spd = self.min_speed;
                    self.move_towards_thrust_with_status_and_effects(
                        m,
                        0.0,
                        spd,
                        dt,
                        frame,
                        self.is_braking,
                        surface_y,
                        effects,
                    );
                }
                return true;
            }
            LocomotorAppearance::Hover => {
                self.physics_turning = PhysicsTurningType::TurnNone;
                if self.is_motive() {
                    // C++ maintainCurrentPositionHover (Locomotor.cpp:2527-2576):
                    // motive force along heading toward minSpeed. No vel scrub.
                    self.apply_hover_maintain_brake();
                }
                return true;
            }
            _ => {}
        }

        // Ground-appearance residual: scrub horizontal velocity (legs/treads/wheels).
        let airborne_loco = self.is_kind_of(crate::game_logic::KindOf::Aircraft)
            || matches!(
                self.loco_behavior_z,
                LocomotorBehaviorZ::SurfaceRelativeHeight
                    | LocomotorBehaviorZ::SmoothRelativeToHighestLayer
                    | LocomotorBehaviorZ::AbsoluteHeight
                    | LocomotorBehaviorZ::FixedSurfaceRelativeHeight
                    | LocomotorBehaviorZ::FixedAbsoluteHeight
                    | LocomotorBehaviorZ::RelativeToGroundAndBuildings
            );
        if !airborne_loco {
            self.scrub_velocity_2d(0.0);
        }

        airborne_loco
    }

    /// C++ Locomotor.cpp:2420-2474: appearance, then altitude at maintainPos.
    pub fn loco_maintain_current_position(&mut self, ground_y: f32, dt: f32) -> bool {
        let appearance_needs_update = self.loco_maintain_appearance(dt);
        let maintain_y = self.maintain_pos.map(|p| p.y);
        let needs_z = self.handle_behavior_z(ground_y, maintain_y);
        appearance_needs_update || needs_z
    }

    /// C++ `AIUpdateInterface::chooseGoodLocomotorFromCurrentSet` (AIUpdate.cpp:833-872).
    pub fn choose_good_locomotor_from_current_set(
        &mut self,
        cell_type: gamelogic::ai::pathfind_astar::PathfindCellType,
    ) {
        if self.locomotor_set_names.iter().all(|name| name.is_empty())
            && self.thing.template.authored_locomotor_sets.is_none()
        {
            let fallback = crate::game_logic::locomotor_bootstrap::locomotor_set_names_for_unit(
                &self.thing.template.name,
            );
            if fallback.iter().any(|name| !name.is_empty()) {
                self.locomotor_set_names = fallback;
            }
        }
        let acceptable =
            crate::game_logic::locomotor_bootstrap::valid_locomotor_surfaces_for_cell_type(
                cell_type,
            );
        let chosen =
            crate::game_logic::locomotor_bootstrap::choose_best_locomotor_name_for_surfaces(
                &self.locomotor_set_names,
                acceptable,
            )
            .or_else(|| {
                self.cur_locomotor_name
                    .clone()
                    .filter(|name| !name.is_empty())
            })
            .or_else(|| {
                crate::game_logic::locomotor_bootstrap::choose_best_locomotor_name_for_surfaces(
                    &self.locomotor_set_names,
                    crate::game_logic::object::LOCO_SURFACE_GROUND,
                )
            });
        let Some(name) = chosen else {
            return;
        };
        if self
            .cur_locomotor_name
            .as_deref()
            .is_some_and(|cur| cur.eq_ignore_ascii_case(&name))
        {
            return;
        }
        let Some(binding) =
            crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding(&name)
        else {
            return;
        };
        crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(self, &binding);
        self.cur_locomotor_name = Some(name);
        self.precise_z_pos = false;
        self.no_slow_down_as_approaching_dest = false;
        self.ultra_accurate = false;
        // C++ setUltraAccurate(FALSE) only clears the flag (Locomotor.h:333).
        // doLocomotor then setPhysicsOptions (AIUpdate.cpp:2132) and must not
        // keep the +0.5 that apply wrote while the old flag was still true.
        self.set_locomotor_physics_options();
    }

    /// C++ `Locomotor::moveTowardsPositionHover` OVER_WATER (Locomotor.cpp:1868-1886).
    pub fn apply_hover_over_water(&mut self, underwater: bool) {
        if !matches!(self.loco_appearance, LocomotorAppearance::Hover) {
            if self.over_water {
                self.over_water = false;
                self.refresh_model_condition_bits();
            }
            return;
        }
        if self.over_water == underwater {
            return;
        }
        self.over_water = underwater;
        self.refresh_model_condition_bits();
    }

    /// C++ `AIUpdateInterface::needToRotate` (AIUpdate.cpp:1380-1403).
    pub fn need_to_rotate(&self) -> bool {
        if self.waiting_for_path {
            return true;
        }
        if self.wander_width_factor > 0.0 {
            return false;
        }
        let Some(tgt) = self.movement.target_position else {
            return false;
        };
        let us = self.get_position();
        let dx = tgt.x - us.x;
        let dz = tgt.z - us.z;
        if dx * dx + dz * dz < 1.0e-6 {
            return false;
        }
        let desired = (-dz).atan2(dx);
        let mut delta = desired - self.get_orientation();
        while delta > std::f32::consts::PI {
            delta -= std::f32::consts::TAU;
        }
        while delta < -std::f32::consts::PI {
            delta += std::f32::consts::TAU;
        }
        delta.abs() > std::f32::consts::PI / 30.0
    }

    /// C++ doLocomotor blocked-frame bookkeeping (AIUpdate.cpp:2116-2127).
    pub fn tick_do_locomotor_blocked_frames(&mut self) {
        // C++ doLocomotor (AIUpdate.cpp:2116-2125) only increments or zeroes.
        // The rotate reset to 1 lives in the collision scan, not here.
        if self.is_blocked {
            self.num_frames_blocked = self.num_frames_blocked.saturating_add(1);
        } else {
            self.num_frames_blocked = 0;
        }
        self.is_blocked = false;
    }

    /// C++ doLocomotor desiredSpeed then bumpSpeedLimit cap
    /// (`AIUpdate.cpp:2144-2217`). Formation/team/column stamp
    /// `group_speed_factor` from AIGroup::getSpeed / FAST_AS_POSSIBLE.
    pub fn apply_do_locomotor_blocked_speed(&mut self, mut speed: f32) -> f32 {
        speed *= self.group_speed_factor.clamp(0.0, 1.0);
        let blocked = self.num_frames_blocked > 0;
        if blocked && speed > self.cur_max_blocked_speed {
            speed = self.cur_max_blocked_speed;
            if self.bump_speed_limit > speed {
                self.bump_speed_limit = speed;
            }
            self.bump_speed_limit *= 0.95;
            speed = self.bump_speed_limit;
        } else {
            // C++ FAST_AS_POSSIBLE (999999), not f32::MAX.
            if self.bump_speed_limit < 999_999.0 {
                if self.bump_speed_limit < speed * 0.2 {
                    self.bump_speed_limit = speed * 0.2;
                }
                self.bump_speed_limit *= 1.05;
            }
            if speed > self.bump_speed_limit {
                speed = self.bump_speed_limit;
            }
        }
        speed.max(0.0)
    }

    /// Wings/thrust hold the last waypoint by circling instead of `stop_moving`.
    pub fn holds_air_position_when_idle(&self) -> bool {
        matches!(
            self.loco_appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        )
    }

    /// C++ Locomotor::setPhysicsOptions residual.
    pub fn set_locomotor_physics_options(&mut self) {
        // C++ EXTRA_FRIC 0.5 when ULTRA_ACCURATE. Last-writer vs OCL/SlowDeath:
        // loco update overwrites extraFriction; debris/disabled skip this path
        // so authored ExtraFriction sticks (C++ those objects have no loco tick).
        if self.can_move()
            && self
                .slow_death
                .as_ref()
                .map(|s| !s.is_active())
                .unwrap_or(true)
        {
            let ultra = if self.ultra_accurate { 0.5 } else { 0.0 };
            self.extra_friction = self.loco_extra_2d_friction + ultra;
        }
        self.apply_friction_2d_when_airborne = self.loco_apply_2d_friction_airborne;
        // C++ setStickToGround(getStickToGround()). The binding already wrote
        // the template flag. Do not force every infantry unit on.
        if self.is_kind_of(crate::game_logic::KindOf::Infantry) {
            if matches!(self.loco_appearance, LocomotorAppearance::Other) {
                self.loco_appearance = LocomotorAppearance::LegsTwo;
            }
        } else if self.is_kind_of(crate::game_logic::KindOf::Aircraft) {
            if matches!(self.loco_appearance, LocomotorAppearance::Other) {
                self.loco_appearance = LocomotorAppearance::Wings;
            }
        } else if self.is_kind_of(crate::game_logic::KindOf::Vehicle) {
            if matches!(self.loco_appearance, LocomotorAppearance::Other) {
                // Fail-closed: vehicles default treads-like (tanks common in host).
                self.loco_appearance = LocomotorAppearance::Treads;
            }
        }
    }

    /// C++ PhysicsBehavior::setExtraFriction (OCL / SlowDeath).
    pub fn set_extra_friction(&mut self, friction: f32) {
        self.extra_friction = friction;
    }

    /// C++ PhysicsBehavior::setExtraBounciness (OCL CreateDebris / SlowDeath).
    pub fn set_extra_bounciness(&mut self, bounciness: f32) {
        self.extra_bounciness = bounciness;
    }

    /// C++ PhysicsBehavior::setBounceSound.
    pub fn set_bounce_sound(&mut self, name: impl Into<String>) {
        self.bounce_sound_name = name.into();
    }

    /// C++ Object::isAboveTerrain — height above ground > 0.
    pub fn is_above_terrain(&self) -> bool {
        self.get_position().y > self.ground_height
    }

    /// C++ `GeometryInfo::getBoundingSphereRadius`, or pick radius when unauthored.
    pub fn physics_collide_sphere_radius(&self) -> f32 {
        let g = self.geometry_info();
        if g.authored {
            g.bounding_sphere_radius()
        } else {
            self.selection_radius.max(1.0)
        }
    }

    /// C++ `GeometryInfo::getBoundingCircleRadius`, or pick radius when unauthored.
    pub fn physics_collide_circle_radius(&self) -> f32 {
        let g = self.geometry_info();
        if g.authored {
            g.bounding_circle_radius()
        } else {
            self.selection_radius.max(1.0)
        }
    }

    /// C++ onCollide radius: 3D sphere if above terrain, else 2D circle.
    pub fn physics_on_collide_radius(&self) -> f32 {
        if self.is_above_terrain() {
            self.physics_collide_sphere_radius()
        } else {
            self.physics_collide_circle_radius()
        }
    }

    /// C++ mobile collide force: -min(overlap, 5) * delta/dist via applyForce.
    /// Airborne (`isAboveTerrain`) uses full 3D delta including host Y.
    pub fn apply_overlap_collide_force(&mut self, other_center: glam::Vec3, overlap: f32) {
        if !self.allow_collide_force {
            return;
        }
        let g = self.geometry_info();
        let lift = match g.geom_type {
            crate::game_logic::thing::HostGeometryType::Sphere => 0.0,
            crate::game_logic::thing::HostGeometryType::Box
            | crate::game_logic::thing::HostGeometryType::Cylinder => g.height * 0.5,
        };
        let us = self.get_position();
        let mut dx = other_center.x - us.x;
        let mut dy = other_center.y - (us.y + lift);
        let mut dz = other_center.z - us.z;
        if !self.is_above_terrain() {
            dy = 0.0;
        }
        let mut dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist < 1.0 {
            dist = 1.0;
        }
        let overlap = overlap.min(5.0);
        let factor = -overlap;
        self.apply_physics_force(glam::Vec3::new(
            factor * dx / dist,
            factor * dy / dist,
            factor * dz / dist,
        ));
    }

    /// C++ Locomotor::getMaxLift residual (host world-Y).
    /// C++ Locomotor::getMaxLift residual (damage-conditioned).
    pub fn get_max_lift(&self) -> f32 {
        self.effective_max_lift()
    }

    /// C++ Locomotor::getMaxLift(BodyDamageType) residual.
    pub fn effective_max_lift(&self) -> f32 {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let definition = self
            .selected_locomotor_definition
            .as_ref()
            .expect("lift controller requires its admitted immutable locomotor definition");
        // Source Locomotor.cpp831: select the damage-conditioned template
        // value, including explicit zero, then apply the mutable runtime cap.
        let lift = match self.body_damage_state {
            HostBodyDamageType::Pristine | HostBodyDamageType::Damaged => definition.max_lift,
            HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble => {
                definition.max_lift_damaged
            }
        };
        lift.min(self.max_lift)
    }

    /// C++ Locomotor::calcLiftToUseAtPt residual (simplified).
    ///
    /// Gravity is leftover `TheGlobalData->m_gravity` (host world-Y).
    /// Returns lift accel to apply (not force).
    pub fn calc_lift_to_use_at_pt(&self, cur_y: f32, preferred_height: f32) -> f32 {
        let gravity = self.physics_gravity();
        let max_gross = self.get_max_lift();
        let mut max_net = max_gross + gravity;
        if max_net < 0.0 {
            max_net = 0.0;
        }
        let cur_vy = if self.loco_appearance == LocomotorAppearance::Thrust {
            self.movement.velocity.y / 30.0
        } else {
            self.movement.velocity.y
        };
        let max_accel = if self.ultra_accurate {
            if cur_vy < 0.0 {
                2.0 * max_net
            } else {
                -2.0 * max_net
            }
        } else if cur_vy < 0.0 {
            max_net
        } else {
            gravity
        };
        let desired_accel = if max_accel.abs() > 0.001 {
            let delta_y = preferred_height - cur_y;
            let brake_dist = (cur_vy * cur_vy) / max_accel.abs().max(1e-6);
            if brake_dist.abs() > delta_y.abs() {
                max_accel
            } else if cur_vy.abs() > self.speed_limit_z {
                self.speed_limit_z - cur_vy
            } else {
                // a = 2(dz - v) assuming t=1 frame
                2.0 * (delta_y - cur_vy)
            }
        } else {
            0.0
        };
        let mut lift = desired_accel - gravity;
        if self.ultra_accurate {
            const UP_FACTOR: f32 = 3.0;
            if lift > UP_FACTOR * max_gross {
                lift = UP_FACTOR * max_gross;
            } else if lift < -max_gross {
                lift = -max_gross;
            }
        } else if lift > max_gross {
            lift = max_gross;
        } else if lift < 0.0 {
            lift = 0.0;
        }
        lift
    }

    /// C++ AIUpdateInterface::requestAttackPath flag residual (before pathfinder).
    pub fn begin_request_attack_path(
        &mut self,
        victim_id: Option<ObjectId>,
        victim_pos: glam::Vec3,
        current_frame: u32,
    ) -> bool {
        // Returns false if should defer (repath too soon).
        self.requested_destination = Some(victim_pos);

        self.requested_victim_id = victim_id;

        self.is_attack_path = true;
        self.is_approach_path = false;
        self.is_safe_path = false;
        self.waiting_for_path = true;
        if self.path_timestamp > 0 && current_frame.saturating_sub(self.path_timestamp) < 3 {
            // C++ setQueueForPathTime(2 * LOGICFRAMES_PER_SECOND) and setLocomotorGoalNone.
            self.queue_for_path_frames = 2
                * crate::game_logic::host_ai_path_combat_residual_wave105::LOGIC_FRAMES_PER_SECOND_RESIDUAL;
            self.set_locomotor_goal_none();
            return false;
        }

        true
    }

    /// C++ AIUpdateInterface::requestPath flag residual (non-attack).
    pub fn begin_request_move_path(&mut self, destination: glam::Vec3, current_frame: u32) -> bool {
        let ok = self.begin_request_move_path_inner(destination, current_frame, true);
        if !ok && !self.movement.path.is_empty() && self.is_blocked_and_stuck {
            self.ignore_collisions_until_frame = current_frame.saturating_add(
                2 * crate::game_logic::host_ai_path_combat_residual_wave105::LOGIC_FRAMES_PER_SECOND_RESIDUAL,
            );
            self.num_frames_blocked = 0;
            self.is_blocked = false;
            self.is_blocked_and_stuck = false;
        }
        ok
    }

    fn begin_request_move_path_inner(
        &mut self,
        destination: glam::Vec3,
        current_frame: u32,
        allow_quick: bool,
    ) -> bool {
        self.requested_destination = Some(destination);

        self.requested_victim_id = None;

        self.is_attack_path = false;
        self.is_approach_path = false;
        self.is_safe_path = false;
        self.is_final_goal = true;
        if allow_quick && self.try_non_aircraft_quick_path(destination, current_frame) {
            return true;
        }
        self.waiting_for_path = true;
        if self.path_timestamp > 0 && current_frame.saturating_sub(self.path_timestamp) < 3 {
            self.queue_for_path_frames = crate::game_logic::host_ai_path_combat_residual_wave105::LOGIC_FRAMES_PER_SECOND_RESIDUAL;
            return false;
        }

        true
    }

    /// Flying non-aircraft `computeQuickPath`. Aircraft need `getAircraftPath`
    /// on the world pathfinder, so they return false and stay queued.
    fn try_non_aircraft_quick_path(&mut self, destination: glam::Vec3, current_frame: u32) -> bool {
        let air_surface =
            (self.locomotor_surfaces & crate::game_logic::object::LOCO_SURFACE_AIR) != 0;
        if !air_surface || crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(self) {
            return false;
        }
        if let Some(last) = self.movement.path.last() {
            if (*last - destination).length_squared() < 0.25 {
                return true;
            }
        }
        if self.is_kind_of(crate::game_logic::KindOf::Aircraft)
            && !self.is_kind_of(crate::game_logic::KindOf::Projectile)
        {
            return false;
        }
        let mut start = self.get_position();
        start.y = destination.y;
        self.is_attack_path = false;
        self.is_exact_path = false;
        self.can_path_through_units = false;
        self.set_locomotor_goal_position_on_path();
        self.movement.path = vec![start, destination];
        self.movement.current_path_index = 1;
        self.movement.target_position = Some(destination);
        self.refresh_follow_path_extra_distance();
        self.path_timestamp = current_frame;
        self.num_frames_blocked = 0;
        self.is_blocked_and_stuck = false;
        self.waiting_for_path = false;
        self.set_status_moving(true);
        true
    }

    /// C++ requestApproachPath residual.
    pub fn begin_request_approach_path(
        &mut self,
        destination: glam::Vec3,
        current_frame: u32,
    ) -> bool {
        let ok = self.begin_request_move_path_inner(destination, current_frame, false);
        self.is_approach_path = true;
        self.is_final_goal = true;
        if !ok {
            self.queue_for_path_frames = 2
                * crate::game_logic::host_ai_path_combat_residual_wave105::LOGIC_FRAMES_PER_SECOND_RESIDUAL;
        }
        ok
    }

    /// C++ requestSafePath residual (`AIUpdate.cpp:549-560`).
    pub fn begin_request_safe_path(
        &mut self,
        repulsor: ObjectId,
        flee_pos: glam::Vec3,
        current_frame: u32,
    ) -> bool {
        self.set_adjust_destinations(false);
        let previous = self.requested_victim_id;
        let ok = self.begin_request_move_path_inner(flee_pos, current_frame, false);
        self.is_safe_path = true;
        self.is_final_goal = false;
        // C++ AIMoveAwayFromRepulsorsState keeps adjustsDestination false
        // (AIStates.cpp:2265 and :2294). It never restores true.
        self.set_adjust_destinations(false);
        if !ok {
            self.queue_for_path_frames = 2
                * crate::game_logic::host_ai_path_combat_residual_wave105::LOGIC_FRAMES_PER_SECOND_RESIDUAL;
        }
        if previous != Some(repulsor) {
            self.safe_path_repulsor2 = previous;
        }
        self.requested_victim_id = Some(repulsor);

        ok
    }

    /// Returns true the frame the delay hits zero while a path is still waiting.
    pub fn tick_path_queue(
        &mut self,
        current_frame: u32,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> bool {
        let expired = if self.queue_for_path_frames == 0 {
            false
        } else {
            self.queue_for_path_frames -= 1;
            if self.queue_for_path_frames > 0 {
                self.waiting_for_path = true;
            }
            self.queue_for_path_frames == 0
                && self.waiting_for_path
                && self.requested_destination.is_some()
        };
        if self.has_temporary_ai_move()
            && !matches!(
                self.transport_return_state,
                Some(
                    crate::game_logic::host_transport_return::TransportReturnState::TemporaryMove { .. }
                )
            )
        {
            let arrived = self.movement.path.is_empty()
                && self.movement.target_position.is_none()
                && !self.waiting_for_path;
            let expired = self
                .unit_ai_runtime
                .temporary_move()
                .is_some_and(|value| value.deadline < current_frame);
            if arrived || expired {
                self.end_temporary_move_overlay(audio);
            }
        }
        expired
    }

    /// C++ privateAttackObject max-shots residual.
    /// C++ Locomotor::getMaxSpeedForCondition residual.
    /// Better than MovementPenaltyDamageState (REALLYDAMAGED) → pristine max;
    /// else → max_speed_damaged (clamped by pristine max).
    pub fn effective_max_speed(&self) -> f32 {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let pristine = self.movement.max_speed.max(0.0);
        let damaged = self
            .movement
            .max_speed_damaged
            .clamp(0.0, pristine.max(0.0));
        // Penalty threshold = ReallyDamaged (GameData.ini residual).
        match self.body_damage_state {
            HostBodyDamageType::Pristine | HostBodyDamageType::Damaged => pristine,
            HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble => {
                if damaged > 0.0 {
                    damaged.min(pristine)
                } else {
                    pristine * 0.5
                }
            }
        }
    }

    /// C++ Locomotor::getMaxTurnRate residual (damage-conditioned).
    pub fn effective_turn_rate(&self) -> f32 {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let pristine = self.movement.turn_rate.max(0.0);
        let damaged = self
            .movement
            .turn_rate_damaged
            .clamp(0.0, pristine.max(0.0));
        let turn = match self.body_damage_state {
            HostBodyDamageType::Pristine | HostBodyDamageType::Damaged => pristine,
            HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble => {
                if damaged > 0.0 {
                    damaged.min(pristine)
                } else {
                    pristine * 0.5
                }
            }
        };
        // C++ Locomotor.cpp:796-798 ULTRA_ACCURATE monster turning.
        if self.ultra_accurate {
            turn * 2.0
        } else {
            turn
        }
    }

    /// C++ Locomotor::getMaxAcceleration residual (damage-conditioned).
    pub fn effective_acceleration(&self) -> f32 {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let pristine = self.movement.acceleration.max(0.0);
        let damaged = self
            .movement
            .acceleration_damaged
            .clamp(0.0, pristine.max(0.0));
        match self.body_damage_state {
            HostBodyDamageType::Pristine | HostBodyDamageType::Damaged => pristine,
            HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble => {
                if damaged > 0.0 {
                    damaged.min(pristine)
                } else {
                    pristine * 0.5
                }
            }
        }
    }

    pub fn set_max_shots_to_fire(&mut self, max_shots: i32) {
        self.max_shots_to_fire = max_shots;
    }

    /// C++ Weapon::getMaxShotCount residual: 0 means cannot fire more.
    /// Host uses -1 as unlimited (also accepts C++ NO_MAX_SHOTS_LIMIT).
    #[inline]
    pub fn has_max_shots_remaining(&self) -> bool {
        self.max_shots_to_fire != 0
    }

    /// C++ `--m_maxShotCount` residual after a successful discharge.
    pub fn consume_max_shot_count(&mut self) {
        const NO_MAX: i32 =
            crate::game_logic::host_ai_path_combat_residual_wave105::NO_MAX_SHOTS_LIMIT;
        if self.max_shots_to_fire == -1 || self.max_shots_to_fire == NO_MAX {
            return;
        }
        if self.max_shots_to_fire > 0 {
            self.max_shots_to_fire -= 1;
        }
    }

    /// C++ AIUpdateInterface::requestPath residual (fail-closed straight path).
    ///
    /// Sets waiting_for_path briefly, installs single-waypoint path to dest.
    /// Full Pathfinder A* is applied by GameLogic when grid is available.
    pub fn request_path(&mut self, destination: glam::Vec3, waypoints: Option<Vec<glam::Vec3>>) {
        self.waiting_for_path = true;
        self.queue_for_path_frames = 0;
        self.maintain_pos_valid = false;
        if let Some(mut wps) = waypoints {
            if wps.is_empty() {
                wps.push(destination);
            }
            self.movement.path = wps;
        } else {
            self.movement.path = vec![destination];
        }
        if self.movement.path.len() >= 2 {
            // C++ AIFollowPathState::onEnter starts at index zero (the
            // current-position node), while movement heads for the next node.
            // The host advances this index by projection on its first update.
            self.movement.current_path_index = 0;
            self.movement.target_position = Some(self.movement.path[1]);
        } else {
            self.movement.current_path_index = 0;
            self.movement.target_position = self.movement.path.first().copied();
        }
        self.is_attack_path = false;
        self.is_exact_path = false;
        self.set_locomotor_goal_position_on_path();
        self.refresh_follow_path_extra_distance();
        self.waiting_for_path = false;
        self.is_braking = false;
        self.start_move();
        self.num_frames_blocked = 0;
        self.is_blocked_and_stuck = false;
        self.set_status_moving(true);
    }

    /// C++ Locomotor::startMove (Locomotor.cpp:761-765) — reset donut timer only.
    pub fn start_move(&mut self) {
        self.donut_timer = u32::MAX;
    }

    /// True if effectively moving (C++ isMoving || isWaitingForPath).
    pub fn is_effectively_moving(&self) -> bool {
        self.waiting_for_path
            || self.movement.target_position.is_some()
            || self.movement.velocity.length_squared() > 0.01
    }

    /// C++ Locomotor::calcMinTurnRadius residual (host units).
    pub fn calc_min_turn_radius(&self) -> f32 {
        let min_speed = self.min_speed.max(0.0);
        // turn_rate is rad/sec; convert to per-frame for C++ parity radius.
        let max_turn_rate = self.movement.turn_rate / 30.0;
        if max_turn_rate > 1.0e-6 {
            // minSpeed is units/sec → per-frame for C++ formula minSpeed/maxTurnRate
            (min_speed / 30.0) / max_turn_rate
        } else {
            999_999.0
        }
    }

    /// C++ Locomotor::fixInvalidPosition residual.
    ///
    /// Fail-closed without full pathfinder neighbor scan: when
    /// `on_invalid_movement_terrain` or cliff cell, push toward valid via motive force.
    pub fn fix_invalid_position(&mut self) -> bool {
        if self.is_dozer || self.is_kind_of(crate::game_logic::KindOf::Aircraft) {
            return false;
        }
        if !self.on_invalid_movement_terrain && !self.cell_is_cliff {
            return false;
        }
        // Push opposite current lateral velocity if sinking into obstacle; else nudge
        // along facing residual (C++ 3×3 neighbor vote simplified).
        let mass = self.physics_get_mass();
        let v = self.movement.velocity;
        let speed2 = v.x * v.x + v.z * v.z;
        if speed2 > 0.01 {
            let inv = 1.0 / speed2.sqrt();
            let nx = -v.x * inv;
            let nz = -v.z * inv;
            // If already leaving (dot with correction > 0.25), skip.
            let leaving = v.x * nx + v.z * nz; // nx opposite vel so leaving is negative of progress
            // correction direction is opposite into-invalid → along -velocity when moving in
            if leaving > 0.25 {
                return false;
            }
            let force = glam::Vec3::new(nx * mass / 5.0, 0.0, nz * mass / 5.0);
            self.apply_motive_force(force);
            self.integrate_locomotor_horizontal_accel();
            return true;
        }
        // Stationary on invalid: nudge along facing.
        let d = self.unit_direction_vector_2d();
        let force = glam::Vec3::new(d.x * mass / 5.0, 0.0, d.y * mass / 5.0);
        self.apply_motive_force(force);
        self.integrate_locomotor_horizontal_accel();
        true
    }

    /// C++ maintainCurrentPositionWings residual — circle around maintain pos.
    pub fn maintain_position_wings(&mut self, dt: f32) {
        self.maintain_position_wings_with_effects(dt, 0, &mut Default::default())
    }
    pub(crate) fn maintain_position_wings_with_effects(
        &mut self,
        dt: f32,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.physics_turning = PhysicsTurningType::TurnNone;
        // C++ maintainCurrentPositionWings: isMotive() && isAboveTerrain().
        if !self.is_motive() || !self.is_above_terrain() {
            return;
        }
        let Some(maintain) = self.maintain_pos else {
            return;
        };
        let mut turn_radius = self.circling_radius;
        if turn_radius.abs() < 1.0e-4 {
            turn_radius = self.calc_min_turn_radius();
        }
        let us = self.get_position();
        let dx = maintain.x - us.x;
        let dz = maintain.z - us.z;
        let mut angle = if dx * dx + dz * dz < 1.0e-6 {
            self.get_orientation()
        } else {
            (-dz).atan2(dx) // host facing convention for direction to maintain
        };
        // C++ aimDir = PI - PI/8
        let mut aim_dir = std::f32::consts::PI - std::f32::consts::PI / 8.0;
        if turn_radius < 0.0 {
            turn_radius = -turn_radius;
            aim_dir = -aim_dir;
        }
        angle += aim_dir;
        let desired = glam::Vec3::new(
            maintain.x + angle.cos() * turn_radius,
            maintain.y,
            maintain.z + (-angle.sin()) * turn_radius, // match host dir xz from angle
        );
        // C++ moveTowardsPositionWings(..., m_template->m_minSpeed).
        let spd = self.min_speed.max(0.0);
        self.movement.target_position = Some(desired);
        let (_t, _rel) = self.rotate_towards_position_with_effects(desired, dt, frame, effects);
        self.apply_forward_speed_force(spd, dt);
        let mut p = self.get_position();
        p.x += self.movement.velocity.x * dt;
        p.z += self.movement.velocity.z * dt;
        self.set_position_with_effects(p, frame, effects);
        self.movement.target_position = None;
        // C++ maintainCurrentPositionWings is 2D circling only
        // (Locomotor.cpp:2488-2524). handleBehaviorZ uses terrain.
    }

    /// C++ `Locomotor::moveTowardsPositionThrust` (`Locomotor.cpp:1891-2004`).
    ///
    /// Leftover GameLogic `move_towards_position_thrust_physics` already has
    /// gravity-aware aim, MaxThrustAngle, and surface PreferredHeight; this
    /// live-host path ports those leftovers onto host Y-up + 3D motive force.
    pub fn move_towards_thrust(
        &mut self,
        goal: glam::Vec3,
        on_path_dist: f32,
        desired_speed: f32,
        dt: f32,
    ) {
        self.move_towards_thrust_with_effects(
            goal,
            on_path_dist,
            desired_speed,
            dt,
            0,
            &mut Default::default(),
        )
    }
    pub(crate) fn move_towards_thrust_with_effects(
        &mut self,
        goal: glam::Vec3,
        on_path_dist: f32,
        desired_speed: f32,
        _dt: f32,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.move_towards_thrust_with_status_and_effects(
            goal,
            on_path_dist,
            desired_speed,
            _dt,
            frame,
            self.is_braking,
            self.ground_height,
            effects,
        );
    }
    pub(crate) fn move_towards_thrust_with_status_and_effects(
        &mut self,
        goal: glam::Vec3,
        on_path_dist: f32,
        desired_speed: f32,
        _dt: f32,
        frame: u32,
        braking_status: bool,
        surface_y: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let step = self.prepare_thrust_step(
            goal,
            on_path_dist,
            desired_speed,
            frame,
            braking_status,
            surface_y,
            effects,
        );
        if let Some(reaction) = step.reaction {
            assert!(
                !reaction.has_tree_fx(),
                "registered Thrust orientation requires its receiving World"
            );
            self.finish_position_reaction(reaction);
        }
        self.finish_thrust_force(step.force);
    }
    pub(in crate::game_logic) fn prepare_thrust_step(
        &mut self,
        goal: glam::Vec3,
        on_path_dist: f32,
        desired_speed: f32,
        frame: u32,
        braking_status: bool,
        surface_y: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> ThrustStep {
        let mut reaction = None;
        // Main admits movement rates in seconds; source locomotor/Physics
        // arithmetic operates in 30Hz frames. Common MinSpeed is already raw
        // seconds (unlike Common Speed/Acceleration), so divide every actual
        // admitted host rate here, not in the shared profile binding.
        const FPS: f32 = 30.0;
        let mut max_speed_frame = self.effective_max_speed() / FPS;
        let min_speed_frame = self.min_speed / FPS;
        let desired_speed_frame =
            (desired_speed / FPS).clamp(min_speed_frame, max_speed_frame.max(min_speed_frame));
        let mut desired_speed_frame = desired_speed_frame;
        let velocity_frame = self.movement.velocity / FPS;
        let actual_frame = velocity_frame.length();
        let braking_frame = self.braking / (FPS * FPS);
        if braking_frame > 0.0 && !self.no_slow_down_as_approaching_dest {
            let slow = calc_slow_down_dist(actual_frame, min_speed_frame, braking_frame);
            // AIUpdate already admitted getPathExtraDistance into this argument.
            if on_path_dist < slow {
                desired_speed_frame = min_speed_frame;
            }
        }
        let us = self.get_position();
        let mut local_goal = goal;
        if self.loco_preferred_height != 0.0 && !self.precise_z_pos {
            let preferred = self.loco_preferred_height + surface_y;
            local_goal.y = us.y + (preferred - us.y) * self.loco_preferred_height_damping;
        }
        let forward = self.get_transform_matrix().x_axis.truncate();
        let speed_delta_frame = desired_speed_frame - actual_frame;
        let max_accel_frame = if speed_delta_frame > 0.0 || braking_frame == 0.0 {
            self.effective_acceleration() / (FPS * FPS)
        } else {
            -braking_frame
        };
        let mut max_turn_frame = self.effective_turn_rate() / FPS;
        let desired_thrust = leftover_calc_direction_to_apply_thrust(
            us,
            velocity_frame,
            local_goal,
            max_accel_frame,
            forward,
            self.physics_gravity(),
        );
        let max_thrust_angle = if max_turn_frame > 0.0 {
            self.max_thrust_angle
        } else {
            0.0
        };
        let (thrust_dir, thrust_angle) =
            leftover_try_to_rotate_vector3d(max_thrust_angle, forward, desired_thrust);
        if !thrust_vel_nearly_zero(actual_frame) {
            let mut desired = velocity_frame;
            let mut adjust = true;
            if braking_status {
                desired = goal - us;
                if thrust_vel_nearly_zero(desired.length_squared()) {
                    adjust = false;
                }
                max_turn_frame *= 3.0;
            }
            if adjust {
                let (actual, relative) =
                    leftover_try_to_rotate_vector3d(max_turn_frame, forward, desired);
                if relative != 0.0 {
                    // Reuse the source Matrix3D kernel. This involution is the
                    // existing source XYZ <-> Main XZY basis (no yaw-only path).
                    let basis = glam::Mat4::from_cols(
                        glam::Vec4::X,
                        glam::Vec4::Z,
                        glam::Vec4::Y,
                        glam::Vec4::W,
                    );
                    let source = gamelogic::common::build_transform_matrix(
                        basis.transform_point3(us),
                        basis.transform_vector3(actual),
                    );
                    reaction = Some(self.prepare_transform_tree_reaction(
                        basis * source * basis,
                        frame,
                        effects,
                    ));
                    self.sync_shock_up_from_transform();
                }
            }
        }
        let force = if speed_delta_frame != 0.0 || thrust_angle != 0.0 {
            if max_speed_frame <= 0.0 {
                max_speed_frame = 0.01;
            }
            Some((thrust_dir, max_accel_frame, max_speed_frame))
        } else {
            None
        };
        ThrustStep { reaction, force }
    }
    pub(in crate::game_logic) fn finish_owned_thrust_step(&mut self, step: ThrustStep) {
        self.finish_thrust_force(step.force);
    }
    fn finish_thrust_force(&mut self, force: Option<(glam::Vec3, f32, f32)>) {
        if let Some((thrust_dir, max_accel_frame, max_speed_frame)) = force {
            let damping = (max_accel_frame / max_speed_frame).clamp(0., 1.);
            // Locomotor.cpp1989 rereads current velocity AFTER orientation.
            let accel_frame = thrust_dir * max_accel_frame - self.movement.velocity / 30. * damping;
            let mass = self.physics_get_mass();
            self.apply_motive_force(accel_frame * (30. * mass));
        }
    }

    /// C++ `Locomotor::maintainCurrentPositionHover` (Locomotor.cpp:2527-2576).
    /// Apply motive force along heading to close speedDelta; do not teleport vel.
    fn apply_hover_maintain_brake(&mut self) {
        let min_speed = self.min_speed.max(1.0e-10);
        let actual = self.forward_speed_2d();
        let speed_delta = min_speed - actual;
        if speed_delta.abs() <= min_speed {
            return;
        }
        let mass = self.physics_get_mass();
        let acceleration = if speed_delta > 0.0 {
            self.movement.acceleration
        } else {
            -self.braking.max(0.0)
        };
        let mut accel_force = mass * acceleration;
        let max_force_needed = mass * speed_delta;
        if accel_force.abs() > max_force_needed.abs() {
            accel_force = max_force_needed;
        }
        let dir = self.unit_direction_vector_2d();
        self.apply_motive_force(glam::Vec3::new(
            accel_force * dir.x,
            0.0,
            accel_force * dir.y,
        ));
        self.integrate_locomotor_horizontal_accel();
    }

    /// Apply forward motive force to close speedDelta (C++ legs/other residual).
    pub(super) fn apply_forward_speed_force(&mut self, goal_speed: f32, dt: f32) {
        let actual = self.forward_speed_2d();
        // When moving backwards residual, treat signed speed.
        let actual = if self.moving_backwards {
            -actual.abs()
        } else {
            actual
        };
        let speed_delta = goal_speed - actual;
        if speed_delta.abs() < 1.0e-5 {
            return;
        }
        let mass = self.physics_get_mass();
        // Host Movement accel is units/sec²; convert impulse for one logic frame.
        let frame_dt = (dt * 30.0).clamp(0.5, 2.0) / 30.0; // ~one frame
        let acceleration = if speed_delta > 0.0 {
            self.movement.acceleration
        } else {
            -self.braking.max(self.movement.acceleration)
        };
        let mut accel_force = mass * acceleration * frame_dt * 30.0; // N-ish
        let max_force_needed = mass * speed_delta;
        if accel_force.abs() > max_force_needed.abs() {
            accel_force = max_force_needed;
        }
        let dir = self.unit_direction_vector_2d();
        let sign = if self.moving_backwards { -1.0 } else { 1.0 };
        self.apply_motive_force(glam::Vec3::new(
            accel_force * dir.x * sign,
            0.0,
            accel_force * dir.y * sign,
        ));
        // Integrate immediately so this frame's movement sees it (host dt path).
        self.integrate_locomotor_horizontal_accel();
        // Also blend velocity toward goal for host-second dt residual.
        let dir = self.unit_direction_vector_2d();
        let target = glam::Vec3::new(
            dir.x * goal_speed * sign,
            self.movement.velocity.y,
            dir.y * goal_speed * sign,
        );
        let max_accel = self.movement.acceleration * dt;
        let diff = target - self.movement.velocity;
        if diff.length() <= max_accel {
            self.movement.velocity = target;
        } else if diff.length() > 1e-6 {
            self.movement.velocity += diff.normalize() * max_accel;
        }
        self.invalidate_velocity_magnitude();
    }

    /// C++ PhysicsBehavior::applyMotiveForce residual.
    ///
    /// Temporarily accepts full force (clears motive), applies, then arms motive
    /// window for MOTIVE_FRAMES so subsequent collide forces are lateral-only.
    pub fn apply_motive_force(&mut self, force: glam::Vec3) {
        let prev = self.motive_frames_remaining;
        self.motive_frames_remaining = 0;
        self.apply_physics_force(force);
        self.motive_frames_remaining = MOTIVE_FRAMES_RESIDUAL.max(prev);
    }

    /// C++ PhysicsBehavior::resetDynamicPhysics residual.
    pub fn reset_dynamic_physics(&mut self) {
        self.physics_accel = glam::Vec3::ZERO;
        self.physics_previous_accel = glam::Vec3::ZERO;
        self.movement.velocity = glam::Vec3::ZERO;
        self.invalidate_velocity_magnitude();
        self.shock_yaw_rate = 0.0;
        self.shock_pitch_rate = 0.0;
        self.shock_roll_rate = 0.0;
        self.motive_frames_remaining = 0;
    }

    /// C++ `PhysicsBehavior::getAcceleration()` = previous-frame accel.
    #[must_use]
    pub fn previous_acceleration(&self) -> glam::Vec3 {
        self.physics_previous_accel
    }

    /// Integrate physics_accel into velocity residual (a → v per logic frame).
    ///
    /// C++ `getAcceleration()` returns `m_prevAccel` (previous frame). Store
    /// the current accel before zeroing so the visual calc can read it.
    pub(in crate::game_logic) fn integrate_physics_force_phase(&mut self) {
        // PhysicsUpdate.cpp622: previous acceleration excludes this phase's
        // gravity/friction. HELD still clears accumulated acceleration.
        self.physics_previous_accel = self.physics_accel;
        if self.is_physics_held() {
            self.physics_accel = glam::Vec3::ZERO;
            return;
        }
        if self.shock_stun_frames == 0 {
            self.physics_accel.y += self.physics_gravity()
                * if self.loco_appearance == LocomotorAppearance::Thrust {
                    30.
                } else {
                    1.
                };
        }
        self.apply_frictional_forces();
        self.integrate_physics_velocity();
    }

    pub fn integrate_physics_accel(&mut self) {
        self.integrate_physics_velocity();
        self.physics_accel = glam::Vec3::ZERO;
    }
    fn integrate_physics_velocity(&mut self) {
        self.integrate_physics_delta_velocity(self.physics_accel, true);
        if self.motive_frames_remaining > 0 {
            self.motive_frames_remaining -= 1
        }
    }
    fn integrate_physics_delta_velocity(&mut self, delta: glam::Vec3, vertical: bool) {
        self.movement.velocity += delta;
        if vertical {
            let tiny_y = if self.loco_appearance == LocomotorAppearance::Thrust {
                0.001 * 30.
            } else {
                0.001
            };
            if self.movement.velocity.y.abs() < tiny_y {
                self.movement.velocity.y = 0.
            }
        }
        self.invalidate_velocity_magnitude();
    }
    /// Existing native horizontal marching remains in its current phase.
    /// It must not consume an unrelated pending vertical/bounce force or age
    /// the Physics motive window before the actual Physics operation.
    fn integrate_locomotor_horizontal_accel(&mut self) {
        self.integrate_physics_delta_velocity(
            glam::Vec3::new(self.physics_accel.x, 0., self.physics_accel.z),
            false,
        );
        self.physics_accel.x = 0.;
        self.physics_accel.z = 0.;
    }

    /// Invalidate cached velocity magnitude residual.
    pub fn invalidate_velocity_magnitude(&mut self) {
        self.velocity_magnitude_cache = -1.0;
    }

    /// C++ PhysicsBehavior::getVelocityMagnitude residual.
    pub fn velocity_magnitude(&mut self) -> f32 {
        if self.velocity_magnitude_cache < 0.0 {
            let v = self.movement.velocity;
            self.velocity_magnitude_cache = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        }
        self.velocity_magnitude_cache
    }

    /// C++ getForwardSpeed2D residual (signed along facing on XZ).
    pub fn forward_speed_2d(&self) -> f32 {
        let dir = self.unit_direction_vector_2d();
        let v = self.movement.velocity;
        let vx = v.x * dir.x;
        let vz = v.z * dir.y;
        let dot = vx + vz;
        let speed = (vx * vx + vz * vz).sqrt();
        if dot >= 0.0 { speed } else { -speed }
    }

    /// C++ getAerodynamicFriction residual (clamped).
    pub fn get_aerodynamic_friction(&self) -> f32 {
        let f = self.aerodynamic_friction + self.extra_friction;
        f.max(MIN_AERO_FRICTION_RESIDUAL).min(MAX_FRICTION_RESIDUAL)
    }

    /// C++ getForwardFriction residual.
    pub fn get_forward_friction(&self) -> f32 {
        let f = self.forward_friction + self.extra_friction;
        f.clamp(MIN_NON_AERO_FRICTION_RESIDUAL, MAX_FRICTION_RESIDUAL)
    }

    /// C++ getLateralFriction residual.
    pub fn get_lateral_friction(&self) -> f32 {
        let f = self.lateral_friction + self.extra_friction;
        f.clamp(MIN_NON_AERO_FRICTION_RESIDUAL, MAX_FRICTION_RESIDUAL)
    }

    /// C++ Thing::isSignificantlyAboveTerrain — height > -(3*3)*gravity.
    pub fn is_significantly_above_terrain(&self) -> bool {
        self.height_treats_as_airborne(self.get_position().y - self.ground_height)
    }

    /// C++ Locomotor.cpp:1005-1008 treatAsAirborne — 3 frames of gravity.
    pub fn height_treats_as_airborne(&self, height_above_surface: f32) -> bool {
        height_above_surface > -(3.0 * 3.0) * self.physics_gravity()
    }

    /// C++ DISABLED_HELD: garrisoned / parachute-cargo / prison / Battle Bus hulk.
    pub fn is_physics_held(&self) -> bool {
        self.contained_by.is_some() || self.status.disabled_held
    }

    /// C++ deckTaxiing: DECK_HEIGHT_OFFSET && AI && curSet == LOCOMOTORSET_TAXIING.
    pub fn is_deck_taxiing(&self) -> bool {
        self.has_object_status_bit("DECK_HEIGHT_OFFSET")
            && self
                .get_cur_locomotor_set_token()
                .is_some_and(|set| set.eq_ignore_ascii_case("SET_TAXIING"))
    }

    /// C++ PhysicsBehavior::applyFrictionalForces residual (host XZ ground).
    pub fn apply_frictional_forces(&mut self) {
        // C++: APPLY_FRICTION2D_WHEN_AIRBORNE || !isSignificantlyAboveTerrain || deckTaxiing
        let use_2d = self.apply_friction_2d_when_airborne
            || !self.is_significantly_above_terrain()
            || self.is_deck_taxiing();

        if use_2d {
            // YPR damping residual: DEFAULT_LATERAL_FRICTION on shock rates.
            let d = 1.0 - DEFAULT_LATERAL_FRICTION_RESIDUAL;
            self.shock_yaw_rate *= d;
            self.shock_pitch_rate *= d;
            self.shock_roll_rate *= d;

            let v = self.movement.velocity;
            if v.x != 0.0 || v.z != 0.0 {
                let dir = self.unit_direction_vector_2d();
                let mass = self.physics_get_mass();
                let lateral_dot = v.x * (-dir.y) + v.z * dir.x;
                let lat_x = lateral_dot * (-dir.y);
                let lat_z = lateral_dot * dir.x;
                let lf = mass * self.get_lateral_friction();
                let mut accel = glam::Vec3::new(-(lf * lat_x), 0.0, -(lf * lat_z));
                if !self.is_motive() {
                    let forward_dot = v.x * dir.x + v.z * dir.y;
                    let fwd_x = forward_dot * dir.x;
                    let fwd_z = forward_dot * dir.y;
                    let ff = mass * self.get_forward_friction();
                    accel.x += -(ff * fwd_x);
                    accel.z += -(ff * fwd_z);
                }
                self.apply_physics_force(accel);
            }
        } else {
            let aero = -self.get_aerodynamic_friction();
            let v = self.movement.velocity;
            self.physics_accel.x += v.x * aero;
            self.physics_accel.y += v.y * aero;
            self.physics_accel.z += v.z * aero;
            let d = 1.0 + aero;
            self.shock_yaw_rate *= d;
            self.shock_pitch_rate *= d;
            self.shock_roll_rate *= d;
        }
    }

    /// C++ PhysicsBehavior::transferVelocityTo residual.
    pub fn transfer_velocity_to(&self, other: &mut Object) {
        other.movement.velocity += self.movement.velocity;
        other.invalidate_velocity_magnitude();
    }

    /// C++ PhysicsBehavior::addVelocityTo residual.
    pub fn add_velocity(&mut self, vel: glam::Vec3) {
        self.movement.velocity += vel;
        self.invalidate_velocity_magnitude();
    }
}

fn thrust_vel_nearly_zero(v: f32) -> bool {
    v.abs() < 0.001
}

/// Leftover `calc_direction_to_apply_thrust` / C++ `Locomotor.cpp:175-250`.
/// Host Y-up: gravity is added to velocity.y (C++ velocity.z).
fn leftover_calc_direction_to_apply_thrust(
    obj_pos: glam::Vec3,
    cur_vel: glam::Vec3,
    goal_pos: glam::Vec3,
    max_accel: f32,
    forward: glam::Vec3,
    gravity: f32,
) -> glam::Vec3 {
    let vec_to_goal = goal_pos - obj_pos;
    if thrust_vel_nearly_zero(vec_to_goal.length_squared()) {
        let len = forward.length();
        return if len > 1.0e-8 {
            forward / len
        } else {
            glam::Vec3::X
        };
    }

    let mut cur_vel = cur_vel;
    cur_vel.y += gravity;

    let dist_to_goal = vec_to_goal.length();
    let cur_vel_mag_sqr = cur_vel.length_squared();
    let cur_vel_mag = cur_vel_mag_sqr.sqrt();
    let max_accel_sqr = max_accel * max_accel;
    let denom = cur_vel_mag_sqr - max_accel_sqr;

    if !thrust_vel_nearly_zero(denom) {
        let t = (dist_to_goal * (cur_vel_mag + max_accel)) / denom;
        let t2 = (dist_to_goal * (cur_vel_mag - max_accel)) / denom;
        if t >= 0.0 || t2 >= 0.0 {
            let t = if t < 0.0 || (t2 >= 0.0 && t2 < t) {
                t2
            } else {
                t
            };
            if !thrust_vel_nearly_zero(t) {
                let mut dir = glam::Vec3::new(
                    vec_to_goal.x / t - cur_vel.x,
                    vec_to_goal.y / t - cur_vel.y,
                    vec_to_goal.z / t - cur_vel.z,
                );
                let len = dir.length();
                if len > 1.0e-8 {
                    dir /= len;
                    return dir;
                }
            }
        }
    }

    let len = vec_to_goal.length();
    if len > 1.0e-8 {
        vec_to_goal / len
    } else {
        glam::Vec3::X
    }
}

/// Leftover `try_to_rotate_vector3d` / C++ `tryToRotateVector3D` (`Locomotor.cpp:91-149`).
fn leftover_try_to_rotate_vector3d(
    max_angle: f32,
    start: glam::Vec3,
    end: glam::Vec3,
) -> (glam::Vec3, f32) {
    if thrust_vel_nearly_zero(max_angle) {
        return (start, 0.0);
    }
    let start_len = start.length();
    let end_len = end.length();
    if start_len < 1.0e-6 || end_len < 1.0e-6 {
        return (end, 0.0);
    }
    let start_n = start / start_len;
    let end_n = end / end_len;
    let cosine = start_n.dot(end_n).clamp(-1.0, 1.0);
    let angle_between = cosine.acos();
    if angle_between.abs() <= max_angle {
        return (end_n, angle_between);
    }
    let axis = start_n.cross(end_n);
    let axis_len = axis.length();
    if axis_len < 1.0e-6 {
        return (end_n, angle_between);
    }
    let axis_n = axis / axis_len;
    let (sin_a, cos_a) = max_angle.sin_cos();
    let rotated = start_n * cos_a
        + axis_n.cross(start_n) * sin_a
        + axis_n * axis_n.dot(start_n) * (1.0 - cos_a);
    let rot_len = rotated.length();
    if rot_len < 1.0e-6 {
        (end_n, max_angle)
    } else {
        (rotated / rot_len, max_angle)
    }
}

/// Only C++ scalar locals and one new transform; no canonical Object clone.
struct PivotTurn {
    transform: Option<glam::Mat4>,
    orientation: Option<f32>,
    turning: PhysicsTurningType,
    relative: f32,
}
impl PivotTurn {
    fn unchanged() -> Self {
        Self {
            transform: None,
            orientation: None,
            turning: PhysicsTurningType::TurnNone,
            relative: 0.0,
        }
    }
}

impl crate::game_logic::GameLogic {
    pub(in crate::game_logic) fn rotate_owned_pivot_with_effects(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        max_turn: f32,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<(PhysicsTurningType, f32)> {
        let plan = self.objects.get(&id)?.plan_loco_pivot(goal, max_turn);
        if let Some(transform) = plan.transform {
            let reaction = self.objects.get_mut(&id)?.prepare_transform_tree_reaction(
                transform,
                frame,
                &mut effects.reborrow_for_object(&mut self.partition_manager),
            );
            if !self.complete_owned_position_reaction(id, reaction, effects) {
                return None;
            }
        }
        let object = self.objects.get_mut(&id)?;
        if let Some(angle) = plan.orientation {
            object.set_orientation_with_partition_owner(angle, &mut self.partition_manager);
        }
        object.physics_turning = plan.turning;
        Some((plan.turning, plan.relative))
    }
    pub(in crate::game_logic) fn rotate_owned_towards_position_with_effects(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        dt: f32,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<(PhysicsTurningType, f32)> {
        let max_turn = self.objects.get(&id)?.maximum_towards_turn(dt);
        self.rotate_owned_pivot_with_effects(id, goal, max_turn, frame, effects)
    }
}

#[cfg(test)]
#[path = "physics/pivot_source_owner_tests.rs"]
mod pivot_source_owner_tests;

#[cfg(test)]
#[path = "physics/source_thrust_owner_tests.rs"]
mod source_thrust_owner_tests;

#[cfg(test)]
mod source_lift_cap_tests {
    use super::*;
    fn admitted(healthy: f32, damaged: f32) -> Object {
        let mut template = game_engine::common::ini::ini_locomotor::LocomotorTemplate::new(
            "TypedLiftFixture".into(),
        );
        template
            .surfaces
            .add_surface(game_engine::common::ini::ini_locomotor::LocomotorSurfaceTypeMask::AIR);
        template.lift = healthy;
        template.lift_damaged = damaged;
        let binding =
            crate::game_logic::locomotor_bootstrap::host_locomotor_binding_from_template(&template)
                .unwrap();
        let mut object = Object::new(
            ThingTemplate::new("TypedLiftActor"),
            ObjectId(91),
            Team::USA,
        );
        crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(&mut object, &binding);
        object
    }
    #[test]
    fn actual_admitted_lift_definition_zero_damage_and_runtime_cap_are_distinct() {
        let mut object = admitted(0.2, 0.);
        assert_eq!(
            object.max_lift,
            crate::game_logic::locomotor_bootstrap::DEFAULT_HOST_LIFT_CAP
        );
        assert_eq!(object.get_max_lift(), 0.2);
        object.body_damage_state =
            crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged;
        assert_eq!(
            object.get_max_lift(),
            0.,
            "explicit damaged zero is not invented half healthy lift"
        );
        object.body_damage_state =
            crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine;
        object.max_lift = 0.05;
        assert_eq!(object.get_max_lift(), 0.05);
        assert_eq!(object.selected_locomotor_definition.unwrap().max_lift, 0.2);
    }
    #[test]
    fn actual_admitted_damaged_lift_can_exceed_healthy_until_source_cap() {
        let mut object = admitted(0.1, 0.3);
        object.body_damage_state =
            crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged;
        assert_eq!(object.get_max_lift(), 0.3);
        object.max_lift = 0.25;
        assert_eq!(object.get_max_lift(), 0.25);
    }
}
