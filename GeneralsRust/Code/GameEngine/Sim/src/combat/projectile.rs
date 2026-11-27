//! Live projectile state and flight for the receiving simulation.

use super::missile_motion::{MissileMotion, MissileSteering};
use super::projectile_contact::ProjectileCollision;
use super::projectile_flight::{
    MissileIgnition, MissilePhase, ProjectileFlight, ProjectileFlightState, ProjectileKinematics,
    ProjectileStep, VirtualFlightStep, missile_fuel_expired, missile_inside_lock_distance,
    projectile_forward_transform,
};
use super::projectile_impact::ProjectileWarhead;
use super::projectile_lifecycle::ProjectileLifecycle;
use crate::damage::{CombatDamage, DeathOutcome};
use crate::object::{GeometryShape, ObjectStatusTypes, VeterancyLevel};
use crate::pathfinding::PATHFIND_CELL_SIZE_F;
use crate::time::LOGIC_FRAME_TIMESTEP;
use crate::weapon::{WeaponAffectsMask, WeaponCollideMask};
use generals_game_domain::{ObjectId, Team};
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

/// Default collision mask used by a shot without an authored weapon.
pub const DEFAULT_PROJECTILE_COLLISIONS: u32 =
    WeaponCollideMask::STRUCTURES | WeaponCollideMask::WALLS;

/// Live queries and effects used by a single synchronous projectile update.
pub trait ProjectileFlightWorld {
    fn missile_kinematics(&self) -> Option<(Vec3, Vec3)>;
    fn missile_transform(&self) -> Option<Mat4>;
    fn missile_max_speed(&self) -> f32;
    fn missile_close_enough_distance(&self) -> f32;
    fn set_missile_position(&mut self, position: Vec3, frame: u32);
    fn advance_missile(
        &mut self,
        motion: &mut MissileMotion,
        goal: Vec3,
        ignited: bool,
        powered: bool,
        steering: MissileSteering,
        frame: u32,
    ) -> bool;
    fn advance_held_missile(&mut self, motion: &mut MissileMotion, goal: Vec3, frame: u32) -> bool;
    fn bridge_deck(&self, position: Vec3, old_layer: u8, armed: bool) -> (Vec3, u8);
    fn publish_pose(&mut self, projectile: &Projectile);
    fn ignition_fx(&mut self, name: &str, projectile: &Projectile);
    fn attach_exhaust(&mut self, projectile: &Projectile) -> Option<u32>;
}

/// Registered target data sampled at the start of the missile callback.
pub struct MissileKillTarget {
    pub origin: Vec3,
    pub edge_distance_squared: f32,
}

// C++ ownership: Projectile, DumbProjectileBehavior, and MissileAIUpdate flight state.

/// C++ Weapon::newProjectileFired binds the exact firing Weapon template.
/// Empty names are authoritative; None on Projectile denotes unbound legacy input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectileStreamBinding {
    pub slot: u8,
    pub name: String,
}

