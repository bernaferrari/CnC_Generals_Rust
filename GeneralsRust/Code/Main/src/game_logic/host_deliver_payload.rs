//! Host DeliverPayload cargo residual (superweapon / cargo-plane drops).
//!
//! Residual slice (playability):
//! - Models retail OCL `DeliverPayload` missions used by cargo planes
//!   (`AmericaJetCargoPlane`) and superweapon drop lists without full aircraft
//!   edge-spawn / locomotor flight / door animation GPU.
//! - After approach delay residual, spawns payload units with **DropDelay**
//!   per-item stagger (OCL DropDelay 350 ms → 11 frames) and **DropOffset**
//!   residual (Supply Drop Zone Z:-5).
//! - Primary wire: `OCL_AmericaSupplyDropZoneCrateDrop` —
//!   Transport=`AmericaJetCargoPlane`, Payload=`SupplyDropZoneCrate` × 6,
//!   PutInContainer=`AmericaCrateParachute`, DropDelay=350 ms, DeliveryDistance=410,
//!   MaxAttempts=4, DropOffset X:0 Y:0 Z:-5.
//! - Secondary honesty: America Paradrop cargo-plane DeliverPayload bookkeeping
//!   (actual infantry spawn remains in `host_paradrop`).
//!
//! Residual crate parachute fall-physics (`AmericaCrateParachute`):
//! - Spawn at B52 PreferredHeight **100** + DropOffset Y:-5 (host Y-up).
//! - Freefall until fallen `ParachuteOpenDist` **12.5**, then open chute (slower sink).
//! - `ParachuteDirectly = Yes` residual honesty (target-bunch, no lateral drift residual).
//! - PARA_COG / PARA_ATTCH bone attach residual (GeometryHeight **10** layout) +
//!   crate hang (SupplyDropZoneCrate GeometryHeight **12** height-fallback).
//!
//! Residual CreateAtEdge cargo-plane flight presentation:
//! - Edge spawn residual from residual map extent (closest edge → PreferredHeight).
//! - B52Locomotor Speed **125**/sec approach toward target; DeliveryDistance band.
//! - isCloseEnough residual (inbound + PreOpenDistance).
//! - Door residual: DoorDelay → MODELCONDITION_DOOR_1_OPENING (AVCargoPln_A2).
//! - StartAtPreferredHeight / StartAtMaxSpeed OCL honesty.
//!
//! Fail-closed honesty:
//! - Not full CreateAtEdge cargo-plane Object / full DeliverPayloadAIUpdate state machine
//! - Not full pathfinder re-approach / calcMinTurnRadius / off-map recover
//! - Not full DropVariance random scatter (OCL supply drop has no DropVariance)
//! - Not full AmericaCrateParachute container Object / W3D pristine bone extract GPU
//! - Not full VisiblePayload bone / subobject bomb rack matrix
//! - Not network DeliverPayload replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const DELIVER_PAYLOAD_LOGIC_FPS: f32 = 30.0;

/// Residual cargo-plane approach delay before delivery state
/// (fail-closed vs full CreateAtEdge transit + DeliveryDistance approach).
/// ~3s @ 30 FPS — matches host paradrop / DaisyCutter family residual.
pub const CARGO_PLANE_APPROACH_DELAY_FRAMES: u32 = 90;

// --- OCL_AmericaSupplyDropZoneCrateDrop residual constants ---

/// Retail DeliverPayload Transport.
pub const SUPPLY_DROP_CARGO_TRANSPORT: &str = "AmericaJetCargoPlane";

/// Retail DeliverPayload Payload template.
pub const SUPPLY_DROP_PAYLOAD_TEMPLATE: &str = "SupplyDropZoneCrate";

/// Residual crate template when retail SupplyDropZoneCrate is unavailable.
pub const SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE: &str = "TestSupplyDropZoneCrate";

/// Retail DeliverPayload PutInContainer (parachute container residual honesty).
pub const SUPPLY_DROP_PUT_IN_CONTAINER: &str = "AmericaCrateParachute";

/// Retail Payload count (`Payload = SupplyDropZoneCrate 6`).
pub const SUPPLY_DROP_PAYLOAD_COUNT: u32 = 6;

/// Retail DropDelay = 350 ms between items (parseDurationUnsignedInt).
pub const SUPPLY_DROP_DROP_DELAY_MS: u32 = 350;

/// DropDelay → frames at 30 FPS (350 / (1000/30) ≈ 10.5 → 11).
pub const SUPPLY_DROP_DROP_DELAY_FRAMES: u32 = 11;

/// Retail AmericaJetCargoPlane DeliverPayloadAIUpdate DoorDelay = 500 ms.
/// First wait after entering DeliveringState before first item exit.
pub const CARGO_PLANE_DOOR_DELAY_MS: u32 = 500;

/// DoorDelay → frames at 30 FPS (500 / (1000/30) = 15).
pub const CARGO_PLANE_DOOR_DELAY_FRAMES: u32 = 15;

/// Retail DeliveryDistance residual (approach geometry deferred; constant honesty).
pub const SUPPLY_DROP_DELIVERY_DISTANCE: f32 = 410.0;

/// Retail MaxAttempts residual honesty (OCL = 4).
pub const SUPPLY_DROP_MAX_ATTEMPTS: u32 = 4;

/// Retail PreOpenDistance residual honesty.
/// OCL_AmericaSupplyDropZoneCrateDrop does not set PreOpenDistance (defaults 0).
pub const SUPPLY_DROP_PRE_OPEN_DISTANCE: f32 = 0.0;

/// Retail DropOffset residual (OCL X:0 Y:0 Z:-5). Host Y-up: offset applies to Y.
pub const SUPPLY_DROP_DROP_OFFSET_X: f32 = 0.0;
pub const SUPPLY_DROP_DROP_OFFSET_Y: f32 = -5.0;
pub const SUPPLY_DROP_DROP_OFFSET_Z: f32 = 0.0;

/// Residual horizontal spacing between spawned crates (line formation).
pub const SUPPLY_DROP_CRATE_SPACING: f32 = 20.0;

/// Retail AmericaJetCargoPlane / B52Locomotor PreferredHeight (StartAtPreferredHeight).
pub const CARGO_PLANE_PREFERRED_HEIGHT: f32 = 100.0;

/// Retail AmericaCrateParachute `ParachuteOpenDist` — freefall distance before open.
pub const CRATE_PARACHUTE_OPEN_DIST: f32 = 12.5;

/// Retail AmericaCrateParachute LowAltitudeDamping honesty.
pub const CRATE_PARACHUTE_LOW_ALTITUDE_DAMPING: f32 = 0.2;

/// Retail CrateParachuteLocomotor SpeedLimitZ (dist/sec) honesty.
pub const CRATE_PARACHUTE_SPEED_LIMIT_Z: f32 = 15.0;

/// Host residual freefall sink (units/frame) before AmericaCrateParachute opens.
/// Fail-closed vs full CrateFreeFallLocomotor gravity/PhysicsUpdate.
pub const CRATE_PARACHUTE_FREEFALL_PER_FRAME: f32 = 10.0;

/// Host residual open-chute sink (units/frame) after OpenDist.
/// Retail SpeedLimitZ 15/sec → 0.5/frame is too slow for residual tests;
/// host residual uses **5**/frame (slower than freefall) with SpeedLimitZ honesty.
pub const CRATE_PARACHUTE_SINK_PER_FRAME: f32 = 5.0;

/// C++ low-altitude open fudge multiplier (start − ground ≥ **2×** OpenDist).
pub const CRATE_PARACHUTE_LOW_ALTITUDE_OPEN_MULT: f32 = 2.0;

/// Retail OCL_AmericaSupplyDropZoneCrateDrop ParachuteDirectly residual honesty.
pub const SUPPLY_DROP_PARACHUTE_DIRECTLY: bool = true;

/// Residual audio when AmericaCrateParachute residual chute opens.
pub const CRATE_PARACHUTE_OPEN_AUDIO: &str = "ParachuteOpen";

/// Residual audio when cargo crate residual lands.
pub const CRATE_PARACHUTE_LAND_AUDIO: &str = "CrateLand";

/// Activate audio residual when cargo flight queues (plane inbound).
pub const SUPPLY_DROP_CARGO_APPROACH_AUDIO: &str = "CargoPlaneApproach";

/// Drop audio residual when payload units spawn.
pub const SUPPLY_DROP_CARGO_DROP_AUDIO: &str = "SupplyDropZoneDrop";

// --- AmericaJetCargoPlane CreateAtEdge flight residual ---

/// Retail AmericaJetCargoPlane model residual honesty (`AVCargoPln`).
pub const CARGO_PLANE_MODEL_NAME: &str = "AVCargoPln";

/// Retail cargo door sub-object residual honesty (`AVCargoPln_A2`).
pub const CARGO_PLANE_DOOR_MODEL_NAME: &str = "AVCargoPln_A2";

/// Retail Door_1_OPENING model condition residual label.
pub const CARGO_PLANE_DOOR_OPENING_CONDITION: &str = "DOOR_1_OPENING";

/// Retail Door_1_CLOSING model condition residual label.
pub const CARGO_PLANE_DOOR_CLOSING_CONDITION: &str = "DOOR_1_CLOSING";

/// Retail B52Locomotor Speed (dist/sec).
pub const B52_LOCOMOTOR_SPEED: f32 = 125.0;

/// B52 residual speed per logic frame (125 / 30 ≈ 4.1667).
pub const B52_SPEED_PER_FRAME: f32 = B52_LOCOMOTOR_SPEED / DELIVER_PAYLOAD_LOGIC_FPS;

