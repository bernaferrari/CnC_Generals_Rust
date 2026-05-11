//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/FlightDeckBehavior.cpp`.
//!
//! Flight Deck Behavior Module
//!
//! Handles aircraft movement and parking behavior for aircraft carriers.
//! Manages runways, aircraft spacing, takeoff/landing sequences, and healing.
//!
//! Author: Kris Morness, May 2003 (Original C++)
//! Converted to Rust: 2025

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

use crate::ai::group::GuardMode;
use crate::ai::{AiCommandType, CommandSourceType, THE_AI};
use crate::common::xfer::XferExt;
use crate::common::ThingTemplate;
use crate::common::{
    AsciiString, Bool, Coord3D, CoordOrigin, Int, Matrix3D, ModelConditionFlags, ObjectID,
    ObjectStatusMaskType, Real, UnsignedInt, FROM_CENTER_2D, LOGICFRAMES_PER_SECOND, NEVER,
    SECONDS_PER_LOGICFRAME_REAL,
};
use crate::helpers::{
    TheGameLogic, TheParticleSystemManager, ThePartitionManager, TheThingFactory,
};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, DieModuleInterface,
    ExitDoorType as ModuleExitDoorType, ExitInterface as ModuleExitInterface,
    ProductionUpdateInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::{
    BehaviorModuleData, PPInfo as SharedPPInfo,
    ParkingPlaceBehaviorInterface as SharedParkingPlaceBehaviorInterface,
    RunwayReservationType as SharedRunwayReservationType, Team as SharedTeam,
};
use crate::object::{Object as GameObject, INVALID_ID as OBJECT_INVALID_ID};
use crate::template::ObjectTemplate;
use crate::weapon::NO_MAX_SHOTS_LIMIT;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, Thing as ModuleThing};
use glam::EulerRot;
use std::any::Any;

/// Maximum number of runways supported
pub const MAX_RUNWAYS: usize = 2;

/// Invalid object ID constant
pub const INVALID_OBJECT_ID: ObjectID = OBJECT_INVALID_ID;

/// Runway definition structure
#[derive(Debug, Clone)]
pub struct RunwayDefinition {
    /// Bone names for parking spaces
    pub spaces_bone_names: Vec<String>,
    /// Bone names for taxi areas
    pub taxi_bone_names: Vec<String>,
    /// Bone names for creation points
    pub creation_bone_names: Vec<String>,
    /// Takeoff bone names (start and end)
    pub takeoff_bone_names: [String; 2],
    /// Landing bone names (start and end)
    pub landing_bone_names: [String; 2],
    /// Catapult particle system template
    pub catapult_particle_system: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct HealingInfo {
    object_id: ObjectID,
    heal_start_frame: UnsignedInt,
}

impl Default for RunwayDefinition {
    fn default() -> Self {
        Self {
            spaces_bone_names: Vec::new(),
            taxi_bone_names: Vec::new(),
            creation_bone_names: Vec::new(),
            takeoff_bone_names: [String::new(), String::new()],
            landing_bone_names: [String::new(), String::new()],
            catapult_particle_system: None,
        }
    }
}

/// Configuration data for flight deck behavior
#[derive(Debug, Clone)]
pub struct FlightDeckBehaviorModuleData {
    pub base: BehaviorModuleData,
    /// Runway definitions
    pub runway_info: [RunwayDefinition; MAX_RUNWAYS],
    /// Thing template name for creating aircraft
    pub thing_template_name: String,
    /// Amount of healing provided per frame
    pub heal_amount: f32,
    /// Approach height for landing aircraft
    pub approach_height: f32,
    /// Landing deck height offset
    pub landing_deck_height_offset: f32,
    /// Number of rows (spaces per runway)
    pub num_rows: i32,
    /// Number of columns (runways)
    pub num_cols: i32,
    /// Frames for cleanup operations
    pub cleanup_frames: u32,
    /// Frames for human follow operations
    pub human_follow_frames: u32,
    /// Frames for replacement operations
    pub replacement_frames: u32,
    /// Frames for docking animation
    pub dock_animation_frames: u32,
    /// Frames between launch waves
    pub launch_wave_frames: u32,
    /// Frames for launch ramp operations
    pub launch_ramp_frames: u32,
    /// Frames for lowering ramp
    pub lower_ramp_frames: u32,
    /// Frames for catapult firing
    pub catapult_fire_frames: u32,
}

impl Default for FlightDeckBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            runway_info: [RunwayDefinition::default(), RunwayDefinition::default()],
            thing_template_name: String::new(),
            heal_amount: 0.0,
            approach_height: 0.0,
            landing_deck_height_offset: 0.0,
            num_rows: 0,
            num_cols: 0,
            cleanup_frames: 0,
            human_follow_frames: 0,
            replacement_frames: 0,
            dock_animation_frames: 0,
            launch_wave_frames: 0,
            launch_ramp_frames: 0,
            lower_ramp_frames: 0,
            catapult_fire_frames: 0,
        }
    }
}

impl FlightDeckBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FLIGHT_DECK_BEHAVIOR_FIELDS)
    }
}

crate::impl_behavior_module_data_via_base!(FlightDeckBehaviorModuleData, base);

fn parse_int_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(i32),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_int(token)?);
    Ok(())
}

fn parse_real_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(f32),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_duration_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(u32),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_string_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(String),
    tokens: &[&str],
) -> Result<(), INIError> {
    let joined = tokens.join(" ");
    setter(INI::parse_ascii_string(&joined)?);
    Ok(())
}

fn parse_string_vector_field(
    _ini: &mut INI,
    target: &mut Vec<String>,
    tokens: &[&str],
) -> Result<(), INIError> {
    target.clear();
    for token in tokens {
        target.push(INI::parse_ascii_string(token)?);
    }
    Ok(())
}

fn parse_runway_strip_field(
    _ini: &mut INI,
    target: &mut [String; 2],
    tokens: &[&str],
) -> Result<(), INIError> {
    let start = tokens.first().ok_or(INIError::InvalidData)?;
    let end = tokens.get(1).ok_or(INIError::InvalidData)?;
    target[0] = INI::parse_ascii_string(start)?;
    target[1] = INI::parse_ascii_string(end)?;
    Ok(())
}

fn parse_runway_spaces_1(
    ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_string_vector_field(ini, &mut data.runway_info[0].spaces_bone_names, tokens)
}

fn parse_runway_taxi_1(
    ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_string_vector_field(ini, &mut data.runway_info[0].taxi_bone_names, tokens)
}

fn parse_runway_creation_1(
    ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_string_vector_field(ini, &mut data.runway_info[0].creation_bone_names, tokens)
}

fn parse_runway_takeoff_1(
    ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_runway_strip_field(ini, &mut data.runway_info[0].takeoff_bone_names, tokens)
}

fn parse_runway_landing_1(
    ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_runway_strip_field(ini, &mut data.runway_info[0].landing_bone_names, tokens)
}

fn parse_runway_catapult_1(
    _ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let joined = tokens.join(" ");
    let name = INI::parse_ascii_string(&joined)?;
    data.runway_info[0].catapult_particle_system = Some(name);
    Ok(())
}

fn parse_runway_spaces_2(
    ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_string_vector_field(ini, &mut data.runway_info[1].spaces_bone_names, tokens)
}

fn parse_runway_taxi_2(
    ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_string_vector_field(ini, &mut data.runway_info[1].taxi_bone_names, tokens)
}

fn parse_runway_creation_2(
    ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_string_vector_field(ini, &mut data.runway_info[1].creation_bone_names, tokens)
}

fn parse_runway_takeoff_2(
    ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_runway_strip_field(ini, &mut data.runway_info[1].takeoff_bone_names, tokens)
}

fn parse_runway_landing_2(
    ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_runway_strip_field(ini, &mut data.runway_info[1].landing_bone_names, tokens)
}

fn parse_runway_catapult_2(
    _ini: &mut INI,
    data: &mut FlightDeckBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let joined = tokens.join(" ");
    let name = INI::parse_ascii_string(&joined)?;
    data.runway_info[1].catapult_particle_system = Some(name);
    Ok(())
}

