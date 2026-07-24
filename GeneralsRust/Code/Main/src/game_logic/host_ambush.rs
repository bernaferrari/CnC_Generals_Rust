//! Host GLA Rebel Ambush special-power residual.
//!
//! Residual slice: host `DoSpecialPower` for SPECIAL_AMBUSH / SuperweaponRebelAmbush
//! queues a spawn at the target location. After a residual fade/approach delay
//! (retail OCL FadeTime = 3000 ms), infantry units spawn near the target
//! (spread-formation residual).
//!
//! Fail-closed honesty:
//! - FadeIn residual: spawned rebels STEALTHED until FadeTime elapses
//! - Science tier Ambush1/2/3 payload counts via AmbushScienceTier residual
//! - DiesOnBadLand residual: underwater/cliff spawn cells kill rebels (drown)
//! - Fail-closed: not full OCLAdjustPositionToPassable snap-to-passable path
//! - Not SharedSyncedTimer / multiplayer academy classification

use super::ObjectId;
use crate::command_system::SpecialPowerType;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const AMBUSH_LOGIC_FPS: f32 = 30.0;

/// Residual unit count for Rebel Ambush1 (retail SUPERWEAPON_RebelAmbush1 Count = 4).
pub const GLA_AMBUSH1_UNIT_COUNT: u32 = 4;

/// Residual scatter radius around target (RadiusCursorRadius = 50 residual).
pub const AMBUSH_SPAWN_RADIUS: f32 = 40.0;

/// Residual infantry template used when retail GLAInfantryRebel is unavailable.
pub const AMBUSH_RESIDUAL_TEMPLATE: &str = "TestInfantry";

/// Preferred retail template name for GLA ambush residual.
pub const GLA_REBEL_TEMPLATE: &str = "GLAInfantryRebel";

/// Host residual ambush kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostAmbushKind {
    /// GLA Rebel Ambush / SuperweaponRebelAmbush residual (Ambush1 payload).
    GLARebelAmbush,
}

impl HostAmbushKind {
    /// Map a command-system power type to a host residual ambush, if supported.
    pub fn from_command_power(power: &SpecialPowerType) -> Option<Self> {
        match power {
            SpecialPowerType::Ambush | SpecialPowerType::TerrorCell => {
                Some(HostAmbushKind::GLARebelAmbush)
            }
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HostAmbushKind::GLARebelAmbush => "GLARebelAmbush",
        }
    }

    /// Fade / approach delay in logic frames before infantry spawn.
    /// Residual of retail OCL FadeTime = 3000 ms → ~3s @ 30 FPS = 90 frames.
    pub fn spawn_delay_frames(self) -> u32 {
        match self {
            HostAmbushKind::GLARebelAmbush => 90,
        }
    }

    /// Number of residual infantry to spawn at ambush time.
    pub fn unit_count(self) -> u32 {
        match self {
            HostAmbushKind::GLARebelAmbush => GLA_AMBUSH1_UNIT_COUNT,
        }
    }

    /// Scatter radius for residual spawn positions.
    pub fn spawn_radius(self) -> f32 {
        match self {
            HostAmbushKind::GLARebelAmbush => AMBUSH_SPAWN_RADIUS,
        }
    }

    /// Preferred unit template for this residual kind.
    pub fn unit_template(self) -> &'static str {
        match self {
            HostAmbushKind::GLARebelAmbush => GLA_REBEL_TEMPLATE,
        }
    }

    /// Audio event name queued on activation (host residual).
    pub fn activate_audio(self) -> &'static str {
        match self {
            HostAmbushKind::GLARebelAmbush => "RebelAmbushActivated",
        }
    }

    /// Audio event name queued when units appear (host residual).
    pub fn spawn_audio(self) -> &'static str {
        match self {
            HostAmbushKind::GLARebelAmbush => "RebelAmbushSpawn",
        }
    }
}

/// Lifecycle of a queued host ambush.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostAmbushPhase {
    /// Queued after DoSpecialPower; waiting for spawn frame.
    Queued,
    /// Spawn resolved; infantry created near target.
    Completed,
    /// Cancelled (source died / invalid) before spawn.
    Cancelled,
}

