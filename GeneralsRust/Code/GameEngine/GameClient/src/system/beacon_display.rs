/*
**  Command & Conquer Generals Zero Hour(tm)
**  Client-side beacon visual/state tracking.
**
**  The original C++ GameClient reacted to beacon placement/removal commands
**  immediately so the HUD, EVA, and radar could respond without waiting for
**  round‑trip confirmation.  This module mirrors that behaviour by keeping a
**  lightweight cache of active beacon markers plus a queue of notifications
**  that the UI layer can consume every frame.
*/

use crate::message_stream::game_message::Coord3D;
use log::{info, warn};
use std::sync::{Mutex, OnceLock};

/// Matches the fuzzy comparison used by GameLogic when pairing commands with
/// already-active beacons.  Using the same tolerance keeps the client-side UI
/// aligned with the authoritative simulation.
pub(crate) const BEACON_MATCH_THRESHOLD: f32 = 3.0;

#[derive(Debug, Clone)]
pub struct BeaconMarker {
    pub player_id: i32,
    pub position: Coord3D,
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub enum BeaconNotification {
    Placed(BeaconMarker),
    Removed {
        player_id: i32,
        position: Coord3D,
    },
    TextUpdated {
        player_id: i32,
        position: Coord3D,
        text: String,
    },
}

#[derive(Default)]
struct BeaconDisplayState {
    markers: Vec<BeaconMarker>,
    pending: Vec<BeaconNotification>,
}

impl BeaconDisplayState {
    fn place(&mut self, marker: BeaconMarker) {
        if let Some(index) = self.find_marker_index(marker.player_id, &marker.position) {
            self.markers.remove(index);
        }

        self.pending
            .push(BeaconNotification::Placed(marker.clone()));
        self.markers.push(marker);
    }

    fn remove(&mut self, player_id: i32, position: &Coord3D) {
        if let Some(index) = self.find_marker_index(player_id, position) {
            let removed = self.markers.remove(index);
            self.pending.push(BeaconNotification::Removed {
                player_id,
                position: removed.position,
            });
        } else {
            warn!(
                "Unable to remove beacon for player {} near ({:.1}, {:.1}, {:.1})",
                player_id, position.x, position.y, position.z
            );
        }
    }

    fn update_text(&mut self, player_id: i32, position: &Coord3D, text: String) {
        if let Some(index) = self.find_marker_index(player_id, position) {
            self.markers[index].text = Some(text.clone());
            let position = self.markers[index].position.clone();
            self.pending.push(BeaconNotification::TextUpdated {
                player_id,
                position,
                text,
            });
        } else {
            warn!(
                "Unable to set beacon text for player {} near ({:.1}, {:.1}, {:.1})",
                player_id, position.x, position.y, position.z
            );
        }
    }

    fn find_marker_index(&self, player_id: i32, position: &Coord3D) -> Option<usize> {
        self.markers.iter().position(|marker| {
            marker.player_id == player_id
                && distance(&marker.position, position) <= BEACON_MATCH_THRESHOLD
        })
    }

    fn drain_notifications(&mut self) -> Vec<BeaconNotification> {
        std::mem::take(&mut self.pending)
    }
}

fn distance(a: &Coord3D, b: &Coord3D) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

static BEACON_DISPLAY: OnceLock<Mutex<BeaconDisplayState>> = OnceLock::new();

fn beacon_state() -> &'static Mutex<BeaconDisplayState> {
    BEACON_DISPLAY.get_or_init(|| Mutex::new(BeaconDisplayState::default()))
}

pub fn record_beacon_placed(player_id: i32, position: Coord3D, text: Option<String>) {
    if let Ok(mut state) = beacon_state().lock() {
        info!(
            "Player {} placed beacon at ({:.1}, {:.1}, {:.1})",
            player_id, position.x, position.y, position.z
        );
        state.place(BeaconMarker {
            player_id,
            position,
            text,
        });
    }
}

pub fn record_beacon_removed(player_id: i32, position: Coord3D) {
    if let Ok(mut state) = beacon_state().lock() {
        state.remove(player_id, &position);
    }
}

pub fn record_beacon_text(player_id: i32, position: Coord3D, text: String) {
    if let Ok(mut state) = beacon_state().lock() {
        state.update_text(player_id, &position, text);
    }
}

pub fn drain_notifications() -> Vec<BeaconNotification> {
    beacon_state()
        .lock()
        .map(|mut state| state.drain_notifications())
        .unwrap_or_default()
}

pub fn snapshot_markers() -> Vec<BeaconMarker> {
    beacon_state()
        .lock()
        .map(|state| state.markers.clone())
        .unwrap_or_default()
}
