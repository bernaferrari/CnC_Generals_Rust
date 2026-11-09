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

// Keep the existing serde map representation readable by older host saves,
// but emit its index and identity keys in a stable order. The contained and
// cave ID vectors retain their gameplay insertion order.
fn serialize_ordered_map<S, K, V>(map: &HashMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    K: Ord + Serialize,
    V: Serialize,
{
    use serde::ser::SerializeMap;
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
    let mut output = serializer.serialize_map(Some(entries.len()))?;
    for (key, value) in entries {
        output.serialize_entry(key, value)?;
    }
    output.end()
}

/// Capacity of a world whose GameData was never admitted (synthetic fixtures).
/// Production worlds always install the session's authored MaxTunnelCapacity.
const UNADMITTED_GAMEDATA_CAPACITY: i32 = 10;

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
    #[serde(serialize_with = "serialize_ordered_map")]
    pub entry_cave: HashMap<u32, ObjectId>,
    pub cave_ids: Vec<ObjectId>,
    pub controlling_team: Option<Team>,
}

/// Live host CaveSystem: index → shared tracker.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCaveSystem {
    /// Session definition; never serialized with mutable tracker state.
    #[serde(skip)]
    capacity: Option<i32>,
    pub enters: u32,
    pub exits: u32,
    pub cross_exits: u32,
    pub caves_destroyed: u32,
    pub cave_ins: u32,
    pub cave_in_kills: u32,
    pub captures: u32,
    pub index_switches: u32,
    #[serde(serialize_with = "serialize_ordered_map")]
    networks: HashMap<i32, HostCaveNetwork>,
    #[serde(serialize_with = "serialize_ordered_map")]
    cave_index_of: HashMap<u32, i32>,
    #[serde(serialize_with = "serialize_ordered_map")]
    original_team: HashMap<u32, Team>,
}

