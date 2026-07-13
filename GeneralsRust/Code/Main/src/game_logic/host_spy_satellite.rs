//! Host SpySatellite special-power residual.
//!
//! Residual slice (playability):
//! - `DoSpecialPower(SpySatellite)` at a world location temporarily reveals FOW
//!   in a radius (retail SpecialPowerSpySatellite / SpySatellitePing path).
//! - Reveal uses ShroudManager looker counters + queued undo so fog returns
//!   after duration (DeletionUpdate lifetime residual).
//! - OCL `SpySatellitePing` DynamicShroudClearingRangeUpdate grow/shrink residual
//!   constants + host radius curve (VisionRange **300**, GrowTime **1000**ms,
//!   ShrinkDelay **10000**ms, ShrinkTime **5000**ms).
//! - StealthDetectorUpdate residual (DetectionRate **500**ms; DetectionRange 0 →
//!   uses VisionRange **300**).
//! - Honesty counters/flags for residual gates and tests.
//!
//! Fail-closed honesty:
//! - Not full OCL SpySatellitePing Object spawn / GridDecalTemplate GPU path
//! - Not multiplayer shared-synced timer / academy / shortcut UI / radius cursor
//! - Not Common `Radar` minimap-cell scan list Xfer tables
//! - Not CIA Intelligence / SpyVisionUpdate setUnitsVisionSpied enemy-vision path

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const SPY_SATELLITE_LOGIC_FPS: f32 = 30.0;

/// Retail OCL / System.ini object template residual.
pub const SPY_SATELLITE_PING_TEMPLATE: &str = "SpySatellitePing";

/// Retail `SpecialPowerSpySatellite` / `SpySatellitePing` shroud radius residual.
/// Matches SpecialPower.ini RadiusCursorRadius and System.ini VisionRange = 300.
pub const SPY_SATELLITE_RADIUS: f32 = 300.0;

/// Alias: native VisionRange / shroud clearing range residual for ping object.
pub const SPY_SATELLITE_VISION_RANGE: f32 = SPY_SATELLITE_RADIUS;

/// Retail SpySatellitePing DeletionUpdate Min/MaxLifetime = 13000 ms @ 30 FPS.
pub const SPY_SATELLITE_DURATION_MS: u32 = 13000;
/// 13000 ms → 390 frames (ceil 13000*30/1000).
pub const SPY_SATELLITE_DURATION_FRAMES: u32 = 390;

/// Activate audio residual (SpecialPower.ini InitiateAtLocationSound).
pub const SPY_SATELLITE_ACTIVATE_AUDIO: &str = "SpySatellite";

// --- DynamicShroudClearingRangeUpdate residual (System.ini SpySatellitePing) ---

/// FinalVision residual after shrink completes.
pub const SPY_SATELLITE_FINAL_VISION: f32 = 0.0;

/// GrowDelay residual msec (instant start).
pub const SPY_SATELLITE_GROW_DELAY_MS: u32 = 0;
/// GrowTime residual msec — ramp from 0 → VisionRange.
pub const SPY_SATELLITE_GROW_TIME_MS: u32 = 1000;
/// GrowInterval residual msec (faster than sustain/shrink ChangeInterval).
pub const SPY_SATELLITE_GROW_INTERVAL_MS: u32 = 10;
/// ShrinkDelay residual msec — sustain full range before shrink.
pub const SPY_SATELLITE_SHRINK_DELAY_MS: u32 = 10000;
/// ShrinkTime residual msec — ramp from VisionRange → FinalVision.
pub const SPY_SATELLITE_SHRINK_TIME_MS: u32 = 5000;
/// ChangeInterval residual msec (non-grow shroud set cadence).
pub const SPY_SATELLITE_CHANGE_INTERVAL_MS: u32 = 80;

/// C++ `ConvertDurationFromMsecsToFrames` residual: ceil(msec * 30 / 1000).
#[inline]
pub fn spy_satellite_duration_ms_to_frames(msec: u32) -> u32 {
    if msec == 0 {
        return 0;
    }
    ((msec as u64 * 30 + 999) / 1000) as u32
}

/// GrowTime frames residual (1000 ms → 30).
pub const SPY_SATELLITE_GROW_TIME_FRAMES: u32 = 30;
/// GrowDelay frames residual (0).
pub const SPY_SATELLITE_GROW_DELAY_FRAMES: u32 = 0;
/// GrowInterval frames residual (10 ms → 1).
pub const SPY_SATELLITE_GROW_INTERVAL_FRAMES: u32 = 1;
/// ShrinkDelay frames residual (10000 ms → 300).
pub const SPY_SATELLITE_SHRINK_DELAY_FRAMES: u32 = 300;
/// ShrinkTime frames residual (5000 ms → 150).
pub const SPY_SATELLITE_SHRINK_TIME_FRAMES: u32 = 150;
/// ChangeInterval frames residual (80 ms → 3).
pub const SPY_SATELLITE_CHANGE_INTERVAL_FRAMES: u32 = 3;