const FLIGHT_DECK_BEHAVIOR_FIELDS: &[FieldParse<FlightDeckBehaviorModuleData>] = &[
    FieldParse {
        token: "NumRunways",
        parse: |ini, data, tokens| parse_int_field(ini, &mut |v| data.num_cols = v, tokens),
    },
    FieldParse {
        token: "NumSpacesPerRunway",
        parse: |ini, data, tokens| parse_int_field(ini, &mut |v| data.num_rows = v, tokens),
    },
    FieldParse {
        token: "Runway1Spaces",
        parse: parse_runway_spaces_1,
    },
    FieldParse {
        token: "Runway1Takeoff",
        parse: parse_runway_takeoff_1,
    },
    FieldParse {
        token: "Runway1Landing",
        parse: parse_runway_landing_1,
    },
    FieldParse {
        token: "Runway1Taxi",
        parse: parse_runway_taxi_1,
    },
    FieldParse {
        token: "Runway1Creation",
        parse: parse_runway_creation_1,
    },
    FieldParse {
        token: "Runway1CatapultSystem",
        parse: parse_runway_catapult_1,
    },
    FieldParse {
        token: "Runway2Spaces",
        parse: parse_runway_spaces_2,
    },
    FieldParse {
        token: "Runway2Takeoff",
        parse: parse_runway_takeoff_2,
    },
    FieldParse {
        token: "Runway2Landing",
        parse: parse_runway_landing_2,
    },
    FieldParse {
        token: "Runway2Taxi",
        parse: parse_runway_taxi_2,
    },
    FieldParse {
        token: "Runway2Creation",
        parse: parse_runway_creation_2,
    },
    FieldParse {
        token: "Runway2CatapultSystem",
        parse: parse_runway_catapult_2,
    },
    FieldParse {
        token: "ApproachHeight",
        parse: |ini, data, tokens| parse_real_field(ini, &mut |v| data.approach_height = v, tokens),
    },
    FieldParse {
        token: "LandingDeckHeightOffset",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut |v| data.landing_deck_height_offset = v, tokens)
        },
    },
    FieldParse {
        token: "HealAmountPerSecond",
        parse: |ini, data, tokens| parse_real_field(ini, &mut |v| data.heal_amount = v, tokens),
    },
    FieldParse {
        token: "ParkingCleanupPeriod",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.cleanup_frames = v, tokens)
        },
    },
    FieldParse {
        token: "HumanFollowPeriod",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.human_follow_frames = v, tokens)
        },
    },
    FieldParse {
        token: "PayloadTemplate",
        parse: |ini, data, tokens| {
            parse_string_field(ini, &mut |v| data.thing_template_name = v, tokens)
        },
    },
    FieldParse {
        token: "ReplacementDelay",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.replacement_frames = v, tokens)
        },
    },
    FieldParse {
        token: "DockAnimationDelay",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.dock_animation_frames = v, tokens)
        },
    },
    FieldParse {
        token: "LaunchWaveDelay",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.launch_wave_frames = v, tokens)
        },
    },
    FieldParse {
        token: "LaunchRampDelay",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.launch_ramp_frames = v, tokens)
        },
    },
    FieldParse {
        token: "LowerRampDelay",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.lower_ramp_frames = v, tokens)
        },
    },
    FieldParse {
        token: "CatapultFireDelay",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.catapult_fire_frames = v, tokens)
        },
    },
];

/// Result type for behavior operations
pub type BehaviorResult<T> = Result<T, BehaviorError>;

/// Error types for behavior operations
#[derive(Debug, thiserror::Error)]
pub enum BehaviorError {
    #[error("Object not found: {id}")]
    ObjectNotFound { id: ObjectID },
    #[error("No available parking space")]
    NoAvailableParkingSpace,
    #[error("No available runway")]
    NoAvailableRunway,
}

/// Parking space information (matches C++ FlightDeckInfo).
#[derive(Debug, Clone)]
struct ParkingSpace {
    pub object_id: ObjectID,
    pub position: Coord3D,
    pub orientation: f32,
    pub runway_index: usize,
}

/// Runway information (matches C++ RunwayInfo).
#[derive(Debug, Clone)]
struct Runway {
    pub start: Coord3D,
    pub start_transform: Matrix3D,
    pub end: Coord3D,
    pub landing_start: Coord3D,
    pub landing_end: Coord3D,
    pub taxi_locations: Vec<Coord3D>,
    pub creation_locations: Vec<Coord3D>,
    pub start_orient: f32,
    pub in_use_by_for_takeoff: ObjectID,
    pub in_use_by_for_landing: ObjectID,
}

impl Default for Runway {
    fn default() -> Self {
        Self {
            start: Coord3D::origin(),
            start_transform: Matrix3D::IDENTITY,
            end: Coord3D::origin(),
            landing_start: Coord3D::origin(),
            landing_end: Coord3D::origin(),
            taxi_locations: Vec::new(),
            creation_locations: Vec::new(),
            start_orient: 0.0,
            in_use_by_for_takeoff: OBJECT_INVALID_ID,
            in_use_by_for_landing: OBJECT_INVALID_ID,
        }
    }
}

/// Thread-safe flight deck behavior implementation
#[derive(Debug)]
pub struct FlightDeckBehavior {
    /// Configuration data
    config: FlightDeckBehaviorModuleData,
    /// Template for aircraft creation
    thing_template: Option<Arc<dyn ThingTemplate>>,
    /// Internal state
    state: Arc<RwLock<FlightDeckState>>,
    /// Cached taxi locations per runway (for returning references)
    taxi_locations_cache: Vec<Vec<Coord3D>>,
    /// Cached creation locations per runway (for returning references)
    creation_locations_cache: Vec<Vec<Coord3D>>,
    /// Designated command target (matches C++ carrier order propagation)
    designated_target: ObjectID,
    /// Designated command position
    designated_position: Coord3D,
    /// Designated command type
    designated_command: AiCommandType,
    /// Object ID this behavior belongs to
    object_id: ObjectID,
}

/// Internal state for the flight deck
#[derive(Debug, Clone)]
struct FlightDeckState {
    /// Parking spaces
    parking_spaces: Vec<ParkingSpace>,
    /// Runway information
    runways: Vec<Runway>,
    /// Objects being healed
    healing_objects: VecDeque<HealingInfo>,
    /// Next heal frame
    next_heal_frame: UnsignedInt,
    /// Next cleanup frame
    next_cleanup_frame: UnsignedInt,
    /// Started production frame
    started_production_frame: UnsignedInt,
    /// Next allowed production frame
    next_allowed_production_frame: UnsignedInt,
    /// Next launch wave frame per runway
    next_launch_wave_frame: [UnsignedInt; MAX_RUNWAYS],
    /// Ramp up completion frame per runway
    ramp_up_frame: [UnsignedInt; MAX_RUNWAYS],
    /// Catapult fire frame per runway
    catapult_system_frame: [UnsignedInt; MAX_RUNWAYS],
    /// Lower ramp frame per runway
    lower_ramp_frame: [UnsignedInt; MAX_RUNWAYS],
    /// Ramp state per runway
    ramp_up: [Bool; MAX_RUNWAYS],
    /// Whether bone locations have been resolved
    got_info: Bool,
}

impl FlightDeckBehavior {
    /// Create a new flight deck behavior
    pub fn new(object_id: ObjectID, config: FlightDeckBehaviorModuleData) -> Self {
        let num_spaces = (config.num_rows * config.num_cols) as usize;
        let mut parking_spaces = Vec::with_capacity(num_spaces);

        // Initialize parking spaces
        // Note: In C++ (FlightDeckBehavior.cpp:163-212), positions are calculated from bone names
        // using getSingleLogicalBonePosition(). In Rust, we initialize with zero and expect
        // the game engine to provide bone position lookups during runtime.
        let num_rows = config.num_rows.max(0) as usize;
        let num_cols = config.num_cols.max(0) as usize;
        for _row in 0..num_rows {
            for col in 0..num_cols {
                parking_spaces.push(ParkingSpace {
                    object_id: INVALID_OBJECT_ID,
                    position: Coord3D::new(0.0, 0.0, 0.0), // Will be set from bone positions at runtime
                    orientation: 0.0,
                    runway_index: col,
                });
            }
        }

        // Initialize runways
        let mut runways = Vec::with_capacity(config.num_cols as usize);
        for _ in 0..config.num_cols {
            runways.push(Runway::default());
        }

        let state = FlightDeckState {
            parking_spaces,
            runways,
            healing_objects: VecDeque::new(),
            next_heal_frame: NEVER,
            next_cleanup_frame: 0,
            started_production_frame: NEVER,
            next_allowed_production_frame: 0,
            next_launch_wave_frame: [0; MAX_RUNWAYS],
            ramp_up_frame: [0; MAX_RUNWAYS],
            catapult_system_frame: [0; MAX_RUNWAYS],
            lower_ramp_frame: [0; MAX_RUNWAYS],
            ramp_up: [false; MAX_RUNWAYS],
            got_info: false,
        };

        TheGameLogic::set_wake_frame(object_id, UpdateSleepTime::None);

        Self {
            config,
            thing_template: None,
            state: Arc::new(RwLock::new(state)),
            taxi_locations_cache: vec![Vec::new(); num_cols],
            creation_locations_cache: vec![Vec::new(); num_cols],
            designated_target: INVALID_OBJECT_ID,
            designated_position: Coord3D::origin(),
            designated_command: AiCommandType::NoCommand,
            object_id,
        }
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<FlightDeckBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing
            .as_object()
            .ok_or_else(|| "FlightDeckBehavior requires an owning object".to_string())?;
        Ok(Self::new(
            module_object.get_object_id(),
            module_data.as_ref().clone(),
        ))
    }

    /// Update the flight deck behavior (called each frame)
    pub fn update(&mut self, current_frame: u32) -> BehaviorResult<UpdateSleepTime> {
        if !self.state.read().unwrap().got_info {
            self.build_info(true)?;
        }
        {
            let mut state = self.state.write().unwrap();
            self.purge_dead(&mut state);

            // Update healing
            self.update_healing(&mut state, current_frame)?;

            // Update parking space assignments
            self.update_parking_assignments(&mut state, current_frame);

            // Handle replacement production
            self.update_replacements(&mut state, current_frame);
        }

        let mut state_for_launch = { self.state.read().unwrap().clone() };
        self.update_launch_waves(&mut state_for_launch, current_frame);
        {
            let mut state = self.state.write().unwrap();
            *state = state_for_launch;
        }

        let state = self.state.read().unwrap();
        if let Some(owner_arc) = TheGameLogic::find_object_by_id(self.object_id) {
            if let Ok(mut owner_guard) = owner_arc.write() {
                let has_aircraft = state
                    .parking_spaces
                    .iter()
                    .any(|space| space.object_id != INVALID_OBJECT_ID);
                owner_guard.set_status(ObjectStatusMaskType::NO_ATTACK, !has_aircraft);
            }
        }

        Ok(UPDATE_SLEEP_NONE)
    }