/// One live shot with its launch context and continuation state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projectile {
    pub id: ObjectId,
    pub position: Vec3,
    pub velocity: Vec3,
    pub target_position: Vec3,
    pub damage: f32,
    pub damage_type: CombatDamage,
    pub shooter_id: ObjectId,
    pub target_id: Option<ObjectId>,
    /// Runtime velocity in world distance per second; pending Weapon.ini
    /// distance-per-frame values are normalized before this object is created.
    pub speed: f32,
    pub lifetime: f32,
    /// Parsed Object INI behavior governing lifetime. `None` means the
    /// ProjectileObject was unresolved or does not expose a supported behavior;
    /// that intentionally has no invented generic timeout.
    pub projectile_lifecycle: Option<ProjectileLifecycle>,
    /// C++ `MissileAIUpdate::KILL_SELF` entry frame after a followed target disappears.
    pub missile_kill_self_started_frame: Option<u32>,
    /// Parsed lifetime/fuel duration. Zero means unlimited or unresolved.
    pub max_lifetime: f32,
    pub is_homing: bool,
    pub explosion_radius: f32,
    /// C++ DeathType carried to kill application.
    pub death_type: DeathOutcome,
    /// C++ ProjectileObject for presentation mesh key.
    pub projectile_object_name: String,
    /// Geometry captured from the receiving projectile template at admission.
    /// Framed separately to preserve the positional projectile snapshot.
    #[serde(skip)]
    pub collision_geometry: Option<GeometryShape>,
    /// C++ Weapon.ini ProjectileDetonationFX (spawned at impact).
    pub detonation_fx_name: String,
    /// C++ Weapon.ini ProjectileDetonationOCL name (spawned at impact).
    pub detonation_ocl_name: String,
    /// C++ Weapon.ini ProjectileExhaust PSys name (in-flight trail).
    pub exhaust_name: String,
    /// Firing object's ownership frozen at launch.  A projectile can outlive
    /// its firing object, while its detonation OCL must still create objects
    /// for the original team.
    pub source_team: Team,
    /// Exact default Team assigned to this projectile at launch. Unlike the
    /// firing object's Team, this is the projectile's own C++ object Team.
    #[serde(skip)]
    pub source_projectile_team_id: Option<u32>,
    /// Firing object's controlling player frozen at launch (C++ getRelationship).
    #[serde(default)]
    pub source_owner_player_id: Option<u32>,
    /// Firing object's team instance name frozen at launch (team-id overrides).
    #[serde(default)]
    pub source_team_instance_name: String,
    /// Firing object's veterancy frozen at launch for OCL `InheritsVeterancy`.
    pub source_veterancy: VeterancyLevel,
    /// C++ SecondaryDamage (outer splash ring amount).
    pub secondary_damage: f32,
    /// C++ SecondaryDamageRadius.
    pub secondary_damage_radius: f32,
    /// C++ ShockWaveAmount.
    pub shock_wave_amount: f32,
    /// C++ ShockWaveRadius.
    pub shock_wave_radius: f32,
    /// C++ ShockWaveTaperOff.
    pub shock_wave_taper_off: f32,
    /// C++ RadiusDamageAffects mask.
    pub radius_damage_affects: u32,
    /// C++ ProjectileCollidesWith mask.
    pub projectile_collides: u32,
    /// C++ HistoricBonus weapon-template key (empty = none).
    pub historic_weapon_key: String,
    /// Fired WeaponTemplate DamageStatusType (C++ DamageInfoInput).
    #[serde(skip)]
    pub damage_status_type: Option<ObjectStatusTypes>,
    /// Runtime continuation is framed separately by CPRJ v2, preserving v1 bytes.
    #[serde(skip)]
    pub stream_binding: Option<ProjectileStreamBinding>,
    pub historic_bonus_time_frames: u32,
    pub historic_bonus_count: i32,
    pub historic_bonus_radius: f32,
    pub historic_bonus_weapon: String,
    /// C++ Weapon.ini MissileCallsOnDie.
    pub die_on_detonate: bool,
    /// C++ DumbProjectile / MissileAI flight + warhead snapshot.
    pub flight: Option<ProjectileFlight>,
    /// Runtime Bezier / ignition / lock / layer state.
    pub flight_runtime: ProjectileFlightState,
    /// Live Object-owned THRUST and Physics continuation, framed by CPRJ.
    #[serde(skip)]
    pub missile_motion: Option<MissileMotion>,
    /// Factory-created projectile Object generation and world epoch.
    #[serde(skip)]
    pub projectile_actor_birth: Option<(u64, u64)>,
    /// C++ MissileAIUpdate `m_framesTillDecoyed` (absolute frame, 0 = none).
    #[serde(default)]
    pub frames_till_decoyed: u32,
    /// C++ MissileAIUpdate `m_noDamage` — diverted decoy path deals no HP.
    #[serde(default)]
    pub no_damage: bool,
    /// Launch already ran `reportMissileForCountermeasures`.
    #[serde(default)]
    pub cm_reported: bool,
    /// C++ `KINDOF_SMALL_MISSILE` on the projectile object.
    #[serde(default)]
    pub is_small_missile: bool,
    /// C++ `MissileAIUpdate::m_exhaustID` from `createAttachedParticleSystemID`.
    #[serde(default)]
    pub leftover_exhaust_id: u32,
    /// Conditions retained by the flight module, and conditions set on the
    /// projectile itself by its authored upgrade modules.
    pub launcher_bonus_mask: u64,
    pub projectile_bonus_mask: u64,
    pub projectile_upgrades_initialized: bool,
    pub computed_bonus_mask: Option<u64>,
}

