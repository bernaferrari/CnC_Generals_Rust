use super::*;

/// Animation window manager for handling screen transitions (C++-accurate).
pub struct AnimateWindowManager<T: AnimationTarget> {
    win_list: Vec<AnimateWindow<T>>,
    win_must_finish_list: Vec<AnimateWindow<T>>,
    needs_update: bool,
    reverse: bool,
    screen_size: (i32, i32),
    slide_from_right: ProcessAnimateWindowSlideFromRight,
    slide_from_right_fast: ProcessAnimateWindowSlideFromRightFast,
    slide_from_left: ProcessAnimateWindowSlideFromLeft,
    slide_from_top: ProcessAnimateWindowSlideFromTop,
    slide_from_top_fast: ProcessAnimateWindowSlideFromTopFast,
    slide_from_bottom: ProcessAnimateWindowSlideFromBottom,
    slide_from_bottom_timed: ProcessAnimateWindowSlideFromBottomTimed,
    spiral: ProcessAnimateWindowSpiral,
    no_op: ProcessAnimateWindowNoOp,
}

impl<T: AnimationTarget> AnimateWindowManager<T> {
    pub fn new() -> Self {
        let screen_size = (800, 600);
        Self {
            win_list: Vec::new(),
            win_must_finish_list: Vec::new(),
            needs_update: false,
            reverse: false,
            screen_size,
            slide_from_right: ProcessAnimateWindowSlideFromRight::new(),
            slide_from_right_fast: ProcessAnimateWindowSlideFromRightFast::new(),
            slide_from_left: ProcessAnimateWindowSlideFromLeft::new(),
            slide_from_top: ProcessAnimateWindowSlideFromTop::new(),
            slide_from_top_fast: ProcessAnimateWindowSlideFromTopFast::new(),
            slide_from_bottom: ProcessAnimateWindowSlideFromBottom::new(),
            slide_from_bottom_timed: ProcessAnimateWindowSlideFromBottomTimed::new(),
            spiral: ProcessAnimateWindowSpiral::new(screen_size),
            no_op: ProcessAnimateWindowNoOp,
        }
    }

    pub fn set_screen_size(&mut self, width: i32, height: i32) {
        self.screen_size = (width, height);
        self.spiral = ProcessAnimateWindowSpiral::new(self.screen_size);
    }

    pub fn init(&mut self) {
        self.win_list.clear();
        self.win_must_finish_list.clear();
        self.needs_update = false;
        self.reverse = false;
    }

    pub fn reset(&mut self) {
        self.reset_to_rest_position();
        self.win_list.clear();
        self.win_must_finish_list.clear();
        self.needs_update = false;
        self.reverse = false;
    }

    pub fn update(&mut self) {
        self.update_at(Instant::now());
    }

