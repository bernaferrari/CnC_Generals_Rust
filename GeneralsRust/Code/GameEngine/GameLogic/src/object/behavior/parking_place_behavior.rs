//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/ParkingPlaceBehavior.cpp`.
//!
//! Parking Place Behavior Module
//!
//! Handles aircraft parking, runway management, healing, and exit logic for airfields.
//! Manages parking spaces, runways, takeoff/landing coordination, and auto-healing.
//!
//! Originally from: GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/ParkingPlaceBehavior.cpp
//! Original Author: Steven Johnson, June 2002
//! Rust port: 2025

use std::collections::VecDeque;
use std::sync::{Arc, RwLock, Weak};

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use glam::EulerRot;

use crate::ai::THE_AI;
use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, Coord3D, CoordOrigin, Int, KindOf, ModuleData, NameKeyGenerator, ObjectID,
    ObjectStatusMaskType, ObjectStatusTypes, Real, UnsignedInt, LOGICFRAMES_PER_SECOND,
};
use crate::helpers::TheGameLogic;
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, ExitDoorType as ModuleExitDoorType,
    ExitInterface as ModuleExitInterface, UpdateModuleInterface, UpdateSleepTime,
    UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::behavior::behavior_module::{
    ObjectTemplate, PPInfo as BehaviorPPInfo,
    ParkingPlaceBehaviorInterface as ParkingPlaceBehaviorInterfaceTrait,
    RunwayReservationType as BehaviorRunwayReservationType, Team,
};
use crate::object::{Object as GameObject, Object, INVALID_ID as OBJECT_INVALID_ID};

/// Heal rate in frames per second (C++ line 526)
const HEAL_RATE_FRAMES: UnsignedInt = LOGICFRAMES_PER_SECOND / 5;
const FOREVER: UnsignedInt = u32::MAX;

/// Parking place information structure
/// Matches C++ ParkingPlaceInfo
#[derive(Debug, Clone)]
struct ParkingPlaceInfo {
    /// Location for parking
    location: Coord3D,
    /// Orientation at parking spot
    orientation: Real,
    /// Hangar start position
    hangar_start: Coord3D,
    /// Hangar start orientation
    hangar_start_orient: Real,
    /// Prep position before runway
    prep: Coord3D,
    /// Which runway this space belongs to
    runway: Int,
    /// Which door this space uses
    door: u32,
    /// Object currently in this space
    object_in_space: ObjectID,
    /// Reserved for unit exiting production
    reserved_for_exit: Bool,
}

/// Runway information structure
/// Matches C++ RunwayInfo
#[derive(Debug, Clone)]
struct RunwayInfo {
    /// Runway start position
    start: Coord3D,
    /// Runway end position
    end: Coord3D,
    /// Object currently using runway
    in_use_by: ObjectID,
    /// Next object waiting for takeoff
    next_in_line_for_takeoff: ObjectID,
    /// Whether current user was in line
    was_in_line: Bool,
}

/// Healing information structure
/// Matches C++ HealingInfo
#[derive(Debug, Clone)]
struct HealingInfo {
    /// ID of object being healed
    getting_healed_id: ObjectID,
    /// Frame when healing started
    heal_start_frame: UnsignedInt,
}

/// Parking place info for external use
/// Matches C++ PPInfo
#[derive(Debug, Clone)]
pub struct PPInfo {
    pub parking_space: Coord3D,
    pub parking_orientation: Real,
    pub runway_prep: Coord3D,
    pub runway_start: Coord3D,
    pub runway_end: Coord3D,
    pub runway_approach: Coord3D,
    pub runway_exit: Coord3D,
    pub hangar_internal: Coord3D,
    pub hangar_internal_orient: Real,
    pub runway_takeoff_dist: Real,
}

impl Default for PPInfo {
    fn default() -> Self {
        Self {
            parking_space: Coord3D::origin(),
            parking_orientation: 0.0,
            runway_prep: Coord3D::origin(),
            runway_start: Coord3D::origin(),
            runway_end: Coord3D::origin(),
            runway_approach: Coord3D::origin(),
            runway_exit: Coord3D::origin(),
            hangar_internal: Coord3D::origin(),
            hangar_internal_orient: 0.0,
            runway_takeoff_dist: 0.0,
        }
    }
}

/// Module data for parking place behavior
#[derive(Clone, Debug)]
pub struct ParkingPlaceBehaviorModuleData {
    pub base: BehaviorModuleData,
    /// Number of rows of parking spaces
    pub num_rows: Int,
    /// Number of columns (runways)
    pub num_cols: Int,
    /// Whether runways exist
    pub has_runways: Bool,
    /// Approach height for landing
    pub approach_height: Real,
    /// Landing deck height offset
    pub landing_deck_height_offset: Real,
    /// Whether to park in hangars
    pub park_in_hangars: Bool,
    /// Healing amount per second
    pub heal_amount: Real,
}

impl Default for ParkingPlaceBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            num_rows: 0,
            num_cols: 0,
            has_runways: false,
            approach_height: 0.0,
            landing_deck_height_offset: 0.0,
            park_in_hangars: false,
            heal_amount: 0.0,
        }
    }
}

impl Snapshotable for ParkingPlaceBehaviorModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

crate::impl_legacy_module_data_via_base!(ParkingPlaceBehaviorModuleData, base);

impl ParkingPlaceBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, PARKING_PLACE_BEHAVIOR_FIELDS)
    }
}

fn parse_num_rows(
    _ini: &mut INI,
    data: &mut ParkingPlaceBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.num_rows = INI::parse_int(token)?;
    Ok(())
}

fn parse_num_cols(
    _ini: &mut INI,
    data: &mut ParkingPlaceBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.num_cols = INI::parse_int(token)?;
    Ok(())
}

fn parse_approach_height(
    _ini: &mut INI,
    data: &mut ParkingPlaceBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.approach_height = INI::parse_real(token)?;
    Ok(())
}

fn parse_landing_deck_height_offset(
    _ini: &mut INI,
    data: &mut ParkingPlaceBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.landing_deck_height_offset = INI::parse_real(token)?;
    Ok(())
}

fn parse_has_runways(
    _ini: &mut INI,
    data: &mut ParkingPlaceBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.has_runways = INI::parse_bool(token)?;
    Ok(())
}

fn parse_park_in_hangars(
    _ini: &mut INI,
    data: &mut ParkingPlaceBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.park_in_hangars = INI::parse_bool(token)?;
    Ok(())
}