/// One pending or completed host ambush mission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostAmbushMission {
    pub id: u32,
    pub kind: HostAmbushKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub activate_frame: u32,
    pub spawn_frame: u32,
    pub phase: HostAmbushPhase,
    /// Template used (or intended) for spawned infantry.
    pub unit_template: String,
    /// Number of units requested at queue time.
    pub unit_count: u32,
    /// Object ids of infantry successfully created at spawn.
    pub spawned_unit_ids: Vec<ObjectId>,
}

/// Spawn plan for one due ambush (computed before mutable create).
#[derive(Debug, Clone)]
pub struct HostAmbushSpawnPlan {
    pub mission_id: u32,
    pub kind: HostAmbushKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub unit_template: String,
    pub spawn_positions: Vec<Vec3>,
}

/// Pending FadeIn residual clear (object becomes visible after FadeTime).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAmbushFadeClear {
    pub object_id: ObjectId,
    pub clear_frame: u32,
}

/// Host registry of ambush missions that queue and complete.
#[derive(Debug, Clone, Default)]
pub struct HostAmbushRegistry {
    next_id: u32,
    missions: HashMap<u32, HostAmbushMission>,
    completed_this_frame: Vec<u32>,
    activated_this_frame: Vec<u32>,
    /// Units currently FadeIn-stealthed residual.
    pub pending_fade_clears: Vec<PendingAmbushFadeClear>,
    /// Honesty: FadeIn stealth grants applied.
    pub fade_in_grants: u32,
    /// Honesty: FadeIn clears completed.
    pub fade_in_clears: u32,
    /// Honesty: DiesOnBadLand kills applied.
    pub dies_on_bad_land_kills: u32,
}

