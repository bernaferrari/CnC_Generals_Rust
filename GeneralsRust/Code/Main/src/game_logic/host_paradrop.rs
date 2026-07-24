//! Host America Paradrop / Airborne special-power residual.
//!
//! Residual slice: host `DoSpecialPower` for America Paradrop (Airborne)
//! queues a drop at the target location. After a flight/approach delay,
//! infantry units spawn near the target (line formation residual).
//!
//! Wave 70 residual pack (retail SpecialPower.ini / ObjectCreationList.ini):
//! - Special power residual: SuperweaponParadropAmerica ReloadTime **240000**ms → **7200**f,
//!   RadiusCursor **50**, RequiredScience **SCIENCE_Paradrop1**, SharedSyncedTimer **Yes**.
//! - Payload residual: SUPERWEAPON_Paradrop1 → AmericaInfantryRanger × **5**,
//!   DropDelay **150**ms → **5**f, DropSpacing **30**, approach residual **90**f,
//!   PutInContainer **AmericaParachute**, Transport **AmericaJetCargoPlane**.
//! - Honesty: `honesty_paradrop_residual_pack_ok` + layer honesty tests.
//!
//! Wave 76 residual pack (science-tier OCL matrix residual):
//! - SCIENCE_Paradrop1 → Rangers **5**, DropDelay **150**ms, OCL SUPERWEAPON_Paradrop1
//! - SCIENCE_Paradrop2 → Rangers **10**, DropDelay **80**ms, OCL SUPERWEAPON_Paradrop2
//! - SCIENCE_Paradrop3 → Rangers **20** (2×10 dual plane), DropDelay **80**ms,
//!   dual DeliverPayload planes **2**, OCL SUPERWEAPON_Paradrop3
//! - DeliveryDecal residual: SCCParadrop_USA / SHADOW_ALPHA_DECAL /
//!   Color **R:227 G:229 B:22** / OpacityMin/Max **25%/50%** / Throb **500**ms
//! - Honesty: `honesty_paradrop_science_tier_residual_pack_wave76`
//!
//! Fail-closed honesty:
//! - Not full OCL DeliverPayload cargo plane path
//! - Not full parachute containers / AmericaParachute fall physics
//! - Not live dual-plane spawn Object / multiplayer shared timer

use super::ObjectId;
use crate::command_system::SpecialPowerType;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const PARADROP_LOGIC_FPS: f32 = 30.0;

/// Residual unit count for America Paradrop1 (retail: 5 Rangers in first payload).
pub const AMERICA_PARADROP_UNIT_COUNT: u32 = 5;

/// Spacing between residual drop points (matches ParadropPower PARADROP_SPACING).
pub const PARADROP_DROP_SPACING: f32 = 30.0;

/// Residual infantry template used when retail AmericaInfantryRanger is unavailable.
pub const PARADROP_RESIDUAL_TEMPLATE: &str = "TestInfantry";

/// Preferred retail template name for America airborne residual.
pub const AMERICA_RANGER_TEMPLATE: &str = "AmericaInfantryRanger";

/// Retail SuperweaponParadropAmerica template residual.
pub const PARADROP_SPECIAL_POWER: &str = "SuperweaponParadropAmerica";
/// Retail Enum SPECIAL_PARADROP_AMERICA residual.
pub const PARADROP_SPECIAL_ENUM: &str = "SPECIAL_PARADROP_AMERICA";
/// Retail ReloadTime residual (msec).
pub const PARADROP_RELOAD_MS: u32 = 240_000;
/// ReloadTime 240000ms → 7200 frames @ 30 FPS.
pub const PARADROP_RELOAD_FRAMES: u32 = 7_200;
/// Retail RequiredScience residual.
pub const PARADROP_REQUIRED_SCIENCE: &str = "SCIENCE_Paradrop1";
/// Retail RadiusCursorRadius residual.
pub const PARADROP_RADIUS_CURSOR: f32 = 50.0;
/// Retail SharedSyncedTimer residual.
pub const PARADROP_SHARED_SYNCED_TIMER: bool = true;
/// Retail ShortcutPower residual.
pub const PARADROP_SHORTCUT_POWER: bool = true;
/// Retail SUPERWEAPON_Paradrop1 OCL residual.
pub const PARADROP_OCL: &str = "SUPERWEAPON_Paradrop1";
/// Retail AmericaJetCargoPlane transport residual.
pub const PARADROP_TRANSPORT: &str = "AmericaJetCargoPlane";
/// Retail DeliveryDistance residual (0 = drop at target residual band).
pub const PARADROP_DELIVERY_DISTANCE: f32 = 80.0;
/// Retail DropDelay residual (msec between drops).
pub const PARADROP_DROP_DELAY_MS: u32 = 150;
/// DropDelay 150ms → 5 frames @ 30 FPS.
pub const PARADROP_DROP_DELAY_FRAMES: u32 = 5;
/// Retail PutInContainer residual.
pub const PARADROP_PARACHUTE_CONTAINER: &str = "AmericaParachute";
/// Residual flight/approach delay frames (~3s host residual, not full OCL transit).
pub const PARADROP_APPROACH_DELAY_FRAMES: u32 = 90;
/// Retail MaxAttempts residual.
pub const PARADROP_MAX_ATTEMPTS: u32 = 4;

