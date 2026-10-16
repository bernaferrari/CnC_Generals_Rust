//! Logical view state owned by the session, independent of a window or GPU.
//! The native camera writes this target directly; FX observes the same value.
pub(crate) mod camera;
pub(crate) mod scripted;

use game_engine::common::global_data::GlobalData;
use game_engine::common::ini::ini_fx_list::CameraShakeType;
use game_engine::common::view_impulse::ViewImpulse;
use glam::Vec3;

#[derive(Debug, Default)]
pub(crate) struct SessionView {
    pub(crate) camera: camera::SessionCamera,
    pub(crate) scripted: scripted::ScriptCameraState,
    pub(crate) target: Vec3,
    pub(crate) pending_focus_previous_target: Option<Vec3>,
    pub(crate) impulse: SessionViewImpulse,
    #[cfg(feature = "game_client")]
    pub(crate) filter: game_client::display::view_filter::ViewFilterState,
}

#[derive(Debug, Default)]
pub(crate) struct SessionViewImpulse {
    pub(crate) intensity: f32,
    pub(crate) cosine: f32,
    pub(crate) sine: f32,
    pub(crate) offset: Vec3,
}

/// Detached owner value for transactional admission, never another active view.
#[derive(Debug, PartialEq)]
pub(crate) struct SessionViewState {
    pub(crate) target: Vec3,
    pub(crate) intensity: f32,
    pub(crate) cosine: f32,
    pub(crate) sine: f32,
    pub(crate) offset: Vec3,
}

impl SessionView {
    pub(crate) fn capture(&self) -> SessionViewState {
        SessionViewState {
            target: self.target,
            intensity: self.impulse.intensity,
            cosine: self.impulse.cosine,
            sine: self.impulse.sine,
            offset: self.impulse.offset,
        }
    }

    pub(crate) fn admit(&mut self, state: SessionViewState) -> Result<(), String> {
        if !state.target.is_finite()
            || !state.offset.is_finite()
            || !state.intensity.is_finite()
            || !state.cosine.is_finite()
            || !state.sine.is_finite()
        {
            return Err("invalid session view state".into());
        }
        self.target = state.target;
        self.impulse = SessionViewImpulse {
            intensity: state.intensity,
            cosine: state.cosine,
            sine: state.sine,
            offset: state.offset,
        };
        Ok(())
    }

    pub(crate) fn enqueue(
        &mut self,
        position: Vec3,
        kind: CameraShakeType,
        rules: &GlobalData,
        random: &mut dyn FnMut(f32, f32) -> f32,
    ) {
        self.impulse
            .enqueue(position, self.target, kind, rules, random);
    }

    /// Caller owns full-view admission; enqueue never commits an offset.
    pub(crate) fn advance_impulse(&mut self) -> Vec3 {
        self.impulse.step()
    }
}

impl SessionViewImpulse {
    pub(crate) fn enqueue(
        &mut self,
        position: Vec3,
        target: Vec3,
        kind: CameraShakeType,
        rules: &GlobalData,
        random: &mut dyn FnMut(f32, f32) -> f32,
    ) {
        ViewImpulse {
            intensity: &mut self.intensity,
            cosine: &mut self.cosine,
            sine: &mut self.sine,
        }
        .enqueue(
            [position.x, position.z],
            [target.x, target.z],
            kind,
            rules,
            random,
        );
    }

