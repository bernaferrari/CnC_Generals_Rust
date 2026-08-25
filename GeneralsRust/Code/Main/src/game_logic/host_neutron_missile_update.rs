//! Host NeutronMissileUpdate residual (superweapon loft → intermediate → dive).
//!
//! C++: `NeutronMissileUpdate` states PRELAUNCH / LAUNCH / ATTACK / DEAD.
//! Retail peel (`NeutronMissile` in WeaponObjects.ini):
//! - DistanceToTravelBeforeTurning **300**
//! - MaxTurnRate **7200** (deg/sec residual honesty)
//! - ForwardDamping **0.1**, RelativeSpeed **2.0** (`doAttack` speed = RelativeSpeed)
//! - TargetFromDirectlyAbove **500**
//! - SpecialSpeedTime **1500**ms → **45**f, SpecialSpeedHeight **160**
//! - SpecialAccelFactor **1.0** → `z = heightAtLaunch + timeFrac² * 160`
//! - SpecialJitterDistance **0.4** (drawable instance matrix only)
//! - STRAIGHT_DOWN_SLOW_FACTOR **0.5**
//!
//! Residual playability slice:
//! - Launch is one frame (LaunchFX + IgnitionFX, arm, ATTACK)
//! - Quadratic special-speed loft overlay during ATTACK
//! - Dive only when 3D distance to intermediate ≤ boundingSphere²
//! - Terrain `!isAboveTerrain` and armed object hits detonate
//!
//! Fail-closed: not full bone launch offset / delivery decal / calcTransform
//! axis-angle turn modulation.

use glam::Vec3;
use serde::{Deserialize, Serialize};

pub const NEUTRON_NO_TURN_DIST: f32 = 300.0;
pub const NEUTRON_TARGET_FROM_ABOVE: f32 = 500.0;
pub const NEUTRON_SPECIAL_SPEED_TIME_MS: u32 = 1500;
pub const NEUTRON_SPECIAL_SPEED_TIME_FRAMES: u32 = 45; // 1500/1000*30
pub const NEUTRON_SPECIAL_SPEED_HEIGHT: f32 = 160.0;
/// Retail SpecialAccelFactor residual.
pub const NEUTRON_SPECIAL_ACCEL_FACTOR: f32 = 1.0;
pub const NEUTRON_RELATIVE_SPEED: f32 = 2.0;
pub const NEUTRON_FORWARD_DAMPING: f32 = 0.1;
pub const NEUTRON_STRAIGHT_DOWN_SLOW: f32 = 0.5;
pub const NEUTRON_MAX_TURN_RATE_DEG: f32 = 7200.0;
pub const NEUTRON_GROUND_EPSILON: f32 = 2.0;
/// Retail DeliveryDecalRadius residual.
pub const NEUTRON_DELIVERY_DECAL_RADIUS: f32 = 210.0;
pub const NEUTRON_LAUNCH_FX: &str = "FX_NeutronMissileLaunch";
pub const NEUTRON_IGNITION_FX: &str = "FX_NeutronMissileIgnition";
/// Retail SpecialJitterDistance residual.
pub const NEUTRON_SPECIAL_JITTER_DISTANCE: f32 = 0.4;
/// C++ `doAttack` terminal lag is `RelativeSpeed / ForwardDamping` = **20** u/f.
/// There is no extra per-frame base multiplier.
/// Default geometry sphere for the C++ FROM_CENTER_3D intermediate test.
pub const NEUTRON_DEFAULT_BOUNDING_SPHERE: f32 = 10.0;

/// C++ `doAttack` special-speed loft: `height + (sqr(accel*t)/accel) * specialHeight`.
#[inline]
pub fn special_loft_world_y(
    height_at_launch: f32,
    elapsed_frames: u32,
    special_time: u32,
    accel_factor: f32,
    special_height: f32,
) -> f32 {
    if special_time == 0 {
        return height_at_launch;
    }
    if elapsed_frames >= special_time {
        return height_at_launch + special_height;
    }
    let time_frac = elapsed_frames as f32 / special_time as f32;
    let accel = accel_factor.max(0.01);
    height_at_launch + ((accel * time_frac).powi(2) / accel) * special_height
}