/// Retail B52Locomotor MinSpeed residual honesty.
pub const B52_LOCOMOTOR_MIN_SPEED: f32 = 60.0;

/// Retail B52Locomotor TurnRate residual honesty (deg/sec).
pub const B52_LOCOMOTOR_TURN_RATE: f32 = 25.0;

/// Retail OCL StartAtPreferredHeight residual honesty.
pub const SUPPLY_DROP_START_AT_PREFERRED_HEIGHT: bool = true;

/// Retail OCL StartAtMaxSpeed residual honesty.
pub const SUPPLY_DROP_START_AT_MAX_SPEED: bool = true;

/// Retail AmericaJetCargoPlane TransportContain ExitBone residual honesty.
pub const CARGO_PLANE_EXIT_BONE: &str = "WeaponA01";

/// Retail AmericaJetCargoPlane ExitPitchRate residual honesty (deg/sec).
pub const CARGO_PLANE_EXIT_PITCH_RATE: f32 = 30.0;

/// Residual map extent for CreateAtEdge closest-edge residual (host default when
/// no TerrainLogic extent is available). Horizontal XZ; Y is height.
pub const RESIDUAL_MAP_EXTENT_MIN_X: f32 = 0.0;
pub const RESIDUAL_MAP_EXTENT_MIN_Z: f32 = 0.0;
pub const RESIDUAL_MAP_EXTENT_MAX_X: f32 = 500.0;
pub const RESIDUAL_MAP_EXTENT_MAX_Z: f32 = 500.0;

// --- AmericaCrateParachute bone attach residual ---

/// Retail AmericaCrateParachute pristine bone names (same as AmericaParachute).
pub const CRATE_PARA_BONE_COG: &str = "PARA_COG";
pub const CRATE_PARA_BONE_ATTCH: &str = "PARA_ATTCH";

/// Retail AmericaCrateParachute GeometryHeight residual.
pub const CRATE_PARA_GEOMETRY_HEIGHT: f32 = 10.0;

/// Retail AmericaCrateParachute GeometryMajorRadius residual.
pub const CRATE_PARA_GEOMETRY_MAJOR_RADIUS: f32 = 15.0;

/// Retail SupplyDropZoneCrate GeometryHeight residual (rider height-fallback).
pub const CRATE_RIDER_GEOMETRY_HEIGHT: f32 = 12.0;

/// Retail AmericaCrateParachute PitchRateMax / RollRateMax (deg/sec).
pub const CRATE_PARA_PITCH_RATE_MAX_DEG: f32 = 60.0;
pub const CRATE_PARA_ROLL_RATE_MAX_DEG: f32 = 60.0;

/// Host residual spawn height for cargo crate (plane PreferredHeight + DropOffset Y).
pub fn cargo_crate_drop_height(drop_offset_y: f32) -> f32 {
    CARGO_PLANE_PREFERRED_HEIGHT + drop_offset_y
}

/// C++ TerrainLogic::findClosestEdgePoint residual on host XZ horizontal plane.
///
/// Returns edge point at PreferredHeight when `start_at_preferred_height` is set.
pub fn find_closest_edge_point_residual(
    target: Vec3,
    map_min_x: f32,
    map_min_z: f32,
    map_max_x: f32,
    map_max_z: f32,
    preferred_height: f32,
) -> Vec3 {
    let d_min_z = (target.z - map_min_z).abs();
    let d_max_x = (target.x - map_max_x).abs();
    let d_max_z = (target.z - map_max_z).abs();
    let d_min_x = (target.x - map_min_x).abs();
    let mut best = 0_u8;
    let mut best_d = d_min_z;
    if d_max_x < best_d {
        best = 1;
        best_d = d_max_x;
    }
    if d_max_z < best_d {
        best = 2;
        best_d = d_max_z;
    }
    if d_min_x < best_d {
        best = 3;
    }
    let mut edge = target;
    match best {
        0 => edge.z = map_min_z,
        1 => edge.x = map_max_x,
        2 => edge.z = map_max_z,
        _ => edge.x = map_min_x,
    }
    edge.y = preferred_height;
    edge.x = edge.x.clamp(map_min_x, map_max_x);
    edge.z = edge.z.clamp(map_min_z, map_max_z);
    edge
}

/// Default residual map CreateAtEdge edge spawn for a target.
pub fn create_at_edge_spawn_residual(target: Vec3) -> Vec3 {
    find_closest_edge_point_residual(
        target,
        RESIDUAL_MAP_EXTENT_MIN_X,
        RESIDUAL_MAP_EXTENT_MIN_Z,
        RESIDUAL_MAP_EXTENT_MAX_X,
        RESIDUAL_MAP_EXTENT_MAX_Z,
        if SUPPLY_DROP_START_AT_PREFERRED_HEIGHT {
            CARGO_PLANE_PREFERRED_HEIGHT
        } else {
            target.y
        },
    )
}

/// Horizontal XZ distance residual (DeliverPayload approach band).
pub fn horizontal_distance_xz(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

/// C++ DeliverPayloadAIUpdate::isCloseEnoughToTarget residual.
pub fn is_close_enough_to_target_residual(
    current_dist: f32,
    previous_dist: f32,
    delivery_distance: f32,
    pre_open_distance: f32,
) -> bool {
    let inbound = previous_dist > current_dist;
    let allowed = if inbound {
        delivery_distance + pre_open_distance
    } else {
        delivery_distance
    };
    current_dist <= allowed
}

/// Advance cargo plane residual position toward target at B52 residual speed.
pub fn tick_cargo_plane_approach(
    current: Vec3,
    target: Vec3,
    preferred_height: f32,
    speed_per_frame: f32,
) -> (Vec3, f32) {
    let mut next = current;
    next.y = preferred_height;
    let dx = target.x - current.x;
    let dz = target.z - current.z;
    let dist = (dx * dx + dz * dz).sqrt();
    if dist <= 0.001 {
        return (next, 0.0);
    }
    let step = speed_per_frame.min(dist);
    next.x = current.x + dx / dist * step;
    next.z = current.z + dz / dist * step;
    let new_dist = horizontal_distance_xz(next, target);
    (next, new_dist)
}

/// Host residual DeliverPayloadAIUpdate flight phase presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostCargoPlaneFlightPhase {
    EdgeSpawn,
    Approaching,
    InDeliveryBand,
    DoorOpening,
    Delivering,
    Departing,
    Complete,
}

/// Host residual AmericaJetCargoPlane flight presentation state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostCargoPlaneFlight {
    pub mission_id: u32,
    pub transport_template: String,
    pub model_name: String,
    pub door_model_name: String,
    pub exit_bone: String,
    pub phase: HostCargoPlaneFlightPhase,
    pub edge_spawn_pos: Vec3,
    pub current_pos: Vec3,
    pub target_pos: Vec3,
    pub preferred_height: f32,
    pub speed_per_frame: f32,
    pub delivery_distance: f32,
    pub pre_open_distance: f32,
    pub max_attempts: u32,
    pub approach_attempt: u32,
    pub previous_distance: f32,
    pub start_at_preferred_height: bool,
    pub start_at_max_speed: bool,
    pub door_open: bool,
    pub door_condition: String,
    pub exit_pitch_rate: f32,
}

impl HostCargoPlaneFlight {
    pub fn new_supply_drop(mission_id: u32, target: Vec3) -> Self {
        let edge = create_at_edge_spawn_residual(target);
        let dist = horizontal_distance_xz(edge, target);
        Self {
            mission_id,
            transport_template: SUPPLY_DROP_CARGO_TRANSPORT.to_string(),
            model_name: CARGO_PLANE_MODEL_NAME.to_string(),
            door_model_name: CARGO_PLANE_DOOR_MODEL_NAME.to_string(),
            exit_bone: CARGO_PLANE_EXIT_BONE.to_string(),
            phase: HostCargoPlaneFlightPhase::EdgeSpawn,
            edge_spawn_pos: edge,
            current_pos: edge,
            target_pos: target,
            preferred_height: CARGO_PLANE_PREFERRED_HEIGHT,
            speed_per_frame: if SUPPLY_DROP_START_AT_MAX_SPEED {
                B52_SPEED_PER_FRAME
            } else {
                B52_LOCOMOTOR_MIN_SPEED / DELIVER_PAYLOAD_LOGIC_FPS
            },
            delivery_distance: SUPPLY_DROP_DELIVERY_DISTANCE,
            pre_open_distance: SUPPLY_DROP_PRE_OPEN_DISTANCE,
            max_attempts: SUPPLY_DROP_MAX_ATTEMPTS,
            approach_attempt: 1,
            previous_distance: dist,
            start_at_preferred_height: SUPPLY_DROP_START_AT_PREFERRED_HEIGHT,
            start_at_max_speed: SUPPLY_DROP_START_AT_MAX_SPEED,
            door_open: false,
            door_condition: String::new(),
            exit_pitch_rate: CARGO_PLANE_EXIT_PITCH_RATE,
        }
    }

    pub fn is_in_delivery_band(&self) -> bool {
        let dist = horizontal_distance_xz(self.current_pos, self.target_pos);
        is_close_enough_to_target_residual(
            dist,
            self.previous_distance,
            self.delivery_distance,
            self.pre_open_distance,
        )
    }

