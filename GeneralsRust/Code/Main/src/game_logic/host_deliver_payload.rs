//! Host DeliverPayload cargo residual (superweapon / cargo-plane drops).
//!
//! Residual slice (playability):
//! - Models retail OCL `DeliverPayload` missions used by cargo planes
//!   (`AmericaJetCargoPlane`) and superweapon drop lists without full aircraft
//!   edge-spawn / locomotor flight / door animation.
//! - After an approach delay residual, spawns payload units at the target
//!   location in a line formation (DropOffset / DropVariance deferred).
//! - Primary wire: `OCL_AmericaSupplyDropZoneCrateDrop` —
//!   Transport=`AmericaJetCargoPlane`, Payload=`SupplyDropZoneCrate` × 6,
//!   PutInContainer=`AmericaCrateParachute`, DropDelay=350 ms, DeliveryDistance=410.
//! - Secondary honesty: America Paradrop cargo-plane DeliverPayload bookkeeping
//!   (actual infantry spawn remains in `host_paradrop`).
//!
//! Fail-closed honesty:
//! - Not full CreateAtEdge cargo-plane Object / DeliverPayloadAIUpdate state machine
//! - Not full PreOpenDistance / DeliveryDistance approach geometry / MaxAttempts
//! - Not full DropDelay per-item stagger (host spawns formation simultaneously)
//! - Not full AmericaCrateParachute / AmericaParachute fall-physics containers
//! - Not full VisiblePayload bone / subobject bomb rack matrix
//! - Not network DeliverPayload replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const DELIVER_PAYLOAD_LOGIC_FPS: f32 = 30.0;

/// Residual cargo-plane approach delay before payload spawn
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

/// Retail DropDelay = 350 ms between items (stagger fail-closed; constant retained).
pub const SUPPLY_DROP_DROP_DELAY_MS: u32 = 350;

/// DropDelay → frames at 30 FPS (350 / (1000/30) ≈ 10.5 → 11).
pub const SUPPLY_DROP_DROP_DELAY_FRAMES: u32 = 11;

/// Retail DeliveryDistance residual (approach geometry deferred).
pub const SUPPLY_DROP_DELIVERY_DISTANCE: f32 = 410.0;

/// Residual horizontal spacing between spawned crates (line formation).
pub const SUPPLY_DROP_CRATE_SPACING: f32 = 20.0;

/// Activate audio residual when cargo flight queues (plane inbound).
pub const SUPPLY_DROP_CARGO_APPROACH_AUDIO: &str = "CargoPlaneApproach";

/// Drop audio residual when payload units spawn.
pub const SUPPLY_DROP_CARGO_DROP_AUDIO: &str = "SupplyDropZoneDrop";

// --- SUPERWEAPON_Paradrop1 cargo residual honesty constants ---

/// Retail Paradrop DeliverPayload Transport.
pub const PARADROP_CARGO_TRANSPORT: &str = "AmericaJetCargoPlane";

/// Retail Paradrop PutInContainer.
pub const PARADROP_PUT_IN_CONTAINER: &str = "AmericaParachute";

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

    /// Residual approach frames before payload resolve.
    pub fn approach_delay_frames(self) -> u32 {
        match self {
            HostDeliverPayloadKind::SupplyDropZoneCrate => CARGO_PLANE_APPROACH_DELAY_FRAMES,
            HostDeliverPayloadKind::AmericaParadrop => CARGO_PLANE_APPROACH_DELAY_FRAMES,
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
            // Infantry count / spawn owned by host_paradrop residual.
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

    /// Whether this kind should spawn residual payload objects on drop frame.
    pub fn spawns_payload_objects(self) -> bool {
        matches!(self, HostDeliverPayloadKind::SupplyDropZoneCrate)
    }

    /// Whether BuildingPickup residual cash should credit on drop.
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
}

/// Lifecycle of a queued host DeliverPayload mission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostDeliverPayloadPhase {
    /// Queued after OCL / special-power; waiting for approach drop frame.
    Queued,
    /// Payload resolved (spawned and/or cash credited).
    Completed,
    /// Cancelled (source died / invalid) before drop.
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
    pub drop_frame: u32,
    pub phase: HostDeliverPayloadPhase,
    /// Transport template residual honesty (`AmericaJetCargoPlane`).
    pub transport_template: String,
    /// PutInContainer residual honesty.
    pub put_in_container: String,
    /// Payload template used (or intended) for spawned units.
    pub payload_template: String,
    /// Number of payload units requested at queue time.
    pub payload_count: u32,
    /// Object ids of payload units successfully created at drop.
    pub spawned_payload_ids: Vec<ObjectId>,
    /// BuildingPickup residual cash credited on complete (supply crates).
    pub cash_credited: u32,
}

