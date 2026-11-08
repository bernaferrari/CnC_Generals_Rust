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
pub struct BeaconDisplayState {
    markers: Vec<BeaconMarker>,
    pending: Vec<BeaconNotification>,
    residual_last_action: u8,
    residual_marker_count: usize,
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

pub fn record_beacon_placed(
    state: &mut BeaconDisplayState,
    player_id: i32,
    position: Coord3D,
    text: Option<String>,
) {
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

pub fn record_beacon_removed(state: &mut BeaconDisplayState, player_id: i32, position: Coord3D) {
    state.remove(player_id, &position);
}

pub fn record_beacon_text(
    state: &mut BeaconDisplayState,
    player_id: i32,
    position: Coord3D,
    text: String,
) {
    state.update_text(player_id, &position, text);
}

pub fn drain_notifications(state: &mut BeaconDisplayState) -> Vec<BeaconNotification> {
    state.drain_notifications()
}

pub fn snapshot_markers(state: &BeaconDisplayState) -> Vec<BeaconMarker> {
    state.markers.clone()
}

/// Residual: last Beacon display action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualBeaconAction {
    None = 0,
    Place = 1,
    Remove = 2,
    Text = 3,
    Drain = 4,
}

fn residual_beacon_action_store(state: &mut BeaconDisplayState, action: ResidualBeaconAction) {
    state.residual_last_action = action as u8;
}

/// Residual: last Beacon residual action.
pub fn residual_beacon_last_action(state: &BeaconDisplayState) -> ResidualBeaconAction {
    match state.residual_last_action {
        1 => ResidualBeaconAction::Place,
        2 => ResidualBeaconAction::Remove,
        3 => ResidualBeaconAction::Text,
        4 => ResidualBeaconAction::Drain,
        _ => ResidualBeaconAction::None,
    }
}

/// Residual: current residual marker count (from snapshot).
pub fn residual_beacon_marker_count(state: &BeaconDisplayState) -> usize {
    state.residual_marker_count
}

fn residual_beacon_sync_count(state: &mut BeaconDisplayState) {
    state.residual_marker_count = state.markers.len();
}

/// Residual: place beacon without multiplayer/command stream.
pub fn simulate_beacon_place(
    state: &mut BeaconDisplayState,
    player_id: i32, x: f32, y: f32, z: f32, text: Option<&str>) -> bool {
    record_beacon_placed(state, player_id, Coord3D { x, y, z }, text.map(|s| s.to_string()));
    residual_beacon_sync_count(state);
    residual_beacon_action_store(state, ResidualBeaconAction::Place);
    residual_beacon_marker_count(state) > 0
}

/// Residual: remove beacon without multiplayer/command stream.
pub fn simulate_beacon_remove(
    state: &mut BeaconDisplayState,
    player_id: i32, x: f32, y: f32, z: f32) -> bool {
    record_beacon_removed(state, player_id, Coord3D { x, y, z });
    residual_beacon_sync_count(state);
    residual_beacon_action_store(state, ResidualBeaconAction::Remove);
    true
}

/// Residual: update beacon text without edit widget.
pub fn simulate_beacon_set_text(
    state: &mut BeaconDisplayState,
    player_id: i32, x: f32, y: f32, z: f32, text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    record_beacon_text(state, player_id, Coord3D { x, y, z }, text.to_string());
    residual_beacon_sync_count(state);
    residual_beacon_action_store(state, ResidualBeaconAction::Text);
    true
}

/// Residual: drain pending beacon notifications.
pub fn simulate_beacon_drain_notifications(state: &mut BeaconDisplayState) -> usize {
    let n = drain_notifications(state).len();
    residual_beacon_action_store(state, ResidualBeaconAction::Drain);
    residual_beacon_sync_count(state);;
    n
}

/// Residual: place then set text composite.
pub fn simulate_beacon_prepare_place_with_text(
    state: &mut BeaconDisplayState,
    player_id: i32,
    x: f32,
    y: f32,
    z: f32,
    text: &str,
) -> bool {
    if !simulate_beacon_place(state, player_id, x, y, z, None) {
        return false;
    }
    simulate_beacon_set_text(state, player_id, x, y, z, text)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacon_state_is_isolated_per_owner() {
        let mut a = BeaconDisplayState::default();
        let mut b = BeaconDisplayState::default();
        assert!(simulate_beacon_place(&mut a, 1, 5.0, 6.0, 0.0, Some("a")));
        assert_eq!(snapshot_markers(&a).len(), 1);
        assert!(snapshot_markers(&b).is_empty());
        assert!(simulate_beacon_place(&mut b, 2, 9.0, 9.0, 0.0, Some("b")));
        assert!(simulate_beacon_remove(&mut a, 1, 5.0, 6.0, 0.0));
        assert!(snapshot_markers(&a).is_empty());
        assert_eq!(snapshot_markers(&b).len(), 1);
    }
}