    fn purge_dead(&self, state: &mut FlightDeckState) {
        for space in state.parking_spaces.iter_mut() {
            if space.object_id == INVALID_OBJECT_ID {
                continue;
            }
            let is_dead = TheGameLogic::find_object_by_id(space.object_id)
                .and_then(|arc| arc.read().ok().map(|guard| guard.is_effectively_dead()))
                .unwrap_or(true);
            if is_dead {
                space.object_id = INVALID_OBJECT_ID;
            }
        }

        for runway in state.runways.iter_mut() {
            if runway.in_use_by_for_takeoff != INVALID_OBJECT_ID {
                let dead = TheGameLogic::find_object_by_id(runway.in_use_by_for_takeoff)
                    .and_then(|arc| arc.read().ok().map(|guard| guard.is_effectively_dead()))
                    .unwrap_or(true);
                if dead {
                    runway.in_use_by_for_takeoff = INVALID_OBJECT_ID;
                }
            }
            if runway.in_use_by_for_landing != INVALID_OBJECT_ID {
                let dead = TheGameLogic::find_object_by_id(runway.in_use_by_for_landing)
                    .and_then(|arc| arc.read().ok().map(|guard| guard.is_effectively_dead()))
                    .unwrap_or(true);
                if dead {
                    runway.in_use_by_for_landing = INVALID_OBJECT_ID;
                }
            }
        }

        let mut purged = false;
        state.healing_objects.retain(|info| {
            if info.object_id == INVALID_OBJECT_ID {
                purged = true;
                return false;
            }
            let dead = TheGameLogic::find_object_by_id(info.object_id)
                .and_then(|arc| arc.read().ok().map(|guard| guard.is_effectively_dead()))
                .unwrap_or(true);
            if dead {
                purged = true;
            }
            !dead
        });
        if purged {
            self.reset_wake_frame(state);
        }
    }