impl Projectile {
    pub fn detonate_calls_kill(&self) -> bool {
        self.projectile_lifecycle
            .is_some_and(ProjectileLifecycle::detonate_calls_kill)
    }

    pub fn owns_receiving_actor(&self) -> bool {
        self.missile_motion.is_some() || self.detonate_calls_kill() || self.is_dumb_flight()
    }

    pub fn is_dumb_flight(&self) -> bool {
        matches!(self.flight, Some(ProjectileFlight::Dumb(_)))
    }

    /// DumbProjectileBehavior.cpp:611–643 uses the first tangent on the
    /// zeroeth sample and the previous curve sample on subsequent updates.
    pub fn dumb_flight_transform(&self, position: Vec3) -> Option<glam::Mat4> {
        let Some(ProjectileFlight::Dumb(flight)) = &self.flight else {
            return None;
        };
        if !flight.orient_to_flight_path || flight.tumble_randomly {
            return None;
        }
        let index = self.flight_runtime.step.saturating_sub(1).max(1);
        let path = &self.flight_runtime.path;
        let direction = (*path.get(index)? - path[index - 1]).normalize_or_zero();
        (direction != Vec3::ZERO).then(|| projectile_forward_transform(position, direction))
    }

    pub fn new(
        id: ObjectId,
        start_pos: Vec3,
        target_pos: Vec3,
        damage: f32,
        damage_type: CombatDamage,
        shooter_id: ObjectId,
        target_id: Option<ObjectId>,
    ) -> Self {
        let direction = (target_pos - start_pos).normalize_or_zero();
        // Caller overwrites speed/velocity via fire_projectile.
        let speed = 0.0;

        Self {
            id,
            position: start_pos,
            velocity: direction * speed,
            target_position: target_pos,
            damage,
            damage_type,
            shooter_id,
            target_id,
            speed,
            lifetime: 0.0,
            projectile_lifecycle: None,
            missile_kill_self_started_frame: None,
            max_lifetime: 0.0,
            is_homing: false,
            explosion_radius: 0.0,
            death_type: DeathOutcome::Normal,
            projectile_object_name: String::new(),
            collision_geometry: None,
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            source_team: Team::Neutral,
            source_projectile_team_id: None,
            source_owner_player_id: None,
            source_team_instance_name: String::new(),
            source_veterancy: VeterancyLevel::Rookie,
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects: WeaponAffectsMask::DEFAULT,
            projectile_collides: DEFAULT_PROJECTILE_COLLISIONS,
            historic_weapon_key: String::new(),
            damage_status_type: None,
            stream_binding: None,
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
            flight: None,
            flight_runtime: ProjectileFlightState::default(),
            missile_motion: None,
            projectile_actor_birth: None,
            frames_till_decoyed: 0,
            no_damage: false,
            cm_reported: false,
            is_small_missile: false,
            leftover_exhaust_id: 0,
            launcher_bonus_mask: 0,
            projectile_bonus_mask: 0,
            projectile_upgrades_initialized: false,
            computed_bonus_mask: None,
        }
    }

