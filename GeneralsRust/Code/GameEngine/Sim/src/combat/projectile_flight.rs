//! Authored projectile flight and the receiving per-shot flight state.

use super::projectile_lifecycle::{
    ProjectileLifecycle, optional_bool, optional_duration_frames, optional_int, optional_percent,
    optional_real, optional_string, optional_velocity_per_frame,
};
use crate::{kind_of::KindOf, time::LOGIC_FRAMES_PER_SECOND};
use glam::{Mat4, Vec3, Vec4};

const MISSILE_DEFAULT_LOCK_DISTANCE: f32 = 75.0;
const LAYER_GROUND: u8 = 1;

/// Parsed DumbProjectileBehavior flight / warhead fields.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "HostDumbProjectileFlight")]
pub struct DumbProjectileFlight {
    pub first_height: f32,
    pub second_height: f32,
    pub first_percent_indent: f32,
    pub second_percent_indent: f32,
    pub orient_to_flight_path: bool,
    pub tumble_randomly: bool,
    /// C++ `parseVelocityReal` → dist per logic frame.
    pub flight_path_adjust_per_frame: f32,
    pub garrison_hit_kill_count: i32,
    pub garrison_required: Vec<KindOf>,
    pub garrison_forbidden: Vec<KindOf>,
    pub garrison_hit_kill_fx: String,
}

impl Default for DumbProjectileFlight {
    fn default() -> Self {
        Self {
            first_height: 0.0,
            second_height: 0.0,
            first_percent_indent: 0.0,
            second_percent_indent: 0.0,
            orient_to_flight_path: true,
            tumble_randomly: false,
            flight_path_adjust_per_frame: 0.0,
            garrison_hit_kill_count: 0,
            garrison_required: Vec::new(),
            garrison_forbidden: Vec::new(),
            garrison_hit_kill_fx: String::new(),
        }
    }
}

/// Parsed MissileAIUpdate ignition / lock / warhead fields.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "HostMissileFlight")]
pub struct MissileFlight {
    pub ignition_delay_frames: u32,
    pub lock_distance: f32,
    pub garrison_hit_kill_count: i32,
    pub garrison_required: Vec<KindOf>,
    pub garrison_forbidden: Vec<KindOf>,
    pub garrison_hit_kill_fx: String,
    pub ignition_fx: String,
}

impl Default for MissileFlight {
    fn default() -> Self {
        Self {
            ignition_delay_frames: 0,
            lock_distance: MISSILE_DEFAULT_LOCK_DISTANCE,
            garrison_hit_kill_count: 0,
            garrison_required: Vec::new(),
            garrison_forbidden: Vec::new(),
            garrison_hit_kill_fx: String::new(),
            ignition_fx: String::new(),
        }
    }
}

/// Authored C++ projectile module snapshot used by the live host flight path.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "HostProjectileFlight")]
pub enum ProjectileFlight {
    Dumb(DumbProjectileFlight),
    Missile(MissileFlight),
}

impl ProjectileFlight {
    pub fn garrison_hit_kill_count(&self) -> i32 {
        match self {
            Self::Dumb(d) => d.garrison_hit_kill_count,
            Self::Missile(m) => m.garrison_hit_kill_count,
        }
    }

    pub fn garrison_required(&self) -> &[KindOf] {
        match self {
            Self::Dumb(d) => &d.garrison_required,
            Self::Missile(m) => &m.garrison_required,
        }
    }

    pub fn garrison_forbidden(&self) -> &[KindOf] {
        match self {
            Self::Dumb(d) => &d.garrison_forbidden,
            Self::Missile(m) => &m.garrison_forbidden,
        }
    }

