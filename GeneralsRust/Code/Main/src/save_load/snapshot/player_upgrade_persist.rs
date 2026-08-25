//! Persist C++ `Player::m_upgradesCompleted` onto the live host.
//!
//! C++ `Player::xfer` (`Player.cpp:4065`) writes the completed upgrade mask
//! as an `xferUpgradeMask` name list. Leftover `Player` already does that.
//! Live restore historically constructed `completed_upgrades` empty, so
//! Worker Shoes / Subliminal / Flashbang vanished after load.
//!
//! Nested `PlayerSnapshot.upgrades` is the name list (no version bump).
//! Completed `HostUpgradeRegistry` PLAYER entries cover research that only
//! lived in the registry at save time.

use super::player::PlayerSnapshot;
use super::types::WorldSnapshot;
use crate::game_logic::GameLogic;
use crate::game_logic::host_upgrades::{HostUpgradePhase, is_object_scoped_upgrade};

/// Write live `completed_upgrades` onto nested `PlayerSnapshot.upgrades`.
/// Also union completed PLAYER host-upgrade records so research that only
/// lived in the registry at save time still has a name list.
pub fn stamp_completed_upgrades(players: &mut [PlayerSnapshot], game_logic: &GameLogic) {
    for snap in players {
        let Some(player) = game_logic.get_player(snap.id) else {
            continue;
        };
        let mut names: Vec<String> = player.completed_upgrades.iter().cloned().collect();
        for entry in game_logic.host_upgrades().entries_snapshot() {
            if entry.player_id != snap.id {
                continue;
            }
            if entry.phase != HostUpgradePhase::Completed {
                continue;
            }
            if is_object_scoped_upgrade(&entry.name) {
                continue;
            }
            names.push(entry.name.clone());
        }
        names.sort();
        names.dedup();
        snap.upgrades = names;
    }
}

/// Rebuild live `completed_upgrades` from the snapshot name list plus
/// completed PLAYER host-upgrade records. C++ `addUpgrade(..., COMPLETE)`.
pub fn apply_completed_upgrades(snapshot: &WorldSnapshot, game_logic: &mut GameLogic) {
    for snap in &snapshot.players {
        let Some(player) = game_logic.get_player_mut(snap.id) else {
            continue;
        };
        for name in &snap.upgrades {
            player.add_completed_upgrade(name);
        }
    }
    for entry in &snapshot.host_upgrades.entries {
        if entry.phase != HostUpgradePhase::Completed {
            continue;
        }
        if is_object_scoped_upgrade(&entry.name) {
            continue;
        }
        if let Some(player) = game_logic.get_player_mut(entry.player_id) {
            player.add_completed_upgrade(&entry.name);
        }
    }
}

/// After `HostUpgradeRegistry` restore, copy completed PLAYER names onto
/// the live host mask. Does not need `WorldSnapshot` so it can live inside
/// `restore_host_upgrades`.
pub fn apply_from_live_registry(game_logic: &mut GameLogic) {
    let entries = game_logic.host_upgrades().entries_snapshot();
    for entry in entries {
        if entry.phase != HostUpgradePhase::Completed {
            continue;
        }
        if is_object_scoped_upgrade(&entry.name) {
            continue;
        }
        if let Some(player) = game_logic.get_player_mut(entry.player_id) {
            player.add_completed_upgrade(&entry.name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_upgrades::{
        UPGRADE_AMERICA_FLASHBANG, UPGRADE_CHINA_SUBLIMINAL_MESSAGING, UPGRADE_GLA_WORKER_SHOES,
    };
    use crate::game_logic::{Player, Team};

    fn has_upgrade(names: impl IntoIterator<Item = impl AsRef<str>>, want: &str) -> bool {
        names
            .into_iter()
            .any(|name| name.as_ref().eq_ignore_ascii_case(want))
    }

    #[test]
    fn complete_researched_upgrade_fills_completed_upgrades() {
        let mut player = Player::new(1, Team::GLA, "GLA", true);
        player.complete_researched_upgrade(UPGRADE_GLA_WORKER_SHOES);
        assert!(player.completed_upgrades.contains(UPGRADE_GLA_WORKER_SHOES));
        player.complete_researched_upgrade("Upgrade_BecomeRealGLABarracks");
        assert!(
            !player
                .completed_upgrades
                .iter()
                .any(|name| name.eq_ignore_ascii_case("Upgrade_BecomeRealGLABarracks"))
        );
    }

    #[test]
    fn snapshot_round_trips_player_completed_upgrades() {
        let mut source = GameLogic::new();
        let mut player = Player::new(1, Team::USA, "USA", true);
        player.add_completed_upgrade(UPGRADE_AMERICA_FLASHBANG);
        player.add_completed_upgrade(UPGRADE_CHINA_SUBLIMINAL_MESSAGING);
        player.add_completed_upgrade(UPGRADE_GLA_WORKER_SHOES);
        source.add_player(player);

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        let snap = snapshot
            .players
            .iter()
            .find(|p| p.id == 1)
            .expect("player snap");
        assert!(
            has_upgrade(snap.upgrades.iter(), UPGRADE_AMERICA_FLASHBANG),
            "stamp {:?}",
            snap.upgrades
        );
        assert!(has_upgrade(
            snap.upgrades.iter(),
            UPGRADE_CHINA_SUBLIMINAL_MESSAGING
        ));
        assert!(has_upgrade(snap.upgrades.iter(), UPGRADE_GLA_WORKER_SHOES));

        let mut restored = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.get_player(1).expect("player");
        assert!(
            loaded
                .completed_upgrades
                .contains(UPGRADE_AMERICA_FLASHBANG),
            "loaded {:?}",
            loaded.completed_upgrades
        );
        assert!(
            loaded
                .completed_upgrades
                .contains(UPGRADE_CHINA_SUBLIMINAL_MESSAGING)
        );
        assert!(loaded.completed_upgrades.contains(UPGRADE_GLA_WORKER_SHOES));
        assert!(loaded.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG));
        assert!(loaded.has_unlocked_upgrade(UPGRADE_CHINA_SUBLIMINAL_MESSAGING));
        assert!(loaded.has_unlocked_upgrade(UPGRADE_GLA_WORKER_SHOES));
    }

    #[test]
    fn registry_completed_player_upgrade_restores_when_hashset_empty() {
        let mut source = GameLogic::new();
        source.add_player(Player::new(1, Team::USA, "USA", true));
        source
            .host_upgrades_mut()
            .record_queue(UPGRADE_AMERICA_FLASHBANG, Team::USA, 1, 0, None);
        source
            .host_upgrades_mut()
            .record_complete(UPGRADE_AMERICA_FLASHBANG, 1, 10, 1);
        assert!(
            source
                .get_player(1)
                .expect("src")
                .completed_upgrades
                .is_empty()
        );

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            snapshot
                .players
                .iter()
                .find(|p| p.id == 1)
                .is_some_and(|p| has_upgrade(p.upgrades.iter(), UPGRADE_AMERICA_FLASHBANG)),
            "stamp must union completed registry names"
        );

        let mut restored = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.get_player(1).expect("player");
        assert!(
            loaded
                .completed_upgrades
                .contains(UPGRADE_AMERICA_FLASHBANG),
            "registry complete must refill completed_upgrades {:?}",
            loaded.completed_upgrades
        );
    }
}