// --- Wave 76: Paradrop science-tier payload residual pack ---

/// Retail SCIENCE_Paradrop1 residual.
pub const SCIENCE_PARADROP1: &str = "SCIENCE_Paradrop1";
/// Retail SCIENCE_Paradrop2 residual.
pub const SCIENCE_PARADROP2: &str = "SCIENCE_Paradrop2";
/// Retail SCIENCE_Paradrop3 residual.
pub const SCIENCE_PARADROP3: &str = "SCIENCE_Paradrop3";
/// Retail SUPERWEAPON_Paradrop1 OCL residual.
pub const PARADROP_OCL_TIER1: &str = "SUPERWEAPON_Paradrop1";
/// Retail SUPERWEAPON_Paradrop2 OCL residual.
pub const PARADROP_OCL_TIER2: &str = "SUPERWEAPON_Paradrop2";
/// Retail SUPERWEAPON_Paradrop3 OCL residual.
pub const PARADROP_OCL_TIER3: &str = "SUPERWEAPON_Paradrop3";
/// Retail L1 Payload AmericaInfantryRanger count residual.
pub const PARADROP_RANGER_COUNT_L1: u32 = 5;
/// Retail L2 Payload AmericaInfantryRanger count residual (single plane).
pub const PARADROP_RANGER_COUNT_L2: u32 = 10;
/// Retail L3 Payload total residual (2 planes × 10 Rangers).
pub const PARADROP_RANGER_COUNT_L3: u32 = 20;
/// Retail L3 per-plane Payload residual.
pub const PARADROP_RANGER_COUNT_L3_PER_PLANE: u32 = 10;
/// Retail L3 dual DeliverPayload plane count residual.
pub const PARADROP_PLANE_COUNT_L3: u32 = 2;
/// Retail L1 DropDelay residual (msec).
pub const PARADROP_DROP_DELAY_L1_MS: u32 = 150;
/// L1 DropDelay 150ms → 5 frames @ 30 FPS.
pub const PARADROP_DROP_DELAY_L1_FRAMES: u32 = 5;
/// Retail L2/L3 DropDelay residual (msec).
pub const PARADROP_DROP_DELAY_L2_MS: u32 = 80;
/// L2/L3 DropDelay 80ms → 2 frames @ 30 FPS (round).
pub const PARADROP_DROP_DELAY_L2_FRAMES: u32 = 2;
/// Retail PreOpenDistance residual (all tiers).
pub const PARADROP_PRE_OPEN_DISTANCE: f32 = 300.0;
/// Retail DeliveryDecal Texture residual.
pub const PARADROP_DECAL_TEXTURE: &str = "SCCParadrop_USA";
/// Retail DeliveryDecal Style residual.
pub const PARADROP_DECAL_STYLE: &str = "SHADOW_ALPHA_DECAL";
/// Retail DeliveryDecal OpacityMin residual (percent).
pub const PARADROP_DECAL_OPACITY_MIN_PCT: u32 = 25;
/// Retail DeliveryDecal OpacityMax residual (percent).
pub const PARADROP_DECAL_OPACITY_MAX_PCT: u32 = 50;
/// Retail DeliveryDecal OpacityThrobTime residual (msec).
pub const PARADROP_DECAL_THROB_MS: u32 = 500;
/// Retail DeliveryDecal Color residual (R:227 G:229 B:22 A:255).
pub const PARADROP_DECAL_COLOR: (u8, u8, u8, u8) = (227, 229, 22, 255);
/// Retail DeliveryDecalRadius residual.
pub const PARADROP_DECAL_RADIUS: f32 = 50.0;

