//! Host ProneUpdate residual (infantry cower after damage).
//!
//! C++: `ProneUpdate`
//! - `goProne(damage)`: `proneFrames += actualDamage * DamageToFramesRatio`
//! - While prone: MODELCONDITION_PRONE + OBJECT_STATUS_NO_ATTACK
//! - Countdown each frame; clear effects at 0
//!
//! Retail peels (often commented in ZH, still module-complete):
//! - `DamageToFramesRatio = 5.0` → 20 damage ⇒ 100 frames prone
//!
//! Fail-closed: not full SlaveUpdate shared-damage prone / drawable anim blend.

use serde::{Deserialize, Serialize};

/// Default C++ module data ratio.
pub const PRONE_DEFAULT_DAMAGE_TO_FRAMES_RATIO: f32 = 1.0;
/// Commented GLA worker/slave peel residual.
pub const PRONE_GLA_DAMAGE_TO_FRAMES_RATIO: f32 = 5.0;
/// Model condition name residual.
pub const PRONE_MODEL_CONDITION: &str = "PRONE";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostProneUpdateData {
    pub prone_frames: i32,
    pub damage_to_frames_ratio: f32,
    pub model_prone: bool,
    pub no_attack: bool,
}

impl Default for HostProneUpdateData {
    fn default() -> Self {
        Self {
            prone_frames: 0,
            damage_to_frames_ratio: PRONE_DEFAULT_DAMAGE_TO_FRAMES_RATIO,
            model_prone: false,
            no_attack: false,
        }
    }
}

impl HostProneUpdateData {
    pub fn with_ratio(ratio: f32) -> Self {
        Self {
            damage_to_frames_ratio: ratio.max(0.0),
            ..Self::default()
        }
    }

    pub fn for_template(template_name: &str) -> Option<Self> {
        if is_prone_update_template(template_name) {
            let ratio = if is_gla_worker_prone_template(template_name) {
                PRONE_GLA_DAMAGE_TO_FRAMES_RATIO
            } else {
                PRONE_DEFAULT_DAMAGE_TO_FRAMES_RATIO
            };
            Some(Self::with_ratio(ratio))
        } else {
            None
        }
    }

    pub fn is_prone(&self) -> bool {
        self.prone_frames > 0
    }

    /// C++ goProne — returns true if effects just started.
    pub fn go_prone_damage(&mut self, actual_damage_dealt: f32) -> bool {
        let was = self.prone_frames > 0;
        let add = (actual_damage_dealt.max(0.0) * self.damage_to_frames_ratio).round() as i32;
        self.prone_frames = self.prone_frames.saturating_add(add.max(0));
        if !was && self.prone_frames > 0 {
            self.start_effects();
            true
        } else {
            false
        }
    }

    fn start_effects(&mut self) {
        self.model_prone = true;
        self.no_attack = true;
    }

    fn stop_effects(&mut self) {
        self.model_prone = false;
        self.no_attack = false;
    }

    /// One frame countdown. Returns true if just stopped being prone.
    pub fn tick(&mut self) -> bool {
        if self.prone_frames <= 0 {
            return false;
        }
        self.prone_frames -= 1;
        if self.prone_frames == 0 {
            self.stop_effects();
            true
        } else {
            false
        }
    }
}

pub fn is_prone_update_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Infantry residual hosts (module often present even when INI-commented).
    n.contains("infantry")
        || n.contains("ranger")
        || n.contains("rebel")
        || n.contains("redguard")
        || n.contains("tankhunter")
        || n.contains("pathfinder")
        || n.contains("jarmenkell")
        || n.contains("hijacker")
        || n.contains("worker")
        || n.contains("dozer") && n.contains("infantry")
}

fn is_gla_worker_prone_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("worker") || n.contains("slave")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostProneUpdateRegistry {
    pub installed: u32,
    pub go_prone: u32,
    pub recoveries: u32,
    pub total_prone_frames_added: u32,
}

impl HostProneUpdateRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_install(&mut self) {
        self.installed = self.installed.saturating_add(1);
    }
    pub fn record_go_prone(&mut self, frames_added: u32) {
        self.go_prone = self.go_prone.saturating_add(1);
        self.total_prone_frames_added = self.total_prone_frames_added.saturating_add(frames_added);
    }
    pub fn record_recovery(&mut self) {
        self.recoveries = self.recoveries.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.go_prone > 0
    }
}

pub fn honesty_prone_update_residual_ok() -> bool {
    PRONE_GLA_DAMAGE_TO_FRAMES_RATIO == 5.0
        && PRONE_MODEL_CONDITION == "PRONE"
        && {
            let mut d = HostProneUpdateData::with_ratio(5.0);
            let started = d.go_prone_damage(20.0);
            started && d.prone_frames == 100
        }
        && is_prone_update_template("AmericaInfantryRanger")
        && is_prone_update_template("GLAInfantryWorker")
        && !is_prone_update_template("AmericaTankCrusader")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_and_countdown() {
        assert!(honesty_prone_update_residual_ok());
        let mut d = HostProneUpdateData::with_ratio(5.0);
        assert!(d.go_prone_damage(20.0));
        assert!(d.is_prone() && d.no_attack && d.model_prone);
        for _ in 0..99 {
            assert!(!d.tick());
        }
        assert!(d.tick());
        assert!(!d.is_prone() && !d.no_attack);
    }
}