    fn build_info(&mut self, create_units: Bool) -> BehaviorResult<()> {
        let mut state = self.state.write().unwrap();
        if state.got_info {
            return Ok(());
        }

        let Some(owner_arc) = TheGameLogic::find_object_by_id(self.object_id) else {
            return Err(BehaviorError::ObjectNotFound { id: self.object_id });
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return Err(BehaviorError::ObjectNotFound { id: self.object_id });
        };
        if owner_guard.test_status(crate::common::ObjectStatusTypes::UnderConstruction)
            || owner_guard.test_status(crate::common::ObjectStatusTypes::Sold)
        {
            return Ok(());
        }

        self.thing_template =
            TheThingFactory::find_template(self.config.thing_template_name.as_str());

        let num_rows = self.config.num_rows.max(0) as usize;
        let num_cols = self.config.num_cols.max(0).min(MAX_RUNWAYS as i32) as usize;

        state.parking_spaces.clear();
        state.runways.clear();
        self.taxi_locations_cache = vec![Vec::new(); num_cols];
        self.creation_locations_cache = vec![Vec::new(); num_cols];

        for row in 0..num_rows {
            for col in 0..num_cols {
                let runway_info = &self.config.runway_info[col];
                let bone_name = runway_info.spaces_bone_names.get(row);
                let mut prep = Coord3D::origin();
                let mut orient = 0.0;
                if let Some(bone_name) = bone_name {
                    let (found, pos, transform) =
                        owner_guard.get_single_logical_bone_position(bone_name);
                    if found {
                        prep = pos;
                        let (_, rotation, _) = transform.to_scale_rotation_translation();
                        orient = rotation.to_euler(EulerRot::XYZ).2;
                    }
                }

                let mut object_id = INVALID_OBJECT_ID;
                if let (Some(template), true) = (&self.thing_template, create_units) {
                    if let Some(player_arc) = owner_guard.get_controlling_player() {
                        if let Ok(player_guard) = player_arc.read() {
                            if let Some(team_arc) = player_guard.get_default_team() {
                                if let Ok(team_guard) = team_arc.read() {
                                    if let Ok(factory) = TheThingFactory::get() {
                                        if let Ok(jet_arc) =
                                            factory.new_object(Arc::clone(template), &*team_guard)
                                        {
                                            if let Ok(mut jet_guard) = jet_arc.write() {
                                                jet_guard.set_producer(Some(&owner_guard));
                                                if self.config.landing_deck_height_offset != 0.0 {
                                                    jet_guard.set_status(
                                                        ObjectStatusMaskType::DECK_HEIGHT_OFFSET,
                                                        true,
                                                    );
                                                }
                                                let _ = jet_guard.set_position(&prep);
                                                let _ = jet_guard.set_orientation(orient);
                                                object_id = jet_guard.get_id();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                state.parking_spaces.push(ParkingSpace {
                    object_id,
                    position: prep,
                    orientation: orient,
                    runway_index: col,
                });
            }
        }

        for col in 0..num_cols {
            let runway_info = &self.config.runway_info[col];
            let mut runway = Runway::default();

            let (found_start, start, _) =
                owner_guard.get_single_logical_bone_position(&runway_info.takeoff_bone_names[0]);
            let (found_end, end, _) =
                owner_guard.get_single_logical_bone_position(&runway_info.takeoff_bone_names[1]);
            if found_start {
                runway.start = start;
            }
            if found_end {
                runway.end = end;
            }

            let (found_landing_start, landing_start, _) =
                owner_guard.get_single_logical_bone_position(&runway_info.landing_bone_names[0]);
            let (found_landing_end, landing_end, _) =
                owner_guard.get_single_logical_bone_position(&runway_info.landing_bone_names[1]);
            if found_landing_start {
                runway.landing_start = landing_start;
            }
            if found_landing_end {
                runway.landing_end = landing_end;
            }

            runway.taxi_locations.clear();
            for bone in &runway_info.taxi_bone_names {
                let (found, pos, _) = owner_guard.get_single_logical_bone_position(bone);
                if found {
                    runway.taxi_locations.push(pos);
                }
            }
            if col < self.taxi_locations_cache.len() {
                self.taxi_locations_cache[col] = runway.taxi_locations.clone();
            }

            runway.creation_locations.clear();
            let mut first_creation = true;
            for bone in &runway_info.creation_bone_names {
                let (found, pos, transform) = owner_guard.get_single_logical_bone_position(bone);
                if found {
                    runway.creation_locations.push(pos);
                    if first_creation {
                        first_creation = false;
                        runway.start_transform = transform;
                        let (_, rotation, _) = transform.to_scale_rotation_translation();
                        runway.start_orient = rotation.to_euler(EulerRot::XYZ).2;
                    }
                }
            }
            if col < self.creation_locations_cache.len() {
                self.creation_locations_cache[col] = runway.creation_locations.clone();
            }

            runway.in_use_by_for_takeoff = INVALID_OBJECT_ID;
            runway.in_use_by_for_landing = INVALID_OBJECT_ID;

            state.runways.push(runway);
        }

        state.got_info = true;
        Ok(())
    }

    /// Update healing for parked aircraft
    fn update_healing(&self, state: &mut FlightDeckState, now: UnsignedInt) -> BehaviorResult<()> {
        const HEAL_RATE_FRAMES: UnsignedInt = LOGICFRAMES_PER_SECOND / 5;

        if now < state.next_heal_frame {
            return Ok(());
        }

        state.next_heal_frame = now + HEAL_RATE_FRAMES;
        let amount =
            HEAL_RATE_FRAMES as f32 * self.config.heal_amount * SECONDS_PER_LOGICFRAME_REAL;

        state.healing_objects.retain(|info| {
            if info.object_id == INVALID_OBJECT_ID {
                return false;
            }
            let Some(obj_arc) = TheGameLogic::find_object_by_id(info.object_id) else {
                return false;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                return true;
            };
            if obj_guard.is_effectively_dead() {
                return false;
            }
            drop(obj_guard);
            let _ = self.heal_object(info.object_id, amount);
            true
        });
        Ok(())
    }

    fn reset_wake_frame(&self, state: &mut FlightDeckState) {
        const HEAL_RATE_FRAMES: UnsignedInt = LOGICFRAMES_PER_SECOND / 5;
        if state.healing_objects.is_empty() {
            state.next_heal_frame = NEVER;
        } else {
            state.next_heal_frame = TheGameLogic::get_frame() + HEAL_RATE_FRAMES;
        }
    }

    fn update_parking_assignments(&self, state: &mut FlightDeckState, now: UnsignedInt) {
        if self.config.cleanup_frames == 0 {
            return;
        }

        if now < state.next_cleanup_frame {
            return;
        }

        state.next_cleanup_frame = now + self.config.cleanup_frames;

        let num_cols = self.config.num_cols.max(0) as usize;
        if num_cols == 0 {
            return;
        }

        let mut complete = vec![false; num_cols];

        for index in 0..state.parking_spaces.len() {
            let runway_index = state.parking_spaces[index].runway_index;
            let non_idle_id = state.parking_spaces[index].object_id;

            let non_idle_can_give_up = if non_idle_id == INVALID_OBJECT_ID {
                true
            } else if let Some(obj_arc) = TheGameLogic::find_object_by_id(non_idle_id) {
                if let Ok(obj_guard) = obj_arc.read() {
                    self.is_able_to_give_up_parking_space(&obj_guard, state)
                } else {
                    false
                }
            } else {
                true
            };

            if !non_idle_can_give_up {
                continue;
            }

            let mut moved = false;
            let mut temp_index = index + num_cols;
            while temp_index < state.parking_spaces.len() {
                if complete[runway_index] {
                    break;
                }

                let parked_id = state.parking_spaces[temp_index].object_id;
                if parked_id != INVALID_OBJECT_ID {
                    if let Some(parked_arc) = TheGameLogic::find_object_by_id(parked_id) {
                        if let Ok(mut parked_guard) = parked_arc.write() {
                            if self.is_able_to_move_forward(&parked_guard) {
                                state.parking_spaces[index].object_id = parked_id;
                                state.parking_spaces[temp_index].object_id = non_idle_id;

                                parked_guard
                                    .set_status(ObjectStatusMaskType::REASSIGN_PARKING, true);

                                if let Some(ai) = parked_guard.get_ai() {
                                    let mut exit_path = Vec::with_capacity(1);
                                    exit_path.push(state.parking_spaces[index].position);
                                    ai.ai_follow_exit_production_path(
                                        &exit_path,
                                        Some(self.object_id),
                                        crate::ai::CommandSourceType::FromAi,
                                    );
                                }

                                complete[runway_index] = true;
                                state.next_cleanup_frame = now + self.config.human_follow_frames;
                                moved = true;
                            }
                        }
                    }
                }

                if moved {
                    break;
                }

                temp_index += num_cols;
            }
        }
    }

    fn update_replacements(&self, state: &mut FlightDeckState, now: UnsignedInt) {
        if state.next_allowed_production_frame <= now {
            state.started_production_frame = NEVER;
        }

        if self.config.thing_template_name.is_empty() {
            return;
        }

        let Some(owner_arc) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };
        let Ok(owner_guard) = owner_arc.write() else {
            return;
        };

        for space in &state.parking_spaces {
            if space.object_id != INVALID_OBJECT_ID {
                continue;
            }

            if now < state.next_allowed_production_frame {
                break;
            }

            let mut queued = false;
            let mut checked = false;
            for module_handle in owner_guard.behavior_modules() {
                let matched = module_handle.with_module_downcast::<
                    crate::object::production::production_update_complete::ProductionUpdateCompleteModule,
                    _,
                    _,
                >(|prod_module| {
                    let prod = prod_module.behavior_mut();
                    if prod.get_queue_size() == 0 && !prod.is_producing() {
                        let player_id =
                            owner_guard.get_controlling_player_id().unwrap_or(0) as ObjectID;
                        if prod
                            .start_production(self.config.thing_template_name.clone(), player_id)
                            .is_ok()
                        {
                            queued = true;
                        }
                    }
                });
                if matched.is_some() {
                    checked = true;
                    break;
                }
            }

            if !checked {
                for behavior in owner_guard.get_behavior_modules() {
                    let Ok(mut behavior_guard) = behavior.lock() else {
                        continue;
                    };
                    if let Some(prod) = behavior_guard.get_production_update_interface() {
                        if prod.get_queue_size() == 0 && !prod.is_producing() {
                            let player_id =
                                owner_guard.get_controlling_player_id().unwrap_or(0) as ObjectID;
                            if prod
                                .start_production(
                                    self.config.thing_template_name.clone(),
                                    player_id,
                                )
                                .is_ok()
                            {
                                queued = true;
                            }
                        }
                    }

                    if queued {
                        break;
                    }
                }
            }

            if queued {
                state.started_production_frame = now;
                state.next_allowed_production_frame =
                    now + self.config.replacement_frames + self.config.dock_animation_frames;
            }

            break;
        }
    }

    fn has_takeoff_orders(&mut self) -> Bool {
        let target_alive = if self.designated_target == INVALID_OBJECT_ID {
            true
        } else {
            TheGameLogic::find_object_by_id(self.designated_target).is_some()
        };

        match self.designated_command {
            AiCommandType::GuardPosition
            | AiCommandType::AttackPosition
            | AiCommandType::AttackMoveToPosition => true,
            AiCommandType::ForceAttackObject | AiCommandType::AttackObject => {
                if target_alive {
                    true
                } else {
                    self.designated_command = AiCommandType::NoCommand;
                    self.designated_target = INVALID_OBJECT_ID;
                    false
                }
            }
            AiCommandType::Idle => false,
            _ => false,
        }
    }

    fn propagate_orders_to_planes(&self, state: &FlightDeckState) {
        for space in &state.parking_spaces {
            if space.object_id == INVALID_OBJECT_ID {
                continue;
            }
            let Some(jet_arc) = TheGameLogic::find_object_by_id(space.object_id) else {
                continue;
            };
            let Ok(jet_guard) = jet_arc.read() else {
                continue;
            };
            if self.is_able_to_give_up_parking_space(&jet_guard, state) {
                self.propagate_order_to_specific_plane(&jet_guard);
            }
        }
    }

    fn propagate_order_to_specific_plane(&self, jet: &GameObject) {
        let Some(ai) = jet.get_ai() else {
            return;
        };
        let target_arc = if self.designated_target != INVALID_OBJECT_ID {
            TheGameLogic::find_object_by_id(self.designated_target)
        } else {
            None
        };

        match self.designated_command {
            AiCommandType::GuardPosition => {
                ai.ai_guard_position(
                    &self.designated_position,
                    GuardMode::Normal,
                    CommandSourceType::FromAi,
                );
            }
            AiCommandType::AttackPosition => {
                ai.ai_attack_position(
                    &self.designated_position,
                    NO_MAX_SHOTS_LIMIT,
                    CommandSourceType::FromAi,
                );
            }
            AiCommandType::ForceAttackObject | AiCommandType::AttackObject => {
                if let Some(target_arc) = target_arc {
                    ai.ai_force_attack_object(
                        &target_arc,
                        NO_MAX_SHOTS_LIMIT,
                        CommandSourceType::FromPlayer,
                    );
                }
            }
            AiCommandType::AttackMoveToPosition => {
                ai.ai_attack_move_to_position(
                    &self.designated_position,
                    NO_MAX_SHOTS_LIMIT,
                    CommandSourceType::FromAi,
                );
            }
            AiCommandType::Idle => {
                ai.ai_enter(self.object_id, CommandSourceType::FromAi);
            }
            _ => {}
        }
    }

    fn door_flags_for_runway(
        &self,
        runway_index: usize,
    ) -> Option<(ModelConditionFlags, ModelConditionFlags)> {
        match runway_index {
            0 => Some((
                ModelConditionFlags::DOOR_2_OPENING,
                ModelConditionFlags::DOOR_2_CLOSING,
            )),
            1 => Some((
                ModelConditionFlags::DOOR_3_OPENING,
                ModelConditionFlags::DOOR_3_CLOSING,
            )),
            2 => Some((
                ModelConditionFlags::DOOR_4_OPENING,
                ModelConditionFlags::DOOR_4_CLOSING,
            )),
            _ => None,
        }
    }

    fn update_launch_waves(&mut self, state: &mut FlightDeckState, now: UnsignedInt) {
        let num_cols = self.config.num_cols.max(0) as usize;

        let Some(owner_arc) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };
        let Ok(mut owner_guard) = owner_arc.write() else {
            return;
        };

        for i in 0..num_cols {
            let front_space = match state.parking_spaces.get(i) {
                Some(space) => space,
                None => continue,
            };

            let jet_id = front_space.object_id;
            let Some(jet_arc) = TheGameLogic::find_object_by_id(jet_id) else {
                continue;
            };
            let Ok(jet_guard) = jet_arc.read() else {
                continue;
            };

            if !self.is_able_to_give_up_parking_space(&jet_guard, state)
                && self.is_in_position_to_takeoff(&jet_guard, state)
                && self.has_takeoff_orders()
            {
                if state.next_launch_wave_frame[i] <= now {
                    if !state.ramp_up[i] {
                        state.ramp_up[i] = true;
                        state.ramp_up_frame[i] = now + self.config.launch_ramp_frames;
                        state.lower_ramp_frame[i] = NEVER;

                        if let Some((opening, closing)) = self.door_flags_for_runway(i) {
                            let _ =
                                owner_guard.clear_and_set_model_condition_flags(closing, opening);
                        }
                    }

                    if state.ramp_up[i] && state.ramp_up_frame[i] <= now {
                        self.propagate_order_to_specific_plane(&jet_guard);
                        state.next_launch_wave_frame[i] = now + self.config.launch_wave_frames;
                        state.catapult_system_frame[i] = now + self.config.catapult_fire_frames;
                        state.lower_ramp_frame[i] = now + self.config.lower_ramp_frames;
                    }
                }
            }

            if state.catapult_system_frame[i] <= now {
                if let Some(template) = self
                    .config
                    .runway_info
                    .get(i)
                    .and_then(|info| info.catapult_particle_system.as_deref())
                {
                    if let Some(manager) = TheParticleSystemManager::get() {
                        if let Some(system_id) = manager.create_particle_system(Some(template)) {
                            let runway = &state.runways[i];
                            manager
                                .set_particle_system_transform(system_id, &runway.start_transform);
                            manager.set_particle_system_position(system_id, &runway.start);
                        }
                    }
                }
                state.catapult_system_frame[i] = NEVER;
            }

            if state.ramp_up[i] && state.lower_ramp_frame[i] <= now {
                state.ramp_up[i] = false;
                if let Some((opening, closing)) = self.door_flags_for_runway(i) {
                    let _ = owner_guard.clear_and_set_model_condition_flags(opening, closing);
                }
            }
        }
    }

    /// Heal an object
    fn heal_object(&self, object_id: ObjectID, amount: f32) -> BehaviorResult<()> {
        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Err(BehaviorError::ObjectNotFound { id: object_id });
        };
        let Ok(mut object_guard) = object_arc.write() else {
            return Err(BehaviorError::ObjectNotFound { id: object_id });
        };

        if let Some(source_arc) = TheGameLogic::find_object_by_id(self.object_id) {
            if let Ok(source_guard) = source_arc.read() {
                let _ = object_guard.attempt_healing(amount, Some(&*source_guard));
                return Ok(());
            }
        }
        let _ = object_guard.attempt_healing(amount, None);
        Ok(())
    }

    /// Process object exit
    fn process_object_exit(&mut self, object_id: ObjectID) -> BehaviorResult<()> {
        let mut pp_info = SharedPPInfo::default();
        let parking_offset = if let Some(arc) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(guard) = arc.read() {
                if let Some(ai) = guard.get_ai() {
                    if let Ok(ai_guard) = ai.lock() {
                        ai_guard.get_parking_offset()
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };

        if !self.reserve_space(object_id, parking_offset, &mut pp_info) {
            return Err(BehaviorError::NoAvailableParkingSpace);
        }

        let (creation_locations, start_orient) = {
            let state = self.state.read().unwrap();
            let space_index = state
                .parking_spaces
                .iter()
                .position(|space| space.object_id == object_id)
                .ok_or(BehaviorError::NoAvailableParkingSpace)?;
            let runway_index = state.parking_spaces[space_index].runway_index;
            let runway = state
                .runways
                .get(runway_index)
                .ok_or(BehaviorError::NoAvailableRunway)?;
            (runway.creation_locations.clone(), runway.start_orient)
        };

        if creation_locations.is_empty() {
            return Err(BehaviorError::NoAvailableRunway);
        }

        let creation_pos = creation_locations[0];
        let heading = start_orient;

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Err(BehaviorError::ObjectNotFound { id: object_id });
        };
        let Ok(mut object_guard) = object_arc.write() else {
            return Err(BehaviorError::ObjectNotFound { id: object_id });
        };

        let _ = object_guard.set_position(&creation_pos);
        let _ = object_guard.set_orientation(heading);
        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pf_arc) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pf_arc.write() {
                    pf.add_object_to_map(object_id, &[creation_pos], false);
                }
            }
        }

        if let Some(ai) = object_guard.get_ai() {
            let mut exit_path = Vec::with_capacity(1);
            exit_path.push(pp_info.parking_space);
            ai.ai_follow_exit_production_path(
                &exit_path,
                Some(self.object_id),
                CommandSourceType::FromAi,
            );
        }

        Ok(())
    }

    /// Kill all parked units
    pub fn kill_all_parked_units(&self) -> BehaviorResult<()> {
        let mut state = self.state.write().unwrap();
        for space in &state.parking_spaces {
            if space.object_id != INVALID_OBJECT_ID {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(space.object_id) else {
                    continue;
                };
                let Ok(mut obj_guard) = obj_arc.write() else {
                    continue;
                };
                if obj_guard.is_effectively_dead() {
                    continue;
                }

                let takeoff_or_landing = obj_guard
                    .get_ai()
                    .and_then(|ai| {
                        ai.lock()
                            .ok()
                            .map(|ai| ai.is_takeoff_or_landing_in_progress())
                    })
                    .unwrap_or(false);

                if obj_guard.is_above_terrain() && !takeoff_or_landing {
                    continue;
                }

                obj_guard.kill(None, None);
            }
        }
        self.purge_dead(&mut state);
        Ok(())
    }

    /// Defect all parked units to a new team
    pub fn defect_all_parked_units(
        &mut self,
        new_team: Arc<RwLock<SharedTeam>>,
        detection_time: u32,
    ) -> BehaviorResult<()> {
        let parked_ids: Vec<ObjectID> = {
            let state = self.state.read().unwrap();
            state
                .parking_spaces
                .iter()
                .filter_map(|space| {
                    if space.object_id != INVALID_OBJECT_ID {
                        Some(space.object_id)
                    } else {
                        None
                    }
                })
                .collect()
        };

        for object_id in parked_ids {
            self.defect_object_to_team(object_id, Arc::clone(&new_team), detection_time)?;
        }
        Ok(())
    }

    /// Kill an object
    #[allow(dead_code)]
    fn kill_object(&self, _object_id: ObjectID) -> BehaviorResult<()> {
        Ok(())
    }

    /// Defect object to team
    fn defect_object_to_team(
        &mut self,
        object_id: ObjectID,
        team: Arc<RwLock<SharedTeam>>,
        detection_time: u32,
    ) -> BehaviorResult<()> {
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(());
        };
        let Ok(mut obj_guard) = obj_arc.write() else {
            return Ok(());
        };
        if obj_guard.is_effectively_dead() {
            return Ok(());
        }

        let takeoff_or_landing = obj_guard
            .get_ai()
            .and_then(|ai| {
                ai.lock()
                    .ok()
                    .map(|ai| ai.is_takeoff_or_landing_in_progress())
            })
            .unwrap_or(false);

        if obj_guard.is_above_terrain() && !takeoff_or_landing {
            let new_team_player_id = team.read().ok().and_then(|t| t.get_controlling_player_id());
            let obj_player_id = obj_guard.get_controlling_player_id();
            if new_team_player_id != obj_player_id {
                if obj_guard.get_producer_id() == self.object_id {
                    obj_guard.set_producer(None);
                }
                drop(obj_guard);
                self.release_space(object_id);
            }
            return Ok(());
        }

        obj_guard.defect(Some(team), detection_time);
        Ok(())
    }

    /// Set healee object
    pub fn set_healee(&self, healee: ObjectID, add: bool) {
        let mut state = self.state.write().unwrap();
        if add {
            if state
                .healing_objects
                .iter()
                .any(|info| info.object_id == healee)
            {
                return;
            }
            state.healing_objects.push_back(HealingInfo {
                object_id: healee,
                heal_start_frame: TheGameLogic::get_frame(),
            });
            self.reset_wake_frame(&mut state);
        } else {
            let initial_len = state.healing_objects.len();
            state
                .healing_objects
                .retain(|info| info.object_id != healee);
            if state.healing_objects.len() != initial_len {
                self.reset_wake_frame(&mut state);
            }
        }
    }

    /// Calculate best parking assignment
    pub fn calc_best_parking_assignment(
        &mut self,
        object_id: ObjectID,
    ) -> BehaviorResult<(Coord3D, Option<Int>, Option<Int>)> {
        let mut state = self.state.write().unwrap();

        let mut runway_index: Option<usize> = None;
        let mut my_index: Option<usize> = None;
        let mut my_pos = Coord3D::origin();

        for (index, space) in state.parking_spaces.iter().enumerate() {
            if space.object_id == object_id {
                runway_index = Some(space.runway_index);
                my_index = Some(index);
                my_pos = space.position;
                break;
            }
        }

        let Some(runway_index) = runway_index else {
            return Err(BehaviorError::ObjectNotFound { id: object_id });
        };
        let Some(my_index) = my_index else {
            return Err(BehaviorError::ObjectNotFound { id: object_id });
        };

        let mut best_index: Option<usize> = None;
        let mut best_jet_id: ObjectID = INVALID_OBJECT_ID;
        let mut check_for_plane_in_way = false;
        let mut target_pos = my_pos;

        for (index, space) in state.parking_spaces.iter().enumerate() {
            let non_idle_jet_id = space.object_id;

            if index == my_index {
                if let Some(best_index) = best_index {
                    let best_id = best_jet_id;
                    state.parking_spaces[my_index].object_id = best_id;
                    state.parking_spaces[best_index].object_id = object_id;
                    return Ok((target_pos, Some(my_index as Int), Some(best_index as Int)));
                }
                return Err(BehaviorError::NoAvailableParkingSpace);
            }

            if space.runway_index == runway_index {
                let can_take = if non_idle_jet_id == INVALID_OBJECT_ID {
                    true
                } else if let Some(jet_arc) = TheGameLogic::find_object_by_id(non_idle_jet_id) {
                    if let Ok(jet_guard) = jet_arc.read() {
                        self.is_able_to_give_up_parking_space(&jet_guard, &state)
                    } else {
                        false
                    }
                } else {
                    true
                };

                if can_take {
                    if !check_for_plane_in_way {
                        best_index = Some(index);
                        best_jet_id = non_idle_jet_id;
                        check_for_plane_in_way = true;
                        target_pos = space.position;
                    }
                } else if check_for_plane_in_way {
                    check_for_plane_in_way = false;
                    target_pos = my_pos;
                    best_index = None;
                }
            }
        }

        Err(BehaviorError::NoAvailableParkingSpace)
    }

    /// Set rally point
    pub fn set_rally_point(&self, position: Option<Coord3D>) {
        let _ = position;
    }

    /// Get approach height
    pub fn get_approach_height(&self) -> f32 {
        self.config.approach_height
    }

    /// Get landing deck height offset
    pub fn get_landing_deck_height_offset(&self) -> f32 {
        self.config.landing_deck_height_offset
    }

    pub fn ai_do_command(
        &mut self,
        command: AiCommandType,
        position: Option<Coord3D>,
        target: Option<ObjectID>,
        source: CommandSourceType,
    ) {
        if source == CommandSourceType::FromAi {
            return;
        }

        match command {
            AiCommandType::GuardPosition
            | AiCommandType::AttackPosition
            | AiCommandType::AttackMoveToPosition => {
                self.designated_target = INVALID_OBJECT_ID;
                self.designated_position = position.unwrap_or_else(Coord3D::origin);
                self.designated_command = command;
            }
            AiCommandType::ForceAttackObject | AiCommandType::AttackObject => {
                self.designated_target = target.unwrap_or(INVALID_OBJECT_ID);
                self.designated_position = Coord3D::origin();
                self.designated_command = command;
            }
            AiCommandType::Idle => {
                self.designated_target = INVALID_OBJECT_ID;
                self.designated_position = Coord3D::origin();
                self.designated_command = command;
            }
            _ => {
                self.designated_command = AiCommandType::NoCommand;
            }
        }

        let state = self.state.read().unwrap();
        self.propagate_orders_to_planes(&state);
    }

    fn is_able_to_give_up_parking_space(&self, jet: &GameObject, state: &FlightDeckState) -> Bool {
        if jet.is_airborne_target() {
            return true;
        }

        let Some(ai_arc) = jet.get_ai() else {
            return false;
        };
        let Ok(mut ai) = ai_arc.lock() else {
            return false;
        };

        if !ai.is_idle() && !ai.is_taxiing_to_parking() {
            let pending = ai.get_pending_command_type();
            if pending == Some(crate::ai::AiCommandType::Enter)
                || self.designated_command == crate::ai::AiCommandType::Idle
            {
                ai.purge_pending_command();
                return false;
            }

            for runway in &state.runways {
                if runway.in_use_by_for_takeoff == jet.get_id() {
                    return false;
                }
            }

            return true;
        }

        false
    }

    fn is_in_position_to_takeoff(&self, jet: &GameObject, state: &FlightDeckState) -> Bool {
        if jet.get_ai().is_none() {
            return false;
        }

        let num_cols = self.config.num_cols.max(0) as usize;
        let row_limit = num_cols.min(state.parking_spaces.len());
        for space in state.parking_spaces.iter().take(row_limit) {
            if space.object_id == jet.get_id() {
                let dist_sqr = ThePartitionManager::get_distance_squared_to_pos(
                    jet,
                    &space.position,
                    FROM_CENTER_2D,
                );
                if dist_sqr < 10.0 {
                    return true;
                }
                return false;
            }
        }

        false
    }

    fn is_able_to_move_forward(&self, jet: &GameObject) -> Bool {
        let Some(ai_arc) = jet.get_ai() else {
            return false;
        };
        let Ok(ai) = ai_arc.lock() else {
            return false;
        };

        !jet.is_airborne_target() && (ai.is_idle() || ai.is_reloading())
    }
}

impl FlightDeckBehavior {
    fn reset_pp_info(&self, info: &mut SharedPPInfo) {
        *info = SharedPPInfo::default();
    }
}

impl SharedParkingPlaceBehaviorInterface for FlightDeckBehavior {
    fn should_reserve_door_when_queued(&self, _thing_template: &ObjectTemplate) -> Bool {
        true
    }

    fn has_available_space_for(&self, _thing_template: &ObjectTemplate) -> Bool {
        let state = self.state.read().unwrap();
        if !state.got_info {
            return false;
        }
        for space in &state.parking_spaces {
            let mut id = space.object_id;
            if id != INVALID_OBJECT_ID {
                let dead = TheGameLogic::find_object_by_id(id)
                    .and_then(|arc| arc.read().ok().map(|guard| guard.is_effectively_dead()))
                    .unwrap_or(true);
                if dead {
                    id = INVALID_OBJECT_ID;
                }
            }
            if id == INVALID_OBJECT_ID {
                return true;
            }
        }
        false
    }

    fn has_reserved_space(&self, object_id: ObjectID) -> Bool {
        if object_id == INVALID_OBJECT_ID {
            return false;
        }
        let state = self.state.read().unwrap();
        if !state.got_info {
            return false;
        }
        state
            .parking_spaces
            .iter()
            .any(|space| space.object_id == object_id)
    }

    fn get_space_index(&self, object_id: ObjectID) -> Int {
        if object_id == INVALID_OBJECT_ID {
            return -1;
        }
        let state = self.state.read().unwrap();
        state
            .parking_spaces
            .iter()
            .position(|space| space.object_id == object_id)
            .map(|i| i as Int)
            .unwrap_or(-1)
    }

    fn reserve_space(
        &mut self,
        object_id: ObjectID,
        parking_offset: Real,
        info: &mut SharedPPInfo,
    ) -> Bool {
        if object_id == INVALID_OBJECT_ID {
            return false;
        }

        if !self.state.read().unwrap().got_info {
            let _ = self.build_info(true);
        }

        let mut state = self.state.write().unwrap();
        self.purge_dead(&mut state);

        let mut target_index = None;
        for (index, space) in state.parking_spaces.iter().enumerate() {
            if space.object_id == object_id {
                target_index = Some(index);
                break;
            }
        }

        if target_index.is_none() {
            target_index = state
                .parking_spaces
                .iter()
                .position(|space| space.object_id == INVALID_OBJECT_ID);
        }

        let Some(target_index) = target_index else {
            return false;
        };

        state.parking_spaces[target_index].object_id = object_id;
        drop(state);

        if self.config.landing_deck_height_offset != 0.0 {
            if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut guard) = obj.write() {
                    guard.set_status(ObjectStatusMaskType::DECK_HEIGHT_OFFSET, true);
                }
            }
        }

        self.calc_pp_info(object_id, info);
        if parking_offset != 0.0 {
            info.parking_space.x += parking_offset * info.parking_orientation.cos();
            info.parking_space.y += parking_offset * info.parking_orientation.sin();
        }

        true
    }

