// Split from `gui/shell/base.rs` dump. Included by `base/mod.rs`.
// C++ AnimateWindow.cpp: transition animations and AnimateWindowManager.
#[derive(Debug, Clone)]
pub struct AnimateWindow {
    window: Rc<RefCell<GameWindow>>,
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

impl AnimateWindow {
    pub fn new(
        window: Rc<RefCell<GameWindow>>,
        anim_type: AnimationType,
        needs_to_finish: bool,
    ) -> Self {
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

trait ProcessAnimateWindow {
    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32));
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        max_delay_ms: u64,
        screen_size: (i32, i32),
    );
    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        screen_size: (i32, i32),
    ) -> bool;
    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        screen_size: (i32, i32),
    ) -> bool;
    fn set_max_duration(&mut self, _duration_ms: u64) {}
}

struct ProcessAnimateWindowNoOp;

impl ProcessAnimateWindow for ProcessAnimateWindowNoOp {
    fn init_animate_window(&self, _anim_win: &mut AnimateWindow, _screen_size: (i32, i32)) {}
    fn init_reverse_animate_window(
        &self,
        _anim_win: &mut AnimateWindow,
        _max_delay_ms: u64,
        _screen_size: (i32, i32),
    ) {
    }
    fn update_animate_window(
        &self,
        _anim_win: &mut AnimateWindow,
        _now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        true
    }
    fn reverse_animate_window(
        &self,
        _anim_win: &mut AnimateWindow,
        _now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        true
    }
}

