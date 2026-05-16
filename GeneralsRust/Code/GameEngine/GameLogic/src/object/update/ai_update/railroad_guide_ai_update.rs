//! RailroadBehavior - rail track following logic for trains.
//!
//! Ported from GameLogic/Object/Update/AIUpdate/RailroadGuideAIUpdate.cpp.

use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::ai::THE_AI;
use crate::common::audio::AudioEventRts;
use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, Coord3D, Int, ModelConditionFlags, ObjectID, Real, UnsignedInt, WaypointID,
    FROM_CENTER_2D, INVALID_ID, INVALID_WAYPOINT_ID,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{
    TheAudio, TheGameLogic, ThePartitionManager, TheTerrainLogic, TheThingFactory,
};
use crate::modules::{
    BehaviorModuleInterface, CollideModuleInterface, ContainModuleInterfaceExt, PhysicsBehavior,
    UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::PhysicsBehaviorModuleData;
use crate::object::contain::open_contain::ObjectRelationship;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData, NameKeyType, TrainControlInterface, TrainPullInfo,
};
use glam::{Mat4, Vec3};

const NORMAL_VEL_Z: Real = 0.25;
const NORMAL_MASS: Real = 50.0;
const FRAMES_UNPULLED_LONG_ENOUGH_TO_UNHITCH: Int = 2;
const FACADE_WAYPOINT_ID: WaypointID = 0x00_FACADE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StationTask {
    DoNothing,
    Disembark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConductorState {
    ApplyBrakes,
    WaitAtStation,
    Accelerate,
    Coast,
}

#[derive(Debug, Clone)]
struct TrackPoint {
    position: Coord3D,
    distance_from_prev: Real,
    distance_from_first: Real,
    is_first_point: Bool,
    is_last_point: Bool,
    is_tunnel_or_bridge: Bool,
    is_station: Bool,
    is_disembark: Bool,
    is_ping_pong: Bool,
    handle: WaypointID,
}

impl TrackPoint {
    fn new() -> Self {
        Self {
            position: Coord3D::ZERO,
            distance_from_prev: 0.0,
            distance_from_first: 0.0,
            is_first_point: false,
            is_last_point: false,
            is_tunnel_or_bridge: false,
            is_station: false,
            is_disembark: false,
            is_ping_pong: false,
            handle: FACADE_WAYPOINT_ID,
        }
    }

    #[allow(dead_code)]
    fn clear(&mut self) {
        *self = Self::new();
    }
}

#[derive(Debug)]
struct TrainTrack {
    ref_count: Int,
    length: Real,
    is_looping: Bool,
    point_list: Vec<TrackPoint>,
}

impl TrainTrack {
    fn new() -> Self {
        Self {
            ref_count: 1,
            length: 0.0,
            is_looping: false,
            point_list: Vec::new(),
        }
    }

    fn inc_reference(&mut self) {
        self.ref_count += 1;
    }

    #[allow(dead_code)]
    fn release_reference(&mut self) -> Bool {
        self.ref_count -= 1;
        self.ref_count <= 0
    }

    fn get_point_list(&self) -> Option<&[TrackPoint]> {
        Some(&self.point_list)
    }

    fn get_writable_point_list(&mut self) -> Option<&mut Vec<TrackPoint>> {
        if self.ref_count == 1 {
            Some(&mut self.point_list)
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct RailroadPhysicsHandle {
    velocity: Coord3D,
    mass: Real,
    allow_bouncing: Bool,
    allow_airborne_friction: Bool,
    allow_to_fall: Bool,
    yaw_rate: Real,
    pitch_rate: Real,
    roll_rate: Real,
    extra_friction: Real,
    extra_bounciness: Real,
    turning: i32,
    bounce_sound: Option<AudioEventRts>,
    last_collidee: ObjectID,
}

impl RailroadPhysicsHandle {
    fn new(mass: Real) -> Self {
        Self {
            velocity: Coord3D::ZERO,
            mass,
            allow_bouncing: false,
            allow_airborne_friction: false,
            allow_to_fall: false,
            yaw_rate: 0.0,
            pitch_rate: 0.0,
            roll_rate: 0.0,
            extra_friction: 0.0,
            extra_bounciness: 0.0,
            turning: 0,
            bounce_sound: None,
            last_collidee: INVALID_ID,
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |result: std::io::Result<()>| result.map_err(|e| e.to_string());
        xfer.xfer_coord3d(&mut self.velocity);
        xfer_io(xfer.xfer_real(&mut self.mass))?;
        xfer_io(xfer.xfer_bool(&mut self.allow_bouncing))?;
        xfer_io(xfer.xfer_bool(&mut self.allow_airborne_friction))?;
        xfer_io(xfer.xfer_bool(&mut self.allow_to_fall))?;
        xfer_io(xfer.xfer_real(&mut self.yaw_rate))?;
        xfer_io(xfer.xfer_real(&mut self.pitch_rate))?;
        xfer_io(xfer.xfer_real(&mut self.roll_rate))?;
        xfer_io(xfer.xfer_real(&mut self.extra_friction))?;
        xfer_io(xfer.xfer_real(&mut self.extra_bounciness))?;
        xfer_io(xfer.xfer_object_id(&mut self.last_collidee))?;
        xfer_io(xfer.xfer_int(&mut self.turning))?;
        Ok(())
    }
}

impl PhysicsBehavior for RailroadPhysicsHandle {
    fn update(&mut self, _dt: f32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn get_velocity(&self) -> Coord3D {
        self.velocity
    }

    fn set_velocity(&mut self, velocity: &Coord3D) {
        self.velocity = *velocity;
    }

    fn is_on_ground(&self) -> bool {
        !self.allow_to_fall
    }

    fn apply_force(&mut self, force: &Coord3D) {
        let mass = if self.mass.abs() < 0.0001 {
            0.0001
        } else {
            self.mass
        };
        self.velocity += *force / mass;
    }

    fn set_yaw_rate(&mut self, rate: Real) {
        self.yaw_rate = rate;
    }

    fn set_roll_rate(&mut self, rate: Real) {
        self.roll_rate = rate;
    }

    fn set_pitch_rate(&mut self, rate: Real) {
        self.pitch_rate = rate;
    }

    fn set_mass(&mut self, mass: Real) {
        self.mass = mass;
    }

    fn get_mass(&self) -> Real {
        self.mass
    }

    fn set_extra_friction(&mut self, friction: Real) {
        self.extra_friction = friction;
    }

    fn set_extra_bounciness(&mut self, bounciness: Real) {
        self.extra_bounciness = bounciness;
    }

    fn set_allow_bouncing(&mut self, allow: bool) {
        self.allow_bouncing = allow;
    }

    fn set_allow_airborne_friction(&mut self, allow: bool) {
        self.allow_airborne_friction = allow;
    }

    fn set_allow_to_fall(&mut self, allow: bool) {
        self.allow_to_fall = allow;
    }

    fn get_allow_to_fall(&self) -> bool {
        self.allow_to_fall
    }

    fn set_turning(&mut self, turning: i32) {
        self.turning = turning;
    }

    fn set_bounce_sound(&mut self, sound: Option<AudioEventRts>) {
        self.bounce_sound = sound;
    }

    fn get_bounce_sound(&self) -> Option<AudioEventRts> {
        self.bounce_sound.clone()
    }

    fn get_ignore_collisions_with(&self) -> ObjectID {
        INVALID_ID
    }

    fn set_ignore_collisions_with(&mut self, _obj_id: ObjectID) {
        // Railroad physics does not participate in collisions.
    }

    fn get_last_collidee(&self) -> ObjectID {
        self.last_collidee
    }

    fn get_turning(&self) -> Real {
        self.turning as Real
    }
}

#[derive(Debug, Clone)]
struct PullInfo {
    direction: Real,
    speed: Real,
    track_distance: Real,
    tow_hitch_position: Coord3D,
    most_recent_special_point_handle: WaypointID,
    previous_waypoint: WaypointID,
    current_waypoint: WaypointID,
}

impl Default for PullInfo {
    fn default() -> Self {
        Self {
            direction: 1.0,
            speed: 0.0,
            track_distance: 0.0,
            tow_hitch_position: Coord3D::ZERO,
            most_recent_special_point_handle: FACADE_WAYPOINT_ID,
            previous_waypoint: FACADE_WAYPOINT_ID,
            current_waypoint: FACADE_WAYPOINT_ID,
        }
    }
}

impl PullInfo {
    fn to_train_pull_info(&self) -> TrainPullInfo {
        TrainPullInfo {
            direction: self.direction,
            speed: self.speed,
            track_distance: self.track_distance,
            tow_hitch_position: self.tow_hitch_position.to_array(),
            most_recent_special_point_handle: self.most_recent_special_point_handle,
            previous_waypoint: self.previous_waypoint,
            current_waypoint: self.current_waypoint,
        }
    }

    fn copy_from_train_pull_info(&mut self, info: TrainPullInfo) {
        self.direction = info.direction;
        self.speed = info.speed;
        self.track_distance = info.track_distance;
        self.tow_hitch_position = Coord3D::from_array(info.tow_hitch_position);
        self.most_recent_special_point_handle = info.most_recent_special_point_handle;
        self.previous_waypoint = info.previous_waypoint;
        self.current_waypoint = info.current_waypoint;
    }

    fn xfer_pull_info(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |result: std::io::Result<()>| result.map_err(|e| e.to_string());
        let mut version: u32 = 1;
        xfer_io(xfer.xfer_u32(&mut version))?;
        xfer_io(xfer.xfer_real(&mut self.direction))?;
        xfer_io(xfer.xfer_real(&mut self.speed))?;
        xfer_io(xfer.xfer_real(&mut self.track_distance))?;
        xfer.xfer_coord3d(&mut self.tow_hitch_position);
        let mut handle = self.most_recent_special_point_handle as u32;
        xfer_io(xfer.xfer_u32(&mut handle))?;
        self.most_recent_special_point_handle = handle as WaypointID;
        let mut prev = self.previous_waypoint as u32;
        xfer_io(xfer.xfer_u32(&mut prev))?;
        self.previous_waypoint = prev as WaypointID;
        let mut current = self.current_waypoint as u32;
        xfer_io(xfer.xfer_u32(&mut current))?;
        self.current_waypoint = current as WaypointID;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RailroadBehaviorModuleData {
    pub base: PhysicsBehaviorModuleData,
    pub carriage_template_names: Vec<AsciiString>,
    pub path_prefix_name: AsciiString,
    pub crash_fx_template_name: AsciiString,
    pub is_locomotive: Bool,
    pub running_garrison_speed_max: Real,
    pub kill_speed_min: Real,
    pub speed_max: Real,
    pub acceleration: Real,
    pub braking: Real,
    pub friction: Real,
    pub wait_at_station_time: UnsignedInt,
    pub running_sound: AudioEventRts,
    pub clickety_clack_sound: AudioEventRts,
    pub whistle_sound: AudioEventRts,
    pub meaty_impact_default_sound: AudioEventRts,
    pub big_metal_impact_default_sound: AudioEventRts,
    pub small_metal_impact_default_sound: AudioEventRts,
}

impl Default for RailroadBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: PhysicsBehaviorModuleData::default(),
            carriage_template_names: Vec::new(),
            path_prefix_name: AsciiString::new(),
            crash_fx_template_name: AsciiString::new(),
            is_locomotive: false,
            running_garrison_speed_max: 1.0,
            kill_speed_min: 1.0,
            speed_max: 4.0,
            acceleration: 1.01,
            braking: 0.99,
            friction: 0.97,
            wait_at_station_time: 150,
            running_sound: AudioEventRts::default(),
            clickety_clack_sound: AudioEventRts::default(),
            whistle_sound: AudioEventRts::default(),
            meaty_impact_default_sound: AudioEventRts::default(),
            big_metal_impact_default_sound: AudioEventRts::default(),
            small_metal_impact_default_sound: AudioEventRts::default(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(RailroadBehaviorModuleData, base);

fn parse_audio_event(
    setter: &mut dyn FnMut(AudioEventRts),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(AudioEventRts::new(token));
    Ok(())
}

fn parse_real_field(setter: &mut dyn FnMut(Real), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(Bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_duration_unsigned_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_carriage_list(
    _ini: &mut INI,
    data: &mut RailroadBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens)?;
    data.carriage_template_names
        .extend(values.iter().map(|t| AsciiString::from(*t)));
    Ok(())
}

fn parse_ascii_field(setter: &mut dyn FnMut(AsciiString), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(AsciiString::from(token));
    Ok(())
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn value_tokens<'a>(tokens: &'a [&'a str]) -> Result<Vec<&'a str>, INIError> {
    let values: Vec<_> = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();
    if values.is_empty() {
        return Err(INIError::InvalidData);
    }
    Ok(values)
}

const RAILROAD_BEHAVIOR_FIELDS: &[FieldParse<RailroadBehaviorModuleData>] = &[
    FieldParse {
        token: "CarriageTemplateName",
        parse: parse_carriage_list,
    },
    FieldParse {
        token: "PathPrefixName",
        parse: |_, data, tokens| parse_ascii_field(&mut |v| data.path_prefix_name = v, tokens),
    },
    FieldParse {
        token: "CrashFXTemplateName",
        parse: |_, data, tokens| {
            parse_ascii_field(&mut |v| data.crash_fx_template_name = v, tokens)
        },
    },
    FieldParse {
        token: "IsLocomotive",
        parse: |_, data, tokens| parse_bool_field(&mut |v| data.is_locomotive = v, tokens),
    },
    FieldParse {
        token: "RunningGarrisonSpeedMax",
        parse: |_, data, tokens| {
            parse_real_field(&mut |v| data.running_garrison_speed_max = v, tokens)
        },
    },
    FieldParse {
        token: "KillSpeedMin",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.kill_speed_min = v, tokens),
    },
    FieldParse {
        token: "SpeedMax",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.speed_max = v, tokens),
    },
    FieldParse {
        token: "Acceleration",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.acceleration = v, tokens),
    },
    FieldParse {
        token: "Braking",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.braking = v, tokens),
    },
    FieldParse {
        token: "Friction",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.friction = v, tokens),
    },
    FieldParse {
        token: "WaitAtStationTime",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(&mut |v| data.wait_at_station_time = v, tokens)
        },
    },
    FieldParse {
        token: "RunningSound",
        parse: |_, data, tokens| parse_audio_event(&mut |v| data.running_sound = v, tokens),
    },
    FieldParse {
        token: "ClicketyClackSound",
        parse: |_, data, tokens| parse_audio_event(&mut |v| data.clickety_clack_sound = v, tokens),
    },
    FieldParse {
        token: "WhistleSound",
        parse: |_, data, tokens| parse_audio_event(&mut |v| data.whistle_sound = v, tokens),
    },
    FieldParse {
        token: "MeatyBounceSound",
        parse: |_, data, tokens| {
            parse_audio_event(&mut |v| data.meaty_impact_default_sound = v, tokens)
        },
    },
    FieldParse {
        token: "BigMetalBounceSound",
        parse: |_, data, tokens| {
            parse_audio_event(&mut |v| data.big_metal_impact_default_sound = v, tokens)
        },
    },
    FieldParse {
        token: "SmallMetalBounceSound",
        parse: |_, data, tokens| {
            parse_audio_event(&mut |v| data.small_metal_impact_default_sound = v, tokens)
        },
    },
];

impl RailroadBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields(self, RAILROAD_BEHAVIOR_FIELDS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_field(data: &mut RailroadBehaviorModuleData, token: &str, values: &[&str]) {
        let field = RAILROAD_BEHAVIOR_FIELDS
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, data, values).expect("field parses");
    }

    #[test]
    fn railroad_behavior_fields_accept_ini_equals_token() {
        let mut data = RailroadBehaviorModuleData::default();

        parse_field(
            &mut data,
            "CarriageTemplateName",
            &["=", "TrainCarA", "TrainCarB"],
        );
        parse_field(&mut data, "PathPrefixName", &["=", "TrainPath"]);
        parse_field(&mut data, "CrashFXTemplateName", &["=", "TrainCrashFX"]);
        parse_field(&mut data, "IsLocomotive", &["=", "Yes"]);
        parse_field(&mut data, "RunningGarrisonSpeedMax", &["=", "2.5"]);
        parse_field(&mut data, "KillSpeedMin", &["=", "3.25"]);
        parse_field(&mut data, "SpeedMax", &["=", "4.75"]);
        parse_field(&mut data, "Acceleration", &["=", "1.05"]);
        parse_field(&mut data, "Braking", &["=", "0.92"]);
        parse_field(&mut data, "Friction", &["=", "0.88"]);
        parse_field(&mut data, "WaitAtStationTime", &["=", "3000"]);
        parse_field(&mut data, "RunningSound", &["=", "TrainRunning"]);
        parse_field(&mut data, "ClicketyClackSound", &["=", "TrainClickety"]);
        parse_field(&mut data, "WhistleSound", &["=", "TrainWhistle"]);
        parse_field(&mut data, "MeatyBounceSound", &["=", "TrainMeatyHit"]);
        parse_field(&mut data, "BigMetalBounceSound", &["=", "TrainBigMetalHit"]);
        parse_field(
            &mut data,
            "SmallMetalBounceSound",
            &["=", "TrainSmallMetalHit"],
        );

        assert_eq!(data.carriage_template_names.len(), 2);
        assert_eq!(data.carriage_template_names[0].as_str(), "TrainCarA");
        assert_eq!(data.carriage_template_names[1].as_str(), "TrainCarB");
        assert_eq!(data.path_prefix_name.as_str(), "TrainPath");
        assert_eq!(data.crash_fx_template_name.as_str(), "TrainCrashFX");
        assert!(data.is_locomotive);
        assert_eq!(data.running_garrison_speed_max, 2.5);
        assert_eq!(data.kill_speed_min, 3.25);
        assert_eq!(data.speed_max, 4.75);
        assert_eq!(data.acceleration, 1.05);
        assert_eq!(data.braking, 0.92);
        assert_eq!(data.friction, 0.88);
        assert_eq!(data.wait_at_station_time, 90);
        assert_eq!(data.running_sound.get_event_name(), "TrainRunning");
        assert_eq!(data.clickety_clack_sound.get_event_name(), "TrainClickety");
        assert_eq!(data.whistle_sound.get_event_name(), "TrainWhistle");
        assert_eq!(
            data.meaty_impact_default_sound.get_event_name(),
            "TrainMeatyHit"
        );
        assert_eq!(
            data.big_metal_impact_default_sound.get_event_name(),
            "TrainBigMetalHit"
        );
        assert_eq!(
            data.small_metal_impact_default_sound.get_event_name(),
            "TrainSmallMetalHit"
        );
    }
}

#[derive(Debug)]
pub struct RailroadBehavior {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<RailroadBehaviorModuleData>,
    next_station_task: StationTask,
    trailer_id: ObjectID,
    current_point_handle: WaypointID,
    wait_at_station_timer: Int,
    anchor_waypoint_id: WaypointID,
    carriages_created: Bool,
    has_ever_been_hitched: Bool,
    track_data_loaded: Bool,
    waiting_in_wings: Bool,
    end_of_line: Bool,
    is_locomotive: Bool,
    is_lead_carriage: Bool,
    wants_to_be_lead_carriage: Int,
    disembark: Bool,
    in_tunnel: Bool,
    held: Bool,
    conductor_state: ConductorState,
    pull_info: PullInfo,
    conductor_pull_info: PullInfo,
    running_sound: AudioEventRts,
    clickety_clack_sound: AudioEventRts,
    whistle_sound: AudioEventRts,
    physics_handle: Arc<Mutex<RailroadPhysicsHandle>>,
    track: Option<Arc<Mutex<TrainTrack>>>,
}

impl RailroadBehavior {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<RailroadBehaviorModuleData>()
            .ok_or("Invalid module data")?;

        let mut running_sound = specific_data.running_sound.clone();
        let mut clickety_clack_sound = specific_data.clickety_clack_sound.clone();
        let mut whistle_sound = specific_data.whistle_sound.clone();

        if let Ok(obj_guard) = object.read() {
            let obj_id = obj_guard.get_id();
            running_sound.set_object_id(obj_id);
            clickety_clack_sound.set_object_id(obj_id);
            whistle_sound.set_object_id(obj_id);
        }

        let is_locomotive = specific_data.is_locomotive;
        let physics_handle = Arc::new(Mutex::new(RailroadPhysicsHandle::new(
            specific_data.base.mass,
        )));

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_station_task: StationTask::DoNothing,
            trailer_id: INVALID_ID,
            current_point_handle: FACADE_WAYPOINT_ID,
            wait_at_station_timer: 0,
            anchor_waypoint_id: INVALID_WAYPOINT_ID,
            carriages_created: false,
            has_ever_been_hitched: false,
            track_data_loaded: false,
            waiting_in_wings: true,
            end_of_line: false,
            is_locomotive,
            is_lead_carriage: is_locomotive,
            wants_to_be_lead_carriage: 0,
            disembark: false,
            in_tunnel: false,
            held: false,
            conductor_state: if is_locomotive {
                ConductorState::Accelerate
            } else {
                ConductorState::Coast
            },
            pull_info: PullInfo::default(),
            conductor_pull_info: PullInfo::default(),
            running_sound,
            clickety_clack_sound,
            whistle_sound,
            physics_handle,
            track: None,
        })
    }

    fn get_object(&self) -> Option<Arc<RwLock<GameObject>>> {
        self.object.upgrade()
    }

    fn is_railroad(&self) -> Bool {
        let Some(track) = &self.track else {
            return false;
        };
        let Ok(track_guard) = track.lock() else {
            return false;
        };
        if track_guard.point_list.is_empty() {
            return false;
        }
        if self.waiting_in_wings || self.end_of_line {
            return false;
        }
        if self.is_lead_carriage {
            return true;
        }
        if self.trailer_id == INVALID_ID {
            return true;
        }
        false
    }

    fn play_impact_sound(&self, victim: &GameObject, impact_position: &Coord3D) {
        let mut impact = AudioEventRts::default();
        let mut has_bounce_sound = false;

        if let Some(physics) = victim.get_physics() {
            if let Ok(phys_guard) = physics.lock() {
                if let Some(sound) = phys_guard.get_bounce_sound() {
                    impact = sound.clone();
                    has_bounce_sound = true;
                }
            }
        }

        if !has_bounce_sound {
            if victim.is_kind_of(crate::common::KindOf::Infantry) {
                impact = self.module_data.meaty_impact_default_sound.clone();
            } else if victim.is_kind_of(crate::common::KindOf::Vehicle)
                || victim.is_kind_of(crate::common::KindOf::Structure)
            {
                impact = self.module_data.big_metal_impact_default_sound.clone();
            } else if victim.is_kind_of(crate::common::KindOf::Vehicle) {
                impact = self.module_data.small_metal_impact_default_sound.clone();
            }
        }

        if impact.get_event_name().is_empty() {
            return;
        }

        let mut vel = NORMAL_VEL_Z;
        let mut mass = NORMAL_MASS;

        impact.set_position(&(impact_position.x, impact_position.y, impact_position.z));

        if let Some(physics) = victim.get_physics() {
            if let Ok(phys_guard) = physics.lock() {
                vel += phys_guard.get_velocity().length();
                mass += phys_guard.get_mass();
                vel *= 0.5;
                mass *= 0.5;
                let pos = victim.get_position();
                impact.set_position(&(pos.x, pos.y, pos.z));
            }
        }

        vel = vel.clamp(0.0, NORMAL_VEL_Z);
        mass = mass.clamp(0.0, NORMAL_MASS);

        let mut volume = normalize_to_range(mu_law(vel, NORMAL_VEL_Z, 500.0), -1.0, 1.0, 0.25, 1.0);
        volume *= normalize_to_range(mu_law(mass, NORMAL_MASS, 500.0), -1.0, 1.0, 0.25, 1.0);
        impact.set_volume(volume);

        if let Some(player) = victim.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                impact.set_player_index(player_guard.get_player_index() as u32);
            }
        }

        if let Some(audio) = TheAudio::get() {
            let _ = audio.add_audio_event(&impact);
        }
    }

    fn load_track_data(&mut self) {
        if self.track.is_some() {
            return;
        }

        let Some(obj_arc) = self.get_object() else {
            return;
        };
        let Ok(obj_guard) = obj_arc.write() else {
            return;
        };
        let my_pos = *obj_guard.get_position();

        let terrain = crate::terrain::get_terrain_logic();
        let Ok(terrain_guard) = terrain.read() else {
            return;
        };

        let mut anchor_waypoint_id = None;
        if self.anchor_waypoint_id == INVALID_WAYPOINT_ID {
            let mut best_distance = Real::MAX;
            let mut scanner = terrain_guard.get_first_waypoint();
            while let Some(waypoint) = scanner {
                let delta = my_pos - *waypoint.get_location();
                let dist = delta.length();
                if dist < best_distance {
                    best_distance = dist;
                    anchor_waypoint_id = Some(waypoint.get_id());
                    self.anchor_waypoint_id = waypoint.get_id();
                }
                scanner = waypoint.get_next();
            }
        } else {
            anchor_waypoint_id = Some(self.anchor_waypoint_id);
        }

        let Some(anchor_id) = anchor_waypoint_id else {
            return;
        };

        let Some(anchor_waypoint) = terrain_guard.get_waypoint_by_id(anchor_id) else {
            return;
        };

        let mut track = TrainTrack::new();
        let track_list = track.get_writable_point_list();
        let Some(track_list) = track_list else {
            return;
        };
        let mut track_length = 0.0;

        let mut scanner = Some(anchor_waypoint);
        let mut dist_from_to;

        if let Some(scanner_wp) = scanner {
            let mut track_point = TrackPoint::new();
            track_point.distance_from_prev = 0.0;
            track_point.distance_from_first = 0.0;
            track_point.is_first_point = true;
            track_point.is_last_point = false;
            track_point.is_tunnel_or_bridge = scanner_wp.get_name().as_str().ends_with("Tunnel");
            track_point.is_station = scanner_wp.get_name().as_str().ends_with("Station");
            track_point.is_disembark = scanner_wp.get_name().as_str().ends_with("Disembark");
            track_point.is_ping_pong = false;
            track_point.position = *scanner_wp.get_location();
            track_point.handle = scanner_wp.get_id();
            track_list.push(track_point);
        }

        while let Some(scanner_wp) = scanner {
            let mut track_point = TrackPoint::new();
            if scanner_wp.get_num_links() > 0 {
                let next_id = scanner_wp.get_link(0);
                let next_wp = next_id.and_then(|id| terrain_guard.get_waypoint_by_id(id));
                if let Some(next_wp) = next_wp {
                    let from_pos = *scanner_wp.get_location();
                    let to_pos = *next_wp.get_location();
                    dist_from_to = (from_pos - to_pos).length();
                    track_length += dist_from_to;

                    track_point.distance_from_prev = dist_from_to;
                    track_point.distance_from_first = track_length;
                    track_point.is_first_point = false;
                    track_point.is_last_point = next_wp.get_link(0).is_none();
                    track_point.is_tunnel_or_bridge =
                        next_wp.get_name().as_str().ends_with("Tunnel");
                    track_point.is_station = next_wp.get_name().as_str().ends_with("Station");
                    track_point.is_ping_pong = scanner_wp.get_name().as_str().ends_with("PingPong");
                    track_point.is_disembark =
                        scanner_wp.get_name().as_str().ends_with("Disembark");
                    track_point.position = *next_wp.get_location();
                    track_point.handle = scanner_wp.get_id();
                    track_list.push(track_point);

                    scanner = Some(next_wp);
                } else {
                    scanner = None;
                }
            } else {
                break;
            }

            if let Some(scanner_wp) = scanner {
                if scanner_wp.get_id() == anchor_waypoint.get_id() {
                    track.is_looping = true;
                    break;
                }
            }
        }

        track.length = track_length;

        self.track = Some(Arc::new(Mutex::new(track)));
    }

    fn make_a_wall_out_of_this_train(&mut self, on: Bool) {
        if let Ok(ai) = THE_AI.write() {
            if let Some(object) = self.get_object() {
                if let Ok(obj_guard) = object.read() {
                    if let Some(pathfinder) = ai.pathfinder() {
                        if let Ok(mut pf) = pathfinder.write() {
                            if on {
                                pf.create_wall_from_object(&obj_guard);
                            } else {
                                pf.remove_wall_from_object(&obj_guard);
                            }
                        }
                    }
                }
            }
        }

        if self.trailer_id != INVALID_ID {
            if let Some(trailer) = TheGameLogic::find_object_by_id(self.trailer_id) {
                if let Ok(trailer_guard) = trailer.read() {
                    if let Some(module) = trailer_guard.find_update_module("RailroadBehavior") {
                        module.with_module(|module| {
                            if let Some(train) = module.get_train_control_interface() {
                                train.set_train_wall(on);
                            }
                        });
                    }
                }
            }
        }
    }

    fn disembark(&mut self) {
        if let Some(obj_arc) = self.get_object() {
            if let Ok(obj_guard) = obj_arc.write() {
                if let Some(contain) = obj_guard.get_contain() {
                    let _ = contain.order_all_passengers_to_exit(
                        crate::common::CommandSourceType::FromAi,
                        false,
                    );
                }
            }
        }

        if self.trailer_id != INVALID_ID {
            if let Some(trailer) = TheGameLogic::find_object_by_id(self.trailer_id) {
                if let Ok(trailer_guard) = trailer.read() {
                    if let Some(module) = trailer_guard.find_update_module("RailroadBehavior") {
                        module.with_module(|module| {
                            if let Some(train) = module.get_train_control_interface() {
                                train.disembark_passengers();
                            }
                        });
                    }
                }
            }
        }
    }

    fn create_carriages(&mut self) {
        if !self.is_locomotive {
            return;
        }

        let Some(obj_arc) = self.get_object() else {
            return;
        };
        let Ok(obj_guard) = obj_arc.write() else {
            return;
        };

        let max_radius = obj_guard.get_geometry_info().get_major_radius() * 2.0;
        let mut my_hitch_loc = *obj_guard.get_position();
        let (dir_x, dir_y) = obj_guard.get_unit_direction_vector_2d();
        let mut hitch_offset = Coord3D::new(dir_x, dir_y, 0.0);
        hitch_offset *= -max_radius;
        my_hitch_loc += hitch_offset;

        let close_carriage = if self.trailer_id != INVALID_ID {
            if let Some(obj) = TheGameLogic::find_object_by_id(self.trailer_id) {
                if let Ok(guard) = obj.read() {
                    Some(guard.get_id())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            ThePartitionManager::get().and_then(|pm| {
                pm.get_closest_object(&my_hitch_loc, max_radius, |candidate| {
                    if candidate.get_id() == obj_guard.get_id() {
                        return false;
                    }
                    let Some(module) = candidate.find_update_module("RailroadBehavior") else {
                        return false;
                    };
                    module.with_module(|module| {
                        module.get_train_control_interface().is_some_and(|train| {
                            !train.has_ever_been_hitched()
                                && candidate.get_relationship_to(&*obj_guard)
                                    == ObjectRelationship::Ally
                        })
                    })
                })
            })
        };

        let mut template_iter = self.module_data.carriage_template_names.iter();
        let Some(first_template_name) = template_iter.next() else {
            self.carriages_created = true;
            return;
        };

        let first_carriage = if let Some(close_id) = close_carriage {
            TheGameLogic::find_object_by_id(close_id)
        } else {
            TheThingFactory::find_template(first_template_name.as_str()).and_then(|template| {
                obj_guard.get_team().and_then(|team| {
                    team.read().ok().and_then(|team_guard| {
                        TheThingFactory::get()
                            .ok()
                            .and_then(|factory| factory.new_object(template, &*team_guard).ok())
                    })
                })
            })
        };

        if let Some(first_carriage) = first_carriage {
            if let Ok(mut carriage_guard) = first_carriage.write() {
                carriage_guard.set_producer(Some(&*obj_guard));
                self.trailer_id = carriage_guard.get_id();
            }

            if let Ok(carriage_guard) = first_carriage.read() {
                if let Some(module) = carriage_guard.find_update_module("RailroadBehavior") {
                    let _ = module.with_module_downcast::<
                        crate::object::update::ai_update::railroad_guide_ai_update::RailroadBehaviorModule,
                        _,
                        _,
                    >(|module| {
                        if close_carriage.is_some() {
                            module.behavior_mut().hitch_new_carriage_by_proximity(
                                obj_guard.get_id(),
                                self.track.clone(),
                            );
                        } else {
                            module.behavior_mut().hitch_new_carriage_by_template(
                                obj_guard.get_id(),
                                template_iter.map(|s| s.clone()).collect(),
                                self.track.clone(),
                            );
                        }
                    });
                }
            }
        }

        self.carriages_created = true;
    }

    fn hitch_new_carriage_by_template(
        &mut self,
        loco_id: ObjectID,
        list: Vec<AsciiString>,
        track: Option<Arc<Mutex<TrainTrack>>>,
    ) {
        if self.is_locomotive {
            return;
        }

        let Some(locomotive) = TheGameLogic::find_object_by_id(loco_id) else {
            return;
        };
        let Ok(locomotive_guard) = locomotive.read() else {
            return;
        };

        self.track = track.clone();
        if let Some(track) = &self.track {
            if let Ok(mut guard) = track.lock() {
                guard.inc_reference();
            }
        }
        self.has_ever_been_hitched = true;

        let mut iter = list.into_iter();
        let Some(next_name) = iter.next() else {
            return;
        };

        let Some(template) = TheThingFactory::find_template(next_name.as_str()) else {
            return;
        };
        let Some(team) = locomotive_guard.get_team() else {
            return;
        };
        let Some(new_carriage) = team.read().ok().and_then(|team_guard| {
            TheThingFactory::get()
                .ok()
                .and_then(|factory| factory.new_object(template, &*team_guard).ok())
        }) else {
            return;
        };

        if let Ok(mut guard) = new_carriage.write() {
            guard.set_producer(Some(&*locomotive_guard));
            self.trailer_id = guard.get_id();
        }

        let remaining_templates: Vec<AsciiString> = iter.collect();
        let next_track = track.clone();
        let module = {
            let Ok(carriage_guard) = new_carriage.read() else {
                return;
            };
            carriage_guard.find_update_module("RailroadBehavior")
        };
        if let Some(module) = module {
            let _ = module.with_module_downcast::<
                crate::object::update::ai_update::railroad_guide_ai_update::RailroadBehaviorModule,
                _,
                _,
            >(|module| {
                module
                    .behavior_mut()
                    .hitch_new_carriage_by_template(loco_id, remaining_templates, next_track);
            });
        }
    }

    fn hitch_new_carriage_by_proximity(
        &mut self,
        loco_id: ObjectID,
        track: Option<Arc<Mutex<TrainTrack>>>,
    ) {
        if self.is_locomotive {
            return;
        }

        let Some(locomotive) = TheGameLogic::find_object_by_id(loco_id) else {
            return;
        };
        let Ok(_locomotive_guard) = locomotive.read() else {
            return;
        };

        self.track = track.clone();
        if let Some(track) = &self.track {
            if let Ok(mut guard) = track.lock() {
                guard.inc_reference();
            }
        }
        self.has_ever_been_hitched = true;

        let Some(obj_arc) = self.get_object() else {
            return;
        };
        let Ok(obj_guard) = obj_arc.write() else {
            return;
        };

        let max_radius = obj_guard.get_geometry_info().get_major_radius() * 2.0;
        let mut my_hitch_loc = *obj_guard.get_position();
        let (dir_x, dir_y) = obj_guard.get_unit_direction_vector_2d();
        let mut hitch_offset = Coord3D::new(dir_x, dir_y, 0.0);
        hitch_offset *= -max_radius;
        my_hitch_loc += hitch_offset;

        let close_carriage = if self.trailer_id != INVALID_ID {
            if let Some(obj) = TheGameLogic::find_object_by_id(self.trailer_id) {
                if let Ok(guard) = obj.read() {
                    Some(guard.get_id())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            ThePartitionManager::get().and_then(|pm| {
                pm.get_closest_object(&my_hitch_loc, max_radius, |candidate| {
                    if candidate.get_id() == obj_guard.get_id() {
                        return false;
                    }
                    if let Some(module) = candidate.find_update_module("RailroadBehavior") {
                        return module.with_module(|module| {
                            module.get_train_control_interface().is_some_and(|train| {
                                !train.has_ever_been_hitched()
                                    && candidate.get_relationship_to(&*obj_guard)
                                        == ObjectRelationship::Ally
                            })
                        });
                    }
                    false
                })
            })
        };

        if let Some(close_id) = close_carriage {
            if let Some(close) = TheGameLogic::find_object_by_id(close_id) {
                if let Ok(mut close_guard) = close.write() {
                    close_guard.set_producer(Some(&*obj_guard));
                    self.trailer_id = close_guard.get_id();
                }
                if let Ok(close_guard) = close.read() {
                    if let Some(module) = close_guard.find_update_module("RailroadBehavior") {
                        let _ = module.with_module_downcast::<
                            crate::object::update::ai_update::railroad_guide_ai_update::RailroadBehaviorModule,
                            _,
                            _,
                        >(|module| {
                            module.behavior_mut().hitch_new_carriage_by_proximity(
                                obj_guard.get_id(),
                                track,
                            );
                        });
                    }
                }
            }
        }
    }

    fn get_pulled(&mut self, info: &mut PullInfo) {
        self.wants_to_be_lead_carriage = 0;

        if self.track.is_none() {
            return;
        }

        self.conductor_pull_info = info.clone();
        info.previous_waypoint = info.current_waypoint;
        let mut local_pull_info = self.pull_info.clone();
        self.update_position_track_distance(info, &mut local_pull_info);
        self.pull_info = local_pull_info;

        if self.trailer_id != INVALID_ID {
            if let Some(trailer) = TheGameLogic::find_object_by_id(self.trailer_id) {
                if let Ok(trailer_guard) = trailer.read() {
                    if let Some(module) = trailer_guard.find_update_module("RailroadBehavior") {
                        module.with_module(|module| {
                            if let Some(train) = module.get_train_control_interface() {
                                let mut pull_info = self.pull_info.to_train_pull_info();
                                train.get_pulled(&mut pull_info);
                                self.pull_info.copy_from_train_pull_info(pull_info);
                            }
                        });
                    }
                }
            }
        } else {
            self.trailer_id = INVALID_ID;
            if self.end_of_line {
                if let Some(obj_arc) = self.get_object() {
                    if let Ok(obj_guard) = obj_arc.write() {
                        let _ = TheGameLogic::destroy_object(&obj_guard);
                    }
                }
            }
        }
    }

    fn update_position_track_distance(&mut self, puller_info: &PullInfo, my_info: &mut PullInfo) {
        let Some(track) = &self.track else {
            return;
        };
        let Some(obj_arc) = self.get_object() else {
            return;
        };
        let Ok(mut obj_guard) = obj_arc.write() else {
            return;
        };

        let hitch_radius = obj_guard.get_geometry_info().get_major_radius();
        my_info.track_distance = puller_info.track_distance - (hitch_radius * 2.0);
        my_info.speed = puller_info.speed;
        my_info.direction = puller_info.direction;

        let track_length = track.lock().map(|t| t.length).unwrap_or(0.0);
        self.find_pos_by_path_distance(
            &mut my_info.tow_hitch_position,
            my_info.track_distance,
            track_length,
            false,
        );

        let mut car_position = Coord3D::ZERO;
        self.find_pos_by_path_distance(
            &mut car_position,
            my_info.track_distance,
            track_length,
            true,
        );

        let mut turn_pos = *obj_guard.get_position();
        if !self.in_tunnel {
            if let Some(terrain) = TheTerrainLogic::get() {
                turn_pos.z = terrain.get_ground_height(turn_pos.x, turn_pos.y, None);
            }
        }

        let (dir_x, dir_y) = obj_guard.get_unit_direction_vector_2d();
        turn_pos.x += dir_x * -hitch_radius;
        turn_pos.y += dir_y * -hitch_radius;

        let track_pos_delta = Coord3D::new(
            car_position.x - turn_pos.x,
            car_position.y - turn_pos.y,
            0.0,
        );

        let dx = puller_info.tow_hitch_position.x - turn_pos.x;
        let dy = puller_info.tow_hitch_position.y - turn_pos.y;
        let desired_angle = dy.atan2(dx);

        let rel_angle = std_angle_diff(desired_angle, obj_guard.get_orientation());

        let mut tmp = Mat4::from_translation(Vec3::new(turn_pos.x, turn_pos.y, 0.0));
        tmp *= Mat4::from_translation(Vec3::new(track_pos_delta.x, track_pos_delta.y, 0.0));
        tmp *= Mat4::from_rotation_z(rel_angle);
        tmp *= Mat4::from_translation(Vec3::new(-turn_pos.x, -turn_pos.y, 0.0));

        let mtx = tmp * obj_guard.get_transform_matrix();
        obj_guard.set_transform_matrix(&mtx);

        if !self.in_tunnel {
            if let Some(terrain) = TheTerrainLogic::get() {
                let z = terrain.get_ground_height(turn_pos.x, turn_pos.y, None);
                let mut pos = *obj_guard.get_position();
                pos.z = z;
                let _ = obj_guard.set_position(&pos);
            }
        }

        if let Ok(mut phys_guard) = self.physics_handle.lock() {
            let (dir_x, dir_y) = obj_guard.get_unit_direction_vector_2d();
            let velocity = Coord3D::new(dir_x * my_info.speed, dir_y * my_info.speed, 0.0);
            phys_guard.set_velocity(&velocity);
        }

        obj_guard.handle_partition_cell_maintenance();
    }

    #[allow(dead_code)]
    fn destroy_whole_train_now(&mut self) {
        if let Some(obj_arc) = self.get_object() {
            if let Ok(obj_guard) = obj_arc.write() {
                let _ = TheGameLogic::destroy_object(&obj_guard);
            }
        }

        if self.trailer_id != INVALID_ID {
            if let Some(trailer) = TheGameLogic::find_object_by_id(self.trailer_id) {
                if let Ok(trailer_guard) = trailer.read() {
                    if let Some(module) = trailer_guard.find_update_module("RailroadBehavior") {
                        module.with_module(|module| {
                            if let Some(train) = module.get_train_control_interface() {
                                train.destroy_whole_train_now();
                            }
                        });
                    }
                }
            }
        }
    }

    pub fn set_held(&mut self, held: Bool) {
        self.held = held;
    }

    fn find_pos_by_path_distance(
        &mut self,
        pos: &mut Coord3D,
        dist: Real,
        length: Real,
        set_state: Bool,
    ) {
        let Some(track) = &self.track else {
            return;
        };

        self.waiting_in_wings = false;

        let mut actual_distance = dist;
        if let Ok(track_guard) = track.lock() {
            if track_guard.is_looping {
                while actual_distance < 0.0 {
                    actual_distance += length;
                }
                while actual_distance > length {
                    actual_distance -= length;
                }
            } else {
                if dist < 0.0 {
                    self.waiting_in_wings = true;
                } else if dist >= length {
                    self.end_of_line = true;
                }
                actual_distance = dist.clamp(0.0, length);
            }

            *pos = Coord3D::ZERO;
            let Some(point_list) = track_guard.get_point_list() else {
                return;
            };
            let mut iter = point_list.iter();
            let mut this_point = iter.next();
            while let Some(tp) = this_point {
                let next_point = iter.next();
                if tp.distance_from_first < actual_distance {
                    let mut this_pos = tp.position;
                    if let Some(next) = next_point {
                        if next.distance_from_first > actual_distance {
                            let handle_found = tp.handle;
                            let edge = self.current_point_handle != handle_found;
                            if set_state {
                                self.in_tunnel = tp.is_tunnel_or_bridge;
                                if self.is_locomotive {
                                    if edge {
                                        self.current_point_handle = handle_found;
                                        if tp.is_station {
                                            self.conductor_state = ConductorState::ApplyBrakes;
                                            self.disembark = false;
                                            if let Some(audio) = TheAudio::get() {
                                                audio.remove_audio_event(
                                                    self.running_sound.get_playing_handle(),
                                                );
                                            }
                                        } else if tp.is_disembark {
                                            self.conductor_state = ConductorState::ApplyBrakes;
                                            self.disembark = true;
                                            if let Some(audio) = TheAudio::get() {
                                                audio.remove_audio_event(
                                                    self.running_sound.get_playing_handle(),
                                                );
                                            }
                                        } else if tp.is_ping_pong
                                            && self
                                                .conductor_pull_info
                                                .most_recent_special_point_handle
                                                != handle_found
                                        {
                                            self.conductor_pull_info
                                                .most_recent_special_point_handle = handle_found;
                                            self.conductor_state = ConductorState::ApplyBrakes;
                                            self.disembark = false;
                                            if let Some(audio) = TheAudio::get() {
                                                audio.remove_audio_event(
                                                    self.running_sound.get_playing_handle(),
                                                );
                                            }
                                            self.conductor_pull_info.direction =
                                                -self.conductor_pull_info.direction;
                                        }
                                    }
                                }

                                if edge && !self.in_tunnel {
                                    if let Some(audio) = TheAudio::get() {
                                        let _ = audio.add_audio_event(&self.clickety_clack_sound);
                                    }
                                    if let Some(obj_arc) = self.get_object() {
                                        if let Ok(obj_guard) = obj_arc.write() {
                                            let pos = obj_guard.get_position();
                                            self.clickety_clack_sound
                                                .set_position(&(pos.x, pos.y, pos.z));
                                            self.clickety_clack_sound
                                                .set_volume(self.conductor_pull_info.speed / 10.0);
                                        }
                                    }
                                }
                            }

                            let difference = actual_distance - tp.distance_from_first;
                            let mut delta = next.position - this_pos;
                            if delta.length() > 0.0 {
                                delta = delta.normalize();
                            }
                            delta *= difference;
                            this_pos += delta;
                            *pos = this_pos;
                            return;
                        } else {
                            *pos = this_pos;
                        }
                    } else {
                        *pos = this_pos;
                    }
                }
                this_point = next_point;
            }
        }
    }

    fn on_collide(&mut self, other: &mut GameObject, loc: &Coord3D, _normal: &Coord3D) {
        if self.waiting_in_wings || self.end_of_line {
            return;
        }

        let (my_id, my_dir, my_loc, us_radius) = if let Some(obj_arc) = self.get_object() {
            if let Ok(guard) = obj_arc.read() {
                let (x, y) = guard.get_unit_direction_vector_2d();
                (
                    guard.get_id(),
                    Coord3D::new(x, y, 0.0),
                    *guard.get_position(),
                    guard.get_geometry_info().get_major_radius(),
                )
            } else {
                (INVALID_ID, Coord3D::ZERO, Coord3D::ZERO, 0.0)
            }
        } else {
            (INVALID_ID, Coord3D::ZERO, Coord3D::ZERO, 0.0)
        };

        if other
            .get_behavior_modules()
            .iter()
            .filter_map(|m| m.lock().ok())
            .any(|mut module| module.get_collide().map_or(false, |c| c.is_railroad()))
        {
            if self.is_locomotive {
                other.kill(None, None);
            } else if self.is_lead_carriage {
                other.kill(None, None);
                if let Some(obj_arc) = self.get_object() {
                    if let Ok(mut guard) = obj_arc.write() {
                        guard.kill(None, None);
                    }
                }
            }
            return;
        }

        if other.is_kind_of(crate::common::KindOf::Structure) {
            let is_faction = other.is_kind_of(crate::common::KindOf::FSPower)
                || other.is_kind_of(crate::common::KindOf::Factory)
                || other.is_kind_of(crate::common::KindOf::Defense)
                || other.is_kind_of(crate::common::KindOf::FSTechnology)
                || other.is_kind_of(crate::common::KindOf::RebuildHole);
            if is_faction {
                self.play_impact_sound(other, other.get_position());
                other.kill(None, None);
                return;
            }

            if let Some(_module) = other.find_update_module("DemoTrapUpdate") {
                if !other.test_status(crate::common::ObjectStatusTypes::UnderConstruction) {
                    if let Some(obj_arc) = self.get_object() {
                        if let Ok(mut guard) = obj_arc.write() {
                            guard.kill(None, None);
                        }
                    }
                }
                self.play_impact_sound(other, other.get_position());
                other.kill(None, None);
                return;
            }
        }

        let Some(physics) = other.get_physics() else {
            return;
        };

        if self.conductor_state == ConductorState::WaitAtStation
            && self.pull_info.speed < self.module_data.running_garrison_speed_max
        {
            if let Some(ai) = other.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    if ai_guard.get_enter_target() == Some(my_id) {
                        return;
                    }
                }
            }
        }

        if other.get_contained_by() == Some(my_id) {
            return;
        }

        let victim_is_infantry = other.is_kind_of(crate::common::KindOf::Infantry);
        let their_loc = *other.get_position();

        let mut dlt = their_loc - my_loc;

        if let Some(audio) = TheAudio::get() {
            if !self.whistle_sound.is_currently_playing() {
                let handle = audio.add_audio_event(&self.whistle_sound);
                self.whistle_sound.set_playing_handle(handle);
            }
        }

        let dist = dlt.length();
        let them_radius = other.get_geometry_info().get_major_radius();
        let mut overlap = (us_radius + them_radius) - dist + 1.0;
        if dist > 0.0 {
            dlt = dlt.normalize();
        }

        if !victim_is_infantry {
            overlap /= 4.0;
            let new_pos = their_loc + dlt * overlap;
            let _ = other.set_position(&new_pos);
        }

        if self.conductor_state == ConductorState::WaitAtStation
            || (self.conductor_state == ConductorState::Coast
                && self.pull_info.speed < self.module_data.running_garrison_speed_max)
            || !self.is_locomotive
        {
            return;
        }

        let delta = (their_loc - my_loc).normalize();
        let dot = delta.x * my_dir.x + delta.y * my_dir.y + delta.z * my_dir.z;

        if other.is_effectively_dead() {
            let vel = delta * (self.pull_info.speed * 0.66).min(0.3);
            if let Ok(mut phys_guard) = physics.lock() {
                phys_guard.add_velocity_to(&vel);
            }
        } else {
            if self.pull_info.speed >= self.module_data.kill_speed_min {
                other.kill(None, None);
                if let Ok(mut phys_guard) = physics.lock() {
                    phys_guard.set_pitch_rate(crate::helpers::get_game_logic_random_value_real(
                        -0.03, 0.03,
                    ));
                    phys_guard.set_roll_rate(crate::helpers::get_game_logic_random_value_real(
                        -0.03, 0.03,
                    ));
                }
            } else {
                self.play_impact_sound(other, loc);
                let mut damage_info = DamageInfo::new();
                damage_info.input.damage_type = DamageType::Crush;
                damage_info.input.death_type = DeathType::Crushed;
                damage_info.input.source_id = my_id;
                damage_info.input.amount = self.pull_info.speed * 10.0;
                damage_info.sync_from_input();
                let _ = other.attempt_damage(&mut damage_info);
            }

            let mut heft = their_loc;
            if let Some(terrain) = TheTerrainLogic::get() {
                let ground = terrain.get_ground_height(heft.x, heft.y, None);
                heft.z = ground + 2.0;
            }
            let _ = other.set_position(&heft);

            let mut delta_vel = delta;
            delta_vel.z =
                crate::helpers::get_game_logic_random_value_real(0.05, self.pull_info.speed / 10.0);
            delta_vel *= dot;

            if !(victim_is_infantry
                && physics
                    .lock()
                    .ok()
                    .map(|p| p.get_velocity().length())
                    .unwrap_or(0.0)
                    > 5.0)
            {
                if let Ok(mut phys_guard) = physics.lock() {
                    phys_guard.add_velocity_to(&delta_vel);
                }
            }

            if let Ok(mut phys_guard) = physics.lock() {
                phys_guard.set_allow_to_fall(true);
                phys_guard.set_allow_bouncing(true);
                phys_guard.set_allow_airborne_friction(true);

                let cross = my_dir.cross(Coord3D::new(0.0, 0.0, 1.0));
                let delta_norm = delta.normalize();
                let deviation_cog =
                    cross.x * delta_norm.x + cross.y * delta_norm.y + cross.z * delta_norm.z;
                if dot > 0.0 {
                    phys_guard.set_yaw_rate(deviation_cog * -0.06 * self.pull_info.speed);
                }
            }
        }
    }
}

impl UpdateModuleInterface for RailroadBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        if !self.track_data_loaded && self.is_locomotive {
            self.load_track_data();
            if self.track.is_some() {
                self.create_carriages();
            }
            self.track_data_loaded = true;
        }

        if self.track.is_none() {
            return Ok(UPDATE_SLEEP_NONE);
        }

        if self.is_locomotive {
            match self.conductor_state {
                ConductorState::ApplyBrakes => {
                    self.conductor_pull_info.speed *= self.module_data.braking;
                    if self.conductor_pull_info.speed.abs() < 0.1 {
                        self.conductor_pull_info.speed = 0.0;
                        self.wait_at_station_timer = self.module_data.wait_at_station_time as Int;
                        self.conductor_state = ConductorState::WaitAtStation;
                        self.make_a_wall_out_of_this_train(true);
                        if self.disembark {
                            self.disembark();
                            self.disembark = false;
                        }
                    }
                }
                ConductorState::WaitAtStation => {
                    self.wait_at_station_timer -= 1;
                    if self.wait_at_station_timer <= 0 && !self.held {
                        self.conductor_state = ConductorState::Accelerate;
                        self.conductor_pull_info.speed = 0.05 * self.conductor_pull_info.direction;
                        if let Some(audio) = TheAudio::get() {
                            let handle = audio.add_audio_event(&self.running_sound);
                            self.running_sound.set_playing_handle(handle);
                        }
                        self.make_a_wall_out_of_this_train(false);
                    } else if self.wait_at_station_timer
                        == (self.module_data.wait_at_station_time / 4) as Int
                    {
                        if let Some(audio) = TheAudio::get() {
                            let handle = audio.add_audio_event(&self.whistle_sound);
                            self.whistle_sound.set_playing_handle(handle);
                        }
                    }
                }
                ConductorState::Accelerate => {
                    self.conductor_pull_info.speed += 0.02 * self.conductor_pull_info.direction;
                    self.conductor_pull_info.speed *= self.module_data.acceleration;
                    if self.conductor_pull_info.speed > self.module_data.speed_max {
                        self.conductor_pull_info.speed = self.module_data.speed_max;
                    } else if self.conductor_pull_info.speed < -self.module_data.speed_max {
                        self.conductor_pull_info.speed = -self.module_data.speed_max;
                    }

                    if let Some(audio) = TheAudio::get() {
                        if !self.running_sound.is_currently_playing() {
                            let handle = audio.add_audio_event(&self.running_sound);
                            self.running_sound.set_playing_handle(handle);
                        }
                    }
                }
                _ => {}
            }
        }

        if self.wants_to_be_lead_carriage > FRAMES_UNPULLED_LONG_ENOUGH_TO_UNHITCH {
            self.is_lead_carriage = true;
        }

        if self.is_lead_carriage {
            if self.conductor_state == ConductorState::Coast {
                self.conductor_pull_info.speed *= self.module_data.friction;
                if let Some(audio) = TheAudio::get() {
                    audio.remove_audio_event(self.running_sound.get_playing_handle());
                }
            }

            if let Some(track_arc) = self.track.clone() {
                if let Ok(track_guard) = track_arc.lock() {
                    self.conductor_pull_info.track_distance += self.conductor_pull_info.speed;
                    if track_guard.is_looping {
                        while self.conductor_pull_info.track_distance > track_guard.length {
                            self.conductor_pull_info.track_distance -= track_guard.length;
                        }
                        while self.conductor_pull_info.track_distance < 0.0 {
                            self.conductor_pull_info.track_distance += track_guard.length;
                        }
                    }

                    let mut tow_hitch = self.conductor_pull_info.tow_hitch_position;
                    self.find_pos_by_path_distance(
                        &mut tow_hitch,
                        self.conductor_pull_info.track_distance,
                        track_guard.length,
                        false,
                    );
                    self.conductor_pull_info.tow_hitch_position = tow_hitch;

                    let conductor_info = self.conductor_pull_info.clone();
                    let mut next_pull = self.pull_info.clone();
                    self.update_position_track_distance(&conductor_info, &mut next_pull);
                    self.pull_info = next_pull;
                }
            }

            if self.trailer_id != INVALID_ID {
                if let Some(trailer) = TheGameLogic::find_object_by_id(self.trailer_id) {
                    if let Ok(trailer_guard) = trailer.read() {
                        if let Some(module) = trailer_guard.find_update_module("RailroadBehavior") {
                            module.with_module(|module| {
                                if let Some(train) = module.get_train_control_interface() {
                                    let mut pull_info = self.pull_info.to_train_pull_info();
                                    train.get_pulled(&mut pull_info);
                                    self.pull_info.copy_from_train_pull_info(pull_info);
                                }
                            });
                        }
                    }
                }
            } else {
                self.trailer_id = INVALID_ID;
                if self.end_of_line {
                    if let Some(obj_arc) = self.get_object() {
                        if let Ok(obj_guard) = obj_arc.write() {
                            let _ = TheGameLogic::destroy_object(&obj_guard);
                        }
                    }
                }
            }
        } else if self.wants_to_be_lead_carriage <= FRAMES_UNPULLED_LONG_ENOUGH_TO_UNHITCH {
            self.wants_to_be_lead_carriage += 1;
        }

        if let Some(obj_arc) = self.get_object() {
            if let Ok(obj_guard) = obj_arc.write() {
                if let Some(drawable) = obj_guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        if let Some(track) = &self.track {
                            if let Ok(track_guard) = track.lock() {
                                if !track_guard.is_looping {
                                    let _ = draw_guard.set_drawable_hidden(
                                        self.waiting_in_wings || self.end_of_line,
                                    );
                                }
                            }
                        }

                        let draw_pos = draw_guard.get_position();
                        if let Some(terrain) = TheTerrainLogic::get() {
                            if draw_pos.z
                                < terrain.get_ground_height(draw_pos.x, draw_pos.y, None) - 3.0
                            {
                                draw_guard
                                    .set_model_condition_state(ModelConditionFlags::OVER_WATER);
                            } else {
                                draw_guard
                                    .clear_model_condition_state(ModelConditionFlags::OVER_WATER);
                            }
                        }
                    }
                }
            }
        }

        Ok(UPDATE_SLEEP_NONE)
    }
}