/// Residual Paradrop science tier (payload ranger count / drop delay / plane count).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParadropScienceTier {
    /// SCIENCE_Paradrop1 → 5 Rangers, DropDelay 150ms, 1 plane.
    Level1,
    /// SCIENCE_Paradrop2 → 10 Rangers, DropDelay 80ms, 1 plane.
    Level2,
    /// SCIENCE_Paradrop3 → 20 Rangers (2×10), DropDelay 80ms, 2 planes.
    Level3,
}

impl ParadropScienceTier {
    /// Retail total AmericaInfantryRanger payload count for this science tier.
    pub fn ranger_count(self) -> u32 {
        match self {
            ParadropScienceTier::Level1 => PARADROP_RANGER_COUNT_L1,
            ParadropScienceTier::Level2 => PARADROP_RANGER_COUNT_L2,
            ParadropScienceTier::Level3 => PARADROP_RANGER_COUNT_L3,
        }
    }

    /// Retail DropDelay residual (msec) for this science tier.
    pub fn drop_delay_ms(self) -> u32 {
        match self {
            ParadropScienceTier::Level1 => PARADROP_DROP_DELAY_L1_MS,
            ParadropScienceTier::Level2 | ParadropScienceTier::Level3 => PARADROP_DROP_DELAY_L2_MS,
        }
    }

    /// Retail DropDelay residual (logic frames) for this science tier.
    pub fn drop_delay_frames(self) -> u32 {
        match self {
            ParadropScienceTier::Level1 => PARADROP_DROP_DELAY_L1_FRAMES,
            ParadropScienceTier::Level2 | ParadropScienceTier::Level3 => {
                PARADROP_DROP_DELAY_L2_FRAMES
            }
        }
    }

    /// Retail DeliverPayload plane count residual for this science tier.
    pub fn plane_count(self) -> u32 {
        match self {
            ParadropScienceTier::Level1 | ParadropScienceTier::Level2 => 1,
            ParadropScienceTier::Level3 => PARADROP_PLANE_COUNT_L3,
        }
    }

    /// Retail science residual name for this tier.
    pub fn science_name(self) -> &'static str {
        match self {
            ParadropScienceTier::Level1 => SCIENCE_PARADROP1,
            ParadropScienceTier::Level2 => SCIENCE_PARADROP2,
            ParadropScienceTier::Level3 => SCIENCE_PARADROP3,
        }
    }

    /// Retail SUPERWEAPON_ParadropN OCL residual name.
    pub fn ocl_name(self) -> &'static str {
        match self {
            ParadropScienceTier::Level1 => PARADROP_OCL_TIER1,
            ParadropScienceTier::Level2 => PARADROP_OCL_TIER2,
            ParadropScienceTier::Level3 => PARADROP_OCL_TIER3,
        }
    }

    /// Map SCIENCE_Paradrop1/2/3 to tier.
    pub fn from_science_name(name: &str) -> Option<Self> {
        let n = name.to_ascii_lowercase();
        if n.contains("paradrop3") {
            Some(ParadropScienceTier::Level3)
        } else if n.contains("paradrop2") {
            Some(ParadropScienceTier::Level2)
        } else if n.contains("paradrop1") || n.contains("paradrop") {
            Some(ParadropScienceTier::Level1)
        } else {
            None
        }
    }

    /// Select highest unlocked Paradrop science tier from a science name list.
    pub fn highest_from_sciences<'a, I>(sciences: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut best = ParadropScienceTier::Level1;
        for s in sciences {
            if let Some(tier) = Self::from_science_name(s) {
                best = match (best, tier) {
                    (_, ParadropScienceTier::Level3) | (ParadropScienceTier::Level3, _) => {
                        ParadropScienceTier::Level3
                    }
                    (_, ParadropScienceTier::Level2) | (ParadropScienceTier::Level2, _) => {
                        ParadropScienceTier::Level2
                    }
                    _ => ParadropScienceTier::Level1,
                };
            }
        }
        best
    }
}

/// Host residual paradrop kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostParadropKind {
    /// USA America Airborne / SuperweaponParadropAmerica residual.
    AmericaParadrop,
}

impl HostParadropKind {
    /// Map a command-system power type to a host residual paradrop, if supported.
    pub fn from_command_power(power: &SpecialPowerType) -> Option<Self> {
        match power {
            SpecialPowerType::Paradrop
            | SpecialPowerType::InfantryParadrop
            | SpecialPowerType::TankParadrop => Some(HostParadropKind::AmericaParadrop),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HostParadropKind::AmericaParadrop => "AmericaParadrop",
        }
    }

