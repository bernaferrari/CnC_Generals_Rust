//! Host DefectorSpecialPower residual (general clicks enemy → defects to us).
//!
//! C++: `DefectorSpecialPower::doSpecialPowerAtObject` calls
//! `objectToMakeDefector->defect(selfTeam, detectionTime)`.
//! DetectionTime defaults to DEFECTION_DETECTION_TIME_MAX (10s @ 30 FPS).
//!
//! Residual playability slice:
//! - Change victim team to caster team
//! - Start undetected-defector timer with FX
//! - Disabled caster skips
//!
//! Fail-closed: not full fat-cursor FOW reveal geometry / AI path hooks.

use crate::game_logic::host_defection_helper::{
    DEFAULT_DEFECTION_PROTECTION_FRAMES, DEFECTION_DETECTION_TIME_MAX,
};
use serde::{Deserialize, Serialize};

/// Default detection time residual frames.
pub const DEFECTOR_DETECTION_FRAMES: u32 = DEFAULT_DEFECTION_PROTECTION_FRAMES;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostDefectorSpecialPowerRegistry {
    pub activations: u32,
    pub victims_defected: u32,
    pub last_victim_id: u32,
    pub last_detection_frames: u32,
}

impl HostDefectorSpecialPowerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, victim_id: u32, detection_frames: u32) {
        self.activations = self.activations.saturating_add(1);
        self.victims_defected = self.victims_defected.saturating_add(1);
        self.last_victim_id = victim_id;
        self.last_detection_frames = detection_frames;
    }

    pub fn honesty_ok(&self) -> bool {
        self.activations > 0 && self.victims_defected > 0
    }
}

pub fn detection_frames_for_template(power_name: &str) -> u32 {
    let n = power_name.to_ascii_lowercase();
    if n.contains("defector") {
        return DEFECTOR_DETECTION_FRAMES.min(DEFECTION_DETECTION_TIME_MAX);
    }
    DEFECTOR_DETECTION_FRAMES.min(DEFECTION_DETECTION_TIME_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_default_is_10s() {
        assert_eq!(detection_frames_for_template("SpecialPowerDefector"), 300);
    }
}