fn parse_heal_amount_per_second(
    _ini: &mut INI,
    data: &mut ParkingPlaceBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.heal_amount = INI::parse_real(token)?;
    Ok(())
}

fn required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

const PARKING_PLACE_BEHAVIOR_FIELDS: &[FieldParse<ParkingPlaceBehaviorModuleData>] = &[
    FieldParse {
        token: "NumRows",
        parse: parse_num_rows,
    },
    FieldParse {
        token: "NumCols",
        parse: parse_num_cols,
    },
    FieldParse {
        token: "ApproachHeight",
        parse: parse_approach_height,
    },
    FieldParse {
        token: "LandingDeckHeightOffset",
        parse: parse_landing_deck_height_offset,
    },
    FieldParse {
        token: "HasRunways",
        parse: parse_has_runways,
    },
    FieldParse {
        token: "ParkInHangars",
        parse: parse_park_in_hangars,
    },
    FieldParse {
        token: "HealAmountPerSecond",
        parse: parse_heal_amount_per_second,
    },
];

/// Parking place behavior module
/// Matches C++ ParkingPlaceBehavior implementation
pub struct ParkingPlaceBehavior {
    /// Weak reference to owning object
    object_id: ObjectID,
    /// Module data
    module_data: Arc<ParkingPlaceBehaviorModuleData>,
    /// Whether parking info has been built
    got_info: Bool,
    /// Inherited UpdateModule scheduler state.
    next_call_frame_and_phase: UnsignedInt,
    /// Parking spaces
    spaces: Vec<ParkingPlaceInfo>,
    /// Runways
    runways: Vec<RunwayInfo>,
    /// Objects being healed
    healing: VecDeque<HealingInfo>,
    /// Helicopter rally point
    heli_rally_point: Coord3D,
    /// Whether heli rally point exists
    heli_rally_point_exists: Bool,
    /// Next frame to run healing update
    next_heal_frame: UnsignedInt,
}

impl ParkingPlaceBehavior {
    /// Create new parking place behavior
    /// Matches C++ ParkingPlaceBehavior::ParkingPlaceBehavior (line 32)
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<ParkingPlaceBehaviorModuleData>()
            .ok_or("Invalid module data")?;

        if let Ok(owner_guard) = object.read() {
            TheGameLogic::set_wake_frame(owner_guard.get_id(), UpdateSleepTime::None);
        }

