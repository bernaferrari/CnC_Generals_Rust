use glam::Mat4;
use ww3d_animation::{
    AnimatedModel, HAnimClass, HTreeClass, SkeletonState,
    AnimationMode as W3DAnimMode,
};

pub struct W3DSkeletonState {
    inner: SkeletonState,
}

impl W3DSkeletonState {
    pub fn new(htree: HTreeClass) -> Self {
        Self {
            inner: SkeletonState::new(htree),
        }
    }

    pub fn skinning_matrices_flat(&self) -> Vec<f32> {
        self.inner.get_skinning_matrices_flat()
    }

    pub fn bone_count(&self) -> usize {
        self.inner.bone_count()
    }

    pub fn get_bone_transform(&self, bone_idx: usize) -> Option<Mat4> {
        self.inner.get_bone_transform(bone_idx)
    }

    pub fn inner(&self) -> &SkeletonState {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut SkeletonState {
        &mut self.inner
    }
}

pub struct W3DAnimatedModel {
    inner: AnimatedModel,
}

impl W3DAnimatedModel {
    pub fn new(htree: HTreeClass) -> Self {
        Self {
            inner: AnimatedModel::new(htree),
        }
    }

    pub fn set_animation(&mut self, animation: HAnimClass) {
        self.inner.set_animation(animation);
    }

    pub fn transition_to(&mut self, animation: HAnimClass, blend_duration: f32) {
        self.inner.transition_to(animation, blend_duration);
    }

    pub fn update(&mut self, delta_time: f32, root_transform: Mat4) {
        self.inner.update(delta_time, root_transform);
    }

    pub fn skinning_matrices_flat(&self) -> Vec<f32> {
        self.inner.get_skinning_matrices_flat()
    }

    pub fn skinning_matrices(&self) -> Vec<Mat4> {
        self.inner.get_skinning_matrices()
    }

    pub fn set_animation_mode(&mut self, mode: W3DAnimMode) {
        self.inner.set_animation_mode(mode);
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.inner.set_speed(speed);
    }

    pub fn pause(&mut self) {
        self.inner.pause();
    }

    pub fn resume(&mut self) {
        self.inner.resume();
    }

    pub fn is_playing(&self) -> bool {
        self.inner.is_playing()
    }

    pub fn current_frame(&self) -> f32 {
        self.inner.get_current_frame()
    }

    pub fn set_frame(&mut self, frame: f32) {
        self.inner.set_frame(frame);
    }

    pub fn skeleton(&self) -> &SkeletonState {
        &self.inner.skeleton
    }

    pub fn skeleton_mut(&mut self) -> &mut SkeletonState {
        &mut self.inner.skeleton
    }

    pub fn inner(&self) -> &AnimatedModel {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut AnimatedModel {
        &mut self.inner
    }
}

pub struct W3DAnimationController {
    model: Option<W3DAnimatedModel>,
    htree: Option<HTreeClass>,
}

impl W3DAnimationController {
    pub fn new() -> Self {
        Self {
            model: None,
            htree: None,
        }
    }

    pub fn set_hierarchy(&mut self, htree: HTreeClass) {
        self.model = Some(W3DAnimatedModel::new(htree));
    }

    pub fn set_animation(&mut self, animation: HAnimClass) {
        if let Some(ref mut model) = self.model {
            model.set_animation(animation);
        }
    }

    pub fn transition_to(&mut self, animation: HAnimClass, blend_duration: f32) {
        if let Some(ref mut model) = self.model {
            model.transition_to(animation, blend_duration);
        }
    }

    pub fn update(&mut self, delta_time: f32, root_transform: Mat4) {
        if let Some(ref mut model) = self.model {
            model.update(delta_time, root_transform);
        }
    }

    pub fn skinning_matrices_flat(&self) -> Vec<f32> {
        self.model
            .as_ref()
            .map(|m| m.skinning_matrices_flat())
            .unwrap_or_default()
    }

    pub fn bone_transform(&self, bone_idx: usize) -> Option<Mat4> {
        self.model
            .as_ref()
            .and_then(|m| m.skeleton().get_bone_transform(bone_idx))
    }

    pub fn bone_count(&self) -> usize {
        self.model
            .as_ref()
            .map(|m| m.skeleton().bone_count())
            .unwrap_or(0)
    }

    pub fn has_model(&self) -> bool {
        self.model.is_some()
    }
}

impl Default for W3DAnimationController {
    fn default() -> Self {
        Self::new()
    }
}
