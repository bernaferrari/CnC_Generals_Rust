//! Instance-owned CPU viewport filter phase; W3DView and native rendering share it.
use super::view::{FilterMode, FilterType, Point3, Vector2, ViewFilterComposite};
use serde::{Deserialize, Serialize};

pub const MOTION_BLUR_MAX_COUNT: i32 = 60;
const COUNT_STEP: i32 = 5;
const DEFAULT_PAN_FACTOR: i32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewFilterState {
    filter: FilterType,
    mode: FilterMode,
    position: Option<Point3>,
    fade_total: i32,
    fade_progress: i32,
    fade_direction: i32,
    count: i32,
    decrement: bool,
    last_logic_frame: u32,
    skip_render: bool,
    prior_scroll: Vector2,
    pan_factor: i32,
}
#[derive(Debug, Clone, Copy)]
pub struct FilterPostRender {
    pub look_at: Option<Point3>,
    pub continue_effect: bool,
    pub skip_render: bool,
    pub motion_blur_count: i32,
    pub pan: bool,
    pub additive: bool,
    pub fade: f32,
    pub scroll_delta: Vector2,
}
impl Default for ViewFilterState {
    fn default() -> Self {
        Self {
            filter: FilterType::Null,
            mode: FilterMode::Null,
            position: None,
            fade_total: 0,
            fade_progress: 0,
            fade_direction: 0,
            count: 0,
            decrement: false,
            last_logic_frame: 0,
            skip_render: false,
            prior_scroll: Vector2::zero(),
            pan_factor: DEFAULT_PAN_FACTOR,
        }
    }
}
impl ViewFilterState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn filter(&self) -> FilterType {
        self.filter
    }
    pub fn mode(&self) -> FilterMode {
        self.mode
    }
    pub fn skip_render(&self) -> bool {
        self.skip_render
    }
    pub fn set_filter(&mut self, filter: FilterType) -> bool {
        self.filter = filter;
        self.setup();
        true
    }
    pub fn set_mode(&mut self, mode: FilterMode) -> bool {
        self.mode = mode;
        self.setup();
        true
    }
    pub fn set_pos(&mut self, position: Point3) {
        self.position = Some(position);
    }
    pub fn set_motion_blur_follow(&mut self, amount: i32) {
        self.mode = FilterMode::MBPanAlpha;
        self.filter = FilterType::MotionBlur;
        self.setup();
        self.pan_factor = if amount < 1 {
            DEFAULT_PAN_FACTOR
        } else {
            amount
        };
    }
    pub fn set_fade_parameters(&mut self, frames: i32, direction: i32) {
        self.fade_total = frames.max(0);
        self.fade_progress = 0;
        self.fade_direction = direction;
    }
    fn setup(&mut self) {
        if self.filter != FilterType::MotionBlur || self.mode == FilterMode::Null {
            return;
        }
        match self.mode {
            FilterMode::MBPanAlpha => self.pan_factor = DEFAULT_PAN_FACTOR,
            FilterMode::MBPanAlpha1 => self.pan_factor = 1,
            FilterMode::MBPanAlpha2 => self.pan_factor = 2,
            FilterMode::MBPanAlpha3 => self.pan_factor = 3,
            _ => {} // CPP setup preserves the prior factor outside pan modes.
        }
        if self.mode != FilterMode::MBEndPanAlpha {
            self.count = 0;
        }
        self.decrement = false;
        self.skip_render = false;
        if matches!(
            self.mode,
            FilterMode::MBOutAlpha | FilterMode::MBOutSaturate
        ) {
            self.count = MOTION_BLUR_MAX_COUNT;
            self.decrement = true;
        }
    }
    fn uses_fade(&self) -> bool {
        // C++ BW::set and CrossFade::preRender advance their fades; the
        // MotionBlur shader has no fade clock. Preserve the existing Crossfade
        // adapter here; its separate mask/extra-render protocol is not modeled
        // by this shared phase record yet.
        matches!(
            self.filter,
            FilterType::BlackAndWhite | FilterType::Crossfade
        )
    }
    pub fn tick_fade(&mut self) {
        if !self.uses_fade() || self.fade_direction == 0 {
            return;
        }
        self.fade_progress += 1;
        if self.fade_progress >= self.fade_total {
            if self.fade_direction < 0 && self.filter == FilterType::BlackAndWhite {
                self.mode = FilterMode::Null;
                self.filter = FilterType::Null;
            }
            self.fade_total = 0;
            self.fade_progress = 0;
            self.fade_direction = 0;
        }
    }
    pub fn composite(&self, scroll_delta: Vector2) -> ViewFilterComposite {
        let fade = if self.uses_fade() && self.fade_total > 0 {
            let t = (self.fade_progress as f32 / self.fade_total as f32).clamp(0., 1.);
            if self.fade_direction < 0 { 1. - t } else { t }
        } else if self.filter == FilterType::Null {
            0.
        } else {
            1.
        };
        ViewFilterComposite {
            filter: self.filter,
            mode: self.mode,
            fade,
            scroll_delta,
            zoom_to: self.position,
        }
    }
    /// Call only after successful render-to-texture admission. No tactical View
    /// lookup or camera mutation occurs here; lookAt is an owned postRender result.
    pub fn post_render(&mut self, logic_frame: u32, scroll: Vector2) -> FilterPostRender {
        let mut result = FilterPostRender {
            look_at: None,
            continue_effect: self.filter != FilterType::Null,
            skip_render: self.skip_render,
            motion_blur_count: self.count,
            pan: false,
            additive: matches!(
                self.mode,
                FilterMode::MBInAndOutSaturate
                    | FilterMode::MBInSaturate
                    | FilterMode::MBOutSaturate
            ),
            fade: self.composite(scroll).fade,
            scroll_delta: scroll,
        };
        if self.filter != FilterType::MotionBlur {
            return result;
        }
        let pan = matches!(
            self.mode,
            FilterMode::MBPanAlpha
                | FilterMode::MBPanAlpha1
                | FilterMode::MBPanAlpha2
                | FilterMode::MBPanAlpha3
        );
        if pan {
            let length = scroll.length();
            self.decrement = false;
            self.count = ((length * 200. * self.pan_factor as f32 / DEFAULT_PAN_FACTOR as f32)
                as i32)
                .max(self.pan_factor / 2)
                .min(self.pan_factor);
            self.prior_scroll = scroll;
        } else if self.mode == FilterMode::MBEndPanAlpha {
            self.decrement = false;
            self.count -= 1;
            if self.count < 2 {
                result.continue_effect = false;
            }
        }
        let pan = pan || self.mode == FilterMode::MBEndPanAlpha;
        self.skip_render = false;
        if !pan && self.last_logic_frame != logic_frame {
            if self.decrement {
                self.count -= COUNT_STEP;
                if self.count < 1 {
                    self.decrement = false;
                    result.continue_effect = false;
                } else {
                    self.skip_render = true;
                }
            } else {
                self.count += COUNT_STEP;
                if self.count >= MOTION_BLUR_MAX_COUNT {
                    self.decrement = true;
                    if matches!(
                        self.mode,
                        FilterMode::MBInAndOutAlpha | FilterMode::MBInAndOutSaturate
                    ) && self.position.is_some()
                    {
                        result.look_at = self.position;
                    } else {
                        result.continue_effect = false;
                    }
                } else {
                    self.skip_render = true;
                }
            }
        }
        self.last_logic_frame = logic_frame;
        result.pan = pan;
        if self.mode == FilterMode::MBEndPanAlpha {
            result.scroll_delta = self.prior_scroll;
        }
        result.motion_blur_count = self.count;
        result.skip_render = self.skip_render;
        if !result.continue_effect {
            self.position = None;
            self.filter = FilterType::Null;
            self.mode = FilterMode::Null;
        }
        result
    }
    pub fn validate(&self) -> Result<(), String> {
        if self
            .position
            .is_some_and(|p| !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()))
            || !(self.prior_scroll.x.is_finite() && self.prior_scroll.y.is_finite())
            || self.fade_total < 0
            || self.fade_progress < 0
            || self.fade_progress > self.fade_total
            || self.pan_factor < 1
            || self.count < -COUNT_STEP
            || self.count > self.pan_factor.max(MOTION_BLUR_MAX_COUNT)
            || !(-1..=1).contains(&self.fade_direction)
        {
            return Err("invalid viewport filter continuation".into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "view_filter_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "view_filter_cpp_tests.rs"]
mod cpp_tests;
