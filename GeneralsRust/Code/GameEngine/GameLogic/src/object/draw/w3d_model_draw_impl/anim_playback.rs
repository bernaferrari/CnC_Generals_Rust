/// C++ `setCurAnimDurationInMsec`: multiplier = natural / desired.
/// Do not rewrite the clip's native frame count.
fn anim_duration_multiplier(natural_ms: Real, desired_ms: Real) -> Real {
    if natural_ms > 0.0 && desired_ms > 0.0 {
        natural_ms / desired_ms
    } else {
        1.0
    }
}

/// Advance a discrete AnimMode clip by `speed` frames (HLOD frame-rate multiplier).
fn advance_anim_mode(
    mode: AnimMode,
    frame: i32,
    num_frames: i32,
    steps: i32,
) -> (i32, bool) {
    if num_frames <= 0 || steps <= 0 {
        let complete = matches!(mode, AnimMode::Once | AnimMode::OnceBackwards)
            && match mode {
                AnimMode::Once => frame >= num_frames.saturating_sub(1),
                AnimMode::OnceBackwards => frame <= 0,
                _ => false,
            };
        return (frame, complete);
    }

    let last = num_frames.saturating_sub(1);
    let mut current = frame;
    let mut complete = false;
    for _ in 0..steps {
        match mode {
            AnimMode::Loop | AnimMode::LoopPingPong => {
                current = (current + 1).rem_euclid(num_frames);
                complete = false;
            }
            AnimMode::LoopBackwards => {
                current -= 1;
                if current < 0 {
                    current = last;
                }
                complete = false;
            }
            AnimMode::Manual => {
                complete = false;
            }
            AnimMode::Once => {
                if current < last {
                    current += 1;
                    complete = false;
                } else {
                    complete = true;
                }
            }
            AnimMode::OnceBackwards => {
                if current > 0 {
                    current -= 1;
                    complete = false;
                } else {
                    complete = true;
                }
            }
        }
    }
    (current, complete)
}

impl W3DModelDraw {
    fn current_natural_duration_ms(&self) -> Real {
        let Some(state) = self.current_state() else {
            return 0.0;
        };
        if self.which_anim_in_cur_state < 0 {
            return 0.0;
        }
        let idx = self.which_anim_in_cur_state as usize;
        state
            .animations
            .get(idx)
            .map(|anim| {
                if anim.natural_duration_ms > 0.0 {
                    anim.natural_duration_ms
                } else {
                    self.current_anim_num_frames.max(1) as Real * MSEC_PER_LOGICFRAME_REAL
                }
            })
            .unwrap_or(0.0)
    }

    fn apply_animation_frame_once(&mut self, frame: i32) {
        // C++ setAnimationFrame is a one-shot Set_Animation(handle, frame).
        // Do not latch a manual_frame that reticks forever.
        self.animation_override.manual_frame = None;
        if self.current_anim_num_frames > 0 {
            self.current_anim_frame = frame.clamp(0, self.current_anim_num_frames - 1);
        } else {
            self.current_anim_frame = frame.max(0);
        }
        self.anim_frame_accumulator = 0.0;
        self.current_anim_complete = false;
    }

    fn apply_cur_anim_duration_multiplier(&mut self, desired_ms: Real) {
        let natural = self.current_natural_duration_ms();
        self.current_anim_speed_factor = anim_duration_multiplier(natural, desired_ms);
        self.current_anim_complete = false;
    }

    fn tick_animation_with_speed(&mut self) {
        if self.pause_animation {
            return;
        }
        let Some(cur_state) = self.current_state().cloned() else {
            self.current_anim_complete = true;
            return;
        };
        if self.which_anim_in_cur_state < 0 || cur_state.animations.is_empty() {
            self.current_anim_complete = true;
            return;
        }

        self.current_anim_num_frames = self.animation_total_frames(&cur_state).max(1);
        let speed = if self.current_anim_speed_factor.is_finite() {
            self.current_anim_speed_factor.max(0.0)
        } else {
            1.0
        };
        self.anim_frame_accumulator += speed;
        let steps = self.anim_frame_accumulator.floor() as i32;
        self.anim_frame_accumulator -= steps as Real;
        let (frame, complete) = advance_anim_mode(
            cur_state.anim_mode,
            self.current_anim_frame,
            self.current_anim_num_frames,
            steps,
        );
        self.current_anim_frame = frame;
        self.current_anim_complete = complete;
    }
}
