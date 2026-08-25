//! Host RailroadBehavior residual (C++ `RailroadBehavior::update`).
//!
//! Live host never populates crate `OBJECT_REGISTRY`, so crate
//! `RailroadBehavior::update` early-outs. This residual is the production
//! path: locomotives follow track waypoints, wait at stations, and hitch
//! carriages — the minimal C++ `RailroadGuideAIUpdate.cpp` update loop.
//!
//! Sources:
//! - `RailroadGuideAIUpdate.cpp` `RailroadBehavior::update` (~652-832)
//! - `loadTrackData` (~480-616)
//! - `FindPosByPathDistance` (~1355-1482)
//! - `createCarriages` / `getPulled` / `updatePositionTrackDistance`
//!
//! Fail-closed: not full collide crush / xfer. Collide audio is C++
//! `playImpactSound` (bounce / TrainMeatyHit / TrainBigMetalHit / TrainSmallMetalHit).

use super::ObjectId;
use crate::game_logic::{AudioEventRequest, GameLogic, KindOf, Team};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;

/// C++ `RailroadBehaviorModuleData::m_speedMax` default.
pub const RAILROAD_SPEED_MAX: f32 = 4.0;
/// C++ `m_acceleration` default.
pub const RAILROAD_ACCELERATION: f32 = 1.01;
/// C++ `m_braking` default.
pub const RAILROAD_BRAKING: f32 = 0.99;
/// C++ `m_friction` default.
pub const RAILROAD_FRICTION: f32 = 0.97;
/// C++ `m_waitAtStationTime` default (logic frames).
pub const RAILROAD_WAIT_AT_STATION_FRAMES: i32 = 150;
/// C++ `0.02f * direction` push-start while accelerating.
pub const RAILROAD_ACCEL_PUSH: f32 = 0.02;
/// C++ station-depart kick `0.05f * direction`.
pub const RAILROAD_STATION_DEPART_SPEED: f32 = 0.05;
/// C++ `fabs(speed) < 0.1f` stop threshold.
pub const RAILROAD_STOP_SPEED: f32 = 0.1;
/// C++ `getMajorRadius()` fallback when host geometry is missing.
pub const RAILROAD_DEFAULT_HITCH_RADIUS: f32 = 20.0;
/// C++ `FRAMES_UNPULLED_LONG_ENOUGH_TO_UNHITCH`.
pub const RAILROAD_UNHITCH_FRAMES: i32 = 2;
/// C++ `RailroadBehaviorModuleData::m_runningSound` retail `TrainRunning`.
pub const RAILROAD_RUNNING_SOUND: &str = "TrainRunning";
/// C++ `m_whistleSound` retail `TrainWhistle`.
pub const RAILROAD_WHISTLE_SOUND: &str = "TrainWhistle";
pub const RAILROAD_CLICKETY_SOUND: &str = "TrainClickety";
/// Leftover INI `MeatyBounceSound` retail `TrainMeatyHit`.
pub const RAILROAD_MEATY_SOUND: &str = "TrainMeatyHit";
/// Leftover INI `BigMetalBounceSound` retail `TrainBigMetalHit`.
pub const RAILROAD_BIG_METAL_SOUND: &str = "TrainBigMetalHit";
/// Leftover INI `SmallMetalBounceSound` retail `TrainSmallMetalHit`.
pub const RAILROAD_SMALL_METAL_SOUND: &str = "TrainSmallMetalHit";
/// Leftover `RailroadBehaviorModuleData::kill_speed_min` default.
pub const RAILROAD_KILL_SPEED_MIN: f32 = 1.0;
/// Leftover `running_garrison_speed_max` default.
pub const RAILROAD_RUNNING_GARRISON_SPEED_MAX: f32 = 1.0;
/// Leftover / C++ `NORMAL_VEL_Z` in `playImpactSound`.
const RAILROAD_NORMAL_VEL_Z: f32 = 0.25;
/// Leftover / C++ `NORMAL_MASS` in `playImpactSound`.
const RAILROAD_NORMAL_MASS: f32 = 50.0;

#[derive(Debug, Clone)]
struct RailroadAudioCue {
    event_name: String,
    looping: bool,
    stop: bool,
    position: Option<Vec3>,
    player_index: Option<i32>,
    /// C++ `AudioEventRTS::setVolume` residual (impact only).
    volume: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
struct RailroadCollideSnap {
    id: ObjectId,
    waiting_in_wings: bool,
    end_of_line: bool,
    is_locomotive: bool,
    conductor_state: HostConductorState,
    speed: f32,
    hitch_radius: f32,
}

/// C++ `RailroadBehavior::playImpactSound` event pick.
/// Victim bounce sound wins; else KindOf: Infantry → Meaty,
/// HugeVehicle||Structure → BigMetal, Vehicle → SmallMetal.
pub fn leftover_railroad_impact_event_name(
    bounce_sound: &str,
    infantry: bool,
    huge_vehicle: bool,
    structure: bool,
    vehicle: bool,
) -> Option<String> {
    if !bounce_sound.is_empty() {
        return Some(bounce_sound.to_string());
    }
    if infantry {
        Some(RAILROAD_MEATY_SOUND.to_string())
    } else if huge_vehicle || structure {
        Some(RAILROAD_BIG_METAL_SOUND.to_string())
    } else if vehicle {
        Some(RAILROAD_SMALL_METAL_SOUND.to_string())
    } else {
        None
    }
}

pub fn leftover_railroad_impact_volume(vel_len: f32, victim_mass: f32, has_physics: bool) -> f32 {
    let mut vel = RAILROAD_NORMAL_VEL_Z;
    let mut mass = RAILROAD_NORMAL_MASS;
    if has_physics {
        vel += vel_len;
        mass += victim_mass;
        vel *= 0.5;
        mass *= 0.5;
    }
    vel = vel.clamp(0.0, RAILROAD_NORMAL_VEL_Z);
    mass = mass.clamp(0.0, RAILROAD_NORMAL_MASS);
    let mut volume = leftover_normalize_to_range(
        leftover_mu_law(vel, RAILROAD_NORMAL_VEL_Z, 500.0),
        -1.0,
        1.0,
        0.25,
        1.0,
    );
    volume *= leftover_normalize_to_range(
        leftover_mu_law(mass, RAILROAD_NORMAL_MASS, 500.0),
        -1.0,
        1.0,
        0.25,
        1.0,
    );
    volume
}

fn leftover_normalize_to_range(
    value: f32,
    in_min: f32,
    in_max: f32,
    out_min: f32,
    out_max: f32,
) -> f32 {
    if (in_max - in_min).abs() < f32::EPSILON {
        return out_min;
    }
    let t = (value - in_min) / (in_max - in_min);
    out_min + (out_max - out_min) * t
}

fn leftover_mu_law(value: f32, max_value: f32, mu: f32) -> f32 {
    let normalized = value / max_value.max(0.0001);
    let numerator = (1.0 + mu * normalized.abs()).ln();
    let denominator = (1.0 + mu).ln();
    normalized.signum() * (numerator / denominator)
}

fn leftover_railroad_faction_structure(obj: &crate::game_logic::Object) -> bool {
    // Leftover on_collide: FSPower / Factory / Defense / FSTechnology / RebuildHole.
    obj.is_kind_of(KindOf::FSPower)
        || obj.is_kind_of(KindOf::FSWarFactory)
        || obj.is_kind_of(KindOf::FSBarracks)
        || obj.is_kind_of(KindOf::FSAirfield)
        || obj.is_kind_of(KindOf::FSBaseDefense)
        || obj.is_kind_of(KindOf::FSTechnology)
        || obj
            .template_name
            .to_ascii_lowercase()
            .contains("rebuildhole")
}

fn leftover_railroad_demo_trap(obj: &crate::game_logic::Object) -> bool {
    obj.is_kind_of(KindOf::DemoTrap)
}

/// Leftover `on_collide` sites that call `play_impact_sound`.
fn leftover_railroad_should_play_impact(
    is_locomotive: bool,
    conductor: HostConductorState,
    speed: f32,
    victim_is_structure: bool,
    victim_is_faction: bool,
    victim_is_demo_trap: bool,
    victim_contained_by_train: bool,
    victim_entering_train: bool,
    victim_dead: bool,
) -> bool {
    if victim_is_structure && victim_is_faction {
        return true;
    }
    if victim_is_structure && victim_is_demo_trap {
        return true;
    }
    if conductor == HostConductorState::WaitAtStation
        && speed < RAILROAD_RUNNING_GARRISON_SPEED_MAX
        && victim_entering_train
    {
        return false;
    }
    if victim_contained_by_train {
        return false;
    }
    if conductor == HostConductorState::WaitAtStation
        || (conductor == HostConductorState::Coast && speed < RAILROAD_RUNNING_GARRISON_SPEED_MAX)
        || !is_locomotive
    {
        return false;
    }
    if victim_dead {
        return false;
    }
    // Leftover kill-speed path kills without play_impact_sound.
    speed < RAILROAD_KILL_SPEED_MIN
}

const FACADE_HANDLE: u32 = 0x00_FACADE;

fn alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// C++ `IsLocomotive = Yes` name residual (no KINDOF_TRAIN in retail).
pub fn is_railroad_locomotive_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    if n.contains("transport") || n.contains("railed") {
        return false;
    }
    n.contains("trainengine") || n.contains("locomotive")
}