impl HostAmbushRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            missions: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
            pending_fade_clears: Vec::new(),
            fade_in_grants: 0,
            fade_in_clears: 0,
            dies_on_bad_land_kills: 0,
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    
    pub fn schedule_fade_in(&mut self, object_id: ObjectId, spawn_frame: u32) {
        if !AMBUSH_FADE_IN {
            return;
        }
        self.pending_fade_clears.push(PendingAmbushFadeClear {
            object_id,
            clear_frame: spawn_frame.saturating_add(AMBUSH_FADE_TIME_FRAMES),
        });
        self.fade_in_grants = self.fade_in_grants.saturating_add(1);
    }

    pub fn take_due_fade_clears(&mut self, frame: u32) -> Vec<ObjectId> {
        let mut due = Vec::new();
        let mut keep = Vec::new();
        for p in self.pending_fade_clears.drain(..) {
            if p.clear_frame <= frame {
                due.push(p.object_id);
            } else {
                keep.push(p);
            }
        }
        self.pending_fade_clears = keep;
        if !due.is_empty() {
            self.fade_in_clears = self.fade_in_clears.saturating_add(due.len() as u32);
        }
        due
    }

    pub fn honesty_fade_in_ok(&self) -> bool {
        AMBUSH_FADE_IN
            && self.fade_in_grants > 0
            && (self.fade_in_clears > 0 || !self.pending_fade_clears.is_empty())
    }

    pub fn record_dies_on_bad_land_kill(&mut self) {
        self.dies_on_bad_land_kills = self.dies_on_bad_land_kills.saturating_add(1);
    }

    pub fn honesty_dies_on_bad_land_ok(&self) -> bool {
        AMBUSH_DIES_ON_BAD_LAND && self.dies_on_bad_land_kills > 0
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
        missions: impl IntoIterator<Item = HostAmbushMission>,
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
            .filter(|m| m.phase == HostAmbushPhase::Queued)
            .count()
    }

    pub fn completed_count(&self) -> usize {
        self.missions
            .values()
            .filter(|m| m.phase == HostAmbushPhase::Completed)
            .count()
    }

    pub fn get(&self, id: u32) -> Option<&HostAmbushMission> {
        self.missions.get(&id)
    }

    pub fn missions_snapshot(&self) -> Vec<HostAmbushMission> {
        let mut v: Vec<_> = self.missions.values().cloned().collect();
        v.sort_by_key(|m| m.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostAmbushKind) -> Vec<&HostAmbushMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostAmbushPhase::Queued && m.kind == kind)
            .collect()
    }

    pub fn completed_of_kind(&self, kind: HostAmbushKind) -> Vec<&HostAmbushMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostAmbushPhase::Completed && m.kind == kind)
            .collect()
    }

    pub fn activated_this_frame(&self) -> &[u32] {
        &self.activated_this_frame
    }

    pub fn completed_this_frame(&self) -> &[u32] {
        &self.completed_this_frame
    }

    /// Circular scatter spawn positions around target (SpreadFormation residual).
    pub fn spawn_positions(center: Vec3, unit_count: u32, radius: f32) -> Vec<Vec3> {
        if unit_count == 0 {
            return Vec::new();
        }
        let n = unit_count as usize;
        let mut positions = Vec::with_capacity(n);
        let angle_step = std::f32::consts::TAU / n as f32;
        for i in 0..n {
            let angle = i as f32 * angle_step;
            // Alternate inner/outer ring residual so units are not stacked.
            let dist = if n == 1 {
                0.0
            } else if i % 2 == 0 {
                radius * 0.6
            } else {
                radius
            };
            positions.push(Vec3::new(
                center.x + angle.cos() * dist,
                center.y,
                center.z + angle.sin() * dist,
            ));
        }
        positions
    }

    /// Queue an ambush mission. Returns host mission id.
    pub fn queue(
        &mut self,
        kind: HostAmbushKind,
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

    /// Queue with explicit science-tier residual unit count (Ambush1/2/3).
    pub fn queue_with_unit_count(
        &mut self,
        kind: HostAmbushKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
        unit_template: impl Into<String>,
        unit_count: u32,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let spawn_frame = activate_frame.saturating_add(kind.spawn_delay_frames());
        let mission = HostAmbushMission {
            id,
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            spawn_frame,
            phase: HostAmbushPhase::Queued,
            unit_template: unit_template.into(),
            unit_count: unit_count.max(1),
            spawned_unit_ids: Vec::new(),
        };
        self.missions.insert(id, mission);
        self.activated_this_frame.push(id);
        id
    }

    /// Build spawn plans for all missions whose spawn frame has arrived.
    pub fn plan_due_spawns(&self, current_frame: u32) -> Vec<HostAmbushSpawnPlan> {
        let mut plans = Vec::new();
        for mission in self.missions.values() {
            if mission.phase != HostAmbushPhase::Queued || current_frame < mission.spawn_frame {
                continue;
            }
            let spawn_positions = Self::spawn_positions(
                mission.target_position,
                mission.unit_count,
                mission.kind.spawn_radius(),
            );
            plans.push(HostAmbushSpawnPlan {
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

    /// Record spawn results after GameLogic created units.
    pub fn record_spawn_complete(&mut self, mission_id: u32, spawned_unit_ids: Vec<ObjectId>) {
        if let Some(mission) = self.missions.get_mut(&mission_id) {
            if mission.phase == HostAmbushPhase::Queued {
                mission.phase = HostAmbushPhase::Completed;
                mission.spawned_unit_ids = spawned_unit_ids;
                self.completed_this_frame.push(mission_id);
            }
        }
    }

    /// Cancel pending missions owned by a destroyed source object.
    pub fn cancel_for_source(&mut self, source: ObjectId) {
        for mission in self.missions.values_mut() {
            if mission.source_object == source && mission.phase == HostAmbushPhase::Queued {
                mission.phase = HostAmbushPhase::Cancelled;
            }
        }
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    pub fn honesty_queue_ok(&self, kind: HostAmbushKind) -> bool {
        !self.pending_of_kind(kind).is_empty()
    }

    /// True if at least one mission of `kind` completed with one or more units spawned.
    pub fn honesty_complete_ok(&self, kind: HostAmbushKind) -> bool {
        self.completed_of_kind(kind)
            .iter()
            .any(|m| m.phase == HostAmbushPhase::Completed && !m.spawned_unit_ids.is_empty())
    }

    /// Combined host path honesty: completed ambush with spawned infantry.
    pub fn honesty_host_path_ok(&self, kind: HostAmbushKind) -> bool {
        self.honesty_complete_ok(kind)
    }
}

// --- Wave 69 residual honesty peels (retail SpecialPower / OCL) ---

/// Retail SpecialPower template name residual.
pub const AMBUSH_SPECIAL_POWER_TEMPLATE: &str = "SuperweaponRebelAmbush";
/// Retail Enum residual.
pub const AMBUSH_SPECIAL_POWER_ENUM: &str = "SPECIAL_AMBUSH";
/// Retail RequiredScience residual (Ambush1).
pub const AMBUSH_REQUIRED_SCIENCE: &str = "SCIENCE_RebelAmbush1";
/// Retail ReloadTime residual (msec).
pub const AMBUSH_RELOAD_TIME_MS: u32 = 240_000;
/// Retail ReloadTime 240000ms → 7200 frames @ 30 FPS.
pub const AMBUSH_RELOAD_TIME_FRAMES: u32 = 7_200;
/// Retail RadiusCursorRadius residual.
pub const AMBUSH_RADIUS_CURSOR: f32 = 50.0;
/// Retail SharedSyncedTimer residual.
pub const AMBUSH_SHARED_SYNCED_TIMER: bool = true;
/// Retail PublicTimer residual.
pub const AMBUSH_PUBLIC_TIMER: bool = false;
/// Retail ShortcutPower residual.
pub const AMBUSH_SHORTCUT_POWER: bool = true;
/// Retail OCL CreateObject FadeTime residual (msec).
pub const AMBUSH_FADE_TIME_MS: u32 = 3_000;
/// FadeTime 3000ms → 90 frames @ 30 FPS.
pub const AMBUSH_FADE_TIME_FRAMES: u32 = 90;
/// Retail SUPERWEAPON_RebelAmbush1 ObjectCreationList name residual.
pub const AMBUSH_OCL_AMBUSH1: &str = "SUPERWEAPON_RebelAmbush1";
/// Retail Ambush2 / Ambush3 unit counts residual (science tiers; host uses Ambush1).
pub const GLA_AMBUSH2_UNIT_COUNT: u32 = 8;
pub const GLA_AMBUSH3_UNIT_COUNT: u32 = 16;

/// Retail SCIENCE_RebelAmbush1 residual.
pub const SCIENCE_AMBUSH1: &str = "SCIENCE_RebelAmbush1";
/// Retail SCIENCE_RebelAmbush2 residual.
pub const SCIENCE_AMBUSH2: &str = "SCIENCE_RebelAmbush2";
/// Retail SCIENCE_RebelAmbush3 residual.
pub const SCIENCE_AMBUSH3: &str = "SCIENCE_RebelAmbush3";

/// Residual Ambush science tier (payload 4 / 8 / 16 Rebels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum AmbushScienceTier {
    #[default]
    Level1,
    Level2,
    Level3,
}

impl AmbushScienceTier {
    pub fn rebel_count(self) -> u32 {
        match self {
            AmbushScienceTier::Level1 => GLA_AMBUSH1_UNIT_COUNT,
            AmbushScienceTier::Level2 => GLA_AMBUSH2_UNIT_COUNT,
            AmbushScienceTier::Level3 => GLA_AMBUSH3_UNIT_COUNT,
        }
    }

    pub fn science_name(self) -> &'static str {
        match self {
            AmbushScienceTier::Level1 => SCIENCE_AMBUSH1,
            AmbushScienceTier::Level2 => SCIENCE_AMBUSH2,
            AmbushScienceTier::Level3 => SCIENCE_AMBUSH3,
        }
    }

    pub fn from_science_name(name: &str) -> Option<Self> {
        let n = name.to_ascii_lowercase();
        if n.contains("ambush3") || n.contains("rebelambush3") {
            Some(AmbushScienceTier::Level3)
        } else if n.contains("ambush2") || n.contains("rebelambush2") {
            Some(AmbushScienceTier::Level2)
        } else if n.contains("ambush1") || n.contains("rebelambush") || n.contains("ambush") {
            Some(AmbushScienceTier::Level1)
        } else {
            None
        }
    }

    pub fn highest_from_sciences<'a, I>(sciences: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut best = AmbushScienceTier::Level1;
        for s in sciences {
            if let Some(tier) = Self::from_science_name(s) {
                best = match (best, tier) {
                    (_, AmbushScienceTier::Level3) | (AmbushScienceTier::Level3, _) => {
                        AmbushScienceTier::Level3
                    }
                    (_, AmbushScienceTier::Level2) | (AmbushScienceTier::Level2, _) => {
                        AmbushScienceTier::Level2
                    }
                    _ => AmbushScienceTier::Level1,
                };
            }
        }
        best
    }
}

/// Retail SpreadFormation MinDistanceA residual.
pub const AMBUSH_MIN_DISTANCE_A: f32 = 20.0;
/// Retail SpreadFormation MinDistanceB residual.
pub const AMBUSH_MIN_DISTANCE_B: f32 = 30.0;
/// Retail SpreadFormation MaxDistanceFormation residual.
pub const AMBUSH_MAX_DISTANCE_FORMATION: f32 = 400.0;
/// Retail OCL DiesOnBadLand residual.
pub const AMBUSH_DIES_ON_BAD_LAND: bool = true;
/// Retail FadeIn residual.
pub const AMBUSH_FADE_IN: bool = true;

/// Convert residual msec → logic frames @ 30 FPS (round half-up).
pub fn ambush_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * AMBUSH_LOGIC_FPS / 1000.0).round() as u32
}

