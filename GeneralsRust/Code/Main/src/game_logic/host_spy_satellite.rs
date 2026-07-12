//! Host SpySatellite special-power residual.
//!
//! Residual slice (playability):
//! - `DoSpecialPower(SpySatellite)` at a world location temporarily reveals FOW
//!   in a radius (retail SpecialPowerSpySatellite / SpySatellitePing path).
//! - Reveal uses ShroudManager looker counters + queued undo so fog returns
//!   after duration (DeletionUpdate lifetime residual).
//! - Honesty counters/flags for residual gates and tests.
//!
//! Fail-closed honesty:
//! - Not full OCL SpySatellitePing object spawn / DynamicShroudClearingRangeUpdate
//!   grow/shrink curve / StealthDetectorUpdate parity
//! - Not multiplayer shared-synced timer / academy / shortcut UI / radius cursor
//! - Not Common `Radar` minimap-cell scan list Xfer tables
//! - Not CIA Intelligence / SpyVisionUpdate setUnitsVisionSpied enemy-vision path

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const SPY_SATELLITE_LOGIC_FPS: f32 = 30.0;

/// Retail `SpecialPowerSpySatellite` / `SpySatellitePing` shroud radius residual.
/// Matches SpecialPower.ini RadiusCursorRadius and System.ini VisionRange = 300.
pub const SPY_SATELLITE_RADIUS: f32 = 300.0;

/// Retail SpySatellitePing DeletionUpdate Min/MaxLifetime = 13000 ms @ 30 FPS.
pub const SPY_SATELLITE_DURATION_FRAMES: u32 = 390;

/// Activate audio residual (SpecialPower.ini InitiateAtLocationSound).
pub const SPY_SATELLITE_ACTIVATE_AUDIO: &str = "SpySatellite";

/// One active residual spy satellite scan (host-side bookkeeping for honesty / expiry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostSpySatellite {
    pub id: u32,
    pub player_id: u32,
    pub player_mask: u32,
    pub location: Vec3,
    pub radius: f32,
    pub activate_frame: u32,
    pub expires_frame: u32,
    pub caster_id: Option<ObjectId>,
    /// True after ShroudManager confirmed center cell visible for player.
    pub fow_reveal_ok: bool,
}

impl HostSpySatellite {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn contains_horizontal(&self, pos: Vec3) -> bool {
        let dx = pos.x - self.location.x;
        let dz = pos.z - self.location.z;
        dx * dx + dz * dz <= self.radius * self.radius
    }
}

/// Host residual registry for SpySatellite special power activations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostSpySatelliteRegistry {
    next_id: u32,
    /// Active (not yet expired) residual scans.
    active: Vec<HostSpySatellite>,
    /// Total activations (honesty).
    pub activations: u32,
    /// Activations that observably cleared FOW at the scan center.
    pub fow_reveals: u32,
    /// Scans that have expired (undo applied or tracked past expires_frame).
    pub expirations: u32,
}

impl HostSpySatelliteRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_scans(&self) -> &[HostSpySatellite] {
        &self.active
    }

    pub fn activations(&self) -> u32 {
        self.activations
    }

    pub fn fow_reveals(&self) -> u32 {
        self.fow_reveals
    }

    pub fn expirations(&self) -> u32 {
        self.expirations
    }

    /// Record a successful residual activation.
    pub fn record_activation(&mut self, scan: HostSpySatellite) {
        self.activations = self.activations.saturating_add(1);
        if scan.fow_reveal_ok {
            self.fow_reveals = self.fow_reveals.saturating_add(1);
        }
        self.active.push(scan);
    }

    /// Drop expired bookkeeping entries (shroud undo is handled separately).
    pub fn prune_expired(&mut self, current_frame: u32) {
        let before = self.active.len();
        self.active.retain(|s| !s.is_expired(current_frame));
        let removed = before.saturating_sub(self.active.len()) as u32;
        self.expirations = self.expirations.saturating_add(removed);
    }

    /// Allocate the next residual scan id.
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Residual honesty: at least one scan activated.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activations > 0
    }

    /// Residual honesty: FOW reveal was observed at least once.
    pub fn honesty_fow_reveal_ok(&self) -> bool {
        self.fow_reveals > 0
    }

    /// Combined host path: activated and FOW-visible residual.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_fow_reveal_ok()
    }

    /// True if any active residual scan covers `pos` for `player_id`.
    pub fn is_position_in_active_scan(&self, player_id: u32, pos: Vec3) -> bool {
        self.active
            .iter()
            .any(|s| s.player_id == player_id && s.contains_horizontal(pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_records_activation_and_honesty() {
        let mut reg = HostSpySatelliteRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.alloc_id();
        reg.record_activation(HostSpySatellite {
            id,
            player_id: 0,
            player_mask: 1,
            location: Vec3::new(100.0, 0.0, 100.0),
            radius: SPY_SATELLITE_RADIUS,
            activate_frame: 0,
            expires_frame: SPY_SATELLITE_DURATION_FRAMES,
            caster_id: Some(ObjectId(1)),
            fow_reveal_ok: true,
        });
        assert_eq!(reg.activations(), 1);
        assert_eq!(reg.fow_reveals(), 1);
        assert_eq!(reg.active_count(), 1);
        assert!(reg.honesty_host_path_ok());
        assert!(reg.is_position_in_active_scan(0, Vec3::new(100.0, 0.0, 100.0)));
        // Radius 300 must cover a point 200 units away.
        assert!(reg.is_position_in_active_scan(0, Vec3::new(280.0, 0.0, 100.0)));
        assert!(!reg.is_position_in_active_scan(0, Vec3::new(500.0, 0.0, 500.0)));

        reg.prune_expired(SPY_SATELLITE_DURATION_FRAMES);
        assert_eq!(reg.active_count(), 0);
        assert_eq!(reg.expirations(), 1);
        // Honesty remains after expiry (historical).
        assert!(reg.honesty_host_path_ok());
    }
}