    /// Flight / approach delay in logic frames before infantry spawn.
    /// Residual (not full cargo-plane OCL transit): ~3s @ 30 FPS.
    pub fn drop_delay_frames(self) -> u32 {
        match self {
            HostParadropKind::AmericaParadrop => 90,
        }
    }

    /// Number of residual infantry to spawn at drop time.
    pub fn unit_count(self) -> u32 {
        match self {
            HostParadropKind::AmericaParadrop => AMERICA_PARADROP_UNIT_COUNT,
        }
    }

    /// Horizontal spacing between drop points.
    pub fn drop_spacing(self) -> f32 {
        match self {
            HostParadropKind::AmericaParadrop => PARADROP_DROP_SPACING,
        }
    }

    /// Preferred unit template for this residual kind.
    pub fn unit_template(self) -> &'static str {
        match self {
            HostParadropKind::AmericaParadrop => AMERICA_RANGER_TEMPLATE,
        }
    }

    /// Audio event name queued on activation (host residual).
    pub fn activate_audio(self) -> &'static str {
        match self {
            HostParadropKind::AmericaParadrop => "SuperweaponParadrop",
        }
    }

    /// Audio event name queued when units land/spawn (host residual).
    pub fn drop_audio(self) -> &'static str {
        match self {
            HostParadropKind::AmericaParadrop => "ParadropLanding",
        }
    }
}

/// Lifecycle of a queued host paradrop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostParadropPhase {
    /// Queued after DoSpecialPower; waiting for drop frame.
    Queued,
    /// Drop resolved; infantry spawned.
    Completed,
    /// Cancelled (source died / invalid) before drop.
    Cancelled,
}

/// One pending or completed host paradrop mission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostParadropMission {
    pub id: u32,
    pub kind: HostParadropKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub activate_frame: u32,
    pub drop_frame: u32,
    pub phase: HostParadropPhase,
    /// Template used (or intended) for spawned infantry.
    pub unit_template: String,
    /// Number of units requested at queue time.
    pub unit_count: u32,
    /// Object ids of infantry successfully created at drop.
    pub spawned_unit_ids: Vec<ObjectId>,
}

/// Spawn plan for one due paradrop (computed before mutable create).
#[derive(Debug, Clone)]
pub struct HostParadropDropPlan {
    pub mission_id: u32,
    pub kind: HostParadropKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub unit_template: String,
    pub spawn_positions: Vec<Vec3>,
}

/// Host registry of paradrop missions that queue and complete.
#[derive(Debug, Clone, Default)]
pub struct HostParadropRegistry {
    next_id: u32,
    missions: HashMap<u32, HostParadropMission>,
    completed_this_frame: Vec<u32>,
    activated_this_frame: Vec<u32>,
    /// C++ AmericaJetCargoPlane DeliverPayload residual counters.
    pub transports_spawned: u32,
    /// C++ AmericaParachute containers dropped residual.
    pub parachutes_dropped: u32,
}