/// Map-placed / INI carriage residual.
pub fn is_railroad_carriage_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() || is_railroad_locomotive_template(template_name) {
        return false;
    }
    if n.contains("transport") || n.contains("railed") {
        return false;
    }
    n.contains("traincar")
        || n.contains("traincoal")
        || n.contains("traintanker")
        || n.contains("traincaboose")
        || n.contains("carriage")
        || (n.contains("train")
            && (n.contains("car") || n.contains("coal") || n.contains("tanker")))
}

pub fn is_railroad_template(template_name: &str) -> bool {
    is_railroad_locomotive_template(template_name) || is_railroad_carriage_template(template_name)
}

/// C++ `RailroadBehavior::ConductorState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostConductorState {
    ApplyBrakes,
    WaitAtStation,
    Accelerate,
    Coast,
}

/// C++ `TrackPoint`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostTrackPoint {
    pub position: Vec3,
    pub distance_from_prev: f32,
    pub distance_from_first: f32,
    pub handle: u32,
    pub is_station: bool,
    pub is_disembark: bool,
    pub is_ping_pong: bool,
    pub is_tunnel_or_bridge: bool,
}

/// C++ `TrainTrack`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostTrainTrack {
    pub points: Vec<HostTrackPoint>,
    pub length: f32,
    pub is_looping: bool,
}

impl HostTrainTrack {
    /// C++ `RailroadBehavior::loadTrackData` walk of `getLink(0)`.
    pub fn from_linked_waypoints(
        waypoints: &[HostWaypointSnap],
        anchor_index: usize,
    ) -> Option<Self> {
        if waypoints.is_empty() || anchor_index >= waypoints.len() {
            return None;
        }
        let by_id: HashMap<u32, usize> = waypoints
            .iter()
            .enumerate()
            .map(|(i, w)| (w.id, i))
            .collect();
        let anchor = &waypoints[anchor_index];
        let mut track = HostTrainTrack::default();
        track.points.push(HostTrackPoint {
            position: anchor.position,
            distance_from_prev: 0.0,
            distance_from_first: 0.0,
            handle: anchor.id,
            is_station: anchor.name.ends_with("Station"),
            is_disembark: anchor.name.ends_with("Disembark"),
            is_ping_pong: false,
            is_tunnel_or_bridge: anchor.name.ends_with("Tunnel"),
        });
        let mut scanner_id = anchor.id;
        let mut hops = 0usize;
        loop {
            hops += 1;
            if hops > 4096 {
                break;
            }
            let Some(&idx) = by_id.get(&scanner_id) else {
                break;
            };
            let scanner = &waypoints[idx];
            let Some(next_id) = scanner.link0 else {
                break;
            };
            let Some(&next_idx) = by_id.get(&next_id) else {
                break;
            };
            let next = &waypoints[next_idx];
            let delta = next.position - scanner.position;
            let dist = delta.length();
            track.length += dist;
            track.points.push(HostTrackPoint {
                position: next.position,
                distance_from_prev: dist,
                distance_from_first: track.length,
                handle: scanner.id,
                is_station: next.name.ends_with("Station"),
                is_disembark: scanner.name.ends_with("Disembark"),
                is_ping_pong: scanner.name.ends_with("PingPong"),
                is_tunnel_or_bridge: next.name.ends_with("Tunnel"),
            });
            scanner_id = next.id;
            if scanner_id == anchor.id {
                track.is_looping = true;
                break;
            }
        }
        if track.points.len() < 2 {
            return None;
        }
        Some(track)
    }
}

/// Terrain / test waypoint snapshot (host XZ plane).
#[derive(Debug, Clone)]
pub struct HostWaypointSnap {
    pub id: u32,
    pub name: String,
    pub position: Vec3,
    pub link0: Option<u32>,
}

/// C++ Coord3D (x/y plane, z height) → host Vec3 (x/z plane, y height).
pub fn railroad_coord_to_host(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, z, y)
}

/// C++ `FindPosByPathDistance` interpolation.
pub fn find_pos_by_path_distance(
    track: &HostTrainTrack,
    dist: f32,
) -> (Vec3, bool, bool, Option<u32>) {
    if track.points.is_empty() {
        return (Vec3::ZERO, false, false, None);
    }
    let mut actual = dist;
    let mut waiting_in_wings = false;
    let mut end_of_line = false;
    if track.is_looping && track.length > 0.0 {
        while actual < 0.0 {
            actual += track.length;
        }
        while actual > track.length {
            actual -= track.length;
        }
    } else {
        if dist < 0.0 {
            waiting_in_wings = true;
        } else if dist >= track.length {
            end_of_line = true;
        }
        actual = dist.clamp(0.0, track.length);
    }
    if actual <= track.points[0].distance_from_first {
        return (
            track.points[0].position,
            waiting_in_wings,
            end_of_line,
            Some(track.points[0].handle),
        );
    }
    for pair in track.points.windows(2) {
        let this_pt = &pair[0];
        let next_pt = &pair[1];
        if this_pt.distance_from_first < actual && next_pt.distance_from_first >= actual {
            let difference = actual - this_pt.distance_from_first;
            let mut delta = next_pt.position - this_pt.position;
            let len = delta.length();
            if len > 0.0 {
                delta /= len;
            }
            return (
                this_pt.position + delta * difference,
                waiting_in_wings,
                end_of_line,
                Some(this_pt.handle),
            );
        }
    }
    let last = track.points.last().unwrap();
    (
        last.position,
        waiting_in_wings,
        end_of_line,
        Some(last.handle),
    )
}

/// Per-car C++ `RailroadBehavior` runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostRailroadCar {
    pub object_id: ObjectId,
    pub is_locomotive: bool,
    pub is_lead_carriage: bool,
    pub track_data_loaded: bool,
    pub carriages_created: bool,
    pub has_ever_been_hitched: bool,
    pub held: bool,
    pub disembark: bool,
    pub in_tunnel: bool,
    pub waiting_in_wings: bool,
    pub end_of_line: bool,
    pub conductor_state: HostConductorState,
    pub speed: f32,
    pub direction: f32,
    pub track_distance: f32,
    pub wait_at_station_timer: i32,
    pub current_point_handle: u32,
    pub most_recent_special_handle: u32,
    pub trailer_id: Option<ObjectId>,
    pub hitch_radius: f32,
    pub speed_max: f32,
    pub acceleration: f32,
    pub braking: f32,
    pub friction: f32,
    pub wait_at_station_time: i32,
    pub carriage_template_names: Vec<String>,
    pub track: Option<HostTrainTrack>,
    /// C++ `m_runningSound.isCurrentlyPlaying()`.
    #[serde(default)]
    pub running_sound_playing: bool,
    #[serde(skip)]
    pending_audio: Vec<RailroadAudioCue>,
}