    /// Advance one original shell update at the caller's timestamp.
    /// Velocity styles advance once per call, rather than by elapsed time.
    pub fn update_at(&mut self, now: Instant) {
        let reverse = self.reverse;
        let screen_size = self.screen_size;
        if self.needs_update {
            self.needs_update = false;
            let (
                slide_from_right,
                slide_from_right_fast,
                slide_from_left,
                slide_from_top,
                slide_from_top_fast,
                slide_from_bottom,
                slide_from_bottom_timed,
                spiral,
                no_op,
            ) = (
                &self.slide_from_right,
                &self.slide_from_right_fast,
                &self.slide_from_left,
                &self.slide_from_top,
                &self.slide_from_top_fast,
                &self.slide_from_bottom,
                &self.slide_from_bottom_timed,
                &self.spiral,
                &self.no_op,
            );
            for anim_win in &mut self.win_must_finish_list {
                let process: &dyn ProcessAnimateWindow<T> = match anim_win.anim_type {
                    AnimationType::SlideRight => slide_from_right,
                    AnimationType::SlideRightFast => slide_from_right_fast,
                    AnimationType::SlideLeft => slide_from_left,
                    AnimationType::SlideTop => slide_from_top,
                    AnimationType::SlideTopFast => slide_from_top_fast,
                    AnimationType::SlideBottom => slide_from_bottom,
                    AnimationType::SlideBottomTimed => slide_from_bottom_timed,
                    AnimationType::Spiral => spiral,
                    AnimationType::None => no_op,
                };

                let finished = if reverse {
                    process.reverse_animate_window(anim_win, now, screen_size)
                } else {
                    process.update_animate_window(anim_win, now, screen_size)
                };
                if !finished {
                    self.needs_update = true;
                }
            }
        }

        let (
            slide_from_right,
            slide_from_right_fast,
            slide_from_left,
            slide_from_top,
            slide_from_top_fast,
            slide_from_bottom,
            slide_from_bottom_timed,
            spiral,
            no_op,
        ) = (
            &self.slide_from_right,
            &self.slide_from_right_fast,
            &self.slide_from_left,
            &self.slide_from_top,
            &self.slide_from_top_fast,
            &self.slide_from_bottom,
            &self.slide_from_bottom_timed,
            &self.spiral,
            &self.no_op,
        );
        for anim_win in &mut self.win_list {
            let process: &dyn ProcessAnimateWindow<T> = match anim_win.anim_type {
                AnimationType::SlideRight => slide_from_right,
                AnimationType::SlideRightFast => slide_from_right_fast,
                AnimationType::SlideLeft => slide_from_left,
                AnimationType::SlideTop => slide_from_top,
                AnimationType::SlideTopFast => slide_from_top_fast,
                AnimationType::SlideBottom => slide_from_bottom,
                AnimationType::SlideBottomTimed => slide_from_bottom_timed,
                AnimationType::Spiral => spiral,
                AnimationType::None => no_op,
            };
            if reverse {
                process.reverse_animate_window(anim_win, now, screen_size);
            } else {
                process.update_animate_window(anim_win, now, screen_size);
            }
        }
    }

    pub fn register_window(
        &mut self,
        window: T,
        anim_type: AnimationType,
        needs_to_finish: bool,
        duration_ms: u64,
        delay_ms: u64,
    ) {
        if anim_type == AnimationType::None {
            // None never enters either animation list.
            return;
        }
        let mut anim_win = AnimateWindow::new(window, anim_type, needs_to_finish);
        anim_win.set_delay(delay_ms);
        let screen_size = self.screen_size;
        let process = self.process_for_mut(anim_type);
        process.set_max_duration(duration_ms);
        process.init_animate_window(&mut anim_win, screen_size);
        if needs_to_finish {
            self.win_must_finish_list.push(anim_win);
            self.needs_update = true;
        } else {
            self.win_list.push(anim_win);
        }
    }

    fn process_for(&self, anim_type: AnimationType) -> &dyn ProcessAnimateWindow<T> {
        match anim_type {
            AnimationType::SlideRight => &self.slide_from_right,
            AnimationType::SlideRightFast => &self.slide_from_right_fast,
            AnimationType::SlideLeft => &self.slide_from_left,
            AnimationType::SlideTop => &self.slide_from_top,
            AnimationType::SlideTopFast => &self.slide_from_top_fast,
            AnimationType::SlideBottom => &self.slide_from_bottom,
            AnimationType::SlideBottomTimed => &self.slide_from_bottom_timed,
            AnimationType::Spiral => &self.spiral,
            AnimationType::None => &self.no_op,
        }
    }

    fn process_for_mut(&mut self, anim_type: AnimationType) -> &mut dyn ProcessAnimateWindow<T> {
        match anim_type {
            AnimationType::SlideRight => &mut self.slide_from_right,
            AnimationType::SlideRightFast => &mut self.slide_from_right_fast,
            AnimationType::SlideLeft => &mut self.slide_from_left,
            AnimationType::SlideTop => &mut self.slide_from_top,
            AnimationType::SlideTopFast => &mut self.slide_from_top_fast,
            AnimationType::SlideBottom => &mut self.slide_from_bottom,
            AnimationType::SlideBottomTimed => &mut self.slide_from_bottom_timed,
            AnimationType::Spiral => &mut self.spiral,
            AnimationType::None => &mut self.no_op,
        }
    }