impl HostParadropRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            missions: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
            transports_spawned: 0,
            parachutes_dropped: 0,
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

    pub fn restore_from_snapshot(
        &mut self,
        next_id: u32,
        missions: impl IntoIterator<Item = HostParadropMission>,
    ) {
        self.clear();
        let mut max_id = 0_u32;
        for mission in missions {
            max_id = max_id.max(mission.id);
            self.missions.insert(mission.id, mission);
        }
        self.next_id = next_id.max(max_id.saturating_add(1)).max(1);
    }

    pub fn mission_count(&self) -> usize {
        self.missions.len()
    }

    pub fn pending_count(&self) -> usize {
        self.missions
            .values()
            .filter(|m| m.phase == HostParadropPhase::Queued)
            .count()
    }

    pub fn completed_count(&self) -> usize {
        self.missions
            .values()
            .filter(|m| m.phase == HostParadropPhase::Completed)
            .count()
    }

    pub fn get(&self, id: u32) -> Option<&HostParadropMission> {
        self.missions.get(&id)
    }

    pub fn missions_snapshot(&self) -> Vec<HostParadropMission> {
        let mut v: Vec<_> = self.missions.values().cloned().collect();
        v.sort_by_key(|m| m.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostParadropKind) -> Vec<&HostParadropMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostParadropPhase::Queued && m.kind == kind)
            .collect()
    }

    pub fn completed_of_kind(&self, kind: HostParadropKind) -> Vec<&HostParadropMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostParadropPhase::Completed && m.kind == kind)
            .collect()
    }

    pub fn activated_this_frame(&self) -> &[u32] {
        &self.activated_this_frame
    }

    pub fn completed_this_frame(&self) -> &[u32] {
        &self.completed_this_frame
    }

    /// Line-formation drop positions around target (matches ParadropPower default).
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

    /// Queue a paradrop mission. Returns host mission id.
    pub fn queue(
        &mut self,
        kind: HostParadropKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
        unit_template: impl Into<String>,
    ) -> u32 {
        self.queue_with_unit_count(
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            unit_template,
            kind.unit_count(),
        )
    }

    /// Queue with explicit science-tier residual unit count (Paradrop1/2/3).
    pub fn queue_with_unit_count(
        &mut self,
        kind: HostParadropKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
        unit_template: impl Into<String>,
        unit_count: u32,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let drop_frame = activate_frame.saturating_add(kind.drop_delay_frames());
        let mission = HostParadropMission {
            id,
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            drop_frame,
            phase: HostParadropPhase::Queued,
            unit_template: unit_template.into(),
            unit_count: unit_count.max(1),
            spawned_unit_ids: Vec::new(),
        };
        self.missions.insert(id, mission);
        self.activated_this_frame.push(id);
        id
    }

    /// Build drop plans for all missions whose drop frame has arrived.
    pub fn plan_due_drops(&self, current_frame: u32) -> Vec<HostParadropDropPlan> {
        let mut plans = Vec::new();
        for mission in self.missions.values() {
            if mission.phase != HostParadropPhase::Queued || current_frame < mission.drop_frame {
                continue;
            }
            let spawn_positions = Self::drop_positions(
                mission.target_position,
                mission.unit_count,
                mission.kind.drop_spacing(),
            );
            plans.push(HostParadropDropPlan {
                mission_id: mission.id,
                kind: mission.kind,
                source_object: mission.source_object,
                source_team: mission.source_team,
                target_position: mission.target_position,
                unit_template: mission.unit_template.clone(),
                spawn_positions,
            });
        }
        plans.sort_by_key(|p| p.mission_id);
        plans
    }

    /// Record drop results after GameLogic spawned units.
    pub fn record_drop_complete(&mut self, mission_id: u32, spawned_unit_ids: Vec<ObjectId>) {
        if let Some(mission) = self.missions.get_mut(&mission_id) {
            if mission.phase == HostParadropPhase::Queued {
                mission.phase = HostParadropPhase::Completed;
                mission.spawned_unit_ids = spawned_unit_ids;
                self.completed_this_frame.push(mission_id);
            }
        }
    }

    /// Cancel pending missions owned by a destroyed source object.
    pub fn cancel_for_source(&mut self, source: ObjectId) {
        for mission in self.missions.values_mut() {
            if mission.source_object == source && mission.phase == HostParadropPhase::Queued {
                mission.phase = HostParadropPhase::Cancelled;
            }
        }
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    pub fn honesty_queue_ok(&self, kind: HostParadropKind) -> bool {
        !self.pending_of_kind(kind).is_empty()
    }

    /// True if at least one mission of `kind` completed with one or more units spawned.
    pub fn honesty_complete_ok(&self, kind: HostParadropKind) -> bool {
        self.completed_of_kind(kind)
            .iter()
            .any(|m| m.phase == HostParadropPhase::Completed && !m.spawned_unit_ids.is_empty())
    }

    /// Combined host path honesty: completed drop with spawned infantry.
    pub fn honesty_host_path_ok(&self, kind: HostParadropKind) -> bool {
        self.honesty_complete_ok(kind)
    }
    /// C++ AmericaJetCargoPlane live flight residual honesty.
    pub fn honesty_cargo_plane_path_ok(&self) -> bool {
        self.transports_spawned > 0 && self.parachutes_dropped > 0
    }

}

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn paradrop_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * PARADROP_LOGIC_FPS / 1000.0).round() as u32
}

// --- Wave 70 residual honesty packs ---

