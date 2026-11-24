//! Defection cover and timer effects for the live match.
//!
//! C++ ObjectDefectionHelper checks expiry before observing firing or death.
//! The flash cadence and authored audio require the current Drawable. A pilot
//! can use the same timer for cover with its effects disabled.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefectionFlash {
    Selected,
    Revealed,
}

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
        }
    }
}

impl HostDefectionHelperData {
    /// C++ startDefectionTimer uses the requested duration without clamping.
    /// Its unsigned deadline wraps, and update compares it as an absolute frame.
    pub fn start_defection_timer(&mut self, now: u32, num_frames: u32, with_fx: bool) {
        if !self.undetected_defector {
            return;
        }
        self.detection_start = now;
        self.detection_end = now.wrapping_add(num_frames);
        self.flash_phase = 0.0;
        self.do_defector_fx = with_fx;
        self.flash_this_frame = false;
        self.final_white_flash = false;
    }

    /// C++ friend_setUndetectedDefector changes only the private status bit.
    /// Timer fields remain intact for Xfer, even after cover is blown.
    pub fn set_undetected_defector(&mut self, on: bool) {
        self.undetected_defector = on;
    }

    pub fn is_undetected_defector(&self) -> bool {
        self.undetected_defector
    }

    /// Blow cover immediately (attack / special power / death).
    pub fn blow_cover(&mut self) {
        self.set_undetected_defector(false);
    }

    /// C++ ObjectDefectionHelper::update. Cover expires even without a Drawable.
    pub(crate) fn tick(
        &mut self,
        now: u32,
        is_firing: bool,
        effectively_dead: bool,
        has_drawable: bool,
    ) -> Option<DefectionFlash> {
        self.flash_this_frame = false;
        self.final_white_flash = false;
        if !self.undetected_defector {
            return None;
        }
        let show_fx = has_drawable && self.do_defector_fx;
        if now >= self.detection_end {
            self.undetected_defector = false;
            self.final_white_flash = show_fx;
            return show_fx.then_some(DefectionFlash::Revealed);
        }
        if effectively_dead || is_firing {
            self.blow_cover();
            return None;
        }
        if show_fx {
            let last_phase = (self.flash_phase as i32) & 1;
            let time_left = (self.detection_end - now) as f32;
            let max_t = DEFECTION_DETECTION_TIME_MAX as f32;
            self.flash_phase += 0.5 * (1.0 - (time_left / max_t));
            let this_phase = (self.flash_phase as i32) & 1;
            if last_phase != 0 && this_phase == 0 {
                self.flash_this_frame = true;
                return Some(DefectionFlash::Selected);
            }
        }
        None
    }
}

impl crate::game_logic::GameLogic {
    /// A nullable C++ Drawable lookup mapped to this instance's current binding.
    pub(in crate::game_logic) fn accepts_defector_drawable(
        &self,
        object: &crate::game_logic::Object,
        effects: &crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        effects.view.as_deref().is_some_and(|view| {
            view.has_current_drawable(&self.live_draw_playback_identity(object))
        })
    }

    /// Submit flash and authored MiscAudio after the caller admits the Drawable.
    pub(in crate::game_logic) fn present_defection_flash(
        &mut self,
        id: crate::game_logic::ObjectId,
        flash: DefectionFlash,
    ) {
        let Some(object) = self.host_object(id) else {
            return;
        };
        let position = object.get_position();
        let color = match flash {
            DefectionFlash::Revealed => [1.0; 3],
            DefectionFlash::Selected => {
                let (use_house, saturation) = {
                    let data = self
                        .world_services
                        .common()
                        .system
                        .runtime_global_data
                        .read();
                    (
                        data.selection_flash_house_color,
                        data.selection_flash_saturation_factor,
                    )
                };
                let color = if use_house {
                    let rgb = if let Some(raw) = object.custom_indicator_color {
                        [(raw >> 16) as u8, (raw >> 8) as u8, raw as u8]
                    } else {
                        object
                            .owner_player_id
                            .and_then(|owner| self.get_player(owner))
                            .map_or([0; 3], |player| {
                                let (r, g, b) = player.color_rgb;
                                [r, g, b]
                            })
                    };
                    rgb.map(|value| f32::from(value) / 255.0)
                } else {
                    [1.0; 3]
                };
                crate::game_logic::host_hero_abilities::saturate_selection_flash_rgb(
                    color, saturation,
                )
            }
        };
        if let Some(object) = self.host_object_mut(id) {
            object.flash_as_selected_with_color(color);
        }
        let event = {
            let misc = self.world_services.common().ini.misc_audio.read();
            let configured = match flash {
                DefectionFlash::Selected => &misc.defector_timer_tick_sound,
                DefectionFlash::Revealed => &misc.defector_timer_ding_sound,
            };
            let name = configured.playable_event_name();
            (!name.is_empty()).then(|| {
                crate::game_logic::AudioEventRequest::new(name)
                    .with_object(id)
                    .with_position(position)
            })
        };
        if let Some(event) = event {
            self.queue_audio_event(event);
        }
    }
}

/// Apply defect residual: change team + start undetected timer.
pub fn defect_team_residual(
    data: &mut HostDefectionHelperData,
    now: u32,
    protection_frames: u32,
    with_fx: bool,
) {
    data.set_undetected_defector(protection_frames > 0);
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
            let _ = d.tick(f, false, false, true);
            assert!(d.is_undetected_defector());
        }
        assert_eq!(
            d.tick(30, false, false, true),
            Some(DefectionFlash::Revealed)
        );
        assert!(!d.is_undetected_defector());
        assert!(d.final_white_flash);
    }

    #[test]
    fn firing_blows_cover() {
        let mut d = HostDefectionHelperData::default();
        d.set_undetected_defector(true);
        d.start_defection_timer(0, 300, true);
        assert_eq!(d.tick(5, true, false, true), None);
        assert!(!d.is_undetected_defector());
    }

    #[test]
    fn pilot_invulnerable_no_fx() {
        let mut d = HostDefectionHelperData::default();
        defect_team_residual(&mut d, 10, PILOT_INVULNERABLE_DEFECTION_FRAMES, false);
        assert!(d.is_undetected_defector());
        assert!(!d.do_defector_fx);
        assert_eq!(
            d.tick(10 + PILOT_INVULNERABLE_DEFECTION_FRAMES, false, false, true),
            None
        );
        assert!(!d.is_undetected_defector());
        assert!(!d.final_white_flash);
    }
}
