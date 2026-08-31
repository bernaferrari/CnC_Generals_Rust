//! Host CaveSystem residual (C++ `CaveSystem` + `CaveContain`).
//!
//! Caves share a `TunnelTracker`-style pool keyed by `CaveIndex`, not by
//! player. First occupant defects every connected cave; last occupant leaving
//! restores `m_originalTeam`. Destroying the last entrance cave-in-kills the
//! pool (`CaveContain::onDie` + `TunnelTracker::onTunnelDestroyed`). Ranger
//! capture does not kick (`isKickOutOnCapture = false`).

use super::{ObjectId, Team};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// C++ CaveContain default capacity follows the shared tracker (same 10).
pub const MAX_CAVE_CAPACITY: usize = 10;

/// C++ `TunnelTracker::onTunnelDestroyed` result for one CaveIndex.
#[derive(Debug, Clone, Default)]
pub struct CaveDestroyedOutcome {
    pub cave_in: bool,
    pub cave_in_units: Vec<ObjectId>,
    pub remapped_to: Option<ObjectId>,
}

/// First-occupant / last-empty capture transitions (CaveContain.cpp:254-336).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaveCaptureEvent {
    None,
    FirstOccupant(Team),
    LastEmpty,
}

/// Shared contain state for one CaveIndex.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCaveNetwork {
    pub contained: Vec<ObjectId>,
    pub entry_cave: HashMap<u32, ObjectId>,
    pub cave_ids: Vec<ObjectId>,
    pub controlling_team: Option<Team>,
}

/// Live host CaveSystem: index → shared tracker.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCaveSystem {
    pub enters: u32,
    pub exits: u32,
    pub cross_exits: u32,
    pub caves_destroyed: u32,
    pub cave_ins: u32,
    pub cave_in_kills: u32,
    pub captures: u32,
    pub index_switches: u32,
    networks: HashMap<i32, HostCaveNetwork>,
    cave_index_of: HashMap<u32, i32>,
    original_team: HashMap<u32, Team>,
}

