//! Policies from C++ ProcessAnimateWindow.cpp and AnimateWindowManager.cpp.
use crate::{AnimationType, Coord2D, Coord2DF};
use std::cmp::min;
use std::time::{Duration, Instant};

mod no_op;
use no_op::ProcessAnimateWindowNoOp;
mod slide_from_right;
use slide_from_right::ProcessAnimateWindowSlideFromRight;
mod slide_from_left;
use slide_from_left::ProcessAnimateWindowSlideFromLeft;
mod slide_from_top;
use slide_from_top::ProcessAnimateWindowSlideFromTop;
mod slide_from_top_fast;
use slide_from_top_fast::ProcessAnimateWindowSlideFromTopFast;
mod slide_from_bottom;
use slide_from_bottom::ProcessAnimateWindowSlideFromBottom;
mod slide_from_bottom_timed;
use slide_from_bottom_timed::ProcessAnimateWindowSlideFromBottomTimed;
mod spiral;
use spiral::ProcessAnimateWindowSpiral;
mod slide_from_right_fast;
use slide_from_right_fast::ProcessAnimateWindowSlideFromRightFast;
mod manager;
pub use manager::AnimateWindowManager;

/// Geometry operations required by the original shell animation policies.
///
/// Mutations must be synchronous: later animations and callbacks observe the position immediately.
/// The target chooses its ownership mechanism; the model adds no shared ownership or locking.
pub trait AnimationTarget {
    fn get_position(&self) -> (i32, i32);
    fn get_size(&self) -> (i32, i32);
    fn set_position(&mut self, x: i32, y: i32);
}

#[derive(Debug, Clone)]
pub struct AnimateWindow<T: AnimationTarget> {
    window: T,
    anim_type: AnimationType,
    delay_ms: u64,
    start_pos: Coord2D,
    end_pos: Coord2D,
    cur_pos: Coord2D,
    rest_pos: Coord2D,
    vel: Coord2DF,
    needs_to_finish: bool,
    finished: bool,
    start_time: Instant,
    end_time: Option<Instant>,
}

impl<T: AnimationTarget> AnimateWindow<T> {
    pub fn new(window: T, anim_type: AnimationType, needs_to_finish: bool) -> Self {
        Self {
            window,
            anim_type,
            delay_ms: 0,
            start_pos: Coord2D::zero(),
            end_pos: Coord2D::zero(),
            cur_pos: Coord2D::zero(),
            rest_pos: Coord2D::zero(),
            vel: Coord2DF::new(0.0, 0.0),
            needs_to_finish,
            finished: false,
            start_time: Instant::now(),
            end_time: None,
        }
    }

    pub fn set_anim_data(
        &mut self,
        start_pos: Coord2D,
        end_pos: Coord2D,
        cur_pos: Coord2D,
        rest_pos: Coord2D,
        vel: Coord2DF,
        start_time: Instant,
        end_time: Option<Instant>,
    ) {
        self.start_pos = start_pos;
        self.end_pos = end_pos;
        self.cur_pos = cur_pos;
        self.rest_pos = rest_pos;
        self.vel = vel;
        self.start_time = start_time;
        self.end_time = end_time;
    }

    pub fn set_delay(&mut self, delay_ms: u64) {
        self.delay_ms = delay_ms;
    }

    pub fn get_delay(&self) -> u64 {
        self.delay_ms
    }
}

trait ProcessAnimateWindow<T: AnimationTarget> {
    fn init_animate_window(&self, anim_win: &mut AnimateWindow<T>, screen_size: (i32, i32));
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow<T>,
        max_delay_ms: u64,
        screen_size: (i32, i32),
    );
    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow<T>,
        now: Instant,
        screen_size: (i32, i32),
    ) -> bool;
    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow<T>,
        now: Instant,
        screen_size: (i32, i32),
    ) -> bool;
    fn set_max_duration(&mut self, _duration_ms: u64) {}
}

#[cfg(test)]
mod tests;
