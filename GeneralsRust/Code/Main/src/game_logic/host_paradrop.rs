//! Host America Paradrop / Airborne special-power residual.
//!
//! Residual slice: host `DoSpecialPower` for America Paradrop (Airborne)
//! queues a drop at the target location. After a flight/approach delay,
//! infantry units spawn near the target (line formation residual).
//!
//! Fail-closed honesty:
//! - Not full OCL DeliverPayload cargo plane path
//! - Not full parachute containers / AmericaParachute fall physics
//! - Not full science upgrade OCL matrix (Paradrop2/3 payload tiers)
//! - Not multiplayer shared timer / academy classification

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
            SpecialPowerType::Paradrop => Some(HostParadropKind::AmericaParadrop),
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
}

impl HostParadropRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            missions: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.missions.clear();
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
        self.next_id = 1;
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
            unit_count: kind.unit_count(),
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

        let spawned = vec![ObjectId(10), ObjectId(11), ObjectId(12), ObjectId(13), ObjectId(14)];
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
}