        Ok(Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data: Arc::new(specific_data.clone()),
            got_info: false,
            next_call_frame_and_phase: 0,
            spaces: Vec::new(),
            runways: Vec::new(),
            healing: VecDeque::new(),
            heli_rally_point: Coord3D::new(0.0, 0.0, 0.0),
            heli_rally_point_exists: false,
            next_heal_frame: FOREVER,
        })
    }

    /// Get module data
    fn get_module_data(&self) -> &ParkingPlaceBehaviorModuleData {
        &self.module_data
    }

    fn set_hold_door_open(&self, door: u32, open: Bool) {
        let Some(owner) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        for behavior in &owner_guard.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };
            if let Some(prod) = behavior_guard.get_production_update_interface() {
                prod.set_hold_door_open(door as usize, open);
                break;
            }
        }
    }

    /// Build parking info from bones
    /// Matches C++ ParkingPlaceBehavior::buildInfo (line 56)
    fn build_info(&mut self) {
        if self.got_info {
            return;
        }

        let owner = match (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            Some(owner) => owner,
            None => return,
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        if owner_guard.test_status(ObjectStatusTypes::UnderConstruction)
            || owner_guard.test_status(ObjectStatusTypes::Sold)
        {
            return;
        }

        let num_rows = self.module_data.num_rows;
        let num_cols = self.module_data.num_cols;
        let has_runways = self.module_data.has_runways;

        self.spaces.reserve((num_rows * num_cols) as usize);

        let mut door = 0;
        for row in 0..num_rows {
            for col in 0..num_cols {
                let hangar_bone = format!("Runway{}Park{}Han", col + 1, row + 1);
                let parking_bone = format!("Runway{}Parking{}", col + 1, row + 1);
                let prep_bone = format!("Runway{}Prep{}", col + 1, row + 1);

                let (_, hangar_start, hangar_transform) =
                    owner_guard.get_single_logical_bone_position(&hangar_bone);
                let (_, location, parking_transform) =
                    owner_guard.get_single_logical_bone_position(&parking_bone);
                let (_, prep, _) = owner_guard.get_single_logical_bone_position(&prep_bone);

                let hangar_start_orient = hangar_transform
                    .to_scale_rotation_translation()
                    .1
                    .to_euler(EulerRot::XYZ)
                    .2;
                let orientation = parking_transform
                    .to_scale_rotation_translation()
                    .1
                    .to_euler(EulerRot::XYZ)
                    .2;

                let info = ParkingPlaceInfo {
                    hangar_start,
                    hangar_start_orient,
                    location,
                    orientation,
                    prep,
                    runway: col,
                    door,
                    object_in_space: OBJECT_INVALID_ID,
                    reserved_for_exit: false,
                };

                door += 1;
                self.spaces.push(info);
            }
        }

        let max_door = door as usize;
        for behavior in &owner_guard.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };
            if let Some(prod) = behavior_guard.get_production_update_interface() {
                for door_index in 0..max_door {
                    prod.set_hold_door_open(door_index, false);
                }
                break;
            }
        }

        if has_runways {
            self.runways.reserve(num_cols as usize);
            for _col in 0..num_cols {
                let col = _col;
                let start_bone = format!("RunwayStart{}", col + 1);
                let end_bone = format!("RunwayEnd{}", col + 1);

                let (_, start, _) = owner_guard.get_single_logical_bone_position(&start_bone);
                let (_, end, _) = owner_guard.get_single_logical_bone_position(&end_bone);

                self.runways.push(RunwayInfo {
                    start,
                    end,
                    in_use_by: OBJECT_INVALID_ID,
                    next_in_line_for_takeoff: OBJECT_INVALID_ID,
                    was_in_line: false,
                });
            }
        }

        self.got_info = true;
    }

    /// Purge dead objects from tracking
    /// Matches C++ ParkingPlaceBehavior::purgeDead (line 128)
    fn purge_dead(&mut self) {
        self.build_info();
        let mut cleared_doors: Vec<u32> = Vec::new();

        for space in &mut self.spaces {
            if space.object_in_space != OBJECT_INVALID_ID {
                let is_dead = TheGameLogic::find_object_by_id(space.object_in_space)
                    .and_then(|obj| obj.read().ok().map(|g| g.is_effectively_dead()))
                    .unwrap_or(true);
                if is_dead {
                    cleared_doors.push(space.door);
                    space.object_in_space = OBJECT_INVALID_ID;
                    space.reserved_for_exit = false;
                }
            }
        }

        if !cleared_doors.is_empty() {
            for door in cleared_doors {
                self.set_hold_door_open(door, false);
            }
        }

        for runway in &mut self.runways {
            if runway.in_use_by != OBJECT_INVALID_ID {
                let is_dead = TheGameLogic::find_object_by_id(runway.in_use_by)
                    .and_then(|obj| obj.read().ok().map(|g| g.is_effectively_dead()))
                    .unwrap_or(true);
                if is_dead {
                    runway.in_use_by = OBJECT_INVALID_ID;
                    runway.was_in_line = false;
                }
            }
            if runway.next_in_line_for_takeoff != OBJECT_INVALID_ID {
                let is_dead = TheGameLogic::find_object_by_id(runway.next_in_line_for_takeoff)
                    .and_then(|obj| obj.read().ok().map(|g| g.is_effectively_dead()))
                    .unwrap_or(true);
                if is_dead {
                    runway.next_in_line_for_takeoff = OBJECT_INVALID_ID;
                }
            }
        }

        // Purge dead from healing list
        let mut purged_healing = false;
        self.healing.retain(|info| {
            if info.getting_healed_id == OBJECT_INVALID_ID {
                purged_healing = true;
                return false;
            }
            let is_dead = TheGameLogic::find_object_by_id(info.getting_healed_id)
                .and_then(|obj| obj.read().ok().map(|g| g.is_effectively_dead()))
                .unwrap_or(true);
            if is_dead {
                purged_healing = true;
            }
            !is_dead
        });
        if purged_healing {
            self.reset_wake_frame();
        }
    }

    /// Check if object has reserved space
    /// Matches C++ ParkingPlaceBehavior::hasReservedSpace (line 198)
    pub fn has_reserved_space(&self, id: ObjectID) -> Bool {
        if !self.got_info || id == OBJECT_INVALID_ID {
            return false;
        }

        self.spaces.iter().any(|s| s.object_in_space == id)
    }

    /// Get space index for object
    /// Matches C++ ParkingPlaceBehavior::getSpaceIndex (line 215)
    pub fn get_space_index(&self, id: ObjectID) -> Int {
        if id == OBJECT_INVALID_ID {
            return -1;
        }

        self.spaces
            .iter()
            .position(|s| s.object_in_space == id)
            .map(|pos| pos as Int)
            .unwrap_or(-1)
    }

    /// Find parking place info for object
    /// Matches C++ ParkingPlaceBehavior::findPPI (line 234)
    #[allow(dead_code)]
    fn find_ppi(&mut self, id: ObjectID) -> Option<&mut ParkingPlaceInfo> {
        if !self.got_info || id == OBJECT_INVALID_ID {
            return None;
        }

        self.spaces.iter_mut().find(|s| s.object_in_space == id)
    }

    /// Find empty parking place
    /// Matches C++ ParkingPlaceBehavior::findEmptyPPI (line 251)
    #[allow(dead_code)]
    fn find_empty_ppi(&mut self) -> Option<&mut ParkingPlaceInfo> {
        if !self.got_info {
            return None;
        }

        self.spaces
            .iter_mut()
            .find(|s| s.object_in_space == OBJECT_INVALID_ID && !s.reserved_for_exit)
    }

    /// Check if has available space for thing template
    /// Matches C++ ParkingPlaceBehavior::hasAvailableSpaceFor (line 277)
    pub fn has_available_space_for(&self, thing_template: &ObjectTemplate) -> Bool {
        if !self.got_info {
            return false;
        }

        if thing_template.is_kind_of(KindOf::ProducedAtHelipad) {
            return true;
        }

        for space in &self.spaces {
            let mut id = space.object_in_space;
            if id != OBJECT_INVALID_ID {
                let is_dead = TheGameLogic::find_object_by_id(id)
                    .and_then(|obj| obj.read().ok().map(|g| g.is_effectively_dead()))
                    .unwrap_or(true);
                if is_dead {
                    id = OBJECT_INVALID_ID;
                }
            }
            if id == OBJECT_INVALID_ID && !space.reserved_for_exit {
                return true;
            }
        }

        false
    }

    /// Reserve space for object
    /// Matches C++ ParkingPlaceBehavior::reserveSpace (line 309)
    pub fn reserve_space(
        &mut self,
        id: ObjectID,
        parking_offset: Real,
        info: Option<&mut PPInfo>,
    ) -> Bool {
        self.build_info();
        self.purge_dead();

        let existing_idx = self.spaces.iter().position(|s| s.object_in_space == id);
        let idx = existing_idx.or_else(|| {
            self.spaces
                .iter()
                .position(|s| s.object_in_space == OBJECT_INVALID_ID && !s.reserved_for_exit)
        });

        let Some(idx) = idx else {
            return false;
        };

        let door = self.spaces[idx].door;
        self.spaces[idx].object_in_space = id;
        self.spaces[idx].reserved_for_exit = false;

        if self.module_data.landing_deck_height_offset != 0.0 {
            if let Some(obj) = TheGameLogic::find_object_by_id(id) {
                if let Ok(mut guard) = obj.write() {
                    guard.set_status(ObjectStatusMaskType::DECK_HEIGHT_OFFSET, true);
                }
            }
        }

        if let Some(info_out) = info {
            self.calc_pp_info(id, info_out);

            if parking_offset != 0.0 {
                let orientation = self.spaces[idx].orientation;
                info_out.parking_space.x += parking_offset * orientation.cos();
                info_out.parking_space.y += parking_offset * orientation.sin();
            }
        }

        self.set_hold_door_open(door, true);

        true
    }

    /// Calculate parking place info
    /// Matches C++ ParkingPlaceBehavior::calcPPInfo (line 357)
    fn calc_pp_info(&self, id: ObjectID, info: &mut PPInfo) {
        let space_opt = self.spaces.iter().find(|s| s.object_in_space == id);

        if let Some(ppi) = space_opt {
            let data = self.get_module_data();

            if let Some(runway) = self.runways.get(ppi.runway as usize) {
                info.parking_space = if data.park_in_hangars {
                    ppi.hangar_start
                } else {
                    ppi.location
                };

                info.runway_prep = ppi.prep;

                info.parking_orientation = if data.park_in_hangars {
                    ppi.hangar_start_orient
                } else {
                    ppi.orientation
                };

                info.runway_start = runway.start;
                info.runway_end = runway.end;
                info.runway_approach = runway.end;

                // Calculate approach position with distance factor
                const APPROACH_DIST: Real = 0.75;
                info.runway_approach.x += (runway.end.x - runway.start.x) * APPROACH_DIST;
                info.runway_approach.y += (runway.end.y - runway.start.y) * APPROACH_DIST;
                info.runway_approach.z =
                    runway.end.z + data.approach_height + data.landing_deck_height_offset;

                info.runway_exit = info.runway_approach;
                info.hangar_internal = ppi.hangar_start;
                info.hangar_internal_orient = ppi.hangar_start_orient;

                // Calculate runway takeoff distance
                let dx = info.runway_start.x - info.runway_end.x;
                let dy = info.runway_start.y - info.runway_end.y;
                let dz = info.runway_start.z - info.runway_end.z;
                info.runway_takeoff_dist = (dx * dx + dy * dy + dz * dz).sqrt();

                // Check if was in line (adjust runway start)
                for rw in &self.runways {
                    if rw.in_use_by == id && rw.was_in_line {
                        info.runway_start = info.runway_prep;
                    }
                }
            }
        }
    }

    /// Release space occupied by object
    /// Matches C++ ParkingPlaceBehavior::releaseSpace (line 402)
    pub fn release_space(&mut self, id: ObjectID) {
        self.build_info();
        self.purge_dead();

        let mut doors_to_close = Vec::new();
        for space in &mut self.spaces {
            if space.object_in_space == id {
                let door = space.door;
                space.object_in_space = OBJECT_INVALID_ID;
                space.reserved_for_exit = false;
                doors_to_close.push(door);
            }
        }
        for door in doors_to_close {
            self.set_hold_door_open(door, false);
        }

        if let Some(obj) = TheGameLogic::find_object_by_id(id) {
            if let Ok(mut guard) = obj.write() {
                guard.clear_status(ObjectStatusMaskType::DECK_HEIGHT_OFFSET);
            }
        }
    }

    /// Reserve runway for takeoff or landing
    /// Matches C++ ParkingPlaceBehavior::reserveRunway (line 453)
    pub fn reserve_runway(&mut self, id: ObjectID, for_landing: Bool) -> Bool {
        self.build_info();
        self.purge_dead();

        // Find runway for this object's parking space
        let runway_idx = self
            .spaces
            .iter()
            .find(|s| s.object_in_space == id)
            .map(|s| s.runway);

        let Some(runway_idx) = runway_idx else {
            return false;
        };

        if let Some(runway) = self.runways.get_mut(runway_idx as usize) {
            if runway.in_use_by == id {
                return true;
            } else if runway.in_use_by == OBJECT_INVALID_ID {
                runway.in_use_by = id;

                if runway.next_in_line_for_takeoff == id {
                    runway.next_in_line_for_takeoff = OBJECT_INVALID_ID;
                    runway.was_in_line = true;
                } else {
                    runway.was_in_line = false;
                }

                return true;
            } else if !for_landing && runway.next_in_line_for_takeoff == OBJECT_INVALID_ID {
                runway.next_in_line_for_takeoff = id;
                return false; // yes, that's right (C++ comment line 498)
            }
        }

        false
    }

    /// Release runway reservation
    /// Matches C++ ParkingPlaceBehavior::releaseRunway (line 505)
    pub fn release_runway(&mut self, id: ObjectID) {
        self.build_info();
        self.purge_dead();

        for runway in &mut self.runways {
            if runway.in_use_by == id {
                runway.in_use_by = OBJECT_INVALID_ID;
                runway.was_in_line = false;
            }
            if runway.next_in_line_for_takeoff == id {
                runway.next_in_line_for_takeoff = OBJECT_INVALID_ID;
            }
        }
    }

    /// Set healee (add or remove from healing list)
    /// Matches C++ ParkingPlaceBehavior::setHealee (line 542)
    pub fn set_healee(&mut self, healee_id: ObjectID, add: Bool) {
        if add {
            // Check if already in list
            if !self
                .healing
                .iter()
                .any(|h| h.getting_healed_id == healee_id)
            {
                let info = HealingInfo {
                    getting_healed_id: healee_id,
                    heal_start_frame: self.get_current_frame(),
                };
                self.healing.push_back(info);
                self.reset_wake_frame();
            }
        } else {
            // Remove from list
            self.healing.retain(|h| h.getting_healed_id != healee_id);
            self.reset_wake_frame();
        }
    }

    /// Reset wake frame based on healing state
    /// Matches C++ ParkingPlaceBehavior::resetWakeFrame (line 529)
    fn reset_wake_frame(&mut self) {
        if self.healing.is_empty() {
            self.next_heal_frame = FOREVER;
        } else {
            self.next_heal_frame = self.get_current_frame() + HEAL_RATE_FRAMES;
        }
    }

    /// Set rally point for helicopters
    /// Matches C++ ParkingPlaceBehavior::setRallyPoint (line 839)
    pub fn set_rally_point(&mut self, pos: &Coord3D) {
        self.heli_rally_point_exists = true;
        self.heli_rally_point = *pos;
    }

    /// Get rally point for helicopters
    /// Matches C++ ParkingPlaceBehavior::getRallyPoint (line 847)
    pub fn get_rally_point(&self) -> Option<&Coord3D> {
        if self.heli_rally_point_exists {
            Some(&self.heli_rally_point)
        } else {
            None
        }
    }

    /// Get current game frame
    fn get_current_frame(&self) -> UnsignedInt {
        TheGameLogic::get_frame()
    }

    /// Kill all parked units
    /// Matches C++ ParkingPlaceBehavior::killAllParkedUnits (line 614)
    pub fn kill_all_parked_units(&mut self) {
        self.build_info();
        self.purge_dead();

        for space in &self.spaces {
            if space.object_in_space != OBJECT_INVALID_ID {
                let Some(obj) = TheGameLogic::find_object_by_id(space.object_in_space) else {
                    continue;
                };
                let Ok(mut guard) = obj.write() else {
                    continue;
                };
                if guard.is_effectively_dead() {
                    continue;
                }

                let takeoff_or_landing = guard
                    .get_ai()
                    .and_then(|ai| {
                        ai.lock()
                            .ok()
                            .map(|ai| ai.is_takeoff_or_landing_in_progress())
                    })
                    .unwrap_or(false);

                if guard.is_above_terrain() && !takeoff_or_landing {
                    continue;
                }

                guard.kill(None, None);
            }
        }

        self.purge_dead();
    }
}

