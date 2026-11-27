//! Adapter from simulation missile motion to receiving objects and terrain.
use super::*;
use crate::game_logic::{
    ScriptPresentationEffects, game_logic::FlightTerrainView, partition_manager::PartitionManager,
};
use generals_sim::combat::missile_motion::{MissileBody, MissileBodyState};
use generals_sim::time::{LOGIC_FRAME_TIMESTEP, LOGIC_FRAMES_PER_SECOND};

pub(super) use generals_sim::combat::missile_motion::MissileSteering;

pub(in crate::game_logic) enum MissileInitialVelocity {
    Existing(Vec3),
    InheritAsMotiveForce { distance_per_frame: Vec3 },
}

pub(crate) fn launch(
    actor: &mut Object,
    profile: &crate::game_logic::projectile_definitions::ProjectileProfile,
    transform: glam::Mat4,
    initial_velocity: MissileInitialVelocity,
    target: Vec3,
    weapon_speed: f32,
    frame: u32,
) -> Option<MissileMotion> {
    let locomotor = profile.locomotor.as_ref()?;
    let launch = profile.missile_launch.as_ref()?;
    let physics = profile
        .behaviors
        .iter()
        .find_map(|module| module.physics.as_ref())?;
    // Retain the authored identity of the exact selected binding so Object
    // load-postprocess can rebind its immutable locomotor facts.
    let locomotor_name = profile
        .locomotor_sets
        .iter()
        .find(|set| set.name.eq_ignore_ascii_case("SET_NORMAL"))?
        .members
        .iter()
        .find(|(_, binding)| binding.as_ref() == Some(locomotor))
        .map(|(name, _)| name.as_str())?;
    if actor.cur_locomotor_name.as_deref() != Some(locomotor_name) {
        actor.cur_locomotor_name = Some(locomotor_name.to_owned());
    }
    crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(actor, locomotor);
    actor.physics_rules.gravity = profile.gravity;
    actor.physics_mass = physics.mass;
    actor.forward_friction = physics.forward_friction;
    actor.lateral_friction = physics.lateral_friction;
    actor.z_friction = physics.z_friction;
    actor.aerodynamic_friction = physics.aerodynamic_friction;
    let position = transform.w_axis.truncate();
    let direction = MissileMotion::launch_pose(transform, target)?
        .x_axis
        .truncate()
        .normalize_or_zero();
    match initial_velocity {
        MissileInitialVelocity::Existing(velocity) => actor.movement.velocity = velocity,
        MissileInitialVelocity::InheritAsMotiveForce { distance_per_frame } => {
            // Missile motion uses distance per second. The payload's
            // inherited force enters the first Physics update alongside
            // MissileAI's authored InitialVelocity force.
            let force = distance_per_frame / LOGIC_FRAME_TIMESTEP * actor.physics_get_mass();
            actor.apply_motive_force(force, frame);
        }
    }
    let initial = if launch.use_weapon_speed {
        weapon_speed
    } else {
        launch.initial_velocity_per_frame * LOGIC_FRAMES_PER_SECOND
    };
    if launch.use_weapon_speed {
        actor.movement.max_speed = actor.movement.max_speed.min(weapon_speed);
        actor.movement.max_speed_damaged = actor.movement.max_speed_damaged.min(weapon_speed);
        actor.movement.acceleration = actor
            .movement
            .acceleration
            .min(weapon_speed * LOGIC_FRAMES_PER_SECOND);
        actor.movement.acceleration_damaged = actor
            .movement
            .acceleration_damaged
            .min(weapon_speed * LOGIC_FRAMES_PER_SECOND);
    }
    if initial > 0.0 {
        actor.apply_motive_force(direction * (actor.physics_get_mass() * initial), frame);
    }
    Some(MissileMotion::new(
        profile.gravity,
        launch.distance_before_turning,
        position,
        launch.distance_before_diving,
    ))
}

struct ObjectMissileBody<'actor, 'terrain, 'world, 'effects, 'recipient> {
    actor: &'actor mut Object,
    terrain: Option<&'terrain FlightTerrainView<'world>>,
    effects: &'effects mut ScriptPresentationEffects<'recipient>,
}