/// Wave 69 residual honesty: special-power residual peel.
pub fn honesty_ambush_special_power_residual_ok() -> bool {
    AMBUSH_SPECIAL_POWER_TEMPLATE == "SuperweaponRebelAmbush"
        && AMBUSH_SPECIAL_POWER_ENUM == "SPECIAL_AMBUSH"
        && AMBUSH_REQUIRED_SCIENCE == "SCIENCE_RebelAmbush1"
        && AMBUSH_RELOAD_TIME_MS == 240_000
        && AMBUSH_RELOAD_TIME_FRAMES == ambush_ms_to_frames(AMBUSH_RELOAD_TIME_MS)
        && AMBUSH_RELOAD_TIME_FRAMES == 7_200
        && (AMBUSH_RADIUS_CURSOR - 50.0).abs() < 0.01
        && AMBUSH_SHARED_SYNCED_TIMER
        && !AMBUSH_PUBLIC_TIMER
        && AMBUSH_SHORTCUT_POWER
        && HostAmbushKind::GLARebelAmbush.activate_audio() == "RebelAmbushActivated"
        && HostAmbushKind::from_command_power(&crate::command_system::SpecialPowerType::Ambush)
            == Some(HostAmbushKind::GLARebelAmbush)
}

/// Wave 69 residual honesty: OCL spawn residual peel.
pub fn honesty_ambush_spawn_ocl_residual_ok() -> bool {
    AMBUSH_OCL_AMBUSH1 == "SUPERWEAPON_RebelAmbush1"
        && GLA_REBEL_TEMPLATE == "GLAInfantryRebel"
        && GLA_AMBUSH1_UNIT_COUNT == 4
        && GLA_AMBUSH2_UNIT_COUNT == 8
        && GLA_AMBUSH3_UNIT_COUNT == 16
        && AMBUSH_FADE_TIME_MS == 3_000
        && AMBUSH_FADE_TIME_FRAMES == ambush_ms_to_frames(AMBUSH_FADE_TIME_MS)
        && AMBUSH_FADE_TIME_FRAMES == 90
        && HostAmbushKind::GLARebelAmbush.spawn_delay_frames() == AMBUSH_FADE_TIME_FRAMES
        && HostAmbushKind::GLARebelAmbush.unit_count() == GLA_AMBUSH1_UNIT_COUNT
        && (AMBUSH_SPAWN_RADIUS - 40.0).abs() < 0.01
        && (AMBUSH_MIN_DISTANCE_A - 20.0).abs() < 0.01
        && (AMBUSH_MIN_DISTANCE_B - 30.0).abs() < 0.01
        && (AMBUSH_MAX_DISTANCE_FORMATION - 400.0).abs() < 0.01
        && AMBUSH_FADE_IN
        && AMBUSH_FADE_TIME_FRAMES == 90
        && AMBUSH_DIES_ON_BAD_LAND
        && HostAmbushKind::GLARebelAmbush.unit_template() == GLA_REBEL_TEMPLATE
}

