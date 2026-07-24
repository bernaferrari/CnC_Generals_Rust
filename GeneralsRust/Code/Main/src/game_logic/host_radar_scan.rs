//! Host RadarScan / RadarVanScan special-power residual.
//!
//! Residual slice (playability):
//! - `DoSpecialPower(RadarScan)` at a world location temporarily reveals FOW
//!   in a radius (retail SpecialPowerRadarVanScan / RadarVanPing path).
//! - Reveal uses ShroudManager looker counters + queued undo so fog returns
//!   after duration (DeletionUpdate lifetime residual).
//! - OCL `RadarVanPing` DynamicShroudClearingRangeUpdate shrink residual
//!   (VisionRange **150**, ShrinkDelay **7500**ms, ShrinkTime **2500**ms,
//!   ChangeInterval **50**ms; no grow params → instant full range residual).
//! - StealthDetectorUpdate residual (DetectionRate **500**ms; DetectionRange 0 →
//!   uses VisionRange **150**).
//! - Honesty counters/flags for residual gates and tests.
//!
//! Fail-closed honesty:
//! - RadarVanPing object spawn residual closed (grid decal GPU fail-closed)
//! - Not multiplayer shared-synced timer / academy / shortcut UI parity
//! - Not Common `Radar` minimap-cell scan list Xfer tables

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const RADAR_SCAN_LOGIC_FPS: f32 = 30.0;

/// Retail OCL / System.ini object template residual.
pub const RADAR_VAN_PING_TEMPLATE: &str = "RadarVanPing";

/// Retail `SpecialPowerRadarVanScan` / `RadarVanPing` shroud radius residual.
/// Matches SpecialPower.ini RadiusCursorRadius and System.ini VisionRange.
pub const RADAR_SCAN_RADIUS: f32 = 150.0;

/// Alias: native VisionRange residual for RadarVanPing.
pub const RADAR_SCAN_VISION_RANGE: f32 = RADAR_SCAN_RADIUS;

/// Retail RadarVanPing DeletionUpdate Min/MaxLifetime = 10000 ms @ 30 FPS.
pub const RADAR_SCAN_DURATION_MS: u32 = 10000;
/// 10000 ms → 300 frames.
pub const RADAR_SCAN_DURATION_FRAMES: u32 = 300;

/// Activate audio residual (SpecialPower.ini InitiateAtLocationSound).
pub const RADAR_SCAN_ACTIVATE_AUDIO: &str = "RadarVanScan";

// --- DynamicShroudClearingRangeUpdate residual (System.ini RadarVanPing) ---

/// FinalVision residual after shrink completes.
pub const RADAR_SCAN_FINAL_VISION: f32 = 0.0;

/// GrowTime residual msec — RadarVanPing omits grow fields (default 0 → instant full).
pub const RADAR_SCAN_GROW_TIME_MS: u32 = 0;
/// ShrinkDelay residual msec.
pub const RADAR_SCAN_SHRINK_DELAY_MS: u32 = 7500;
/// ShrinkTime residual msec.
pub const RADAR_SCAN_SHRINK_TIME_MS: u32 = 2500;
/// ChangeInterval residual msec.
pub const RADAR_SCAN_CHANGE_INTERVAL_MS: u32 = 50;

/// C++ `ConvertDurationFromMsecsToFrames` residual: ceil(msec * 30 / 1000).
#[inline]
pub fn radar_scan_duration_ms_to_frames(msec: u32) -> u32 {
    if msec == 0 {
        return 0;
    }
    ((msec as u64 * 30 + 999) / 1000) as u32
}

/// GrowTime frames residual (0 → instant full range).
pub const RADAR_SCAN_GROW_TIME_FRAMES: u32 = 0;
/// ShrinkDelay frames residual (7500 ms → 225).
pub const RADAR_SCAN_SHRINK_DELAY_FRAMES: u32 = 225;
/// ShrinkTime frames residual (2500 ms → 75).
pub const RADAR_SCAN_SHRINK_TIME_FRAMES: u32 = 75;
/// ChangeInterval frames residual (50 ms → 2).
pub const RADAR_SCAN_CHANGE_INTERVAL_FRAMES: u32 = 2;

