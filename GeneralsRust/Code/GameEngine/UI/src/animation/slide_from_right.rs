use super::*;

pub(super) struct ProcessAnimateWindowSlideFromRight {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromRight {
    pub(super) fn new() -> Self {
        let slow_down_ratio = 0.67;
        Self {
            max_vel: Coord2DF::new(-40.0, 0.0),
            slow_down_threshold: 80,
            slow_down_ratio,
            speed_up_ratio: 2.0 - slow_down_ratio,
        }
    }
}

impl<T: AnimationTarget> ProcessAnimateWindow<T> for ProcessAnimateWindowSlideFromRight {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow<T>,
        max_delay_ms: u64,
        _screen_size: (i32, i32),
    ) {
        if anim_win.get_delay() > 0 {
            anim_win.start_time =
                Instant::now() + Duration::from_millis(max_delay_ms - anim_win.get_delay());
        }
        anim_win.vel.x *= -1.0;
        anim_win.vel.y *= -1.0;
        anim_win.finished = false;
        let pos = {
            let win = &anim_win.window;
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        anim_win.cur_pos.y = pos.y;
        anim_win.end_pos.y = pos.y;
        anim_win.start_pos.y = pos.y;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow<T>, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = &anim_win.window;
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let travel_distance = screen_width;
        let start_pos = Coord2D::new(rest_pos.x + travel_distance, rest_pos.y);
        let cur_pos = start_pos;
        {
            let win = &mut anim_win.window;
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        let vel = self.max_vel;
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            vel,
            Instant::now() + Duration::from_millis(anim_win.get_delay()),
            None,
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
        let mut cur_pos = anim_win.cur_pos;
        let end_pos = anim_win.end_pos;
        let mut vel = anim_win.vel;
        cur_pos.x += vel.x as i32;

        if cur_pos.x < end_pos.x {
            cur_pos.x = end_pos.x;
            anim_win.finished = true;
            return true;
        }
        {
            let win = &mut anim_win.window;
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        if cur_pos.x - end_pos.x <= self.slow_down_threshold {
            vel.x *= self.slow_down_ratio;
        }
        if vel.x >= -1.0 {
            vel.x = -1.0;
        }
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
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
        let mut cur_pos = anim_win.cur_pos;
        let start_pos = anim_win.start_pos;
        let mut vel = anim_win.vel;
        cur_pos.x += vel.x as i32;

        if cur_pos.x > start_pos.x {
            cur_pos.x = start_pos.x;
            anim_win.finished = true;
            let win = &mut anim_win.window;
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let win = &mut anim_win.window;
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        let end_pos = anim_win.end_pos;
        if cur_pos.x - end_pos.x <= self.slow_down_threshold {
            vel.x *= self.speed_up_ratio;
        } else {
            vel.x = -self.max_vel.x;
        }
        if vel.x > -self.max_vel.x {
            vel.x = -self.max_vel.x;
        }
        anim_win.vel = vel;
        false
    }
}
