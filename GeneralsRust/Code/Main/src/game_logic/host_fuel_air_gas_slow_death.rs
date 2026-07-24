//! Host FuelAir gas SlowDeathBehavior residual (Daisy/MOAB detonation gas).
//!
//! C++ retail `SupW_AuroraFuelAirGas` / `AirF_AuroraBombGas`:
//! - HeightDieUpdate TargetHeight **15**
//! - SlowDeathBehavior DestructionDelay **1000**ms → **30**f (+variance 100ms → 3f)
//! - FX INITIAL AirF_FX_AuroraBombIgnite, FINAL FX_DaisyCutterFinalExplosion
//! - Weapon MIDPOINT DaisyCutterFlameWeapon (5 / r100 FLAME)
//! - Weapon FINAL SupW_FuelBombDetonationWeapon (900 / r70) or DaisyCutterDetonationWeapon (2000 / r100)
//!
//! Residual playability slice:
//! - Install on gas templates spawned from CreateObjectDie
//! - Tick delay → midpoint flame pulse → final detonation area damage
//! - Destroy gas object after final
//!
//! Fail-closed: not full SlowDeath probability matrix / particle destroy-at-height /
//! tree fire propagation beyond flame weapon residual.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Retail HeightDie TargetHeight residual.
pub const FUEL_AIR_GAS_HEIGHT_DIE: f32 = 15.0;
/// DestructionDelay 1000ms @ 30 FPS.
pub const FUEL_AIR_GAS_DESTRUCTION_DELAY_FRAMES: u32 = 30;
/// DestructionDelayVariance 100ms → 3 frames residual (use half for midpoint).
pub const FUEL_AIR_GAS_DESTRUCTION_VARIANCE_FRAMES: u32 = 3;
/// Midpoint flame fires halfway through destruction delay.
pub const FUEL_AIR_GAS_MIDPOINT_FRACTION: f32 = 0.5;

pub const FUEL_AIR_FLAME_WEAPON: &str = "DaisyCutterFlameWeapon";
pub const FUEL_AIR_FLAME_DAMAGE: f32 = 5.0;
pub const FUEL_AIR_FLAME_RADIUS: f32 = 100.0;

pub const SUPW_FUEL_BOMB_DETONATION_WEAPON: &str = "SupW_FuelBombDetonationWeapon";
pub const SUPW_FUEL_BOMB_DAMAGE: f32 = 900.0;
pub const SUPW_FUEL_BOMB_RADIUS: f32 = 70.0;

pub const DAISY_DETONATION_WEAPON: &str = "DaisyCutterDetonationWeapon";
pub const DAISY_DETONATION_DAMAGE: f32 = 2000.0;
pub const DAISY_DETONATION_RADIUS: f32 = 100.0;