    pub fn reverse_animate_window(&mut self) {
        self.reverse = true;
        self.needs_update = true;
        let screen_size = self.screen_size;
        let mut max_delay = 0;
        for anim_win in &self.win_must_finish_list {
            if anim_win.get_delay() > max_delay {
                max_delay = anim_win.get_delay();
            }
        }

        let (
            slide_from_right,
            slide_from_right_fast,
            slide_from_left,
            slide_from_top,
            slide_from_top_fast,
            slide_from_bottom,
            slide_from_bottom_timed,
            spiral,
            no_op,
        ) = (
            &self.slide_from_right,
            &self.slide_from_right_fast,
            &self.slide_from_left,
            &self.slide_from_top,
            &self.slide_from_top_fast,
            &self.slide_from_bottom,
            &self.slide_from_bottom_timed,
            &self.spiral,
            &self.no_op,
        );
        for anim_win in &mut self.win_must_finish_list {
            let process: &dyn ProcessAnimateWindow<T> = match anim_win.anim_type {
                AnimationType::SlideRight => slide_from_right,
                AnimationType::SlideRightFast => slide_from_right_fast,
                AnimationType::SlideLeft => slide_from_left,
                AnimationType::SlideTop => slide_from_top,
                AnimationType::SlideTopFast => slide_from_top_fast,
                AnimationType::SlideBottom => slide_from_bottom,
                AnimationType::SlideBottomTimed => slide_from_bottom_timed,
                AnimationType::Spiral => spiral,
                AnimationType::None => no_op,
            };
            process.init_reverse_animate_window(anim_win, max_delay, screen_size);
            anim_win.finished = false;
        }

        let (
            slide_from_right,
            slide_from_right_fast,
            slide_from_left,
            slide_from_top,
            slide_from_top_fast,
            slide_from_bottom,
            slide_from_bottom_timed,
            spiral,
            no_op,
        ) = (
            &self.slide_from_right,
            &self.slide_from_right_fast,
            &self.slide_from_left,
            &self.slide_from_top,
            &self.slide_from_top_fast,
            &self.slide_from_bottom,
            &self.slide_from_bottom_timed,
            &self.spiral,
            &self.no_op,
        );
        for anim_win in &mut self.win_list {
            let process: &dyn ProcessAnimateWindow<T> = match anim_win.anim_type {
                AnimationType::SlideRight => slide_from_right,
                AnimationType::SlideRightFast => slide_from_right_fast,
                AnimationType::SlideLeft => slide_from_left,
                AnimationType::SlideTop => slide_from_top,
                AnimationType::SlideTopFast => slide_from_top_fast,
                AnimationType::SlideBottom => slide_from_bottom,
                AnimationType::SlideBottomTimed => slide_from_bottom_timed,
                AnimationType::Spiral => spiral,
                AnimationType::None => no_op,
            };
            process.init_reverse_animate_window(anim_win, 0, screen_size);
            anim_win.finished = false;
        }
    }

    pub fn reset_to_rest_position(&mut self) {
        for anim_win in &mut self.win_must_finish_list {
            let rest_pos = anim_win.rest_pos;
            let win = &mut anim_win.window;
            let _ = win.set_position(rest_pos.x, rest_pos.y);
        }
        for anim_win in &mut self.win_list {
            let rest_pos = anim_win.rest_pos;
            let win = &mut anim_win.window;
            let _ = win.set_position(rest_pos.x, rest_pos.y);
        }
    }

    pub fn is_finished(&self) -> bool {
        !self.needs_update
    }

    pub fn is_reversed(&self) -> bool {
        self.reverse
    }

    pub fn is_empty(&self) -> bool {
        self.win_list.is_empty() && self.win_must_finish_list.is_empty()
    }
}

impl<T: AnimationTarget> Default for AnimateWindowManager<T> {
    fn default() -> Self {
        Self::new()
    }
}