    pub fn garrison_hit_kill_fx(&self) -> &str {
        match self {
            Self::Dumb(d) => &d.garrison_hit_kill_fx,
            Self::Missile(m) => &m.garrison_hit_kill_fx,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "HostMissilePhase")]
pub enum MissilePhase {
    Launch,
    Attack,
    Kill,
    /// MissileAIUpdate::detonate hides its drawable while KILL_SELF waits.
    /// Appended so older serialized phase discriminants stay unchanged.
    Detonated,
}

/// Per-projectile runtime for Bezier stepping, ignition, lock, and layers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename = "HostProjectileRuntime")]
pub struct ProjectileFlightState {
    pub path: Vec<Vec3>,
    pub step: usize,
    pub path_start: Vec3,
    pub path_end: Vec3,
    pub path_speed_per_frame: f32,
    pub path_segments: usize,
    pub layer: u8,
    pub missile_armed: bool,
    pub missile_phase: MissilePhase,
    pub original_target_pos: Vec3,
}

impl Default for ProjectileFlightState {
    fn default() -> Self {
        Self {
            path: Vec::new(),
            step: 0,
            path_start: Vec3::ZERO,
            path_end: Vec3::ZERO,
            path_speed_per_frame: 0.0,
            path_segments: 0,
            layer: LAYER_GROUND,
            missile_armed: true,
            missile_phase: MissilePhase::Attack,
            original_target_pos: Vec3::ZERO,
        }
    }
}

pub fn parse_dumb_flight(
    data: &dyn game_engine::common::thing::module::ModuleData,
) -> DumbProjectileFlight {
    DumbProjectileFlight {
        first_height: optional_real(data, "FirstHeight").flatten().unwrap_or(0.0),
        second_height: optional_real(data, "SecondHeight").flatten().unwrap_or(0.0),
        first_percent_indent: optional_percent(data, "FirstPercentIndent")
            .flatten()
            .unwrap_or(0.0),
        second_percent_indent: optional_percent(data, "SecondPercentIndent")
            .flatten()
            .unwrap_or(0.0),
        orient_to_flight_path: optional_bool(data, "OrientToFlightPath")
            .flatten()
            .unwrap_or(true),
        tumble_randomly: optional_bool(data, "TumbleRandomly")
            .flatten()
            .unwrap_or(false),
        flight_path_adjust_per_frame: optional_velocity_per_frame(
            data,
            "FlightPathAdjustDistPerSecond",
        )
        .flatten()
        .unwrap_or(0.0),
        garrison_hit_kill_count: optional_int(data, "GarrisonHitKillCount")
            .flatten()
            .unwrap_or(0),
        garrison_required: parse_kindof_field(data, "GarrisonHitKillRequiredKindOf"),
        garrison_forbidden: parse_kindof_field(data, "GarrisonHitKillForbiddenKindOf"),
        garrison_hit_kill_fx: optional_string(data, "GarrisonHitKillFX")
            .filter(|name| !name.eq_ignore_ascii_case("none"))
            .unwrap_or_default(),
    }
}

pub fn parse_missile_flight(
    data: &dyn game_engine::common::thing::module::ModuleData,
) -> MissileFlight {
    MissileFlight {
        ignition_delay_frames: optional_duration_frames(data, "IgnitionDelay")
            .flatten()
            .unwrap_or(0),
        lock_distance: optional_real(data, "DistanceToTargetForLock")
            .flatten()
            .unwrap_or(MISSILE_DEFAULT_LOCK_DISTANCE),
        garrison_hit_kill_count: optional_int(data, "GarrisonHitKillCount")
            .flatten()
            .unwrap_or(0),
        garrison_required: parse_kindof_field(data, "GarrisonHitKillRequiredKindOf"),
        garrison_forbidden: parse_kindof_field(data, "GarrisonHitKillForbiddenKindOf"),
        garrison_hit_kill_fx: optional_string(data, "GarrisonHitKillFX")
            .filter(|name| !name.eq_ignore_ascii_case("none"))
            .unwrap_or_default(),
        ignition_fx: optional_string(data, "IgnitionFX")
            .filter(|name| !name.eq_ignore_ascii_case("none"))
            .unwrap_or_default(),
    }
}

fn parse_kindof_field(
    data: &dyn game_engine::common::thing::module::ModuleData,
    field: &str,
) -> Vec<KindOf> {
    let Some(raw) = data.get_ini_field(field) else {
        return Vec::new();
    };
    raw.split(|c: char| c.is_whitespace() || c == '+' || c == '|' || c == ',')
        .filter_map(parse_kindof_token)
        .collect()
}

fn parse_kindof_token(token: &str) -> Option<KindOf> {
    let token = token
        .trim()
        .trim_matches(',')
        .trim_start_matches("KINDOF_")
        .trim_start_matches("KINDOF");
    if token.is_empty() {
        return None;
    }
    match token.to_ascii_uppercase().as_str() {
        "INFANTRY" => Some(KindOf::Infantry),
        "HERO" => Some(KindOf::Hero),
        "VEHICLE" => Some(KindOf::Vehicle),
        "AIRCRAFT" => Some(KindOf::Aircraft),
        "STRUCTURE" => Some(KindOf::Structure),
        "PROJECTILE" => Some(KindOf::Projectile),
        "DRONE" => Some(KindOf::Drone),
        "DOZER" => Some(KindOf::Dozer),
        "HARVESTER" => Some(KindOf::Harvester),
        "MINE" => Some(KindOf::Mine),
        "DEMOTRAP" => Some(KindOf::DemoTrap),
        "SMALLMISSILE" | "SMALL_MISSILE" => Some(KindOf::SmallMissile),
        "BALLISTICMISSILE" | "BALLISTIC_MISSILE" => Some(KindOf::BallisticMissile),
        _ => None,
    }
}

fn host_to_cpp(pos: Vec3) -> glam::Vec3 {
    glam::Vec3::new(pos.x, pos.z, pos.y)
}

fn cpp_to_host(pos: glam::Vec3) -> Vec3 {
    Vec3::new(pos.x, pos.z, pos.y)
}

/// C++ calcFlightPath in host Y-up space.
pub fn build_dumb_bezier_path(
    start: Vec3,
    end: Vec3,
    flight: &DumbProjectileFlight,
    speed_per_second: f32,
    existing_segments: usize,
    highest: f32,
) -> (Vec<Vec3>, usize) {
    let speed_per_frame = if speed_per_second > 0.0 {
        speed_per_second / LOGIC_FRAMES_PER_SECOND
    } else {
        1.0
    };
    let (points, segs) = super::bezier::calc_dumb_projectile_bezier_points(
        host_to_cpp(start),
        host_to_cpp(end),
        flight.first_height,
        flight.second_height,
        flight.first_percent_indent,
        flight.second_percent_indent,
        highest,
        speed_per_frame,
        existing_segments,
    );
    (points.into_iter().map(cpp_to_host).collect(), segs)
}

pub fn adjust_flight_path_end(
    runtime: &mut ProjectileFlightState,
    flight: &DumbProjectileFlight,
    new_victim_center: Vec3,
    terrain_high: impl FnOnce(Vec3, Vec3) -> Option<f32>,
) -> bool {
    if flight.flight_path_adjust_per_frame <= 0.0 {
        return true;
    }
    let delta = new_victim_center - runtime.path_end;
    let dist_sq = delta.length_squared();
    if dist_sq <= 0.1 {
        return true;
    }
    let dist = dist_sq.sqrt();
    let move_dist = dist.min(flight.flight_path_adjust_per_frame);
    if move_dist <= 0.0 {
        return true;
    }
    runtime.path_end += delta * (move_dist / dist);
    let Some(highest) = terrain_high(runtime.path_start, runtime.path_end) else {
        return false;
    };
    let (path, segs) = build_dumb_bezier_path(
        runtime.path_start,
        runtime.path_end,
        flight,
        runtime.path_speed_per_frame * LOGIC_FRAMES_PER_SECOND,
        runtime.path_segments,
        highest,
    );
    runtime.path = path;
    runtime.path_segments = segs;
    true
}

/// C++ MissileAIUpdate::doAttackState lock radius. Non-tracking shots halve it.
pub fn missile_lock_distance(lock_distance: f32, tracking: bool) -> f32 {
    if lock_distance <= 0.0 {
        0.0
    } else if tracking {
        lock_distance
    } else {
        lock_distance * 0.5
    }
}

pub fn missile_inside_lock_distance(
    pos: Vec3,
    goal: Vec3,
    lock_distance: f32,
    tracking: bool,
) -> bool {
    let lock = missile_lock_distance(lock_distance, tracking);
    if lock <= 0.0 {
        return false;
    }
    let dx = pos.x - goal.x;
    let dz = pos.z - goal.z;
    dx * dx + dz * dz < lock * lock
}

/// Result of one host projectile behavior step.  `Detonate` is deliberately
/// distinct from `Remove`: C++ DumbProjectile expiry and missile no-fuel
/// detonation must execute the weapon's authored impact path, whereas followed
/// target loss enters MissileAIUpdate's quiet delayed self-destroy state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectileStep {
    Alive,
    Hold,
    Detonate {
        /// Missiles enter KILL_SELF for the authored contrail delay.
        retain_actor: bool,
        /// Pose before physics when the same frame advances beyond the impact.
        pre_physics_pose: Option<glam::Mat4>,
    },
    Remove,
}

