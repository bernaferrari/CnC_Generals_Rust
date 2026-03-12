use crate::common::{AsciiString, Coord3D, Int, UnsignedInt};
use log::{info, warn};
use std::sync::{Mutex, OnceLock};

/// Maximum distance (in world units) used when matching beacon commands to
/// previously placed beacons. This mirrors the fuzzy comparison used in the
/// original C++ client where a beacon is addressed by the mouse location.
const BEACON_MATCH_THRESHOLD: f32 = 3.0;

/// Represents a player-issued beacon on the tactical map.
#[derive(Debug, Clone)]
pub struct BeaconEntry {
    pub player_id: Int,
    pub position: Coord3D,
    pub text: Option<AsciiString>,
    pub created_frame: UnsignedInt,
}

impl BeaconEntry {
    fn distance_to(&self, other: &Coord3D) -> f32 {
        (self.position - *other).length()
    }
}

/// Manages the collection of active beacons for the current match. The data is
/// consumed by UI and audio layers to display the familiar multiplayer markers.
#[derive(Debug, Default)]
pub struct BeaconManager {
    beacons: Vec<BeaconEntry>,
    pending_updates: Vec<BeaconUpdate>,
}

impl BeaconManager {
    pub fn new() -> Self {
        Self {
            beacons: Vec::new(),
            pending_updates: Vec::new(),
        }
    }

    /// Create or replace a beacon for the given player.
    pub fn place_beacon(
        &mut self,
        player_id: Int,
        position: Coord3D,
        frame: UnsignedInt,
    ) -> &BeaconEntry {
        // Remove prior beacon at nearly the same location to avoid duplicates.
        if let Some(index) = self.find_beacon_index(player_id, &position) {
            self.beacons.remove(index);
        }

        info!(
            "Player {} placed beacon at ({:.1}, {:.1}, {:.1}) [frame {}]",
            player_id, position.x, position.y, position.z, frame
        );

        let entry = BeaconEntry {
            player_id,
            position,
            text: None,
            created_frame: frame,
        };

        self.pending_updates
            .push(BeaconUpdate::Placed(entry.clone()));
        self.beacons.push(entry);

        self.beacons
            .last()
            .expect("beacon list is never empty after push")
    }

    /// Remove a beacon near the supplied position. Returns true when a beacon
    /// was removed, false when no matching beacon was found.
    pub fn remove_beacon(&mut self, player_id: Int, position: &Coord3D) -> bool {
        if let Some(index) = self.find_beacon_index(player_id, position) {
            let beacon = self.beacons.remove(index);
            info!(
                "Player {} removed beacon at ({:.1}, {:.1}, {:.1})",
                player_id, beacon.position.x, beacon.position.y, beacon.position.z
            );
            self.pending_updates.push(BeaconUpdate::Removed {
                player_id,
                position: beacon.position,
            });
            true
        } else {
            warn!(
                "Player {} attempted to remove missing beacon at ({:.1}, {:.1}, {:.1})",
                player_id, position.x, position.y, position.z
            );
            false
        }
    }

    /// Remove the most recently created beacon for the player. This mirrors the
    /// client-side "remove my beacon" command that does not supply an explicit
    /// position.
    pub fn remove_latest_beacon(&mut self, player_id: Int) -> bool {
        if let Some((index, beacon)) = self
            .beacons
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.player_id == player_id)
            .max_by_key(|(_, entry)| entry.created_frame)
            .map(|(i, entry)| (i, entry.clone()))
        {
            self.beacons.remove(index);
            info!(
                "Player {} removed latest beacon at ({:.1}, {:.1}, {:.1})",
                player_id, beacon.position.x, beacon.position.y, beacon.position.z
            );
            self.pending_updates.push(BeaconUpdate::Removed {
                player_id,
                position: beacon.position,
            });
            true
        } else {
            false
        }
    }

    /// Update the text for an existing beacon. Returns true when the beacon was
    /// found and updated.
    pub fn set_beacon_text(
        &mut self,
        player_id: Int,
        position: &Coord3D,
        text: AsciiString,
    ) -> bool {
        if let Some(index) = self.find_beacon_index(player_id, position) {
            let empty = text.is_empty();
            self.beacons[index].text = if empty { None } else { Some(text.clone()) };
            info!(
                "Player {} set beacon text near ({:.1}, {:.1}, {:.1}) to '{}'",
                player_id, position.x, position.y, position.z, text
            );
            self.pending_updates.push(BeaconUpdate::TextUpdated {
                player_id,
                position: self.beacons[index].position,
                text,
            });
            true
        } else {
            warn!(
                "Player {} attempted to set text for missing beacon at ({:.1}, {:.1}, {:.1})",
                player_id, position.x, position.y, position.z
            );
            false
        }
    }

    /// Drain accumulated updates since the last call. Consumers (UI/audio) call
    /// this each frame to react to beacon changes without scanning the entire
    /// list.
    pub fn drain_updates(&mut self) -> Vec<BeaconUpdate> {
        let mut drained = Vec::new();
        std::mem::swap(&mut self.pending_updates, &mut drained);
        drained
    }

    /// Snapshot current beacons for UI display/debug.
    pub fn snapshot(&self) -> Vec<BeaconEntry> {
        self.beacons.clone()
    }

    fn find_beacon_index(&self, player_id: Int, position: &Coord3D) -> Option<usize> {
        self.beacons.iter().position(|entry| {
            entry.player_id == player_id && entry.distance_to(position) <= BEACON_MATCH_THRESHOLD
        })
    }
}

static BEACON_MANAGER: OnceLock<Mutex<BeaconManager>> = OnceLock::new();

/// Access the global BeaconManager. The UI layer consumes entries from this
/// registry to display and synchronize strategic markers.
pub fn get_beacon_manager() -> &'static Mutex<BeaconManager> {
    BEACON_MANAGER.get_or_init(|| Mutex::new(BeaconManager::new()))
}

/// Convenience helper that drains all beacon updates since the previous call.
pub fn drain_beacon_updates() -> Vec<BeaconUpdate> {
    match get_beacon_manager().lock() {
        Ok(mut manager) => manager.drain_updates(),
        Err(_) => Vec::new(),
    }
}

/// Snapshot all active beacons for UI layers that want immediate state.
pub fn snapshot_beacons() -> Vec<BeaconEntry> {
    match get_beacon_manager().lock() {
        Ok(manager) => manager.snapshot(),
        Err(_) => Vec::new(),
    }
}

/// Describes the delta applied to the beacon list so rendering/audio layers can
/// react without re‑scanning the complete state each frame.
#[derive(Debug, Clone)]
pub enum BeaconUpdate {
    Placed(BeaconEntry),
    Removed {
        player_id: Int,
        position: Coord3D,
    },
    TextUpdated {
        player_id: Int,
        position: Coord3D,
        text: AsciiString,
    },
}