    /// Initialize the trajectory and damage of a newly fired shot.
    pub fn fired(
        projectile_id: ObjectId,
        shooter_pos: Vec3,
        target_pos: Vec3,
        damage: f32,
        splash_radius: f32,
        shooter_id: ObjectId,
        target_id: Option<ObjectId>,
        speed: f32,
        is_homing: bool,
    ) -> Self {
        let mut projectile = Self::new(
            projectile_id,
            shooter_pos,
            target_pos,
            damage,
            // C++ WeaponTemplate ctor defaults m_damageType to DAMAGE_EXPLOSION
            // (Weapon.cpp:249); the host Weapon payload carries no authored
            // type, so the direct-fire default is Explosion, not Bullet.
            CombatDamage::Explosive,
            shooter_id,
            target_id,
        );

        // C++ radius damage from WeaponTemplate splash/radius.
        projectile.explosion_radius = splash_radius.max(0.0);
        projectile.is_homing = is_homing && !Self::is_instant_speed(speed);

        if Self::is_instant_speed(speed) {
            // Laser / hitscan: spawn already at impact for same-frame resolve.
            projectile.speed = 0.0;
            projectile.velocity = Vec3::ZERO;
            projectile.position = target_pos;
            projectile.target_position = target_pos;
        } else {
            let spd = if speed > 0.0 { speed } else { 200.0 };
            let dir = (target_pos - shooter_pos).normalize_or_zero();
            projectile.speed = spd;
            projectile.velocity = dir * spd;
        }

        projectile
    }

    /// Attach resolved Object INI lifecycle; flight binding selects content.
    pub fn set_projectile_lifecycle(&mut self, lifecycle: Option<ProjectileLifecycle>) {
        self.projectile_lifecycle = lifecycle;
        self.missile_kill_self_started_frame = None;
        self.max_lifetime = lifecycle
            .map(ProjectileLifecycle::lifetime_seconds)
            .unwrap_or(0.0);
    }

    /// Bind parsed C++ flight/warhead and build the DumbProjectile Bezier.
    pub fn bind_authored_flight(
        &mut self,
        start: Vec3,
        end: Vec3,
        speed_per_second: f32,
        terrain_height: impl FnOnce(Vec3, Vec3) -> Option<f32>,
    ) -> bool {
        self.flight_runtime.bind(
            self.flight.as_ref(),
            start,
            end,
            speed_per_second,
            terrain_height,
        )
    }

    pub fn is_warhead_armed(&self) -> bool {
        match &self.flight {
            Some(ProjectileFlight::Missile(_)) => self.flight_runtime.missile_armed,
            _ => true,
        }
    }

    /// C++ `MissileAIUpdate::doLaunchState` then fall-through `doIgnitionState`.
    ///
    /// Returns `false` while still waiting `IgnitionDelay` (pose-hold, unarmed).
    /// On the ignition frame: leftover `FXList::doFXObj`, attach exhaust, arm.
    fn try_enter_missile_ignition(
        &mut self,
        frame: u32,
        world: &mut impl ProjectileFlightWorld,
    ) -> bool {
        match self.flight_runtime.ignition(self.flight.as_ref(), frame) {
            MissileIgnition::Waiting => false,
            MissileIgnition::Ready => true,
            MissileIgnition::Ignite(fx) => {
                let fx = fx.to_owned();
                self.play_missile_ignition(&fx, world);
                self.flight_runtime.arm();
                true
            }
        }
    }

    /// C++ `MissileAIUpdate::doIgnitionState` (MissileAIUpdate.cpp:462-466).
    fn play_missile_ignition(&mut self, ignition_fx: &str, world: &mut impl ProjectileFlightWorld) {
        world.publish_pose(self);
        if !ignition_fx.is_empty() {
            world.ignition_fx(ignition_fx, self);
        }
        if !self.exhaust_name.is_empty() && self.leftover_exhaust_id == 0 {
            if let Some(id) = world.attach_exhaust(self) {
                self.leftover_exhaust_id = id;
            }
        }
    }