struct ProcessAnimateWindowSlideFromRight {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromRight {
    fn new() -> Self {
        let slow_down_ratio = 0.67;
        Self {
            max_vel: Coord2DF::new(-40.0, 0.0),
            slow_down_threshold: 80,
            slow_down_ratio,
            speed_up_ratio: 2.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromRight {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        anim_win.cur_pos.y = pos.y;
        anim_win.end_pos.y = pos.y;
        anim_win.start_pos.y = pos.y;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let travel_distance = screen_width;
        let start_pos = Coord2D::new(rest_pos.x + travel_distance, rest_pos.y);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
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
        anim_win: &mut AnimateWindow,
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
            let mut win = anim_win.window.borrow_mut();
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
        anim_win: &mut AnimateWindow,
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
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
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

struct ProcessAnimateWindowSlideFromLeft {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromLeft {
    fn new() -> Self {
        let slow_down_ratio = 0.67;
        Self {
            max_vel: Coord2DF::new(40.0, 0.0),
            slow_down_threshold: 80,
            slow_down_ratio,
            speed_up_ratio: 2.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromLeft {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let start_pos = Coord2D::new(rest_pos.x - screen_width, rest_pos.y);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
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
        anim_win: &mut AnimateWindow,
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
        if cur_pos.x > end_pos.x {
            cur_pos.x = end_pos.x;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            anim_win.finished = true;
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        if end_pos.x - cur_pos.x <= self.slow_down_threshold {
            vel.x *= self.slow_down_ratio;
        }
        if vel.x < 1.0 {
            vel.x = 1.0;
        }
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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

        if cur_pos.x < start_pos.x {
            cur_pos.x = start_pos.x;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        let end_pos = anim_win.end_pos;
        if end_pos.x - cur_pos.x <= self.slow_down_threshold {
            vel.x *= self.speed_up_ratio;
        } else {
            vel.x = -self.max_vel.x;
        }
        if vel.x < -self.max_vel.x {
            vel.x = -self.max_vel.x;
        }
        anim_win.vel = vel;
        false
    }
}

struct ProcessAnimateWindowSlideFromTop {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromTop {
    fn new() -> Self {
        let slow_down_ratio = 0.67;
        Self {
            max_vel: Coord2DF::new(0.0, 40.0),
            slow_down_threshold: 80,
            slow_down_ratio,
            speed_up_ratio: 2.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromTop {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let travel_distance = screen_width;
        let start_pos = Coord2D::new(rest_pos.x, rest_pos.y - travel_distance);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
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
        anim_win: &mut AnimateWindow,
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
        cur_pos.y += vel.y as i32;
        if cur_pos.y > end_pos.y {
            cur_pos.y = end_pos.y;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            anim_win.finished = true;
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        if end_pos.y - cur_pos.y <= self.slow_down_threshold {
            vel.y *= self.slow_down_ratio;
        }
        if vel.y <= 1.0 {
            vel.y = 1.0;
        }
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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
        cur_pos.y += vel.y as i32;

        if cur_pos.y < start_pos.y {
            cur_pos.y = start_pos.y;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        let end_pos = anim_win.end_pos;
        if end_pos.y - cur_pos.y <= self.slow_down_threshold {
            vel.y *= self.speed_up_ratio;
        } else {
            vel.y = -self.max_vel.y;
        }
        if vel.y < -self.max_vel.y {
            vel.y = -self.max_vel.y;
        }
        anim_win.vel = vel;
        false
    }
}

struct ProcessAnimateWindowSlideFromTopFast {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromTopFast {
    fn new() -> Self {
        let slow_down_ratio = 0.67;
        Self {
            max_vel: Coord2DF::new(0.0, 60.0),
            slow_down_threshold: 40,
            slow_down_ratio,
            speed_up_ratio: 4.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromTopFast {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let travel_distance = screen_width;
        let start_pos = Coord2D::new(rest_pos.x, rest_pos.y - travel_distance);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
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
        anim_win: &mut AnimateWindow,
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
        cur_pos.y += vel.y as i32;
        if cur_pos.y > end_pos.y {
            cur_pos.y = end_pos.y;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            anim_win.finished = true;
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        if end_pos.y - cur_pos.y <= self.slow_down_threshold {
            vel.y *= self.slow_down_ratio;
        }
        if vel.y <= 1.0 {
            vel.y = 1.0;
        }
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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
        cur_pos.y += vel.y as i32;

        if cur_pos.y < start_pos.y {
            cur_pos.y = start_pos.y;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        let end_pos = anim_win.end_pos;
        if end_pos.y - cur_pos.y <= self.slow_down_threshold {
            vel.y *= self.speed_up_ratio;
        } else {
            vel.y = -self.max_vel.y;
        }
        if vel.y < -self.max_vel.y {
            vel.y = -self.max_vel.y;
        }
        anim_win.vel = vel;
        false
    }
}

struct ProcessAnimateWindowSlideFromBottom {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromBottom {
    fn new() -> Self {
        let slow_down_ratio = 0.67;
        Self {
            max_vel: Coord2DF::new(0.0, -40.0),
            slow_down_threshold: 80,
            slow_down_ratio,
            speed_up_ratio: 2.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromBottom {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let travel_distance = screen_width;
        let start_pos = Coord2D::new(rest_pos.x, rest_pos.y + travel_distance);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
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
        anim_win: &mut AnimateWindow,
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
        cur_pos.y += vel.y as i32;
        if cur_pos.y < end_pos.y {
            cur_pos.y = end_pos.y;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        if cur_pos.y - end_pos.y <= self.slow_down_threshold {
            vel.y *= self.slow_down_ratio;
        }
        if vel.y >= -1.0 {
            vel.y = -1.0;
        }
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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
        cur_pos.y += vel.y as i32;

        if cur_pos.y > start_pos.y {
            cur_pos.y = start_pos.y;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        let end_pos = anim_win.end_pos;
        if cur_pos.y - end_pos.y <= self.slow_down_threshold {
            vel.y *= self.speed_up_ratio;
        } else {
            vel.y = -self.max_vel.y;
        }
        if vel.y > -self.max_vel.y {
            vel.y = -self.max_vel.y;
        }
        anim_win.vel = vel;
        false
    }
}

struct ProcessAnimateWindowSlideFromBottomTimed {
    max_duration_ms: u64,
}

impl ProcessAnimateWindowSlideFromBottomTimed {
    fn new() -> Self {
        Self {
            max_duration_ms: 1000,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromBottomTimed {
    fn set_max_duration(&mut self, duration_ms: u64) {
        self.max_duration_ms = duration_ms;
    }

    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        _max_delay_ms: u64,
        screen_size: (i32, i32),
    ) {
        let (screen_width, _) = screen_size;
        let rest_pos = anim_win.rest_pos;
        let start_pos = rest_pos;
        let mut cur_pos = start_pos;
        let end_pos = Coord2D::new(rest_pos.x, rest_pos.y + screen_width);
        {
            let mut win = anim_win.window.borrow_mut();
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

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let start_pos = Coord2D::new(rest_pos.x, rest_pos.y + screen_width);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
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
        anim_win: &mut AnimateWindow,
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
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        let elapsed_ms = now.duration_since(anim_win.start_time).as_millis() as f32;
        let percent_done = elapsed_ms / self.max_duration_ms as f32;
        cur_pos.y = start_pos.y + ((end_pos.y - start_pos.y) as f32 * percent_done) as i32;
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        screen_size: (i32, i32),
    ) -> bool {
        self.update_animate_window(anim_win, now, screen_size)
    }
}

struct ProcessAnimateWindowSpiral {
    max_r: f32,
    delta_theta: f32,
}

impl ProcessAnimateWindowSpiral {
    fn new(screen_size: (i32, i32)) -> Self {
        let max_r = (screen_size.0 / 2) as f32;
        Self {
            max_r,
            delta_theta: 0.33,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSpiral {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, _screen_size: (i32, i32)) {
        let rest_pos = {
            let win = anim_win.window.borrow();
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
            let mut win = anim_win.window.borrow_mut();
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
        anim_win: &mut AnimateWindow,
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
            let win = anim_win.window.borrow();
            win.get_size()
        };
        let max_size = min(size.0 / 2, size.1 / 2);
        if vel.y < max_size as f32 {
            let rest_pos = anim_win.rest_pos;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(rest_pos.x, rest_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        anim_win.vel = vel;
        false
    }
}

struct ProcessAnimateWindowSlideFromRightFast {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromRightFast {
    fn new() -> Self {
        let slow_down_ratio = 0.77;
        Self {
            max_vel: Coord2DF::new(-80.0, 0.0),
            slow_down_threshold: 60,
            slow_down_ratio,
            speed_up_ratio: 3.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromRightFast {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
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
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let travel_distance = screen_width;
        let start_pos = Coord2D::new(rest_pos.x + travel_distance, rest_pos.y);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
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
        anim_win: &mut AnimateWindow,
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
            let mut win = anim_win.window.borrow_mut();
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
        anim_win: &mut AnimateWindow,
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
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
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

/// Animation window manager for handling screen transitions (C++-accurate).
pub struct AnimateWindowManager {
    win_list: Vec<AnimateWindow>,
    win_must_finish_list: Vec<AnimateWindow>,
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

impl AnimateWindowManager {
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
        let now = Instant::now();
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
                let process: &dyn ProcessAnimateWindow = match anim_win.anim_type {
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
            let process: &dyn ProcessAnimateWindow = match anim_win.anim_type {
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
        window: Rc<RefCell<GameWindow>>,
        anim_type: AnimationType,
        needs_to_finish: bool,
        duration_ms: u64,
        delay_ms: u64,
    ) {
        if anim_type == AnimationType::None {
            log::debug!("Ignoring AnimationType::None for animate window registration");
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

    fn process_for(&self, anim_type: AnimationType) -> &dyn ProcessAnimateWindow {
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

    fn process_for_mut(&mut self, anim_type: AnimationType) -> &mut dyn ProcessAnimateWindow {
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
            let process: &dyn ProcessAnimateWindow = match anim_win.anim_type {
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
            let process: &dyn ProcessAnimateWindow = match anim_win.anim_type {
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
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(rest_pos.x, rest_pos.y);
        }
        for anim_win in &mut self.win_list {
            let rest_pos = anim_win.rest_pos;
            let mut win = anim_win.window.borrow_mut();
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

impl Default for AnimateWindowManager {
    fn default() -> Self {
        Self::new()
    }
}
