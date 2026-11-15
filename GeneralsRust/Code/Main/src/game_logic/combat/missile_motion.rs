//! Missile-owned use of the same locomotor and physics kernels as live Objects.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MissileMotion {
    actor: Box<Object>,
    gravity: f32,
    no_turn_distance: f32,
    previous_position: Vec3,
    dive_distance: f32,
    /// ATTACK_NOTURN executes once before ATTACK, even for zero distance.
    #[serde(default = "legacy_attack_turn_enabled")]
    attack_turn_enabled: bool,
    /// PhysicsBehavior's UPDATE_EVER_RUN, retained with the airborne latch.
    #[serde(default)]
    physics_initialized: bool,
}

// Snapshots written before ATTACK_NOTURN was represented turned as soon as
// the distance expired; retain that continuation when the field is absent.
fn legacy_attack_turn_enabled() -> bool { true }

impl MissileMotion {
    pub(crate) fn validate(&self) -> bool {
        self.gravity.is_finite() && self.no_turn_distance.is_finite()
            && self.previous_position.is_finite() && self.dive_distance.is_finite()
            && self.actor.get_position().is_finite()
            && self.actor.get_transform_matrix().is_finite()
            && self.actor.movement.velocity.is_finite()
            && self.actor.physics_accel.is_finite()
    }
    pub(super) fn position(&self) -> Vec3 { self.actor.get_position() }
    pub(super) fn velocity(&self) -> Vec3 { self.actor.movement.velocity }

    pub(crate) fn launch(
        id: ObjectId,
        profile: &crate::game_logic::projectile_definitions::ProjectileProfile,
        transform: glam::Mat4,
        source_velocity: Vec3,
        target: Vec3,
        weapon_speed: f32,
    ) -> Option<Self> {
        let locomotor = profile.locomotor.as_ref()?;
        let launch = profile.missile_launch.as_ref()?;
        let physics = profile.behaviors.iter().find_map(|module| module.physics.as_ref())?;
        let mut template = ThingTemplate::new("MissileMotion");
        template.add_kind_of(KindOf::Projectile);
        let mut actor = Box::new(Object::new(template, id, Team::Neutral));
        crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(&mut actor, locomotor);
        actor.physics_rules.gravity = profile.gravity;
        actor.physics_mass = physics.mass;
        actor.forward_friction = physics.forward_friction;
        actor.lateral_friction = physics.lateral_friction;
        actor.z_friction = physics.z_friction;
        actor.aerodynamic_friction = physics.aerodynamic_friction;
        let position = transform.w_axis.truncate();
        let mut direction = transform.x_axis.truncate().normalize_or_zero();
        if direction == Vec3::ZERO { return None; }
        let delta = target - position;
        direction.y += 2.0 * delta.y.max(0.0) / Vec3::new(delta.x, 0.0, delta.z).length().max(1.0);
        direction = direction.normalize_or_zero();
        let basis = glam::Mat4::from_cols(glam::Vec4::X, glam::Vec4::Z, glam::Vec4::Y, glam::Vec4::W);
        let source = gamelogic::common::build_transform_matrix(basis.transform_point3(position), basis.transform_vector3(direction));
        actor.set_transform_matrix(basis * source * basis);
        actor.movement.velocity = source_velocity;
        let initial = if launch.use_weapon_speed { weapon_speed } else { launch.initial_velocity_per_frame * 30.0 };
        if launch.use_weapon_speed {
            actor.movement.max_speed = actor.movement.max_speed.min(weapon_speed);
            actor.movement.max_speed_damaged = actor.movement.max_speed_damaged.min(weapon_speed);
            actor.movement.acceleration = actor.movement.acceleration.min(weapon_speed * 30.0);
            actor.movement.acceleration_damaged = actor.movement.acceleration_damaged.min(weapon_speed * 30.0);
        }
        if initial > 0.0 {
            actor.apply_motive_force(direction * (actor.physics_get_mass() * initial), 0);
        }
        Some(Self { actor, gravity: profile.gravity, no_turn_distance: launch.distance_before_turning,
            previous_position: position, dive_distance: launch.distance_before_diving,
            attack_turn_enabled: false, physics_initialized: false })
    }

