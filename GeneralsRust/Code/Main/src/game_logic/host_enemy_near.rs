//! Host EnemyNearUpdate residual (MODELCONDITION_ENEMYNEAR for walls/props).
//!
//! C++: `EnemyNearUpdate::update`
//! - Every `ScanDelayTime` frames (default **1s** = **30**f), scan for closest
//!   enemy within vision range (`AI::findClosestEnemy` / CAN_SEE residual).
//! - On rising edge: set model condition `ENEMYNEAR`
//! - On falling edge: clear `ENEMYNEAR`
//!
//! Retail peel (`FactionBuilding.ini` defensive wall): empty module body → defaults.
//!
//! Fail-closed: not full AI mood CAN_SEE shroud matrix / drawable anim scrub /
//! random ctor scan bias beyond optional salt.

use serde::{Deserialize, Serialize};

pub const ENEMY_NEAR_LOGIC_FPS: f32 = 30.0;
/// C++ default `m_enemyScanDelayTime = LOGICFRAMES_PER_SECOND`.
pub const ENEMY_NEAR_DEFAULT_SCAN_DELAY_FRAMES: u32 = 30;
/// Model condition bit name residual.
pub const ENEMY_NEAR_MODEL_CONDITION: &str = "ENEMYNEAR";

pub fn enemy_near_ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * ENEMY_NEAR_LOGIC_FPS / 1000.0).round() as u32
}

/// Per-object EnemyNearUpdate residual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEnemyNearData {
    pub enemy_near: bool,
    pub scan_delay: u32,
    pub scan_delay_time: u32,
    pub model_enemy_near: bool,
    pub vision_range: f32,
}

impl Default for HostEnemyNearData {
    fn default() -> Self {
        Self {
            enemy_near: false,
            scan_delay: 0,
            scan_delay_time: ENEMY_NEAR_DEFAULT_SCAN_DELAY_FRAMES,
            model_enemy_near: false,
            vision_range: 150.0, // host default residual vision for walls
        }
    }
}

impl HostEnemyNearData {
    pub fn new(vision_range: f32) -> Self {
        Self {
            vision_range: vision_range.max(0.0),
            ..Self::default()
        }
    }

    pub fn for_template(template_name: &str, vision_range: f32) -> Option<Self> {
        if is_enemy_near_template(template_name) {
            Some(Self::new(vision_range))
        } else {
            None
        }
    }

    /// One frame residual. `enemy_present` is the scan result when a scan runs.
    /// Returns `(became_near, became_clear)`.
    pub fn tick(&mut self, enemy_present_if_scanning: Option<bool>) -> (bool, bool) {
        let was = self.enemy_near;
        if self.scan_delay == 0 {
            self.scan_delay = self.scan_delay_time.max(1);
            if let Some(present) = enemy_present_if_scanning {
                self.enemy_near = present;
            }
        } else {
            self.scan_delay = self.scan_delay.saturating_sub(1);
        }
        let became_near = self.enemy_near && !was;
        let became_clear = !self.enemy_near && was;
        if became_near {
            self.model_enemy_near = true;
        } else if became_clear {
            self.model_enemy_near = false;
        }
        (became_near, became_clear)
    }

    /// Force an immediate scan result (tests / script).
    pub fn force_scan(&mut self, enemy_present: bool) -> (bool, bool) {
        self.scan_delay = 0;
        self.tick(Some(enemy_present))
    }
}

/// Defensive wall / props that carry EnemyNearUpdate.
pub fn is_enemy_near_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("wall")
        || n.contains("fence")
        || n.contains("bunker") && n.contains("china")
        || n.contains("enemynear")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostEnemyNearRegistry {
    pub installed: u32,
    pub scans: u32,
    pub became_near: u32,
    pub became_clear: u32,
}

impl HostEnemyNearRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_install(&mut self) {
        self.installed = self.installed.saturating_add(1);
    }
    pub fn record_scan(&mut self) {
        self.scans = self.scans.saturating_add(1);
    }
    pub fn record_near(&mut self) {
        self.became_near = self.became_near.saturating_add(1);
    }
    pub fn record_clear(&mut self) {
        self.became_clear = self.became_clear.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.became_near > 0 || self.scans > 0
    }
}

pub fn honesty_enemy_near_residual_ok() -> bool {
    ENEMY_NEAR_DEFAULT_SCAN_DELAY_FRAMES == 30
        && enemy_near_ms_to_frames(1_000) == 30
        && ENEMY_NEAR_MODEL_CONDITION == "ENEMYNEAR"
        && is_enemy_near_template("AmericaWallSegment")
        && is_enemy_near_template("ChinaWall")
        && !is_enemy_near_template("AmericaTankCrusader")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack() {
        assert!(honesty_enemy_near_residual_ok());
    }

    #[test]
    fn rising_and_falling_edges() {
        let mut d = HostEnemyNearData::new(100.0);
        d.scan_delay = 0;
        let (near, clear) = d.tick(Some(true));
        assert!(near && !clear);
        assert!(d.model_enemy_near);
        // Drain delay without changing until next scan.
        for _ in 0..d.scan_delay_time {
            let _ = d.tick(None);
        }
        d.scan_delay = 0;
        let (near, clear) = d.tick(Some(false));
        assert!(!near && clear);
        assert!(!d.model_enemy_near);
    }
}