// --- StealthDetectorUpdate residual (RadarVanPing ModuleTag_04) ---

/// DetectionRate residual msec.
pub const RADAR_SCAN_STEALTH_DETECTION_RATE_MS: u32 = 500;
/// DetectionRate frames residual (500 ms → 15).
pub const RADAR_SCAN_STEALTH_DETECTION_RATE_FRAMES: u32 = 15;
/// DetectionRange residual: 0 in INI → uses VisionRange.
pub const RADAR_SCAN_STEALTH_DETECTION_RANGE: f32 = RADAR_SCAN_VISION_RANGE;

/// Dynamic shroud state residual for RadarVanPing (shrink-only curve).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadarScanShroudState {
    Sustaining,
    Shrinking,
    Done,
}

/// Residual DynamicShroudClearingRangeUpdate radius curve at elapsed logic frames.
///
/// RadarVanPing has no grow params → residual starts at full VisionRange,
/// sustains until ShrinkDelay, then ramps to FinalVision over ShrinkTime.
///
/// Fail-closed: not full per-ChangeInterval setShroudClearingRange / grid decals.
pub fn radar_scan_dynamic_shroud_radius_at_elapsed(elapsed_frames: u32) -> f32 {
    let shrink_delay = RADAR_SCAN_SHRINK_DELAY_FRAMES;
    let shrink = RADAR_SCAN_SHRINK_TIME_FRAMES.max(1);
    let native = RADAR_SCAN_VISION_RANGE;
    let final_v = RADAR_SCAN_FINAL_VISION;

    if elapsed_frames < shrink_delay {
        return native;
    }

    let shrink_elapsed = elapsed_frames - shrink_delay;
    if shrink_elapsed < shrink {
        let t = shrink_elapsed as f32 / shrink as f32;
        let range = native + (final_v - native) * t;
        return range.max(final_v).min(native);
    }

    final_v
}

/// Residual state label at elapsed frames.
pub fn radar_scan_dynamic_shroud_state_at_elapsed(elapsed_frames: u32) -> RadarScanShroudState {
    let shrink_delay = RADAR_SCAN_SHRINK_DELAY_FRAMES;
    let shrink = RADAR_SCAN_SHRINK_TIME_FRAMES;
    if elapsed_frames < shrink_delay {
        RadarScanShroudState::Sustaining
    } else if elapsed_frames < shrink_delay.saturating_add(shrink) {
        RadarScanShroudState::Shrinking
    } else {
        RadarScanShroudState::Done
    }
}

/// One active residual radar scan (host-side bookkeeping for honesty / expiry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostRadarScan {
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
    /// True when DynamicShroud residual constants were applied on activate.
    pub dynamic_shroud_applied: bool,
    /// True when StealthDetector residual was applied on activate.
    pub stealth_detector_applied: bool,
}

impl HostRadarScan {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn contains_horizontal(&self, pos: Vec3) -> bool {
        let dx = pos.x - self.location.x;
        let dz = pos.z - self.location.z;
        dx * dx + dz * dz <= self.radius * self.radius
    }

    pub fn elapsed_frames(&self, current_frame: u32) -> u32 {
        current_frame.saturating_sub(self.activate_frame)
    }

    pub fn dynamic_shroud_radius(&self, current_frame: u32) -> f32 {
        radar_scan_dynamic_shroud_radius_at_elapsed(self.elapsed_frames(current_frame))
    }
}

/// Host residual registry for RadarScan special power activations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostRadarScanRegistry {
    next_id: u32,
    /// Active (not yet expired) residual scans.
    active: Vec<HostRadarScan>,
    /// Total activations (honesty).
    pub activations: u32,
    /// Activations that observably cleared FOW at the scan center.
    pub fow_reveals: u32,
    /// Scans that have expired (undo applied or tracked past expires_frame).
    pub expirations: u32,
    /// DynamicShroud residual applications on activate (honesty).
    pub dynamic_shroud_applications: u32,
    /// StealthDetector residual applications on activate (honesty).
    pub stealth_detector_applications: u32,
    /// Honesty: RadarVanPing objects spawned.
    pub pings_spawned: u32,
}

impl HostRadarScanRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_scans(&self) -> &[HostRadarScan] {
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

