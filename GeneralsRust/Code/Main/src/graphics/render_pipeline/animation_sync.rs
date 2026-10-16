//! Lazy HAnim progression at the admitted WW3D Sync, on the existing Draw owner.
use super::*;
impl ObjectAnimationState {
    pub(super) fn initial_direction(mode: &crate::assets::AuthoredDrawAnimationMode) -> f32 {
        use crate::assets::AuthoredDrawAnimationMode::*;
        if matches!(mode, LoopBackwards | OnceBackwards) {
            -1.
        } else {
            1.
        }
    }
    /// Source Set_Animation resets the Sync baseline after a real binding install.
    pub(super) fn install_animation_clock(&mut self, sync_ms: u32) {
        self.last_sync_ms = sync_ms;
        self.frame_rate_multiplier = 1.;
        self.direction = Self::initial_direction(&self.playback_mode);
    }
    /// Source setter changes only the multiplier, without sampling or rebasing.
    pub(super) fn set_frame_rate_multiplier(&mut self, multiplier: f32) {
        self.frame_rate_multiplier = multiplier;
    }
    pub(super) fn admit_sync(&mut self, sync_ms: u32) {
        use crate::assets::AuthoredDrawAnimationMode::*;
        assert!(
            !matches!(self.playback_mode, Unsupported(_)),
            "unsupported authored animation mode has no installed HAnim clock"
        );
        let old_previous = self.previous_frame;
        self.previous_frame = self.current_frame;
        let terminal = self.num_frames.saturating_sub(1) as f32;
        if !matches!(self.playback_mode, Manual | Unsupported(_)) {
            let elapsed = sync_ms.wrapping_sub(self.last_sync_ms) as f32;
            let delta =
                self.frame_rate * self.frame_rate_multiplier * self.direction * elapsed * 0.001;
            self.current_frame += delta;
            match self.playback_mode {
                Once => {
                    if self.current_frame >= terminal {
                        self.current_frame = terminal;
                    }
                }
                Loop => {
                    if self.current_frame >= terminal {
                        self.current_frame -= terminal;
                    }
                    if self.current_frame >= terminal {
                        self.current_frame = 0.;
                    }
                }
                OnceBackwards => {
                    if self.current_frame < 0. {
                        self.current_frame = 0.;
                    }
                }
                LoopBackwards => {
                    if self.current_frame < 0. {
                        self.current_frame += terminal;
                    }
                    if self.current_frame < 0. {
                        self.current_frame = terminal;
                    }
                }
                LoopPingPong => {
                    if self.direction >= 1. {
                        if self.current_frame >= terminal {
                            self.current_frame = terminal * 2. - self.current_frame;
                            if self.current_frame >= terminal {
                                self.current_frame = terminal;
                            }
                            self.direction *= -1.;
                        }
                    } else if self.current_frame < 0. {
                        self.current_frame = -self.current_frame;
                        if self.current_frame >= terminal {
                            self.current_frame = 0.;
                        }
                        self.direction *= -1.;
                    }
                }
                Manual | Unsupported(_) => unreachable!(),
            }
        }
        self.last_sync_ms = sync_ms;
        // Single_Anim_Progress preserves PrevFrame on repeated sampling.
        if self.current_frame == self.previous_frame {
            self.previous_frame = old_previous;
        }
    }
}

impl ObjectVisualState {
    pub(super) fn install_animation_pause(&mut self, paused: bool, sync_ms: u32) {
        if self.animation_paused == paused {
            return;
        }
        self.animation_paused = paused;
        if let Some(animation) = self.animation.as_mut() {
            // Peek_Animation_And_Info returns stored Frame without advancing.
            animation.previous_frame = animation.current_frame;
            animation.playback_mode = if paused {
                self.animation_resume_mode = Some(animation.playback_mode.clone());
                crate::assets::AuthoredDrawAnimationMode::Manual
            } else {
                self.animation_resume_mode
                    .clone()
                    .unwrap_or(crate::assets::AuthoredDrawAnimationMode::Loop)
            };
            animation.install_animation_clock(sync_ms);
        }
    }
}

#[cfg(test)]
#[path = "animation_sync_tests.rs"]
mod tests;