    pub(crate) fn step(&mut self) -> Vec3 {
        let offset = ViewImpulse {
            intensity: &mut self.intensity,
            cosine: &mut self.cosine,
            sine: &mut self.sine,
        }
        .step();
        self.offset = Vec3::new(offset[0], 0.0, offset[1]);
        self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_is_canonical_and_impulse_continuation_is_instance_owned() {
        let mut rules = GlobalData::default();
        rules.shake_normal_intensity = 2.0;
        rules.max_shake_range = 100.0;
        rules.max_shake_intensity = 10.0;
        let mut owner = SessionView::default();
        owner.target = Vec3::new(10.0, 900.0, 20.0);
        owner.enqueue(
            Vec3::new(10.0, -900.0, 20.0),
            CameraShakeType::Normal,
            &rules,
            &mut |_, _| 0.0,
        );
        assert_eq!(owner.impulse.intensity, 2.0);
        assert_eq!(owner.impulse.offset, Vec3::ZERO);
        owner.advance_impulse();
        let saved = owner.capture();
        let mut restored = SessionView::default();
        restored.admit(saved).unwrap();
        let mut peer = SessionView::default();
        peer.target = Vec3::splat(10000.0);
        peer.enqueue(Vec3::ZERO, CameraShakeType::Normal, &rules, &mut |_, _| 1.0);
        for _ in 0..8 {
            peer.advance_impulse();
            assert_eq!(owner.advance_impulse(), restored.advance_impulse());
            assert_eq!(owner.capture(), restored.capture());
        }
        owner.target = Vec3::new(1000.0, 0.0, 1000.0);
        let intensity = owner.impulse.intensity;
        owner.enqueue(Vec3::ZERO, CameraShakeType::Normal, &rules, &mut |_, _| 0.0);
        assert_eq!(
            owner.impulse.intensity, intensity,
            "FX immediately reads changed canonical target"
        );
    }
    #[test]
    fn invalid_detached_admission_leaves_receiver_unchanged() {
        let mut receiver = SessionView::default();
        receiver.target = Vec3::new(1., 2., 3.);
        let before = receiver.capture();
        let mut candidate = SessionView::default().capture();
        candidate.intensity = f32::NAN;
        assert!(receiver.admit(candidate).is_err());
        assert_eq!(receiver.capture(), before);
    }
}

#[cfg(feature = "game_client")]
mod filter_commands {
    use crate::game_logic::mission_scripts::CameraMotionBlurRequest;
    use game_client::display::{
        view::{FilterMode, FilterType, Point3},
        view_filter::ViewFilterState,
    };
    use glam::Vec3;
    #[cfg(feature = "game_client")]
    pub(crate) fn admit_motion_blur(
        filter: &mut ViewFilterState,
        request: &CameraMotionBlurRequest,
    ) -> Option<Vec3> {
        match *request {
            CameraMotionBlurRequest::Basic { zoom_in, saturate } => {
                if filter.set_filter(FilterType::MotionBlur) {
                    let mode = match (zoom_in, saturate) {
                        (true, true) => FilterMode::MBInSaturate,
                        (false, true) => FilterMode::MBOutSaturate,
                        (true, false) => FilterMode::MBInAlpha,
                        (false, false) => FilterMode::MBOutAlpha,
                    };
                    if !filter.set_mode(mode) {
                        filter.set_filter(FilterType::Null);
                    }
                }
            }
            CameraMotionBlurRequest::Jump { position, saturate } => {
                let passed = filter.set_filter(FilterType::MotionBlur)
                    && filter.set_mode(if saturate {
                        FilterMode::MBInAndOutSaturate
                    } else {
                        FilterMode::MBInAndOutAlpha
                    });
                if passed {
                    filter.set_pos(Point3::new(position.x, position.z, position.y));
                } else {
                    filter.set_filter(FilterType::Null);
                    return Some(position);
                }
            }
            CameraMotionBlurRequest::Follow { amount } => {
                filter.set_motion_blur_follow(amount);
            }
            CameraMotionBlurRequest::EndFollow => {
                filter.set_mode(FilterMode::MBEndPanAlpha);
                filter.set_filter(FilterType::MotionBlur);
            }
        }
        None
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn admit_bw(filter: &mut ViewFilterState, enabled: bool, frames: i32) {
        if enabled {
            filter.set_mode(FilterMode::BWBlackAndWhite);
            filter.set_filter(FilterType::BlackAndWhite);
            filter.set_fade_parameters(frames, 1);
        } else if filter.filter() == FilterType::BlackAndWhite {
            filter.set_fade_parameters(frames, -1);
        }
    }
}

#[cfg(feature = "game_client")]
impl SessionView {
    pub(crate) fn apply_bw(&mut self, enabled: bool, frames: i32) {
        filter_commands::admit_bw(&mut self.filter, enabled, frames);
    }
    pub(crate) fn apply_motion_blur(
        &mut self,
        request: &crate::game_logic::mission_scripts::CameraMotionBlurRequest,
    ) -> Option<Vec3> {
        filter_commands::admit_motion_blur(&mut self.filter, request)
    }
}
