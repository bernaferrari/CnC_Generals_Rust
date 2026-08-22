//! Host Satellite Hack I/II — C++ `SpyVisionUpdate::upgradeImplementation`.
//!
//! Retail China Internet Center authors two `SpyVisionUpdate` modules:
//! - Hack I (`TriggeredBy Upgrade_ChinaSatelliteHackOne`): self-powered,
//!   duration 0 (permanent), `SpyOnKindof = COMMANDCENTER`.
//! - Hack II (`TriggeredBy Upgrade_ChinaSatelliteHackTwo`): self-powered
//!   pulse (20s on / 220s off), default kindof (all units).
//!
//! Live previously only swapped the Internet Center command set.

use crate::game_logic::host_hacker_income::is_internet_center_template;
use crate::game_logic::host_upgrades::{
    UPGRADE_CHINA_SATELLITE_HACK_ONE, UPGRADE_CHINA_SATELLITE_HACK_TWO,
};

/// Retail Hack II `SelfPoweredDuration` residual (msec).
pub const SATELLITE_HACK_TWO_DURATION_MS: u32 = 20_000;
/// 20000 ms @ 30 FPS → 600 frames (`parseDurationUnsignedInt`).
pub const SATELLITE_HACK_TWO_DURATION_FRAMES: u32 = 600;
/// Retail Hack II `SelfPoweredInterval` residual (msec).
/// 20s on + 220s off = 4-minute cycle (wiki “25s every 4 min” is approximate).
pub const SATELLITE_HACK_TWO_INTERVAL_MS: u32 = 220_000;
/// 220000 ms @ 30 FPS → 6600 frames.
pub const SATELLITE_HACK_TWO_INTERVAL_FRAMES: u32 = 6_600;

/// One `SpyVisionUpdate` upgrade peel (duration 0 = permanent / `UINT_MAX`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SatelliteHackSpySpec {
    /// C++ `m_selfPoweredDuration` in logic frames. 0 → permanent.
    pub duration_frames: u32,
    /// C++ `m_selfPoweredInterval` in logic frames (off-time between pulses).
    pub interval_frames: u32,
    /// C++ `m_selfPowered`.
    pub self_powered: bool,
    /// Retail Hack I `SpyOnKindof = COMMANDCENTER`.
    pub command_centers_only: bool,
}

/// True when `upgrade` is Satellite Hack I or II.
pub fn is_satellite_hack_upgrade(upgrade: &str) -> bool {
    satellite_hack_spy_spec(upgrade).is_some()
}

/// C++ `SpyVisionUpdate` module data for this `TriggeredBy` name.
pub fn satellite_hack_spy_spec(upgrade: &str) -> Option<SatelliteHackSpySpec> {
    let u = upgrade.to_ascii_lowercase();
    if u.contains("satellitehacktwo")
        || u.contains("satellitehack2")
        || u.eq_ignore_ascii_case(UPGRADE_CHINA_SATELLITE_HACK_TWO)
    {
        return Some(SatelliteHackSpySpec {
            duration_frames: SATELLITE_HACK_TWO_DURATION_FRAMES,
            interval_frames: SATELLITE_HACK_TWO_INTERVAL_FRAMES,
            self_powered: true,
            command_centers_only: false,
        });
    }
    if u.contains("satellitehack") {
        return Some(SatelliteHackSpySpec {
            duration_frames: 0,
            interval_frames: 0,
            self_powered: true,
            command_centers_only: true,
        });
    }
    None
}

/// Retail only Internet Center authors `Behavior = SpyVisionUpdate`.
pub fn object_authors_spy_vision_update(template_name: &str) -> bool {
    is_internet_center_template(template_name)
}

/// C++ `activateSpyVision`: duration 0 → `m_deactivateFrame = UINT_MAX`.
pub fn spy_vision_deactivate_frame(now: u32, duration_frames: u32) -> u32 {
    if duration_frames == 0 {
        u32::MAX
    } else {
        now.saturating_add(duration_frames)
    }
}

pub fn honesty_satellite_hack_spy_residual_ok() -> bool {
    matches!(
        satellite_hack_spy_spec(UPGRADE_CHINA_SATELLITE_HACK_ONE),
        Some(s) if s.duration_frames == 0 && s.command_centers_only && s.self_powered
    ) && matches!(
        satellite_hack_spy_spec(UPGRADE_CHINA_SATELLITE_HACK_TWO),
        Some(s)
            if s.duration_frames == SATELLITE_HACK_TWO_DURATION_FRAMES
                && s.interval_frames == SATELLITE_HACK_TWO_INTERVAL_FRAMES
                && !s.command_centers_only
    ) && object_authors_spy_vision_update("ChinaInternetCenter")
        && object_authors_spy_vision_update("Tank_ChinaInternetCenter")
        && !object_authors_spy_vision_update("ChinaWarFactory")
        && spy_vision_deactivate_frame(10, 0) == u32::MAX
        && spy_vision_deactivate_frame(10, 600) == 610
}
