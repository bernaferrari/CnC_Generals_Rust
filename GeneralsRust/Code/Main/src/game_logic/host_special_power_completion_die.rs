//! Host SpecialPowerCompletionDie + PowerPlantUpdate rod-extend residuals.
//!
//! C++:
//! - `SpecialPowerCompletionDie::onDie` →
//!   `TheScriptEngine->notifyOfCompletedSpecialPower(player, SP name, creatorID)`
//! - `PowerPlantUpdate::extendRods(true)` sets POWER_PLANT_UPGRADING then after
//!   RodsExtendTime frames clears to POWER_PLANT_UPGRADED.
//!
//! Residual playability slice:
//! - Objects carrying `special_power_completion` die → script event + honesty
//! - Advanced Control Rods starts UPGRADING, completes after RodsExtendTime
//!
//! Fail-closed: not full ScriptEngine condition evaluation matrix /
//! multi-creator edge cases / drawable-only radar extend path.

use serde::{Deserialize, Serialize};

/// America PowerPlantUpdate RodsExtendTime residual (600ms → 18f @ 30 FPS).
pub const AMERICA_RODS_EXTEND_FRAMES: u32 = 18;
/// China RodsExtendTime residual (1ms → 1 frame ceil).
pub const CHINA_RODS_EXTEND_FRAMES: u32 = 1;

pub fn rods_extend_frames_for_template(template_name: &str) -> u32 {
    let n = template_name.to_ascii_lowercase();
    if n.contains("china") {
        return CHINA_RODS_EXTEND_FRAMES;
    }
    AMERICA_RODS_EXTEND_FRAMES
}

/// C++ SpecialPowerCompletionDie residual payload on an object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSpecialPowerCompletionDieData {
    pub special_power_name: String,
    pub creator_id: u32,
    pub creator_set: bool,
}

impl HostSpecialPowerCompletionDieData {
    pub fn new(special_power_name: impl Into<String>, creator_id: u32) -> Self {
        Self {
            special_power_name: special_power_name.into(),
            creator_id,
            creator_set: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostSpecialPowerCompletionLog {
    pub notifications: u32,
    pub last_power: String,
    pub last_creator: u32,
    pub rods_extend_starts: u32,
    pub rods_extend_completes: u32,
}

impl HostSpecialPowerCompletionLog {
    pub fn record_notify(&mut self, power: &str, creator: u32) {
        self.notifications = self.notifications.saturating_add(1);
        self.last_power = power.to_string();
        self.last_creator = creator;
    }
    pub fn record_rods_start(&mut self) {
        self.rods_extend_starts = self.rods_extend_starts.saturating_add(1);
    }
    pub fn record_rods_complete(&mut self) {
        self.rods_extend_completes = self.rods_extend_completes.saturating_add(1);
    }
    pub fn honesty_ok(&self) -> bool {
        self.notifications > 0 || self.rods_extend_completes > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn america_rods_18_frames() {
        assert_eq!(rods_extend_frames_for_template("AmericaPowerPlant"), 18);
        assert_eq!(rods_extend_frames_for_template("ChinaPowerPlant"), 1);
    }
}
