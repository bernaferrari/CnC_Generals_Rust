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
//! - Fail-closed: not full container Object / W3D bone / locomotor force matrix.
//!
//! Fail-closed honesty:
//! - Not full CreateAtEdge cargo-plane Object / DeliverPayloadAIUpdate state machine
//! - Not full PreOpenDistance approach geometry flight path (constants retained)
//! - Not full DropVariance random scatter (OCL supply drop has no DropVariance)
//! - Not full AmericaCrateParachute container Object / W3D bone attach matrix
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

/// Host residual spawn height for cargo crate (plane PreferredHeight + DropOffset Y).
pub fn cargo_crate_drop_height(drop_offset_y: f32) -> f32 {
    CARGO_PLANE_PREFERRED_HEIGHT + drop_offset_y
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
}

impl HostDeliverPayloadRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            missions: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
            items_spawned_this_frame: Vec::new(),
            flights_queued: 0,
            payload_spawned_total: 0,
            cash_credited_total: 0,
            stagger_items_total: 0,
            crate_parachute_opens: 0,
            crate_parachute_lands: 0,
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
        id
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
}