/// Combined Wave 69 Ambush residual honesty pack.
pub fn honesty_ambush_residual_pack_ok() -> bool {
    honesty_ambush_special_power_residual_ok() && honesty_ambush_spawn_ocl_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn ambush_maps_from_command_power() {
        assert_eq!(
            HostAmbushKind::from_command_power(&SpecialPowerType::Ambush),
            Some(HostAmbushKind::GLARebelAmbush)
        );
        assert_eq!(
            HostAmbushKind::from_command_power(&SpecialPowerType::Paradrop),
            None
        );
        assert_eq!(
            HostAmbushKind::from_command_power(&SpecialPowerType::DaisyCutter),
            None
        );
    }

    #[test]
    fn queue_and_complete_ambush_spawn_plan() {
        let mut reg = HostAmbushRegistry::new();
        let id = reg.queue(
            HostAmbushKind::GLARebelAmbush,
            ObjectId(1),
            Team::GLA,
            Vec3::new(100.0, 0.0, 50.0),
            0,
            AMBUSH_RESIDUAL_TEMPLATE,
        );
        assert!(reg.honesty_queue_ok(HostAmbushKind::GLARebelAmbush));
        assert!(!reg.honesty_complete_ok(HostAmbushKind::GLARebelAmbush));

        let mission = reg.get(id).expect("mission");
        assert_eq!(mission.spawn_frame, 90);
        assert_eq!(mission.phase, HostAmbushPhase::Queued);
        assert_eq!(mission.unit_count, GLA_AMBUSH1_UNIT_COUNT);

        assert!(reg.plan_due_spawns(89).is_empty());

        let plans = reg.plan_due_spawns(90);
        assert_eq!(plans.len(), 1);
        assert_eq!(
            plans[0].spawn_positions.len(),
            GLA_AMBUSH1_UNIT_COUNT as usize
        );
        // All spawn positions should be near target.
        for pos in &plans[0].spawn_positions {
            let dx = pos.x - 100.0;
            let dz = pos.z - 50.0;
            let dist = (dx * dx + dz * dz).sqrt();
            assert!(dist <= AMBUSH_SPAWN_RADIUS + 0.1);
        }

        let spawned = vec![ObjectId(10), ObjectId(11), ObjectId(12), ObjectId(13)];
        reg.record_spawn_complete(id, spawned.clone());
        assert!(reg.honesty_complete_ok(HostAmbushKind::GLARebelAmbush));
        assert!(reg.honesty_host_path_ok(HostAmbushKind::GLARebelAmbush));
        assert_eq!(reg.get(id).unwrap().phase, HostAmbushPhase::Completed);
        assert_eq!(reg.get(id).unwrap().spawned_unit_ids, spawned);
    }

    #[test]
    fn spawn_positions_circle_formation() {
        let positions = HostAmbushRegistry::spawn_positions(Vec3::ZERO, 4, 40.0);
        assert_eq!(positions.len(), 4);
        for pos in &positions {
            let dist = (pos.x * pos.x + pos.z * pos.z).sqrt();
            assert!(dist > 0.0);
            assert!(dist <= 40.0 + 0.01);
        }
    }

    #[test]
    fn restore_from_snapshot_keeps_pending_spawn_frame() {
        let mut reg = HostAmbushRegistry::new();
        let id = reg.queue(
            HostAmbushKind::GLARebelAmbush,
            ObjectId(9),
            Team::GLA,
            Vec3::new(1.0, 0.0, 2.0),
            10,
            AMBUSH_RESIDUAL_TEMPLATE,
        );
        let snap = reg.missions_snapshot();
        let next = reg.next_id();

        let mut loaded = HostAmbushRegistry::new();
        loaded.restore_from_snapshot(next, snap);
        assert_eq!(loaded.pending_count(), 1);
        let m = loaded.get(id).expect("restored mission");
        assert_eq!(m.spawn_frame, 100);
        assert_eq!(m.phase, HostAmbushPhase::Queued);
        assert_eq!(loaded.next_id(), next);
    }

    #[test]
    fn ambush_residual_pack_honesty_wave69() {
        assert_eq!(ambush_ms_to_frames(3_000), 90);
        assert_eq!(ambush_ms_to_frames(240_000), 7_200);
        assert_eq!(ambush_ms_to_frames(0), 0);
        assert!(honesty_ambush_special_power_residual_ok());
        assert!(honesty_ambush_spawn_ocl_residual_ok());
        assert!(honesty_ambush_residual_pack_ok());
        assert_eq!(GLA_AMBUSH1_UNIT_COUNT, 4);
        assert_eq!(GLA_AMBUSH2_UNIT_COUNT, 8);
        assert_eq!(GLA_AMBUSH3_UNIT_COUNT, 16);
        assert_eq!(AMBUSH_REQUIRED_SCIENCE, "SCIENCE_RebelAmbush1");
        assert!(AMBUSH_SHARED_SYNCED_TIMER);
        assert!(!AMBUSH_PUBLIC_TIMER);
    }
}
