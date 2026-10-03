use super::*;

pub(super) struct ProcessAnimateWindowNoOp;

impl<T: AnimationTarget> ProcessAnimateWindow<T> for ProcessAnimateWindowNoOp {
    fn init_animate_window(&self, _anim_win: &mut AnimateWindow<T>, _screen_size: (i32, i32)) {}
    fn init_reverse_animate_window(
        &self,
        _anim_win: &mut AnimateWindow<T>,
        _max_delay_ms: u64,
        _screen_size: (i32, i32),
    ) {
    }
    fn update_animate_window(
        &self,
        _anim_win: &mut AnimateWindow<T>,
        _now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        true
    }
    fn reverse_animate_window(
        &self,
        _anim_win: &mut AnimateWindow<T>,
        _now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        true
    }
}