impl UpdateModuleInterface for ParkingPlaceBehavior {
    /// Update callback - handle healing
    /// Matches C++ ParkingPlaceBehavior::update (line 649)
    fn update_simple(&mut self) -> UpdateSleepTime {
        // Keep buildInfo and dead-purged stuff up to date for client to peek at
        self.build_info();
        self.purge_dead();

        let now = self.get_current_frame();
        if now >= self.next_heal_frame {
            self.next_heal_frame = now + HEAL_RATE_FRAMES;
            let heal_amount = self.get_module_data().heal_amount;
            let owner_arc = (if self.object_id == crate::common::INVALID_ID {
                None
            } else {
                crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
            });
            let mut healing = std::mem::take(&mut self.healing);

            healing.retain(|info| {
                if info.getting_healed_id != OBJECT_INVALID_ID {
                    let Some(obj_to_heal) = TheGameLogic::find_object_by_id(info.getting_healed_id)
                    else {
                        return false;
                    };
                    let Ok(mut obj_guard) = obj_to_heal.write() else {
                        return true;
                    };
                    if obj_guard.is_effectively_dead() {
                        return false;
                    }

                    let amount = (HEAL_RATE_FRAMES as f32)
                        * heal_amount
                        * crate::common::SECONDS_PER_LOGICFRAME_REAL;
                    if let Some(owner) = owner_arc.as_ref() {
                        if let Ok(source_guard) = owner.read() {
                            let _ = obj_guard.attempt_healing(amount, Some(&*source_guard));
                        } else {
                            let _ = obj_guard.attempt_healing(amount, None);
                        }
                    } else {
                        let _ = obj_guard.attempt_healing(amount, None);
                    }
                    true
                } else {
                    false
                }
            });
            self.healing = healing;
        }

        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for ParkingPlaceBehavior {
    fn get_module_name(&self) -> &'static str {
        "ParkingPlaceBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_update_exit_interface(&mut self) -> Option<&mut dyn ModuleExitInterface> {
        Some(self)
    }

    fn get_parking_place_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn crate::object::behavior::behavior_module::ParkingPlaceBehaviorInterface>
    {
        Some(self)
    }

    /// On die callback
    /// Matches C++ ParkingPlaceBehavior::onDie (line 643)
    fn on_die(
        &mut self,
        _damage_info: &crate::common::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.kill_all_parked_units();
        Ok(())
    }
}

impl Snapshotable for ParkingPlaceBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 3;
        xfer.xfer_version(&mut version, 3)
            .map_err(|e| format!("ParkingPlaceBehavior version xfer failed: {:?}", e))?;

        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)?;

        let mut spaces_count: u8 = self.spaces.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut spaces_count)
            .map_err(|e| e.to_string())?;
        for space in self.spaces.iter().take(spaces_count as usize) {
            let mut object_id = space.object_in_space;
            let mut reserved_for_exit = space.reserved_for_exit;
            xfer.xfer_object_id(&mut object_id)
                .map_err(|e| e.to_string())?;
            xfer.xfer_bool(&mut reserved_for_exit)
                .map_err(|e| e.to_string())?;
        }

        let mut runways_count: u8 = self.runways.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut runways_count)
            .map_err(|e| e.to_string())?;
        for runway in self.runways.iter().take(runways_count as usize) {
            let mut in_use_by = runway.in_use_by;
            let mut next_in_line = runway.next_in_line_for_takeoff;
            let mut was_in_line = runway.was_in_line;
            xfer.xfer_object_id(&mut in_use_by)
                .map_err(|e| e.to_string())?;
            xfer.xfer_object_id(&mut next_in_line)
                .map_err(|e| e.to_string())?;
            xfer.xfer_bool(&mut was_in_line)
                .map_err(|e| e.to_string())?;
        }

        let mut heal_count: u8 = self.healing.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut heal_count)
            .map_err(|e| e.to_string())?;
        for info in self.healing.iter().take(heal_count as usize) {
            let mut healed_id = info.getting_healed_id;
            let mut heal_start_frame = info.heal_start_frame;
            xfer.xfer_object_id(&mut healed_id)
                .map_err(|e| e.to_string())?;
            xfer.xfer_unsigned_int(&mut heal_start_frame)
                .map_err(|e| e.to_string())?;
        }

        if version >= 2 {
            let mut heli_rally_point = self.heli_rally_point;
            xfer.xfer_coord3d(&mut heli_rally_point);
            let mut heli_rally_point_exists = self.heli_rally_point_exists;
            xfer.xfer_bool(&mut heli_rally_point_exists)
                .map_err(|e| e.to_string())?;
        }

        if version >= 3 {
            let mut next_heal_frame = self.next_heal_frame;
            xfer.xfer_unsigned_int(&mut next_heal_frame)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 3;
        xfer.xfer_version(&mut version, 3)
            .map_err(|e| format!("ParkingPlaceBehavior version xfer failed: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        if xfer.get_xfer_mode() == XferMode::Load {
            self.build_info();
        }

        let mut spaces_count: u8 = self.spaces.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut spaces_count)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for space in self.spaces.iter().take(spaces_count as usize) {
                let mut object_id = space.object_in_space;
                let mut reserved_for_exit = space.reserved_for_exit;
                xfer.xfer_object_id(&mut object_id)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut reserved_for_exit)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            for (index, _) in (0..spaces_count).enumerate() {
                let mut object_id: ObjectID = OBJECT_INVALID_ID;
                let mut reserved_for_exit = false;
                xfer.xfer_object_id(&mut object_id)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut reserved_for_exit)
                    .map_err(|e| e.to_string())?;
                if let Some(space) = self.spaces.get_mut(index) {
                    space.object_in_space = object_id;
                    space.reserved_for_exit = reserved_for_exit;
                }
            }
        }

        let mut runways_count: u8 = self.runways.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut runways_count)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for runway in self.runways.iter().take(runways_count as usize) {
                let mut in_use_by = runway.in_use_by;
                let mut next_in_line = runway.next_in_line_for_takeoff;
                let mut was_in_line = runway.was_in_line;
                xfer.xfer_object_id(&mut in_use_by)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_object_id(&mut next_in_line)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut was_in_line)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            for (index, _) in (0..runways_count).enumerate() {
                let mut in_use_by: ObjectID = OBJECT_INVALID_ID;
                let mut next_in_line: ObjectID = OBJECT_INVALID_ID;
                let mut was_in_line = false;
                xfer.xfer_object_id(&mut in_use_by)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_object_id(&mut next_in_line)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut was_in_line)
                    .map_err(|e| e.to_string())?;
                if let Some(runway) = self.runways.get_mut(index) {
                    runway.in_use_by = in_use_by;
                    runway.next_in_line_for_takeoff = next_in_line;
                    runway.was_in_line = was_in_line;
                }
            }
        }

        let mut heal_count: u8 = self.healing.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut heal_count)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for info in self.healing.iter().take(heal_count as usize) {
                let mut healed_id = info.getting_healed_id;
                let mut heal_start_frame = info.heal_start_frame;
                xfer.xfer_object_id(&mut healed_id)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_int(&mut heal_start_frame)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            self.healing.clear();
            for _ in 0..heal_count {
                let mut healed_id: ObjectID = OBJECT_INVALID_ID;
                let mut heal_start_frame: UnsignedInt = 0;
                xfer.xfer_object_id(&mut healed_id)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_int(&mut heal_start_frame)
                    .map_err(|e| e.to_string())?;
                self.healing.push_back(HealingInfo {
                    getting_healed_id: healed_id,
                    heal_start_frame,
                });
            }
        }

        if version >= 2 {
            xfer.xfer_coord3d(&mut self.heli_rally_point);
            xfer.xfer_bool(&mut self.heli_rally_point_exists)
                .map_err(|e| e.to_string())?;
        }

        if version >= 3 {
            xfer.xfer_unsigned_int(&mut self.next_heal_frame)
                .map_err(|e| e.to_string())?;
        } else if xfer.get_xfer_mode() == XferMode::Load {
            self.next_heal_frame = 0;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes ParkingPlaceBehavior through the common Module trait.
pub struct ParkingPlaceBehaviorModule {
    behavior: ParkingPlaceBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<ParkingPlaceBehaviorModuleData>,
}

impl ParkingPlaceBehaviorModule {
    pub fn new(
        behavior: ParkingPlaceBehavior,
        module_name: &AsciiString,
        module_data: Arc<ParkingPlaceBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut ParkingPlaceBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for ParkingPlaceBehaviorModule {
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

impl Module for ParkingPlaceBehaviorModule {
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

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

impl ParkingPlaceBehavior {
    fn door_index_from_module_exit(door: ModuleExitDoorType) -> Option<usize> {
        match door {
            ModuleExitDoorType::Door1 => Some(0),
            ModuleExitDoorType::Door2 => Some(1),
            ModuleExitDoorType::Door3 => Some(2),
            ModuleExitDoorType::Door4 => Some(3),
            _ => None,
        }
    }

    fn module_exit_from_door_index(index: u32) -> ModuleExitDoorType {
        match index {
            0 => ModuleExitDoorType::Door1,
            1 => ModuleExitDoorType::Door2,
            2 => ModuleExitDoorType::Door3,
            3 => ModuleExitDoorType::Door4,
            _ => ModuleExitDoorType::NoneAvailable,
        }
    }
}

impl ModuleExitInterface for ParkingPlaceBehavior {
    fn can_exit(&self, _object_id: ObjectID) -> bool {
        true
    }

    fn exit(&mut self, _object_id: ObjectID) -> bool {
        true
    }

    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&crate::object::Object>,
        spawn: Option<&crate::object::Object>,
    ) -> ModuleExitDoorType {
        let produced_at_helipad = spawn
            .map(|obj| obj.is_kind_of(crate::common::KindOf::ProducedAtHelipad))
            .unwrap_or(false);
        if produced_at_helipad {
            return ModuleExitDoorType::None;
        }

        self.build_info();
        self.purge_dead();

        if let Some(ppi) = self
            .spaces
            .iter_mut()
            .find(|s| s.object_in_space == OBJECT_INVALID_ID && !s.reserved_for_exit)
        {
            ppi.object_in_space = OBJECT_INVALID_ID;
            ppi.reserved_for_exit = true;
            return Self::module_exit_from_door_index(ppi.door);
        }

        ModuleExitDoorType::NoneAvailable
    }

    fn unreserve_door_for_exit(&mut self, door: ModuleExitDoorType) {
        let Some(index) = Self::door_index_from_module_exit(door) else {
            return;
        };
        self.build_info();
        self.purge_dead();
        for space in &mut self.spaces {
            if space.door as usize == index && space.reserved_for_exit {
                space.object_in_space = OBJECT_INVALID_ID;
                space.reserved_for_exit = false;
                return;
            }
        }
    }

    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(ParkingPlaceBehavior::get_rally_point(self).copied())
    }

    fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        door: ModuleExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.build_info();
        self.purge_dead();

        let mut ppinfo = PPInfo::default();
        let (object_id, produced_at_helipad) = {
            let guard = obj.read().map_err(|_| "object lock poisoned")?;
            (guard.get_id(), guard.is_kind_of(KindOf::ProducedAtHelipad))
        };

        if door != ModuleExitDoorType::None {
            let Some(door_index) = Self::door_index_from_module_exit(door) else {
                return Err("invalid exit door".into());
            };
            let Some(ppi) = self.spaces.iter_mut().find(|s| {
                s.object_in_space == OBJECT_INVALID_ID
                    && s.reserved_for_exit
                    && s.door as usize == door_index
            }) else {
                return Err("exit door reservation not found".into());
            };
            ppi.object_in_space = object_id;
            ppi.reserved_for_exit = false;
        }

        let parking_offset = obj
            .read()
            .ok()
            .and_then(|guard| guard.get_ai())
            .and_then(|ai| ai.lock().ok().map(|ai| ai.get_parking_offset()))
            .unwrap_or(0.0);

        if produced_at_helipad {
            if let Some(owner) = (if self.object_id == crate::common::INVALID_ID {
                None
            } else {
                crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
            }) {
                if let Ok(owner_guard) = owner.read() {
                    let (found, pos, transform) =
                        owner_guard.get_single_logical_bone_position("HeliPark01");
                    if found {
                        let rotation = transform.to_scale_rotation_translation().1;
                        let orient = rotation.to_euler(EulerRot::XYZ).2;
                        ppinfo.hangar_internal = pos;
                        ppinfo.hangar_internal_orient = orient;
                        ppinfo.parking_space = pos;
                        ppinfo.parking_orientation = orient;
                    } else {
                        ppinfo.hangar_internal = *owner_guard.get_position();
                        ppinfo.hangar_internal_orient = owner_guard.get_orientation();
                        ppinfo.parking_space = ppinfo.hangar_internal;
                        ppinfo.parking_orientation = ppinfo.hangar_internal_orient;
                    }
                }
            }
        } else if !self.reserve_space(object_id, parking_offset, Some(&mut ppinfo)) {
            if let Some(owner) = (if self.object_id == crate::common::INVALID_ID {
                None
            } else {
                crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
            }) {
                if let Ok(owner_guard) = owner.read() {
                    ppinfo.parking_space = *owner_guard.get_position();
                    ppinfo.parking_orientation = owner_guard.get_orientation();
                    ppinfo.hangar_internal = ppinfo.parking_space;
                    ppinfo.hangar_internal_orient = ppinfo.parking_orientation;
                }
            }
        }

        {
            let mut guard = obj.write().map_err(|_| "object lock poisoned")?;
            let _ = guard.set_position(&ppinfo.hangar_internal);
            let _ = guard.set_orientation(ppinfo.hangar_internal_orient);
        }
        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pf_arc) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pf_arc.write() {
                    pf.add_object_to_map(object_id, &[ppinfo.hangar_internal], false);
                }
            }
        }

        if let Ok(guard) = obj.read() {
            if let Some(ai) = guard.get_ai() {
                let owner_id = (if self.object_id == crate::common::INVALID_ID {
                    None
                } else {
                    crate::helpers::TheGameLogic::find_object_by_id(self.object_id).or_else(|| {
                        crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id)
                    })
                })
                .and_then(|o| o.read().ok().map(|g| g.get_id()))
                .unwrap_or(OBJECT_INVALID_ID);
                if produced_at_helipad {
                    if let Some(rally_point) = self.get_rally_point() {
                        ai.ai_move_to_position(
                            rally_point,
                            false,
                            crate::ai::CommandSourceType::FromAi,
                        );
                    } else {
                        ai.ai_move_to_position(
                            &ppinfo.parking_space,
                            false,
                            crate::ai::CommandSourceType::FromAi,
                        );
                    }
                } else {
                    let mut exit_path = Vec::with_capacity(1);
                    exit_path.push(ppinfo.parking_space);
                    ai.ai_follow_exit_production_path(
                        &exit_path,
                        Some(owner_id),
                        crate::ai::CommandSourceType::FromAi,
                    );
                }
            }
        }

        Ok(())
    }
}