    fn release_space(&mut self, object_id: ObjectID) {
        if !self.state.read().unwrap().got_info {
            let _ = self.build_info(true);
        }
        let mut state = self.state.write().unwrap();
        self.purge_dead(&mut state);
        for space in state.parking_spaces.iter_mut() {
            if space.object_id == object_id {
                space.object_id = INVALID_OBJECT_ID;
                break;
            }
        }

        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(mut guard) = obj.write() {
                guard.clear_status(ObjectStatusMaskType::DECK_HEIGHT_OFFSET);
            }
        }
    }

    fn reserve_runway(&mut self, object_id: ObjectID, for_landing: Bool) -> Bool {
        if object_id == INVALID_OBJECT_ID {
            return false;
        }
        if !self.state.read().unwrap().got_info {
            let _ = self.build_info(true);
        }
        let mut state = self.state.write().unwrap();
        self.purge_dead(&mut state);
        let mut runway_index: Option<usize> = None;

        if !for_landing {
            let num_cols = self.config.num_cols.max(0) as usize;
            let row_limit = num_cols.min(state.parking_spaces.len());
            for space in state.parking_spaces.iter().take(row_limit) {
                if space.object_id == object_id {
                    runway_index = Some(space.runway_index);
                    break;
                }
            }
        } else {
            for space in state.parking_spaces.iter() {
                if space.object_id == object_id {
                    runway_index = Some(space.runway_index);
                    break;
                }
            }
        }

        let Some(runway_index) = runway_index else {
            return false;
        };

        let Some(runway) = state.runways.get_mut(runway_index) else {
            return false;
        };

        if for_landing {
            if runway.in_use_by_for_landing == object_id {
                return true;
            }
            if runway.in_use_by_for_landing == INVALID_OBJECT_ID {
                runway.in_use_by_for_landing = object_id;
                return true;
            }
            return false;
        }

        if runway.in_use_by_for_takeoff == object_id {
            return true;
        }
        if runway.in_use_by_for_takeoff == INVALID_OBJECT_ID {
            runway.in_use_by_for_takeoff = object_id;
            return true;
        }
        false
    }

    fn calc_pp_info(&self, object_id: ObjectID, info: &mut SharedPPInfo) {
        let state = self.state.read().unwrap();
        if !state.got_info {
            return;
        }
        if let Some((_, space)) = state
            .parking_spaces
            .iter()
            .enumerate()
            .find(|(_, s)| s.object_id == object_id)
        {
            self.reset_pp_info(info);
            let runway_index = space.runway_index;
            if let Some(runway) = state.runways.get(runway_index) {
                let approach_dist = 0.75;
                info.parking_space = space.position;
                info.runway_prep = space.position;
                info.parking_orientation = space.orientation;
                info.runway_start = runway.start;
                info.runway_end = runway.end;
                info.runway_exit = runway.end;
                info.runway_exit.x += (runway.end.x - runway.start.x) * approach_dist;
                info.runway_exit.y += (runway.end.y - runway.start.y) * approach_dist;
                info.runway_exit.z = runway.end.z
                    + self.config.approach_height
                    + self.config.landing_deck_height_offset;

                info.runway_landing_start = runway.landing_start;
                info.runway_landing_end = runway.landing_end;
                info.runway_approach = runway.landing_start;
                info.runway_approach.x +=
                    (runway.landing_start.x - runway.landing_end.x) * approach_dist;
                info.runway_approach.y +=
                    (runway.landing_start.y - runway.landing_end.y) * approach_dist;
                info.runway_approach.z = runway.landing_start.z
                    + self.config.approach_height
                    + self.config.landing_deck_height_offset;

                let vector = info.runway_start - info.runway_end;
                info.runway_takeoff_dist = vector.length();

                if runway.in_use_by_for_takeoff == object_id {
                    info.runway_start = info.runway_prep;
                }
            }
        }
    }

    fn release_runway(&mut self, object_id: ObjectID) {
        if !self.state.read().unwrap().got_info {
            let _ = self.build_info(true);
        }
        let mut state = self.state.write().unwrap();
        self.purge_dead(&mut state);

        for runway in state.runways.iter_mut() {
            if runway.in_use_by_for_takeoff == object_id {
                runway.in_use_by_for_takeoff = INVALID_OBJECT_ID;
            }
            if runway.in_use_by_for_landing == object_id {
                runway.in_use_by_for_landing = INVALID_OBJECT_ID;
            }
        }
    }

    fn get_runway_count(&self) -> Int {
        self.config.num_cols.max(0)
    }

    fn get_runway_reservation(
        &self,
        runway_index: Int,
        reservation_type: SharedRunwayReservationType,
    ) -> ObjectID {
        if !self.state.read().unwrap().got_info {
            return INVALID_OBJECT_ID;
        }
        let mut state = self.state.write().unwrap();
        self.purge_dead(&mut state);
        if let Some(runway) = state.runways.get(runway_index as usize) {
            match reservation_type {
                SharedRunwayReservationType::Takeoff => runway.in_use_by_for_takeoff,
                SharedRunwayReservationType::Landing => runway.in_use_by_for_landing,
            }
        } else {
            INVALID_OBJECT_ID
        }
    }

    fn transfer_runway_reservation_to_next_in_line_for_takeoff(&mut self, object_id: ObjectID) {
        let _ = object_id;
    }

    fn get_approach_height(&self) -> Real {
        self.config.approach_height
    }

    fn get_landing_deck_height_offset(&self) -> Real {
        self.config.landing_deck_height_offset
    }

    fn set_healee(&mut self, healee: Option<Arc<RwLock<GameObject>>>, add: Bool) {
        let object_id = healee.and_then(|obj| obj.read().ok().map(|guard| guard.get_id()));
        let Some(object_id) = object_id else { return };
        FlightDeckBehavior::set_healee(self, object_id, add);
    }

    fn kill_all_parked_units(&mut self) {
        let _ = FlightDeckBehavior::kill_all_parked_units(self);
    }

    fn defect_all_parked_units(
        &mut self,
        new_team: Arc<RwLock<SharedTeam>>,
        detection_time: UnsignedInt,
    ) {
        let _ = FlightDeckBehavior::defect_all_parked_units(self, new_team, detection_time);
    }

    fn calc_best_parking_assignment(
        &mut self,
        object_id: ObjectID,
        pos: &mut Coord3D,
        mut old_index: Option<&mut Int>,
        mut new_index: Option<&mut Int>,
    ) -> Bool {
        match FlightDeckBehavior::calc_best_parking_assignment(self, object_id) {
            Ok((position, old, new)) => {
                *pos = position;
                if let (Some(old_index), Some(old)) = (old_index.as_deref_mut(), old) {
                    *old_index = old;
                }
                if let (Some(new_index), Some(new)) = (new_index.as_deref_mut(), new) {
                    *new_index = new;
                }
                true
            }
            Err(_) => false,
        }
    }

    fn get_taxi_locations(&self, id: ObjectID) -> Option<&Vec<Coord3D>> {
        let state = self.state.read().unwrap();
        let runway_index = state
            .parking_spaces
            .iter()
            .find(|space| space.object_id == id)
            .map(|space| space.runway_index)?;
        self.taxi_locations_cache.get(runway_index)
    }

    fn get_creation_locations(&self, id: ObjectID) -> Option<&Vec<Coord3D>> {
        let state = self.state.read().unwrap();
        let runway_index = state
            .parking_spaces
            .iter()
            .find(|space| space.object_id == id)
            .map(|space| space.runway_index)?;
        self.creation_locations_cache.get(runway_index)
    }
}

