use super::*;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;

static PREV_ACCEL: Lazy<Mutex<HashMap<u32, glam::Vec3>>> = Lazy::new(|| Mutex::new(HashMap::new()));

impl Object {
    pub fn take_damage(&mut self, damage: f32) -> bool {
        self.take_damage_from(damage, None)
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
        self.set_orientation(ori);
        // Then apply this increment of pitch/roll onto the transform so
        // upside-down kill can see integrated up-Y, not a synthetic scalar.
        let pryf = self.pitch_roll_yaw_factor;
        self.apply_physics_ypr(0.0, dpitch * pryf, droll * pryf);
        // C++ applyRandomRotation: setAllowBouncing(true) after stick gate.
        self.shock_allow_bounce = true;
        self.record_host_shock_stun();
    }

    pub fn apply_shock_wave_impulse(&mut self, force: glam::Vec3) -> bool {
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
        self.apply_shock_random_rotation(seed);
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
            self.ai_state,
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
        (2.0 * leftover_loco_gravity().abs() * height.abs()).sqrt()
    }

    /// C++ PhysicsBehaviorModuleData::m_minFallSpeedForDamage default (height 40).
    pub fn min_fall_speed_for_damage() -> f32 {
        Self::height_to_fall_speed(40.0)
    }

    /// Leftover `height_to_speed(40)` with leftover/retail gravity (~2.385).
    /// Remaps the old live g=1 default `sqrt(80)` so tosses can splat.
    pub(super) fn leftover_compare_min_fall_speed(stored: f32) -> f32 {
        let leftover_default = Self::min_fall_speed_for_damage();
        if !stored.is_finite() || stored <= 0.0 {
            return leftover_default;
        }
        if (stored - (80.0f32).sqrt()).abs() < 1e-2 {
            leftover_default
        } else {
            stored
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
        if let Some(goal) = self.host_blocked_by_goal() {
            let us = self.get_position();
            let dx = (goal.x - us.x).abs();
            let dz = (goal.z - us.z).abs();
            if dx < PATHFIND_CELL_SIZE_F_RESIDUAL && dz < PATHFIND_CELL_SIZE_F_RESIDUAL {
                return false;
            }
        }

        if self.can_crush_or_squish(other, is_ally) {
            return false;
        }
        let other_ground =
            other.can_move() && !other.status.airborne_target && !other.is_parachuting();
        if !other_ground {
            return false;
        }
        if self.moving_backwards {
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

        // Relative angle of other from us along our facing.
        let collision_angle = self.relative_angle_2d_to(them);
        let other_angle = other.relative_angle_2d_to(us);
        let mut angle_limit = std::f32::consts::FRAC_PI_4; // 45 deg
        let other_moving = other.movement.velocity.length_squared() > 0.01;
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

        // Long blocked + opposite heading: pass through residual.
        if self.num_frames_blocked > 30 && dir_dot <= 0.0 {
            return false;
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
            .or(self.movement.target_position)
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
            return 0.0;
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
        self.status.moving
            || self.movement.target_position.is_some()
            || !self.movement.path.is_empty()
            || self.movement.velocity.length_squared() > 0.01
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
    ) -> bool {
        if !self.allow_collide_force {
            return false;
        }
        if self.can_path_through_units {
            self.is_blocked = false;
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
        let self_ground = self.can_move() && !self.status.airborne_target && !self.is_parachuting();
        let other_ground =
            other.can_move() && !other.status.airborne_target && !other.is_parachuting();
        if !self_ground || !other_ground {
            return false;
        }

        let self_moving = self.host_ai_is_moving();
        if self_moving {
            let blocked = self.ai_blocked_by(other, is_ally);
            if blocked {
                // Panic infantry bounces residual.
                if self.is_kind_of(crate::game_logic::KindOf::Infantry) && self.is_panicking {
                    return true;
                }
                self.is_blocked = true;
                if self.num_frames_blocked == 0 {
                    self.num_frames_blocked = 1;
                }
                let max_speed = self.calculate_max_blocked_speed(other);
                if max_speed < self.cur_max_blocked_speed {
                    self.cur_max_blocked_speed = max_speed;
                }
                // C++ processCollision: rotate resets blockedFrames; stopped other → stuck.
                if !self.need_to_rotate() {
                    if !other.host_ai_is_moving() {
                        self.is_blocked_and_stuck = true;
                    }
                } else {
                    self.num_frames_blocked = 1;
                }
                // Vehicle into infantry: request move-away residual.
                if other.is_kind_of(crate::game_logic::KindOf::Infantry)
                    && !self.is_kind_of(crate::game_logic::KindOf::Infantry)
                {
                    // C++ busy/using-ability gate residual.
                    if !other.status.using_ability {
                        self.request_other_move_away = Some(other.id);
                    }
                }
                return false;
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
        let mut max_turn = self.effective_turn_rate() * dt;
        if matches!(
            self.loco_appearance,
            LocomotorAppearance::WheelsFour | LocomotorAppearance::Motorcycle
        ) {
            max_turn *= self.wheeled_turn_factor();
        }
        self.rotate_obj_around_loco_pivot(goal, max_turn)
    }

    /// C++ `Locomotor::setUltraAccurate` + `setPhysicsOptions`.
    pub fn set_ultra_accurate(&mut self, ultra: bool) {
        if self.ultra_accurate == ultra {
            return;
        }
        self.ultra_accurate = ultra;
        self.set_locomotor_physics_options();
        self.record_host_locomotor();
    }

    /// C++ `Locomotor::setUsePreciseZPos` (PRECISE_Z_POS).
    pub fn set_precise_z_pos(&mut self, precise: bool) {
        if self.precise_z_pos == precise {
            return;
        }
        self.precise_z_pos = precise;
        self.record_host_locomotor();
    }

    /// C++ `Locomotor::setAllowInvalidPosition` (ALLOW_INVALID_POSITION).
    pub fn set_allow_invalid_position(&mut self, allow: bool) {
        if self.allow_invalid_position == allow {
            return;
        }
        self.allow_invalid_position = allow;
        self.record_host_locomotor();
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
            (self.movement.velocity.length() / turn_speed)
                .abs()
                .min(1.0)
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
        let angle = self.get_orientation();
        let mut offset = self.turn_pivot_offset;
        if self.is_braking {
            offset = 0.0;
        }
        let us = self.get_position();
        let (dx, dz, turn_pos) = if offset.abs() > 1e-6 {
            let radius = self.selection_radius.max(1.0);
            let turn_point = offset * radius;
            let dir = self.unit_direction_vector_2d();
            let turn_pos =
                glam::Vec3::new(us.x + dir.x * turn_point, us.y, us.z + dir.y * turn_point);
            let dx = goal.x - turn_pos.x;
            let dz = goal.z - turn_pos.z;
            if dx.abs() < 0.1 && dz.abs() < 0.1 {
                self.physics_turning = PhysicsTurningType::TurnNone;
                self.record_host_locomotor();
                return (PhysicsTurningType::TurnNone, 0.0);
            }
            (dx, dz, Some(turn_pos))
        } else {
            let dx = goal.x - us.x;
            let dz = goal.z - us.z;
            if dx * dx + dz * dz < 1.0e-8 {
                self.physics_turning = PhysicsTurningType::TurnNone;
                self.record_host_locomotor();
                return (PhysicsTurningType::TurnNone, 0.0);
            }
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
        if let Some(tp) = turn_pos {
            // C++ T(pivot)*Rz(amount)*T(-pivot). Host set_orientation is
            // T*Ry(angle) with local +X → (cos, 0, -sin), so the same
            // pre-rotate about the pivot is Ry, not a raw XY Rz.
            let cos_a = amount.cos();
            let sin_a = amount.sin();
            let rx = us.x - tp.x;
            let rz = us.z - tp.z;
            let nx = tp.x + rx * cos_a + rz * sin_a;
            let nz = tp.z - rx * sin_a + rz * cos_a;
            self.set_position(glam::Vec3::new(nx, us.y, nz));
        }
        self.set_orientation(angle + amount);
        self.physics_turning = turning;
        self.record_host_locomotor();
        (turning, rel)
    }

    /// C++ Locomotor::locoUpdate_moveTowardsAngle residual.
    ///
    /// XY only. C++ `handleBehaviorZ` is one leftover `getSurfaceHtAtPt` pass
    /// (`Locomotor.cpp:884-895`); live FACE caller applies that leftover-terrain
    /// Z once. Never treat pose-Y as surface (hq-jg55x).
    pub fn loco_update_move_towards_angle(&mut self, goal_angle: f32, dt: f32) {
        self.maintain_pos_valid = false;
        if self.shock_stun_frames > 0 {
            return;
        }
        let min_speed = self.min_speed;
        if min_speed > 0.0 {
            let us = self.get_position();
            let desired = glam::Vec3::new(
                us.x + goal_angle.cos() * min_speed * 2.0,
                us.y,
                us.z + (-goal_angle.sin()) * min_speed * 2.0,
            );
            let prev = self.movement.target_position;
            self.movement.target_position = Some(desired);
            let _ = self.rotate_towards_position(desired, dt);
            self.apply_forward_speed_force(min_speed, dt);
            let p = self.get_position() + self.movement.velocity * dt;
            self.set_position(p);
            self.movement.target_position = prev;
        } else {
            let us = self.get_position();
            let desired = glam::Vec3::new(
                us.x + goal_angle.cos() * 1000.0,
                us.y,
                us.z + (-goal_angle.sin()) * 1000.0,
            );
            let _ = self.rotate_towards_position(desired, dt);
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
    pub fn handle_behavior_z(&mut self, ground_y: f32, goal_y: Option<f32>) -> bool {
        match self.loco_behavior_z {
            LocomotorBehaviorZ::NoZMotiveForce => false,
            LocomotorBehaviorZ::SeaLevel => {
                // Fail-closed: no water table — snap to ground layer.
                let mut p = self.get_position();
                p.y = ground_y;
                self.set_position(p);
                true
            }
            LocomotorBehaviorZ::FixedSurfaceRelativeHeight
            | LocomotorBehaviorZ::RelativeToGroundAndBuildings => {
                // C++ Locomotor.cpp:2223-2246 — hard-snap, ignore physics.
                let surface = if matches!(
                    self.loco_behavior_z,
                    LocomotorBehaviorZ::RelativeToGroundAndBuildings
                ) {
                    leftover_ground_or_structure_height(self.get_position(), ground_y)
                } else {
                    ground_y
                };
                let mut p = self.get_position();
                p.y = self.loco_preferred_height + surface;
                self.set_position(p);
                true
            }
            LocomotorBehaviorZ::FixedAbsoluteHeight => {
                let mut p = self.get_position();
                p.y = self.loco_preferred_height;
                self.set_position(p);
                true
            }
            LocomotorBehaviorZ::SurfaceRelativeHeight => {
                self.apply_surface_relative_lift(ground_y, goal_y)
            }
            LocomotorBehaviorZ::SmoothRelativeToHighestLayer => {
                let surface = leftover_highest_layer_height(self.get_position(), ground_y);
                self.apply_surface_relative_lift(surface, goal_y)
            }
            LocomotorBehaviorZ::AbsoluteHeight => {
                // C++ Locomotor.cpp:2288-2317 — same lift as SurfaceRelative
                // with surfaceHt = 0 (not kinematic snap).
                self.apply_surface_relative_lift(0.0, goal_y)
            }
        }
    }

    /// C++ `Z_SURFACE_RELATIVE_HEIGHT` / `Z_SMOOTH_RELATIVE_TO_HIGHEST_LAYER`
    /// lift residual (`Locomotor.cpp:2288-2317` / `:2248-2285`).
    fn apply_surface_relative_lift(&mut self, surface_y: f32, goal_y: Option<f32>) -> bool {
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
        delta *= self.loco_preferred_height_damping.clamp(0.0, 1.0);
        let preferred = p.y + delta;
        let lift = self.calc_lift_to_use_at_pt(p.y, preferred);
        if lift.abs() > 1.0e-4 {
            let force_y = lift * self.physics_get_mass();
            self.apply_motive_force(glam::Vec3::new(0.0, force_y, 0.0));
        }
        true
    }

    /// C++ `TheTerrainLogic->getHighestLayerForDestination` + `getLayerHeight`
    /// (`clip=false`) so flyers ride bridge decks.
    pub fn highest_layer_surface_ht(&self, ground_y: f32) -> f32 {
        leftover_highest_layer_height(self.get_position(), ground_y)
    }

    /// C++ `Locomotor::getSurfaceHtAtPt` at the object's current XZ.
    pub(crate) fn leftover_surface_ht(&self, ground_y: f32) -> f32 {
        leftover_surface_ht_at_pt(self.get_position(), ground_y)
    }

    /// C++ Locomotor::locoUpdate_maintainCurrentPosition residual.
    ///
    /// Stops horizontal motion for legs/treads/wheels; hover/wings need constant Z.
    pub fn loco_maintain_current_position(&mut self, ground_y: f32, dt: f32) -> bool {
        if !self.maintain_pos_valid {
            self.maintain_pos = Some(self.get_position());
            self.record_host_combat_attack();
            self.maintain_pos_valid = true;
        }
        // C++ Locomotor.cpp:2420-2421 — reset donut window and clear IS_BRAKING.
        self.start_move();
        self.is_braking = false;
        self.physics_turning = PhysicsTurningType::TurnNone;
        self.record_host_locomotor();

        // Appearance-specific maintain residual.
        match self.loco_appearance {
            LocomotorAppearance::Wings => {
                self.motive_frames_remaining = MOTIVE_FRAMES_RESIDUAL.max(1);
                // 2D circle only. C++ handleBehaviorZ runs after with
                // getSurfaceHtAtPt (hq-xlays) — not own altitude.
                self.maintain_position_wings(dt.max(1.0 / 30.0));
                return true;
            }
            LocomotorAppearance::Thrust => {
                if let Some(m) = self.maintain_pos {
                    let spd = self.min_speed.max(1.0);
                    self.move_towards_thrust(m, 0.0, spd, dt.max(1.0 / 30.0));
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
                let maintain_y = self.maintain_pos.map(|p| p.y);
                let _ = self.handle_behavior_z(ground_y, maintain_y);
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

        let maintain_y = self.maintain_pos.map(|p| p.y);
        let needs_z = self.handle_behavior_z(ground_y, maintain_y);
        // Hover/air need constant calling; ground settled does not.
        airborne_loco || needs_z
    }

    /// C++ `AIUpdateInterface::chooseGoodLocomotorFromCurrentSet` (AIUpdate.cpp:833-872).
    pub fn choose_good_locomotor_from_current_set(
        &mut self,
        cell_type: gamelogic::ai::pathfind_astar::PathfindCellType,
    ) {
        if self.locomotor_set_names.len() < 2 {
            let fallback = crate::game_logic::locomotor_bootstrap::locomotor_set_names_for_unit(
                &self.thing.template.name,
            );
            if fallback.len() >= 2 {
                self.locomotor_set_names = fallback;
            }
        }
        if self.locomotor_set_names.len() < 2 {
            return;
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
            .or_else(|| self.cur_locomotor_name.clone())
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
    }

    /// C++ `Locomotor::moveTowardsPositionHover` OVER_WATER (Locomotor.cpp:1868-1886).
    pub fn apply_hover_over_water(&mut self, underwater: bool) {
        if !matches!(self.loco_appearance, LocomotorAppearance::Hover) {
            if self.over_water {
                self.over_water = false;
                self.refresh_model_condition_bits();
                self.record_host_locomotor();
            }
            return;
        }
        if self.over_water == underwater {
            return;
        }
        self.over_water = underwater;
        self.refresh_model_condition_bits();
        self.record_host_locomotor();
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
        if self.is_blocked {
            if self.need_to_rotate() {
                self.num_frames_blocked = 1;
            } else {
                self.num_frames_blocked = self.num_frames_blocked.saturating_add(1);
            }
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
        } else if self.bump_speed_limit < f32::MAX {
            // C++ only eases while bump < FAST_AS_POSSIBLE (AIUpdate.cpp:2209-2217).
            if self.bump_speed_limit < speed * 0.2 {
                self.bump_speed_limit = speed * 0.2;
            }
            self.bump_speed_limit *= 1.05;
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
        // Walking units stick to ground residual.
        if self.is_kind_of(crate::game_logic::KindOf::Infantry) {
            self.stick_to_ground = true;
            if matches!(self.loco_appearance, LocomotorAppearance::Other) {
                self.loco_appearance = LocomotorAppearance::LegsTwo;
                self.record_host_locomotor();
            }
        } else if self.is_kind_of(crate::game_logic::KindOf::Aircraft) {
            if matches!(self.loco_appearance, LocomotorAppearance::Other) {
                self.loco_appearance = LocomotorAppearance::Wings;
                self.record_host_locomotor();
            }
        } else if self.is_kind_of(crate::game_logic::KindOf::Vehicle) {
            if matches!(self.loco_appearance, LocomotorAppearance::Other) {
                // Fail-closed: vehicles default treads-like (tanks common in host).
                self.loco_appearance = LocomotorAppearance::Treads;
                self.record_host_locomotor();
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
        let g = &self.thing.template.geometry_info;
        if g.authored {
            g.bounding_sphere_radius().max(1.0)
        } else {
            self.selection_radius.max(1.0)
        }
    }

    /// C++ `GeometryInfo::getBoundingCircleRadius`, or pick radius when unauthored.
    pub fn physics_collide_circle_radius(&self) -> f32 {
        let g = &self.thing.template.geometry_info;
        if g.authored {
            g.bounding_circle_radius().max(1.0)
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
        let us = self.get_position();
        let mut dx = other_center.x - us.x;
        let mut dy = other_center.y - us.y;
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
        let pristine = self.max_lift.max(0.0);
        let damaged = self.max_lift_damaged.clamp(0.0, pristine.max(0.0));
        match self.body_damage_state {
            HostBodyDamageType::Pristine | HostBodyDamageType::Damaged => pristine,
            HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble => {
                if damaged > 0.0 {
                    damaged.min(pristine)
                } else if pristine > 0.0 {
                    pristine * 0.5
                } else {
                    0.0
                }
            }
        }
    }

    /// C++ Locomotor::calcLiftToUseAtPt residual (simplified).
    ///
    /// Gravity is leftover `TheGlobalData->m_gravity` (host world-Y).
    /// Returns lift accel to apply (not force).
    pub fn calc_lift_to_use_at_pt(&self, cur_y: f32, preferred_height: f32) -> f32 {
        let gravity = leftover_loco_gravity();
        let max_gross = self.get_max_lift();
        let mut max_net = max_gross + gravity;
        if max_net < 0.0 {
            max_net = 0.0;
        }
        let cur_vy = self.movement.velocity.y;
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
        self.record_host_ai_request();
        self.requested_victim_id = victim_id;
        self.record_host_ai_request();
        self.is_attack_path = true;
        self.is_approach_path = false;
        self.record_host_locomotor();
        self.is_safe_path = false;
        self.waiting_for_path = true;
        if self.path_timestamp > 0 && current_frame.saturating_sub(self.path_timestamp) < 3 {
            // C++ setQueueForPathTime(2 sec)
            self.queue_for_path_frames = 60;
            return false;
        }
        self.path_timestamp = current_frame;
        self.record_host_ai_request();
        true
    }

    /// C++ AIUpdateInterface::requestPath flag residual (non-attack).
    pub fn begin_request_move_path(&mut self, destination: glam::Vec3, current_frame: u32) -> bool {
        self.requested_destination = Some(destination);
        self.record_host_ai_request();
        self.requested_victim_id = None;
        self.record_host_ai_request();
        self.is_attack_path = false;
        self.is_exact_path = false;
        self.is_approach_path = false;
        self.record_host_locomotor();
        self.is_safe_path = false;
        self.waiting_for_path = true;
        if self.path_timestamp > 0 && current_frame.saturating_sub(self.path_timestamp) < 3 {
            self.queue_for_path_frames = 60;
            return false;
        }
        self.path_timestamp = current_frame;
        self.record_host_ai_request();
        true
    }

    /// C++ requestApproachPath residual.
    pub fn begin_request_approach_path(
        &mut self,
        destination: glam::Vec3,
        current_frame: u32,
    ) -> bool {
        let ok = self.begin_request_move_path(destination, current_frame);
        self.is_approach_path = true;
        self.record_host_locomotor();
        ok
    }

    /// C++ requestSafePath residual (`AIUpdate.cpp:549-560`).
    pub fn begin_request_safe_path(
        &mut self,
        repulsor: ObjectId,
        flee_pos: glam::Vec3,
        current_frame: u32,
    ) -> bool {
        let ok = self.begin_request_move_path(flee_pos, current_frame);
        self.is_safe_path = true;
        if self.requested_victim_id != Some(repulsor) {
            self.safe_path_repulsor2 = self.requested_victim_id;
        }
        self.requested_victim_id = Some(repulsor);
        self.record_host_ai_request();
        ok
    }

    /// Tick path queue delay residual.
    pub fn tick_path_queue(&mut self) {
        if self.queue_for_path_frames > 0 {
            self.queue_for_path_frames -= 1;
        }
        if self.temporary_move_frames > 0 {
            self.temporary_move_frames -= 1;
            if self.temporary_move_frames == 0
                && matches!(self.ai_state, AIState::Moving)
                && self.movement.target_position.is_none()
            {
                // Temporary AI move expired with no destination — idle residual.
                self.set_ai_state(AIState::Idle);
                self.record_host_combat_attack();
            }
        }
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
        self.record_host_combat_attack();
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
        self.movement.current_path_index = 0;
        self.movement.target_position = self.movement.path.first().copied();
        self.waiting_for_path = false;
        self.is_braking = false;
        self.start_move();
        self.record_host_movement();
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
            self.integrate_physics_accel();
            return true;
        }
        // Stationary on invalid: nudge along facing.
        let d = self.unit_direction_vector_2d();
        let force = glam::Vec3::new(d.x * mass / 5.0, 0.0, d.y * mass / 5.0);
        self.apply_motive_force(force);
        self.integrate_physics_accel();
        true
    }

    /// C++ maintainCurrentPositionWings residual — circle around maintain pos.
    pub fn maintain_position_wings(&mut self, dt: f32) {
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
        let (_t, _rel) = self.rotate_towards_position(desired, dt);
        self.apply_forward_speed_force(spd, dt);
        let p = self.get_position() + self.movement.velocity * dt;
        self.set_position(p);
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
        mut desired_speed: f32,
        dt: f32,
    ) {
        let mut max_speed = self.effective_max_speed();
        desired_speed = desired_speed.clamp(self.min_speed, max_speed.max(self.min_speed));
        let actual = self.movement.velocity.length();
        if self.braking > 0.0 && !self.no_slow_down_as_approaching_dest {
            let slow = calc_slow_down_dist(actual, self.min_speed, self.braking);
            if on_path_dist < slow {
                desired_speed = self.min_speed;
            }
        }

        let mut local_goal = goal;
        if self.loco_preferred_height != 0.0 && !self.precise_z_pos {
            // C++ getSurfaceHtAtPt: leftover water/ground, not current altitude.
            let surface = leftover_surface_ht_at_pt(self.get_position(), self.ground_height);
            let preferred = self.loco_preferred_height + surface;
            let delta = (preferred - self.get_position().y) * self.loco_preferred_height_damping;
            local_goal.y = self.get_position().y + delta;
        }

        let us = self.get_position();
        let (fx, fz) = self.unit_direction_xz();
        let forward = glam::Vec3::new(fx, 0.0, fz);
        let speed_delta = desired_speed - actual;
        let max_accel = if speed_delta > 0.0 || self.braking <= 0.0 {
            self.effective_acceleration()
        } else {
            -self.braking
        };
        let mut max_turn_rate = self.effective_turn_rate();

        let desired_thrust = leftover_calc_direction_to_apply_thrust(
            us,
            self.movement.velocity,
            local_goal,
            max_accel,
            forward,
        );
        let max_thrust_angle = if max_turn_rate > 0.0 {
            self.max_thrust_angle
        } else {
            0.0
        };
        let (thrust_dir, thrust_angle) =
            leftover_try_to_rotate_vector3d(max_thrust_angle, forward, desired_thrust);

        // C++ orients to velocity (3× turn while braking, aim at original goal).
        if !thrust_vel_nearly_zero(self.movement.velocity.length()) {
            let mut vel = self.movement.velocity;
            let mut adjust = true;
            if self.is_braking {
                vel = goal - us;
                if thrust_vel_nearly_zero(vel.length()) {
                    adjust = false;
                }
                max_turn_rate *= 3.0;
            }
            if adjust {
                let desired_yaw = (-vel.z).atan2(vel.x);
                let aim = us + glam::Vec3::new(desired_yaw.cos(), 0.0, -desired_yaw.sin());
                let _ = self.rotate_obj_around_loco_pivot(aim, max_turn_rate * dt);
            }
        }

        if speed_delta != 0.0 || thrust_angle != 0.0 {
            if max_speed <= 0.0 {
                max_speed = 0.01;
            }
            let damping = (max_accel / max_speed).clamp(0.0, 1.0);
            let accel = thrust_dir * max_accel - self.movement.velocity * damping;
            let mass = self.physics_get_mass();
            self.apply_motive_force(accel * mass);
            self.integrate_physics_accel();
        }
        let p = us + self.movement.velocity * dt;
        self.set_position(p);
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
        self.integrate_physics_accel();
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
        self.integrate_physics_accel();
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
        self.record_host_movement();
    }

    /// C++ PhysicsBehavior::applyMotiveForce residual.
    ///
    /// Temporarily accepts full force (clears motive), applies, then arms motive
    /// window for MOTIVE_FRAMES so subsequent collide forces are lateral-only.
    pub fn apply_motive_force(&mut self, force: glam::Vec3) {
        let prev = self.motive_frames_remaining;
        self.motive_frames_remaining = 0;
        self.record_host_physics_motive();
        self.apply_physics_force(force);
        self.motive_frames_remaining = MOTIVE_FRAMES_RESIDUAL.max(prev);
        self.record_host_physics_motive();
    }

    /// C++ PhysicsBehavior::resetDynamicPhysics residual.
    pub fn reset_dynamic_physics(&mut self) {
        self.physics_accel = glam::Vec3::ZERO;
        self.movement.velocity = glam::Vec3::ZERO;
        self.invalidate_velocity_magnitude();
        self.shock_yaw_rate = 0.0;
        self.shock_pitch_rate = 0.0;
        self.shock_roll_rate = 0.0;
        self.motive_frames_remaining = 0;
        self.record_host_physics_motive();
        self.record_host_movement();
    }

    /// C++ `PhysicsBehavior::getAcceleration()` = previous-frame accel.
    #[must_use]
    pub fn previous_acceleration(&self) -> glam::Vec3 {
        PREV_ACCEL
            .lock()
            .get(&self.id.0)
            .copied()
            .unwrap_or(glam::Vec3::ZERO)
    }

    /// Integrate physics_accel into velocity residual (a → v per logic frame).
    ///
    /// C++ `getAcceleration()` returns `m_prevAccel` (previous frame). Store
    /// the current accel before zeroing so the visual calc can read it.
    pub fn integrate_physics_accel(&mut self) {
        PREV_ACCEL.lock().insert(self.id.0, self.physics_accel);
        if self.physics_accel != glam::Vec3::ZERO {
            self.movement.velocity += self.physics_accel;
            self.physics_accel = glam::Vec3::ZERO;
            self.invalidate_velocity_magnitude();
        }
        if self.motive_frames_remaining > 0 {
            self.motive_frames_remaining -= 1;
        }
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
        Self::height_treats_as_airborne(self.get_position().y - self.ground_height)
    }

    /// C++ Locomotor.cpp:1005-1008 treatAsAirborne — 3 frames of gravity.
    pub fn height_treats_as_airborne(height_above_surface: f32) -> bool {
        height_above_surface > -(3.0 * 3.0) * leftover_loco_gravity()
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

/// C++ `PartitionManager::getGroundOrStructureHeight` via leftover cells
/// (`PartitionManager.cpp:4674`). Host XZ maps to leftover XY. If leftover
/// partition is empty or shorter than the live surface, keep `ground_y`.
fn leftover_ground_or_structure_height(pos: glam::Vec3, ground_y: f32) -> f32 {
    let leftover = gamelogic::object::collide::partition_manager::PARTITION_MANAGER
        .read()
        .ok()
        .map(|pm| pm.get_ground_or_structure_height(pos.x, pos.z));
    match leftover {
        Some(h) if h > ground_y + 1.0e-3 => h,
        _ => ground_y,
    }
}

fn thrust_vel_nearly_zero(v: f32) -> bool {
    v.abs() < 0.001
}

/// Leftover `TheGlobalData->m_gravity` (parseAccelerationReal). Host Y-up == C++ Z.
/// Ctor defaults (-1.0 / leftover INI -9.8) are unparsed; retail GameData.ini is -64/900.
pub(super) fn leftover_loco_gravity() -> f32 {
    leftover_physics_gravity()
}

fn leftover_physics_gravity() -> f32 {
    const RETAIL: f32 = Object::SHOCK_GRAVITY;
    let raw = leftover_global_gravity();
    match raw {
        Some(g) if g.is_finite() && g < 0.0 && (g + 1.0).abs() > 1e-4 && (g + 9.8).abs() > 1e-3 => {
            g
        }
        _ => RETAIL,
    }
}

fn leftover_global_gravity() -> Option<f32> {
    if let Some(data) = game_engine::common::ini::get_global_data() {
        return Some(data.read().gravity);
    }
    game_engine::common::global_data::read_safe()
        .ok()
        .map(|data| data.gravity)
}

/// Leftover `TheGlobalData->m_groundStiffness`. Ctor 0.5 → retail 0.8.
pub(super) fn leftover_ground_stiffness() -> f32 {
    sanitize_stiffness(leftover_global_stiffness_pair().0, Object::GROUND_STIFFNESS)
}

/// Leftover `TheGlobalData->m_structureStiffness`. Ctor 0.5 / leftover 1.0 → retail 0.3.
pub(super) fn leftover_structure_stiffness() -> f32 {
    sanitize_stiffness(
        leftover_global_stiffness_pair().1,
        Object::STRUCTURE_STIFFNESS,
    )
}

fn leftover_global_stiffness_pair() -> (Option<f32>, Option<f32>) {
    if let Ok(data) = game_engine::common::global_data::read_safe() {
        return (Some(data.ground_stiffness), Some(data.structure_stiffness));
    }
    if let Some(data) = game_engine::common::ini::get_global_data() {
        let data = data.read();
        return (Some(data.ground_stiffness), Some(data.structure_stiffness));
    }
    (None, None)
}

fn sanitize_stiffness(raw: Option<f32>, retail: f32) -> f32 {
    match raw {
        Some(s) if s.is_finite() && (s - 0.5).abs() > 1e-6 && (s - 1.0).abs() > 1e-6 => {
            s.clamp(0.01, 0.99)
        }
        _ => retail.clamp(0.01, 0.99),
    }
}

/// C++ `Locomotor::getSurfaceHtAtPt` via leftover `TheTerrainLogic`.
/// Host XZ maps to leftover XY. Empty leftover terrain keeps `ground_y`.
fn leftover_surface_ht_at_pt(pos: glam::Vec3, ground_y: f32) -> f32 {
    let Some(terrain) = gamelogic::helpers::TheTerrainLogic::get() else {
        return ground_y;
    };
    let mut water_z = 0.0;
    let mut terrain_z = 0.0;
    if terrain.is_underwater(pos.x, pos.z, Some(&mut water_z), Some(&mut terrain_z)) {
        return water_z;
    }
    if terrain_z.abs() > 1.0e-3 {
        terrain_z
    } else {
        ground_y
    }
}

/// C++ `handleBehaviorZ` `Z_SMOOTH_RELATIVE_TO_HIGHEST_LAYER`
/// (`Locomotor.cpp:2248-2285`): if leftover terrain has a bridge/wall
/// above ground, lift off that layer (clip=false). Empty leftover keeps
/// `ground_y` (same fail-closed as `leftover_surface_ht_at_pt`).
fn leftover_highest_layer_height(pos: glam::Vec3, ground_y: f32) -> f32 {
    let Some(terrain) = gamelogic::helpers::TheTerrainLogic::get() else {
        return ground_y;
    };
    let cpp = gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y);
    let layer = terrain.get_highest_layer_for_destination(&cpp);
    let h = leftover_layer_height_unclipped(pos, layer)
        .unwrap_or_else(|| terrain.get_layer_height(pos.x, pos.z, layer));
    if h > ground_y + 1.0e-3 {
        h
    } else {
        leftover_surface_ht_at_pt(pos, ground_y)
    }
}

fn leftover_layer_height_unclipped(
    pos: glam::Vec3,
    layer: gamelogic::common::PathfindLayerEnum,
) -> Option<f32> {
    let terrain = gamelogic::terrain::get_terrain_logic();
    let guard = terrain.try_read().ok()?;
    let path_layer = match layer {
        gamelogic::common::PathfindLayerEnum::Air => gamelogic::path::PathfindLayerEnum::Top,
        gamelogic::common::PathfindLayerEnum::Tunnel
        | gamelogic::common::PathfindLayerEnum::Water
        | gamelogic::common::PathfindLayerEnum::Last => gamelogic::path::PathfindLayerEnum::Ground,
        other => gamelogic::path::PathfindLayerEnum::from_u32(other as u32),
    };
    Some(guard.get_layer_height(pos.x, pos.z, path_layer, None, false))
}

/// Leftover `calc_direction_to_apply_thrust` / C++ `Locomotor.cpp:175-250`.
/// Host Y-up: gravity is added to velocity.y (C++ velocity.z).
fn leftover_calc_direction_to_apply_thrust(
    obj_pos: glam::Vec3,
    cur_vel: glam::Vec3,
    goal_pos: glam::Vec3,
    max_accel: f32,
    forward: glam::Vec3,
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
    cur_vel.y += leftover_loco_gravity();

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