impl HostRailroadCar {
    pub fn new_locomotive(object_id: ObjectId) -> Self {
        Self {
            object_id,
            is_locomotive: true,
            is_lead_carriage: true,
            track_data_loaded: false,
            carriages_created: false,
            has_ever_been_hitched: false,
            held: false,
            disembark: false,
            in_tunnel: false,
            waiting_in_wings: false,
            end_of_line: false,
            conductor_state: HostConductorState::Accelerate,
            speed: 0.0,
            direction: 1.0,
            track_distance: 0.0,
            wait_at_station_timer: 0,
            current_point_handle: FACADE_HANDLE,
            most_recent_special_handle: FACADE_HANDLE,
            trailer_id: None,
            hitch_radius: RAILROAD_DEFAULT_HITCH_RADIUS,
            speed_max: RAILROAD_SPEED_MAX,
            acceleration: RAILROAD_ACCELERATION,
            braking: RAILROAD_BRAKING,
            friction: RAILROAD_FRICTION,
            wait_at_station_time: RAILROAD_WAIT_AT_STATION_FRAMES,
            carriage_template_names: Vec::new(),
            track: None,
            running_sound_playing: false,
            pending_audio: Vec::new(),
        }
    }

    pub fn new_carriage(object_id: ObjectId) -> Self {
        let mut car = Self::new_locomotive(object_id);
        car.is_locomotive = false;
        car.is_lead_carriage = false;
        car.conductor_state = HostConductorState::Coast;
        car
    }

    fn play_running(&mut self) {
        if self.running_sound_playing {
            return;
        }
        self.running_sound_playing = true;
        self.pending_audio.push(RailroadAudioCue {
            event_name: RAILROAD_RUNNING_SOUND.to_string(),
            looping: true,
            stop: false,
            position: None,
            player_index: None,
            volume: None,
        });
    }

    fn stop_running(&mut self) {
        if !self.running_sound_playing {
            return;
        }
        self.running_sound_playing = false;
        self.pending_audio.push(RailroadAudioCue {
            event_name: RAILROAD_RUNNING_SOUND.to_string(),
            looping: true,
            stop: true,
            position: None,
            player_index: None,
            volume: None,
        });
    }

    fn take_pending_audio(&mut self) -> Vec<RailroadAudioCue> {
        std::mem::take(&mut self.pending_audio)
    }

    /// C++ `RailroadBehavior::playImpactSound` → pending TheAudio cue.
    fn play_impact_sound(
        &mut self,
        event_name: String,
        position: Vec3,
        player_index: Option<i32>,
        volume: f32,
    ) {
        self.pending_audio.push(RailroadAudioCue {
            event_name,
            looping: false,
            stop: false,
            position: Some(position),
            player_index,
            volume: Some(volume),
        });
    }

    /// C++ locomotive conductor block inside `RailroadBehavior::update`.
    pub fn tick_conductor(&mut self) {
        if !self.is_locomotive {
            return;
        }
        match self.conductor_state {
            HostConductorState::ApplyBrakes => {
                self.speed *= self.braking;
                if self.speed.abs() < RAILROAD_STOP_SPEED {
                    self.speed = 0.0;
                    self.wait_at_station_timer = self.wait_at_station_time;
                    self.conductor_state = HostConductorState::WaitAtStation;
                }
            }
            HostConductorState::WaitAtStation => {
                self.wait_at_station_timer -= 1;
                if self.wait_at_station_timer <= 0 && !self.held {
                    self.conductor_state = HostConductorState::Accelerate;
                    self.speed = RAILROAD_STATION_DEPART_SPEED * self.direction;
                    // C++ :713 depart → m_runningSound.
                    self.play_running();
                } else if self.wait_at_station_timer == self.wait_at_station_time / 4 {
                    // C++ :719-721 whistle at wait/4.
                    self.pending_audio.push(RailroadAudioCue {
                        event_name: RAILROAD_WHISTLE_SOUND.to_string(),
                        looping: false,
                        stop: false,
                        position: None,
                        player_index: None,
                        volume: None,
                    });
                }
            }
            HostConductorState::Accelerate => {
                self.speed += RAILROAD_ACCEL_PUSH * self.direction;
                self.speed *= self.acceleration;
                if self.speed > self.speed_max {
                    self.speed = self.speed_max;
                } else if self.speed < -self.speed_max {
                    self.speed = -self.speed_max;
                }
                // C++ :739-740 restart running if not already playing.
                self.play_running();
            }
            HostConductorState::Coast => {
                self.speed *= self.friction;
            }
        }
    }

    /// C++ lead-carriage `trackDistance += speed` + station sniff.
    pub fn advance_along_track(&mut self) -> Option<Vec3> {
        if self.track.is_none() {
            return None;
        }
        if self.is_lead_carriage {
            if self.conductor_state == HostConductorState::Coast {
                self.speed *= self.friction;
                // C++ :766 lead-carriage coast removes the running handle.
                self.stop_running();
            }
            let (is_looping, length) = self
                .track
                .as_ref()
                .map(|t| (t.is_looping, t.length))
                .unwrap_or((false, 0.0));
            self.track_distance += self.speed;
            if is_looping && length > 0.0 {
                while self.track_distance > length {
                    self.track_distance -= length;
                }
                while self.track_distance < 0.0 {
                    self.track_distance += length;
                }
            }
        }
        let track_distance = self.track_distance;
        let (pos, waiting, end, handle) = {
            let track = self.track.as_ref()?;
            find_pos_by_path_distance(track, track_distance)
        };
        self.waiting_in_wings = waiting;
        self.end_of_line = end;
        if let Some(handle) = handle {
            let edge = handle != self.current_point_handle;
            if edge {
                let pt = self
                    .track
                    .as_ref()
                    .and_then(|t| t.points.iter().find(|p| p.handle == handle).cloned());
                if let Some(pt) = pt {
                    self.in_tunnel = pt.is_tunnel_or_bridge;
                    if self.is_locomotive {
                        if pt.is_station || pt.is_disembark {
                            self.conductor_state = HostConductorState::ApplyBrakes;
                            self.disembark = pt.is_disembark;
                            // C++ :1432/:1438 station / disembark stop running.
                            self.stop_running();
                        } else if pt.is_ping_pong && self.most_recent_special_handle != handle {
                            self.most_recent_special_handle = handle;
                            self.conductor_state = HostConductorState::ApplyBrakes;
                            self.disembark = false;
                            self.stop_running();
                            self.direction = -self.direction;
                        }
                    }
                    // C++ :1451-1455 clickety at pose, volume speed/10 (pose only).
                    if !self.in_tunnel {
                        self.pending_audio.push(RailroadAudioCue {
                            event_name: RAILROAD_CLICKETY_SOUND.to_string(),
                            looping: false,
                            stop: false,
                            position: Some(pos),
                            player_index: None,
                            volume: None,
                        });
                    }
                }
                self.current_point_handle = handle;
            }
        }
        Some(pos)
    }
}

#[derive(Debug, Clone, Default)]
pub struct HostRailroadRegistry {
    cars: HashMap<ObjectId, HostRailroadCar>,
    /// Test / script inject: track used when TerrainLogic is empty.
    injected_tracks: HashMap<ObjectId, HostTrainTrack>,
    moved_count: u32,
}