impl HostCaveSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        let capacity = self.capacity;
        *self = Self::default();
        self.capacity = capacity;
    }

    pub(crate) fn set_capacity(&mut self, capacity: i32) {
        self.capacity = Some(capacity);
    }

    pub(crate) fn capacity(&self) -> i32 {
        self.capacity.unwrap_or(UNADMITTED_GAMEDATA_CAPACITY)
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
    pub(crate) fn networks(&self) -> impl Iterator<Item = (i32, &HostCaveNetwork)> {
        self.networks.iter().map(|(&index, network)| (index, network))
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
        i64::try_from(self.contain_count(index)).unwrap_or(i64::MAX) < i64::from(self.capacity())
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
    pub fn on_cave_destroyed(&mut self, dead_cave: ObjectId) -> CaveDestroyedOutcome {
        self.caves_destroyed = self.caves_destroyed.saturating_add(1);
        let Some(index) = self.cave_index_of.remove(&dead_cave.0) else {
            return CaveDestroyedOutcome::default();
        };
        if let Some(net) = self.networks.get_mut(&index) {
            net.cave_ids.retain(|id| *id != dead_cave);
        }
        let remaining = self.networks.get(&index).and_then(|net| net.cave_ids.first()).copied();
        if remaining.is_none() {
            let units = self.contained_for_index(index);
            self.cave_ins = self.cave_ins.saturating_add(1);
            self.cave_in_kills = self.cave_in_kills.saturating_add(units.len() as u32);
            CaveDestroyedOutcome {
                cave_in: true,
                cave_in_units: units,
                remapped_to: None,
            }
        } else {
            let remapped_to = remaining.expect("remaining cave checked above");
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

    /// TunnelTracker clears the shared list only after all cave-in destroy
    /// callbacks have run, without issuing containment exit callbacks.
    pub fn finish_cave_in(&mut self, index: i32) {
        if let Some(net) = self.networks.get_mut(&index) {
            net.contained.clear();
            net.entry_cave.clear();
            net.controlling_team = None;
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


#[cfg(test)]
mod tests {
    use super::*;
#[test]
fn cave_saved_world_continues_in_receiving_capacity_without_touching_same_id_peer() {
    use crate::game_logic::{AIState, ContainModuleKind, GameLogic, KindOf, ObjectAudio, Player, ThingTemplate};
    use crate::save_load::snapshot::{ClientDrawableWorldSnapshot, SnapshotBuilder, WorldSnapshot};
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo, Snapshot, Xfer, XferLoad, XferSave};
    use std::io::Cursor;
    use glam::Vec3;
    use std::time::{Duration, SystemTime};

    fn world(capacity: i32) -> (GameLogic, ObjectId, ObjectId, ObjectId, ObjectId) {
        let mut logic = GameLogic::new();
        logic.admit_tunnel_capacity_baseline(capacity);
        for (id, team, name) in [(0, Team::USA, "CaveUSA"), (3, Team::Neutral, "CaveNeutral")] {
            logic.add_player(Player::new(id, team, name, true).with_script_identity(name));
        }
        let mut cave = ThingTemplate::new("SavedCaveEntrance");
        cave.add_kind_of(KindOf::Structure).add_kind_of(KindOf::Selectable).set_health(500.0);
        cave.contain_module.kind = ContainModuleKind::Cave;
        cave.contain_module.cave_index = 5;
        logic.templates.insert("SavedCaveEntrance".into(), cave);
        let mut infantry = ThingTemplate::new("SavedCaveInfantry");
        infantry.add_kind_of(KindOf::Infantry).add_kind_of(KindOf::Selectable).set_health(100.0);
        logic.templates.insert("SavedCaveInfantry".into(), infantry);
        let first = logic.create_object("SavedCaveEntrance", Team::Neutral, Vec3::ZERO).unwrap();
        let second = logic.create_object("SavedCaveEntrance", Team::Neutral, Vec3::new(40.0, 0.0, 0.0)).unwrap();
        let neutral = logic.host_default_team_id(3).unwrap();
        assert!(logic.set_host_object_team(first, neutral));
        assert!(logic.set_host_object_team(second, neutral));
        let a = logic.create_object("SavedCaveInfantry", Team::USA, Vec3::ZERO).unwrap();
        let b = logic.create_object("SavedCaveInfantry", Team::USA, Vec3::ZERO).unwrap();
        (logic, first, second, a, b)
    }

    fn enter(logic: &mut GameLogic, rider: ObjectId, cave: ObjectId) {
        let object = logic.host_object_mut(rider).unwrap();
        object.target = Some(cave);
        object.set_ai_state(AIState::Entering, 0, &mut ObjectAudio::record(&mut Vec::new()));
        logic.try_physics_collide(rider, cave, 1.0).unwrap();
        assert_eq!(logic.host_object(rider).unwrap().contained_by, Some(cave));
    }

    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "cave_saved_world_continues_in_receiving_capacity_without_touching_same_id_peer",
        || {
            let (mut source, first, second, a, b) = world(3);
            let (mut receiving, rf, rs, ra, rb) = world(2);
            let (mut peer, pf, ps, pa, pb) = world(1);
            assert_eq!((first, second, a, b), (rf, rs, ra, rb));
            assert_eq!((first, second, a, b), (pf, ps, pa, pb));
            enter(&mut source, b, second);
            enter(&mut source, a, first);
            enter(&mut peer, a, first);

            let dir = tempfile::TempDir::new().unwrap();
            let mut manager = SaveFileManager::with_save_directory(dir.path());
            manager.init().unwrap();
            let info = SaveGameInfo {
                pristine_map_name: None,
                filename: "caves".into(),
                display_name: "Caves".into(),
                description: String::new(),
                map_name: "CaveMap".into(),
                campaign_side: None,
                mission_number: None,
                save_date: SystemTime::now(),
                game_version: env!("CARGO_PKG_VERSION").into(),
                play_time: Duration::ZERO,
                difficulty: GameDifficulty::Medium,
                save_type: SaveFileType::Normal,
            };
            manager.save_game_with_client_drawable_snapshot(
                "caves", &source, ClientDrawableWorldSnapshot::default(), &info,
            ).unwrap();
            let mut snapshot = manager.load_game_snapshot("caves").unwrap().snapshot;
            assert_eq!(snapshot.cave_system.cave_ids_for_index(5), vec![first, second]);
            assert_eq!(snapshot.cave_system.contained_for_index(5), vec![b, a]);
            assert!(snapshot.cave_system.network(4).is_none());
            // The separate direct-Xfer route has the same host cave blob but
            // different outer framing from the named-chunk .sav route.
            let mut bytes = Cursor::new(Vec::new());
            {
                let mut writer = XferSave::new(&mut bytes);
                snapshot.xfer(&mut writer).unwrap();
                writer.xfer_u32(&mut 0xcafe_babe).unwrap();
            }
            let mut direct = WorldSnapshot::default();
            let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
            direct.xfer(&mut reader).unwrap();
            let mut following = 0;
            reader.xfer_u32(&mut following).unwrap();
            assert_eq!(following, 0xcafe_babe);
            assert_eq!(direct.cave_system.cave_ids_for_index(5), vec![first, second]);
            assert_eq!(direct.cave_system.contained_for_index(5), vec![b, a]);
            SnapshotBuilder::new().restore_from_snapshot(&snapshot, &mut receiving).unwrap();
            assert_eq!(receiving.cave_system_residual().contained_for_index(5), vec![b, a]);
            assert!(!receiving.cave_system_residual().has_capacity(5));
            assert!(!receiving.partition_manager.is_registered(a.0));
            assert!(!receiving.partition_manager.is_registered(b.0));
            assert!(receiving.exit_cave_unit(b, first));
            assert!(receiving.cave_system_residual().has_capacity(5));
            assert_eq!(receiving.cave_system_residual().contained_for_index(5), vec![a]);
            assert_eq!(peer.cave_system_residual().contained_for_index(5), vec![a]);
            assert!(!peer.cave_system_residual().has_capacity(5));
            assert_eq!(peer.host_object(a).unwrap().contained_by, Some(first));
            assert_eq!(peer.host_object(b).unwrap().contained_by, None);
            assert_eq!(source.cave_system_residual().contained_for_index(5), vec![b, a]);
        },
    );
}

    #[test]
    fn saved_cave_maps_are_ordered_without_reordering_tracker_lists() {
        fn populated(reverse: bool) -> HostCaveSystem {
            let mut system = HostCaveSystem::new();
            let indices: Vec<i32> = if reverse { (0..32).rev().collect() } else { (0..32).collect() };
            for index in indices {
                let first = ObjectId(100 + index as u32 * 2);
                let second = ObjectId(first.0 + 1);
                system.register_cave(second, index * 2, Team::Neutral);
                system.register_cave(first, index * 2, Team::Neutral);
                assert!(system.record_enter(index * 2, ObjectId(1000 + index as u32), second, Team::USA).0);
                assert!(system.record_enter(index * 2, ObjectId(2000 + index as u32), first, Team::USA).0);
            }
            system
        }

        let first = populated(false);
        let second = populated(true);
        let bytes = bincode_legacy::serialize(&first).unwrap();
        assert_eq!(bytes, bincode_legacy::serialize(&second).unwrap());
        let mut restored: HostCaveSystem = bincode_legacy::deserialize(&bytes).unwrap();
        assert_eq!(restored.cave_ids_for_index(0), vec![ObjectId(101), ObjectId(100)]);
        assert_eq!(restored.contained_for_index(0), vec![ObjectId(1000), ObjectId(2000)]);
        assert!(restored.network(1).is_none());
        let outcome = restored.on_cave_destroyed(ObjectId(101));
        assert_eq!(outcome.remapped_to, Some(ObjectId(100)));
        assert_eq!(restored.record_exit(0, ObjectId(1000), ObjectId(100)).0, Some(ObjectId(100)));
    }

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
        assert!(!sys.try_set_cave_index(a, 0));
        let _ = sys.record_exit(0, u, b);
        assert!(sys.try_set_cave_index(a, 0));
        assert_eq!(sys.cave_ids_for_index(0), vec![b, a]);
        assert!(sys.try_set_cave_index(b, 1));
        assert_eq!(sys.index_of(b), Some(1));
        assert_eq!(sys.index_switches, 2);
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
        let keep = sys.on_cave_destroyed(a);
        assert!(!keep.cave_in);
        assert_eq!(keep.remapped_to, Some(b));
        let last = sys.on_cave_destroyed(b);
        assert!(last.cave_in);
        assert_eq!(last.cave_in_units, vec![u]);
        assert_eq!(sys.contained_for_index(0), vec![u]);
        assert_eq!(sys.exits, 0);
        sys.finish_cave_in(0);
        assert!(sys.contained_for_index(0).is_empty());
        assert_eq!(sys.exits, 0);
        assert!(sys.honesty_cave_in_ok());
    }
}