/// Wave 70 residual honesty: SuperweaponParadropAmerica special-power residual peel.
pub fn honesty_paradrop_special_power_residual_ok() -> bool {
    PARADROP_SPECIAL_POWER == "SuperweaponParadropAmerica"
        && PARADROP_SPECIAL_ENUM == "SPECIAL_PARADROP_AMERICA"
        && PARADROP_REQUIRED_SCIENCE == "SCIENCE_Paradrop1"
        && PARADROP_RELOAD_MS == 240_000
        && PARADROP_RELOAD_FRAMES == paradrop_ms_to_frames(PARADROP_RELOAD_MS)
        && PARADROP_RELOAD_FRAMES == 7_200
        && (PARADROP_RADIUS_CURSOR - 50.0).abs() < 0.01
        && PARADROP_SHARED_SYNCED_TIMER
        && PARADROP_SHORTCUT_POWER
        && HostParadropKind::from_command_power(&SpecialPowerType::Paradrop)
            == Some(HostParadropKind::AmericaParadrop)
}

/// Wave 70 residual honesty: SUPERWEAPON_Paradrop1 payload residual peel.
pub fn honesty_paradrop_payload_residual_ok() -> bool {
    PARADROP_OCL == "SUPERWEAPON_Paradrop1"
        && PARADROP_TRANSPORT == "AmericaJetCargoPlane"
        && AMERICA_PARADROP_UNIT_COUNT == 5
        && AMERICA_RANGER_TEMPLATE == "AmericaInfantryRanger"
        && PARADROP_PARACHUTE_CONTAINER == "AmericaParachute"
        && PARADROP_DROP_DELAY_MS == 150
        && PARADROP_DROP_DELAY_FRAMES == paradrop_ms_to_frames(PARADROP_DROP_DELAY_MS)
        && PARADROP_DROP_DELAY_FRAMES == 5
        && (PARADROP_DROP_SPACING - 30.0).abs() < 0.01
        && PARADROP_APPROACH_DELAY_FRAMES == 90
        && HostParadropKind::AmericaParadrop.drop_delay_frames() == 90
        && HostParadropKind::AmericaParadrop.unit_count() == 5
        && PARADROP_MAX_ATTEMPTS == 4
        && HostParadropKind::AmericaParadrop.activate_audio() == "SuperweaponParadrop"
        && HostParadropKind::AmericaParadrop.drop_audio() == "ParadropLanding"
        && {
            let positions = HostParadropRegistry::drop_positions(glam::Vec3::ZERO, 5, 30.0);
            positions.len() == 5 && (positions[2].x).abs() < 0.01
        }
}

/// Combined Wave 70 Paradrop residual honesty pack.
pub fn honesty_paradrop_residual_pack_ok() -> bool {
    honesty_paradrop_special_power_residual_ok() && honesty_paradrop_payload_residual_ok()
}