impl HostRailroadRegistry {
    pub fn get(&self, id: ObjectId) -> Option<&HostRailroadCar> {
        self.cars.get(&id)
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut HostRailroadCar> {
        self.cars.get_mut(&id)
    }

    pub fn inject_track(&mut self, id: ObjectId, track: HostTrainTrack) {
        self.injected_tracks.insert(id, track);
    }

    pub fn moved_count(&self) -> u32 {
        self.moved_count
    }

    pub fn ensure_car(&mut self, id: ObjectId, locomotive: bool) -> &mut HostRailroadCar {
        self.cars.entry(id).or_insert_with(|| {
            if locomotive {
                HostRailroadCar::new_locomotive(id)
            } else {
                HostRailroadCar::new_carriage(id)
            }
        })
    }

    /// Replace a car after save/load. C++ `RailroadBehavior::xfer` v3.
    pub fn restore_car(&mut self, car: HostRailroadCar) {
        self.cars.insert(car.object_id, car);
    }

    fn collide_snaps(&self) -> Vec<RailroadCollideSnap> {
        self.cars
            .values()
            .map(|c| RailroadCollideSnap {
                id: c.object_id,
                waiting_in_wings: c.waiting_in_wings,
                end_of_line: c.end_of_line,
                is_locomotive: c.is_locomotive,
                conductor_state: c.conductor_state,
                speed: c.speed,
                hitch_radius: c.hitch_radius,
            })
            .collect()
    }

    fn drain_audio(&mut self) -> Vec<(ObjectId, RailroadAudioCue)> {
        let mut out = Vec::new();
        for car in self.cars.values_mut() {
            let id = car.object_id;
            out.extend(car.take_pending_audio().into_iter().map(|cue| (id, cue)));
        }
        out
    }
}

thread_local! {
    static RAILROAD: RefCell<HostRailroadRegistry> = RefCell::new(HostRailroadRegistry::default());
}

pub fn railroad_registry_reset() {
    RAILROAD.with(|r| *r.borrow_mut() = HostRailroadRegistry::default());
}

pub fn with_railroad_registry<R>(f: impl FnOnce(&HostRailroadRegistry) -> R) -> R {
    RAILROAD.with(|r| f(&r.borrow()))
}

pub fn with_railroad_registry_mut<R>(f: impl FnOnce(&mut HostRailroadRegistry) -> R) -> R {
    RAILROAD.with(|r| f(&mut r.borrow_mut()))
}

pub fn inject_railroad_track(id: ObjectId, track: HostTrainTrack) {
    with_railroad_registry_mut(|reg| reg.inject_track(id, track));
}

pub fn railroad_car(id: ObjectId) -> Option<HostRailroadCar> {
    with_railroad_registry(|reg| reg.get(id).cloned())
}

/// C++ `RailroadBehavior::xfer` load: restore conductor/track/hitch.
pub fn restore_railroad_car(car: HostRailroadCar) {
    with_railroad_registry_mut(|reg| reg.restore_car(car));
}

/// Snapshot TerrainLogic waypoints (C++ `TheTerrainLogic->getFirstWaypoint`).
pub fn snapshot_terrain_waypoints() -> Vec<HostWaypointSnap> {
    let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut cur = terrain.get_first_waypoint();
    while let Some(wp) = cur {
        let loc = wp.get_location();
        out.push(HostWaypointSnap {
            id: wp.get_id(),
            name: wp.get_name().as_str().to_string(),
            position: railroad_coord_to_host(loc.x, loc.y, loc.z),
            link0: wp.get_link(0),
        });
        cur = wp.get_next();
    }
    out
}

fn load_track_near(pos: Vec3, snaps: &[HostWaypointSnap]) -> Option<HostTrainTrack> {
    if snaps.is_empty() {
        return None;
    }
    let mut best = 0usize;
    let mut best_d = f32::MAX;
    for (i, wp) in snaps.iter().enumerate() {
        let d = (wp.position - pos).length();
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    HostTrainTrack::from_linked_waypoints(snaps, best)
}

fn heading_xz(from: Vec3, to: Vec3) -> f32 {
    let dx = to.x - from.x;
    let dz = to.z - from.z;
    if dx == 0.0 && dz == 0.0 {
        0.0
    } else {
        dz.atan2(dx)
    }
}

fn xz_dist(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

impl GameLogic {
    /// Live host railroad tick. C++ `RailroadBehavior::update` per locomotive.
    pub fn update_railroads(&mut self) {
        let living: Vec<(ObjectId, String, Vec3, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                if obj.status.under_construction || obj.construction_percent + 0.001 < 1.0 {
                    return None;
                }
                if !is_railroad_template(&obj.template_name)
                    && with_railroad_registry(|r| r.get(*id).is_none())
                {
                    return None;
                }
                Some((
                    *id,
                    obj.template_name.clone(),
                    obj.get_position(),
                    is_railroad_locomotive_template(&obj.template_name),
                ))
            })
            .collect();

        if living.is_empty() {
            return;
        }

        let terrain_snaps = snapshot_terrain_waypoints();

        // Ensure cars + load tracks (C++ loadTrackData once per locomotive).
        for (id, _name, pos, is_loco) in &living {
            with_railroad_registry_mut(|reg| {
                let injected = if *is_loco {
                    reg.injected_tracks.get(id).cloned()
                } else {
                    None
                };
                let car = reg.ensure_car(*id, *is_loco);
                if car.track_data_loaded {
                    return;
                }
                if *is_loco {
                    car.track = injected.or_else(|| load_track_near(*pos, &terrain_snaps));
                }
                car.track_data_loaded = true;
            });
        }

        // Share the locomotive track with any still-trackless carriages.
        let loco_tracks: Vec<(ObjectId, HostTrainTrack)> = with_railroad_registry(|reg| {
            living
                .iter()
                .filter_map(|(id, _, _, is_loco)| {
                    if !*is_loco {
                        return None;
                    }
                    reg.get(*id).and_then(|c| c.track.clone()).map(|t| (*id, t))
                })
                .collect()
        });

        let hitch_jobs: Vec<(ObjectId, Vec3, Team, f32, Vec<String>, Option<ObjectId>)> =
            with_railroad_registry(|reg| {
                living
                    .iter()
                    .filter_map(|(id, _, pos, is_loco)| {
                        if !*is_loco {
                            return None;
                        }
                        let car = reg.get(*id)?;
                        if car.carriages_created || car.track.is_none() {
                            return None;
                        }
                        let team = self.objects.get(id).map(|o| o.team).unwrap_or(Team::USA);
                        Some((
                            *id,
                            *pos,
                            team,
                            car.hitch_radius,
                            car.carriage_template_names.clone(),
                            car.trailer_id,
                        ))
                    })
                    .collect()
            });
        for (loco_id, loco_pos, team, hitch_r, templates, existing_trailer) in hitch_jobs {
            let mut trailer = existing_trailer;
            if trailer.is_none() {
                let max_r = hitch_r * 2.0;
                let mut best: Option<(ObjectId, f32)> = None;
                for (oid, name, pos, is_loco) in &living {
                    if *oid == loco_id || *is_loco {
                        continue;
                    }
                    if !is_railroad_carriage_template(name)
                        && with_railroad_registry(|r| {
                            r.get(*oid).map(|c| c.is_locomotive).unwrap_or(true)
                        })
                    {
                        continue;
                    }
                    let already = with_railroad_registry(|r| {
                        r.get(*oid)
                            .map(|c| c.has_ever_been_hitched)
                            .unwrap_or(false)
                    });
                    if already {
                        continue;
                    }
                    let d = xz_dist(loco_pos, *pos);
                    if d <= max_r * 3.0 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                        best = Some((*oid, d));
                    }
                }
                trailer = best.map(|(id, _)| id);
            }
            if trailer.is_none() {
                if let Some(first) = templates.first() {
                    let spawn_pos = Vec3::new(loco_pos.x - hitch_r * 2.0, loco_pos.y, loco_pos.z);
                    if let Some(cid) = self.create_object(first, team, spawn_pos) {
                        trailer = Some(cid);
                    }
                }
            }
            if let Some(tid) = trailer {
                with_railroad_registry_mut(|reg| {
                    if let Some(loco) = reg.get_mut(loco_id) {
                        loco.trailer_id = Some(tid);
                        loco.carriages_created = true;
                    }
                    let track = reg.get(loco_id).and_then(|c| c.track.clone());
                    let car = reg.ensure_car(tid, false);
                    car.has_ever_been_hitched = true;
                    if car.track.is_none() {
                        car.track = track;
                    }
                    car.track_data_loaded = true;
                });
                // Remaining INI carriages after the first.
                let mut parent = tid;
                for extra in templates.iter().skip(1) {
                    let parent_pos = self
                        .objects
                        .get(&parent)
                        .map(|o| o.get_position())
                        .unwrap_or(loco_pos);
                    let spawn_pos =
                        Vec3::new(parent_pos.x - hitch_r * 2.0, parent_pos.y, parent_pos.z);
                    let Some(cid) = self.create_object(extra, team, spawn_pos) else {
                        break;
                    };
                    with_railroad_registry_mut(|reg| {
                        if let Some(p) = reg.get_mut(parent) {
                            p.trailer_id = Some(cid);
                        }
                        let track = reg.get(loco_id).and_then(|c| c.track.clone());
                        let car = reg.ensure_car(cid, false);
                        car.has_ever_been_hitched = true;
                        car.track = track;
                        car.track_data_loaded = true;
                    });
                    parent = cid;
                }
            } else {
                with_railroad_registry_mut(|reg| {
                    if let Some(loco) = reg.get_mut(loco_id) {
                        loco.carriages_created = true;
                    }
                });
            }
        }

        // Tick locomotives (lead cars) then pull the hitch chain.
        let loco_ids: Vec<ObjectId> = living
            .iter()
            .filter_map(|(id, _, _, is_loco)| is_loco.then_some(*id))
            .collect();

        let mut poses: Vec<(ObjectId, Vec3, f32)> = Vec::new();
        let mut wall_ops: Vec<(ObjectId, bool)> = Vec::new();
        for loco_id in loco_ids {
            with_railroad_registry_mut(|reg| {
                if let Some(track) = loco_tracks
                    .iter()
                    .find(|(id, _)| *id == loco_id)
                    .map(|(_, t)| t.clone())
                {
                    if let Some(car) = reg.get_mut(loco_id) {
                        if car.track.is_none() {
                            car.track = Some(track);
                        }
                    }
                }
                let (
                    pos,
                    heading,
                    mut pull_dist,
                    pull_speed,
                    pull_dir,
                    mut next,
                    track,
                    before,
                    after,
                ) = {
                    let Some(loco) = reg.get_mut(loco_id) else {
                        return;
                    };
                    if loco.track.is_none() {
                        return;
                    }
                    let before = loco.conductor_state;
                    loco.tick_conductor();
                    let after = loco.conductor_state;
                    let Some(pos) = loco.advance_along_track() else {
                        return;
                    };
                    let heading = {
                        let track = loco.track.as_ref().unwrap();
                        let (ahead, _, _, _) = find_pos_by_path_distance(
                            track,
                            loco.track_distance + loco.hitch_radius.max(1.0),
                        );
                        heading_xz(pos, ahead)
                    };
                    (
                        pos,
                        heading,
                        loco.track_distance,
                        loco.speed,
                        loco.direction,
                        loco.trailer_id,
                        loco.track.clone(),
                        before,
                        after,
                    )
                };
                if before != HostConductorState::WaitAtStation
                    && after == HostConductorState::WaitAtStation
                {
                    wall_ops.push((loco_id, true));
                } else if before == HostConductorState::WaitAtStation
                    && after != HostConductorState::WaitAtStation
                {
                    wall_ops.push((loco_id, false));
                }
                poses.push((loco_id, pos, heading));
                reg.moved_count = reg.moved_count.saturating_add(1);

                // Pull trailers: C++ getPulled / updatePositionTrackDistance.
                while let Some(tid) = next {
                    let hitch_r = reg
                        .get(tid)
                        .map(|c| c.hitch_radius)
                        .unwrap_or(RAILROAD_DEFAULT_HITCH_RADIUS);
                    pull_dist -= hitch_r * 2.0;
                    let Some(car) = reg.get_mut(tid) else {
                        break;
                    };
                    if car.track.is_none() {
                        car.track = track.clone();
                    }
                    car.speed = pull_speed;
                    car.direction = pull_dir;
                    car.track_distance = pull_dist;
                    car.has_ever_been_hitched = true;
                    if let Some(pos) = car.advance_along_track() {
                        let heading = car
                            .track
                            .as_ref()
                            .map(|t| {
                                let (ahead, _, _, _) = find_pos_by_path_distance(
                                    t,
                                    car.track_distance + car.hitch_radius.max(1.0),
                                );
                                heading_xz(pos, ahead)
                            })
                            .unwrap_or(0.0);
                        poses.push((tid, pos, heading));
                    }
                    next = car.trailer_id;
                }
            });
        }

        for (head, on) in wall_ops.iter().copied().filter(|(_, on)| !*on) {
            self.make_a_wall_out_of_this_train(head, false);
        }
        for (id, pos, heading) in poses {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.set_position(pos);
                obj.set_orientation(heading);
            }
        }
        for (head, on) in wall_ops.iter().copied().filter(|(_, on)| *on) {
            self.make_a_wall_out_of_this_train(head, true);
        }

        // C++ RailroadBehavior::onCollide → playImpactSound.
        self.queue_railroad_collide_impact_audio();

        // C++ TheAudio add/remove on the locomotive / car pose.
        let audio = with_railroad_registry_mut(|reg| reg.drain_audio());
        for (id, cue) in audio {
            let pos = cue
                .position
                .or_else(|| self.objects.get(&id).map(|o| o.get_position()));
            let priority = cue
                .volume
                .map(|vol| (64.0 + vol * 136.0).clamp(0.0, 255.0) as u8)
                .unwrap_or(140);
            let mut req = AudioEventRequest::new(&cue.event_name)
                .with_object(id)
                .with_priority(priority);
            if let Some(pos) = pos {
                req = req.with_position(pos);
            }
            if cue.looping {
                req = req.looping();
            }
            if cue.stop {
                req = req.stopping();
            }
            if let Some(player_index) = cue.player_index {
                req = req.with_player_index(player_index);
            }
            self.queue_audio_event(req);
        }
    }

