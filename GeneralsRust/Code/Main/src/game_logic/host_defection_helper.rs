//! Host ObjectDefectionHelper residual (undetected defector timer + FX).
//!
//! C++: after `defect()` / pilot invulnerable, object is UNDETECTED_DEFECTOR until
//! timer ends. Helper flashes selection + tick audio as time runs out; attacking
//! or dying blows cover early. Final ding + white flash when timer expires.
//!
//! Residual playability slice:
//! - `start_defection_timer(frames, with_fx)` residual
//! - Default max detection window **10s** (300 frames @ 30 FPS)
//! - Blow cover on effectively-dead or IS_FIRING_WEAPON
//! - Flash phase residual accelerates near end
//! - Presentation: defector_flash + undetected_defector flags
//!
//! Fail-closed: not full relationship matrix / groupDoSpecialPower cover blow
//! beyond firing bit / drawable flash white RGB path beyond residual events.

use serde::{Deserialize, Serialize};

/// C++ DEFECTION_DETECTION_TIME_MAX = LOGICFRAMES_PER_SECOND * 10.
pub const DEFECTION_DETECTION_TIME_MAX: u32 = 30 * 10;
/// C++ DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT residual (often same 10s).
pub const DEFAULT_DEFECTION_PROTECTION_FRAMES: u32 = DEFECTION_DETECTION_TIME_MAX;
/// Pilot OCL InvulnerableTime 2000ms → 60f residual (withDefectorFX=false path).
pub const PILOT_INVULNERABLE_DEFECTION_FRAMES: u32 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostDefectionHelperData {
    pub detection_start: u32,
    pub detection_end: u32,
    pub flash_phase: f32,
    pub do_defector_fx: bool,
    /// True while friend_setUndetectedDefector(TRUE).
    pub undetected_defector: bool,
    /// Presentation residual: selection flash this frame.
    pub flash_this_frame: bool,
    /// Presentation residual: final white flash when timer expires.
    pub final_white_flash: bool,
    /// Audio residual keys for presentation drain.
    pub pending_audio: Vec<String>,
}

impl Default for HostDefectionHelperData {
    fn default() -> Self {
        Self {
            detection_start: 0,
            detection_end: 0,
            flash_phase: 0.0,
            do_defector_fx: false,
            undetected_defector: false,
            flash_this_frame: false,
            final_white_flash: false,
            pending_audio: Vec::new(),
        }
    }
}

impl HostDefectionHelperData {
    /// C++ startDefectionTimer residual.
    pub fn start_defection_timer(&mut self, now: u32, num_frames: u32, with_fx: bool) {
        if !self.undetected_defector {
            return;
        }
        let frames = num_frames.max(1).min(DEFECTION_DETECTION_TIME_MAX);
        self.detection_start = now;
        self.detection_end = now.saturating_add(frames);
        self.flash_phase = 0.0;
        self.do_defector_fx = with_fx;
        self.flash_this_frame = false;
        self.final_white_flash = false;
        self.pending_audio.clear();
    }

    /// Mark as undetected defector (call before start_defection_timer).
    pub fn set_undetected_defector(&mut self, on: bool) {
        self.undetected_defector = on;
        if !on {
            self.do_defector_fx = false;
            self.flash_this_frame = false;
            self.final_white_flash = false;
            self.detection_end = 0;
            self.detection_start = 0;
            self.flash_phase = 0.0;
        }
    }

    pub fn is_undetected_defector(&self) -> bool {
        self.undetected_defector
    }

    /// Blow cover immediately (attack / special power / death).
    pub fn blow_cover(&mut self) {
        self.set_undetected_defector(false);
    }

    /// C++ ObjectDefectionHelper::update residual.
    ///
    /// `is_firing` = OBJECT_STATUS_IS_FIRING_WEAPON; `effectively_dead` blows cover.
    pub fn tick(&mut self, now: u32, is_firing: bool, effectively_dead: bool) {
        self.flash_this_frame = false;
        self.final_white_flash = false;
        if !self.undetected_defector {
            return;
        }
        if effectively_dead || is_firing {
            self.blow_cover();
            return;
        }
        if self.detection_end == 0 {
            return;
        }
        if now >= self.detection_end {
            self.undetected_defector = false;
            if self.do_defector_fx {
                self.final_white_flash = true;
                self.pending_audio.push("DefectorTimerDing".into());
            }
            self.do_defector_fx = false;
            self.detection_end = 0;
            return;
        }
        if self.do_defector_fx {
            let last_phase = (self.flash_phase as i32) & 1;
            let time_left = self.detection_end.saturating_sub(now) as f32;
            let max_t = DEFECTION_DETECTION_TIME_MAX as f32;
            self.flash_phase += 0.5 * (1.0 - (time_left / max_t));
            let this_phase = (self.flash_phase as i32) & 1;
            if last_phase != 0 && this_phase == 0 {
                self.flash_this_frame = true;
                self.pending_audio.push("DefectorTimerTick".into());
            }
        }
    }

    pub fn drain_audio(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_audio)
    }
}

/// Apply defect residual: change team + start undetected timer.
pub fn defect_team_residual(
    data: &mut HostDefectionHelperData,
    now: u32,
    protection_frames: u32,
    with_fx: bool,
) {
    data.set_undetected_defector(true);
    data.start_defection_timer(now, protection_frames, with_fx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_expires_and_dings() {
        let mut d = HostDefectionHelperData::default();
        d.set_undetected_defector(true);
        d.start_defection_timer(0, 30, true);
        assert!(d.is_undetected_defector());
        for f in 0..29 {
            d.tick(f, false, false);
            assert!(d.is_undetected_defector());
        }
        d.tick(30, false, false);
        assert!(!d.is_undetected_defector());
        assert!(d.final_white_flash);
        assert!(d.drain_audio().iter().any(|a| a.contains("Ding")));
    }

    #[test]
    fn firing_blows_cover() {
        let mut d = HostDefectionHelperData::default();
        d.set_undetected_defector(true);
        d.start_defection_timer(0, 300, true);
        d.tick(5, true, false);
        assert!(!d.is_undetected_defector());
    }

    #[test]
    fn pilot_invulnerable_no_fx() {
        let mut d = HostDefectionHelperData::default();
        defect_team_residual(&mut d, 10, PILOT_INVULNERABLE_DEFECTION_FRAMES, false);
        assert!(d.is_undetected_defector());
        assert!(!d.do_defector_fx);
        d.tick(10 + PILOT_INVULNERABLE_DEFECTION_FRAMES, false, false);
        assert!(!d.is_undetected_defector());
        assert!(!d.final_white_flash);
    }
}