impl ParkingPlaceBehavior {
    fn fill_behavior_pp_info(&self, id: ObjectID, info: &mut BehaviorPPInfo) {
        let space_opt = self.spaces.iter().find(|s| s.object_in_space == id);

        if let Some(ppi) = space_opt {
            let data = self.get_module_data();

            if let Some(runway) = self.runways.get(ppi.runway as usize) {
                info.parking_space = if data.park_in_hangars {
                    ppi.hangar_start
                } else {
                    ppi.location
                };

                info.runway_prep = ppi.prep;

                info.parking_orientation = if data.park_in_hangars {
                    ppi.hangar_start_orient
                } else {
                    ppi.orientation
                };

                info.runway_start = runway.start;
                info.runway_end = runway.end;
                info.runway_landing_start = runway.start;
                info.runway_landing_end = runway.end;
                info.runway_approach = runway.end;

                // Calculate approach position with distance factor.
                const APPROACH_DIST: Real = 0.75;
                info.runway_approach.x += (runway.end.x - runway.start.x) * APPROACH_DIST;
                info.runway_approach.y += (runway.end.y - runway.start.y) * APPROACH_DIST;
                info.runway_approach.z =
                    runway.end.z + data.approach_height + data.landing_deck_height_offset;

                info.runway_exit = info.runway_approach;
                info.hangar_internal = ppi.hangar_start;
                info.hangar_internal_orient = ppi.hangar_start_orient;

                let dx = info.runway_start.x - info.runway_end.x;
                let dy = info.runway_start.y - info.runway_end.y;
                let dz = info.runway_start.z - info.runway_end.z;
                info.runway_takeoff_dist = (dx * dx + dy * dy + dz * dz).sqrt();

                for rw in &self.runways {
                    if rw.in_use_by == id && rw.was_in_line {
                        info.runway_start = info.runway_prep;
                        info.runway_landing_start = info.runway_prep;
                    }
                }
            }
        }
    }
}