impl MissileBody for ObjectMissileBody<'_, '_, '_, '_, '_> {
    fn position(&self) -> Vec3 {
        self.actor.get_position()
    }

    fn state(&mut self) -> MissileBodyState<'_> {
        MissileBodyState {
            movement: &mut self.actor.movement,
            ground_height: &mut self.actor.ground_height,
            was_airborne: &mut self.actor.was_airborne_last_frame,
            precise_elevation: &mut self.actor.precise_z_pos,
            preferred_height: self.actor.loco_preferred_height,
            min_speed: self.actor.min_speed,
            immune_to_falling_damage: self.actor.immune_to_falling_damage,
        }
    }

    fn set_gravity(&mut self, gravity: f32) {
        self.actor.physics_rules.gravity = gravity;
    }

    fn raw_ground(&self, position: Vec3, fallback: f32) -> f32 {
        self.terrain
            .map_or(fallback, |view| view.raw_ground(position, fallback))
    }

    fn highest_surface(&self) -> f32 {
        self.terrain.map_or(self.actor.ground_height, |view| {
            view.highest_surface(self.actor)
        })
    }

    fn apply_thrust(
        &mut self,
        goal: Vec3,
        distance: f32,
        speed: f32,
        frame: u32,
        braking: bool,
        ground: f32,
    ) {
        self.actor.move_towards_thrust_with_status_and_effects(
            goal,
            distance,
            speed,
            LOGIC_FRAME_TIMESTEP,
            frame,
            braking,
            ground,
            self.effects,
        );
    }

    fn integrate_force(&mut self, frame: u32) {
        self.actor.integrate_physics_force_phase(frame);
    }

    fn finish_movement(&mut self, position: Vec3, frame: u32) {
        self.actor
            .set_position_with_effects(position, frame, self.effects);
        self.actor.physics_accel = Vec3::ZERO;
    }
}