impl BehaviorModuleInterface for RailroadBehavior {
    fn get_module_name(&self) -> &str {
        "RailroadBehavior"
    }

    fn get_collide(&mut self) -> Option<&mut dyn CollideModuleInterface> {
        Some(self)
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(obj_arc) = self.get_object() {
            if let Ok(mut obj_guard) = obj_arc.write() {
                let physics: Arc<Mutex<dyn PhysicsBehavior>> = self.physics_handle.clone();
                obj_guard.set_physics(Some(physics));
            }
        }
        Ok(())
    }

    fn on_die(
        &mut self,
        _damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(self.running_sound.get_playing_handle());
        }
        self.track = None;
        Ok(())
    }
}

impl CollideModuleInterface for RailroadBehavior {
    fn on_collision(&mut self, _object_id: ObjectID, other_id: ObjectID) {
        let Some(other) = TheGameLogic::find_object_by_id(other_id) else {
            return;
        };
        let Ok(mut other_guard) = other.write() else {
            return;
        };
        let loc = *other_guard.get_position();
        let normal = Coord3D::new(0.0, 0.0, 1.0);
        self.on_collide(&mut other_guard, &loc, &normal);
    }

    fn is_railroad(&self) -> bool {
        self.is_railroad()
    }
}

impl Snapshotable for RailroadBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |result: std::io::Result<()>| result.map_err(|e| e.to_string());
        let mut version: u32 = 3;
        xfer_io(xfer.xfer_u32(&mut version))?;
        if version >= 2 {
            if let Ok(mut phys_guard) = self.physics_handle.lock() {
                phys_guard.xfer(xfer)?;
            }

            let mut next_station_task = self.next_station_task as i32;
            xfer_io(xfer.xfer_i32(&mut next_station_task))?;
            if xfer.is_loading() {
                self.next_station_task = match next_station_task {
                    1 => StationTask::Disembark,
                    _ => StationTask::DoNothing,
                };
            }

            xfer_io(xfer.xfer_object_id(&mut self.trailer_id))?;

            let mut handle = self.current_point_handle as u32;
            xfer_io(xfer.xfer_u32(&mut handle))?;
            self.current_point_handle = handle as WaypointID;

            xfer_io(xfer.xfer_i32(&mut self.wait_at_station_timer))?;
            xfer_io(xfer.xfer_bool(&mut self.carriages_created))?;
            xfer_io(xfer.xfer_bool(&mut self.has_ever_been_hitched))?;
            xfer_io(xfer.xfer_bool(&mut self.waiting_in_wings))?;
            xfer_io(xfer.xfer_bool(&mut self.end_of_line))?;
            xfer_io(xfer.xfer_bool(&mut self.is_locomotive))?;
            xfer_io(xfer.xfer_bool(&mut self.is_lead_carriage))?;
            xfer_io(xfer.xfer_i32(&mut self.wants_to_be_lead_carriage))?;
            xfer_io(xfer.xfer_bool(&mut self.disembark))?;
            xfer_io(xfer.xfer_bool(&mut self.in_tunnel))?;

            let mut conductor_state = self.conductor_state as i32;
            xfer_io(xfer.xfer_i32(&mut conductor_state))?;
            if xfer.is_loading() {
                self.conductor_state = match conductor_state {
                    0 => ConductorState::ApplyBrakes,
                    1 => ConductorState::WaitAtStation,
                    2 => ConductorState::Accelerate,
                    3 => ConductorState::Coast,
                    _ => ConductorState::WaitAtStation,
                };
            }

            let mut anchor = self.anchor_waypoint_id as u32;
            xfer_io(xfer.xfer_u32(&mut anchor))?;
            self.anchor_waypoint_id = anchor as WaypointID;

            self.pull_info.xfer_pull_info(xfer)?;
            self.conductor_pull_info.xfer_pull_info(xfer)?;
        }

        if version >= 3 {
            xfer_io(xfer.xfer_bool(&mut self.held))?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.track_data_loaded = false;
        self.running_sound = self.module_data.running_sound.clone();
        self.clickety_clack_sound = self.module_data.clickety_clack_sound.clone();
        self.whistle_sound = self.module_data.whistle_sound.clone();
        if let Ok(mut phys_guard) = self.physics_handle.lock() {
            phys_guard.set_mass(self.module_data.base.mass);
        }
        if let Some(obj_arc) = self.get_object() {
            if let Ok(obj_guard) = obj_arc.write() {
                let obj_id = obj_guard.get_id();
                self.running_sound.set_object_id(obj_id);
                self.clickety_clack_sound.set_object_id(obj_id);
                self.whistle_sound.set_object_id(obj_id);
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct RailroadBehaviorModule {
    module_name_key: NameKeyType,
    module_data: Arc<RailroadBehaviorModuleData>,
    behavior: RailroadBehavior,
}

impl RailroadBehaviorModule {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<RailroadBehaviorModuleData>,
        object: Arc<RwLock<GameObject>>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let behavior = RailroadBehavior::new(object, data.clone())?;
        Ok(Self {
            module_name_key,
            module_data: data,
            behavior,
        })
    }

    pub fn behavior(&self) -> &RailroadBehavior {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut RailroadBehavior {
        &mut self.behavior
    }
}

impl Module for RailroadBehaviorModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn get_train_control_interface(&mut self) -> Option<&mut dyn TrainControlInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) {
        let _ = BehaviorModuleInterface::on_object_created(&mut self.behavior);
    }
}

impl TrainControlInterface for RailroadBehaviorModule {
    fn has_ever_been_hitched(&self) -> bool {
        self.behavior.has_ever_been_hitched
    }

    fn get_pulled(&mut self, info: &mut TrainPullInfo) {
        let mut pull_info = PullInfo::default();
        pull_info.copy_from_train_pull_info(info.clone());
        self.behavior.get_pulled(&mut pull_info);
        *info = pull_info.to_train_pull_info();
    }

    fn set_held(&mut self, held: Bool) {
        self.behavior.set_held(held);
    }

    fn set_train_wall(&mut self, on: bool) {
        self.behavior.make_a_wall_out_of_this_train(on);
    }

    fn disembark_passengers(&mut self) {
        self.behavior.disembark();
    }

    fn destroy_whole_train_now(&mut self) {
        self.behavior.destroy_whole_train_now();
    }
}

impl Snapshotable for RailroadBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

fn std_angle_diff(angle1: Real, angle2: Real) -> Real {
    let mut diff = angle1 - angle2;
    while diff > std::f32::consts::PI {
        diff -= 2.0 * std::f32::consts::PI;
    }
    while diff < -std::f32::consts::PI {
        diff += 2.0 * std::f32::consts::PI;
    }
    diff
}

fn normalize_to_range(
    value: Real,
    in_min: Real,
    in_max: Real,
    out_min: Real,
    out_max: Real,
) -> Real {
    if (in_max - in_min).abs() < f32::EPSILON {
        return out_min;
    }
    let t = (value - in_min) / (in_max - in_min);
    out_min + (out_max - out_min) * t
}

fn mu_law(value: Real, max_value: Real, mu: Real) -> Real {
    let normalized = value / max_value.max(0.0001);
    let numerator = (1.0 + mu * normalized.abs()).ln();
    let denominator = (1.0 + mu).ln();
    normalized.signum() * (numerator / denominator)
}