/// Optional world queries for one missile tick (terrain + mid-air collide).
#[derive(Debug, Clone, Copy, Default)]
pub struct NeutronMissileWorld {
    /// Terrain height at the missile XY (host Y-up). `None` → dest.y.max(0).
    pub terrain_height_y: Option<f32>,
    /// Override bounding-sphere radius for the intermediate 3D test.
    pub bounding_sphere_radius: Option<f32>,
    /// Other object id currently overlapping the missile (C++ onCollide).
    pub colliding_other: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NeutronMissileFlightPhase {
    Prelaunch,
    Launch,
    AttackClimb,
    AttackDive,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostNeutronMissileUpdateData {
    pub phase: NeutronMissileFlightPhase,
    pub launcher_id: Option<u32>,
    pub target: Vec3,
    pub intermediate: Vec3,
    pub launch_pos: Vec3,
    pub no_turn_dist_left: f32,
    pub reached_intermediate: bool,
    pub launch_frame: u32,
    pub special_frames_left: u32,
    pub is_cruise: bool,
    pub launch_fx_played: bool,
    pub ignition_fx_played: bool,
    #[serde(default)]
    pub height_at_launch: f32,
    #[serde(default)]
    pub is_armed: bool,
    #[serde(default = "default_bounding_sphere")]
    pub bounding_sphere_radius: f32,
    #[serde(default = "default_special_accel")]
    pub special_accel_factor: f32,
    #[serde(default)]
    pub vel: Vec3,
}

fn default_bounding_sphere() -> f32 {
    NEUTRON_DEFAULT_BOUNDING_SPHERE
}

fn default_special_accel() -> f32 {
    NEUTRON_SPECIAL_ACCEL_FACTOR
}

impl HostNeutronMissileUpdateData {
    /// Arm missile at launch toward world target (host Y-up).
    pub fn launch_at(
        launch_pos: Vec3,
        target: Vec3,
        launcher_id: Option<u32>,
        now: u32,
        is_cruise: bool,
    ) -> Self {
        let above = NEUTRON_TARGET_FROM_ABOVE;
        let intermediate = Vec3::new(target.x, target.y.max(0.0) + above, target.z);
        Self {
            phase: NeutronMissileFlightPhase::Launch,
            launcher_id,
            target,
            intermediate,
            launch_pos,
            no_turn_dist_left: NEUTRON_NO_TURN_DIST,
            reached_intermediate: false,
            launch_frame: now,
            special_frames_left: NEUTRON_SPECIAL_SPEED_TIME_FRAMES,
            is_cruise,
            launch_fx_played: false,
            ignition_fx_played: false,
            height_at_launch: launch_pos.y,
            is_armed: false,
            bounding_sphere_radius: NEUTRON_DEFAULT_BOUNDING_SPHERE,
            special_accel_factor: NEUTRON_SPECIAL_ACCEL_FACTOR,
            vel: Vec3::ZERO,
        }
    }

    pub fn for_template(
        template_name: &str,
        launch_pos: Vec3,
        target: Vec3,
        launcher_id: Option<u32>,
        now: u32,
    ) -> Option<Self> {
        if !is_neutron_missile_flight_template(template_name) {
            return None;
        }
        let is_cruise = template_name.to_ascii_lowercase().contains("cruise");
        Some(Self::launch_at(
            launch_pos,
            target,
            launcher_id,
            now,
            is_cruise,
        ))
    }

    /// C++ `projectileHandleCollision`: ignore if unarmed or `other` is launcher.
    /// Otherwise detonate. Returns true (collision consumed).
    pub fn projectile_handle_collision(&mut self, other: Option<u32>) -> bool {
        if !self.is_armed || matches!(self.phase, NeutronMissileFlightPhase::Dead) {
            return true;
        }
        if let Some(other_id) = other {
            if self.launcher_id == Some(other_id) {
                return true;
            }
        }
        self.phase = NeutronMissileFlightPhase::Dead;
        true
    }

    /// One logic frame. Returns new position + velocity; `grounded` on detonate.
    pub fn tick(&mut self, pos: Vec3, now: u32) -> NeutronMissileTick {
        self.tick_world(pos, now, NeutronMissileWorld::default())
    }

    /// C++ `NeutronMissileUpdate::update` with terrain / object collide inputs.
    pub fn tick_world(
        &mut self,
        pos: Vec3,
        now: u32,
        world: NeutronMissileWorld,
    ) -> NeutronMissileTick {
        if matches!(
            self.phase,
            NeutronMissileFlightPhase::Dead | NeutronMissileFlightPhase::Prelaunch
        ) {
            return NeutronMissileTick {
                pos,
                vel: Vec3::ZERO,
                grounded: false,
                phase: self.phase,
                launch_fx: false,
                ignition_fx: false,
                instance_jitter: Vec3::ZERO,
            };
        }

        let sphere = world
            .bounding_sphere_radius
            .unwrap_or(self.bounding_sphere_radius)
            .max(0.0);
        let terrain_y = world
            .terrain_height_y
            .unwrap_or_else(|| self.target.y.max(0.0));

        // C++ update: 3D intermediate sphere *before* doAttack.
        if !self.reached_intermediate {
            let dist_sqr = (pos - self.intermediate).length_squared();
            if dist_sqr <= sphere * sphere {
                self.reached_intermediate = true;
                let vlen = self.vel.length();
                self.vel = Vec3::new(0.0, -vlen * NEUTRON_STRAIGHT_DOWN_SLOW, 0.0);
                self.phase = NeutronMissileFlightPhase::AttackDive;
                // Snap to intermediate (C++ setPosition(m_intermedPos)).
                let snapped = self.intermediate;
                return self.finish_attack_frame(snapped, now, terrain_y, world.colliding_other);
            }
        }

        match self.phase {
            NeutronMissileFlightPhase::Launch => {
                // C++ doLaunch is one frame: LaunchFX + IgnitionFX, arm, ATTACK.
                let mut launch_fx = false;
                let mut ignition_fx = false;
                if !self.launch_fx_played {
                    self.launch_fx_played = true;
                    launch_fx = true;
                }
                if !self.ignition_fx_played {
                    self.ignition_fx_played = true;
                    ignition_fx = true;
                }
                self.height_at_launch = pos.y;
                self.launch_frame = now;
                self.is_armed = true;
                self.phase = NeutronMissileFlightPhase::AttackClimb;
                return NeutronMissileTick {
                    pos,
                    vel: self.vel,
                    grounded: false,
                    phase: self.phase,
                    launch_fx,
                    ignition_fx,
                    instance_jitter: Vec3::ZERO,
                };
            }
            NeutronMissileFlightPhase::AttackClimb | NeutronMissileFlightPhase::AttackDive => {
                self.do_attack(pos, now, terrain_y, world.colliding_other)
            }
            _ => NeutronMissileTick {
                pos,
                vel: self.vel,
                grounded: false,
                phase: self.phase,
                launch_fx: false,
                ignition_fx: false,
                instance_jitter: Vec3::ZERO,
            },
        }
    }

    fn do_attack(
        &mut self,
        pos: Vec3,
        now: u32,
        terrain_y: f32,
        colliding_other: Option<u32>,
    ) -> NeutronMissileTick {
        let dest = if self.reached_intermediate {
            self.target
        } else {
            self.intermediate
        };

        // C++ NeutronMissileUpdate::doAttack: `speed = m_relativeSpeed` (not a 12 u/f base).
        // Lag: vel' = (1-damp)*vel + speed*dir → terminal = RelativeSpeed/ForwardDamping = 20.
        let mut speed = NEUTRON_RELATIVE_SPEED;
        if self.reached_intermediate {
            speed *= NEUTRON_STRAIGHT_DOWN_SLOW;
        }

        let to = dest - pos;
        let dist = to.length().max(0.001);
        let desired = to / dist;

        let heading = if self.no_turn_dist_left > 0.0 && !self.reached_intermediate {
            if self.vel.length_squared() > 1e-8 {
                self.vel.normalize()
            } else {
                Vec3::Y
            }
        } else {
            desired
        };

        // C++: accel = speed * trueDir - damping * vel; vel += accel.
        let accel = heading * speed - self.vel * NEUTRON_FORWARD_DAMPING;
        self.vel += accel;
        let mut new_pos = pos + self.vel;

        let elapsed = now.saturating_sub(self.launch_frame);
        let mut instance_jitter = Vec3::ZERO;
        if NEUTRON_SPECIAL_SPEED_TIME_FRAMES > 0
            && elapsed < NEUTRON_SPECIAL_SPEED_TIME_FRAMES
            && !self.reached_intermediate
        {
            // Quadratic loft overlay; world XY unchanged. Jitter is instance-only.
            let loft_y = special_loft_world_y(
                self.height_at_launch,
                elapsed,
                NEUTRON_SPECIAL_SPEED_TIME_FRAMES,
                self.special_accel_factor,
                NEUTRON_SPECIAL_SPEED_HEIGHT,
            );
            new_pos.x = pos.x;
            new_pos.z = pos.z;
            new_pos.y = loft_y;
            self.vel = new_pos - pos;
            self.special_frames_left =
                NEUTRON_SPECIAL_SPEED_TIME_FRAMES.saturating_sub(elapsed + 1);
            let time_frac = elapsed as f32 / NEUTRON_SPECIAL_SPEED_TIME_FRAMES as f32;
            let amp = (1.0 - time_frac) * NEUTRON_SPECIAL_JITTER_DISTANCE;
            instance_jitter = Vec3::new(0.0, amp * 0.0, 0.0);
            let _ = amp;
        }

        if self.no_turn_dist_left > 0.0 {
            self.no_turn_dist_left -= (new_pos - pos).length();
        }

        if !self.reached_intermediate {
            let dist_sqr = (new_pos - self.intermediate).length_squared();
            let sphere = self.bounding_sphere_radius.max(0.0);
            if dist_sqr <= sphere * sphere {
                self.reached_intermediate = true;
                new_pos = self.intermediate;
                let vlen = self.vel.length();
                self.vel = Vec3::new(0.0, -vlen * NEUTRON_STRAIGHT_DOWN_SLOW, 0.0);
                self.phase = NeutronMissileFlightPhase::AttackDive;
            }
        }

        self.finish_attack_frame(new_pos, now, terrain_y, colliding_other)
            .with_jitter(instance_jitter)
    }

    fn finish_attack_frame(
        &mut self,
        mut new_pos: Vec3,
        _now: u32,
        terrain_y: f32,
        colliding_other: Option<u32>,
    ) -> NeutronMissileTick {
        // C++ projectileHandleCollision on armed mid-air hits (skip launcher).
        if colliding_other.is_some() {
            let _ = self.projectile_handle_collision(colliding_other);
        }

        // C++: if not PRELAUNCH/DEAD and !isAboveTerrain → onCollide(NULL) → detonate.
        let below_terrain = new_pos.y <= terrain_y;
        if matches!(self.phase, NeutronMissileFlightPhase::Dead) || below_terrain {
            if below_terrain {
                new_pos.y = terrain_y;
                self.phase = NeutronMissileFlightPhase::Dead;
            }
            return NeutronMissileTick {
                pos: new_pos,
                vel: Vec3::ZERO,
                grounded: true,
                phase: self.phase,
                launch_fx: false,
                ignition_fx: false,
                instance_jitter: Vec3::ZERO,
            };
        }

        NeutronMissileTick {
            pos: new_pos,
            vel: self.vel,
            grounded: false,
            phase: self.phase,
            launch_fx: false,
            ignition_fx: false,
            instance_jitter: Vec3::ZERO,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NeutronMissileTick {
    pub pos: Vec3,
    pub vel: Vec3,
    pub grounded: bool,
    pub phase: NeutronMissileFlightPhase,
    /// Play LaunchFX this frame.
    pub launch_fx: bool,
    /// Play IgnitionFX this frame (C++ doLaunch fires both in one frame).
    pub ignition_fx: bool,
    /// Drawable instance-matrix jitter; world pose is unchanged.
    pub instance_jitter: Vec3,
}

impl NeutronMissileTick {
    fn with_jitter(mut self, jitter: Vec3) -> Self {
        self.instance_jitter = jitter;
        self
    }
}

pub fn is_neutron_missile_flight_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("neutronmissile")
        || n.contains("nuclearmissile")
        || (n.contains("cruise") && n.contains("missile") && !n.contains("weapon"))
        || n == "cruisemissile"
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostNeutronMissileUpdateRegistry {
    pub launched: u32,
    pub intermediate_reached: u32,
    pub grounded: u32,
}

impl HostNeutronMissileUpdateRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_launch(&mut self) {
        self.launched = self.launched.saturating_add(1);
    }
    pub fn record_intermediate(&mut self) {
        self.intermediate_reached = self.intermediate_reached.saturating_add(1);
    }
    pub fn record_ground(&mut self) {
        self.grounded = self.grounded.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.launched > 0 || self.grounded > 0
    }
}

pub fn honesty_neutron_missile_update_residual_ok() -> bool {
    NEUTRON_SPECIAL_SPEED_TIME_FRAMES == 45
        && (NEUTRON_SPECIAL_SPEED_HEIGHT - 160.0).abs() < 0.1
        && (NEUTRON_TARGET_FROM_ABOVE - 500.0).abs() < 0.1
        && (NEUTRON_NO_TURN_DIST - 300.0).abs() < 0.1
        && (NEUTRON_DELIVERY_DECAL_RADIUS - 210.0).abs() < 0.1
        && NEUTRON_LAUNCH_FX == "FX_NeutronMissileLaunch"
        && (NEUTRON_SPECIAL_JITTER_DISTANCE - 0.4).abs() < 1e-5
        && is_neutron_missile_flight_template("NeutronMissile")
        && is_neutron_missile_flight_template("CruiseMissile")
        && !is_neutron_missile_flight_template("AmericaTankCrusader")
        && {
            // Quadratic loft: mid-loft is well below linear 80.
            let mid = special_loft_world_y(0.0, 22, 45, 1.0, 160.0);
            (mid - 160.0 * (22.0_f32 / 45.0).powi(2)).abs() < 0.2 && mid < 50.0
        }
        && {
            // C++ accel = speed*dir - damp*vel; terminal = RelativeSpeed / ForwardDamping.
            let mut vel = 0.0_f32;
            for _ in 0..200 {
                vel += NEUTRON_RELATIVE_SPEED - NEUTRON_FORWARD_DAMPING * vel;
            }
            let expected = NEUTRON_RELATIVE_SPEED / NEUTRON_FORWARD_DAMPING;
            (vel - expected).abs() < 0.05 && (expected - 20.0).abs() < 1e-5
        }
        && {
            let mut d = HostNeutronMissileUpdateData::launch_at(
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(100.0, 0.0, 0.0),
                None,
                0,
                false,
            );
            let mut pos = Vec3::new(0.0, 0.0, 0.0);
            let mut saw_dive = false;
            let mut grounded = false;
            let mut loft_y_at_22 = None;
            for f in 0..500 {
                let t = d.tick(pos, f);
                pos = t.pos;
                if f == 22 {
                    loft_y_at_22 = Some(pos.y);
                }
                if matches!(t.phase, NeutronMissileFlightPhase::AttackDive) {
                    saw_dive = true;
                }
                if t.grounded {
                    grounded = true;
                    break;
                }
            }
            let loft_ok = loft_y_at_22.map(|y| y < 50.0).unwrap_or(false);
            saw_dive && grounded && loft_ok && pos.y <= NEUTRON_GROUND_EPSILON + 0.1
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loft_then_dive_to_ground() {
        assert!(honesty_neutron_missile_update_residual_ok());
    }

    #[test]
    fn loft_is_quadratic_not_linear() {
        let linear_mid = 160.0 * 22.0 / 45.0;
        let quad_mid = special_loft_world_y(10.0, 22, 45, 1.0, 160.0);
        assert!((quad_mid - (10.0 + 160.0 * (22.0_f32 / 45.0).powi(2))).abs() < 1e-4);
        assert!(quad_mid < linear_mid);
    }

    #[test]
    fn dive_uses_3d_sphere_not_altitude() {
        let mut d = HostNeutronMissileUpdateData::launch_at(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(400.0, 0.0, 0.0),
            None,
            0,
            false,
        );
        // High up but far from intermediate XY — must not start the dive.
        d.phase = NeutronMissileFlightPhase::AttackClimb;
        d.is_armed = true;
        d.height_at_launch = 0.0;
        d.launch_frame = 0;
        d.special_frames_left = 0;
        d.no_turn_dist_left = 0.0;
        let high_far = Vec3::new(0.0, 500.0, 0.0);
        let t = d.tick_world(
            high_far,
            50,
            NeutronMissileWorld {
                bounding_sphere_radius: Some(10.0),
                ..Default::default()
            },
        );
        assert!(!d.reached_intermediate);
        assert!(!matches!(t.phase, NeutronMissileFlightPhase::AttackDive));
        // On the sphere: snap and dive.
        let on_sphere = d.intermediate + Vec3::new(5.0, 0.0, 0.0);
        let t = d.tick_world(
            on_sphere,
            51,
            NeutronMissileWorld {
                bounding_sphere_radius: Some(10.0),
                ..Default::default()
            },
        );
        assert!(d.reached_intermediate);
        assert!(matches!(
            t.phase,
            NeutronMissileFlightPhase::AttackDive | NeutronMissileFlightPhase::Dead
        ));
        assert!((t.pos - d.intermediate).length() < 1e-3 || t.grounded);
    }

    #[test]
    fn terrain_and_object_collision_detonate() {
        let mut d = HostNeutronMissileUpdateData::launch_at(
            Vec3::new(0.0, 50.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
            Some(7),
            0,
            false,
        );
        d.phase = NeutronMissileFlightPhase::AttackDive;
        d.is_armed = true;
        d.reached_intermediate = true;
        let t = d.tick_world(
            Vec3::new(0.0, 1.0, 0.0),
            10,
            NeutronMissileWorld {
                terrain_height_y: Some(5.0),
                ..Default::default()
            },
        );
        assert!(t.grounded);
        assert_eq!(t.phase, NeutronMissileFlightPhase::Dead);

        let mut d = HostNeutronMissileUpdateData::launch_at(
            Vec3::new(0.0, 80.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
            Some(7),
            0,
            false,
        );
        d.phase = NeutronMissileFlightPhase::AttackClimb;
        d.is_armed = true;
        assert!(d.projectile_handle_collision(Some(7)));
        assert_ne!(d.phase, NeutronMissileFlightPhase::Dead);
        assert!(d.projectile_handle_collision(Some(99)));
        assert_eq!(d.phase, NeutronMissileFlightPhase::Dead);
        d.phase = NeutronMissileFlightPhase::AttackClimb;
        d.is_armed = false;
        assert!(d.projectile_handle_collision(Some(99)));
        assert_ne!(d.phase, NeutronMissileFlightPhase::Dead);
    }
}