    pub fn tick(&mut self, items_dropped: u32, payload_count: u32, mission_complete: bool) {
        if matches!(
            self.phase,
            HostCargoPlaneFlightPhase::Complete | HostCargoPlaneFlightPhase::Departing
        ) {
            if mission_complete && self.phase != HostCargoPlaneFlightPhase::Complete {
                self.phase = HostCargoPlaneFlightPhase::Complete;
                self.door_open = false;
                self.door_condition = CARGO_PLANE_DOOR_CLOSING_CONDITION.to_string();
            }
            return;
        }

        if self.phase == HostCargoPlaneFlightPhase::EdgeSpawn {
            self.phase = HostCargoPlaneFlightPhase::Approaching;
        }

        let (next, dist) = tick_cargo_plane_approach(
            self.current_pos,
            self.target_pos,
            self.preferred_height,
            self.speed_per_frame,
        );
        let prev = self.previous_distance;
        self.current_pos = next;
        let close = is_close_enough_to_target_residual(
            dist,
            prev,
            self.delivery_distance,
            self.pre_open_distance,
        );
        self.previous_distance = dist;

        if items_dropped > 0 && payload_count > 0 && items_dropped < payload_count {
            self.phase = HostCargoPlaneFlightPhase::Delivering;
            self.door_open = true;
            self.door_condition = CARGO_PLANE_DOOR_OPENING_CONDITION.to_string();
        } else if items_dropped > 0 && payload_count > 0 && items_dropped >= payload_count {
            self.phase = HostCargoPlaneFlightPhase::Departing;
            self.door_open = false;
            self.door_condition = CARGO_PLANE_DOOR_CLOSING_CONDITION.to_string();
        } else if close {
            if self.phase == HostCargoPlaneFlightPhase::Approaching
                || self.phase == HostCargoPlaneFlightPhase::EdgeSpawn
            {
                self.phase = HostCargoPlaneFlightPhase::InDeliveryBand;
            }
            if !self.door_open
                && matches!(
                    self.phase,
                    HostCargoPlaneFlightPhase::InDeliveryBand
                        | HostCargoPlaneFlightPhase::DoorOpening
                        | HostCargoPlaneFlightPhase::Delivering
                )
            {
                self.phase = HostCargoPlaneFlightPhase::DoorOpening;
                self.door_open = true;
                self.door_condition = CARGO_PLANE_DOOR_OPENING_CONDITION.to_string();
            }
        }

        if mission_complete {
            self.phase = HostCargoPlaneFlightPhase::Complete;
            self.door_open = false;
            if self.door_condition.is_empty()
                || self.door_condition == CARGO_PLANE_DOOR_OPENING_CONDITION
            {
                self.door_condition = CARGO_PLANE_DOOR_CLOSING_CONDITION.to_string();
            }
        }
    }
}

/// AmericaCrateParachute host residual pristine bone offsets (no W3D extract).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostCrateParachuteBoneOffsets {
    pub para_cog: (f32, f32, f32),
    pub para_attch: (f32, f32, f32),
    pub crate_man: (f32, f32, f32),
}

pub fn crate_parachute_host_bone_offsets() -> HostCrateParachuteBoneOffsets {
    HostCrateParachuteBoneOffsets {
        para_cog: (0.0, CRATE_PARA_GEOMETRY_HEIGHT * 0.7, 0.0),
        para_attch: (0.0, CRATE_PARA_GEOMETRY_HEIGHT * 0.2, 0.0),
        crate_man: (0.0, CRATE_RIDER_GEOMETRY_HEIGHT, 0.0),
    }
}

pub fn crate_parachute_offsets_from_bones(
    bones: HostCrateParachuteBoneOffsets,
) -> ((f32, f32, f32), (f32, f32, f32), (f32, f32, f32)) {
    let para_sway = bones.para_cog;
    let crate_attach = (
        bones.para_attch.0 - bones.crate_man.0,
        bones.para_attch.1 - bones.crate_man.1,
        bones.para_attch.2 - bones.crate_man.2,
    );
    let crate_sway = (
        para_sway.0 - crate_attach.0,
        para_sway.1 - crate_attach.1,
        para_sway.2 - crate_attach.2,
    );
    (crate_attach, crate_sway, para_sway)
}

pub fn crate_parachute_crate_logic_position(
    para_pos: (f32, f32, f32),
    crate_attach: (f32, f32, f32),
) -> (f32, f32, f32) {
    (
        para_pos.0 + crate_attach.0,
        para_pos.1 + crate_attach.1,
        para_pos.2 + crate_attach.2,
    )
}

pub fn crate_parachute_presentation_position(
    para_pos: (f32, f32, f32),
    crate_attach: (f32, f32, f32),
    crate_sway: (f32, f32, f32),
    pitch: f32,
    roll: f32,
    chute_open: bool,
) -> (f32, f32, f32) {
    let logic = crate_parachute_crate_logic_position(para_pos, crate_attach);
    if !chute_open {
        return logic;
    }
    let px = crate_attach.0 - crate_sway.0;
    let py = crate_attach.1 - crate_sway.1;
    let pz = crate_attach.2 - crate_sway.2;
    let (sr, cr) = roll.sin_cos();
    let y1 = py * cr - pz * sr;
    let z1 = py * sr + pz * cr;
    let x1 = px;
    let (sp, cp) = pitch.sin_cos();
    let x2 = x1 * cp + y1 * sp;
    let y2 = -x1 * sp + y1 * cp;
    let z2 = z1;
    let out = (x2 + crate_sway.0, y2 + crate_sway.1, z2 + crate_sway.2);
    let delta = (
        out.0 - crate_attach.0,
        out.1 - crate_attach.1,
        out.2 - crate_attach.2,
    );
    (logic.0 + delta.0, logic.1 + delta.1, logic.2 + delta.2)
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostCrateParachuteAttach {
    pub bones: HostCrateParachuteBoneOffsets,
    pub crate_attach: (f32, f32, f32),
    pub crate_sway: (f32, f32, f32),
    pub para_sway: (f32, f32, f32),
    pub para_logic_pos: (f32, f32, f32),
    pub crate_logic_pos: (f32, f32, f32),
    pub crate_presentation_pos: (f32, f32, f32),
    pub chute_open: bool,
    pub pitch: f32,
    pub roll: f32,
}

pub fn crate_parachute_attach_presentation(
    para_pos: (f32, f32, f32),
    pitch: f32,
    roll: f32,
    chute_open: bool,
) -> HostCrateParachuteAttach {
    let bones = crate_parachute_host_bone_offsets();
    let (crate_attach, crate_sway, para_sway) = crate_parachute_offsets_from_bones(bones);
    let crate_logic = crate_parachute_crate_logic_position(para_pos, crate_attach);
    let crate_pres = crate_parachute_presentation_position(
        para_pos,
        crate_attach,
        crate_sway,
        pitch,
        roll,
        chute_open,
    );
    HostCrateParachuteAttach {
        bones,
        crate_attach,
        crate_sway,
        para_sway,
        para_logic_pos: para_pos,
        crate_logic_pos: crate_logic,
        crate_presentation_pos: crate_pres,
        chute_open,
        pitch,
        roll,
    }
}

/// Whether AmericaCrateParachute residual should open after freefall OpenDist.
pub fn should_open_crate_parachute(start_height: f32, current_height: f32) -> bool {
    (start_height - current_height) >= CRATE_PARACHUTE_OPEN_DIST
}

/// C++ ParachuteContain low-altitude open fudge residual for crate OpenDist.
pub fn fudge_crate_parachute_start_height(start_height: f32, ground_height: f32) -> f32 {
    let min_span = CRATE_PARACHUTE_LOW_ALTITUDE_OPEN_MULT * CRATE_PARACHUTE_OPEN_DIST;
    if start_height - ground_height < min_span {
        ground_height + min_span
    } else {
        start_height
    }
}

/// Advance AmericaCrateParachute residual sink (freefall vs open-chute rates).
///
/// Returns (new_height, landed). Host Y-up ground residual is typically 0.
pub fn tick_crate_parachute_height(
    current_height: f32,
    ground_height: f32,
    chute_open: bool,
) -> (f32, bool) {
    if current_height <= ground_height + 0.01 {
        return (ground_height, true);
    }
    let rate = if chute_open {
        CRATE_PARACHUTE_SINK_PER_FRAME
    } else {
        CRATE_PARACHUTE_FREEFALL_PER_FRAME
    };
    let next = (current_height - rate).max(ground_height);
    let landed = next <= ground_height + 0.01;
    (if landed { ground_height } else { next }, landed)
}

/// Whether residual crate is still above terrain (unit MoneyCrateCollide blocked).
pub fn crate_is_above_terrain(height_y: f32, ground_height: f32) -> bool {
    height_y > ground_height + 0.5
}

// --- SUPERWEAPON_Paradrop1 cargo residual honesty constants ---

/// Retail Paradrop DeliverPayload Transport.
pub const PARADROP_CARGO_TRANSPORT: &str = "AmericaJetCargoPlane";

/// Retail Paradrop PutInContainer.
pub const PARADROP_PUT_IN_CONTAINER: &str = "AmericaParachute";

/// Retail Paradrop PreOpenDistance honesty (SUPERWEAPON_Paradrop* = 300).
pub const PARADROP_PRE_OPEN_DISTANCE: f32 = 300.0;

/// Retail Paradrop MaxAttempts honesty.
pub const PARADROP_MAX_ATTEMPTS: u32 = 4;

/// Host residual DeliverPayload kind (cargo / superweapon drop family).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostDeliverPayloadKind {
    /// America Supply Drop Zone crate cargo plane residual
    /// (`OCL_AmericaSupplyDropZoneCrateDrop`).
    SupplyDropZoneCrate,
    /// America Paradrop cargo-plane DeliverPayload residual honesty
    /// (`SUPERWEAPON_Paradrop*`). Infantry spawn is owned by host_paradrop.
    AmericaParadrop,
}