impl ParkingPlaceBehaviorInterfaceTrait for ParkingPlaceBehavior {
    fn should_reserve_door_when_queued(&self, thing_template: &ObjectTemplate) -> Bool {
        !thing_template.is_kind_of(KindOf::ProducedAtHelipad)
    }

    fn has_available_space_for(&self, thing_template: &ObjectTemplate) -> Bool {
        ParkingPlaceBehavior::has_available_space_for(self, thing_template)
    }

    fn has_reserved_space(&self, id: ObjectID) -> Bool {
        ParkingPlaceBehavior::has_reserved_space(self, id)
    }

    fn get_space_index(&self, id: ObjectID) -> Int {
        ParkingPlaceBehavior::get_space_index(self, id)
    }

    fn reserve_space(
        &mut self,
        id: ObjectID,
        parking_offset: Real,
        info: &mut BehaviorPPInfo,
    ) -> Bool {
        let mut local = PPInfo {
            parking_space: Coord3D::origin(),
            parking_orientation: 0.0,
            runway_prep: Coord3D::origin(),
            runway_start: Coord3D::origin(),
            runway_end: Coord3D::origin(),
            runway_approach: Coord3D::origin(),
            runway_exit: Coord3D::origin(),
            hangar_internal: Coord3D::origin(),
            hangar_internal_orient: 0.0,
            runway_takeoff_dist: 0.0,
        };

        let reserved =
            ParkingPlaceBehavior::reserve_space(self, id, parking_offset, Some(&mut local));
        if reserved {
            info.parking_space = local.parking_space;
            info.parking_orientation = local.parking_orientation;
            info.runway_prep = local.runway_prep;
            info.runway_start = local.runway_start;
            info.runway_end = local.runway_end;
            info.runway_landing_start = local.runway_start;
            info.runway_landing_end = local.runway_end;
            info.runway_approach = local.runway_approach;
            info.runway_exit = local.runway_exit;
            info.hangar_internal = local.hangar_internal;
            info.hangar_internal_orient = local.hangar_internal_orient;
            info.runway_takeoff_dist = local.runway_takeoff_dist;
        }

        reserved
    }