    /// World-space exhaust residual after IGNITION. Leftover attach wins so
    /// C++ `createAttachedParticleSystemID` is not doubled by host sync.
    pub fn live_exhaust_name(&self) -> &str {
        if self.exhaust_name.is_empty() {
            return "";
        }
        match &self.flight {
            Some(ProjectileFlight::Missile(_)) => {
                if self.flight_runtime.missile_armed && self.leftover_exhaust_id == 0 {
                    self.exhaust_name.as_str()
                } else {
                    ""
                }
            }
            _ => self.exhaust_name.as_str(),
        }
    }

    /// Whether an authored MissileAIUpdate has entered C++ `KILL_SELF`.
    ///
    /// This state belongs to the parsed missile behavior and can retain
    /// the projectile long enough for its contrail to finish.
    pub fn is_missile_kill_self_holding(&self) -> bool {
        matches!(
            self.projectile_lifecycle,
            Some(ProjectileLifecycle::Missile { .. })
        ) && self.missile_kill_self_started_frame.is_some()
    }

    /// A detonated missile retains its object and exhaust, but not its drawable.
    pub fn missile_drawable_hidden(&self) -> bool {
        self.flight_runtime.missile_phase == MissilePhase::Detonated
    }

    fn elapsed_logic_frames(&self) -> u32 {
        // Recover the elapsed logic frames from the combat clock at 30 Hz.
        (self.lifetime.max(0.0) * 30.0).round() as u32
    }

    /// MissileAIUpdate::detonate enters KILL_SELF after emitting its impact.
    pub fn retain_detonated_missile(&mut self) -> bool {
        let frame = self.elapsed_logic_frames();
        self.flight_runtime.retain_detonated_missile(
            self.projectile_lifecycle,
            frame,
            &mut self.missile_kill_self_started_frame,
        )
    }

    fn detonation_step(&mut self, pre_physics_pose: Option<glam::Mat4>) -> ProjectileStep {
        ProjectileStep::Detonate {
            retain_actor: self.retain_detonated_missile(),
            pre_physics_pose,
        }
    }

    fn lifecycle_step(&mut self, target_is_live: bool, defer_target_loss: bool) -> ProjectileStep {
        let frame = self.elapsed_logic_frames();
        self.flight_runtime.lifecycle_step(
            self.projectile_lifecycle,
            self.flight.as_ref(),
            frame,
            &mut self.missile_kill_self_started_frame,
            self.is_homing && self.target_id.is_some() && !target_is_live && !defer_target_loss,
        )
    }

    pub fn completed_missile_kill_move(&self) -> bool {
        self.flight_runtime.missile_phase == MissilePhase::Kill
            && self
                .missile_motion
                .as_ref()
                .is_some_and(MissileMotion::is_move_idle)
    }

