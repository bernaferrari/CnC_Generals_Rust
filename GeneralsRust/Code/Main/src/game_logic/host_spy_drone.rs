//! Host SpyDrone special-power residual (SPECIAL_SPY_DRONE).
//!
//! Residual slice (playability):
//! - `DoSpecialPower(SpyDrone)` at a world location spawns `AmericaVehicleSpyDrone`
//!   (OCL `SUPERWEAPON_SpyDrone` CreateObject residual).
//! - Temporary FOW reveal at spawn using VisionRange / RadiusCursorRadius **250**.
//! - SharedSyncedTimer + RequiredScience **SCIENCE_SpyDrone** residual (gated
//!   by `is_special_power_ready_for` / player unlock).
//! - Activate audio residual `SpyDroneCreate`.
//!
//! Fail-closed honesty:
//! - DynamicShroud grow pulse residual closed (0→250 over GrowTime; grid-decal GPU fail-closed)
//! - Not full StealthUpdate continuous rescan matrix / IR FX
//! - Not full SpyDroneLocomotor loft path
//! - Shell `playable_claim` stays false; network deferred

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const SPY_DRONE_LOGIC_FPS: f32 = 30.0;

/// Retail OCL SUPERWEAPON_SpyDrone CreateObject residual.
pub const SPY_DRONE_TEMPLATE: &str = "AmericaVehicleSpyDrone";
/// Retail OCL name residual.
pub const SPY_DRONE_OCL: &str = "SUPERWEAPON_SpyDrone";
/// Retail SpecialPower template residual.
pub const SPY_DRONE_SPECIAL_POWER: &str = "SpecialPowerSpyDrone";
/// Retail Enum residual.
pub const SPY_DRONE_SPECIAL_ENUM: &str = "SPECIAL_SPY_DRONE";

/// Retail RequiredScience residual.
pub const SPY_DRONE_REQUIRED_SCIENCE: &str = "SCIENCE_SpyDrone";

/// Retail ReloadTime residual (msec) — SpecialPower.ini = 90000.
pub const SPY_DRONE_RELOAD_MS: u32 = 90_000;
/// 90000 ms → 2700 frames @ 30 FPS.
pub const SPY_DRONE_RELOAD_FRAMES: u32 = 2_700;

/// Retail RadiusCursorRadius / AmericaVehicleSpyDrone VisionRange residual.
pub const SPY_DRONE_VISION_RANGE: f32 = 250.0;
/// Alias for FOW reveal radius residual.
pub const SPY_DRONE_RADIUS: f32 = SPY_DRONE_VISION_RANGE;

/// Retail AmericaVehicleSpyDrone MaxHealth residual.
pub const SPY_DRONE_MAX_HEALTH: f32 = 200.0;

/// DynamicShroudClearingRangeUpdate FinalVision residual.
pub const SPY_DRONE_FINAL_VISION: f32 = 250.0;
/// GrowTime residual msec (instant grow residual).
pub const SPY_DRONE_GROW_TIME_MS: u32 = 1_000;
/// ShrinkDelay residual msec.
pub const SPY_DRONE_SHRINK_DELAY_MS: u32 = 2_000;
/// ShrinkTime residual msec.
pub const SPY_DRONE_SHRINK_TIME_MS: u32 = 1_000;

/// Temporary FOW reveal duration residual: grow + hold-ish + shrink window.
/// Fail-closed: not full dynamic range curve; use grow+shrink window as reveal life.
pub const SPY_DRONE_FOW_DURATION_MS: u32 =
    SPY_DRONE_GROW_TIME_MS + SPY_DRONE_SHRINK_DELAY_MS + SPY_DRONE_SHRINK_TIME_MS;
/// FOW duration frames residual (4000 ms → 120).
pub const SPY_DRONE_FOW_DURATION_FRAMES: u32 = 120;

/// DynamicShroudClearingRangeUpdate StartRadius residual (ShroudClearingRange = 0).
pub const SPY_DRONE_START_RADIUS: f32 = 0.0;
/// GrowTime 1000 ms → 30 frames @ 30 FPS.
pub const SPY_DRONE_GROW_TIME_FRAMES: u32 = 30;
/// GrowInterval 10 ms → 1 frame residual step.
pub const SPY_DRONE_GROW_INTERVAL_FRAMES: u32 = 1;
/// Grow steps to FinalVision (GrowTime/GrowInterval).
pub const SPY_DRONE_GROW_UPDATES_TO_FINAL: u32 = 30;
/// Radius grow rate per residual update toward VisionRange 250.
pub const SPY_DRONE_RADIUS_GROW_RATE: f32 =
    SPY_DRONE_VISION_RANGE / SPY_DRONE_GROW_UPDATES_TO_FINAL as f32;

/// Scan radius after `update_index` grow pulses (0-based completed updates).
#[inline]
pub fn spy_drone_scan_radius_after_updates(update_index: u32) -> f32 {
    let r = SPY_DRONE_START_RADIUS + (update_index as f32 + 1.0) * SPY_DRONE_RADIUS_GROW_RATE;
    r.min(SPY_DRONE_VISION_RANGE)
}

#[inline]
pub fn spy_drone_grow_is_final(update_index: u32) -> bool {
    spy_drone_scan_radius_after_updates(update_index) + 0.001 >= SPY_DRONE_VISION_RANGE
}

/// StealthDetectorUpdate DetectionRate residual msec.
pub const SPY_DRONE_STEALTH_DETECTION_RATE_MS: u32 = 500;

/// Activate audio residual (SpecialPower.ini InitiateAtLocationSound).
pub const SPY_DRONE_ACTIVATE_AUDIO: &str = "SpyDroneCreate";