impl HostCaveSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn index_of(&self, cave: ObjectId) -> Option<i32> {
        self.cave_index_of.get(&cave.0).copied()
    }

    pub fn original_team_of(&self, cave: ObjectId) -> Option<Team> {
        self.original_team.get(&cave.0).copied()
    }

    pub fn network(&self, index: i32) -> Option<&HostCaveNetwork> {
        self.networks.get(&index)
    }

    fn network_mut(&mut self, index: i32) -> &mut HostCaveNetwork {
        self.networks.entry(index).or_default()
    }

    pub fn contain_count(&self, index: i32) -> usize {
        self.networks
            .get(&index)
            .map(|n| n.contained.len())
            .unwrap_or(0)
    }

    pub fn has_capacity(&self, index: i32) -> bool {
        self.contain_count(index) < MAX_CAVE_CAPACITY
    }

    pub fn contained_for_index(&self, index: i32) -> Vec<ObjectId> {
        self.networks
            .get(&index)
            .map(|n| n.contained.clone())
            .unwrap_or_default()
    }

    pub fn cave_ids_for_index(&self, index: i32) -> Vec<ObjectId> {
        self.networks
            .get(&index)
            .map(|n| n.cave_ids.clone())
            .unwrap_or_default()
    }

    pub fn is_in_network(&self, index: i32, unit_id: ObjectId) -> bool {
        self.networks
            .get(&index)
            .map(|n| n.contained.contains(&unit_id))
            .unwrap_or(false)
    }

    pub fn index_holding_unit(&self, unit_id: ObjectId) -> Option<i32> {
        for (idx, net) in &self.networks {
            if net.contained.contains(&unit_id) {
                return Some(*idx);
            }
        }
        None
    }

    /// C++ `CaveContain::onBuildComplete` → `registerNewCave` + `onTunnelCreated`.
    pub fn register_cave(&mut self, cave: ObjectId, index: i32, team: Team) {
        self.cave_index_of.insert(cave.0, index);
        self.original_team.entry(cave.0).or_insert(team);
        let net = self.network_mut(index);
        if !net.cave_ids.contains(&cave) {
            net.cave_ids.push(cave);
        }
    }

    /// C++ `CaveSystem::canSwitchIndexToIndex` + `CaveContain::tryToSetCaveIndex`.
    pub fn try_set_cave_index(&mut self, cave: ObjectId, new_index: i32) -> bool {
        let Some(old) = self.cave_index_of.get(&cave.0).copied() else {
            return false;
        };
        if old == new_index {
            return true;
        }
        if !self.can_switch_index_to_index(old, new_index) {
            return false;
        }
        if let Some(net) = self.networks.get_mut(&old) {
            net.cave_ids.retain(|id| *id != cave);
        }
        self.cave_index_of.insert(cave.0, new_index);
        let net = self.network_mut(new_index);
        if !net.cave_ids.contains(&cave) {
            net.cave_ids.push(cave);
        }
        self.index_switches = self.index_switches.saturating_add(1);
        true
    }

    pub fn can_switch_index_to_index(&self, old_index: i32, new_index: i32) -> bool {
        self.contain_count(old_index) == 0 && self.contain_count(new_index) == 0
    }

    pub fn record_enter(
        &mut self,
        index: i32,
        unit_id: ObjectId,
        entry_cave: ObjectId,
        rider_team: Team,
    ) -> (bool, CaveCaptureEvent) {
        if self.is_in_network(index, unit_id) {
            return (true, CaveCaptureEvent::None);
        }
        if !self.has_capacity(index) {
            return (false, CaveCaptureEvent::None);
        }
        let was_empty = self.contain_count(index) == 0;
        self.enters = self.enters.saturating_add(1);
        let event = if was_empty {
            self.captures = self.captures.saturating_add(1);
            CaveCaptureEvent::FirstOccupant(rider_team)
        } else {
            CaveCaptureEvent::None
        };
        let net = self.network_mut(index);
        net.contained.push(unit_id);
        net.entry_cave.insert(unit_id.0, entry_cave);
        if was_empty {
            net.controlling_team = Some(rider_team);
        }
        (true, event)
    }

    pub fn record_exit(
        &mut self,
        index: i32,
        unit_id: ObjectId,
        exit_cave: ObjectId,
    ) -> (Option<ObjectId>, CaveCaptureEvent) {
        let net = match self.networks.get_mut(&index) {
            Some(n) => n,
            None => return (None, CaveCaptureEvent::None),
        };
        let Some(pos) = net.contained.iter().position(|&id| id == unit_id) else {
            return (None, CaveCaptureEvent::None);
        };
        net.contained.remove(pos);
        let entry = net.entry_cave.remove(&unit_id.0);
        self.exits = self.exits.saturating_add(1);
        if let Some(entry_id) = entry {
            if entry_id != exit_cave {
                self.cross_exits = self.cross_exits.saturating_add(1);
            }
        }
        let event = if net.contained.is_empty() {
            net.controlling_team = None;
            CaveCaptureEvent::LastEmpty
        } else {
            CaveCaptureEvent::None
        };
        (entry, event)
    }

    /// C++ `CaveContain::onDie` → `unregisterCave` + `onTunnelDestroyed`.
    pub fn on_cave_destroyed(
        &mut self,
        dead_cave: ObjectId,
        remaining_other: &[ObjectId],
    ) -> CaveDestroyedOutcome {
        self.caves_destroyed = self.caves_destroyed.saturating_add(1);
        let Some(index) = self.cave_index_of.remove(&dead_cave.0) else {
            return CaveDestroyedOutcome::default();
        };
        if let Some(net) = self.networks.get_mut(&index) {
            net.cave_ids.retain(|id| *id != dead_cave);
        }
        if remaining_other.is_empty() {
            let units = self.contained_for_index(index);
            for &uid in &units {
                let _ = self.record_exit(index, uid, dead_cave);
            }
            self.cave_ins = self.cave_ins.saturating_add(1);
            self.cave_in_kills = self.cave_in_kills.saturating_add(units.len() as u32);
            CaveDestroyedOutcome {
                cave_in: true,
                cave_in_units: units,
                remapped_to: None,
            }
        } else {
            let remapped_to = remaining_other[0];
            if let Some(net) = self.networks.get_mut(&index) {
                for entry in net.entry_cave.values_mut() {
                    if *entry == dead_cave {
                        *entry = remapped_to;
                    }
                }
            }
            CaveDestroyedOutcome {
                cave_in: false,
                cave_in_units: Vec::new(),
                remapped_to: Some(remapped_to),
            }
        }
    }

    pub fn honesty_enter_exit_ok(&self) -> bool {
        self.enters > 0 && self.exits > 0
    }

    pub fn honesty_cross_exit_ok(&self) -> bool {
        self.cross_exits > 0
    }

    pub fn honesty_cave_in_ok(&self) -> bool {
        self.cave_ins > 0 && self.cave_in_kills > 0
    }

    pub fn honesty_capture_ok(&self) -> bool {
        self.captures > 0
    }
}

