pub use crate::display::cinematic_camera::ParabolicEase;

pub fn parabolic_ease(param: f32, ease_in_time: f32, ease_out_time: f32) -> f32 {
    let mut ease = ParabolicEase::default();
    ease.set_ease_times(ease_in_time, ease_out_time);
    ease.apply(param)
}