    /// The speed represents immediate impact for a hitscan weapon.
    pub fn is_instant_speed(speed: f32) -> bool {
        super::is_instant_weapon_speed(speed)
    }
    /// Step projectile behavior through its receiving world synchronously.
    pub fn advance_flight(
        &mut self,
        dt: f32,
        target_is_live: bool,
        kill_target: Option<MissileKillTarget>,
        world_frame: Option<u32>,
        world: &mut impl ProjectileFlightWorld,
    ) -> ProjectileStep {
        if let Some((position, velocity)) = world.missile_kinematics() {
            self.position = position;
            self.velocity = velocity;
        }
        if dt.is_finite() && dt > 0.0 {
            self.lifetime += dt;
        }

        // C++ removes a missile below the world before dispatching any state.
        if matches!(&self.flight, Some(ProjectileFlight::Missile(_))) && self.position.y < 0.0 {
            return ProjectileStep::Remove;
        }

        // C++ runs LAUNCH/IGNITION before airborneTargetGone (doAttackState).
        let frame = self.elapsed_logic_frames();
        let physics_frame = world_frame.unwrap_or(frame);
        let missile_was_armed = self.flight_runtime.missile_armed;
        let ignition_ready = self.try_enter_missile_ignition(frame, world);
        if !ignition_ready {
            if let Some(motion) = &mut self.missile_motion {
                world.advance_missile(
                    motion,
                    self.target_position,
                    false,
                    false,
                    MissileSteering::NoTurn,
                    physics_frame,
                );
                (self.position, self.velocity) = world
                    .missile_kinematics()
                    .expect("missile motion requires its world Object");
            }
            return ProjectileStep::Alive;
        }

        let was_kill = self.flight_runtime.missile_phase == MissilePhase::Kill;
        let completed_move = self.completed_missile_kill_move();
        // doAttackState (MissileAIUpdate.cpp:506-553) checks the retained
        // goal and returns on lock before it handles a vanished target.
        // Plan the transition here so lifecycle can still resolve fuel first.
        let entering_kill = self.missile_motion.is_some()
            && missile_was_armed
            && !was_kill
            && matches!(
                &self.flight,
                Some(ProjectileFlight::Missile(rule))
                    if missile_inside_lock_distance(
                        self.position, self.target_position, rule.lock_distance, self.is_homing,
                    )
            );

        // IGNITION arms the missile but does not run ATTACK until the next
        // update. In particular, a missing target cannot skip ignition.
        if missile_was_armed || !matches!(&self.flight, Some(ProjectileFlight::Missile(_))) {
            let lifecycle_step =
                self.lifecycle_step(target_is_live, entering_kill || completed_move);
            if lifecycle_step != ProjectileStep::Alive {
                let detonating = matches!(
                    lifecycle_step,
                    ProjectileStep::Detonate {
                        retain_actor: true,
                        ..
                    }
                );
                if detonating || lifecycle_step == ProjectileStep::Hold {
                    let impact_pose = detonating.then(|| {
                        world
                            .missile_transform()
                            .unwrap_or_else(|| Mat4::from_translation(self.position))
                    });
                    if let Some(motion) = &mut self.missile_motion {
                        let contact =
                            world.advance_held_missile(motion, self.target_position, physics_frame);
                        (self.position, self.velocity) = world
                            .missile_kinematics()
                            .expect("missile motion requires its world Object");
                        if contact && !self.missile_drawable_hidden() {
                            let delta = self.position - self.target_position;
                            let cell = PATHFIND_CELL_SIZE_F;
                            if delta.y <= cell || delta.length_squared() <= (3.0 * cell).powi(2) {
                                return self.detonation_step(None);
                            }
                        }
                    }
                    if let Some(pose) = impact_pose {
                        return ProjectileStep::Detonate {
                            retain_actor: true,
                            pre_physics_pose: Some(pose),
                        };
                    }
                }
                return lifecycle_step;
            }
        }
        if let Some(motion) = &mut self.missile_motion {
            let mut goal = self.target_position;
            if entering_kill {
                self.flight_runtime.missile_phase = MissilePhase::Kill;
                if !self.is_homing {
                    motion.restart_move();
                }
            }
            if let Some(ProjectileFlight::Missile(missile)) = &self.flight {
                if !self.is_homing
                    && self.flight_runtime.missile_phase != MissilePhase::Kill
                    && missile.lock_distance > 0.0
                {
                    goal.y += 10.0;
                }
            }
            let expired =
                missile_fuel_expired(self.projectile_lifecycle, self.flight.as_ref(), frame);
            let mut impact = None;
            if completed_move {
                match kill_target {
                    None => impact = Some(self.position),
                    Some(target) => {
                        let close_enough = world.missile_max_speed() * LOGIC_FRAME_TIMESTEP;
                        if target.edge_distance_squared < close_enough * close_enough {
                            // KILL snaps to the actual object origin, not the
                            // height-adjusted MoveTo destination.
                            world.set_missile_position(target.origin, physics_frame);
                            impact = Some(target.origin);
                        } else {
                            motion.restart_move();
                        }
                    }
                }
            }
            if impact.is_none() {
                let (position, _) = world
                    .missile_kinematics()
                    .expect("missile motion requires its world Object");
                motion.update_move(position, goal, world.missile_close_enough_distance());
            }
            let steering = if was_kill {
                MissileSteering::Braking
            } else if entering_kill {
                MissileSteering::LockTransition
            } else if missile_was_armed {
                MissileSteering::Attack
            } else {
                MissileSteering::NoTurn
            };
            let impact_pose = impact.map(|position| {
                let mut pose = world
                    .missile_transform()
                    .expect("missile motion requires its world Object");
                pose.w_axis = position.extend(1.0);
                pose
            });
            let ground_contact =
                world.advance_missile(motion, goal, true, !expired, steering, physics_frame);
            (self.position, self.velocity) = world
                .missile_kinematics()
                .expect("missile motion requires its world Object");
            if let Some(pose) = impact_pose {
                return self.detonation_step(Some(pose));
            }
            if ground_contact {
                // MissileAIUpdate::projectileHandleCollision ignores a
                // ground contact only when above and far from its AI goal.
                let delta = self.position - goal;
                let cell = PATHFIND_CELL_SIZE_F;
                if delta.y <= cell || delta.length_squared() <= (3.0 * cell).powi(2) {
                    return self.detonation_step(None);
                }
            }
            return self.finish_flight_step(world);
        }

        let mut kinematics = ProjectileKinematics {
            position: self.position,
            velocity: self.velocity,
            goal: self.target_position,
            speed: self.speed,
            homing: self.is_homing,
        };
        let step = self.flight_runtime.advance_virtual(
            self.flight.as_ref(),
            &mut kinematics,
            dt,
            missile_was_armed,
        );
        self.position = kinematics.position;
        self.velocity = kinematics.velocity;
        match step {
            VirtualFlightStep::Advanced => self.finish_flight_step(world),
            VirtualFlightStep::PathExhausted => ProjectileStep::DETONATE,
            VirtualFlightStep::MissileAtGoal => self.detonation_step(None),
        }
    }

