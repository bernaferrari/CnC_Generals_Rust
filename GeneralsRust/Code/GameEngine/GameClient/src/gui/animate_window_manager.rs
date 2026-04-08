use std::cell::RefCell;
use std::rc::Rc;

pub use crate::gui::{AnimateWindowManager, AnimationType, GameWindow};

pub fn init(manager: &mut AnimateWindowManager) {
    manager.init();
}

pub fn reset(manager: &mut AnimateWindowManager) {
    manager.reset();
}

pub fn update(manager: &mut AnimateWindowManager) {
    manager.update();
}

pub fn register_game_window(
    manager: &mut AnimateWindowManager,
    window: Rc<RefCell<GameWindow>>,
    animation_type: AnimationType,
    needs_to_finish: bool,
    duration_ms: u64,
    delay_ms: u64,
) {
    manager.register_window(
        window,
        animation_type,
        needs_to_finish,
        duration_ms,
        delay_ms,
    );
}

pub fn reverse_animate_window(manager: &mut AnimateWindowManager) {
    manager.reverse_animate_window();
}

pub fn reset_to_rest_position(manager: &mut AnimateWindowManager) {
    manager.reset_to_rest_position();
}
