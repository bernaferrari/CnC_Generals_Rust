//! Host BaikonurLaunchPower residual (GLA endgame rocket launch / detonate).
//!
//! C++: `BaikonurLaunchPower::doSpecialPower` sets DOOR_1_OPENING on the tower.
//! `doSpecialPowerAtLocation` spawns `DetonationObject` (BaikonurRocketDetonation)
//! at the target — that object carries NeutronMissileSlowDeath multi-blast.
//!
//! Residual playability slice:
//! - Launch (no location): set DOOR_1_OPENING on source tower
//! - Detonate at location: spawn detonation template + arm Neutron multi-blast
//! - SpecialPowerCompletionDie residual via honesty counter
//! - Audio / FX list residual names
//!
//! Fail-closed: not full script-only GLA endgame cinematic / pad ambient loop.

use serde::{Deserialize, Serialize};

/// Retail DetonationObject residual.
pub const BAIKONUR_DETONATION_OBJECT: &str = "BaikonurRocketDetonation";
/// Retail SpecialPowerTemplate residual.
pub const BAIKONUR_SPECIAL_POWER: &str = "SuperweaponLaunchBaikonurRocket";
/// Retail NeutronMissileSlowDeath FXList residual on detonation.
pub const BAIKONUR_NUKE_FX: &str = "FX_BaikonurNuke";
/// Retail ScorchMarkSize residual.
pub const BAIKONUR_SCORCH_SIZE: f32 = 320.0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBaikonurLaunchRegistry {
    pub launch_count: u32,
    pub detonation_count: u32,
    pub door_opening_count: u32,
    pub last_detonation_pos: Option<(f32, f32)>,
}

impl HostBaikonurLaunchRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_launch_door(&mut self) {
        self.launch_count = self.launch_count.saturating_add(1);
        self.door_opening_count = self.door_opening_count.saturating_add(1);
    }

    pub fn record_detonation(&mut self, x: f32, z: f32) {
        self.detonation_count = self.detonation_count.saturating_add(1);
        self.last_detonation_pos = Some((x, z));
    }

    pub fn honesty_launch_ok(&self) -> bool {
        self.launch_count > 0 || self.door_opening_count > 0
    }

    pub fn honesty_detonation_ok(&self) -> bool {
        self.detonation_count > 0
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_launch_ok() || self.honesty_detonation_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_tracks_launch_and_detonation() {
        let mut r = HostBaikonurLaunchRegistry::new();
        assert!(!r.honesty_host_path_ok());
        r.record_launch_door();
        assert!(r.honesty_launch_ok());
        r.record_detonation(10.0, 20.0);
        assert!(r.honesty_detonation_ok());
        assert_eq!(r.last_detonation_pos, Some((10.0, 20.0)));
    }
}