/// C++ CivilianBuilding.ini / CaveContain CaveIndex templates.
///
/// C++ never name-matches caves: CaveContain is an authored object module
/// (CaveContain.cpp installed via the map object's module list).  The
/// residual name gate must therefore only hit a *cave token*, not any
/// substring: the naive `contains("cave")` crossed the faction/model token
/// boundary of every `AmericaVehicle*` template ("ameriCA VEHicle" contains
/// "cave") and installed CaveContain garrison storage on TransportContain
/// vehicles — Combat Chinook riders landed in
/// `building_data.garrisoned_units` so `transport_count()` stayed 0
/// (TransportContain.cpp:105+ slot storage).  Accept "cave" at a token
/// boundary only: string start, a non-letter separator (`_`, `.`, `-`,
/// digit), or an authored camelCase boundary (uppercase `C` after a
/// lowercase letter).  Authored exclusions ("cavein", "scaffold") stay.
pub fn is_cave_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.contains("cavein") || n.contains("scaffold") {
        return false;
    }
    let lower_bytes = n.as_bytes();
    let orig_bytes = template_name.as_bytes();
    let mut from = 0;
    while let Some(rel) = n[from..].find("cave") {
        let i = from + rel;
        let at_start = i == 0;
        let prev_is_lower = i > 0 && lower_bytes[i - 1].is_ascii_lowercase();
        // camelCase boundary: authored uppercase 'C' following a letter.
        let camel_case =
            i > 0 && orig_bytes[i].is_ascii_uppercase() && prev_is_lower;
        if at_start || !prev_is_lower || camel_case {
            return true;
        }
        from = i + 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_occupant_captures_and_last_empty_uncaptures() {
        // C++ CaveContain.cpp:254-336 recalcApparentControllingPlayer.
        let mut sys = HostCaveSystem::new();
        let a = ObjectId(1);
        let b = ObjectId(2);
        let u = ObjectId(10);
        sys.register_cave(a, 3, Team::Neutral);
        sys.register_cave(b, 3, Team::Neutral);
        let (ok, ev) = sys.record_enter(3, u, a, Team::USA);
        assert!(ok);
        assert_eq!(ev, CaveCaptureEvent::FirstOccupant(Team::USA));
        assert_eq!(sys.network(3).unwrap().controlling_team, Some(Team::USA));
        let (_, ev) = sys.record_exit(3, u, b);
        assert_eq!(ev, CaveCaptureEvent::LastEmpty);
        assert!(sys.network(3).unwrap().controlling_team.is_none());
    }

    #[test]
    fn same_index_shares_inventory_and_set_cave_index_switches_empty() {
        // C++ CaveContain.cpp:43-47,236-249; ScriptActions.cpp:5063 SET_CAVE_INDEX.
        let mut sys = HostCaveSystem::new();
        let a = ObjectId(1);
        let b = ObjectId(2);
        let u = ObjectId(10);
        sys.register_cave(a, 0, Team::Neutral);
        sys.register_cave(b, 0, Team::Neutral);
        assert!(sys.record_enter(0, u, a, Team::GLA).0);
        assert!(sys.is_in_network(0, u));
        assert!(!sys.try_set_cave_index(b, 1));
        let _ = sys.record_exit(0, u, b);
        assert!(sys.try_set_cave_index(b, 1));
        assert_eq!(sys.index_of(b), Some(1));
        assert_eq!(sys.index_switches, 1);
    }

    #[test]
    fn last_cave_destroy_caves_in() {
        // C++ CaveContain.cpp:197-211 + TunnelTracker.cpp:187-220.
        let mut sys = HostCaveSystem::new();
        let a = ObjectId(1);
        let b = ObjectId(2);
        let u = ObjectId(10);
        sys.register_cave(a, 0, Team::Neutral);
        sys.register_cave(b, 0, Team::Neutral);
        assert!(sys.record_enter(0, u, a, Team::USA).0);
        let keep = sys.on_cave_destroyed(a, &[b]);
        assert!(!keep.cave_in);
        assert_eq!(keep.remapped_to, Some(b));
        let last = sys.on_cave_destroyed(b, &[]);
        assert!(last.cave_in);
        assert_eq!(last.cave_in_units, vec![u]);
        assert!(sys.honesty_cave_in_ok());
    }
}