fn with_missile_body<R>(
    actor: &mut Object,
    terrain: Option<&FlightTerrainView<'_>>,
    partition: Option<&mut PartitionManager>,
    advance: impl FnOnce(&mut ObjectMissileBody<'_, '_, '_, '_, '_>) -> R,
) -> R {
    let mut effects = ScriptPresentationEffects::default();
    if let Some(partition) = partition {
        let mut effects = effects.reborrow_for_object(partition);
        advance(&mut ObjectMissileBody {
            actor,
            terrain,
            effects: &mut effects,
        })
    } else {
        advance(&mut ObjectMissileBody {
            actor,
            terrain,
            effects: &mut effects,
        })
    }
}

pub(super) trait MissileMotionForObject {
    fn advance(
        &mut self,
        actor: &mut Object,
        goal: Vec3,
        ignited: bool,
        powered: bool,
        steering: MissileSteering,
        frame: u32,
        terrain: Option<&FlightTerrainView<'_>>,
        partition: Option<&mut PartitionManager>,
    ) -> bool;

    fn advance_held(
        &mut self,
        actor: &mut Object,
        goal: Vec3,
        frame: u32,
        terrain: Option<&FlightTerrainView<'_>>,
        partition: Option<&mut PartitionManager>,
    ) -> bool;
}

impl MissileMotionForObject for MissileMotion {
    fn advance(
        &mut self,
        actor: &mut Object,
        goal: Vec3,
        ignited: bool,
        powered: bool,
        steering: MissileSteering,
        frame: u32,
        terrain: Option<&FlightTerrainView<'_>>,
        partition: Option<&mut PartitionManager>,
    ) -> bool {
        with_missile_body(actor, terrain, partition, |body| {
            self.advance_body(body, goal, ignited, powered, steering, frame)
        })
    }

    fn advance_held(
        &mut self,
        actor: &mut Object,
        goal: Vec3,
        frame: u32,
        terrain: Option<&FlightTerrainView<'_>>,
        partition: Option<&mut PartitionManager>,
    ) -> bool {
        with_missile_body(actor, terrain, partition, |body| {
            self.advance_held_body(body, goal, frame)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::weapon_bootstrap::HostProjectileLifecycle;

    fn body_fixture(
        position: Vec3,
        turn_enabled: bool,
        controls: Option<(bool, bool, bool, bool)>,
    ) -> MissileMotion {
        serde_json::from_value(serde_json::json!({
            "gravity": 0.0, "no_turn_distance": 0.0, "previous_position": position,
            "dive_distance": 0.0, "attack_turn_enabled": turn_enabled, "physics_initialized": false,
            "last_controls": controls, "idle_position": null,
        }))
        .unwrap()
    }

    #[test]
    fn ignition_and_first_attack_do_not_turn_and_save_preserves_next_action() {
        let mut template = ThingTemplate::new("MotionTestMissile");
        template.add_kind_of(KindOf::Projectile);
        let mut actor = Object::new(template, ObjectId(1), Team::Neutral);
        actor.set_position(Vec3::new(0.0, 50.0, 0.0));
        actor.movement.velocity = Vec3::Z * 30.0;
        actor.movement.max_speed = 30.0;
        actor.movement.turn_rate = 3.0;
        actor.movement.turn_rate_damaged = 3.0;
        let mut motion = body_fixture(Vec3::new(0.0, 50.0, 0.0), false, None);
        let goal = Vec3::new(0.0, 50.0, 100.0);
        let forward = actor.get_transform_matrix().x_axis;
        motion.advance(
            &mut actor,
            goal,
            true,
            true,
            MissileSteering::NoTurn,
            1,
            None,
            None,
        );
        assert_eq!(actor.get_transform_matrix().x_axis, forward);
        motion.advance(
            &mut actor,
            goal,
            true,
            true,
            MissileSteering::Attack,
            2,
            None,
            None,
        );
        assert_eq!(actor.get_transform_matrix().x_axis, forward);

        let saved = serde_json::to_vec(&motion).unwrap();
        let mut restored: MissileMotion = serde_json::from_slice(&saved).unwrap();
        let mut restored_actor = actor.clone();
        motion.advance(
            &mut actor,
            goal,
            true,
            true,
            MissileSteering::Attack,
            3,
            None,
            None,
        );
        restored.advance(
            &mut restored_actor,
            goal,
            true,
            true,
            MissileSteering::Attack,
            3,
            None,
            None,
        );
        assert_ne!(actor.get_transform_matrix().x_axis, forward);
        assert_eq!(
            actor.get_transform_matrix(),
            restored_actor.get_transform_matrix()
        );
        assert_eq!(actor.movement.velocity, restored_actor.movement.velocity);
    }

    #[test]
    fn airborne_missile_reports_ground_contact_at_clamped_pose() {
        let mut template = ThingTemplate::new("GroundContactMissile");
        template.add_kind_of(KindOf::Projectile);
        let mut actor = Object::new(template, ObjectId(2), Team::Neutral);
        actor.set_position(Vec3::new(0.0, 1.0, 0.0));
        actor.movement.velocity = Vec3::new(0.0, -60.0, 0.0);
        actor.movement.max_speed = 60.0;
        actor.braking = 0.0;
        let mut motion = body_fixture(Vec3::new(0.0, 1.0, 0.0), true, None);
        assert!(motion.advance(
            &mut actor,
            Vec3::new(10.0, 0.0, 0.0),
            true,
            false,
            MissileSteering::Attack,
            1,
            None,
            None
        ));
        assert_eq!(actor.get_position().y, 0.0);
        assert!(actor.movement.velocity.y <= 0.0);
    }

    fn falling_projectile(start: Vec3, goal: Vec3) -> (Projectile, Object) {
        use crate::game_logic::weapon_bootstrap::{
            HostMissileFlight, HostMissilePhase, HostProjectileFlight,
        };
        let mut template = ThingTemplate::new("GroundImpactMissile");
        template.add_kind_of(KindOf::Projectile);
        let mut actor = Object::new(template, ObjectId(3), Team::Neutral);
        actor.set_position(start);
        actor.movement.velocity = Vec3::new(0.0, -60.0, 0.0);
        actor.movement.max_speed = 60.0;
        actor.movement.acceleration = 0.0;
        actor.movement.acceleration_damaged = 0.0;
        actor.braking = 0.0;
        let mut projectile = Projectile::new(
            ObjectId(3),
            start,
            goal,
            10.0,
            DamageType::Explosive,
            ObjectId(1),
            None,
        );
        projectile.speed = 60.0;
        projectile.flight = Some(HostProjectileFlight::Missile(HostMissileFlight {
            lock_distance: 100.0,
            ..HostMissileFlight::default()
        }));
        projectile.set_projectile_lifecycle(Some(HostProjectileLifecycle::Missile {
            try_to_follow_target: false,
            fuel_lifetime_frames: 0,
            detonate_on_no_fuel: false,
            kill_self_delay_frames: 3,
        }));
        projectile.bind_authored_flight(start, goal, 60.0, |_, _| None);
        projectile.flight_runtime.missile_armed = true;
        projectile.flight_runtime.missile_phase = HostMissilePhase::Attack;
        projectile.missile_motion =
            Some(body_fixture(start, true, Some((true, true, true, false))));
        (projectile, actor)
    }

    #[test]
    fn ground_detonation_keeps_missile_for_authored_kill_self_delay() {
        let (mut projectile, mut actor) = falling_projectile(Vec3::Y, Vec3::ZERO);
        assert_eq!(
            advance_projectile_flight(
                &mut projectile,
                1.0 / 30.0,
                true,
                None,
                None,
                Some(&mut actor),
                None,
                None,
                &mut ()
            ),
            ProjectileStep::Detonate {
                retain_actor: true,
                pre_physics_pose: None
            }
        );
        assert_eq!(projectile.position.y, 0.0);
        assert_eq!(
            advance_projectile_flight(
                &mut projectile,
                1.0 / 30.0,
                true,
                None,
                None,
                Some(&mut actor),
                None,
                None,
                &mut ()
            ),
            ProjectileStep::Hold
        );
        assert_eq!(
            advance_projectile_flight(
                &mut projectile,
                1.0 / 30.0,
                true,
                None,
                None,
                Some(&mut actor),
                None,
                None,
                &mut ()
            ),
            ProjectileStep::Hold
        );
        assert_eq!(
            advance_projectile_flight(
                &mut projectile,
                1.0 / 30.0,
                true,
                None,
                None,
                Some(&mut actor),
                None,
                None,
                &mut ()
            ),
            ProjectileStep::Remove
        );
    }

    #[test]
    fn fuel_detonation_carries_the_pose_before_held_physics_advances() {
        let (mut projectile, mut actor) =
            falling_projectile(Vec3::new(20.0, 60.0, 30.0), Vec3::ZERO);
        let pose = actor.get_transform_matrix();
        projectile.projectile_lifecycle = Some(HostProjectileLifecycle::Missile {
            try_to_follow_target: false,
            fuel_lifetime_frames: 1,
            detonate_on_no_fuel: true,
            kill_self_delay_frames: 3,
        });
        projectile.lifetime = 1.0;

        let step = advance_projectile_flight(

            &mut projectile,
            1.0 / 30.0,
            true,
            None,
            None,
            Some(&mut actor),
            None,
            None,
            &mut (),
        );
        assert_eq!(
            step,
            ProjectileStep::Detonate {
                retain_actor: true,
                pre_physics_pose: Some(pose),
            }
        );
        assert_ne!(actor.get_position(), pose.w_axis.truncate());
    }

    #[test]
    fn lock_transition_ground_contact_uses_original_coordinate_goal() {
        let (mut projectile, mut actor) =
            falling_projectile(Vec3::new(40.0, 1.0, 0.0), Vec3::new(0.0, -15.0, 0.0));
        assert_eq!(
            advance_projectile_flight(
                &mut projectile,
                1.0 / 30.0,
                true,
                None,
                None,
                Some(&mut actor),
                None,
                None,
                &mut ()
            ),
            ProjectileStep::Alive
        );
        assert_eq!(projectile.position.y, 0.0);
        assert_eq!(
            projectile.flight_runtime.missile_phase,
            crate::game_logic::weapon_bootstrap::HostMissilePhase::Kill
        );
    }

    #[test]
    fn missile_just_above_ground_remains_airborne() {
        let (mut projectile, mut actor) =
            falling_projectile(Vec3::new(0.0, 0.02, 0.0), Vec3::X * 100.0);
        let motion = projectile.missile_motion.as_mut().unwrap();
        actor.movement.velocity = Vec3::ZERO;
        assert!(!motion.advance(
            &mut actor,
            Vec3::X * 100.0,
            true,
            false,
            MissileSteering::Attack,
            1,
            None,
            None
        ));
        assert_eq!(actor.get_position().y, 0.02);
        assert!(actor.was_airborne_last_frame);
    }
}
