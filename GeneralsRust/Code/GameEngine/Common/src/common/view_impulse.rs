//! Borrowed W3DView impulse state; enqueue and admitted update are separate.
//! C++ W3DView.cpp:3224-3280 and 1273-1292.
use crate::common::global_data::GlobalData;
use crate::common::ini::ini_fx_list::CameraShakeType;

/// Borrows the existing owner fields. No residual or clock is stored here.
pub struct ViewImpulse<'a> {
    pub intensity: &'a mut f32,
    pub cosine: &'a mut f32,
    pub sine: &'a mut f32,
}

impl ViewImpulse<'_> {
    /// Apply an impulse at its original FX nugget position. Even an out-of-range
    /// event consumes its angle and replaces direction before returning.
    pub fn enqueue(
        &mut self,
        epicenter: [f32; 2],
        view_position: [f32; 2],
        kind: CameraShakeType,
        rules: &GlobalData,
        random: &mut dyn FnMut(f32, f32) -> f32,
    ) {
        self.enqueue_with_rules(
            epicenter,
            view_position,
            kind,
            &crate::common::ini::ini_game_data::CameraShakeRules::from_global_data(rules),
            random,
        );
    }

    /// Same source algorithm over explicitly admitted immutable definitions.
    pub fn enqueue_with_rules(
        &mut self,
        epicenter: [f32; 2],
        view_position: [f32; 2],
        kind: CameraShakeType,
        rules: &crate::common::ini::ini_game_data::CameraShakeRules,
        random: &mut dyn FnMut(f32, f32) -> f32,
    ) {
        let angle = random(0.0, 2.0 * std::f32::consts::PI);
        // Retain the Real overloads verified by the extracted C++ component
        // under the supported clang/math.h profile; no speculative promotion.
        *self.cosine = angle.cos();
        *self.sine = angle.sin();
        let mut intensity = match kind {
            CameraShakeType::Subtle => rules.shake_subtle_intensity,
            CameraShakeType::Normal => rules.shake_normal_intensity,
            CameraShakeType::Strong => rules.shake_strong_intensity,
            CameraShakeType::Severe => rules.shake_severe_intensity,
            CameraShakeType::CineExtreme => rules.shake_cine_extreme_intensity,
            CameraShakeType::CineInsane => rules.shake_cine_insane_intensity,
        };
        let dx = epicenter[0] - view_position[0];
        let dy = epicenter[1] - view_position[1];
        let distance = (dx * dx + dy * dy).sqrt();
        if distance > rules.max_shake_range {
            return;
        }
        intensity *= 1.0 - distance / rules.max_shake_range;
        *self.intensity += intensity;
        if *self.intensity > rules.max_shake_intensity {
            *self.intensity = 3.0;
        }
    }

    /// Commit one full View update. The quiet branch leaves direction intact.
    pub fn step(&mut self) -> [f32; 2] {
        if *self.intensity > 0.01 {
            let offset = [*self.intensity * *self.cosine, *self.intensity * *self.sine];
            *self.intensity *= 0.75;
            *self.cosine = -*self.cosine;
            *self.sine = -*self.sine;
            offset
        } else {
            *self.intensity = 0.0;
            [0.0; 2]
        }
    }
}

#[cfg(test)]
#[path = "view_impulse_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
#[path = "view_impulse_cpp_tests.rs"]
mod cpp_tests;