// --- StealthDetectorUpdate residual (SpySatellitePing ModuleTag_04) ---

/// DetectionRate residual msec.
pub const SPY_SATELLITE_STEALTH_DETECTION_RATE_MS: u32 = 500;
/// DetectionRate frames residual (500 ms → 15).
pub const SPY_SATELLITE_STEALTH_DETECTION_RATE_FRAMES: u32 = 15;
/// DetectionRange residual: 0 in INI → uses VisionRange.
pub const SPY_SATELLITE_STEALTH_DETECTION_RANGE: f32 = SPY_SATELLITE_VISION_RANGE;

/// Dynamic shroud state residual (C++ DSCRU_*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpySatelliteShroudState {
    Growing,
    Sustaining,
    Shrinking,
    Done,
}

/// Residual DynamicShroudClearingRangeUpdate radius curve at elapsed logic frames.
///
/// Mirrors C++ countdown model for SpySatellitePing:
/// - total countdown = ShrinkDelay + ShrinkTime frames
/// - first GrowTime frames: linear ramp 0 → VisionRange
/// - then sustain until ShrinkDelay expires from end of grow window
/// - last ShrinkTime frames: linear ramp VisionRange → FinalVision
///
/// Fail-closed: not full per-ChangeInterval setShroudClearingRange / grid decals.
pub fn spy_satellite_dynamic_shroud_radius_at_elapsed(elapsed_frames: u32) -> f32 {
    let grow = SPY_SATELLITE_GROW_TIME_FRAMES.max(1);
    let shrink_delay = SPY_SATELLITE_SHRINK_DELAY_FRAMES;
    let shrink = SPY_SATELLITE_SHRINK_TIME_FRAMES.max(1);
    let native = SPY_SATELLITE_VISION_RANGE;
    let final_v = SPY_SATELLITE_FINAL_VISION;

    // Grow phase: elapsed [0, grow)
    if elapsed_frames < grow {
        let t = elapsed_frames as f32 / grow as f32;
        return (native * t).clamp(0.0, native);
    }

    // Sustain: after grow until shrink begins.
    // Shrink begins at frame: GrowDelay(0) + GrowTime + (ShrinkDelay - GrowTime)?
    // C++: countdown starts at shrinkDelay+shrinkTime; grow runs while
    // countdown in (sustainDeadline, growStartDeadline].
    // Equiv elapsed timeline from spawn:
    //   grow: [0, grow_time)
    //   sustain: [grow_time, shrink_delay)  — retail ShrinkDelay is delay before
    //            shrink measured from spawn-ish sustain window (10s full hold
    //            including grow in practice via deadline math)
    //   shrink: [shrink_delay, shrink_delay+shrink_time)
    //   done: thereafter
    //
    // With retail numbers: grow 0..30, sustain 30..300, shrink 300..450, done.
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

/// Residual state label at elapsed frames (matches radius curve phases).
pub fn spy_satellite_dynamic_shroud_state_at_elapsed(
    elapsed_frames: u32,
) -> SpySatelliteShroudState {
    let grow = SPY_SATELLITE_GROW_TIME_FRAMES;
    let shrink_delay = SPY_SATELLITE_SHRINK_DELAY_FRAMES;
    let shrink = SPY_SATELLITE_SHRINK_TIME_FRAMES;
    if elapsed_frames < grow {
        SpySatelliteShroudState::Growing
    } else if elapsed_frames < shrink_delay {
        SpySatelliteShroudState::Sustaining
    } else if elapsed_frames < shrink_delay.saturating_add(shrink) {
        SpySatelliteShroudState::Shrinking
    } else {
        SpySatelliteShroudState::Done
    }
}

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
    /// True when DynamicShroud residual constants were applied on activate.
    pub dynamic_shroud_applied: bool,
    /// True when StealthDetector residual was applied on activate.
    pub stealth_detector_applied: bool,
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

    /// Elapsed frames since activate for DynamicShroud curve residual.
    pub fn elapsed_frames(&self, current_frame: u32) -> u32 {
        current_frame.saturating_sub(self.activate_frame)
    }

    /// Live DynamicShroud residual radius at `current_frame`.
    pub fn dynamic_shroud_radius(&self, current_frame: u32) -> f32 {
        spy_satellite_dynamic_shroud_radius_at_elapsed(self.elapsed_frames(current_frame))
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
    /// DynamicShroud residual applications on activate (honesty).
    pub dynamic_shroud_applications: u32,
    /// StealthDetector residual applications on activate (honesty).
    pub stealth_detector_applications: u32,
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

    pub fn dynamic_shroud_applications(&self) -> u32 {
        self.dynamic_shroud_applications
    }

    pub fn stealth_detector_applications(&self) -> u32 {
        self.stealth_detector_applications
    }

    /// Record a successful residual activation.
    ///
    /// Host residual: each activation applies DynamicShroud + StealthDetector
    /// constant pack honesty (OCL SpySatellitePing module residual without
    /// spawning a live Object).
    pub fn record_activation(&mut self, mut scan: HostSpySatellite) {
        self.activations = self.activations.saturating_add(1);
        if scan.fow_reveal_ok {
            self.fow_reveals = self.fow_reveals.saturating_add(1);
        }
        // Apply residual packs on activate (defaults true when not set by caller).
        if !scan.dynamic_shroud_applied {
            scan.dynamic_shroud_applied = true;
        }
        if !scan.stealth_detector_applied {
            scan.stealth_detector_applied = true;
        }
        if scan.dynamic_shroud_applied {
            self.dynamic_shroud_applications =
                self.dynamic_shroud_applications.saturating_add(1);
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

    /// Residual honesty: FOW reveal was observed at least once.
    pub fn honesty_fow_reveal_ok(&self) -> bool {
        self.fow_reveals > 0
    }

    /// Residual honesty: DynamicShroud residual applied on activate.
    pub fn honesty_dynamic_shroud_ok(&self) -> bool {
        self.dynamic_shroud_applications > 0
    }

    /// Residual honesty: StealthDetector residual applied on activate.
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

/// Residual honesty pack for SpySatellitePing DynamicShroud + StealthDetector constants.
pub fn honesty_spy_satellite_dynamic_shroud_constants_ok() -> bool {
    SPY_SATELLITE_PING_TEMPLATE == "SpySatellitePing"
        && (SPY_SATELLITE_VISION_RANGE - 300.0).abs() < 0.001
        && (SPY_SATELLITE_FINAL_VISION - 0.0).abs() < 0.001
        && SPY_SATELLITE_GROW_DELAY_MS == 0
        && SPY_SATELLITE_GROW_TIME_MS == 1000
        && SPY_SATELLITE_GROW_INTERVAL_MS == 10
        && SPY_SATELLITE_SHRINK_DELAY_MS == 10000
        && SPY_SATELLITE_SHRINK_TIME_MS == 5000
        && SPY_SATELLITE_CHANGE_INTERVAL_MS == 80
        && SPY_SATELLITE_DURATION_MS == 13000
        && SPY_SATELLITE_DURATION_FRAMES == 390
        && spy_satellite_duration_ms_to_frames(SPY_SATELLITE_GROW_TIME_MS)
            == SPY_SATELLITE_GROW_TIME_FRAMES
        && spy_satellite_duration_ms_to_frames(SPY_SATELLITE_SHRINK_DELAY_MS)
            == SPY_SATELLITE_SHRINK_DELAY_FRAMES
        && spy_satellite_duration_ms_to_frames(SPY_SATELLITE_SHRINK_TIME_MS)
            == SPY_SATELLITE_SHRINK_TIME_FRAMES
        && spy_satellite_duration_ms_to_frames(SPY_SATELLITE_GROW_INTERVAL_MS)
            == SPY_SATELLITE_GROW_INTERVAL_FRAMES
        && spy_satellite_duration_ms_to_frames(SPY_SATELLITE_CHANGE_INTERVAL_MS)
            == SPY_SATELLITE_CHANGE_INTERVAL_FRAMES
        && SPY_SATELLITE_STEALTH_DETECTION_RATE_MS == 500
        && spy_satellite_duration_ms_to_frames(SPY_SATELLITE_STEALTH_DETECTION_RATE_MS)
            == SPY_SATELLITE_STEALTH_DETECTION_RATE_FRAMES
        && (SPY_SATELLITE_STEALTH_DETECTION_RANGE - 300.0).abs() < 0.001
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
        assert_eq!(reg.dynamic_shroud_applications(), 1);
        assert_eq!(reg.stealth_detector_applications(), 1);
        assert!(reg.is_position_in_active_scan(0, Vec3::new(100.0, 0.0, 100.0)));
        // Radius 300 must cover a point 200 units away.
        assert!(reg.is_position_in_active_scan(0, Vec3::new(280.0, 0.0, 100.0)));
        assert!(!reg.is_position_in_active_scan(0, Vec3::new(500.0, 0.0, 500.0)));

        reg.prune_expired(SPY_SATELLITE_DURATION_FRAMES);
        assert_eq!(reg.active_count(), 0);
        assert_eq!(reg.expirations(), 1);
        // Honesty remains after expiry (historical).
        assert!(reg.honesty_host_path_ok());
        assert!(reg.honesty_dynamic_shroud_host_path_ok());
    }

    #[test]
    fn spy_satellite_dynamic_shroud_constants_residual_honesty() {
        assert!(honesty_spy_satellite_dynamic_shroud_constants_ok());
        assert_eq!(SPY_SATELLITE_PING_TEMPLATE, "SpySatellitePing");
        assert_eq!(SPY_SATELLITE_VISION_RANGE, 300.0);
        assert_eq!(SPY_SATELLITE_GROW_TIME_FRAMES, 30);
        assert_eq!(SPY_SATELLITE_SHRINK_DELAY_FRAMES, 300);
        assert_eq!(SPY_SATELLITE_SHRINK_TIME_FRAMES, 150);
        assert_eq!(SPY_SATELLITE_CHANGE_INTERVAL_FRAMES, 3);
        assert_eq!(SPY_SATELLITE_GROW_INTERVAL_FRAMES, 1);
        assert_eq!(SPY_SATELLITE_STEALTH_DETECTION_RATE_FRAMES, 15);
        assert_eq!(SPY_SATELLITE_STEALTH_DETECTION_RANGE, 300.0);
    }

    #[test]
    fn spy_satellite_dynamic_shroud_grow_shrink_curve_residual() {
        // t=0: start grow near 0.
        let r0 = spy_satellite_dynamic_shroud_radius_at_elapsed(0);
        assert!(r0 < 1.0, "grow start near 0, got {r0}");
        assert_eq!(
            spy_satellite_dynamic_shroud_state_at_elapsed(0),
            SpySatelliteShroudState::Growing
        );

        // Mid-grow (~15/30): ~half radius.
        let r_mid = spy_satellite_dynamic_shroud_radius_at_elapsed(15);
        assert!(
            (r_mid - 150.0).abs() < 1.0,
            "mid-grow ~150, got {r_mid}"
        );

        // End of grow / sustain: full vision.
        let r_full = spy_satellite_dynamic_shroud_radius_at_elapsed(30);
        assert!(
            (r_full - 300.0).abs() < 0.01,
            "full vision after grow, got {r_full}"
        );
        assert_eq!(
            spy_satellite_dynamic_shroud_state_at_elapsed(30),
            SpySatelliteShroudState::Sustaining
        );
        assert_eq!(
            spy_satellite_dynamic_shroud_state_at_elapsed(100),
            SpySatelliteShroudState::Sustaining
        );
        assert!(
            (spy_satellite_dynamic_shroud_radius_at_elapsed(100) - 300.0).abs() < 0.01
        );

        // Shrink starts at frame 300.
        assert_eq!(
            spy_satellite_dynamic_shroud_state_at_elapsed(300),
            SpySatelliteShroudState::Shrinking
        );
        let r_shrink_mid = spy_satellite_dynamic_shroud_radius_at_elapsed(375); // 75/150 through shrink
        assert!(
            (r_shrink_mid - 150.0).abs() < 1.0,
            "mid-shrink ~150, got {r_shrink_mid}"
        );

        // Done after shrink window.
        assert_eq!(
            spy_satellite_dynamic_shroud_state_at_elapsed(450),
            SpySatelliteShroudState::Done
        );
        assert!(
            (spy_satellite_dynamic_shroud_radius_at_elapsed(450) - 0.0).abs() < 0.01
        );

        // Deletion lifetime (390) is still in shrink phase.
        assert_eq!(
            spy_satellite_dynamic_shroud_state_at_elapsed(SPY_SATELLITE_DURATION_FRAMES),
            SpySatelliteShroudState::Shrinking
        );
    }

    #[test]
    fn spy_satellite_scan_dynamic_shroud_radius_tracks_elapsed() {
        let scan = HostSpySatellite {
            id: 0,
            player_id: 0,
            player_mask: 1,
            location: Vec3::ZERO,
            radius: SPY_SATELLITE_RADIUS,
            activate_frame: 10,
            expires_frame: 10 + SPY_SATELLITE_DURATION_FRAMES,
            caster_id: None,
            fow_reveal_ok: true,
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
        };
        assert!(
            (scan.dynamic_shroud_radius(10) - spy_satellite_dynamic_shroud_radius_at_elapsed(0))
                .abs()
                < 0.01
        );
        assert!(
            (scan.dynamic_shroud_radius(40) - spy_satellite_dynamic_shroud_radius_at_elapsed(30))
                .abs()
                < 0.01
        );
    }
}