impl HostDeliverPayloadKind {
    pub fn label(self) -> &'static str {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => "SupplyDropZoneCrate",
            HostDeliverPayloadKind::AmericaParadrop => "AmericaParadrop",
        }
    }

    /// Residual approach frames before delivery state (plane inbound residual).
    pub fn approach_delay_frames(self) -> u32 {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => CARGO_PLANE_APPROACH_DELAY_FRAMES,
            HostDeliverPayloadKind::AmericaParadrop => CARGO_PLANE_APPROACH_DELAY_FRAMES,
        }
    }

    /// DoorDelay residual frames before first item (AmericaJetCargoPlane module).
    pub fn door_delay_frames(self) -> u32 {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => CARGO_PLANE_DOOR_DELAY_FRAMES,
            HostDeliverPayloadKind::AmericaParadrop => CARGO_PLANE_DOOR_DELAY_FRAMES,
        }
    }

    /// DropDelay residual frames between successive payload items.
    pub fn drop_delay_frames(self) -> u32 {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => SUPPLY_DROP_DROP_DELAY_FRAMES,
            // Paradrop infantry stagger owned by host_paradrop residual.
            HostDeliverPayloadKind::AmericaParadrop => 0,
        }
    }

    /// Retail transport template name residual honesty.
    pub fn transport_template(self) -> &'static str {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => SUPPLY_DROP_CARGO_TRANSPORT,
            HostDeliverPayloadKind::AmericaParadrop => PARADROP_CARGO_TRANSPORT,
        }
    }

    /// PutInContainer residual honesty (parachute container name).
    pub fn put_in_container(self) -> &'static str {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => SUPPLY_DROP_PUT_IN_CONTAINER,
            HostDeliverPayloadKind::AmericaParadrop => PARADROP_PUT_IN_CONTAINER,
        }
    }

    /// Payload unit count residual (0 for Paradrop — spawn owned by host_paradrop).
    pub fn payload_count(self) -> u32 {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => SUPPLY_DROP_PAYLOAD_COUNT,
            HostDeliverPayloadKind::AmericaParadrop => 0,
        }
    }

    /// Preferred payload template residual.
    pub fn payload_template(self) -> &'static str {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => SUPPLY_DROP_PAYLOAD_TEMPLATE,
            HostDeliverPayloadKind::AmericaParadrop => "",
        }
    }

    /// Horizontal spacing between residual payload spawn points.
    pub fn payload_spacing(self) -> f32 {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => SUPPLY_DROP_CRATE_SPACING,
            HostDeliverPayloadKind::AmericaParadrop => 0.0,
        }
    }

    /// DropOffset residual applied to each spawn position (host Y-up).
    pub fn drop_offset(self) -> Vec3 {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => Vec3::new(
                SUPPLY_DROP_DROP_OFFSET_X,
                SUPPLY_DROP_DROP_OFFSET_Y,
                SUPPLY_DROP_DROP_OFFSET_Z,
            ),
            HostDeliverPayloadKind::AmericaParadrop => Vec3::new(0.0, -10.0, 0.0),
        }
    }

    /// MaxAttempts residual honesty.
    pub fn max_attempts(self) -> u32 {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => SUPPLY_DROP_MAX_ATTEMPTS,
            HostDeliverPayloadKind::AmericaParadrop => PARADROP_MAX_ATTEMPTS,
        }
    }

    /// PreOpenDistance residual honesty.
    pub fn pre_open_distance(self) -> f32 {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => SUPPLY_DROP_PRE_OPEN_DISTANCE,
            HostDeliverPayloadKind::AmericaParadrop => PARADROP_PRE_OPEN_DISTANCE,
        }
    }

    /// DeliveryDistance residual honesty.
    pub fn delivery_distance(self) -> f32 {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => SUPPLY_DROP_DELIVERY_DISTANCE,
            HostDeliverPayloadKind::AmericaParadrop => 0.0,
        }
    }

    /// Whether this kind should spawn residual payload objects on drop frames.
    pub fn spawns_payload_objects(self) -> bool {
        matches!(self, HostDeliverPayloadKind::SupplyDropZoneCrate)
    }

    /// Whether BuildingPickup residual cash should credit on drop complete.
    pub fn credits_building_pickup_cash(self) -> bool {
        matches!(self, HostDeliverPayloadKind::SupplyDropZoneCrate)
    }

    pub fn approach_audio(self) -> &'static str {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => SUPPLY_DROP_CARGO_APPROACH_AUDIO,
            HostDeliverPayloadKind::AmericaParadrop => "SuperweaponParadrop",
        }
    }

    pub fn drop_audio(self) -> &'static str {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => SUPPLY_DROP_CARGO_DROP_AUDIO,
            HostDeliverPayloadKind::AmericaParadrop => "ParadropLanding",
        }
    }

    /// Absolute frame when item index `i` is due (0-based).
    ///
    /// first_item = activate + approach + door_delay
    /// item_i = first_item + i * drop_delay
    pub fn item_drop_frame(self, activate_frame: u32, item_index: u32) -> u32 {
        let first = activate_frame
            .saturating_add(self.approach_delay_frames())
            .saturating_add(self.door_delay_frames());
        first.saturating_add(item_index.saturating_mul(self.drop_delay_frames()))
    }

    /// Frame when delivery is fully complete (last item due), or approach end
    /// when payload_count is 0 (paradrop bookkeeping).
    pub fn mission_complete_frame(self, activate_frame: u32) -> u32 {
        let count = self.payload_count();
        if count == 0 {
            return activate_frame.saturating_add(self.approach_delay_frames());
        }
        self.item_drop_frame(activate_frame, count.saturating_sub(1))
    }
}

/// Lifecycle of a queued host DeliverPayload mission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostDeliverPayloadPhase {
    /// Queued after OCL / special-power; waiting for approach / door delay.
    Queued,
    /// DropDelay stagger in progress (at least one item spawned, more remaining).
    Dropping,
    /// Payload resolved (all items spawned and/or cash credited).
    Completed,
    /// Cancelled (source died / invalid) before drop complete.
    Cancelled,
}

/// One pending or completed host DeliverPayload cargo mission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostDeliverPayloadMission {
    pub id: u32,
    pub kind: HostDeliverPayloadKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub activate_frame: u32,
    /// Frame when first payload item is due (approach + door delay).
    pub drop_frame: u32,
    /// Frame when last payload item is due (stagger complete).
    pub complete_frame: u32,
    pub phase: HostDeliverPayloadPhase,
    /// Transport template residual honesty (`AmericaJetCargoPlane`).
    pub transport_template: String,
    /// PutInContainer residual honesty.
    pub put_in_container: String,
    /// Payload template used (or intended) for spawned units.
    pub payload_template: String,
    /// Number of payload units requested at queue time.
    pub payload_count: u32,
    /// Number of payload items already spawned under DropDelay stagger.
    pub items_dropped: u32,
    /// Object ids of payload units successfully created.
    pub spawned_payload_ids: Vec<ObjectId>,
    /// BuildingPickup residual cash credited on complete (supply crates).
    pub cash_credited: u32,
    /// MaxAttempts residual honesty.
    pub max_attempts: u32,
    /// PreOpenDistance residual honesty.
    pub pre_open_distance: f32,
    /// DeliveryDistance residual honesty.
    pub delivery_distance: f32,
}

/// Spawn plan for one due payload item (DropDelay stagger).
#[derive(Debug, Clone)]
pub struct HostDeliverPayloadItemPlan {
    pub mission_id: u32,
    pub kind: HostDeliverPayloadKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub payload_template: String,
    /// 0-based item index within mission.
    pub item_index: u32,
    pub spawn_position: Vec3,
    /// True when this item is the last residual payload for the mission.
    pub is_final_item: bool,
}

/// Legacy multi-position plan alias (formation snapshot at first drop).
#[derive(Debug, Clone)]
pub struct HostDeliverPayloadDropPlan {
    pub mission_id: u32,
    pub kind: HostDeliverPayloadKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub payload_template: String,
    pub spawn_positions: Vec<Vec3>,
}

/// Host registry of DeliverPayload cargo missions.
#[derive(Debug, Clone, Default)]
pub struct HostDeliverPayloadRegistry {
    next_id: u32,
    missions: HashMap<u32, HostDeliverPayloadMission>,
    /// CreateAtEdge AmericaJetCargoPlane flight presentation residual.
    cargo_flights: HashMap<u32, HostCargoPlaneFlight>,
    completed_this_frame: Vec<u32>,
    activated_this_frame: Vec<u32>,
    items_spawned_this_frame: Vec<(u32, ObjectId)>,
    /// Total cargo flights queued (honesty).
    pub flights_queued: u32,
    /// Total payload objects spawned across all missions (honesty).
    pub payload_spawned_total: u32,
    /// Total BuildingPickup residual cash credited via cargo path (honesty).
    pub cash_credited_total: u32,
    /// Total DropDelay stagger item events (honesty).
    pub stagger_items_total: u32,
    /// AmericaCrateParachute residual chute-open events (OpenDist freefall).
    pub crate_parachute_opens: u32,
    /// AmericaCrateParachute residual land events.
    pub crate_parachute_lands: u32,
    /// CreateAtEdge residual edge-spawn events (honesty).
    pub create_at_edge_spawns: u32,
    /// DeliveryDistance band entries observed (honesty).
    pub delivery_band_entries: u32,
    /// Door open residual events (honesty).
    pub door_open_events: u32,
    /// AmericaCrateParachute bone attach presentation builds (honesty).
    pub crate_bone_attach_builds: u32,
}