    /// C++ `RailroadBehavior::onCollide` sites that call `playImpactSound`.
    fn queue_railroad_collide_impact_audio(&mut self) {
        let snaps = with_railroad_registry(|reg| reg.collide_snaps());
        let mut jobs: Vec<(ObjectId, String, Vec3, Option<i32>, f32)> = Vec::new();
        for snap in snaps {
            if snap.waiting_in_wings || snap.end_of_line {
                continue;
            }
            let Some(train) = self.objects.get(&snap.id) else {
                continue;
            };
            if !train.is_alive() {
                continue;
            }
            let train_pos = train.get_position();
            let us_r = snap.hitch_radius.max(train.physics_collide_circle_radius());
            let victims: Vec<ObjectId> = self
                .objects
                .keys()
                .copied()
                .filter(|id| *id != snap.id)
                .collect();
            for vid in victims {
                let Some(victim) = self.objects.get(&vid) else {
                    continue;
                };
                if !victim.is_alive() {
                    continue;
                }
                if victim.is_kind_of(KindOf::NoCollide) {
                    continue;
                }
                let victim_is_rail = is_railroad_template(&victim.template_name)
                    || with_railroad_registry(|r| r.get(vid).is_some());
                if victim_is_rail {
                    continue;
                }
                let them_r = victim.physics_collide_circle_radius();
                let dist = xz_dist(train_pos, victim.get_position());
                if (us_r + them_r) - dist + 1.0 <= 0.0 {
                    continue;
                }
                let faction = leftover_railroad_faction_structure(victim);
                let demo = leftover_railroad_demo_trap(victim);
                let entering = victim.target == Some(snap.id);
                let contained = victim.contained_by == Some(snap.id);
                let dead = victim.status.effectively_dead;
                if !leftover_railroad_should_play_impact(
                    snap.is_locomotive,
                    snap.conductor_state,
                    snap.speed,
                    victim.is_kind_of(KindOf::Structure),
                    faction,
                    demo,
                    contained,
                    entering,
                    dead,
                ) {
                    continue;
                }
                let Some(event) = leftover_railroad_impact_event_name(
                    &victim.bounce_sound_name,
                    victim.is_kind_of(KindOf::Infantry),
                    victim.is_kind_of(KindOf::HugeVehicle),
                    victim.is_kind_of(KindOf::Structure),
                    victim.is_kind_of(KindOf::Vehicle),
                ) else {
                    continue;
                };
                // C++ buildings typically lack PhysicsBehavior → max MuLaw volume.
                let has_physics = !victim.is_kind_of(KindOf::Structure);
                let vel = victim.movement.velocity;
                let vel_len = (vel.x * vel.x + vel.y * vel.y + vel.z * vel.z).sqrt();
                let volume =
                    leftover_railroad_impact_volume(vel_len, victim.physics_mass, has_physics);
                jobs.push((
                    snap.id,
                    event,
                    victim.get_position(),
                    victim.owner_player_id.map(|p| p as i32),
                    volume,
                ));
            }
        }
        with_railroad_registry_mut(|reg| {
            for (car_id, event, pos, player, volume) in jobs {
                if let Some(car) = reg.get_mut(car_id) {
                    car.play_impact_sound(event, pos, player, volume);
                }
            }
        });
    }

