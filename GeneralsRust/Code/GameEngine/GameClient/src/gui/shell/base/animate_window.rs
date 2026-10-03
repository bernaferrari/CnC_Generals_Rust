// Production adapter: renderer-independent policy lives in generals_ui.
#[derive(Debug, Clone)]
struct WindowAnimationTarget(Rc<RefCell<GameWindow>>);

impl generals_ui::AnimationTarget for WindowAnimationTarget {
    fn get_position(&self) -> (i32, i32) {
        self.0.borrow().get_position()
    }

    fn get_size(&self) -> (i32, i32) {
        self.0.borrow().get_size()
    }

    fn set_position(&mut self, x: i32, y: i32) {
        let _ = self.0.borrow_mut().set_position(x, y);
    }
}

#[derive(Debug, Clone)]
pub struct AnimateWindow(generals_ui::AnimateWindow<WindowAnimationTarget>);

impl AnimateWindow {
    pub fn new(
        window: Rc<RefCell<GameWindow>>,
        anim_type: AnimationType,
        needs_to_finish: bool,
    ) -> Self {
        Self(generals_ui::AnimateWindow::new(
            WindowAnimationTarget(window),
            anim_type,
            needs_to_finish,
        ))
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
        self.0.set_anim_data(
            start_pos, end_pos, cur_pos, rest_pos, vel, start_time, end_time,
        );
    }

    pub fn set_delay(&mut self, delay_ms: u64) {
        self.0.set_delay(delay_ms);
    }
    pub fn get_delay(&self) -> u64 {
        self.0.get_delay()
    }
}

/// Existing shell window adapter; the transition state is directly owned by the UI model.
#[derive(Default)]
pub struct AnimateWindowManager {
    model: generals_ui::AnimateWindowManager<WindowAnimationTarget>,
}

impl AnimateWindowManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_screen_size(&mut self, width: i32, height: i32) {
        self.model.set_screen_size(width, height);
    }
    pub fn init(&mut self) {
        self.model.init();
    }
    pub fn reset(&mut self) {
        self.model.reset();
    }
    pub fn update(&mut self) {
        self.model.update();
    }
    pub fn update_at(&mut self, now: Instant) {
        self.model.update_at(now);
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
        }
        self.model.register_window(
            WindowAnimationTarget(window),
            anim_type,
            needs_to_finish,
            duration_ms,
            delay_ms,
        );
    }
    pub fn reverse_animate_window(&mut self) {
        self.model.reverse_animate_window();
    }
    pub fn reset_to_rest_position(&mut self) {
        self.model.reset_to_rest_position();
    }
    pub fn is_finished(&self) -> bool {
        self.model.is_finished()
    }
    pub fn is_reversed(&self) -> bool {
        self.model.is_reversed()
    }
    pub fn is_empty(&self) -> bool {
        self.model.is_empty()
    }
}