    fn finish_flight_step(&mut self, world: &mut impl ProjectileFlightWorld) -> ProjectileStep {
        let armed = self.is_warhead_armed();
        let (snapped, new_layer) =
            world.bridge_deck(self.position, self.flight_runtime.layer, armed);
        let old_layer = self.flight_runtime.layer;
        self.flight_runtime.layer = new_layer;
        if armed && old_layer != 1 && new_layer == 1 && snapped != self.position {
            self.position = snapped;
            return self.detonation_step(None);
        }
        ProjectileStep::Alive
    }

    /// Contact inputs for an armed, visible registered actor.
    pub fn registered_collision(&self) -> Option<ProjectileCollision> {
        if self.missile_drawable_hidden() || !self.is_warhead_armed() {
            return None;
        }
        let mut collision = self.collision();
        collision.intended = collision
            .intended
            .filter(|_| self.is_homing || self.is_dumb_flight());
        Some(collision)
    }

    pub fn collision(&self) -> ProjectileCollision {
        ProjectileCollision {
            id: self.id,
            launcher: self.shooter_id,
            intended: self.target_id,
            mask: WeaponCollideMask::new(self.projectile_collides),
            damage_type: self.damage_type,
        }
    }

    pub fn warhead(&self) -> ProjectileWarhead {
        ProjectileWarhead {
            projectile: self.id,
            launcher: self.shooter_id,
            authored_flight: self.flight.is_some(),
            has_registered_actor: self.projectile_actor_birth.is_some(),
            no_damage: self.no_damage,
            damage: self.damage,
            damage_type: self.damage_type,
            death_type: self.death_type,
            damage_status_type: self.damage_status_type,
            primary_radius: self.explosion_radius,
            secondary_radius: self.secondary_damage_radius,
            affects: WeaponAffectsMask::new(self.radius_damage_affects),
        }
    }
}