impl ModuleExitInterface for FlightDeckBehavior {
    fn can_exit(&self, _object_id: ObjectID) -> Bool {
        true
    }

    fn exit(&mut self, object_id: ObjectID) -> Bool {
        let _ = object_id;
        true
    }

    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&crate::object::Object>,
        _spawn: Option<&crate::object::Object>,
    ) -> ModuleExitDoorType {
        ModuleExitDoorType::Door1
    }

    fn unreserve_door_for_exit(&mut self, _door: ModuleExitDoorType) {
        // No-op: flight deck does not use door reservation clearing.
    }

    fn exit_object_via_door(
        &mut self,
        obj: &Arc<RwLock<crate::object::Object>>,
        door: ModuleExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if matches!(
            door,
            ModuleExitDoorType::None | ModuleExitDoorType::NoneAvailable
        ) {
            return Ok(());
        }

        let object_id = obj
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(INVALID_OBJECT_ID);
        let _ = self.process_object_exit(object_id);
        Ok(())
    }

    fn exit_object_by_budding(
        &mut self,
        _obj: &Arc<RwLock<crate::object::Object>>,
        _host: Option<&Arc<RwLock<crate::object::Object>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

impl DieModuleInterface for FlightDeckBehavior {
    fn on_die(
        &mut self,
        _damage_info: &crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = FlightDeckBehavior::kill_all_parked_units(self);
        Ok(())
    }
}

/// Get statistics about the flight deck
impl FlightDeckBehavior {
    pub fn get_statistics(&self) -> FlightDeckStatistics {
        let state = self.state.read().unwrap();

        let occupied_spaces = state
            .parking_spaces
            .iter()
            .filter(|space| space.object_id != INVALID_OBJECT_ID)
            .count();

        FlightDeckStatistics {
            total_parking_spaces: state.parking_spaces.len(),
            occupied_spaces,
            available_spaces: state.parking_spaces.len() - occupied_spaces,
            runway_count: state.runways.len(),
            healing_count: state.healing_objects.len(),
        }
    }
}

/// Statistics for the flight deck
#[derive(Debug, Clone)]
pub struct FlightDeckStatistics {
    pub total_parking_spaces: usize,
    pub occupied_spaces: usize,
    pub available_spaces: usize,
    pub runway_count: usize,
    pub healing_count: usize,
}

impl UpdateModuleInterface for FlightDeckBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let current_frame = TheGameLogic::get_frame();
        match FlightDeckBehavior::update(self, current_frame) {
            Ok(sleep) => Ok(sleep),
            Err(_) => Ok(UPDATE_SLEEP_NONE),
        }
    }
}