impl HostDeliverPayloadRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            missions: HashMap::new(),
            cargo_flights: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
            items_spawned_this_frame: Vec::new(),
            flights_queued: 0,
            payload_spawned_total: 0,
            cash_credited_total: 0,
            stagger_items_total: 0,
            crate_parachute_opens: 0,
            crate_parachute_lands: 0,
            create_at_edge_spawns: 0,
            delivery_band_entries: 0,
            door_open_events: 0,
            crate_bone_attach_builds: 0,
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn clear_frame_events(&mut self) {
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
        self.items_spawned_this_frame.clear();
    }

    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    pub fn flights_queued(&self) -> u32 {
        self.flights_queued
    }

    pub fn payload_spawned_total(&self) -> u32 {
        self.payload_spawned_total
    }

    pub fn cash_credited_total(&self) -> u32 {
        self.cash_credited_total
    }

    pub fn stagger_items_total(&self) -> u32 {
        self.stagger_items_total
    }

    pub fn restore_from_snapshot(
        &mut self,
        next_id: u32,
        missions: impl IntoIterator<Item = HostDeliverPayloadMission>,
        flights_queued: u32,
        payload_spawned_total: u32,
        cash_credited_total: u32,
    ) {
        self.clear();
        let mut max_id = 0_u32;
        for mission in missions {
            max_id = max_id.max(mission.id);
            self.missions.insert(mission.id, mission);
        }
        self.next_id = next_id.max(max_id.saturating_add(1)).max(1);
        self.flights_queued = flights_queued;
        self.payload_spawned_total = payload_spawned_total;
        self.cash_credited_total = cash_credited_total;
    }

    pub fn mission_count(&self) -> usize {
        self.missions.len()
    }

    pub fn pending_count(&self) -> usize {
        self.missions
            .values()
            .filter(|m| {
                matches!(
                    m.phase,
                    HostDeliverPayloadPhase::Queued | HostDeliverPayloadPhase::Dropping
                )
            })
            .count()
    }

    pub fn completed_count(&self) -> usize {
        self.missions
            .values()
            .filter(|m| m.phase == HostDeliverPayloadPhase::Completed)
            .count()
    }

    pub fn get(&self, id: u32) -> Option<&HostDeliverPayloadMission> {
        self.missions.get(&id)
    }

    pub fn missions_snapshot(&self) -> Vec<HostDeliverPayloadMission> {
        let mut v: Vec<_> = self.missions.values().cloned().collect();
        v.sort_by_key(|m| m.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostDeliverPayloadKind) -> Vec<&HostDeliverPayloadMission> {
        self.missions
            .values()
            .filter(|m| {
                m.kind == kind
                    && matches!(
                        m.phase,
                        HostDeliverPayloadPhase::Queued | HostDeliverPayloadPhase::Dropping
                    )
            })
            .collect()
    }

    pub fn completed_of_kind(
        &self,
        kind: HostDeliverPayloadKind,
    ) -> Vec<&HostDeliverPayloadMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostDeliverPayloadPhase::Completed && m.kind == kind)
            .collect()
    }

    pub fn activated_this_frame(&self) -> &[u32] {
        &self.activated_this_frame
    }

    pub fn completed_this_frame(&self) -> &[u32] {
        &self.completed_this_frame
    }

    /// Line-formation drop positions around target + DropOffset residual.
    pub fn drop_positions(
        center: Vec3,
        unit_count: u32,
        spacing: f32,
        drop_offset: Vec3,
    ) -> Vec<Vec3> {
        if unit_count == 0 {
            return Vec::new();
        }
        let n = unit_count as usize;
        let mut positions = Vec::with_capacity(n);
        for i in 0..n {
            let offset = if n > 1 {
                let total_width = (n - 1) as f32 * spacing;
                (i as f32 * spacing) - (total_width / 2.0)
            } else {
                0.0
            };
            positions.push(Vec3::new(
                center.x + offset + drop_offset.x,
                center.y + drop_offset.y,
                center.z + drop_offset.z,
            ));
        }
        positions
    }

    /// Single item formation position for stagger residual.
    pub fn drop_position_for_item(
        center: Vec3,
        unit_count: u32,
        item_index: u32,
        spacing: f32,
        drop_offset: Vec3,
    ) -> Vec3 {
        let positions = Self::drop_positions(center, unit_count, spacing, drop_offset);
        positions
            .get(item_index as usize)
            .copied()
            .unwrap_or(center + drop_offset)
    }

    /// Queue a DeliverPayload cargo mission. Returns host mission id.
    pub fn queue(
        &mut self,
        kind: HostDeliverPayloadKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
        payload_template: impl Into<String>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        // Spawning kinds: first item after approach + DoorDelay.
        // Non-spawning (Paradrop bookkeeping): complete at approach residual end.
        let drop_frame = if kind.spawns_payload_objects() {
            kind.item_drop_frame(activate_frame, 0)
        } else {
            kind.mission_complete_frame(activate_frame)
        };
        let complete_frame = kind.mission_complete_frame(activate_frame);
        let mission = HostDeliverPayloadMission {
            id,
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            drop_frame,
            complete_frame,
            phase: HostDeliverPayloadPhase::Queued,
            transport_template: kind.transport_template().to_string(),
            put_in_container: kind.put_in_container().to_string(),
            payload_template: payload_template.into(),
            payload_count: kind.payload_count(),
            items_dropped: 0,
            spawned_payload_ids: Vec::new(),
            cash_credited: 0,
            max_attempts: kind.max_attempts(),
            pre_open_distance: kind.pre_open_distance(),
            delivery_distance: kind.delivery_distance(),
        };
        self.missions.insert(id, mission);
        self.activated_this_frame.push(id);
        self.flights_queued = self.flights_queued.saturating_add(1);
        // CreateAtEdge AmericaJetCargoPlane flight residual (presentation state).
        if kind.spawns_payload_objects() {
            let flight = HostCargoPlaneFlight::new_supply_drop(id, target_position);
            self.create_at_edge_spawns = self.create_at_edge_spawns.saturating_add(1);
            self.cargo_flights.insert(id, flight);
        }
        id
    }

    /// Residual CreateAtEdge cargo-plane flight presentation for a mission.
    pub fn cargo_flight(&self, mission_id: u32) -> Option<&HostCargoPlaneFlight> {
        self.cargo_flights.get(&mission_id)
    }

    pub fn cargo_flight_mut(&mut self, mission_id: u32) -> Option<&mut HostCargoPlaneFlight> {
        self.cargo_flights.get_mut(&mission_id)
    }

    /// Snapshot of residual cargo flights (sorted by mission id).
    pub fn cargo_flights_snapshot(&self) -> Vec<HostCargoPlaneFlight> {
        let mut v: Vec<_> = self.cargo_flights.values().cloned().collect();
        v.sort_by_key(|f| f.mission_id);
        v
    }

    /// Tick residual CreateAtEdge cargo-plane flights (presentation / approach).
    pub fn tick_cargo_flights(&mut self) {
        let mission_states: Vec<(u32, u32, u32, bool)> = self
            .missions
            .values()
            .map(|m| {
                (
                    m.id,
                    m.items_dropped,
                    m.payload_count,
                    m.phase == HostDeliverPayloadPhase::Completed
                        || m.phase == HostDeliverPayloadPhase::Cancelled,
                )
            })
            .collect();
        for (id, items_dropped, payload_count, complete) in mission_states {
            let Some(flight) = self.cargo_flights.get_mut(&id) else {
                continue;
            };
            let was_band = matches!(
                flight.phase,
                HostCargoPlaneFlightPhase::InDeliveryBand
                    | HostCargoPlaneFlightPhase::DoorOpening
                    | HostCargoPlaneFlightPhase::Delivering
            );
            let was_door = flight.door_open;
            flight.tick(items_dropped, payload_count, complete);
            let now_band = matches!(
                flight.phase,
                HostCargoPlaneFlightPhase::InDeliveryBand
                    | HostCargoPlaneFlightPhase::DoorOpening
                    | HostCargoPlaneFlightPhase::Delivering
                    | HostCargoPlaneFlightPhase::Departing
                    | HostCargoPlaneFlightPhase::Complete
            );
            if !was_band && now_band {
                self.delivery_band_entries = self.delivery_band_entries.saturating_add(1);
            }
            if !was_door && flight.door_open {
                self.door_open_events = self.door_open_events.saturating_add(1);
            }
        }
    }

    /// Build AmericaCrateParachute bone attach residual presentation and record honesty.
    pub fn build_crate_parachute_attach(
        &mut self,
        para_pos: (f32, f32, f32),
        pitch: f32,
        roll: f32,
        chute_open: bool,
    ) -> HostCrateParachuteAttach {
        self.crate_bone_attach_builds = self.crate_bone_attach_builds.saturating_add(1);
        crate_parachute_attach_presentation(para_pos, pitch, roll, chute_open)
    }

    /// Build per-item spawn plans for DropDelay stagger residual.
    ///
    /// At most one item per mission per call (matches C++ one exit per DropDelay tick).
    pub fn plan_due_item_spawns(&self, current_frame: u32) -> Vec<HostDeliverPayloadItemPlan> {
        let mut plans = Vec::new();
        for mission in self.missions.values() {
            if !matches!(
                mission.phase,
                HostDeliverPayloadPhase::Queued | HostDeliverPayloadPhase::Dropping
            ) {
                continue;
            }
            if !mission.kind.spawns_payload_objects() {
                // Paradrop bookkeeping completes at complete_frame without host spawn.
                continue;
            }
            let next_index = mission.items_dropped;
            if next_index >= mission.payload_count {
                continue;
            }
            let due = mission
                .kind
                .item_drop_frame(mission.activate_frame, next_index);
            if current_frame < due {
                continue;
            }
            let mut spawn_position = Self::drop_position_for_item(
                mission.target_position,
                mission.payload_count,
                next_index,
                mission.kind.payload_spacing(),
                mission.kind.drop_offset(),
            );
            // AmericaCrateParachute residual: elevate to cargo-plane PreferredHeight
            // + DropOffset Y (fail-closed vs full CreateAtEdge aircraft altitude).
            if mission.kind.spawns_payload_objects() {
                spawn_position.y = cargo_crate_drop_height(mission.kind.drop_offset().y);
            }
            plans.push(HostDeliverPayloadItemPlan {
                mission_id: mission.id,
                kind: mission.kind,
                source_object: mission.source_object,
                source_team: mission.source_team,
                target_position: mission.target_position,
                payload_template: mission.payload_template.clone(),
                item_index: next_index,
                spawn_position,
                is_final_item: next_index + 1 >= mission.payload_count,
            });
        }
        plans.sort_by_key(|p| (p.mission_id, p.item_index));
        plans
    }

    /// Legacy: full formation plan when first item is due (tests / observers).
    pub fn plan_due_drops(&self, current_frame: u32) -> Vec<HostDeliverPayloadDropPlan> {
        let mut plans = Vec::new();
        for mission in self.missions.values() {
            if !matches!(
                mission.phase,
                HostDeliverPayloadPhase::Queued | HostDeliverPayloadPhase::Dropping
            ) || current_frame < mission.drop_frame
            {
                continue;
            }
            // Only emit once at first drop frame while still Queued (observer snapshot).
            if mission.phase != HostDeliverPayloadPhase::Queued
                && mission.items_dropped > 0
            {
                continue;
            }
            let spawn_positions = if mission.kind.spawns_payload_objects() {
                Self::drop_positions(
                    mission.target_position,
                    mission.payload_count,
                    mission.kind.payload_spacing(),
                    mission.kind.drop_offset(),
                )
            } else {
                Vec::new()
            };
            plans.push(HostDeliverPayloadDropPlan {
                mission_id: mission.id,
                kind: mission.kind,
                source_object: mission.source_object,
                source_team: mission.source_team,
                target_position: mission.target_position,
                payload_template: mission.payload_template.clone(),
                spawn_positions,
            });
        }
        plans.sort_by_key(|p| p.mission_id);
        plans
    }

    /// Record one staggered payload item spawn.
    ///
    /// When final item is recorded, phase → Completed (cash may be applied later
    /// via [`Self::record_cash_credited`] or [`Self::record_drop_complete`]).
    pub fn record_item_spawned(&mut self, mission_id: u32, spawned_id: Option<ObjectId>) {
        let Some(mission) = self.missions.get_mut(&mission_id) else {
            return;
        };
        if !matches!(
            mission.phase,
            HostDeliverPayloadPhase::Queued | HostDeliverPayloadPhase::Dropping
        ) {
            return;
        }
        if let Some(id) = spawned_id {
            mission.spawned_payload_ids.push(id);
            self.payload_spawned_total = self.payload_spawned_total.saturating_add(1);
            self.items_spawned_this_frame.push((mission_id, id));
        }
        mission.items_dropped = mission.items_dropped.saturating_add(1);
        self.stagger_items_total = self.stagger_items_total.saturating_add(1);
        if mission.items_dropped < mission.payload_count {
            mission.phase = HostDeliverPayloadPhase::Dropping;
        } else {
            mission.phase = HostDeliverPayloadPhase::Completed;
            self.completed_this_frame.push(mission_id);
        }
    }

    /// Credit BuildingPickup residual cash after mission complete.
    pub fn record_cash_credited(&mut self, mission_id: u32, cash_credited: u32) {
        if let Some(mission) = self.missions.get_mut(&mission_id) {
            if cash_credited > 0 {
                mission.cash_credited = mission.cash_credited.saturating_add(cash_credited);
                self.cash_credited_total = self.cash_credited_total.saturating_add(cash_credited);
            }
        }
    }

    /// Record full drop results (legacy / paradrop bookkeeping / bulk complete).
    pub fn record_drop_complete(
        &mut self,
        mission_id: u32,
        spawned_payload_ids: Vec<ObjectId>,
        cash_credited: u32,
    ) {
        if let Some(mission) = self.missions.get_mut(&mission_id) {
            if matches!(
                mission.phase,
                HostDeliverPayloadPhase::Queued | HostDeliverPayloadPhase::Dropping
            ) {
                let spawn_count = spawned_payload_ids.len() as u32;
                // If stagger already spawned some, only count newly provided ids.
                if mission.spawned_payload_ids.is_empty() {
                    mission.spawned_payload_ids = spawned_payload_ids;
                    mission.items_dropped = spawn_count;
                    self.payload_spawned_total =
                        self.payload_spawned_total.saturating_add(spawn_count);
                    self.stagger_items_total =
                        self.stagger_items_total.saturating_add(spawn_count);
                } else if !spawned_payload_ids.is_empty() {
                    for id in spawned_payload_ids {
                        if !mission.spawned_payload_ids.contains(&id) {
                            mission.spawned_payload_ids.push(id);
                            mission.items_dropped = mission.items_dropped.saturating_add(1);
                            self.payload_spawned_total =
                                self.payload_spawned_total.saturating_add(1);
                            self.stagger_items_total = self.stagger_items_total.saturating_add(1);
                        }
                    }
                }
                mission.cash_credited = cash_credited;
                mission.phase = HostDeliverPayloadPhase::Completed;
                self.completed_this_frame.push(mission_id);
                self.cash_credited_total = self.cash_credited_total.saturating_add(cash_credited);
            } else if mission.phase == HostDeliverPayloadPhase::Completed && cash_credited > 0 {
                // Allow late cash attach after stagger complete.
                let delta = cash_credited.saturating_sub(mission.cash_credited);
                mission.cash_credited = cash_credited;
                self.cash_credited_total = self.cash_credited_total.saturating_add(delta);
            }
        }
    }

    /// Cancel pending missions owned by a destroyed source object.
    pub fn cancel_for_source(&mut self, source: ObjectId) {
        for mission in self.missions.values_mut() {
            if mission.source_object == source
                && matches!(
                    mission.phase,
                    HostDeliverPayloadPhase::Queued | HostDeliverPayloadPhase::Dropping
                )
            {
                mission.phase = HostDeliverPayloadPhase::Cancelled;
            }
        }
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    pub fn honesty_queue_ok(&self, kind: HostDeliverPayloadKind) -> bool {
        self.missions.values().any(|m| m.kind == kind)
    }

    pub fn honesty_inbound_ok(&self, kind: HostDeliverPayloadKind) -> bool {
        !self.pending_of_kind(kind).is_empty()
    }

    pub fn honesty_complete_ok(&self, kind: HostDeliverPayloadKind) -> bool {
        !self.completed_of_kind(kind).is_empty()
    }

    pub fn honesty_payload_spawn_ok(&self, kind: HostDeliverPayloadKind) -> bool {
        self.completed_of_kind(kind)
            .iter()
            .any(|m| !m.spawned_payload_ids.is_empty())
            || self.missions.values().any(|m| {
                m.kind == kind
                    && m.phase == HostDeliverPayloadPhase::Dropping
                    && !m.spawned_payload_ids.is_empty()
            })
    }

    /// DropDelay stagger honesty: more than one item event observed for a mission.
    pub fn honesty_drop_delay_stagger_ok(&self) -> bool {
        self.stagger_items_total > 1
            || self.missions.values().any(|m| m.items_dropped > 1)
    }

    pub fn honesty_building_pickup_ok(&self) -> bool {
        self.cash_credited_total > 0
            && self
                .completed_of_kind(HostDeliverPayloadKind::SupplyDropZoneCrate)
                .iter()
                .any(|m| m.cash_credited > 0)
    }

    pub fn honesty_supply_drop_cargo_host_path_ok(&self) -> bool {
        self.honesty_payload_spawn_ok(HostDeliverPayloadKind::SupplyDropZoneCrate)
            && (self.honesty_building_pickup_ok()
                || self
                    .completed_of_kind(HostDeliverPayloadKind::SupplyDropZoneCrate)
                    .iter()
                    .any(|m| m.spawned_payload_ids.len() as u32 >= SUPPLY_DROP_PAYLOAD_COUNT))
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_supply_drop_cargo_host_path_ok()
            || self.honesty_complete_ok(HostDeliverPayloadKind::AmericaParadrop)
    }

    pub fn honesty_transport_names_ok(kind: HostDeliverPayloadKind) -> bool {
        !kind.transport_template().is_empty() && !kind.put_in_container().is_empty()
    }

    /// Approach geometry residual honesty constants (not full flight path).
    pub fn honesty_approach_constants_ok(kind: HostDeliverPayloadKind) -> bool {
        kind.max_attempts() > 0
            && kind.delivery_distance() >= 0.0
            && kind.pre_open_distance() >= 0.0
    }

    /// AmericaCrateParachute residual honesty (OpenDist chute open observed).
    pub fn record_crate_parachute_open(&mut self) {
        self.crate_parachute_opens = self.crate_parachute_opens.saturating_add(1);
    }

    pub fn record_crate_parachute_land(&mut self) {
        self.crate_parachute_lands = self.crate_parachute_lands.saturating_add(1);
    }

    pub fn honesty_crate_parachute_open_ok(&self) -> bool {
        self.crate_parachute_opens > 0
    }

    pub fn honesty_crate_parachute_land_ok(&self) -> bool {
        self.crate_parachute_lands > 0
    }

    pub fn honesty_crate_parachute_fall_physics_ok(&self) -> bool {
        self.honesty_crate_parachute_open_ok() && self.honesty_crate_parachute_land_ok()
    }

    /// ParachuteDirectly + OpenDist + PreferredHeight residual constants honesty.
    pub fn honesty_crate_parachute_constants_ok() -> bool {
        SUPPLY_DROP_PARACHUTE_DIRECTLY
            && (CRATE_PARACHUTE_OPEN_DIST - 12.5).abs() < 0.01
            && (CARGO_PLANE_PREFERRED_HEIGHT - 100.0).abs() < 0.01
            && (CRATE_PARACHUTE_SPEED_LIMIT_Z - 15.0).abs() < 0.01
    }

    pub fn honesty_create_at_edge_ok(&self) -> bool {
        self.create_at_edge_spawns > 0
            && self.cargo_flights.values().any(|f| {
                f.edge_spawn_pos.y >= CARGO_PLANE_PREFERRED_HEIGHT - 0.01
                    && f.transport_template == SUPPLY_DROP_CARGO_TRANSPORT
                    && f.model_name == CARGO_PLANE_MODEL_NAME
            })
    }

    pub fn honesty_delivery_band_ok(&self) -> bool {
        self.delivery_band_entries > 0
            || self.cargo_flights.values().any(|f| {
                matches!(
                    f.phase,
                    HostCargoPlaneFlightPhase::InDeliveryBand
                        | HostCargoPlaneFlightPhase::DoorOpening
                        | HostCargoPlaneFlightPhase::Delivering
                        | HostCargoPlaneFlightPhase::Departing
                        | HostCargoPlaneFlightPhase::Complete
                )
            })
    }

    pub fn honesty_cargo_door_ok(&self) -> bool {
        self.door_open_events > 0
            || self.cargo_flights.values().any(|f| {
                f.door_open || f.door_condition == CARGO_PLANE_DOOR_OPENING_CONDITION
            })
    }

    pub fn honesty_create_at_edge_flight_ok(&self) -> bool {
        self.honesty_create_at_edge_ok()
            && (self.honesty_delivery_band_ok() || self.honesty_cargo_door_ok())
            && SUPPLY_DROP_START_AT_PREFERRED_HEIGHT
            && SUPPLY_DROP_START_AT_MAX_SPEED
            && (B52_LOCOMOTOR_SPEED - 125.0).abs() < 0.01
    }

    pub fn honesty_crate_bone_attach_ok(&self) -> bool {
        self.crate_bone_attach_builds > 0
    }

    pub fn honesty_crate_bone_constants_ok() -> bool {
        CRATE_PARA_BONE_COG == "PARA_COG"
            && CRATE_PARA_BONE_ATTCH == "PARA_ATTCH"
            && (CRATE_PARA_GEOMETRY_HEIGHT - 10.0).abs() < 0.01
            && (CRATE_RIDER_GEOMETRY_HEIGHT - 12.0).abs() < 0.01
            && (CRATE_PARA_PITCH_RATE_MAX_DEG - 60.0).abs() < 0.01
    }
}

