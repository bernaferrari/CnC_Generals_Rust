//! Missile-owned use of the same locomotor and physics kernels as live Objects.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MissileMotion {
    actor: Box<Object>,
    gravity: f32,
    no_turn_distance: f32,
    previous_position: Vec3,
    dive_distance: f32,
}

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
            actor.movement.max_speed = weapon_speed;
            actor.movement.max_speed_damaged = weapon_speed;
            actor.movement.acceleration = weapon_speed * 30.0;
            actor.movement.acceleration_damaged = weapon_speed * 30.0;
        }
        if initial > 0.0 {
            actor.apply_motive_force(direction * (actor.physics_get_mass() * initial), 0);
        }
        Some(Self { actor, gravity: profile.gravity, no_turn_distance: launch.distance_before_turning,
            previous_position: position, dive_distance: launch.distance_before_diving })
    }

    pub(super) fn advance(
        &mut self, goal: Vec3, armed: bool, can_turn: bool, braking: bool, frame: u32,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
    ) {
        self.actor.physics_rules.gravity = self.gravity;
        let pos = self.actor.get_position();
        if let Some(terrain) = terrain {
            self.actor.ground_height = terrain.raw_ground(pos, self.actor.ground_height);
        }
        if armed && self.no_turn_distance > 0.0 {
            self.no_turn_distance -= pos.distance(self.previous_position);
            self.previous_position = pos;
        }
        let old_turn = self.actor.movement.turn_rate;
        let old_turn_damaged = self.actor.movement.turn_rate_damaged;
        let old_accel = self.actor.movement.acceleration;
        let old_accel_damaged = self.actor.movement.acceleration_damaged;
        if !armed {
            self.actor.movement.acceleration = 0.0;
            self.actor.movement.acceleration_damaged = 0.0;
        }
        if !can_turn || self.no_turn_distance > 0.0 {
            self.actor.movement.turn_rate = 0.0;
            self.actor.movement.turn_rate_damaged = 0.0;
        }
        if self.actor.loco_preferred_height > 0.0 {
            let horizontal = Vec3::new(goal.x - pos.x, 0.0, goal.z - pos.z);
            if horizontal.length_squared() < self.dive_distance * self.dive_distance {
                self.actor.precise_z_pos = true;
            }
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
        let next = if braking {
            // Locomotor's close-in movement is direct; Physics does not translate braking projectiles.
            let delta = goal - pos;
            pos + delta.clamp_length_max(braking_speed)
        } else {
            pos + self.actor.movement.velocity / 30.0
        };
        self.actor.set_position(next);
        self.actor.physics_accel = Vec3::ZERO;
    }
}