impl BehaviorModuleInterface for FlightDeckBehavior {
    fn get_module_name(&self) -> &'static str {
        "FlightDeckBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_update_exit_interface(&mut self) -> Option<&mut dyn ModuleExitInterface> {
        Some(self)
    }

    fn get_parking_place_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn SharedParkingPlaceBehaviorInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for FlightDeckBehavior {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("FlightDeckBehavior version xfer failed: {:?}", e))?;

        if xfer.get_xfer_mode() == XferMode::Load {
            let _ = self.build_info(false);
        }

        let mut spaces_count: u8 = self
            .state
            .read()
            .unwrap()
            .parking_spaces
            .len()
            .min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut spaces_count)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Save {
            let state = self.state.read().unwrap();
            for space in state.parking_spaces.iter().take(spaces_count as usize) {
                let mut object_id = space.object_id;
                xfer.xfer_object_id(&mut object_id)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            let mut state = self.state.write().unwrap();
            for index in 0..spaces_count as usize {
                let mut object_id: ObjectID = INVALID_OBJECT_ID;
                xfer.xfer_object_id(&mut object_id)
                    .map_err(|e| e.to_string())?;
                if let Some(space) = state.parking_spaces.get_mut(index) {
                    space.object_id = object_id;
                }
            }
        }

        let mut runways_count: u8 = self
            .state
            .read()
            .unwrap()
            .runways
            .len()
            .min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut runways_count)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Save {
            let state = self.state.read().unwrap();
            for runway in state.runways.iter().take(runways_count as usize) {
                let mut takeoff = runway.in_use_by_for_takeoff;
                let mut landing = runway.in_use_by_for_landing;
                xfer.xfer_object_id(&mut takeoff)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_object_id(&mut landing)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            let mut state = self.state.write().unwrap();
            for index in 0..runways_count as usize {
                let mut takeoff: ObjectID = INVALID_OBJECT_ID;
                let mut landing: ObjectID = INVALID_OBJECT_ID;
                xfer.xfer_object_id(&mut takeoff)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_object_id(&mut landing)
                    .map_err(|e| e.to_string())?;
                if let Some(runway) = state.runways.get_mut(index) {
                    runway.in_use_by_for_takeoff = takeoff;
                    runway.in_use_by_for_landing = landing;
                }
            }
        }

        let mut heal_count: u8 = self
            .state
            .read()
            .unwrap()
            .healing_objects
            .len()
            .min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut heal_count)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Save {
            let state = self.state.read().unwrap();
            for info in state.healing_objects.iter().take(heal_count as usize) {
                let mut healed_id = info.object_id;
                let mut heal_start_frame = info.heal_start_frame;
                xfer.xfer_object_id(&mut healed_id)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_int(&mut heal_start_frame)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            let mut state = self.state.write().unwrap();
            state.healing_objects.clear();
            for _ in 0..heal_count {
                let mut healed_id: ObjectID = INVALID_OBJECT_ID;
                let mut heal_start_frame: UnsignedInt = 0;
                xfer.xfer_object_id(&mut healed_id)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_int(&mut heal_start_frame)
                    .map_err(|e| e.to_string())?;
                state.healing_objects.push_back(HealingInfo {
                    object_id: healed_id,
                    heal_start_frame,
                });
            }
        }

        {
            let mut state = self.state.write().unwrap();
            xfer.xfer_unsigned_int(&mut state.next_heal_frame)
                .map_err(|e| e.to_string())?;
            xfer.xfer_unsigned_int(&mut state.next_cleanup_frame)
                .map_err(|e| e.to_string())?;
            xfer.xfer_unsigned_int(&mut state.started_production_frame)
                .map_err(|e| e.to_string())?;
            xfer.xfer_unsigned_int(&mut state.next_allowed_production_frame)
                .map_err(|e| e.to_string())?;
        }

        xfer.xfer_object_id(&mut self.designated_target)
            .map_err(|e| e.to_string())?;
        let mut command_type = self.designated_command as i32;
        xfer.xfer_int(&mut command_type)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.designated_command = ai_command_type_from_i32(command_type);
        }
        xfer.xfer_coord3d(&mut self.designated_position);

        let mut max_runways: UnsignedInt = MAX_RUNWAYS as UnsignedInt;
        xfer.xfer_unsigned_int(&mut max_runways)
            .map_err(|e| e.to_string())?;
        let mut state = self.state.write().unwrap();
        for i in 0..MAX_RUNWAYS {
            if (max_runways as usize) <= MAX_RUNWAYS {
                xfer.xfer_unsigned_int(&mut state.next_launch_wave_frame[i])
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_int(&mut state.ramp_up_frame[i])
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_int(&mut state.catapult_system_frame[i])
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_int(&mut state.lower_ramp_frame[i])
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut state.ramp_up[i])
                    .map_err(|e| e.to_string())?;
            } else {
                let mut dummy_int: UnsignedInt = 0;
                let mut dummy_bool = false;
                xfer.xfer_unsigned_int(&mut dummy_int)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_int(&mut dummy_int)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_int(&mut dummy_int)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_int(&mut dummy_int)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut dummy_bool).map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.thing_template =
            TheThingFactory::find_template(self.config.thing_template_name.as_str());
        Ok(())
    }
}

