//! Chinook AI: one runtime owner, with private files for each behavior.
//!
//! Definitions/INI live in `module_data`; flight transitions in `flight_states`;
//! ropes and rappellers in `combat_drop`; command dispatch in `commands`;
//! supply queries in `supply`; ticks and rotor wash in `update`; Xfer in `snapshot`.
//!
//! Ported from GameLogic/Module/ChinookAIUpdate.h and
//! GameLogic/Object/Update/AIUpdate/ChinookAIUpdate.cpp.

use std::sync::{Arc, RwLock};

use crate::ai::AiCommandParams;
use crate::common::{
    Coord3D, DrawableID, INVALID_ID, KindOf, Matrix3D, ObjectID, Real, UnsignedInt,
};
use crate::object::Object;
use crate::object::drawable::Drawable;
use crate::supply_system::SupplyTruckAIUpdate;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

mod combat_drop;
mod commands;
mod flight_states;
mod module_data;
mod snapshot;
mod supply;
mod update;

#[cfg(test)]
mod tests;

pub use module_data::{ChinookAIUpdateData, ChinookAIUpdateModuleData};

/// Wave 349: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

const AUTO_ACQUIRE_ENEMIES_NAMES: &[&str] = &[
    "YES",
    "STEALTHED",
    "NO",
    "NOTWHILEATTACKING",
    "ATTACK_BUILDINGS",
];
const INVALID_DRAWABLE_ID: DrawableID = 0;

/// Chinook flight status (matches C++ ChinookFlightStatus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChinookFlightStatus {
    TakingOff = 0,
    Flying = 1,
    DoingCombatDrop = 2,
    Landing = 3,
    Landed = 4,
}

/// C++ `ChinookAIStateType` (numerically distinct from `AIStateType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ChinookAIState {
    #[default]
    None,
    TakingOff,
    Landing,
    MoveToAndLand,
    MoveToAndEvac,
    LandAndEvac,
    EvacAndTakeoff,
    MoveToAndEvacAndExitInit,
    MoveToAndEvacAndExit,
    LandAndEvacAndExit,
    EvacAndExit,
    TakeoffAndExit,
    HeadOffMap,
    MoveToCombatDrop,
    DoCombatDrop,
}

/// C++ `ChinookTakeoffOrLandingState` / `ChinookMoveToBldgState` 3-unit threshold.
const CHINOOK_ARRIVE_THRESH: Real = 3.0;
const CHINOOK_ARRIVE_THRESH_SQR: Real = CHINOOK_ARRIVE_THRESH * CHINOOK_ARRIVE_THRESH;
/// C++ `BIGNUM` used to restore lift after takeoff/landing.
const CHINOOK_BIGNUM: Real = 99999.0;

fn chinook_dist_sqr(a: &Coord3D, b: &Coord3D) -> Real {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    dx * dx + dy * dy + dz * dz
}

/// C++ `while (ai->loseOneBox())` crate-visual dump (Drawable::updateDrawableSupplyStatus).
fn chinook_dump_owner_crate_visuals(owner: &Object, max_boxes: i32) {
    let Some(drawable) = owner.get_drawable() else {
        return;
    };
    if let Ok(mut draw_guard) = drawable.try_write() {
        draw_guard.update_supply_status(max_boxes, 0);
    }
}

/// C++ `getObject()->isKindOf(KINDOF_CAN_ATTACK)` — not `OBJECT_STATUS_CAN_ATTACK`.
fn chinook_kind_of_can_attack(owner: &Object) -> bool {
    chinook_attack_allowed_by_kind_of(owner.is_kind_of(KindOf::CanAttack))
}

/// C++ passenger follow: only riders with `getCurrentVictim()==NULL`.
fn chinook_passenger_should_follow_attack(passenger_has_victim: bool) -> bool {
    !passenger_has_victim
}

/// C++ `ChinookAIUpdate.cpp:1408` — KindOf CAN_ATTACK, never garrison ObjectStatus.
pub fn chinook_attack_allowed_by_kind_of(kind_of_can_attack: bool) -> bool {
    kind_of_can_attack
}

/// C++ `ChinookAIUpdate.cpp:1070-1082` idle + want enter/exit + not landed → LANDING.
pub fn chinook_should_auto_land(parent_idle: bool, waiting: bool, landed: bool) -> bool {
    parent_idle && waiting && !landed
}

/// C++ `ChinookAIUpdate.cpp:1084-1086` idle + empty want + landed + no heal pad → TAKING_OFF.
pub fn chinook_should_auto_takeoff(
    parent_idle: bool,
    waiting: bool,
    landed: bool,
    healing_airfield: bool,
) -> bool {
    parent_idle && !waiting && landed && !healing_airfield
}

/// C++ `ChinookAIUpdate.cpp:1022-1028` `getAiFreeToExit`.
pub fn chinook_free_to_exit(
    landed: bool,
    doing_combat_drop: bool,
    exiter_can_rappel: bool,
) -> bool {
    landed || (doing_combat_drop && exiter_can_rappel)
}

