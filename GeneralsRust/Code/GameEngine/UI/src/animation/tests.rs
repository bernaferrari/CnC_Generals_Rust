use crate::{AnimateWindowManager, AnimationTarget, AnimationType};
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

// An observable geometry adapter used only by tests. The model itself adds no Rc or cell.
#[derive(Clone)]
struct Window {
    position: Rc<Cell<(i32, i32)>>,
    size: (i32, i32),
}

impl Window {
    fn new(position: (i32, i32)) -> Self {
        Self {
            position: Rc::new(Cell::new(position)),
            size: (120, 80),
        }
    }
    fn position(&self) -> (i32, i32) {
        self.position.get()
    }
}

impl AnimationTarget for Window {
    fn get_position(&self) -> (i32, i32) {
        self.position.get()
    }
    fn get_size(&self) -> (i32, i32) {
        self.size
    }
    fn set_position(&mut self, x: i32, y: i32) {
        self.position.set((x, y));
    }
}

#[test]
fn every_slide_uses_cpp_display_width_not_height() {
    for (style, expected) in [
        (AnimationType::SlideRight, (825, 40)),
        (AnimationType::SlideRightFast, (825, 40)),
        (AnimationType::SlideLeft, (-775, 40)),
        (AnimationType::SlideTop, (25, -760)),
        (AnimationType::SlideTopFast, (25, -760)),
        (AnimationType::SlideBottom, (25, 840)),
        (AnimationType::SlideBottomTimed, (25, 840)),
    ] {
        let mut model = AnimateWindowManager::new();
        model.set_screen_size(800, 600);
        let window = Window::new((25, 40));
        model.register_window(window.clone(), style, true, 100, 0);
        assert_eq!(window.position(), expected, "{style:?}");
        assert!(!model.is_finished());
    }
}

#[test]
fn velocity_styles_advance_one_step_per_update_even_at_same_timestamp() {
    let mut model = AnimateWindowManager::new();
    let window = Window::new((25, 40));
    model.register_window(window.clone(), AnimationType::SlideRight, true, 100, 0);
    let now = Instant::now() + Duration::from_secs(1);
    model.update_at(now);
    assert_eq!(window.position(), (785, 40));
    model.update_at(now);
    assert_eq!(window.position(), (745, 40));
}

#[test]
fn delayed_animation_does_not_advance_before_its_start() {
    let mut model = AnimateWindowManager::new();
    let window = Window::new((25, 40));
    model.register_window(window.clone(), AnimationType::SlideRight, true, 100, 60_000);
    model.update_at(Instant::now());
    assert_eq!(window.position(), (825, 40));
    assert!(!model.is_finished());
    model.update_at(Instant::now() + Duration::from_secs(120));
    assert_eq!(window.position(), (785, 40));
}

#[test]
fn timed_completion_and_reverse_follow_original_bottom_direction() {
    let mut model = AnimateWindowManager::new();
    let window = Window::new((25, 40));
    model.register_window(
        window.clone(),
        AnimationType::SlideBottomTimed,
        true,
        100,
        0,
    );
    model.update_at(Instant::now() + Duration::from_secs(1));
    assert_eq!(window.position(), (25, 40));
    assert!(model.is_finished());
    model.reverse_animate_window();
    assert!(model.is_reversed());
    assert!(!model.is_finished());
    model.update_at(Instant::now() + Duration::from_secs(1));
    assert_eq!(window.position(), (25, 840));
    assert!(model.is_finished());
}

#[test]
fn optional_animations_do_not_block_but_still_update() {
    let mut model = AnimateWindowManager::new();
    let window = Window::new((25, 40));
    model.register_window(window.clone(), AnimationType::SlideRight, false, 100, 0);
    assert!(model.is_finished());
    assert!(!model.is_empty());
    model.update_at(Instant::now() + Duration::from_secs(1));
    assert_eq!(window.position(), (785, 40));
}

#[test]
fn independent_models_interleave_and_reset_only_their_own_targets() {
    let mut first = AnimateWindowManager::new();
    let mut second = AnimateWindowManager::new();
    first.set_screen_size(800, 600);
    second.set_screen_size(1600, 1200);
    let a = Window::new((25, 40));
    let b = Window::new((25, 40));
    first.register_window(a.clone(), AnimationType::SlideRight, true, 100, 0);
    second.register_window(b.clone(), AnimationType::SlideRight, true, 100, 0);
    let now = Instant::now() + Duration::from_secs(1);
    first.update_at(now);
    second.update_at(now);
    assert_eq!(a.position(), (785, 40));
    assert_eq!(b.position(), (1585, 40));
    first.reset();
    assert_eq!(a.position(), (25, 40));
    assert_eq!(b.position(), (1585, 40));
    assert!(first.is_empty());
    assert!(!second.is_empty());
    second.update_at(now);
    assert_eq!(b.position(), (1545, 40));
    second.reset_to_rest_position();
    assert_eq!(b.position(), (25, 40));
}

#[test]
fn none_and_construction_are_inert() {
    let mut model = AnimateWindowManager::new();
    let window = Window::new((25, 40));
    assert!(model.is_empty());
    assert!(model.is_finished());
    assert!(!model.is_reversed());
    model.register_window(window.clone(), AnimationType::None, true, 100, 0);
    model.update_at(Instant::now() + Duration::from_secs(1));
    assert!(model.is_empty());
    assert_eq!(window.position(), (25, 40));
}

#[test]
fn spiral_observes_target_size_and_restores_the_exact_rest_position() {
    let mut model = AnimateWindowManager::new();
    let window = Window::new((25, 40));
    model.register_window(window.clone(), AnimationType::Spiral, true, 100, 0);
    assert_eq!(window.position(), (425, 40));
    let now = Instant::now() + Duration::from_secs(1);
    for _ in 0..100 {
        model.update_at(now);
    }
    assert!(model.is_finished());
    assert_eq!(window.position(), (25, 40));
}