/// Glue that exposes FlightDeckBehavior through the common Module trait.
pub struct FlightDeckBehaviorModule {
    behavior: FlightDeckBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<FlightDeckBehaviorModuleData>,
}

impl FlightDeckBehaviorModule {
    pub fn new(
        behavior: FlightDeckBehavior,
        module_name: &AsciiString,
        module_data: Arc<FlightDeckBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut FlightDeckBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for FlightDeckBehaviorModule {
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

impl Module for FlightDeckBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }
}

/// Factory for creating FlightDeckBehavior instances
pub struct FlightDeckBehaviorFactory;

impl FlightDeckBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<crate::object::Object>>,
        module_data: Arc<dyn crate::common::ModuleData>,
    ) -> Result<
        Box<dyn crate::modules::BehaviorModuleInterface>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let data = module_data
            .as_any()
            .downcast_ref::<FlightDeckBehaviorModuleData>()
            .ok_or("Invalid module data type for FlightDeckBehavior")?
            .clone();

        let object_id = thing.read().map(|guard| guard.get_object_id()).unwrap_or(0);
        let behavior = FlightDeckBehavior::new(object_id, data);
        Ok(Box::new(behavior))
    }
}

fn ai_command_type_from_i32(value: i32) -> AiCommandType {
    match value {
        -1 => AiCommandType::NoCommand,
        0 => AiCommandType::MoveToPosition,
        1 => AiCommandType::MoveToObject,
        2 => AiCommandType::TightenToPosition,
        3 => AiCommandType::MoveToPositionAndEvacuate,
        4 => AiCommandType::MoveToPositionAndEvacuateAndExit,
        5 => AiCommandType::Idle,
        6 => AiCommandType::FollowWaypointPath,
        7 => AiCommandType::FollowWaypointPathAsTeam,
        8 => AiCommandType::FollowUserPath,
        9 => AiCommandType::FollowPath,
        10 => AiCommandType::FollowExitProductionPath,
        11 => AiCommandType::AttackObject,
        12 => AiCommandType::ForceAttackObject,
        13 => AiCommandType::AttackTeam,
        14 => AiCommandType::AttackPosition,
        15 => AiCommandType::AttackMoveToPosition,
        16 => AiCommandType::AttackFollowWaypointPath,
        17 => AiCommandType::AttackFollowWaypointPathAsTeam,
        18 => AiCommandType::Hunt,
        19 => AiCommandType::Repair,
        20 => AiCommandType::PickUpPrisoner,
        21 => AiCommandType::ReturnPrisoners,
        22 => AiCommandType::ResumeConstruction,
        23 => AiCommandType::GetHealed,
        24 => AiCommandType::GetRepaired,
        25 => AiCommandType::Enter,
        26 => AiCommandType::Dock,
        27 => AiCommandType::Exit,
        28 => AiCommandType::Evacuate,
        29 => AiCommandType::ExecuteRailedTransport,
        30 => AiCommandType::GoProne,
        31 => AiCommandType::GuardPosition,
        32 => AiCommandType::GuardObject,
        33 => AiCommandType::GuardArea,
        34 => AiCommandType::DeployAssaultReturn,
        35 => AiCommandType::AttackArea,
        36 => AiCommandType::HackInternet,
        37 => AiCommandType::FaceObject,
        38 => AiCommandType::FacePosition,
        39 => AiCommandType::RappelInto,
        40 => AiCommandType::CombatDrop,
        41 => AiCommandType::CommandButtonPos,
        42 => AiCommandType::CommandButtonObj,
        43 => AiCommandType::CommandButton,
        44 => AiCommandType::Wander,
        45 => AiCommandType::WanderInPlace,
        46 => AiCommandType::Panic,
        47 => AiCommandType::Busy,
        48 => AiCommandType::FollowWaypointPathExact,
        49 => AiCommandType::FollowWaypointPathAsTeamExact,
        50 => AiCommandType::MoveAwayFromUnit,
        51 => AiCommandType::FollowPathAppend,
        52 => AiCommandType::MoveToPositionEvenIfSleeping,
        53 => AiCommandType::GuardTunnelNetwork,
        54 => AiCommandType::EvacuateInstantly,
        55 => AiCommandType::ExitInstantly,
        56 => AiCommandType::GuardRetaliate,
        _ => AiCommandType::NoCommand,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::Object;
    use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};
    use std::sync::{Arc, RwLock};

    static NEXT_TEST_OBJECT_ID: AtomicU32 = AtomicU32::new(10_000);

    fn next_test_object_id() -> ObjectID {
        NEXT_TEST_OBJECT_ID.fetch_add(1, AtomicOrdering::Relaxed)
    }

    fn create_test_flight_deck() -> FlightDeckBehavior {
        let object_id = next_test_object_id();

        let config = FlightDeckBehaviorModuleData {
            num_rows: 3,
            num_cols: 2,
            heal_amount: 1.0,
            approach_height: 50.0,
            landing_deck_height_offset: 10.0,
            ..Default::default()
        };

        FlightDeckBehavior::new(object_id, config)
    }

    fn register_test_object(object_id: ObjectID) -> Arc<RwLock<Object>> {
        let object = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        OBJECT_REGISTRY.register_object(object_id, &object);
        object
    }

    fn create_test_flight_deck_with_owner() -> (FlightDeckBehavior, Arc<RwLock<Object>>) {
        let owner_id = next_test_object_id();
        let owner = register_test_object(owner_id);
        let config = FlightDeckBehaviorModuleData {
            num_rows: 3,
            num_cols: 2,
            heal_amount: 1.0,
            approach_height: 50.0,
            landing_deck_height_offset: 10.0,
            ..Default::default()
        };

        let flight_deck = FlightDeckBehavior::new(owner_id, config);
        (flight_deck, owner)
    }

    #[test]
    fn test_flight_deck_creation() {
        let flight_deck = create_test_flight_deck();
        let stats = flight_deck.get_statistics();

        assert_eq!(stats.total_parking_spaces, 6); // 3 * 2
        assert_eq!(stats.runway_count, 2);
        assert_eq!(stats.occupied_spaces, 0);
        assert_eq!(stats.available_spaces, 6);
    }

    #[test]
    fn test_parking_space_reservation() {
        let (mut flight_deck, owner) = create_test_flight_deck_with_owner();
        let mut info = SharedPPInfo::default();
        let jet_id = next_test_object_id();
        let _jet = register_test_object(jet_id);

        let result = flight_deck.reserve_space(jet_id, 0.0, &mut info);
        assert!(result);

        let stats = flight_deck.get_statistics();
        assert_eq!(stats.occupied_spaces, 1);
        assert_eq!(stats.available_spaces, 5);

        OBJECT_REGISTRY.unregister_object(jet_id);
        OBJECT_REGISTRY.unregister_object(owner.read().unwrap().get_id());
    }

    #[test]
    fn test_runway_reservation() {
        let (mut flight_deck, owner) = create_test_flight_deck_with_owner();
        let mut info = SharedPPInfo::default();
        let jet_id = next_test_object_id();
        let _jet = register_test_object(jet_id);
        let _ = flight_deck.reserve_space(jet_id, 0.0, &mut info);

        let result = flight_deck.reserve_runway(jet_id, false); // takeoff
        assert!(result);

        let reservation =
            flight_deck.get_runway_reservation(0, SharedRunwayReservationType::Takeoff);
        assert_eq!(reservation, jet_id);

        OBJECT_REGISTRY.unregister_object(jet_id);
        OBJECT_REGISTRY.unregister_object(owner.read().unwrap().get_id());
    }

    #[test]
    fn test_space_release() {
        let (mut flight_deck, owner) = create_test_flight_deck_with_owner();
        let mut info = SharedPPInfo::default();
        let jet_id = next_test_object_id();
        let _jet = register_test_object(jet_id);

        // Reserve space
        let _ = flight_deck.reserve_space(jet_id, 0.0, &mut info);
        assert!(flight_deck.has_reserved_space(jet_id));

        // Release space
        flight_deck.release_space(jet_id);
        assert!(!flight_deck.has_reserved_space(jet_id));

        let stats = flight_deck.get_statistics();
        assert_eq!(stats.occupied_spaces, 0);
        assert_eq!(stats.available_spaces, 6);

        OBJECT_REGISTRY.unregister_object(jet_id);
        OBJECT_REGISTRY.unregister_object(owner.read().unwrap().get_id());
    }

    #[test]
    fn test_healing() {
        let flight_deck = create_test_flight_deck();

        flight_deck.set_healee(42, true);
        let stats = flight_deck.get_statistics();
        assert_eq!(stats.healing_count, 1);

        flight_deck.set_healee(42, false);
        let stats = flight_deck.get_statistics();
        assert_eq!(stats.healing_count, 0);
    }

    #[test]
    fn test_exit_interface() {
        let mut flight_deck = create_test_flight_deck();

        let door = ModuleExitInterface::reserve_door_for_exit(&mut flight_deck, None, None);
        assert_eq!(door, ModuleExitDoorType::Door1);

        let result = ModuleExitInterface::exit(&mut flight_deck, 42);
        assert!(result);
    }
}