/// C++ `ChinookMoveToBldgState::onEnter` preferred-height raise.
pub fn chinook_move_to_bldg_preferred_height(
    old_preferred: Real,
    live_structure: bool,
    geom_max_height_above: Real,
    min_drop_height: Real,
) -> Real {
    if !live_structure {
        return old_preferred;
    }
    let raised = geom_max_height_above + min_drop_height;
    if raised < old_preferred {
        old_preferred
    } else {
        raised
    }
}

/// C++ `ChinookMoveToBldgState::update` — 2D arrival **and** `|z-destZ|<=3`.
pub fn chinook_move_to_bldg_arrived(arrived_2d: bool, z: Real, dest_z: Real) -> bool {
    arrived_2d && (z - dest_z).abs() <= CHINOOK_ARRIVE_THRESH
}

/// C++ `aiDoCommand` evac: take off first when landed and dest farther than 3.
pub fn chinook_evac_needs_takeoff_first(landed: bool, dist_sqr: Real) -> bool {
    landed && dist_sqr > CHINOOK_ARRIVE_THRESH_SQR
}

/// C++ `MOVE_TO_AND_EVAC` → `LAND_AND_EVAC` → `EVAC_AND_TAKEOFF` → `TAKING_OFF`.
pub fn chinook_evac_pipeline() -> [&'static str; 4] {
    [
        "MoveToAndEvac",
        "LandAndEvac",
        "EvacAndTakeoff",
        "TakingOff",
    ]
}

/// C++ evac-and-exit init → move → land → dump → takeoff → `HEAD_OFF_MAP`.
pub fn chinook_evac_and_exit_pipeline() -> [&'static str; 6] {
    [
        "MoveToAndEvacAndExitInit",
        "MoveToAndEvacAndExit",
        "LandAndEvacAndExit",
        "EvacAndExit",
        "TakeoffAndExit",
        "HeadOffMap",
    ]
}

#[derive(Debug, Clone)]
struct RopeInfo {
    rope_drawable: Option<Arc<RwLock<Drawable>>>,
    rope_drawable_id: DrawableID,
    drop_start_mtx: Matrix3D,
    rope_speed: Real,
    rope_len: Real,
    rope_len_max: Real,
    next_drop_time: UnsignedInt,
    rappeller_ids: Vec<ObjectID>,
}

#[derive(Debug, Default)]
struct ChinookCombatDropState {
    ropes: Vec<RopeInfo>,
}

/// Chinook AI Update module (matches C++ ChinookAIUpdate).
#[derive(Debug)]
pub struct ChinookAIUpdate {
    data: ChinookAIUpdateData,
    base: SupplyTruckAIUpdate,
    object_id: ObjectID,
    flight_status: ChinookFlightStatus,
    airfield_for_healing: ObjectID,
    original_pos: Coord3D,
    pending_command: Option<AiCommandParams>,
    combat_drop_started: bool,
    combat_drop_target: Option<ObjectID>,
    combat_drop_pos: Coord3D,
    combat_drop_state: Option<ChinookCombatDropState>,
    machine_state: ChinookAIState,
    goal_object: Option<ObjectID>,
    goal_pos: Coord3D,
    takeoff_landing_dest: Coord3D,
    takeoff_landing_is_landing: bool,
    move_to_bldg_old_preferred: Real,
    move_to_bldg_new_preferred: Real,
    move_to_bldg_dest_z: Real,
}

impl ChinookAIUpdate {
    pub fn new(data: ChinookAIUpdateData, object_id: ObjectID, player_index: i32) -> Self {
        let base = SupplyTruckAIUpdate::new(data.supply.clone(), object_id, player_index as u32);
        Self {
            data,
            base,
            object_id,
            flight_status: ChinookFlightStatus::Flying,
            airfield_for_healing: INVALID_ID,
            original_pos: Coord3D::ZERO,
            pending_command: None,
            combat_drop_started: false,
            combat_drop_target: None,
            combat_drop_pos: Coord3D::ZERO,
            combat_drop_state: None,
            machine_state: ChinookAIState::None,
            goal_object: None,
            goal_pos: Coord3D::ZERO,
            takeoff_landing_dest: Coord3D::ZERO,
            takeoff_landing_is_landing: false,
            move_to_bldg_old_preferred: 0.0,
            move_to_bldg_new_preferred: 0.0,
            move_to_bldg_dest_z: 0.0,
        }
    }

    pub fn record_original_position(&mut self, pos: Coord3D) {
        self.original_pos = pos;
    }

    pub fn get_original_position(&self) -> Coord3D {
        self.original_pos
    }
}

/// Module wrapper for ChinookAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct ChinookAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<ChinookAIUpdateModuleData>,
}

impl ChinookAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<ChinookAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

impl Module for ChinookAIUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for ChinookAIUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Arc::make_mut(&mut self.data).xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