pub const FX_INITIAL_IGNITE: &str = "AirF_FX_AuroraBombIgnite";
pub const FX_FINAL_EXPLOSION: &str = "FX_DaisyCutterFinalExplosion";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FuelAirGasPhase {
    /// Waiting for HeightDie or already grounded; SlowDeath clock running.
    Arming,
    /// Midpoint flame weapon fired.
    MidpointDone,
    /// Final detonation fired; object should die.
    FinalDone,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFuelAirGasSlowDeathData {
    pub phase: FuelAirGasPhase,
    pub arm_frame: u32,
    pub midpoint_frame: u32,
    pub final_frame: u32,
    pub uses_supw_detonation: bool,
    pub initial_fx_queued: bool,
}

impl HostFuelAirGasSlowDeathData {
    pub fn start(now: u32, uses_supw_detonation: bool) -> Self {
        let delay = FUEL_AIR_GAS_DESTRUCTION_DELAY_FRAMES;
        let mid = (delay as f32 * FUEL_AIR_GAS_MIDPOINT_FRACTION) as u32;
        Self {
            phase: FuelAirGasPhase::Arming,
            arm_frame: now,
            midpoint_frame: now.saturating_add(mid.max(1)),
            final_frame: now.saturating_add(delay),
            uses_supw_detonation,
            initial_fx_queued: false,
        }
    }

    pub fn for_template(template_name: &str, now: u32) -> Option<Self> {
        if !is_fuel_air_gas_template(template_name) {
            return None;
        }
        let n = template_name.to_ascii_lowercase();
        // SupW gas uses SupW_FuelBombDetonationWeapon; AirF uses DaisyCutterDetonationWeapon.
        let uses_supw = n.contains("supw") || (n.contains("fuelair") && !n.contains("airf"));
        Some(Self::start(now, uses_supw))
    }

    /// Advance one frame. Returns events to apply.
    pub fn tick(&mut self, now: u32) -> FuelAirGasTickEvent {
        if !self.initial_fx_queued {
            self.initial_fx_queued = true;
            return FuelAirGasTickEvent::InitialFx;
        }
        match self.phase {
            FuelAirGasPhase::Arming if now >= self.midpoint_frame => {
                self.phase = FuelAirGasPhase::MidpointDone;
                FuelAirGasTickEvent::MidpointFlame {
                    damage: FUEL_AIR_FLAME_DAMAGE,
                    radius: FUEL_AIR_FLAME_RADIUS,
                    weapon: FUEL_AIR_FLAME_WEAPON,
                }
            }
            FuelAirGasPhase::MidpointDone | FuelAirGasPhase::Arming
                if now >= self.final_frame =>
            {
                self.phase = FuelAirGasPhase::FinalDone;
                if self.uses_supw_detonation {
                    FuelAirGasTickEvent::FinalDetonation {
                        damage: SUPW_FUEL_BOMB_DAMAGE,
                        radius: SUPW_FUEL_BOMB_RADIUS,
                        weapon: SUPW_FUEL_BOMB_DETONATION_WEAPON,
                        fx: FX_FINAL_EXPLOSION,
                    }
                } else {
                    FuelAirGasTickEvent::FinalDetonation {
                        damage: DAISY_DETONATION_DAMAGE,
                        radius: DAISY_DETONATION_RADIUS,
                        weapon: DAISY_DETONATION_WEAPON,
                        fx: FX_FINAL_EXPLOSION,
                    }
                }
            }
            FuelAirGasPhase::FinalDone => FuelAirGasTickEvent::None,
            _ => FuelAirGasTickEvent::None,
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.phase, FuelAirGasPhase::FinalDone)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FuelAirGasTickEvent {
    None,
    InitialFx,
    MidpointFlame {
        damage: f32,
        radius: f32,
        weapon: &'static str,
    },
    FinalDetonation {
        damage: f32,
        radius: f32,
        weapon: &'static str,
        fx: &'static str,
    },
}

pub fn is_fuel_air_gas_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    (n.contains("fuelair") && n.contains("gas"))
        || n.contains("aurorabombgas")
        || n.contains("aurora_bomb_gas")
        || (n.contains("daisy") && n.contains("gas"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostFuelAirGasRegistry {
    pub installed: u32,
    pub midpoint_flames: u32,
    pub final_detonations: u32,
    pub gas_destroyed: u32,
}

impl HostFuelAirGasRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_install(&mut self) {
        self.installed = self.installed.saturating_add(1);
    }
    pub fn record_midpoint(&mut self) {
        self.midpoint_flames = self.midpoint_flames.saturating_add(1);
    }
    pub fn record_final(&mut self) {
        self.final_detonations = self.final_detonations.saturating_add(1);
    }
    pub fn record_destroy(&mut self) {
        self.gas_destroyed = self.gas_destroyed.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.final_detonations > 0
    }
}

pub fn honesty_fuel_air_gas_slow_death_residual_ok() -> bool {
    FUEL_AIR_GAS_DESTRUCTION_DELAY_FRAMES == 30
        && (FUEL_AIR_GAS_HEIGHT_DIE - 15.0).abs() < 1e-5
        && (SUPW_FUEL_BOMB_DAMAGE - 900.0).abs() < 0.1
        && (DAISY_DETONATION_DAMAGE - 2000.0).abs() < 0.1
        && is_fuel_air_gas_template("SupW_AuroraFuelAirGas")
        && is_fuel_air_gas_template("AirF_AuroraBombGas")
        && !is_fuel_air_gas_template("DaisyCutterBomb")
        && {
            let mut d = HostFuelAirGasSlowDeathData::start(0, true);
            let init_ok = matches!(d.tick(0), FuelAirGasTickEvent::InitialFx);
            let mut saw_mid = false;
            let mut saw_final = false;
            let mut mid_ok = true;
            let mut fin_ok = true;
            for f in 1..=30 {
                match d.tick(f) {
                    FuelAirGasTickEvent::MidpointFlame { damage, .. } => {
                        saw_mid = true;
                        mid_ok = (damage - 5.0).abs() < 0.1;
                    }
                    FuelAirGasTickEvent::FinalDetonation { damage, radius, .. } => {
                        saw_final = true;
                        fin_ok = (damage - 900.0).abs() < 0.1 && (radius - 70.0).abs() < 0.1;
                    }
                    _ => {}
                }
            }
            init_ok && saw_mid && saw_final && mid_ok && fin_ok && d.is_complete()
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_timeline() {
        // honesty uses assert internally — also direct checks
        assert!(honesty_fuel_air_gas_slow_death_residual_ok());
        let d = HostFuelAirGasSlowDeathData::for_template("AirF_AuroraBombGas", 10).unwrap();
        // AirF uses daisy detonation (not supw) when name contains airf
        assert!(!d.uses_supw_detonation);
    }
}