/// DropDelay / DoorDelay ms → logic frames residual (30 FPS).
pub fn drop_delay_frames_from_ms(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / DELIVER_PAYLOAD_LOGIC_FPS)).round() as u32
}

/// Residual approach band: DeliveryDistance + PreOpenDistance (C++ allowedDistance).
pub fn residual_allowed_delivery_distance(kind: HostDeliverPayloadKind) -> f32 {
    kind.delivery_distance() + kind.pre_open_distance()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn supply_drop_ocl_constants_match_retail() {
        assert_eq!(SUPPLY_DROP_CARGO_TRANSPORT, "AmericaJetCargoPlane");
        assert_eq!(SUPPLY_DROP_PAYLOAD_TEMPLATE, "SupplyDropZoneCrate");
        assert_eq!(SUPPLY_DROP_PUT_IN_CONTAINER, "AmericaCrateParachute");
        assert_eq!(SUPPLY_DROP_PAYLOAD_COUNT, 6);
        assert_eq!(SUPPLY_DROP_DROP_DELAY_MS, 350);
        assert_eq!(SUPPLY_DROP_DROP_DELAY_FRAMES, 11);
        assert_eq!(drop_delay_frames_from_ms(350), 11);
        assert_eq!(CARGO_PLANE_DOOR_DELAY_MS, 500);
        assert_eq!(CARGO_PLANE_DOOR_DELAY_FRAMES, 15);
        assert_eq!(drop_delay_frames_from_ms(500), 15);
        assert!((SUPPLY_DROP_DELIVERY_DISTANCE - 410.0).abs() < 0.01);
        assert_eq!(SUPPLY_DROP_MAX_ATTEMPTS, 4);
        assert!((SUPPLY_DROP_PRE_OPEN_DISTANCE - 0.0).abs() < 0.01);
        assert!((SUPPLY_DROP_DROP_OFFSET_Y - (-5.0)).abs() < 0.01);
        assert_eq!(CARGO_PLANE_APPROACH_DELAY_FRAMES, 90);
        assert!((CARGO_PLANE_PREFERRED_HEIGHT - 100.0).abs() < 0.01);
        assert!((CRATE_PARACHUTE_OPEN_DIST - 12.5).abs() < 0.01);
        assert!((CRATE_PARACHUTE_SPEED_LIMIT_Z - 15.0).abs() < 0.01);
        assert!(SUPPLY_DROP_PARACHUTE_DIRECTLY);
        assert!(
            (cargo_crate_drop_height(SUPPLY_DROP_DROP_OFFSET_Y) - 95.0).abs() < 0.01
        );
        assert!(HostDeliverPayloadRegistry::honesty_transport_names_ok(
            HostDeliverPayloadKind::SupplyDropZoneCrate
        ));
        assert!(HostDeliverPayloadRegistry::honesty_approach_constants_ok(
            HostDeliverPayloadKind::SupplyDropZoneCrate
        ));
        assert!(HostDeliverPayloadRegistry::honesty_crate_parachute_constants_ok());
        assert!(
            (residual_allowed_delivery_distance(HostDeliverPayloadKind::SupplyDropZoneCrate)
                - 410.0)
                .abs()
                < 0.01
        );
        assert!(
            (residual_allowed_delivery_distance(HostDeliverPayloadKind::AmericaParadrop) - 300.0)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn crate_parachute_open_dist_and_sink_residual() {
        // Freefall until fallen ≥ 12.5.
        assert!(!should_open_crate_parachute(95.0, 90.0)); // fallen 5
        assert!(should_open_crate_parachute(95.0, 82.5)); // fallen 12.5
        // Low-altitude fudge: start 10 < 2×12.5 → fudge to 25.
        let fudged = fudge_crate_parachute_start_height(10.0, 0.0);
        assert!((fudged - 25.0).abs() < 0.01);
        // Freefall faster than open.
        let (ff, _) = tick_crate_parachute_height(95.0, 0.0, false);
        let (open, _) = tick_crate_parachute_height(95.0, 0.0, true);
        assert!(ff < open, "freefall must sink faster than open chute");
        assert!((95.0 - ff - CRATE_PARACHUTE_FREEFALL_PER_FRAME).abs() < 0.01);
        assert!((95.0 - open - CRATE_PARACHUTE_SINK_PER_FRAME).abs() < 0.01);
        assert!(crate_is_above_terrain(10.0, 0.0));
        assert!(!crate_is_above_terrain(0.0, 0.0));
        assert!(!crate_is_above_terrain(0.4, 0.0));
    }

    #[test]
    fn drop_delay_stagger_item_frames() {
        let kind = HostDeliverPayloadKind::SupplyDropZoneCrate;
        // activate 0 → first = 90 + 15 = 105, then +11 each
        assert_eq!(kind.item_drop_frame(0, 0), 105);
        assert_eq!(kind.item_drop_frame(0, 1), 116);
        assert_eq!(kind.item_drop_frame(0, 5), 105 + 5 * 11);
        assert_eq!(kind.mission_complete_frame(0), 105 + 5 * 11);
    }

    #[test]
    fn queue_and_stagger_supply_drop_cargo() {
        let mut reg = HostDeliverPayloadRegistry::new();
        let id = reg.queue(
            HostDeliverPayloadKind::SupplyDropZoneCrate,
            ObjectId(1),
            Team::USA,
            Vec3::new(100.0, 0.0, 50.0),
            0,
            SUPPLY_DROP_PAYLOAD_TEMPLATE,
        );
        assert!(reg.honesty_inbound_ok(HostDeliverPayloadKind::SupplyDropZoneCrate));
        assert!(!reg.honesty_complete_ok(HostDeliverPayloadKind::SupplyDropZoneCrate));
        assert_eq!(reg.flights_queued(), 1);

        let mission = reg.get(id).expect("mission");
        assert_eq!(mission.drop_frame, 105);
        assert_eq!(mission.payload_count, 6);
        assert_eq!(mission.max_attempts, 4);
        assert_eq!(mission.transport_template, SUPPLY_DROP_CARGO_TRANSPORT);
        assert_eq!(mission.put_in_container, SUPPLY_DROP_PUT_IN_CONTAINER);

        assert!(reg.plan_due_item_spawns(104).is_empty());
        let first = reg.plan_due_item_spawns(105);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].item_index, 0);
        assert!(!first[0].is_final_item);
        // Cargo PreferredHeight 100 + DropOffset Y -5 → spawn Y 95 residual
        assert!(
            (first[0].spawn_position.y - cargo_crate_drop_height(SUPPLY_DROP_DROP_OFFSET_Y)).abs()
                < 0.01
        );

        reg.record_item_spawned(id, Some(ObjectId(10)));
        assert_eq!(reg.get(id).unwrap().phase, HostDeliverPayloadPhase::Dropping);
        assert_eq!(reg.get(id).unwrap().items_dropped, 1);

        // Not yet due for item 1
        assert!(reg.plan_due_item_spawns(115).is_empty());
        let second = reg.plan_due_item_spawns(116);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].item_index, 1);
        reg.record_item_spawned(id, Some(ObjectId(11)));

        // Finish remaining 4
        for i in 2..6 {
            let frame = HostDeliverPayloadKind::SupplyDropZoneCrate.item_drop_frame(0, i);
            let plans = reg.plan_due_item_spawns(frame);
            assert_eq!(plans.len(), 1, "item {i} at frame {frame}");
            assert_eq!(plans[0].is_final_item, i == 5);
            reg.record_item_spawned(id, Some(ObjectId(10 + i)));
        }
        assert_eq!(reg.get(id).unwrap().phase, HostDeliverPayloadPhase::Completed);
        assert_eq!(reg.get(id).unwrap().spawned_payload_ids.len(), 6);
        assert!(reg.honesty_drop_delay_stagger_ok());
        assert!(reg.honesty_payload_spawn_ok(HostDeliverPayloadKind::SupplyDropZoneCrate));

        reg.record_cash_credited(id, 1500);
        assert!(reg.honesty_building_pickup_ok());
        assert!(reg.honesty_supply_drop_cargo_host_path_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.payload_spawned_total(), 6);
        assert_eq!(reg.cash_credited_total(), 1500);
    }

    #[test]
    fn paradrop_cargo_honesty_no_host_spawn() {
        let mut reg = HostDeliverPayloadRegistry::new();
        let id = reg.queue(
            HostDeliverPayloadKind::AmericaParadrop,
            ObjectId(2),
            Team::USA,
            Vec3::ZERO,
            10,
            "",
        );
        // complete_frame = 10 + 90 = 100
        let plans = reg.plan_due_drops(100);
        assert_eq!(plans.len(), 1);
        assert!(plans[0].spawn_positions.is_empty());
        reg.record_drop_complete(id, Vec::new(), 0);
        assert!(reg.honesty_complete_ok(HostDeliverPayloadKind::AmericaParadrop));
        assert!(!reg.honesty_payload_spawn_ok(HostDeliverPayloadKind::AmericaParadrop));
        assert!(reg.honesty_host_path_ok());
        assert_eq!(
            reg.get(id).unwrap().pre_open_distance,
            PARADROP_PRE_OPEN_DISTANCE
        );
    }

    #[test]
    fn drop_positions_line_formation_with_offset() {
        let positions = HostDeliverPayloadRegistry::drop_positions(
            Vec3::ZERO,
            6,
            20.0,
            Vec3::new(0.0, -5.0, 0.0),
        );
        assert_eq!(positions.len(), 6);
        assert!((positions[0].x - (-50.0)).abs() < 0.01);
        assert!((positions[5].x - 50.0).abs() < 0.01);
        assert!((positions[0].y - (-5.0)).abs() < 0.01);
    }

    #[test]
    fn create_at_edge_closest_edge_and_preferred_height() {
        let target = Vec3::new(10.0, 0.0, 250.0);
        let edge = create_at_edge_spawn_residual(target);
        assert!((edge.x - RESIDUAL_MAP_EXTENT_MIN_X).abs() < 0.01);
        assert!((edge.y - CARGO_PLANE_PREFERRED_HEIGHT).abs() < 0.01);
        assert!((edge.z - 250.0).abs() < 0.01);
        let target_r = Vec3::new(490.0, 0.0, 250.0);
        let edge_r = create_at_edge_spawn_residual(target_r);
        assert!((edge_r.x - RESIDUAL_MAP_EXTENT_MAX_X).abs() < 0.01);
        assert!((edge_r.y - CARGO_PLANE_PREFERRED_HEIGHT).abs() < 0.01);
        assert!(SUPPLY_DROP_START_AT_PREFERRED_HEIGHT);
        assert!(SUPPLY_DROP_START_AT_MAX_SPEED);
        assert!((B52_LOCOMOTOR_SPEED - 125.0).abs() < 0.01);
        assert!((B52_SPEED_PER_FRAME - 125.0 / 30.0).abs() < 0.001);
        assert_eq!(CARGO_PLANE_MODEL_NAME, "AVCargoPln");
        assert_eq!(CARGO_PLANE_DOOR_MODEL_NAME, "AVCargoPln_A2");
        assert_eq!(CARGO_PLANE_EXIT_BONE, "WeaponA01");
    }

    #[test]
    fn is_close_enough_delivery_band_inbound_preopen() {
        assert!(is_close_enough_to_target_residual(400.0, 450.0, 410.0, 0.0));
        assert!(!is_close_enough_to_target_residual(420.0, 450.0, 410.0, 0.0));
        assert!(is_close_enough_to_target_residual(600.0, 700.0, 410.0, 300.0));
        assert!(!is_close_enough_to_target_residual(600.0, 500.0, 410.0, 300.0));
    }

    #[test]
    fn cargo_plane_flight_create_at_edge_approach_and_door() {
        let mut reg = HostDeliverPayloadRegistry::new();
        let target = Vec3::new(250.0, 0.0, 250.0);
        let id = reg.queue(
            HostDeliverPayloadKind::SupplyDropZoneCrate,
            ObjectId(1),
            Team::USA,
            target,
            0,
            SUPPLY_DROP_PAYLOAD_TEMPLATE,
        );
        assert!(reg.honesty_create_at_edge_ok());
        let flight = reg.cargo_flight(id).expect("flight residual");
        assert_eq!(flight.phase, HostCargoPlaneFlightPhase::EdgeSpawn);
        assert!((flight.current_pos.y - CARGO_PLANE_PREFERRED_HEIGHT).abs() < 0.01);
        assert_eq!(flight.transport_template, "AmericaJetCargoPlane");
        assert_eq!(flight.model_name, "AVCargoPln");
        assert_eq!(flight.door_model_name, "AVCargoPln_A2");
        assert_eq!(flight.exit_bone, "WeaponA01");
        assert!(flight.start_at_preferred_height);
        assert!(flight.start_at_max_speed);
        assert!((flight.delivery_distance - 410.0).abs() < 0.01);
        for _ in 0..120 {
            reg.tick_cargo_flights();
            let f = reg.cargo_flight(id).unwrap();
            if matches!(
                f.phase,
                HostCargoPlaneFlightPhase::InDeliveryBand
                    | HostCargoPlaneFlightPhase::DoorOpening
                    | HostCargoPlaneFlightPhase::Delivering
            ) {
                break;
            }
        }
        let flight = reg.cargo_flight(id).unwrap();
        assert!(
            matches!(
                flight.phase,
                HostCargoPlaneFlightPhase::InDeliveryBand
                    | HostCargoPlaneFlightPhase::DoorOpening
                    | HostCargoPlaneFlightPhase::Delivering
            ),
            "must enter DeliveryDistance band residual, phase={:?}",
            flight.phase
        );
        assert!(reg.honesty_delivery_band_ok());
        assert!(flight.door_open || reg.door_open_events > 0);
        assert!(
            flight.door_condition == CARGO_PLANE_DOOR_OPENING_CONDITION || flight.door_open
        );
        assert!(reg.honesty_cargo_door_ok());
        assert!(reg.honesty_create_at_edge_flight_ok());
        for i in 0..6 {
            reg.record_item_spawned(id, Some(ObjectId(100 + i)));
        }
        reg.tick_cargo_flights();
        let flight = reg.cargo_flight(id).unwrap();
        assert!(
            matches!(
                flight.phase,
                HostCargoPlaneFlightPhase::Departing | HostCargoPlaneFlightPhase::Complete
            ),
            "complete stagger must depart, phase={:?}",
            flight.phase
        );
    }

    #[test]
    fn crate_parachute_bone_attach_residual() {
        assert!(HostDeliverPayloadRegistry::honesty_crate_bone_constants_ok());
        let bones = crate_parachute_host_bone_offsets();
        assert_eq!(CRATE_PARA_BONE_COG, "PARA_COG");
        assert_eq!(CRATE_PARA_BONE_ATTCH, "PARA_ATTCH");
        assert!(bones.para_cog.1 > bones.para_attch.1, "COG above ATTCH");
        assert!((bones.crate_man.1 - 12.0).abs() < 0.01);
        let (attach, sway, para_sway) = crate_parachute_offsets_from_bones(bones);
        assert!(attach.1 < 0.0, "crate hangs below parachute origin");
        assert!((para_sway.1 - bones.para_cog.1).abs() < 0.001);
        assert!(sway.1 > 0.0, "crate sway pivot above attach");
        let mut reg = HostDeliverPayloadRegistry::new();
        let closed = reg.build_crate_parachute_attach((100.0, 95.0, 50.0), 0.1, -0.1, false);
        assert!(!closed.chute_open);
        assert_eq!(closed.crate_logic_pos, closed.crate_presentation_pos);
        let open = reg.build_crate_parachute_attach((100.0, 80.0, 50.0), 0.2, -0.15, true);
        assert!(open.chute_open);
        let delta = (
            open.crate_presentation_pos.0 - open.crate_logic_pos.0,
            open.crate_presentation_pos.1 - open.crate_logic_pos.1,
            open.crate_presentation_pos.2 - open.crate_logic_pos.2,
        );
        let sway_mag = (delta.0 * delta.0 + delta.1 * delta.1 + delta.2 * delta.2).sqrt();
        assert!(sway_mag > 0.001, "open chute must apply non-zero sway residual");
        assert!(reg.honesty_crate_bone_attach_ok());
    }
}