impl ProjectileStep {
    pub const DETONATE: Self = Self::Detonate {
        retain_actor: false,
        pre_physics_pose: None,
    };
}

pub enum MissileIgnition<'a> {
    Waiting,
    Ready,
    Ignite(&'a str),
}

pub enum VirtualFlightStep {
    Advanced,
    PathExhausted,
    MissileAtGoal,
}

pub struct ProjectileKinematics {
    pub position: Vec3,
    pub velocity: Vec3,
    pub goal: Vec3,
    pub speed: f32,
    pub homing: bool,
}

pub fn missile_fuel_expired(
    lifecycle: Option<ProjectileLifecycle>,
    flight: Option<&ProjectileFlight>,
    frame: u32,
) -> bool {
    let Some(ProjectileLifecycle::Missile {
        fuel_lifetime_frames,
        ..
    }) = lifecycle
    else {
        return false;
    };
    let start = match flight {
        Some(ProjectileFlight::Missile(missile)) => missile.ignition_delay_frames.max(1),
        _ => 0,
    };
    fuel_lifetime_frames > 0 && frame >= start.saturating_add(fuel_lifetime_frames)
}

impl ProjectileFlightState {
    pub fn bind(
        &mut self,
        flight: Option<&ProjectileFlight>,
        start: Vec3,
        end: Vec3,
        speed_per_second: f32,
        terrain_high: impl FnOnce(Vec3, Vec3) -> Option<f32>,
    ) -> bool {
        let Some(flight) = flight else {
            return true;
        };
        self.path_start = start;
        self.path_end = end;
        self.original_target_pos = end;
        self.path_speed_per_frame = if speed_per_second > 0.0 {
            speed_per_second / LOGIC_FRAMES_PER_SECOND
        } else {
            0.0
        };
        match flight {
            ProjectileFlight::Dumb(dumb) => {
                let Some(highest) = terrain_high(start, end) else {
                    return false;
                };
                let (path, segs) =
                    build_dumb_bezier_path(start, end, dumb, speed_per_second, 0, highest);
                self.path = path;
                self.path_segments = segs;
                self.step = 0;
                self.missile_armed = true;
                self.missile_phase = MissilePhase::Attack;
            }
            ProjectileFlight::Missile(_) => {
                // C++ always starts LAUNCH. Delay 0 still visits IGNITION on the
                // first update so authored IgnitionFX plays (MissileAIUpdate.cpp:675-688).
                self.missile_armed = false;
                self.missile_phase = MissilePhase::Launch;
            }
        }
        true
    }