/// Wave 76 residual honesty: SCIENCE_Paradrop1/2/3 payload / plane / drop-delay pack.
///
/// ObjectCreationList.ini SUPERWEAPON_Paradrop1/2/3 residual:
/// Rangers **5/10/20**, DropDelay **150/80/80**ms, L3 dual planes **2**,
/// DeliveryDecal SCCParadrop_USA residual.
/// Fail-closed: not full dual-plane DeliverPayload flight Object.
pub fn honesty_paradrop_science_tier_residual_pack_wave76() -> bool {
    SCIENCE_PARADROP1 == "SCIENCE_Paradrop1"
        && SCIENCE_PARADROP2 == "SCIENCE_Paradrop2"
        && SCIENCE_PARADROP3 == "SCIENCE_Paradrop3"
        && PARADROP_OCL_TIER1 == "SUPERWEAPON_Paradrop1"
        && PARADROP_OCL_TIER2 == "SUPERWEAPON_Paradrop2"
        && PARADROP_OCL_TIER3 == "SUPERWEAPON_Paradrop3"
        && ParadropScienceTier::Level1.ranger_count() == 5
        && ParadropScienceTier::Level2.ranger_count() == 10
        && ParadropScienceTier::Level3.ranger_count() == 20
        && PARADROP_RANGER_COUNT_L3
            == PARADROP_RANGER_COUNT_L3_PER_PLANE * PARADROP_PLANE_COUNT_L3
        && ParadropScienceTier::Level1.plane_count() == 1
        && ParadropScienceTier::Level2.plane_count() == 1
        && ParadropScienceTier::Level3.plane_count() == 2
        && ParadropScienceTier::Level1.drop_delay_ms() == 150
        && ParadropScienceTier::Level2.drop_delay_ms() == 80
        && ParadropScienceTier::Level3.drop_delay_ms() == 80
        && ParadropScienceTier::Level1.drop_delay_frames() == 5
        && ParadropScienceTier::Level2.drop_delay_frames() == 2
        && ParadropScienceTier::Level3.drop_delay_frames() == 2
        && paradrop_ms_to_frames(PARADROP_DROP_DELAY_L1_MS) == PARADROP_DROP_DELAY_L1_FRAMES
        && paradrop_ms_to_frames(PARADROP_DROP_DELAY_L2_MS) == PARADROP_DROP_DELAY_L2_FRAMES
        && ParadropScienceTier::Level1.science_name() == SCIENCE_PARADROP1
        && ParadropScienceTier::Level2.science_name() == SCIENCE_PARADROP2
        && ParadropScienceTier::Level3.science_name() == SCIENCE_PARADROP3
        && ParadropScienceTier::Level1.ocl_name() == PARADROP_OCL_TIER1
        && ParadropScienceTier::Level2.ocl_name() == PARADROP_OCL_TIER2
        && ParadropScienceTier::Level3.ocl_name() == PARADROP_OCL_TIER3
        && (PARADROP_PRE_OPEN_DISTANCE - 300.0).abs() < 0.01
        && PARADROP_DECAL_TEXTURE == "SCCParadrop_USA"
        && PARADROP_DECAL_STYLE == "SHADOW_ALPHA_DECAL"
        && PARADROP_DECAL_OPACITY_MIN_PCT == 25
        && PARADROP_DECAL_OPACITY_MAX_PCT == 50
        && PARADROP_DECAL_THROB_MS == 500
        && PARADROP_DECAL_COLOR == (227, 229, 22, 255)
        && (PARADROP_DECAL_RADIUS - 50.0).abs() < 0.01
        && (PARADROP_DECAL_RADIUS - PARADROP_RADIUS_CURSOR).abs() < 0.01
        && ParadropScienceTier::from_science_name("SCIENCE_Paradrop1")
            == Some(ParadropScienceTier::Level1)
        && ParadropScienceTier::from_science_name("SCIENCE_Paradrop2")
            == Some(ParadropScienceTier::Level2)
        && ParadropScienceTier::from_science_name("SCIENCE_Paradrop3")
            == Some(ParadropScienceTier::Level3)
        && ParadropScienceTier::highest_from_sciences([
            "SCIENCE_Paradrop1",
            "SCIENCE_Paradrop3",
        ]) == ParadropScienceTier::Level3
        // Wave 70 L1 residual still matches science-tier Level1.
        && AMERICA_PARADROP_UNIT_COUNT == PARADROP_RANGER_COUNT_L1
        && PARADROP_DROP_DELAY_MS == PARADROP_DROP_DELAY_L1_MS
        && PARADROP_REQUIRED_SCIENCE == SCIENCE_PARADROP1
        && PARADROP_OCL == PARADROP_OCL_TIER1
}

