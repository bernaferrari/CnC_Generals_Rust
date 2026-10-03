use super::*;

pub(super) struct ProcessAnimateWindowSlideFromBottomTimed {
    max_duration_ms: u64,
}

impl ProcessAnimateWindowSlideFromBottomTimed {
    pub(super) fn new() -> Self {
        Self {
            max_duration_ms: 1000,
        }
    }
}

impl<T: AnimationTarget> ProcessAnimateWindow<T> for ProcessAnimateWindowSlideFromBottomTimed {
    fn set_max_duration(&mut self, duration_ms: u64) {
        self.max_duration_ms = duration_ms;
    }

    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow<T>,
        _max_delay_ms: u64,
        screen_size: (i32, i32),
    ) {
        let (screen_width, _) = screen_size;
        let rest_pos = anim_win.rest_pos;
        let start_pos = rest_pos;
        let mut cur_pos = start_pos;
        let end_pos = Coord2D::new(rest_pos.x, rest_pos.y + screen_width);
        {
            let win = &mut anim_win.window;
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        let now = Instant::now();
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            Coord2DF::new(0.0, 0.0),
            now,
            Some(now + Duration::from_millis(self.max_duration_ms)),
        );
        anim_win.finished = false;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow<T>, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = &anim_win.window;
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let start_pos = Coord2D::new(rest_pos.x, rest_pos.y + screen_width);
        let cur_pos = start_pos;
        {
            let win = &mut anim_win.window;
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        let now = Instant::now();
        let delay = anim_win.get_delay();
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            Coord2DF::new(0.0, 0.0),
            now + Duration::from_millis(delay),
            Some(now + Duration::from_millis(self.max_duration_ms + delay)),
        );
        anim_win.finished = false;
    }

    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow<T>,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let end_time = match anim_win.end_time {
            Some(end_time) => end_time,
            None => return true,
        };
        let start_pos = anim_win.start_pos;
        let mut cur_pos = anim_win.cur_pos;
        let end_pos = anim_win.end_pos;
        if now >= end_time {
            cur_pos.y = end_pos.y;
            anim_win.finished = true;
            let win = &mut anim_win.window;
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        let elapsed_ms = now.duration_since(anim_win.start_time).as_millis() as f32;
        let percent_done = elapsed_ms / self.max_duration_ms as f32;
        cur_pos.y = start_pos.y + ((end_pos.y - start_pos.y) as f32 * percent_done) as i32;
        {
            let win = &mut anim_win.window;
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow<T>,
        now: Instant,
        screen_size: (i32, i32),
    ) -> bool {
        self.update_animate_window(anim_win, now, screen_size)
    }
}