    pub fn ignition<'a>(
        &self,
        flight: Option<&'a ProjectileFlight>,
        frame: u32,
    ) -> MissileIgnition<'a> {
        match flight {
            Some(ProjectileFlight::Missile(missile)) if !self.missile_armed => {
                if frame < missile.ignition_delay_frames {
                    MissileIgnition::Waiting
                } else {
                    MissileIgnition::Ignite(&missile.ignition_fx)
                }
            }
            _ => MissileIgnition::Ready,
        }
    }

    pub fn arm(&mut self) {
        self.missile_armed = true;
        self.missile_phase = MissilePhase::Attack;
    }

    pub fn retain_detonated_missile(
        &mut self,
        lifecycle: Option<ProjectileLifecycle>,
        frame: u32,
        hold_started: &mut Option<u32>,
    ) -> bool {
        if matches!(lifecycle, Some(ProjectileLifecycle::Missile { .. })) {
            if hold_started.is_none() {
                *hold_started = Some(frame);
            }
            self.missile_phase = MissilePhase::Detonated;
            true
        } else {
            false
        }
    }

    pub fn lifecycle_step(
        &mut self,
        lifecycle: Option<ProjectileLifecycle>,
        flight: Option<&ProjectileFlight>,
        frame: u32,
        hold_started: &mut Option<u32>,
        lost_followed_target: bool,
    ) -> ProjectileStep {
        let Some(lifecycle) = lifecycle else {
            return ProjectileStep::Alive;
        };
        match lifecycle {
            ProjectileLifecycle::DumbProjectile {
                max_lifespan_frames,
                ..
            } if frame >= max_lifespan_frames => ProjectileStep::DETONATE,
            ProjectileLifecycle::Missile {
                try_to_follow_target: _,
                fuel_lifetime_frames: _,
                detonate_on_no_fuel,
                kill_self_delay_frames,
            } => {
                if let Some(kill_started) = *hold_started {
                    return if frame >= kill_started.saturating_add(kill_self_delay_frames) {
                        ProjectileStep::Remove
                    } else {
                        ProjectileStep::Hold
                    };
                }

                // MissileAIUpdate::doAttackState checks fuel before its
                // tracked-target-gone transition.  A missile which loses its
                // target on precisely its authored no-fuel frame therefore
                // still detonates when DetonateOnNoFuel is set.  `detonate()`
                // then enters KILL_SELF so its contrail can catch up before
                // the projectile object is destroyed.
                if missile_fuel_expired(Some(lifecycle), flight, frame) {
                    if detonate_on_no_fuel {
                        return ProjectileStep::Detonate {
                            retain_actor: self.retain_detonated_missile(
                                Some(lifecycle),
                                frame,
                                hold_started,
                            ),
                            pre_physics_pose: None,
                        };
                    }
                    if self.missile_phase == MissilePhase::Kill {
                        *hold_started = Some(frame);
                        return ProjectileStep::Hold;
                    }
                }

                // Tracking follows the current AI goal, including a diverted
                // flare, rather than the original designated victim.
                if lost_followed_target {
                    *hold_started = Some(frame);
                    return ProjectileStep::Hold;
                }

                // `FuelLifetime = 0` means infinity. With fuel exhausted but
                // no DetonateOnNoFuel, C++ stops acceleration/turning and
                // discards exhaust; it does not fabricate a detonation or
                // remove the missile here.
                ProjectileStep::Alive
            }
            _ => ProjectileStep::Alive,
        }
    }

    /// Advance position and velocity before the caller resolves terrain contact.
    pub fn advance_virtual(
        &mut self,
        flight: Option<&ProjectileFlight>,
        kinematics: &mut ProjectileKinematics,
        dt: f32,
        missile_was_armed: bool,
    ) -> VirtualFlightStep {
        if kinematics.speed <= 0.0 {
            kinematics.position = kinematics.goal;
            return VirtualFlightStep::Advanced;
        }

        if matches!(flight, Some(ProjectileFlight::Dumb(_))) && !self.path.is_empty() {
            if self.step >= self.path.len() {
                return VirtualFlightStep::PathExhausted;
            }
            let next = self.path[self.step];
            let tangent = next - kinematics.position;
            if tangent.length_squared() > 0.0001 {
                kinematics.velocity = tangent.normalize() * kinematics.speed;
            }
            kinematics.position = next;
            self.step += 1;
            return VirtualFlightStep::Advanced;
        }

        if let Some(ProjectileFlight::Missile(missile)) = flight {
            let was_kill = self.missile_phase == MissilePhase::Kill;
            // LAUNCH may fall through to IGNITION, but neither IGNITION nor
            // ATTACK dispatches the next state until the following update.
            if missile_was_armed
                && self.missile_phase != MissilePhase::Kill
                && missile_inside_lock_distance(
                    kinematics.position,
                    kinematics.goal,
                    missile.lock_distance,
                    kinematics.homing,
                )
            {
                self.missile_phase = MissilePhase::Kill;
            }
            if was_kill {
                let goal = kinematics.goal;
                let close_enough = (kinematics.speed / LOGIC_FRAMES_PER_SECOND).max(1.0);
                if kinematics.position.distance(goal) <= close_enough {
                    kinematics.position = goal;
                    return VirtualFlightStep::MissileAtGoal;
                }
                let dir = (goal - kinematics.position).normalize_or_zero();
                kinematics.velocity = dir * kinematics.speed;
                kinematics.position += kinematics.velocity * dt;
                return VirtualFlightStep::Advanced;
            }
        }

        if kinematics.homing {
            let dir = (kinematics.goal - kinematics.position).normalize_or_zero();
            kinematics.velocity = dir * kinematics.speed;
        }
        kinematics.position += kinematics.velocity * dt;
        VirtualFlightStep::Advanced
    }
}