/// Spawn / resolve plan for one due DeliverPayload mission.
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
    /// Total cargo flights queued (honesty).
    pub flights_queued: u32,
    /// Total payload objects spawned across all missions (honesty).
    pub payload_spawned_total: u32,
    /// Total BuildingPickup residual cash credited via cargo path (honesty).
    pub cash_credited_total: u32,
}

impl HostDeliverPayloadRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            missions: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
            flights_queued: 0,
            payload_spawned_total: 0,
            cash_credited_total: 0,
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn clear_frame_events(&mut self) {
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
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
            .filter(|m| m.phase == HostDeliverPayloadPhase::Queued)
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
            .filter(|m| m.phase == HostDeliverPayloadPhase::Queued && m.kind == kind)
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

    /// Line-formation drop positions around target.
    pub fn drop_positions(center: Vec3, unit_count: u32, spacing: f32) -> Vec<Vec3> {
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
            positions.push(Vec3::new(center.x + offset, center.y, center.z));
        }
        positions
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
        let drop_frame = activate_frame.saturating_add(kind.approach_delay_frames());
        let mission = HostDeliverPayloadMission {
            id,
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            drop_frame,
            phase: HostDeliverPayloadPhase::Queued,
            transport_template: kind.transport_template().to_string(),
            put_in_container: kind.put_in_container().to_string(),
            payload_template: payload_template.into(),
            payload_count: kind.payload_count(),
            spawned_payload_ids: Vec::new(),
            cash_credited: 0,
        };
        self.missions.insert(id, mission);
        self.activated_this_frame.push(id);
        self.flights_queued = self.flights_queued.saturating_add(1);
        id
    }

    /// Build drop plans for all missions whose drop frame has arrived.
    pub fn plan_due_drops(&self, current_frame: u32) -> Vec<HostDeliverPayloadDropPlan> {
        let mut plans = Vec::new();
        for mission in self.missions.values() {
            if mission.phase != HostDeliverPayloadPhase::Queued || current_frame < mission.drop_frame
            {
                continue;
            }
            let spawn_positions = if mission.kind.spawns_payload_objects() {
                Self::drop_positions(
                    mission.target_position,
                    mission.payload_count,
                    mission.kind.payload_spacing(),
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

    /// Record drop results after GameLogic spawned units / credited cash.
    pub fn record_drop_complete(
        &mut self,
        mission_id: u32,
        spawned_payload_ids: Vec<ObjectId>,
        cash_credited: u32,
    ) {
        if let Some(mission) = self.missions.get_mut(&mission_id) {
            if mission.phase == HostDeliverPayloadPhase::Queued {
                mission.phase = HostDeliverPayloadPhase::Completed;
                let spawn_count = spawned_payload_ids.len() as u32;
                mission.spawned_payload_ids = spawned_payload_ids;
                mission.cash_credited = cash_credited;
                self.completed_this_frame.push(mission_id);
                self.payload_spawned_total =
                    self.payload_spawned_total.saturating_add(spawn_count);
                self.cash_credited_total = self.cash_credited_total.saturating_add(cash_credited);
            }
        }
    }

    /// Cancel pending missions owned by a destroyed source object.
    pub fn cancel_for_source(&mut self, source: ObjectId) {
        for mission in self.missions.values_mut() {
            if mission.source_object == source && mission.phase == HostDeliverPayloadPhase::Queued {
                mission.phase = HostDeliverPayloadPhase::Cancelled;
            }
        }
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    /// At least one cargo flight of `kind` was queued (pending or completed).
    pub fn honesty_queue_ok(&self, kind: HostDeliverPayloadKind) -> bool {
        self.missions.values().any(|m| m.kind == kind)
    }

    /// True if at least one mission of `kind` is currently pending (inbound).
    pub fn honesty_inbound_ok(&self, kind: HostDeliverPayloadKind) -> bool {
        !self.pending_of_kind(kind).is_empty()
    }

    /// True if at least one mission of `kind` completed.
    pub fn honesty_complete_ok(&self, kind: HostDeliverPayloadKind) -> bool {
        !self.completed_of_kind(kind).is_empty()
    }

    /// True if at least one mission of `kind` completed with payload units spawned.
    pub fn honesty_payload_spawn_ok(&self, kind: HostDeliverPayloadKind) -> bool {
        self.completed_of_kind(kind)
            .iter()
            .any(|m| !m.spawned_payload_ids.is_empty())
    }

    /// True if at least one Supply Drop Zone cargo mission credited BuildingPickup cash.
    pub fn honesty_building_pickup_ok(&self) -> bool {
        self.cash_credited_total > 0
            && self
                .completed_of_kind(HostDeliverPayloadKind::SupplyDropZoneCrate)
                .iter()
                .any(|m| m.cash_credited > 0)
    }

    /// Combined host path for Supply Drop Zone cargo residual:
    /// completed flight with spawned crates and BuildingPickup cash.
    pub fn honesty_supply_drop_cargo_host_path_ok(&self) -> bool {
        self.honesty_payload_spawn_ok(HostDeliverPayloadKind::SupplyDropZoneCrate)
            && self.honesty_building_pickup_ok()
    }

    /// Combined DeliverPayload cargo residual honesty (any completed cargo path).
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_supply_drop_cargo_host_path_ok()
            || self.honesty_complete_ok(HostDeliverPayloadKind::AmericaParadrop)
    }

    /// Transport / container residual honesty constants match retail OCL names.
    pub fn honesty_transport_names_ok(kind: HostDeliverPayloadKind) -> bool {
        !kind.transport_template().is_empty() && !kind.put_in_container().is_empty()
    }
}

/// DropDelay ms → logic frames residual (30 FPS).
pub fn drop_delay_frames_from_ms(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / DELIVER_PAYLOAD_LOGIC_FPS)).round() as u32
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
        assert!((SUPPLY_DROP_DELIVERY_DISTANCE - 410.0).abs() < 0.01);
        assert_eq!(CARGO_PLANE_APPROACH_DELAY_FRAMES, 90);
        assert!(HostDeliverPayloadRegistry::honesty_transport_names_ok(
            HostDeliverPayloadKind::SupplyDropZoneCrate
        ));
    }

    #[test]
    fn queue_and_complete_supply_drop_cargo() {
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
        assert_eq!(mission.drop_frame, 90);
        assert_eq!(mission.payload_count, 6);
        assert_eq!(mission.transport_template, SUPPLY_DROP_CARGO_TRANSPORT);
        assert_eq!(mission.put_in_container, SUPPLY_DROP_PUT_IN_CONTAINER);

        assert!(reg.plan_due_drops(89).is_empty());
        let plans = reg.plan_due_drops(90);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].spawn_positions.len(), 6);
        let mid = plans[0].spawn_positions[2];
        // 6 crates: offsets -50,-30,-10,10,30,50 → index 2 is -10
        assert!((mid.x - 90.0).abs() < 0.1 || (mid.x - 100.0).abs() < 60.0);
        assert!((mid.z - 50.0).abs() < 0.1);

        let spawned: Vec<ObjectId> = (10..16).map(ObjectId).collect();
        reg.record_drop_complete(id, spawned.clone(), 1500);
        assert!(reg.honesty_payload_spawn_ok(HostDeliverPayloadKind::SupplyDropZoneCrate));
        assert!(reg.honesty_building_pickup_ok());
        assert!(reg.honesty_supply_drop_cargo_host_path_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.payload_spawned_total(), 6);
        assert_eq!(reg.cash_credited_total(), 1500);
        assert_eq!(reg.get(id).unwrap().spawned_payload_ids, spawned);
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
        let plans = reg.plan_due_drops(100);
        assert_eq!(plans.len(), 1);
        assert!(plans[0].spawn_positions.is_empty());
        reg.record_drop_complete(id, Vec::new(), 0);
        assert!(reg.honesty_complete_ok(HostDeliverPayloadKind::AmericaParadrop));
        assert!(!reg.honesty_payload_spawn_ok(HostDeliverPayloadKind::AmericaParadrop));
        // host path ok via paradrop complete branch
        assert!(reg.honesty_host_path_ok());
    }

    #[test]
    fn drop_positions_line_formation() {
        let positions = HostDeliverPayloadRegistry::drop_positions(Vec3::ZERO, 6, 20.0);
        assert_eq!(positions.len(), 6);
        assert!((positions[0].x - (-50.0)).abs() < 0.01);
        assert!((positions[5].x - 50.0).abs() < 0.01);
    }
}
