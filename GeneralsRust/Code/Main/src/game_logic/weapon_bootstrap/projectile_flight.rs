//! Live DumbProjectile / MissileAI flight + warhead snapshot.
//!
//! `HostProjectileLifecycle` stays the small Eq lifetime enum. This sibling
//! owns the C++ Bezier/terrain arc, FlightPathAdjust, IgnitionDelay arming,
//! LockDistance KILL snap, GarrisonHitKill, and bridge-deck detonate that the
//! lightweight CombatSystem projectile previously skipped.

use super::projectile_lifecycle::{
    optional_bool, optional_duration_frames, optional_int, optional_percent, optional_real,
    optional_string, optional_velocity_per_frame, with_projectile_behavior_module,
};
use crate::game_logic::object::Object;
use crate::game_logic::{KindOf, ObjectId};
use glam::Vec3;
use std::collections::HashMap;

const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;
const MISSILE_DEFAULT_LOCK_DISTANCE: f32 = 75.0;
const BRIDGE_DECK_FUDGE: f32 = 2.0;
const LAYER_GROUND: u8 = 1;

/// Parsed DumbProjectileBehavior flight / warhead fields.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HostDumbProjectileFlight {
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

impl Default for HostDumbProjectileFlight {
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
pub struct HostMissileFlight {
    pub ignition_delay_frames: u32,
    pub lock_distance: f32,
    pub garrison_hit_kill_count: i32,
    pub garrison_required: Vec<KindOf>,
    pub garrison_forbidden: Vec<KindOf>,
    pub garrison_hit_kill_fx: String,
    pub ignition_fx: String,
}

impl Default for HostMissileFlight {
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
pub enum HostProjectileFlight {
    Dumb(HostDumbProjectileFlight),
    Missile(HostMissileFlight),
}

impl HostProjectileFlight {
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
pub enum HostMissilePhase {
    Launch,
    Attack,
    Kill,
}

/// Per-projectile runtime for Bezier stepping, ignition, lock, and layers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HostProjectileRuntime {
    pub path: Vec<Vec3>,
    pub step: usize,
    pub path_start: Vec3,
    pub path_end: Vec3,
    pub path_speed_per_frame: f32,
    pub path_segments: usize,
    pub layer: u8,
    pub missile_armed: bool,
    pub missile_phase: HostMissilePhase,
    pub original_target_pos: Vec3,
}

impl Default for HostProjectileRuntime {
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
            missile_phase: HostMissilePhase::Attack,
            original_target_pos: Vec3::ZERO,
        }
    }
}

/// Resolve authored flight/warhead fields from the same Object INI store as
/// `HostProjectileLifecycle`.
pub fn host_projectile_flight_for_object_name(
    projectile_object_name: &str,
) -> Option<HostProjectileFlight> {
    with_projectile_behavior_module(projectile_object_name, |module_name, data| {
        if module_name.eq_ignore_ascii_case("DumbProjectileBehavior") {
            Some(HostProjectileFlight::Dumb(parse_dumb_flight(data)))
        } else if module_name.eq_ignore_ascii_case("MissileAIUpdate") {
            Some(HostProjectileFlight::Missile(parse_missile_flight(data)))
        } else {
            None
        }
    })
}