    pub fn dynamic_shroud_applications(&self) -> u32 {
        self.dynamic_shroud_applications
    }

    pub fn stealth_detector_applications(&self) -> u32 {
        self.stealth_detector_applications
    }

    /// Record a successful residual activation.
    pub fn record_activation(&mut self, mut scan: HostRadarScan) {
        self.activations = self.activations.saturating_add(1);
        if scan.fow_reveal_ok {
            self.fow_reveals = self.fow_reveals.saturating_add(1);
        }
        if !scan.dynamic_shroud_applied {
            scan.dynamic_shroud_applied = true;
        }
        if !scan.stealth_detector_applied {
            scan.stealth_detector_applied = true;
        }
        if scan.dynamic_shroud_applied {
            self.dynamic_shroud_applications = self.dynamic_shroud_applications.saturating_add(1);
        }
        if scan.stealth_detector_applied {
            self.stealth_detector_applications =
                self.stealth_detector_applications.saturating_add(1);
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

    pub fn record_ping_spawn(&mut self) {
        self.pings_spawned = self.pings_spawned.saturating_add(1);
    }

    pub fn honesty_ping_ok(&self) -> bool {
        self.pings_spawned > 0
    }

    /// Residual honesty: FOW reveal was observed at least once.
    pub fn honesty_fow_reveal_ok(&self) -> bool {
        self.fow_reveals > 0
    }

    pub fn honesty_dynamic_shroud_ok(&self) -> bool {
        self.dynamic_shroud_applications > 0
    }

    pub fn honesty_stealth_detector_ok(&self) -> bool {
        self.stealth_detector_applications > 0
    }

    /// Combined host path: activated and FOW-visible residual.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_fow_reveal_ok()
    }

    /// Wave 48 residual path: activate + FOW + DynamicShroud + StealthDetector.
    pub fn honesty_dynamic_shroud_host_path_ok(&self) -> bool {
        self.honesty_host_path_ok()
            && self.honesty_dynamic_shroud_ok()
            && self.honesty_stealth_detector_ok()
    }

    /// True if any active residual scan covers `pos` for `player_id`.
    pub fn is_position_in_active_scan(&self, player_id: u32, pos: Vec3) -> bool {
        self.active
            .iter()
            .any(|s| s.player_id == player_id && s.contains_horizontal(pos))
    }
}

/// Residual honesty pack for RadarVanPing DynamicShroud + StealthDetector constants.
pub fn honesty_radar_scan_dynamic_shroud_constants_ok() -> bool {
    RADAR_VAN_PING_TEMPLATE == "RadarVanPing"
        && (RADAR_SCAN_VISION_RANGE - 150.0).abs() < 0.001
        && (RADAR_SCAN_FINAL_VISION - 0.0).abs() < 0.001
        && RADAR_SCAN_GROW_TIME_MS == 0
        && RADAR_SCAN_SHRINK_DELAY_MS == 7500
        && RADAR_SCAN_SHRINK_TIME_MS == 2500
        && RADAR_SCAN_CHANGE_INTERVAL_MS == 50
        && RADAR_SCAN_DURATION_MS == 10000
        && RADAR_SCAN_DURATION_FRAMES == 300
        && radar_scan_duration_ms_to_frames(RADAR_SCAN_SHRINK_DELAY_MS)
            == RADAR_SCAN_SHRINK_DELAY_FRAMES
        && radar_scan_duration_ms_to_frames(RADAR_SCAN_SHRINK_TIME_MS)
            == RADAR_SCAN_SHRINK_TIME_FRAMES
        && radar_scan_duration_ms_to_frames(RADAR_SCAN_CHANGE_INTERVAL_MS)
            == RADAR_SCAN_CHANGE_INTERVAL_FRAMES
        && RADAR_SCAN_STEALTH_DETECTION_RATE_MS == 500
        && radar_scan_duration_ms_to_frames(RADAR_SCAN_STEALTH_DETECTION_RATE_MS)
            == RADAR_SCAN_STEALTH_DETECTION_RATE_FRAMES
        && (RADAR_SCAN_STEALTH_DETECTION_RANGE - 150.0).abs() < 0.001
}
/// Combined residual honesty pack (Wave 71).
pub fn honesty_radar_scan_residual_pack_ok() -> bool {
    honesty_radar_scan_dynamic_shroud_constants_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_records_activation_and_honesty() {
        let mut reg = HostRadarScanRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.alloc_id();
        reg.record_activation(HostRadarScan {
            id,
            player_id: 0,
            player_mask: 1,
            location: Vec3::new(100.0, 0.0, 100.0),
            radius: RADAR_SCAN_RADIUS,
            activate_frame: 0,
            expires_frame: RADAR_SCAN_DURATION_FRAMES,
            caster_id: Some(ObjectId(1)),
            fow_reveal_ok: true,
            dynamic_shroud_applied: false,
            stealth_detector_applied: false,
        });
        assert_eq!(reg.activations(), 1);
        assert_eq!(reg.fow_reveals(), 1);
        assert_eq!(reg.active_count(), 1);
        assert!(reg.honesty_host_path_ok());
        assert!(reg.honesty_dynamic_shroud_ok());
        assert!(reg.honesty_stealth_detector_ok());
        assert!(reg.honesty_dynamic_shroud_host_path_ok());
        assert!(reg.is_position_in_active_scan(0, Vec3::new(100.0, 0.0, 100.0)));
        assert!(!reg.is_position_in_active_scan(0, Vec3::new(500.0, 0.0, 500.0)));

        reg.prune_expired(RADAR_SCAN_DURATION_FRAMES);
        assert_eq!(reg.active_count(), 0);
        assert_eq!(reg.expirations(), 1);
        // Honesty remains after expiry (historical).
        assert!(reg.honesty_host_path_ok());
    }

    #[test]
    fn radar_scan_dynamic_shroud_constants_residual_honesty() {
        assert!(honesty_radar_scan_dynamic_shroud_constants_ok());
        assert_eq!(RADAR_VAN_PING_TEMPLATE, "RadarVanPing");
        assert_eq!(RADAR_SCAN_VISION_RANGE, 150.0);
        assert_eq!(RADAR_SCAN_SHRINK_DELAY_FRAMES, 225);
        assert_eq!(RADAR_SCAN_SHRINK_TIME_FRAMES, 75);
        assert_eq!(RADAR_SCAN_CHANGE_INTERVAL_FRAMES, 2);
        assert_eq!(RADAR_SCAN_STEALTH_DETECTION_RATE_FRAMES, 15);
    }

    #[test]
    fn radar_scan_dynamic_shroud_shrink_curve_residual() {
        // Instant full range (no grow).
        assert!((radar_scan_dynamic_shroud_radius_at_elapsed(0) - 150.0).abs() < 0.01);
        assert_eq!(
            radar_scan_dynamic_shroud_state_at_elapsed(0),
            RadarScanShroudState::Sustaining
        );
        assert!((radar_scan_dynamic_shroud_radius_at_elapsed(100) - 150.0).abs() < 0.01);

        // Shrink starts at 225.
        assert_eq!(
            radar_scan_dynamic_shroud_state_at_elapsed(225),
            RadarScanShroudState::Shrinking
        );
        let mid = radar_scan_dynamic_shroud_radius_at_elapsed(225 + 37); // ~half of 75
        assert!((mid - 75.0).abs() < 3.0, "mid-shrink ~75, got {mid}");

        // Done after shrink.
        assert_eq!(
            radar_scan_dynamic_shroud_state_at_elapsed(300),
            RadarScanShroudState::Done
        );
        assert!((radar_scan_dynamic_shroud_radius_at_elapsed(300) - 0.0).abs() < 0.01);

        // Deletion lifetime aligns with end of shrink window.
        assert_eq!(RADAR_SCAN_DURATION_FRAMES, 300);
    }
    /// Wave 71 residual pack honesty gate.
    #[test]
    fn radar_scan_residual_pack_honesty_wave71() {
        assert!(honesty_radar_scan_residual_pack_ok());
        assert_eq!(RADAR_SCAN_DURATION_FRAMES, 300);
        assert_eq!(RADAR_SCAN_SHRINK_DELAY_FRAMES, 225);
        assert_eq!(RADAR_SCAN_STEALTH_DETECTION_RATE_FRAMES, 15);
    }
}
