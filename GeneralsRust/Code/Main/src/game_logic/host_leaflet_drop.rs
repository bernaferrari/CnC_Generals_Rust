//! Host USA Leaflet Drop special-power residual — temporary enemy disable.
//!
//! Residual slice (playability):
//! - `DoSpecialPower(LeafletDrop)` at a world location queues a delayed disable
//!   residual (retail SuperweaponLeafletDrop → SUPERWEAPON_LeafletDrop B52
//!   payload → LeafletContainer LeafletDropBehavior).
//! - After Delay (2500 ms), enemy infantry and vehicles in AffectRadius receive
//!   DISABLED_EMP for DisabledDuration (20000 ms) — matches C++
//!   LeafletDropBehavior::doDisableAttack setDisabledUntil(DISABLED_EMP, ...).
//! - Honesty counters/flags for residual gates and tests.
//!
//! Fail-closed honesty:
//! - Not full OCL AmericaJetB52 / LeafletContainer drawable / LeafletFX particles
//! - Not full relationship matrix beyond residual enemy-team filter
//! - Not EarlyLeafletDrop science shortcut timer matrix
//! - Not network leaflet replication (network deferred)

use super::ObjectId;
use crate::command_system::SpecialPowerType;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const LEAFLET_LOGIC_FPS: f32 = 30.0;

/// Retail SuperweaponLeafletDrop RadiusCursorRadius residual (= 110).
/// Also matches LeafletContainer LeafletDropBehavior AffectRadius = 110.0.
pub const HOST_LEAFLET_RADIUS: f32 = 110.0;

/// Retail LeafletContainer Delay = 2500 ms → 75 logic frames.
pub const LEAFLET_DELAY_MS: u32 = 2_500;
/// Delay residual in frames before disable applies.
pub const LEAFLET_DELAY_FRAMES: u32 = (LEAFLET_DELAY_MS * 30) / 1000;

/// Retail LeafletContainer DisabledDuration = 20000 ms → 600 logic frames.
pub const LEAFLET_DISABLED_DURATION_MS: u32 = 20_000;
/// DISABLED_EMP residual duration after leaflets hit.
pub const LEAFLET_DISABLED_DURATION_FRAMES: u32 = (LEAFLET_DISABLED_DURATION_MS * 30) / 1000;

/// Activate audio residual (SoundEffects.ini LeafletDrop).
pub const LEAFLET_ACTIVATE_AUDIO: &str = "LeafletDrop";
/// Impact / disable audio residual (SoundEffects.ini LeafletDropEffect).
pub const LEAFLET_IMPACT_AUDIO: &str = "LeafletDropEffect";

/// Host residual leaflet-drop kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostLeafletDropKind {
    /// USA SPECIAL_LEAFLET_DROP / SuperweaponLeafletDrop residual.
    UsaLeafletDrop,
}

impl HostLeafletDropKind {
    /// Map a command-system power type to a host residual leaflet drop, if supported.
    pub fn from_command_power(power: &SpecialPowerType) -> Option<Self> {
        match power {
            SpecialPowerType::LeafletDrop => Some(HostLeafletDropKind::UsaLeafletDrop),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HostLeafletDropKind::UsaLeafletDrop => "UsaLeafletDrop",
        }
    }

    /// Delay frames before disable residual applies.
    pub fn delay_frames(self) -> u32 {
        match self {
            HostLeafletDropKind::UsaLeafletDrop => LEAFLET_DELAY_FRAMES,
        }
    }

    /// Affect radius residual.
    pub fn radius(self) -> f32 {
        match self {
            HostLeafletDropKind::UsaLeafletDrop => HOST_LEAFLET_RADIUS,
        }
    }

    /// DISABLED_EMP duration frames residual.
    pub fn disabled_duration_frames(self) -> u32 {
        match self {
            HostLeafletDropKind::UsaLeafletDrop => LEAFLET_DISABLED_DURATION_FRAMES,
        }
    }

    pub fn activate_audio(self) -> &'static str {
        match self {
            HostLeafletDropKind::UsaLeafletDrop => LEAFLET_ACTIVATE_AUDIO,
        }
    }

    pub fn impact_audio(self) -> &'static str {
        match self {
            HostLeafletDropKind::UsaLeafletDrop => LEAFLET_IMPACT_AUDIO,
        }
    }
}