/// C++ `Matrix3D::buildTransformMatrix` (`WWMath/matrix3d.cpp:349`).
/// Identity, translate, `Rotate_Z(siny, cosy)`, then `Rotate_Y(-dir.z, len2)`.
pub fn build_transform_matrix(pos: Vec3, dir: Vec3) -> Mat4 {
    let len2 = (dir.x * dir.x + dir.y * dir.y).sqrt();
    let sinp = dir.z;
    let cosp = len2;
    let (siny, cosy) = if len2 != 0.0 {
        (dir.y / len2, dir.x / len2)
    } else {
        (0.0, 1.0)
    };
    let mut row = [[0.0f32; 4]; 3];
    row[0][0] = 1.0;
    row[1][1] = 1.0;
    row[2][2] = 1.0;
    row[0][3] = pos.x;
    row[1][3] = pos.y;
    row[2][3] = pos.z;
    let (s, c) = (siny, cosy);
    for r in 0..3 {
        let tmp1 = row[r][0];
        let tmp2 = row[r][1];
        row[r][0] = c * tmp1 + s * tmp2;
        row[r][1] = -s * tmp1 + c * tmp2;
    }
    let (s, c) = (-sinp, cosp);
    for r in 0..3 {
        let tmp1 = row[r][0];
        let tmp2 = row[r][2];
        row[r][0] = c * tmp1 - s * tmp2;
        row[r][2] = s * tmp1 + c * tmp2;
    }
    Mat4::from_cols(
        Vec4::new(row[0][0], row[1][0], row[2][0], 0.0),
        Vec4::new(row[0][1], row[1][1], row[2][1], 0.0),
        Vec4::new(row[0][2], row[1][2], row[2][2], 0.0),
        Vec4::new(row[0][3], row[1][3], row[2][3], 1.0),
    )
}

pub fn projectile_forward_transform(position: Vec3, direction: Vec3) -> glam::Mat4 {
    let basis = glam::Mat4::from_cols(glam::Vec4::X, glam::Vec4::Z, glam::Vec4::Y, glam::Vec4::W);
    let transform = build_transform_matrix(
        basis.transform_point3(position),
        basis.transform_vector3(direction),
    );
    basis * transform * basis
}