/// Combined Wave 76 Paradrop residual honesty (L1 pack + science-tier pack).
pub fn honesty_paradrop_residual_pack_wave76_ok() -> bool {
    honesty_paradrop_residual_pack_ok() && honesty_paradrop_science_tier_residual_pack_wave76()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn paradrop_maps_from_command_power() {
        assert_eq!(
            HostParadropKind::from_command_power(&SpecialPowerType::Paradrop),
            Some(HostParadropKind::AmericaParadrop)
        );
        assert_eq!(
            HostParadropKind::from_command_power(&SpecialPowerType::Airstrike),
            None
        );
        assert_eq!(
            HostParadropKind::from_command_power(&SpecialPowerType::DaisyCutter),
            None
        );
    }

    #[test]
    fn queue_and_complete_paradrop_drop_plan() {
        let mut reg = HostParadropRegistry::new();
        let id = reg.queue(
            HostParadropKind::AmericaParadrop,
            ObjectId(1),
            Team::USA,
            Vec3::new(100.0, 0.0, 50.0),
            0,
            PARADROP_RESIDUAL_TEMPLATE,
        );
        assert!(reg.honesty_queue_ok(HostParadropKind::AmericaParadrop));
        assert!(!reg.honesty_complete_ok(HostParadropKind::AmericaParadrop));

        let mission = reg.get(id).expect("mission");
        assert_eq!(mission.drop_frame, 90);
        assert_eq!(mission.phase, HostParadropPhase::Queued);
        assert_eq!(mission.unit_count, AMERICA_PARADROP_UNIT_COUNT);

        assert!(reg.plan_due_drops(89).is_empty());

        let plans = reg.plan_due_drops(90);
        assert_eq!(plans.len(), 1);
        assert_eq!(
            plans[0].spawn_positions.len(),
            AMERICA_PARADROP_UNIT_COUNT as usize
        );
        // Center of line should be near target x.
        let mid = plans[0].spawn_positions[2];
        assert!((mid.x - 100.0).abs() < 0.1);
        assert!((mid.z - 50.0).abs() < 0.1);

        let spawned = vec![
            ObjectId(10),
            ObjectId(11),
            ObjectId(12),
            ObjectId(13),
            ObjectId(14),
        ];
        reg.record_drop_complete(id, spawned.clone());
        assert!(reg.honesty_complete_ok(HostParadropKind::AmericaParadrop));
        assert!(reg.honesty_host_path_ok(HostParadropKind::AmericaParadrop));
        assert_eq!(reg.get(id).unwrap().phase, HostParadropPhase::Completed);
        assert_eq!(reg.get(id).unwrap().spawned_unit_ids, spawned);
    }

    #[test]
    fn drop_positions_line_formation() {
        let positions = HostParadropRegistry::drop_positions(Vec3::ZERO, 5, 30.0);
        assert_eq!(positions.len(), 5);
        assert!((positions[0].x - (-60.0)).abs() < 0.01);
        assert!((positions[4].x - 60.0).abs() < 0.01);
        assert!((positions[2].x).abs() < 0.01);
    }

    #[test]
    fn restore_from_snapshot_keeps_pending_drop_frame() {
        let mut reg = HostParadropRegistry::new();
        let id = reg.queue(
            HostParadropKind::AmericaParadrop,
            ObjectId(9),
            Team::USA,
            Vec3::new(1.0, 0.0, 2.0),
            10,
            PARADROP_RESIDUAL_TEMPLATE,
        );
        let snap = reg.missions_snapshot();
        let next = reg.next_id();

        let mut loaded = HostParadropRegistry::new();
        loaded.restore_from_snapshot(next, snap);
        assert_eq!(loaded.pending_count(), 1);
        let m = loaded.get(id).expect("restored mission");
        assert_eq!(m.drop_frame, 100);
        assert_eq!(m.phase, HostParadropPhase::Queued);
        assert_eq!(loaded.next_id(), next);
    }

    #[test]
    fn paradrop_residual_pack_honesty_wave70() {
        assert!(honesty_paradrop_special_power_residual_ok());
        assert!(honesty_paradrop_payload_residual_ok());
        assert!(honesty_paradrop_residual_pack_ok());
        assert_eq!(paradrop_ms_to_frames(240_000), 7_200);
        assert_eq!(paradrop_ms_to_frames(150), 5);
        assert_eq!(AMERICA_PARADROP_UNIT_COUNT, 5);
        assert_eq!(PARADROP_OCL, "SUPERWEAPON_Paradrop1");
        assert_eq!(PARADROP_REQUIRED_SCIENCE, "SCIENCE_Paradrop1");
        assert!((PARADROP_RADIUS_CURSOR - 50.0).abs() < 0.01);
        assert!(PARADROP_SHARED_SYNCED_TIMER);
    }

    /// Wave 76 residual: SCIENCE_Paradrop1/2/3 payload / plane / decal pack.
    #[test]
    fn paradrop_science_tier_residual_pack_wave76_honesty() {
        assert!(honesty_paradrop_science_tier_residual_pack_wave76());
        assert!(honesty_paradrop_residual_pack_wave76_ok());
        assert_eq!(ParadropScienceTier::Level1.ranger_count(), 5);
        assert_eq!(ParadropScienceTier::Level2.ranger_count(), 10);
        assert_eq!(ParadropScienceTier::Level3.ranger_count(), 20);
        assert_eq!(ParadropScienceTier::Level3.plane_count(), 2);
        assert_eq!(paradrop_ms_to_frames(80), 2);
        assert_eq!(PARADROP_DECAL_TEXTURE, "SCCParadrop_USA");
        assert_eq!(PARADROP_DECAL_COLOR, (227, 229, 22, 255));
        assert_eq!(
            ParadropScienceTier::highest_from_sciences([SCIENCE_PARADROP1, SCIENCE_PARADROP2,]),
            ParadropScienceTier::Level2
        );
        // L3 total = 2 planes × 10 rangers.
        assert_eq!(
            PARADROP_RANGER_COUNT_L3,
            PARADROP_RANGER_COUNT_L3_PER_PLANE * PARADROP_PLANE_COUNT_L3
        );
    }
}