/// Lifecycle of a queued host leaflet drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostLeafletDropPhase {
    /// Queued after DoSpecialPower; waiting for delay frame.
    Queued,
    /// Disable residual resolved against enemy infantry/vehicles.
    Completed,
    /// Cancelled (source died / invalid) before impact.
    Cancelled,
}

/// One pending or completed host leaflet drop mission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostLeafletDropMission {
    pub id: u32,
    pub kind: HostLeafletDropKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub activate_frame: u32,
    pub impact_frame: u32,
    pub phase: HostLeafletDropPhase,
    /// Enemy infantry/vehicles that received DISABLED_EMP this impact.
    pub disables: u32,
}

/// Impact plan for one due leaflet drop (computed before mutable disable).
#[derive(Debug, Clone)]
pub struct HostLeafletDropImpactPlan {
    pub mission_id: u32,
    pub kind: HostLeafletDropKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub radius: f32,
    pub disable_until_frame: u32,
}

/// Whether residual target can receive Leaflet disable.
///
/// Retail LeafletDropBehavior::doDisableAttack:
/// - KINDOF_INFANTRY or KINDOF_VEHICLE
/// - relationship ENEMIES only
/// - not self
pub fn is_legal_leaflet_disable_target(
    is_infantry: bool,
    is_vehicle: bool,
    is_alive: bool,
    is_enemy: bool,
    under_construction: bool,
) -> bool {
    if !is_alive || under_construction || !is_enemy {
        return false;
    }
    is_infantry || is_vehicle
}

/// 2D distance check residual (host gameplay x/z plane).
pub fn in_leaflet_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// Host residual registry for LeafletDrop special power missions.
#[derive(Debug, Clone, Default)]
pub struct HostLeafletDropRegistry {
    next_id: u32,
    missions: HashMap<u32, HostLeafletDropMission>,
    completed_this_frame: Vec<u32>,
    activated_this_frame: Vec<u32>,
    /// Total activations (honesty).
    pub activation_count: u32,
    /// Total DISABLED_EMP grants applied across all impacts.
    pub disable_count: u32,
}