    fn release_space(&mut self, id: ObjectID) {
        ParkingPlaceBehavior::release_space(self, id);
    }

    fn reserve_runway(&mut self, id: ObjectID, for_landing: Bool) -> Bool {
        ParkingPlaceBehavior::reserve_runway(self, id, for_landing)
    }

    fn calc_pp_info(&self, id: ObjectID, info: &mut BehaviorPPInfo) {
        self.fill_behavior_pp_info(id, info);
    }

    fn release_runway(&mut self, id: ObjectID) {
        ParkingPlaceBehavior::release_runway(self, id);
    }

    fn get_runway_count(&self) -> Int {
        self.runways.len() as Int
    }

    fn get_runway_reservation(
        &self,
        r: Int,
        reservation_type: BehaviorRunwayReservationType,
    ) -> ObjectID {
        let runway = self.runways.get(r as usize);
        let Some(runway) = runway else {
            return OBJECT_INVALID_ID;
        };

        let _ = reservation_type;
        runway.in_use_by
    }

    fn transfer_runway_reservation_to_next_in_line_for_takeoff(&mut self, id: ObjectID) {
        for runway in &mut self.runways {
            if runway.in_use_by == id && runway.next_in_line_for_takeoff != OBJECT_INVALID_ID {
                runway.in_use_by = runway.next_in_line_for_takeoff;
                runway.was_in_line = true;
                runway.next_in_line_for_takeoff = OBJECT_INVALID_ID;
            }
        }
    }