    pub fn set_railroad_held(&mut self, id: ObjectId, held: bool) {
        with_railroad_registry_mut(|reg| {
            if let Some(car) = reg.get_mut(id) {
                car.held = held;
            }
        });
    }

    /// C++ `RailroadBehavior::makeAWallOutOfThisTrain` — recurse trailer hitch.
    fn make_a_wall_out_of_this_train(&mut self, head: ObjectId, on: bool) {
        let mut chain = Vec::new();
        let mut next = Some(head);
        with_railroad_registry(|reg| {
            while let Some(id) = next {
                chain.push(id);
                next = reg.get(id).and_then(|c| c.trailer_id);
            }
        });
        for id in chain {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if on {
                self.pathfinding_system.create_wall_from_object(obj);
            } else {
                self.pathfinding_system.remove_wall_from_object(obj);
            }
        }
    }
}

/// Honesty: C++ defaults and live tick symbol exist.
pub fn honesty_railroad_residual_ok() -> bool {
    RAILROAD_SPEED_MAX == 4.0
        && RAILROAD_ACCELERATION == 1.01
        && RAILROAD_BRAKING == 0.99
        && RAILROAD_WAIT_AT_STATION_FRAMES == 150
        && RAILROAD_UNHITCH_FRAMES == 2
        && RAILROAD_RUNNING_SOUND == "TrainRunning"
        && RAILROAD_WHISTLE_SOUND == "TrainWhistle"
        && RAILROAD_CLICKETY_SOUND == "TrainClickety"
        && RAILROAD_MEATY_SOUND == "TrainMeatyHit"
        && RAILROAD_BIG_METAL_SOUND == "TrainBigMetalHit"
        && RAILROAD_SMALL_METAL_SOUND == "TrainSmallMetalHit"
        && RAILROAD_KILL_SPEED_MIN == 1.0
        && is_railroad_locomotive_template("CivilianTrainEngine")
        && is_railroad_carriage_template("CivilianTrainCoalCar")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, ThingTemplate};

    fn straight_track(len: f32, station_at: Option<f32>) -> HostTrainTrack {
        let mut points = vec![
            HostTrackPoint {
                position: Vec3::new(0.0, 0.0, 0.0),
                distance_from_prev: 0.0,
                distance_from_first: 0.0,
                handle: 1,
                is_station: false,
                is_disembark: false,
                is_ping_pong: false,
                is_tunnel_or_bridge: false,
            },
            HostTrackPoint {
                position: Vec3::new(len * 0.5, 0.0, 0.0),
                distance_from_prev: len * 0.5,
                distance_from_first: len * 0.5,
                handle: 2,
                is_station: station_at
                    .map(|s| (s - len * 0.5).abs() < 1.0)
                    .unwrap_or(false),
                is_disembark: false,
                is_ping_pong: false,
                is_tunnel_or_bridge: false,
            },
            HostTrackPoint {
                position: Vec3::new(len, 0.0, 0.0),
                distance_from_prev: len * 0.5,
                distance_from_first: len,
                handle: 3,
                is_station: false,
                is_disembark: false,
                is_ping_pong: false,
                is_tunnel_or_bridge: false,
            },
        ];
        if let Some(s) = station_at {
            if (s - len * 0.5).abs() >= 1.0 {
                points[0].is_station = s.abs() < 1.0;
            }
        }
        HostTrainTrack {
            points,
            length: len,
            is_looping: true,
        }
    }

    fn spawn_train(logic: &mut GameLogic, name: &str, pos: Vec3) -> ObjectId {
        let mut tmpl = ThingTemplate::new(name);
        tmpl.add_kind_of(KindOf::Vehicle).set_health(1000.0);
        logic.templates.insert(name.into(), tmpl);
        let id = logic
            .create_object(name, Team::USA, pos)
            .expect("spawn railroad object");
        if let Some(o) = logic.objects.get_mut(&id) {
            o.construction_percent = 1.0;
            o.status.under_construction = false;
        }
        id
    }

    fn wall_blocked(logic: &GameLogic, id: ObjectId) -> bool {
        let Some(obj) = logic.host_object(id) else {
            return false;
        };
        let cell = logic
            .pathfinding_system
            .grid
            .world_to_grid(obj.get_position());
        logic.pathfinding_system.grid.is_static_blocked(cell)
    }

    /// C++ RailroadBehavior::update: locomotive advances trackDistance by speed.
    #[test]
    fn locomotive_moves_along_track() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let id = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        inject_railroad_track(id, straight_track(400.0, None));
        let start = logic.host_object(id).unwrap().get_position();
        for _ in 0..30 {
            logic.update_railroads();
        }
        let after = logic.host_object(id).unwrap().get_position();
        assert!(
            after.x > start.x + 1.0,
            "train must move along track x {start:?} -> {after:?}"
        );
        let car = railroad_car(id).expect("registered locomotive");
        assert!(car.track_distance > 0.0);
        assert!(car.speed > 0.0);
        assert_eq!(car.conductor_state, HostConductorState::Accelerate);
        assert!(
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == RAILROAD_RUNNING_SOUND
                    && e.is_looping
                    && !e.stop
                    && e.object_id == Some(id)
            }),
            "Accelerate must queue TrainRunning: {:?}",
            logic.queued_audio_events
        );
    }

    #[test]
    fn railroad_xfer_fields_survive_lifecycle_envelope() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let id = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        inject_railroad_track(id, straight_track(400.0, None));
        for _ in 0..20 {
            logic.update_railroads();
        }
        let before = railroad_car(id).expect("moving loco");
        assert!(before.speed > 0.0);
        assert!(before.track_distance > 0.0);
        let saved_speed = before.speed;
        let saved_dist = before.track_distance;
        let saved_state = before.conductor_state;

        let envelope = logic
            .host_object(id)
            .expect("obj")
            .entity_lifecycle_envelope();
        railroad_registry_reset();
        assert!(railroad_car(id).is_none());
        if let Some(obj) = logic.host_object_mut(id) {
            obj.entity_apply_lifecycle_envelope(&envelope)
                .expect("apply");
        }
        let after = railroad_car(id).expect("restored loco");
        assert_eq!(after.conductor_state, saved_state);
        assert!((after.speed - saved_speed).abs() < 1e-5);
        assert!((after.track_distance - saved_dist).abs() < 1e-5);
        assert!(after.track_data_loaded);
        railroad_registry_reset();
    }

    /// C++ FindPosByPathDistance station edge → APPLY_BRAKES → WAIT_AT_STATION.
    #[test]
    fn locomotive_waits_at_station_then_departs() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let id = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        inject_railroad_track(id, straight_track(80.0, Some(40.0)));
        with_railroad_registry_mut(|reg| {
            let car = reg.ensure_car(id, true);
            car.wait_at_station_time = 8;
            // C++ default 0.99 needs ~370 frames from SpeedMax; tighten for the test.
            car.braking = 0.8;
        });
        let mut saw_wait = false;
        for _ in 0..200 {
            logic.update_railroads();
            if let Some(car) = railroad_car(id) {
                if car.conductor_state == HostConductorState::WaitAtStation {
                    saw_wait = true;
                    assert!(car.speed.abs() < RAILROAD_STOP_SPEED + 0.01);
                    assert!(
                        wall_blocked(&logic, id),
                        "createAWall must stamp the stopped locomotive"
                    );
                    break;
                }
            }
        }
        assert!(saw_wait, "station waypoint must apply brakes");
        for _ in 0..20 {
            logic.update_railroads();
        }
        let car = railroad_car(id).unwrap();
        assert_eq!(car.conductor_state, HostConductorState::Accelerate);
        assert!(car.speed.abs() > 0.0, "must depart after WaitAtStationTime");
        assert!(
            !wall_blocked(&logic, id),
            "removeWall must clear the locomotive on depart"
        );
    }

    /// hq-w1rig: hitch chain is stamped at station and cleared on depart.
    #[test]
    fn station_stop_stamps_train_and_carriage_wall() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let loco = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::new(0.0, 0.0, 0.0));
        let car = spawn_train(
            &mut logic,
            "CivilianTrainCoalCar",
            Vec3::new(-10.0, 0.0, 0.0),
        );
        inject_railroad_track(loco, straight_track(80.0, Some(40.0)));
        with_railroad_registry_mut(|reg| {
            let loco_car = reg.ensure_car(loco, true);
            loco_car.wait_at_station_time = 8;
            loco_car.braking = 0.8;
        });
        let mut saw_wait = false;
        for _ in 0..200 {
            logic.update_railroads();
            if railroad_car(loco)
                .map(|c| c.conductor_state == HostConductorState::WaitAtStation)
                .unwrap_or(false)
            {
                saw_wait = true;
                assert!(wall_blocked(&logic, loco), "loco wall at station");
                assert!(
                    wall_blocked(&logic, car),
                    "carriage wall via makeAWall recurse"
                );
                break;
            }
        }
        assert!(saw_wait, "must stop at station");
        for _ in 0..20 {
            logic.update_railroads();
        }
        assert_eq!(
            railroad_car(loco).unwrap().conductor_state,
            HostConductorState::Accelerate
        );
        assert!(!wall_blocked(&logic, loco), "removeWall on station depart");
        assert!(
            !wall_blocked(&logic, car),
            "carriage wall must clear on depart"
        );
        railroad_registry_reset();
    }

    #[test]
    fn script_set_train_held_blocks_station_departure() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        logic.scripts_loaded = true;
        let id = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        if let Some(obj) = logic.objects.get_mut(&id) {
            obj.name = "CivilianTrain".into();
        }
        inject_railroad_track(id, straight_track(80.0, Some(40.0)));
        with_railroad_registry_mut(|reg| {
            let car = reg.ensure_car(id, true);
            car.conductor_state = HostConductorState::WaitAtStation;
            car.wait_at_station_timer = 0;
            car.held = false;
        });

        let _ = gamelogic::scripting::take_host_set_train_held_requests();
        gamelogic::scripting::request_host_set_train_held("CivilianTrain", true);
        logic.evaluate_and_execute_scripts(0.0);
        assert!(
            railroad_car(id).expect("car").held,
            "SET_TRAIN_HELD must write HostRailroadCar.held"
        );
        for _ in 0..8 {
            logic.update_railroads();
        }
        assert_eq!(
            railroad_car(id).expect("car").conductor_state,
            HostConductorState::WaitAtStation,
            "held train must not leave WaitAtStation"
        );

        gamelogic::scripting::request_host_set_train_held("CivilianTrain", false);
        logic.evaluate_and_execute_scripts(0.0);
        assert!(!railroad_car(id).expect("car").held);
        logic.update_railroads();
        assert_eq!(
            railroad_car(id).expect("car").conductor_state,
            HostConductorState::Accelerate,
            "released train must depart the station"
        );
        railroad_registry_reset();
    }

    /// C++ getPulled: carriage trackDistance = puller - 2*hitchRadius.
    #[test]
    fn hitch_pulls_carriage_behind_locomotive() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let loco = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::new(0.0, 0.0, 0.0));
        let car = spawn_train(
            &mut logic,
            "CivilianTrainCoalCar",
            Vec3::new(-10.0, 0.0, 0.0),
        );
        inject_railroad_track(loco, straight_track(400.0, None));
        for _ in 0..40 {
            logic.update_railroads();
        }
        let loco_pos = logic.host_object(loco).unwrap().get_position();
        let car_pos = logic.host_object(car).unwrap().get_position();
        assert!(loco_pos.x > 1.0, "loco moved {loco_pos:?}");
        assert!(
            car_pos.x < loco_pos.x,
            "carriage must trail locomotive {car_pos:?} vs {loco_pos:?}"
        );
        let loco_state = railroad_car(loco).unwrap();
        let car_state = railroad_car(car).unwrap();
        assert_eq!(loco_state.trailer_id, Some(car));
        assert!(car_state.has_ever_been_hitched);
        assert!(
            (loco_state.track_distance - car_state.track_distance - loco_state.hitch_radius * 2.0)
                .abs()
                < 1.0,
            "hitch spacing loco={} car={}",
            loco_state.track_distance,
            car_state.track_distance
        );
    }

    #[test]
    fn load_track_from_linked_waypoints_matches_cpp_walk() {
        let snaps = vec![
            HostWaypointSnap {
                id: 10,
                name: "TrainPathStart".into(),
                position: Vec3::new(0.0, 0.0, 0.0),
                link0: Some(11),
            },
            HostWaypointSnap {
                id: 11,
                name: "TrainPathStation".into(),
                position: Vec3::new(100.0, 0.0, 0.0),
                link0: Some(12),
            },
            HostWaypointSnap {
                id: 12,
                name: "TrainPathEnd".into(),
                position: Vec3::new(200.0, 0.0, 0.0),
                link0: None,
            },
        ];
        let track = HostTrainTrack::from_linked_waypoints(&snaps, 0).unwrap();
        assert!(!track.is_looping);
        assert!((track.length - 200.0).abs() < 0.01);
        assert!(track.points[1].is_station);
        let (p, _, end, _) = find_pos_by_path_distance(&track, 50.0);
        assert!((p.x - 50.0).abs() < 0.01);
        assert!(!end);
        let (_, _, end, _) = find_pos_by_path_distance(&track, 250.0);
        assert!(end);
    }

    #[test]
    fn honesty_pack_matches_cpp_defaults() {
        assert!(honesty_railroad_residual_ok());
        assert!(!is_railroad_locomotive_template("AmericaVehicleChinook"));
        assert!(!is_railroad_template("RailedTransport"));
    }

    /// Conductor audio without GameLogic: whistle at wait/4, running on depart.
    #[test]
    fn conductor_audio_cues_match_cpp_handles() {
        let mut car = HostRailroadCar::new_locomotive(ObjectId(1));
        car.conductor_state = HostConductorState::WaitAtStation;
        car.wait_at_station_time = 8;
        car.wait_at_station_timer = 3;
        car.tick_conductor();
        let cues = car.take_pending_audio();
        assert!(
            cues.iter()
                .any(|c| c.event_name == RAILROAD_WHISTLE_SOUND && !c.stop),
            "wait/4 whistle: {cues:?}"
        );
        car.tick_conductor();
        car.tick_conductor();
        let cues = car.take_pending_audio();
        assert!(
            cues.iter()
                .any(|c| c.event_name == RAILROAD_RUNNING_SOUND && c.looping && !c.stop),
            "depart running: {cues:?}"
        );
        assert!(car.running_sound_playing);
        car.tick_conductor();
        let cues = car.take_pending_audio();
        assert!(
            cues.iter()
                .all(|c| c.event_name != RAILROAD_RUNNING_SOUND || c.stop),
            "no restart: {cues:?}"
        );

        let mut car = HostRailroadCar::new_locomotive(ObjectId(2));
        car.track = Some(straight_track(400.0, None));
        car.running_sound_playing = true;
        let pos = car.advance_along_track().expect("pos");
        let cues = car.take_pending_audio();
        assert!(
            cues.iter()
                .any(|c| { c.event_name == RAILROAD_CLICKETY_SOUND && c.position == Some(pos) }),
            "clickety at pose: {cues:?}"
        );
    }

    /// C++ :713/:721/:739-740/:1451-1455 Running / Whistle / Clickety.
    #[test]
    fn locomotive_queues_running_whistle_and_clickety() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let id = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        inject_railroad_track(id, straight_track(400.0, None));
        logic.update_railroads();
        assert!(
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == RAILROAD_RUNNING_SOUND
                    && e.is_looping
                    && !e.stop
                    && e.object_id == Some(id)
            }),
            "depart/accelerate must queue TrainRunning: {:?}",
            logic.queued_audio_events
        );
        assert!(
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == RAILROAD_CLICKETY_SOUND
                    && e.object_id == Some(id)
                    && e.position.is_some()
                    && !e.is_looping
            }),
            "track-joint edge must queue TrainClickety at pose: {:?}",
            logic.queued_audio_events
        );
        logic.queued_audio_events.clear();
        logic.update_railroads();
        assert!(
            logic
                .queued_audio_events
                .iter()
                .all(|e| e.event_type != RAILROAD_RUNNING_SOUND || e.stop),
            "already-playing running must not restart: {:?}",
            logic.queued_audio_events
        );

        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let id = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        inject_railroad_track(id, straight_track(80.0, Some(40.0)));
        with_railroad_registry_mut(|reg| {
            let car = reg.ensure_car(id, true);
            car.conductor_state = HostConductorState::WaitAtStation;
            car.wait_at_station_time = 8;
            car.wait_at_station_timer = 3;
            car.held = false;
            car.running_sound_playing = true;
            car.track_data_loaded = true;
        });
        logic.update_railroads();
        assert!(
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == RAILROAD_WHISTLE_SOUND && e.object_id == Some(id) && !e.stop
            }),
            "wait/4 must queue TrainWhistle: {:?}",
            logic.queued_audio_events
        );
        logic.queued_audio_events.clear();
        logic.update_railroads();
        logic.update_railroads();
        assert!(
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == RAILROAD_RUNNING_SOUND
                    && e.is_looping
                    && !e.stop
                    && e.object_id == Some(id)
            }),
            "station depart must queue TrainRunning: {:?}",
            logic.queued_audio_events
        );
        railroad_registry_reset();
    }

    fn spawn_victim(logic: &mut GameLogic, name: &str, pos: Vec3, kinds: &[KindOf]) -> ObjectId {
        let mut tmpl = ThingTemplate::new(name);
        for kind in kinds {
            tmpl.add_kind_of(*kind);
        }
        tmpl.set_health(100.0);
        logic.templates.insert(name.into(), tmpl);
        let id = logic
            .create_object(name, Team::USA, pos)
            .expect("spawn victim");
        if let Some(o) = logic.objects.get_mut(&id) {
            o.construction_percent = 1.0;
            o.status.under_construction = false;
            o.owner_player_id = Some(3);
            o.selection_radius = 8.0;
        }
        id
    }

    #[test]
    fn leftover_impact_event_name_matches_cpp_kindof() {
        assert_eq!(
            leftover_railroad_impact_event_name("", true, false, false, false).as_deref(),
            Some(RAILROAD_MEATY_SOUND)
        );
        assert_eq!(
            leftover_railroad_impact_event_name("", false, false, false, true).as_deref(),
            Some(RAILROAD_SMALL_METAL_SOUND)
        );
        assert_eq!(
            leftover_railroad_impact_event_name("", false, true, false, true).as_deref(),
            Some(RAILROAD_BIG_METAL_SOUND)
        );
        assert_eq!(
            leftover_railroad_impact_event_name("", false, false, true, false).as_deref(),
            Some(RAILROAD_BIG_METAL_SOUND)
        );
        assert_eq!(
            leftover_railroad_impact_event_name("", false, false, false, false).as_deref(),
            None
        );
        assert_eq!(
            leftover_railroad_impact_event_name("VictimBounce", true, false, false, false)
                .as_deref(),
            Some("VictimBounce")
        );
        let vol = leftover_railroad_impact_volume(0.0, 1.0, true);
        assert!(
            (0.25..=1.0).contains(&vol),
            "C++ MuLaw volume in [0.25, 1.0]: {vol}"
        );
    }

    #[test]
    fn collide_infantry_queues_leftover_meaty_hit() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let loco = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        inject_railroad_track(loco, straight_track(400.0, None));
        let victim = spawn_victim(&mut logic, "AmericaRanger", Vec3::ZERO, &[KindOf::Infantry]);
        logic.update_railroads();
        assert!(
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == RAILROAD_MEATY_SOUND
                    && e.object_id == Some(loco)
                    && e.player_index == Some(3)
                    && e.position.is_some()
                    && !e.stop
            }),
            "sub-kill infantry collide must queue TrainMeatyHit: {:?}",
            logic.queued_audio_events
        );
        let _ = victim;
        railroad_registry_reset();
    }

    #[test]
    fn collide_vehicle_queues_cpp_small_metal_hit() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let loco = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        inject_railroad_track(loco, straight_track(400.0, None));
        let _victim = spawn_victim(
            &mut logic,
            "AmericaTankCrusader",
            Vec3::ZERO,
            &[KindOf::Vehicle],
        );
        logic.update_railroads();
        assert!(
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == RAILROAD_SMALL_METAL_SOUND && e.object_id == Some(loco)
            }),
            "vehicle collide must queue TrainSmallMetalHit: {:?}",
            logic.queued_audio_events
        );
        railroad_registry_reset();
    }

    #[test]
    fn collide_huge_vehicle_queues_cpp_big_metal_hit() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let loco = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        inject_railroad_track(loco, straight_track(400.0, None));
        let _victim = spawn_victim(
            &mut logic,
            "ChinaTankOverlord",
            Vec3::ZERO,
            &[KindOf::Vehicle, KindOf::HugeVehicle],
        );
        logic.update_railroads();
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|e| { e.event_type == RAILROAD_BIG_METAL_SOUND && e.object_id == Some(loco) }),
            "huge-vehicle collide must queue TrainBigMetalHit: {:?}",
            logic.queued_audio_events
        );
        railroad_registry_reset();
    }

    #[test]
    fn collide_faction_structure_queues_leftover_big_metal() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let loco = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        inject_railroad_track(loco, straight_track(400.0, None));
        let _victim = spawn_victim(
            &mut logic,
            "AmericaPowerPlant",
            Vec3::ZERO,
            &[KindOf::Structure, KindOf::FSPower],
        );
        // Leftover faction path plays impact even at kill speed.
        with_railroad_registry_mut(|reg| {
            if let Some(car) = reg.get_mut(loco) {
                car.speed = 3.0;
            }
        });
        logic.update_railroads();
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|e| { e.event_type == RAILROAD_BIG_METAL_SOUND && e.object_id == Some(loco) }),
            "faction structure collide must queue TrainBigMetalHit: {:?}",
            logic.queued_audio_events
        );
        railroad_registry_reset();
    }

    #[test]
    fn collide_bounce_sound_wins_over_kindof() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let loco = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        inject_railroad_track(loco, straight_track(400.0, None));
        let victim = spawn_victim(&mut logic, "BounceRanger", Vec3::ZERO, &[KindOf::Infantry]);
        if let Some(o) = logic.objects.get_mut(&victim) {
            o.bounce_sound_name = "VictimBounce".into();
        }
        logic.update_railroads();
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|e| { e.event_type == "VictimBounce" && e.object_id == Some(loco) }),
            "leftover bounce sound must win: {:?}",
            logic.queued_audio_events
        );
        assert!(
            logic
                .queued_audio_events
                .iter()
                .all(|e| e.event_type != RAILROAD_MEATY_SOUND),
            "bounce must suppress Meaty: {:?}",
            logic.queued_audio_events
        );
        railroad_registry_reset();
    }

    #[test]
    fn collide_kill_speed_does_not_play_impact() {
        railroad_registry_reset();
        let mut logic = GameLogic::new();
        let loco = spawn_train(&mut logic, "CivilianTrainEngine", Vec3::ZERO);
        inject_railroad_track(loco, straight_track(400.0, None));
        logic.update_railroads();
        logic.queued_audio_events.clear();
        with_railroad_registry_mut(|reg| {
            if let Some(car) = reg.get_mut(loco) {
                car.speed = RAILROAD_KILL_SPEED_MIN;
                car.conductor_state = HostConductorState::Accelerate;
            }
        });
        let loco_pos = logic.host_object(loco).unwrap().get_position();
        let _victim = spawn_victim(&mut logic, "KillSpeedRanger", loco_pos, &[KindOf::Infantry]);
        logic.update_railroads();
        assert!(
            logic
                .queued_audio_events
                .iter()
                .all(|e| e.event_type != RAILROAD_MEATY_SOUND
                    && e.event_type != RAILROAD_BIG_METAL_SOUND
                    && e.event_type != RAILROAD_SMALL_METAL_SOUND),
            "leftover kill-speed path has no play_impact_sound: {:?}",
            logic.queued_audio_events
        );
        railroad_registry_reset();
    }
}