impl HostLeafletDropRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            missions: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
            activation_count: 0,
            disable_count: 0,
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn clear_frame_events(&mut self) {
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
    }

    pub fn activation_count(&self) -> u32 {
        self.activation_count
    }

    pub fn disable_count(&self) -> u32 {
        self.disable_count
    }

    pub fn mission_count(&self) -> usize {
        self.missions.len()
    }

    pub fn pending_count(&self) -> usize {
        self.missions
            .values()
            .filter(|m| m.phase == HostLeafletDropPhase::Queued)
            .count()
    }

    pub fn get(&self, id: u32) -> Option<&HostLeafletDropMission> {
        self.missions.get(&id)
    }

    pub fn missions_snapshot(&self) -> Vec<HostLeafletDropMission> {
        let mut v: Vec<_> = self.missions.values().cloned().collect();
        v.sort_by_key(|m| m.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostLeafletDropKind) -> Vec<&HostLeafletDropMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostLeafletDropPhase::Queued && m.kind == kind)
            .collect()
    }

    pub fn completed_of_kind(&self, kind: HostLeafletDropKind) -> Vec<&HostLeafletDropMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostLeafletDropPhase::Completed && m.kind == kind)
            .collect()
    }

    /// Queue a leaflet drop mission. Returns host mission id.
    pub fn queue(
        &mut self,
        kind: HostLeafletDropKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let impact_frame = activate_frame.saturating_add(kind.delay_frames());
        let mission = HostLeafletDropMission {
            id,
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            impact_frame,
            phase: HostLeafletDropPhase::Queued,
            disables: 0,
        };
        self.missions.insert(id, mission);
        self.activated_this_frame.push(id);
        self.activation_count = self.activation_count.saturating_add(1);
        id
    }

    /// Build impact plans for all missions whose delay frame has arrived.
    pub fn plan_due_impacts(&self, current_frame: u32) -> Vec<HostLeafletDropImpactPlan> {
        let mut plans = Vec::new();
        for mission in self.missions.values() {
            if mission.phase != HostLeafletDropPhase::Queued || current_frame < mission.impact_frame
            {
                continue;
            }
            plans.push(HostLeafletDropImpactPlan {
                mission_id: mission.id,
                kind: mission.kind,
                source_object: mission.source_object,
                source_team: mission.source_team,
                target_position: mission.target_position,
                radius: mission.kind.radius(),
                disable_until_frame: current_frame
                    .saturating_add(mission.kind.disabled_duration_frames()),
            });
        }
        plans.sort_by_key(|p| p.mission_id);
        plans
    }

    /// Record impact results after GameLogic applied disables.
    pub fn record_impact_complete(&mut self, mission_id: u32, disables: u32) {
        if let Some(mission) = self.missions.get_mut(&mission_id) {
            if mission.phase == HostLeafletDropPhase::Queued {
                mission.phase = HostLeafletDropPhase::Completed;
                mission.disables = disables;
                self.disable_count = self.disable_count.saturating_add(disables);
                self.completed_this_frame.push(mission_id);
            }
        }
    }

    /// Cancel pending missions owned by a destroyed source object.
    pub fn cancel_for_source(&mut self, source: ObjectId) {
        for mission in self.missions.values_mut() {
            if mission.source_object == source && mission.phase == HostLeafletDropPhase::Queued {
                mission.phase = HostLeafletDropPhase::Cancelled;
            }
        }
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    /// Residual honesty: at least one leaflet drop activated/queued.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activation_count > 0
    }

    /// Residual honesty: at least one unit received DISABLED_EMP from leaflet.
    pub fn honesty_disable_ok(&self) -> bool {
        self.disable_count > 0
    }

    /// Combined host path: activated and applied at least one disable.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_disable_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn leaflet_constants_match_retail_residual() {
        assert!((HOST_LEAFLET_RADIUS - 110.0).abs() < 0.01);
        assert_eq!(LEAFLET_DELAY_FRAMES, 75);
        assert_eq!(LEAFLET_DISABLED_DURATION_FRAMES, 600);
        assert!(!LEAFLET_ACTIVATE_AUDIO.is_empty());
    }

    #[test]
    fn leaflet_maps_from_command_power() {
        assert_eq!(
            HostLeafletDropKind::from_command_power(&SpecialPowerType::LeafletDrop),
            Some(HostLeafletDropKind::UsaLeafletDrop)
        );
        assert_eq!(
            HostLeafletDropKind::from_command_power(&SpecialPowerType::EmpPulse),
            None
        );
        assert_eq!(
            HostLeafletDropKind::from_command_power(&SpecialPowerType::Ambush),
            None
        );
    }

    #[test]
    fn legal_leaflet_disable_target_matrix() {
        // infantry, vehicle, alive, enemy, under_construction
        assert!(is_legal_leaflet_disable_target(
            true, false, true, true, false
        ));
        assert!(is_legal_leaflet_disable_target(
            false, true, true, true, false
        ));
        assert!(!is_legal_leaflet_disable_target(
            false, false, true, true, false
        )); // structure residual
        assert!(!is_legal_leaflet_disable_target(
            true, false, true, false, false
        )); // ally/neutral residual
        assert!(!is_legal_leaflet_disable_target(
            true, false, false, true, false
        ));
        assert!(!is_legal_leaflet_disable_target(
            true, false, true, true, true
        ));
    }

    #[test]
    fn queue_and_complete_leaflet_impact_plan() {
        let mut reg = HostLeafletDropRegistry::new();
        let id = reg.queue(
            HostLeafletDropKind::UsaLeafletDrop,
            ObjectId(1),
            Team::USA,
            Vec3::new(100.0, 0.0, 50.0),
            0,
        );
        assert!(reg.honesty_activate_ok());
        assert!(!reg.honesty_disable_ok());
        assert_eq!(reg.pending_count(), 1);
        assert_eq!(reg.get(id).unwrap().impact_frame, LEAFLET_DELAY_FRAMES);

        assert!(reg.plan_due_impacts(LEAFLET_DELAY_FRAMES - 1).is_empty());
        let plans = reg.plan_due_impacts(LEAFLET_DELAY_FRAMES);
        assert_eq!(plans.len(), 1);
        assert!((plans[0].radius - HOST_LEAFLET_RADIUS).abs() < 0.01);

        reg.record_impact_complete(id, 3);
        assert!(reg.honesty_disable_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.disable_count(), 3);
        assert_eq!(reg.pending_count(), 0);
    }

    #[test]
    fn radius_filter() {
        assert!(in_leaflet_radius_2d((0.0, 0.0), (110.0, 0.0), 110.0));
        assert!(!in_leaflet_radius_2d((0.0, 0.0), (111.0, 0.0), 110.0));
    }
}
