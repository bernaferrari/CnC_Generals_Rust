use super::*;

pub(super) struct ProcessAnimateWindowSpiral {
    max_r: f32,
    delta_theta: f32,
}

impl ProcessAnimateWindowSpiral {
    pub(super) fn new(screen_size: (i32, i32)) -> Self {
        let max_r = (screen_size.0 / 2) as f32;
        Self {
            max_r,
            delta_theta: 0.33,
        }
    }
}

impl<T: AnimationTarget> ProcessAnimateWindow<T> for ProcessAnimateWindowSpiral {
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
        anim_win.vel.x = 0.0;
        anim_win.vel.y = 0.0;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow<T>, _screen_size: (i32, i32)) {
        let rest_pos = {
            let win = &anim_win.window;
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let vel = Coord2DF::new(0.0, self.max_r);
        let start_pos = Coord2D::new(
            (vel.y * vel.x.cos()) as i32 + end_pos.x,
            (vel.y * vel.x.sin()) as i32 + end_pos.y,
        );
        let cur_pos = start_pos;
        {
            let win = &mut anim_win.window;
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
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
        cur_pos.x = (vel.y * vel.x.cos()) as i32 + end_pos.x;
        cur_pos.y = (vel.y * vel.x.sin()) as i32 + end_pos.y;
        vel.x += self.delta_theta;
        vel.y -= 5.0;
        let size = {
            let win = &anim_win.window;
            win.get_size()
        };
        let max_size = min(size.0 / 2, size.1 / 2);
        if vel.y < max_size as f32 {
            let rest_pos = anim_win.rest_pos;
            anim_win.finished = true;
            let win = &mut anim_win.window;
            let _ = win.set_position(rest_pos.x, rest_pos.y);
            return true;
        }
        {
            let win = &mut anim_win.window;
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
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
        let end_pos = anim_win.end_pos;
        let mut vel = anim_win.vel;
        cur_pos.x = (vel.y * vel.x.cos()) as i32 + end_pos.x;
        cur_pos.y = (vel.y * vel.x.sin()) as i32 + end_pos.y;
        vel.x -= self.delta_theta;
        vel.y += 5.0;
        if vel.y > self.max_r {
            anim_win.finished = true;
            return true;
        }
        {
            let win = &mut anim_win.window;
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        anim_win.vel = vel;
        false
    }
}
