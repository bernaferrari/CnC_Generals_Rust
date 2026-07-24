//! Host CheckpointUpdate residual (gate opens for allies when no enemy near).
//!
//! C++: `CheckpointUpdate::update`
//! - Scan vision for closest enemy/ally every `ScanDelayTime` (default **30**f)
//! - Open when `allyNear && !enemyNear`
//! - Model: DOOR_1_OPENING / DOOR_1_CLOSING on state change
//! - Geometry minor radius shrinks while open (`-0.333`/f), grows while closed
//!
//! Retail peel: `AmericaCheckpoint` empty module body → defaults.
//!
//! Fail-closed: not full geometry pathfinder cell rebuild / anim scrub scalar /
//! random scan bias beyond optional salt.

use serde::{Deserialize, Serialize};

pub const CHECKPOINT_LOGIC_FPS: f32 = 30.0;
/// C++ default ScanDelayTime = LOGICFRAMES_PER_SECOND.
pub const CHECKPOINT_DEFAULT_SCAN_DELAY_FRAMES: u32 = 30;
/// C++ minor radius step residual per frame.
pub const CHECKPOINT_RADIUS_STEP: f32 = 0.333;
/// Default max minor radius residual when geometry peel missing.
pub const CHECKPOINT_DEFAULT_MAX_MINOR_RADIUS: f32 = 10.0;

pub fn checkpoint_ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * CHECKPOINT_LOGIC_FPS / 1000.0).round() as u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CheckpointDoorAnim {
    #[default]
    None,
    Opening,
    Closing,
}

impl CheckpointDoorAnim {
    pub fn model_condition(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Opening => Some("DOOR_1_OPENING"),
            Self::Closing => Some("DOOR_1_CLOSING"),
        }
    }
}

/// Per-object CheckpointUpdate residual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCheckpointUpdateData {
    pub enemy_near: bool,
    pub ally_near: bool,
    pub scan_delay: u32,
    pub scan_delay_time: u32,
    pub max_minor_radius: f32,
    /// Live path-blocking radius residual (geometry minor).
    pub path_radius: f32,
    pub door_anim: CheckpointDoorAnim,
    pub open: bool,
    pub vision_range: f32,
}

impl Default for HostCheckpointUpdateData {
    fn default() -> Self {
        Self {
            enemy_near: false,
            ally_near: false,
            scan_delay: 0,
            scan_delay_time: CHECKPOINT_DEFAULT_SCAN_DELAY_FRAMES,
            max_minor_radius: CHECKPOINT_DEFAULT_MAX_MINOR_RADIUS,
            path_radius: CHECKPOINT_DEFAULT_MAX_MINOR_RADIUS,
            door_anim: CheckpointDoorAnim::None,
            open: false,
            vision_range: 100.0, // retail AmericaCheckpoint VisionRange
        }
    }
}

impl HostCheckpointUpdateData {
    pub fn new(max_minor_radius: f32, vision_range: f32) -> Self {
        let r = max_minor_radius.max(0.1);
        Self {
            max_minor_radius: r,
            path_radius: r,
            vision_range: vision_range.max(0.0),
            ..Self::default()
        }
    }

    pub fn for_template(template_name: &str, vision_range: f32) -> Option<Self> {
        if is_checkpoint_update_template(template_name) {
            Some(Self::new(CHECKPOINT_DEFAULT_MAX_MINOR_RADIUS, vision_range))
        } else {
            None
        }
    }

    /// Desired open state: ally near and no enemy near.
    pub fn desired_open(ally_near: bool, enemy_near: bool) -> bool {
        ally_near && !enemy_near
    }

    /// Apply a scan result (when scan_delay hits 0).
    pub fn apply_scan(&mut self, enemy_near: bool, ally_near: bool) -> bool {
        let change = self.enemy_near != enemy_near || self.ally_near != ally_near;
        self.enemy_near = enemy_near;
        self.ally_near = ally_near;
        let open = Self::desired_open(ally_near, enemy_near);
        if change {
            self.open = open;
            self.door_anim = if open {
                CheckpointDoorAnim::Opening
            } else {
                CheckpointDoorAnim::Closing
            };
        }
        change
    }

    /// One frame residual.
    ///
    /// `scan_result`: Some((enemy, ally)) when a scan should run this frame.
    /// Returns true if door anim state changed this frame.
    pub fn tick(&mut self, scan_result: Option<(bool, bool)>) -> bool {
        let mut changed = false;
        if let Some((enemy, ally)) = scan_result {
            self.scan_delay = self.scan_delay_time.max(1);
            changed = self.apply_scan(enemy, ally);
        } else if self.scan_delay > 0 {
            self.scan_delay = self.scan_delay.saturating_sub(1);
        } else {
            // Due for scan but caller provided none — keep prior.
            self.scan_delay = self.scan_delay_time.max(1);
        }

        // Geometry radius crawl residual.
        if self.open {
            if self.path_radius > 0.0 {
                self.path_radius = (self.path_radius - CHECKPOINT_RADIUS_STEP).max(0.0);
            }
        } else if self.path_radius < self.max_minor_radius {
            self.path_radius =
                (self.path_radius + CHECKPOINT_RADIUS_STEP).min(self.max_minor_radius);
        }
        changed
    }

    pub fn needs_scan(&self) -> bool {
        self.scan_delay == 0
    }
}

pub fn is_checkpoint_update_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("checkpoint") || n.contains("gatehouse")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCheckpointUpdateRegistry {
    pub installed: u32,
    pub scans: u32,
    pub opens: u32,
    pub closes: u32,
}

impl HostCheckpointUpdateRegistry {
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
    pub fn record_open(&mut self) {
        self.opens = self.opens.saturating_add(1);
    }
    pub fn record_close(&mut self) {
        self.closes = self.closes.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.opens > 0 || self.scans > 0
    }
}

pub fn honesty_checkpoint_update_residual_ok() -> bool {
    CHECKPOINT_DEFAULT_SCAN_DELAY_FRAMES == 30
        && checkpoint_ms_to_frames(1_000) == 30
        && (CHECKPOINT_RADIUS_STEP - 0.333).abs() < 0.001
        && is_checkpoint_update_template("AmericaCheckpoint")
        && !is_checkpoint_update_template("AmericaTankCrusader")
        && HostCheckpointUpdateData::desired_open(true, false)
        && !HostCheckpointUpdateData::desired_open(true, true)
        && !HostCheckpointUpdateData::desired_open(false, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack() {
        assert!(honesty_checkpoint_update_residual_ok());
    }

    #[test]
    fn ally_opens_enemy_closes() {
        let mut d = HostCheckpointUpdateData::new(9.0, 100.0);
        d.scan_delay = 0;
        let ch = d.tick(Some((false, true)));
        assert!(ch && d.open);
        assert_eq!(d.door_anim, CheckpointDoorAnim::Opening);
        // Radius shrinks while open.
        let r0 = d.path_radius;
        let _ = d.tick(None);
        assert!(d.path_radius < r0);

        d.scan_delay = 0;
        let ch = d.tick(Some((true, true)));
        assert!(ch && !d.open);
        assert_eq!(d.door_anim, CheckpointDoorAnim::Closing);
    }
}