    pub(super) fn advance(
        &mut self, goal: Vec3, ignited: bool, powered: bool, attack: bool, braking: bool, frame: u32,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
    ) -> bool {
        self.actor.physics_rules.gravity = self.gravity;
        let pos = self.actor.get_position();
        if let Some(terrain) = terrain {
            self.actor.ground_height = terrain.raw_ground(pos, self.actor.ground_height);
        }
        if !self.physics_initialized {
            // PhysicsUpdate initializes this latch from the start position on
            // its first frame, before it integrates the missile's velocity.
            self.actor.was_airborne_last_frame = pos.y > self.actor.ground_height;
            self.physics_initialized = true;
        }
        if ignited && self.no_turn_distance > 0.0 {
            self.no_turn_distance -= pos.distance(self.previous_position);
            self.previous_position = pos;
        }
        let old_turn = self.actor.movement.turn_rate;
        let old_turn_damaged = self.actor.movement.turn_rate_damaged;
        let old_accel = self.actor.movement.acceleration;
        let old_accel_damaged = self.actor.movement.acceleration_damaged;
        if !powered {
            self.actor.movement.acceleration = 0.0;
            self.actor.movement.acceleration_damaged = 0.0;
        }
        if !powered || !(braking || (attack && self.attack_turn_enabled && self.no_turn_distance <= 0.0)) {
            self.actor.movement.turn_rate = 0.0;
            self.actor.movement.turn_rate_damaged = 0.0;
        }
        if attack && !braking && self.actor.loco_preferred_height > 0.0 {
            let horizontal = Vec3::new(goal.x - pos.x, 0.0, goal.z - pos.z);
            if horizontal.length_squared() < self.dive_distance * self.dive_distance {
                self.actor.precise_z_pos = true;
            }
        }
        // doAttackState transitions only after using this frame's turn cap.
        if attack && !braking && self.no_turn_distance <= 0.0 {
            self.attack_turn_enabled = true;
        }
        let speed = self.actor.movement.max_speed;
        let ground = terrain.map_or(self.actor.ground_height, |view| view.highest_surface(&self.actor));
        self.actor.move_towards_thrust_with_status_and_effects(goal, pos.distance(goal), speed,
            1.0 / 30.0, frame, braking, ground, &mut Default::default());
        self.actor.movement.turn_rate = old_turn;
        self.actor.movement.turn_rate_damaged = old_turn_damaged;
        self.actor.movement.acceleration = old_accel;
        self.actor.movement.acceleration_damaged = old_accel_damaged;
        let braking_speed = (self.actor.movement.velocity.length() / 30.0).max(10.0 / 30.0);
        self.actor.integrate_physics_force_phase(frame);
        let mut next = if braking {
            // Locomotor's close-in movement is direct; Physics does not translate braking projectiles.
            let delta = goal - pos;
            if delta.length_squared() > 0.001 * 0.001 {
                pos + delta.clamp_length_max(braking_speed)
            } else {
                pos
            }
        } else {
            pos + self.actor.movement.velocity / 30.0
        };
        // PhysicsUpdate clamps against the receiving terrain after Euler, then
        // reports a ground collision only on an airborne-to-ground transition.
        let ground = terrain.map_or(self.actor.ground_height, |view| {
            view.raw_ground(next, self.actor.ground_height)
        });
        self.actor.ground_height = ground;
        if next.y <= ground {
            self.actor.movement.velocity.y = (self.actor.movement.velocity.y
                + (ground - next.y) * 30.0).min(0.0);
            next.y = ground;
        }
        let airborne = next.y > ground;
        let ground_contact = self.actor.was_airborne_last_frame && !airborne
            && !self.actor.immune_to_falling_damage;
        self.actor.was_airborne_last_frame = airborne;
        self.actor.set_position(next);
        self.actor.physics_accel = Vec3::ZERO;
        ground_contact
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignition_and_first_attack_do_not_turn_and_save_preserves_next_action() {
        let mut template = ThingTemplate::new("MotionTestMissile");
        template.add_kind_of(KindOf::Projectile);
        let mut actor = Box::new(Object::new(template, ObjectId(1), Team::Neutral));
        actor.set_position(Vec3::new(0.0, 50.0, 0.0));
        actor.movement.velocity = Vec3::Z * 30.0;
        actor.movement.max_speed = 30.0;
        actor.movement.turn_rate = 3.0;
        actor.movement.turn_rate_damaged = 3.0;
        let mut motion = MissileMotion {
            actor,
            gravity: 0.0,
            no_turn_distance: 0.0,
            previous_position: Vec3::new(0.0, 50.0, 0.0),
            dive_distance: 0.0,
            attack_turn_enabled: false,
            physics_initialized: false,
        };
        let goal = Vec3::new(0.0, 50.0, 100.0);
        let forward = motion.actor.get_transform_matrix().x_axis;
        motion.advance(goal, true, true, false, false, 1, None);
        assert_eq!(motion.actor.get_transform_matrix().x_axis, forward);
        motion.advance(goal, true, true, true, false, 2, None);
        assert_eq!(motion.actor.get_transform_matrix().x_axis, forward);

        let saved = serde_json::to_vec(&motion).unwrap();
        let mut restored: MissileMotion = serde_json::from_slice(&saved).unwrap();
        motion.advance(goal, true, true, true, false, 3, None);
        restored.advance(goal, true, true, true, false, 3, None);
        assert_ne!(motion.actor.get_transform_matrix().x_axis, forward);
        assert_eq!(motion.actor.get_transform_matrix(), restored.actor.get_transform_matrix());
        assert_eq!(motion.velocity(), restored.velocity());
        // CPRJ's older JSON motion payload did not contain this field.
        let mut old_json = serde_json::to_value(&motion).unwrap();
        old_json.as_object_mut().unwrap().remove("attack_turn_enabled");
        let mut old_restored: MissileMotion = serde_json::from_value(old_json).unwrap();
        motion.advance(goal, true, true, true, false, 4, None);
        old_restored.advance(goal, true, true, true, false, 4, None);
        assert_eq!(motion.actor.get_transform_matrix(), old_restored.actor.get_transform_matrix());
        assert_eq!(motion.velocity(), old_restored.velocity());
    }

    #[test]
    fn airborne_missile_reports_ground_contact_at_clamped_pose() {
        let mut template = ThingTemplate::new("GroundContactMissile");
        template.add_kind_of(KindOf::Projectile);
        let mut actor = Box::new(Object::new(template, ObjectId(2), Team::Neutral));
        actor.set_position(Vec3::new(0.0, 1.0, 0.0));
        actor.movement.velocity = Vec3::new(0.0, -60.0, 0.0);
        actor.movement.max_speed = 60.0;
        actor.braking = 0.0;
        let mut motion = MissileMotion {
            actor,
            gravity: 0.0,
            no_turn_distance: 0.0,
            previous_position: Vec3::new(0.0, 1.0, 0.0),
            dive_distance: 0.0,
            attack_turn_enabled: true,
            physics_initialized: false,
        };
        assert!(motion.advance(Vec3::new(10.0, 0.0, 0.0), true, false, true, false, 1, None));
        assert_eq!(motion.position().y, 0.0);
        assert!(motion.velocity().y <= 0.0);
    }

    fn falling_projectile(start: Vec3, goal: Vec3) -> Projectile {
        use crate::game_logic::weapon_bootstrap::{
            HostMissileFlight, HostMissilePhase, HostProjectileFlight, HostProjectileLifecycle,
        };
        let mut template = ThingTemplate::new("GroundImpactMissile");
        template.add_kind_of(KindOf::Projectile);
        let mut actor = Box::new(Object::new(template, ObjectId(3), Team::Neutral));
        actor.set_position(start);
        actor.movement.velocity = Vec3::new(0.0, -60.0, 0.0);
        actor.movement.max_speed = 60.0;
        actor.movement.acceleration = 0.0;
        actor.movement.acceleration_damaged = 0.0;
        actor.braking = 0.0;
        let mut projectile = Projectile::new(ObjectId(3), start, goal, 10.0,
            DamageType::Explosive, ObjectId(1), None);
        projectile.speed = 60.0;
        projectile.flight = Some(HostProjectileFlight::Missile(HostMissileFlight {
            lock_distance: 100.0, ..HostMissileFlight::default()
        }));
        projectile.set_projectile_lifecycle(Some(HostProjectileLifecycle::Missile {
            try_to_follow_target: false, fuel_lifetime_frames: 0,
            detonate_on_no_fuel: false, kill_self_delay_frames: 3,
        }));
        projectile.bind_authored_flight(start, goal, 60.0, None);
        projectile.flight_runtime.missile_armed = true;
        projectile.flight_runtime.missile_phase = HostMissilePhase::Attack;
        projectile.missile_motion = Some(MissileMotion {
            actor, gravity: 0.0, no_turn_distance: 0.0, previous_position: start,
            dive_distance: 0.0, attack_turn_enabled: true, physics_initialized: false,
        });
        projectile
    }

    #[test]
    fn ground_detonation_keeps_missile_for_authored_kill_self_delay() {
        let mut projectile = falling_projectile(Vec3::Y, Vec3::ZERO);
        assert_eq!(projectile.update(1.0 / 30.0, true), ProjectileStep::DetonateAndHold);
        assert_eq!(projectile.position.y, 0.0);
        assert_eq!(projectile.update(1.0 / 30.0, true), ProjectileStep::Hold);
        assert_eq!(projectile.update(1.0 / 30.0, true), ProjectileStep::Hold);
        assert_eq!(projectile.update(1.0 / 30.0, true), ProjectileStep::Remove);
    }

    #[test]
    fn lock_transition_ground_contact_uses_original_coordinate_goal() {
        let mut projectile = falling_projectile(
            Vec3::new(40.0, 1.0, 0.0), Vec3::new(0.0, -15.0, 0.0),
        );
        assert_eq!(projectile.update(1.0 / 30.0, true), ProjectileStep::Alive);
        assert_eq!(projectile.position.y, 0.0);
        assert_eq!(projectile.flight_runtime.missile_phase,
            crate::game_logic::weapon_bootstrap::HostMissilePhase::Kill);
    }

    #[test]
    fn missile_just_above_ground_remains_airborne() {
        let mut projectile = falling_projectile(Vec3::new(0.0, 0.02, 0.0), Vec3::X * 100.0);
        let motion = projectile.missile_motion.as_mut().unwrap();
        motion.actor.movement.velocity = Vec3::ZERO;
        assert!(!motion.advance(Vec3::X * 100.0, true, false, true, false, 1, None));
        assert_eq!(motion.position().y, 0.02);
        assert!(motion.actor.was_airborne_last_frame);
    }
}