fn parse_dumb_flight(
    data: &dyn game_engine::common::thing::module::ModuleData,
) -> HostDumbProjectileFlight {
    HostDumbProjectileFlight {
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

fn parse_missile_flight(
    data: &dyn game_engine::common::thing::module::ModuleData,
) -> HostMissileFlight {
    HostMissileFlight {
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

fn host_to_cpp(pos: Vec3) -> gamelogic::common::Coord3D {
    gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y)
}

fn cpp_to_host(pos: gamelogic::common::Coord3D) -> Vec3 {
    Vec3::new(pos.x, pos.z, pos.y)
}

/// Highest intervening terrain along the 3D chord (C++ estimateTerrainExtremesAlongLine).
/// Empty / unmapped worlds return 0 so the Bezier still clears start/end Z.
pub fn estimate_highest_intervening_terrain(start: Vec3, end: Vec3) -> f32 {
    let start_cpp = host_to_cpp(start);
    let end_cpp = host_to_cpp(end);
    let mut highest = 0.0;
    if let Some(partition) = gamelogic::ThePartitionManager::get() {
        if partition.estimate_terrain_extremes_along_line(start_cpp, end_cpp, &mut highest) {
            return highest;
        }
    }
    if let Some(terrain) = gamelogic::helpers::TheTerrainLogic::get() {
        let dx = end_cpp.x - start_cpp.x;
        let dy = end_cpp.y - start_cpp.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let step = 10.0_f32.max(1.0);
        let steps = ((dist / step).ceil() as i32).max(1);
        let mut max_height = f32::MIN;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = start_cpp.x + dx * t;
            let y = start_cpp.y + dy * t;
            let z = terrain.get_ground_height(x, y, None);
            if z > max_height {
                max_height = z;
            }
        }
        if max_height.is_finite() {
            return max_height;
        }
    }
    0.0
}

/// C++ calcFlightPath in host Y-up space.
pub fn build_dumb_bezier_path(
    start: Vec3,
    end: Vec3,
    flight: &HostDumbProjectileFlight,
    speed_per_second: f32,
    existing_segments: usize,
) -> (Vec<Vec3>, usize) {
    let highest = estimate_highest_intervening_terrain(start, end);
    let speed_per_frame = if speed_per_second > 0.0 {
        speed_per_second / LOGIC_FRAMES_PER_SECOND
    } else {
        1.0
    };
    let (points, segs) =
        gamelogic::object::behavior::dumb_projectile_behavior::calc_dumb_projectile_bezier_points(
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
    runtime: &mut HostProjectileRuntime,
    flight: &HostDumbProjectileFlight,
    new_victim_center: Vec3,
) {
    if flight.flight_path_adjust_per_frame <= 0.0 {
        return;
    }
    let delta = new_victim_center - runtime.path_end;
    let dist_sq = delta.length_squared();
    if dist_sq <= 0.1 {
        return;
    }
    let dist = dist_sq.sqrt();
    let move_dist = dist.min(flight.flight_path_adjust_per_frame);
    if move_dist <= 0.0 {
        return;
    }
    runtime.path_end += delta * (move_dist / dist);
    let (path, segs) = build_dumb_bezier_path(
        runtime.path_start,
        runtime.path_end,
        flight,
        runtime.path_speed_per_frame * LOGIC_FRAMES_PER_SECOND,
        runtime.path_segments.max(1),
    );
    runtime.path = path;
    runtime.path_segments = segs;
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

fn kindof_multi(obj: &Object, required: &[KindOf], forbidden: &[KindOf]) -> bool {
    required.iter().all(|kind| obj.is_kind_of(*kind))
        && forbidden.iter().all(|kind| !obj.is_kind_of(*kind))
}

fn is_garrisonable_container(obj: &Object) -> bool {
    if obj.thing.template.garrison_contain_max.is_some() {
        return true;
    }
    obj.building_data
        .as_ref()
        .is_some_and(|building| building.max_garrison > 0)
}

/// C++ GarrisonHitKillCount collide path. Returns the building center when the
/// grenade/missile consumed itself without a normal detonation weapon.
pub fn apply_garrison_hit_kill(
    objects: &mut HashMap<ObjectId, Object>,
    container_id: ObjectId,
    launcher_id: ObjectId,
    flight: &HostProjectileFlight,
) -> Option<Vec3> {
    let count = flight.garrison_hit_kill_count();
    if count <= 0 {
        return None;
    }
    let contained = {
        let container = objects.get(&container_id)?;
        if !container.is_alive() || !is_garrisonable_container(container) {
            return None;
        }
        if container.is_immune_to_clear_building_attacks() {
            return None;
        }
        let units = container.contained_units();
        if units.is_empty() {
            return None;
        }
        units
    };

    let required = flight.garrison_required().to_vec();
    let forbidden = flight.garrison_forbidden().to_vec();
    let mut num_killed = 0;
    let mut killed = Vec::new();
    for occupant_id in contained {
        if num_killed >= count {
            break;
        }
        let Some(occupant) = objects.get_mut(&occupant_id) else {
            continue;
        };
        if !occupant.is_alive() || !kindof_multi(occupant, &required, &forbidden) {
            continue;
        }
        let _ = launcher_id;
        occupant.health.current = 0.0;
        occupant.status.destroyed = true;
        occupant.status.effectively_dead = true;
        killed.push(occupant_id);
        num_killed += 1;
    }
    if num_killed == 0 {
        return None;
    }
    if let Some(container) = objects.get_mut(&container_id) {
        for occupant_id in killed {
            let _ = container.remove_occupant(occupant_id);
        }
        return Some(container.get_position());
    }
    None
}

/// C++ getHighestLayerForDestination + GROUND transition while still over the
/// previous layer's XY. Armed missiles and all dumb projectiles detonate.
pub fn bridge_deck_detonate_pose(pos: Vec3, old_layer: u8, armed: bool) -> Option<(Vec3, u8)> {
    let Some(terrain) = gamelogic::helpers::TheTerrainLogic::get() else {
        return None;
    };
    let new_layer = terrain.get_highest_layer_for_destination(&host_to_cpp(pos)) as u8;
    if !armed || old_layer == LAYER_GROUND || new_layer != LAYER_GROUND {
        return Some((pos, new_layer));
    }
    let mut test = host_to_cpp(pos);
    test.z = 9999.0;
    let test_layer = terrain.get_highest_layer_for_destination(&test) as u8;
    if test_layer != old_layer {
        return Some((pos, new_layer));
    }
    let layer = gamelogic::common::PathfindLayerEnum::from_u32(old_layer as u32);
    let height = terrain.get_layer_height(test.x, test.y, layer) + BRIDGE_DECK_FUDGE;
    let mut snapped = pos;
    snapped.y = height;
    Some((snapped, new_layer))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_clears_intervening_ridge() {
        let flight = HostDumbProjectileFlight {
            first_height: 50.0,
            second_height: 150.0,
            first_percent_indent: 0.2,
            second_percent_indent: 0.9,
            ..HostDumbProjectileFlight::default()
        };
        let start = Vec3::new(0.0, 10.0, 0.0);
        let end = Vec3::new(100.0, 10.0, 0.0);
        let (path, segs) = build_dumb_bezier_path(start, end, &flight, 250.0, 0);
        assert!(segs > 1);
        assert!(path.len() > 1);
        let mid = path[path.len() / 2];
        assert!(
            mid.y > start.y + 40.0,
            "terrain-clearing arc must rise by FirstHeight, got {}",
            mid.y
        );
    }

    #[test]
    fn lock_distance_halves_for_ground_shots() {
        assert!((missile_lock_distance(75.0, true) - 75.0).abs() < f32::EPSILON);
        assert!((missile_lock_distance(75.0, false) - 37.5).abs() < f32::EPSILON);
        assert!(missile_inside_lock_distance(
            Vec3::ZERO,
            Vec3::new(30.0, 0.0, 0.0),
            75.0,
            false
        ));
        assert!(!missile_inside_lock_distance(
            Vec3::ZERO,
            Vec3::new(40.0, 0.0, 0.0),
            75.0,
            false
        ));
    }

    #[test]
    fn flight_path_adjust_clamps_end_point() {
        let flight = HostDumbProjectileFlight {
            flight_path_adjust_per_frame: 5.0,
            first_height: 10.0,
            second_height: 10.0,
            first_percent_indent: 0.5,
            second_percent_indent: 0.5,
            ..HostDumbProjectileFlight::default()
        };
        let mut runtime = HostProjectileRuntime {
            path_start: Vec3::ZERO,
            path_end: Vec3::new(100.0, 0.0, 0.0),
            path_speed_per_frame: 10.0,
            path_segments: 8,
            ..HostProjectileRuntime::default()
        };
        adjust_flight_path_end(&mut runtime, &flight, Vec3::new(130.0, 0.0, 0.0));
        assert!((runtime.path_end.x - 105.0).abs() < 0.01);
        assert!(!runtime.path.is_empty());
    }

    #[test]
    fn retail_flashbang_and_patriot_flight_fields_parse() {
        match host_projectile_flight_for_object_name("RangerFlashBangGrenade") {
            Some(HostProjectileFlight::Dumb(dumb)) => {
                assert!(dumb.first_height >= 0.0);
                assert!(dumb.second_height >= dumb.first_height || dumb.second_height >= 0.0);
            }
            other => panic!("expected DumbProjectile flight, got {other:?}"),
        }
        match host_projectile_flight_for_object_name("PatriotMissile") {
            Some(HostProjectileFlight::Missile(missile)) => {
                assert!(missile.lock_distance > 0.0);
                // Parsed even when empty; NONE is filtered to "".
                assert!(!missile.ignition_fx.eq_ignore_ascii_case("none"));
            }
            other => panic!("expected Missile flight, got {other:?}"),
        }
        for name in ["TomahawkMissile", "PatriotMissile", "StingerMissile"] {
            if let Some(HostProjectileFlight::Missile(missile)) =
                host_projectile_flight_for_object_name(name)
            {
                assert!(
                    !missile.ignition_fx.eq_ignore_ascii_case("none"),
                    "{name} IgnitionFX must not stay as NONE"
                );
            }
        }
    }
}