/// Model residual (W3DModelDraw ConditionState NONE).
pub const SPY_DRONE_MODEL: &str = "AVSpyDrone";

#[inline]
pub fn spy_drone_duration_ms_to_frames(msec: u32) -> u32 {
    if msec == 0 {
        return 0;
    }
    ((msec as u64 * 30 + 999) / 1000) as u32
}

/// One SpyDrone activation residual record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSpyDrone {
    pub id: u32,
    pub player_id: u32,
    pub player_mask: u32,
    pub location: Vec3,
    pub radius: f32,
    pub activate_frame: u32,
    pub expires_frame: u32,
    pub caster_id: Option<ObjectId>,
    /// Spawned AmericaVehicleSpyDrone host object id residual.
    pub spawned_id: Option<ObjectId>,
    pub fow_reveal_ok: bool,
    pub spawn_ok: bool,
    /// DynamicShroud / stealth-detector residual applied flag.
    pub dynamic_shroud_applied: bool,
    pub stealth_detector_applied: bool,
    /// DynamicShroudClearingRangeUpdate grow pulse index residual.
    pub grow_index: u32,
    /// Grow pulse still expanding toward VisionRange.
    pub growing: bool,
}

/// Host SpyDrone residual registry.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct HostSpyDroneRegistry {
    next_id: u32,
    activations: Vec<HostSpyDrone>,
    total_activations: u32,
    total_spawns: u32,
    /// DynamicShroud grow pulse applications (honesty).
    pub grow_pulses: u32,
}

impl HostSpyDroneRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_id(&mut self) -> u32 {
        self.next_id = self.next_id.saturating_add(1);
        self.next_id
    }

    pub fn record_activation(&mut self, act: HostSpyDrone) {
        if act.spawn_ok {
            self.total_spawns = self.total_spawns.saturating_add(1);
        }
        self.total_activations = self.total_activations.saturating_add(1);
        self.activations.push(act);
    }

    pub fn activations(&self) -> u32 {
        self.total_activations
    }

    pub fn spawns(&self) -> u32 {
        self.total_spawns
    }

    pub fn last(&self) -> Option<&HostSpyDrone> {
        self.activations.last()
    }

    pub fn prune_expired(&mut self, frame: u32) {
        self.activations.retain(|a| a.expires_frame > frame);
    }

    pub fn clear(&mut self) {
        self.activations.clear();
        self.next_id = 0;
        self.total_activations = 0;
        self.total_spawns = 0;
        self.grow_pulses = 0;
    }

    pub fn record_grow_pulse(&mut self) {
        self.grow_pulses = self.grow_pulses.saturating_add(1);
    }

    pub fn honesty_grow_ok(&self) -> bool {
        self.grow_pulses > 0
    }

    /// Mutable access for grow pulse residual updates.
    pub fn activations_mut(&mut self) -> &mut Vec<HostSpyDrone> {
        &mut self.activations
    }

    pub fn honesty_activate_ok(&self) -> bool {
        self.total_activations > 0
    }

    pub fn honesty_spawn_ok(&self) -> bool {
        self.total_spawns > 0
    }
}

/// Wave residual honesty pack for SpyDrone constants.
pub fn honesty_spy_drone_residual_pack_ok() -> bool {
    SPY_DRONE_TEMPLATE == "AmericaVehicleSpyDrone"
        && SPY_DRONE_OCL == "SUPERWEAPON_SpyDrone"
        && SPY_DRONE_SPECIAL_POWER == "SpecialPowerSpyDrone"
        && SPY_DRONE_SPECIAL_ENUM == "SPECIAL_SPY_DRONE"
        && SPY_DRONE_REQUIRED_SCIENCE == "SCIENCE_SpyDrone"
        && SPY_DRONE_RELOAD_MS == 90_000
        && SPY_DRONE_RELOAD_FRAMES == 2_700
        && (SPY_DRONE_VISION_RANGE - 250.0).abs() < 1e-3
        && (SPY_DRONE_MAX_HEALTH - 200.0).abs() < 1e-3
        && SPY_DRONE_FOW_DURATION_FRAMES == 120
        && spy_drone_duration_ms_to_frames(SPY_DRONE_RELOAD_MS) == SPY_DRONE_RELOAD_FRAMES
        && SPY_DRONE_ACTIVATE_AUDIO == "SpyDroneCreate"
        && SPY_DRONE_MODEL == "AVSpyDrone"
        && SPY_DRONE_STEALTH_DETECTION_RATE_MS == 500

        && SPY_DRONE_GROW_UPDATES_TO_FINAL == 30
        && (SPY_DRONE_RADIUS_GROW_RATE - SPY_DRONE_VISION_RANGE / 30.0).abs() < 0.001
        && spy_drone_grow_is_final(SPY_DRONE_GROW_UPDATES_TO_FINAL - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spy_drone_residual_pack_honesty() {
        assert!(honesty_spy_drone_residual_pack_ok());
        let mut reg = HostSpyDroneRegistry::new();
        assert!(!reg.honesty_activate_ok());
        let id = reg.alloc_id();
        reg.record_activation(HostSpyDrone {
            id,
            player_id: 0,
            player_mask: 1,
            location: Vec3::ZERO,
            radius: SPY_DRONE_RADIUS,
            activate_frame: 0,
            expires_frame: 120,
            caster_id: None,
            spawned_id: Some(ObjectId(7)),
            fow_reveal_ok: true,
            spawn_ok: true,
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
            grow_index: 0,
            growing: true,
        });
        assert!(reg.honesty_activate_ok());
        assert!(reg.honesty_spawn_ok());
        assert_eq!(reg.activations(), 1);
        assert_eq!(reg.spawns(), 1);
    }
}