    fn get_approach_height(&self) -> Real {
        self.module_data.approach_height
    }

    fn get_landing_deck_height_offset(&self) -> Real {
        self.module_data.landing_deck_height_offset
    }

    fn set_healee(&mut self, healee: Option<ObjectID>, add: Bool) {
        if let Some(healee_id) = healee {
            if healee_id != OBJECT_INVALID_ID {
                ParkingPlaceBehavior::set_healee(self, healee_id, add);
            }
        }
    }

    fn kill_all_parked_units(&mut self) {
        ParkingPlaceBehavior::kill_all_parked_units(self);
    }

    fn defect_all_parked_units(
        &mut self,
        new_team: Arc<RwLock<Team>>,
        detection_time: UnsignedInt,
    ) {
        self.build_info();
        self.purge_dead();

        let parked_ids: Vec<ObjectID> = self
            .spaces
            .iter()
            .filter_map(|space| {
                if space.object_in_space == OBJECT_INVALID_ID {
                    None
                } else {
                    Some(space.object_in_space)
                }
            })
            .collect();

        for object_id in parked_ids {
            let owner_id = (if self.object_id == crate::common::INVALID_ID {
                None
            } else {
                crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
            })
            .and_then(|owner| owner.read().ok().map(|g| g.get_id()));
            let new_team_player_id = new_team
                .read()
                .ok()
                .and_then(|team| team.get_controlling_player_id());
            let Some(should_release) = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(object_id, |guard| {
                    if guard.is_effectively_dead() {
                        return None;
                    }

                    let takeoff_or_landing = guard
                        .get_ai()
                        .and_then(|ai| {
                            ai.lock()
                                .ok()
                                .map(|ai| ai.is_takeoff_or_landing_in_progress())
                        })
                        .unwrap_or(false);

                    if guard.is_above_terrain() && !takeoff_or_landing {
                        let obj_player_id = guard.get_controlling_player_id();
                        if new_team_player_id != obj_player_id {
                            if let Some(oid) = owner_id {
                                if guard.get_producer_id() == oid {
                                    guard.set_producer(None);
                                }
                            }
                            return Some(true);
                        }
                        Some(false)
                    } else {
                        guard.defect(Some(new_team.clone()), detection_time);
                        Some(false)
                    }
                })
                .flatten()
            else {
                continue;
            };
            if should_release {
                self.release_space(object_id);
            }
        }

        self.purge_dead();
    }

    fn calc_best_parking_assignment(
        &mut self,
        id: ObjectID,
        pos: &mut Coord3D,
        _old_index: Option<&mut Int>,
        new_index: Option<&mut Int>,
    ) -> Bool {
        let _ = id;
        let _ = pos;
        let _ = new_index;
        false
    }

    fn get_taxi_locations(&self, _id: ObjectID) -> Option<&Vec<Coord3D>> {
        None
    }

    fn get_creation_locations(&self, _id: ObjectID) -> Option<&Vec<Coord3D>> {
        None
    }
}

/// Factory for creating parking place behaviors
pub struct ParkingPlaceBehaviorFactory;

impl ParkingPlaceBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(ParkingPlaceBehavior::new(thing, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> ParkingPlaceBehaviorModuleData {
        ParkingPlaceBehaviorModuleData {
            base: BehaviorModuleData::default(),
            num_rows: 0,
            num_cols: 0,
            has_runways: false,
            approach_height: 0.0,
            landing_deck_height_offset: 0.0,
            park_in_hangars: false,
            heal_amount: 0.0,
        }
    }

    #[test]
    fn test_module_data_defaults() {
        let data = ParkingPlaceBehaviorModuleData::default();
        assert_eq!(data.num_rows, 0);
        assert_eq!(data.num_cols, 0);
        assert!(!data.has_runways);
        assert_eq!(data.heal_amount, 0.0);
    }

    #[test]
    fn parking_place_fields_use_cpp_ini_token_handling() {
        let mut ini = INI::new();
        let mut data = ParkingPlaceBehaviorModuleData::default();

        parse_num_rows(&mut ini, &mut data, &["=", "2"]).unwrap();
        parse_num_cols(&mut ini, &mut data, &["=", "4"]).unwrap();
        parse_approach_height(&mut ini, &mut data, &["=", "75.5"]).unwrap();
        parse_landing_deck_height_offset(&mut ini, &mut data, &["=", "6.25"]).unwrap();
        parse_has_runways(&mut ini, &mut data, &["=", "yes"]).unwrap();
        parse_park_in_hangars(&mut ini, &mut data, &["=", "true"]).unwrap();
        parse_heal_amount_per_second(&mut ini, &mut data, &["=", "12.5"]).unwrap();

        assert_eq!(data.num_rows, 2);
        assert_eq!(data.num_cols, 4);
        assert_eq!(data.approach_height, 75.5);
        assert_eq!(data.landing_deck_height_offset, 6.25);
        assert!(data.has_runways);
        assert!(data.park_in_hangars);
        assert_eq!(data.heal_amount, 12.5);
    }

    #[test]
    fn parking_place_rejects_missing_values_like_cpp_parsers() {
        let mut ini = INI::new();
        let mut data = ParkingPlaceBehaviorModuleData::default();

        assert!(matches!(
            parse_num_rows(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
        assert!(matches!(
            parse_approach_height(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
        assert!(matches!(
            parse_has_runways(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
    }
}
