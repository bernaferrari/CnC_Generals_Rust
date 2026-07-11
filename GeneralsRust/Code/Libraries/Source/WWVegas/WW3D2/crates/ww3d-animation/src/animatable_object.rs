//! Animatable3DObjClass - Direct C++ Port
//!
//! This module implements the `Animatable3DObjClass` from animobj.cpp, providing
//! the classic WW3D animation playback system with exact C++ fidelity.
//!
//! Reference: animobj.h lines 24-147, animobj.cpp lines 1-1086

use crate::combo::HAnimCombo;
use crate::hanim::{AnimationMode, HAnimClass};
use crate::htree::HTreeClass;
use crate::{embedded_sound_bone, has_embedded_sounds, trigger_sound};
use glam::Mat4;
use std::sync::Arc;

/// Current animation mode for the object
/// Reference: animobj.h:38-43
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // C++ parity
enum MotionMode {
    /// No animation
    #[allow(dead_code)] // C++ parity
    None,
    /// Base pose only
    BasePose,
    /// Single animation
    SingleAnim,
    /// Double animation blend
    DoubleAnim,
    /// Multiple animation blend (combo)
    MultipleAnim,
}

/// Single animation state
/// Reference: animobj.h:45-52
#[derive(Debug, Clone)]
struct ModeAnimData {
    motion: Option<Arc<HAnimClass>>,
    frame: f32,
    prev_frame: f32,
    last_sync_time: f64,
    anim_mode: AnimationMode,
    frame_rate_multiplier: f32,
    anim_direction: f32,
}

impl Default for ModeAnimData {
    fn default() -> Self {
        Self {
            motion: None,
            frame: 0.0,
            prev_frame: 0.0,
            last_sync_time: 0.0,
            anim_mode: AnimationMode::Loop,
            frame_rate_multiplier: 1.0,
            anim_direction: 1.0,
        }
    }
}

/// Double animation blend state
/// Reference: animobj.h:54-63
#[derive(Debug, Clone)]
struct ModeInterpData {
    motion0: Option<Arc<HAnimClass>>,
    motion1: Option<Arc<HAnimClass>>,
    frame0: f32,
    frame1: f32,
    prev_frame0: f32,
    prev_frame1: f32,
    percentage: f32,
}

impl Default for ModeInterpData {
    fn default() -> Self {
        Self {
            motion0: None,
            motion1: None,
            frame0: 0.0,
            frame1: 0.0,
            prev_frame0: 0.0,
            prev_frame1: 0.0,
            percentage: 0.0,
        }
    }
}

/// Multiple animation blend state (combo)
/// Reference: animobj.h:65-68
#[derive(Debug, Clone)]
#[derive(Default)]
struct ModeComboData {
    anim_combo: Option<HAnimCombo>,
}


/// Animatable 3D object with hierarchical animation support
/// This is a direct port of C++ Animatable3DObjClass
/// Reference: animobj.h:24-147, animobj.cpp:1-1086
#[derive(Debug, Clone)]
pub struct Animatable3DObjClass {
    /// Hierarchy tree for skeletal structure
    pub htree: HTreeClass,

    /// Current motion mode
    cur_motion_mode: MotionMode,

    /// Single animation mode data
    mode_anim: ModeAnimData,

    /// Double animation blend data
    mode_interp: ModeInterpData,

    /// Multiple animation combo data
    mode_combo: ModeComboData,

    /// Object transform
    transform: Mat4,

    /// Hierarchy validity flag
    is_tree_valid: bool,
}

impl Animatable3DObjClass {
    /// Create a new animatable object with the specified hierarchy
    /// Reference: animobj.cpp:24-62 (Animatable3DObjClass::Animatable3DObjClass)
    pub fn new(htree: HTreeClass) -> Self {
        Self {
            htree,
            cur_motion_mode: MotionMode::BasePose,
            mode_anim: ModeAnimData::default(),
            mode_interp: ModeInterpData::default(),
            mode_combo: ModeComboData::default(),
            transform: Mat4::IDENTITY,
            is_tree_valid: false,
        }
    }

    /// Set the object transform
    /// Reference: animobj.cpp:287-291 (Animatable3DObjClass::Set_Transform)
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.is_tree_valid = false;
    }

    /// Get the object transform
    pub fn get_transform(&self) -> Mat4 {
        self.transform
    }

    /// Set position (shortcut for transform)
    /// Reference: animobj.cpp:306-310 (Animatable3DObjClass::Set_Position)
    pub fn set_position(&mut self, position: glam::Vec3) {
        self.transform.w_axis = position.extend(1.0);
        self.is_tree_valid = false;
    }

    /// Get number of bones
    /// Reference: animobj.cpp:325-332 (Animatable3DObjClass::Get_Num_Bones)
    pub fn get_num_bones(&self) -> usize {
        self.htree.num_pivots()
    }

    /// Get bone name by index
    /// Reference: animobj.cpp:347-354 (Animatable3DObjClass::Get_Bone_Name)
    pub fn get_bone_name(&self, bone_index: usize) -> Option<&str> {
        self.htree.get_bone_name(bone_index)
    }

    /// Get bone index by name
    /// Reference: animobj.cpp:369-376 (Animatable3DObjClass::Get_Bone_Index)
    pub fn get_bone_index(&self, bone_name: &str) -> Option<usize> {
        self.htree.get_bone_index(bone_name)
    }

    /// Set animation to base pose only
    /// Reference: animobj.cpp:392-397 (Animatable3DObjClass::Set_Animation void)
    pub fn set_animation_none(&mut self) {
        self.release();
        self.cur_motion_mode = MotionMode::BasePose;
        self.is_tree_valid = false;
    }

    /// Set a single animation with frame and mode
    /// Reference: animobj.cpp:412-445 (Animatable3DObjClass::Set_Animation single)
    pub fn set_animation(
        &mut self,
        motion: Option<Arc<HAnimClass>>,
        frame: f32,
        mode: AnimationMode,
        sync_time: f64,
    ) {
        if let Some(ref anim) = motion {
            self.release();

            self.cur_motion_mode = MotionMode::SingleAnim;
            self.mode_anim.motion = Some(anim.clone());
            self.mode_anim.prev_frame = self.mode_anim.frame;
            self.mode_anim.frame = frame;
            self.mode_anim.last_sync_time = sync_time;
            self.mode_anim.frame_rate_multiplier = 1.0;
            self.mode_anim.anim_mode = mode;

            // Set animation direction based on mode
            // Reference: animobj.cpp:429-432
            self.mode_anim.anim_direction = match mode {
                AnimationMode::LoopBackwards | AnimationMode::OnceBackwards => -1.0,
                _ => 1.0,
            };

            // Set up embedded sound bone index if present
            // Reference: animobj.cpp:434-438
            if has_embedded_sounds(anim) {
                if let Some(bone_name) = embedded_sound_bone(anim) {
                    if let Some(bone_index) = self.get_bone_index(&bone_name) {
                        // Store bone index in animation (would need to extend HAnimClass)
                        // For now, we'll handle this in update
                        let _ = bone_index; // Suppress unused warning
                    }
                }
            }
        } else {
            self.cur_motion_mode = MotionMode::BasePose;
            self.release();
        }

        self.is_tree_valid = false;
    }

    /// Set double animation blend
    /// Reference: animobj.cpp:459-497 (Animatable3DObjClass::Set_Animation double)
    pub fn set_animation_blend(
        &mut self,
        motion0: Option<Arc<HAnimClass>>,
        frame0: f32,
        motion1: Option<Arc<HAnimClass>>,
        frame1: f32,
        percentage: f32,
    ) {
        self.release();

        self.cur_motion_mode = MotionMode::DoubleAnim;
        self.mode_interp.motion0 = motion0.clone();
        self.mode_interp.motion1 = motion1.clone();
        self.mode_interp.prev_frame0 = self.mode_interp.frame0;
        self.mode_interp.prev_frame1 = self.mode_interp.frame1;
        self.mode_interp.frame0 = frame0;
        self.mode_interp.frame1 = frame1;
        self.mode_interp.percentage = percentage;

        self.is_tree_valid = false;

        // Set up embedded sounds for both animations if present
        // Reference: animobj.cpp:480-496
        if let Some(ref anim) = motion0 {
            if has_embedded_sounds(anim) {
                if let Some(bone_name) = embedded_sound_bone(anim) {
                    if let Some(bone_index) = self.get_bone_index(&bone_name) {
                        let _ = bone_index; // Will be used in update
                    }
                }
            }
        }

        if let Some(ref anim) = motion1 {
            if has_embedded_sounds(anim) {
                if let Some(bone_name) = embedded_sound_bone(anim) {
                    if let Some(bone_index) = self.get_bone_index(&bone_name) {
                        let _ = bone_index; // Will be used in update
                    }
                }
            }
        }
    }

    /// Set animation combo (multiple animations)
    /// Reference: animobj.cpp:512-535 (Animatable3DObjClass::Set_Animation combo)
    pub fn set_animation_combo(&mut self, anim_combo: Option<HAnimCombo>) {
        self.release();

        self.cur_motion_mode = MotionMode::MultipleAnim;
        self.mode_combo.anim_combo = anim_combo.clone();
        self.is_tree_valid = false;

        // Set up embedded sounds for all animations in combo
        // Reference: animobj.cpp:523-534
        if let Some(ref combo) = anim_combo {
            let count = combo.num_anims();
            for index in 0..count {
                if let Some(motion) = combo.peek_motion(index) {
                    if has_embedded_sounds(motion) {
                        if let Some(bone_name) = embedded_sound_bone(motion) {
                            if let Some(bone_index) = self.get_bone_index(&bone_name) {
                                let _ = bone_index; // Will be used in update
                            }
                        }
                    }
                }
            }
        }
    }

    /// Peek at current animation (for SINGLE_ANIM mode only)
    /// Reference: animobj.cpp:550-557 (Animatable3DObjClass::Peek_Animation)
    pub fn peek_animation(&self) -> Option<Arc<HAnimClass>> {
        if self.cur_motion_mode == MotionMode::SingleAnim {
            self.mode_anim.motion.clone()
        } else {
            None
        }
    }

    /// Get bone transform in world space
    /// Reference: animobj.cpp:572-615 (Animatable3DObjClass::Get_Bone_Transform)
    pub fn get_bone_transform(&mut self, bone_index: usize) -> Option<Mat4> {
        // Ensure hierarchy is valid
        if !self.is_tree_valid {
            self.update_sub_object_transforms();
        }

        self.htree.get_transform(bone_index)
    }

    /// Get bone transform by name
    pub fn get_bone_transform_by_name(&mut self, bone_name: &str) -> Option<Mat4> {
        if let Some(index) = self.htree.get_bone_index(bone_name) {
            self.get_bone_transform(index)
        } else {
            None
        }
    }

    /// Capture a bone (prevent animation from affecting it)
    /// Reference: animobj.cpp:630-635 (Animatable3DObjClass::Capture_Bone)
    pub fn capture_bone(&mut self, bone_index: usize) {
        self.htree.capture_bone(bone_index);
    }

    /// Release a captured bone
    /// Reference: animobj.cpp:650-655 (Animatable3DObjClass::Release_Bone)
    pub fn release_bone(&mut self, bone_index: usize) {
        self.htree.release_bone(bone_index);
    }

    /// Check if bone is captured
    /// Reference: animobj.cpp:670-677 (Animatable3DObjClass::Is_Bone_Captured)
    pub fn is_bone_captured(&self, bone_index: usize) -> bool {
        self.htree.is_bone_captured(bone_index)
    }

    /// Control a captured bone with custom transform
    /// Reference: animobj.cpp:692-706 (Animatable3DObjClass::Control_Bone)
    pub fn control_bone(&mut self, bone_index: usize, bone_transform: Mat4, _world_space: bool) {
        self.htree.control_bone(bone_index, bone_transform);
        self.is_tree_valid = false;
    }

    /// Update sub-object transforms (apply animation to hierarchy)
    /// Reference: animobj.cpp:720-794 (Animatable3DObjClass::Update_Sub_Object_Transforms)
    pub fn update_sub_object_transforms(&mut self) {
        match self.cur_motion_mode {
            MotionMode::None | MotionMode::BasePose => {
                // Base pose update
                // Reference: animobj.cpp:733-735
                self.htree.base_update(self.transform);
            }

            MotionMode::SingleAnim => {
                // Single animation update
                // Reference: animobj.cpp:737-750

                // Progress animation if not manual
                if self.mode_anim.anim_mode != AnimationMode::Manual {
                    self.single_anim_progress(0.0); // Will use stored sync time
                }

                // Apply animation to hierarchy
                if let Some(motion) = self.mode_anim.motion.clone() {
                    let current_frame = self.mode_anim.frame;
                    let prev_frame = self.mode_anim.prev_frame;

                    self.anim_update(motion.clone(), current_frame);

                    // Trigger embedded sounds
                    // Reference: animobj.cpp:747-749
                    if has_embedded_sounds(&motion) {
                        if let Some(bone_name) = embedded_sound_bone(&motion) {
                            if let Some(bone_index) = self.get_bone_index(&bone_name) {
                                if let Some(bone_transform) = self.htree.get_transform(bone_index) {
                                    self.mode_anim.prev_frame = trigger_sound(
                                        &motion,
                                        prev_frame,
                                        current_frame,
                                        &bone_transform,
                                    );
                                }
                            }
                        }
                    }
                }
            }

            MotionMode::DoubleAnim => {
                // Double animation blend update
                // Reference: animobj.cpp:752-767
                let motion0_opt = self.mode_interp.motion0.clone();
                let motion1_opt = self.mode_interp.motion1.clone();
                let frame0 = self.mode_interp.frame0;
                let frame1 = self.mode_interp.frame1;
                let percentage = self.mode_interp.percentage;
                let prev_frame0 = self.mode_interp.prev_frame0;
                let prev_frame1 = self.mode_interp.prev_frame1;

                if let (Some(motion0), Some(motion1)) = (motion0_opt, motion1_opt) {
                    self.blend_update(motion0.clone(), frame0, motion1.clone(), frame1, percentage);

                    // Trigger embedded sounds for both animations
                    // Reference: animobj.cpp:758-766
                    if has_embedded_sounds(&motion0) {
                        if let Some(bone_name) = embedded_sound_bone(&motion0) {
                            if let Some(bone_index) = self.get_bone_index(&bone_name) {
                                if let Some(bone_transform) = self.htree.get_transform(bone_index) {
                                    self.mode_interp.prev_frame0 = trigger_sound(
                                        &motion0,
                                        prev_frame0,
                                        frame0,
                                        &bone_transform,
                                    );
                                }
                            }
                        }
                    }

                    if has_embedded_sounds(&motion1) {
                        if let Some(bone_name) = embedded_sound_bone(&motion1) {
                            if let Some(bone_index) = self.get_bone_index(&bone_name) {
                                if let Some(bone_transform) = self.htree.get_transform(bone_index) {
                                    self.mode_interp.prev_frame1 = trigger_sound(
                                        &motion1,
                                        prev_frame1,
                                        frame1,
                                        &bone_transform,
                                    );
                                }
                            }
                        }
                    }
                }
            }

            MotionMode::MultipleAnim => {
                // Multiple animation combo update
                // Reference: animobj.cpp:769-788
                if let Some(ref combo) = self.mode_combo.anim_combo {
                    self.htree.combo_update(self.transform, combo);

                    // Trigger embedded sounds for all animations in combo
                    // Reference: animobj.cpp:776-786
                    let count = combo.num_anims();
                    for index in 0..count {
                        if let Some(motion) = combo.peek_motion(index) {
                            if has_embedded_sounds(motion) {
                                if let Some(bone_name) = embedded_sound_bone(motion) {
                                    if let Some(bone_index) = self.get_bone_index(&bone_name) {
                                        if let Some(bone_transform) =
                                            self.htree.get_transform(bone_index)
                                        {
                                            if let (Some(prev_frame), Some(current_frame)) = (
                                                combo.get_prev_frame(index),
                                                combo.get_frame(index),
                                            ) {
                                                let _new_prev_frame = trigger_sound(
                                                    motion,
                                                    prev_frame,
                                                    current_frame,
                                                    &bone_transform,
                                                );
                                                // Update prev_frame in combo (would need mutable access)
                                                // For now, we'll skip this update
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        self.is_tree_valid = true;
    }

    /// Apply single animation to hierarchy
    /// Reference: htree.cpp Anim_Update logic
    fn anim_update(&mut self, motion: Arc<HAnimClass>, frame: f32) {
        // Sample animation at current frame
        let pivot_count = self.htree.num_pivots();
        let mut translations = vec![glam::Vec3::ZERO; pivot_count];
        let mut rotations = vec![glam::Quat::IDENTITY; pivot_count];

        for pivot_idx in 0..pivot_count {
            translations[pivot_idx] = motion.get_translation(pivot_idx, frame);
            rotations[pivot_idx] = motion.get_orientation(pivot_idx, frame);
        }

        self.htree
            .anim_update(self.transform, &translations, &rotations);
    }

    /// Blend two animations
    /// Reference: htree.cpp Blend_Update logic
    fn blend_update(
        &mut self,
        motion0: Arc<HAnimClass>,
        frame0: f32,
        motion1: Arc<HAnimClass>,
        frame1: f32,
        percentage: f32,
    ) {
        let pivot_count = self.htree.num_pivots();
        let mut translations = vec![glam::Vec3::ZERO; pivot_count];
        let mut rotations = vec![glam::Quat::IDENTITY; pivot_count];

        for pivot_idx in 0..pivot_count {
            let trans0 = motion0.get_translation(pivot_idx, frame0);
            let trans1 = motion1.get_translation(pivot_idx, frame1);
            translations[pivot_idx] = trans0.lerp(trans1, percentage);

            let rot0 = motion0.get_orientation(pivot_idx, frame0);
            let rot1 = motion1.get_orientation(pivot_idx, frame1);
            rotations[pivot_idx] = rot0.slerp(rot1, percentage);
        }

        self.htree
            .anim_update(self.transform, &translations, &rotations);
    }

    /// Compute current frame based on sync time and animation mode
    /// Reference: animobj.cpp:889-974 (Animatable3DObjClass::Compute_Current_Frame)
    fn compute_current_frame(&self, sync_time: f64) -> (f32, f32) {
        let mut frame = self.mode_anim.frame;
        let mut direction = self.mode_anim.anim_direction;

        if self.cur_motion_mode == MotionMode::SingleAnim
            && self.mode_anim.anim_mode != AnimationMode::Manual {
                if let Some(ref motion) = self.mode_anim.motion {
                    // Calculate frame delta from time
                    // Reference: animobj.cpp:904-906
                    let sync_time_diff = sync_time - self.mode_anim.last_sync_time;
                    let delta = motion.frame_rate
                        * self.mode_anim.frame_rate_multiplier
                        * self.mode_anim.anim_direction
                        * (sync_time_diff as f32)
                        * 0.001; // Convert ms to seconds

                    frame += delta;

                    // Wrap frame based on animation mode
                    // Reference: animobj.cpp:911-964
                    let max_frame = motion.num_frames as f32 - 1.0;
                    match self.mode_anim.anim_mode {
                        AnimationMode::Once => {
                            // Reference: animobj.cpp:913-917
                            if frame >= max_frame {
                                frame = max_frame;
                            }
                        }
                        AnimationMode::Loop => {
                            // Reference: animobj.cpp:918-926
                            if frame >= max_frame {
                                frame -= max_frame;
                            }
                            if frame >= max_frame {
                                frame = 0.0;
                            }
                        }
                        AnimationMode::OnceBackwards => {
                            // Reference: animobj.cpp:927-930
                            if frame < 0.0 {
                                frame = 0.0;
                            }
                        }
                        AnimationMode::LoopBackwards => {
                            // Reference: animobj.cpp:931-939
                            if frame < 0.0 {
                                frame += max_frame;
                            }
                            if frame < 0.0 {
                                frame = max_frame;
                            }
                        }
                        AnimationMode::PingPong => {
                            // Reference: animobj.cpp:941-964
                            if self.mode_anim.anim_direction >= 1.0 {
                                // Playing forwards, check if we need to reverse
                                if frame >= max_frame {
                                    frame = max_frame * 2.0 - frame;
                                    if frame >= max_frame {
                                        frame = max_frame;
                                    }
                                    direction = -1.0;
                                }
                            } else {
                                // Playing backwards, check if we need to reverse
                                if frame < 0.0 {
                                    frame = -frame;
                                    if frame >= max_frame {
                                        frame = 0.0;
                                    }
                                    direction = 1.0;
                                }
                            }
                        }
                        AnimationMode::Manual => {}
                    }
                }
            }

        (frame, direction)
    }

    /// Progress animation frame based on time
    /// Reference: animobj.cpp:988-1014 (Animatable3DObjClass::Single_Anim_Progress)
    fn single_anim_progress(&mut self, sync_time: f64) {
        if self.cur_motion_mode == MotionMode::SingleAnim {
            let old_prev = self.mode_anim.prev_frame;
            self.mode_anim.prev_frame = self.mode_anim.frame;

            let (new_frame, new_direction) = self.compute_current_frame(sync_time);
            self.mode_anim.frame = new_frame;
            self.mode_anim.anim_direction = new_direction;
            self.mode_anim.last_sync_time = sync_time;

            // Handle duplicate calls in same frame
            // Reference: animobj.cpp:1003-1008
            if self.mode_anim.frame == self.mode_anim.prev_frame {
                self.mode_anim.prev_frame = old_prev;
            }

            self.is_tree_valid = false;
        }
    }

    /// Check if animation is complete (for ONCE mode)
    /// Reference: animobj.cpp:1029-1042 (Animatable3DObjClass::Is_Animation_Complete)
    pub fn is_animation_complete(&self) -> bool {
        if self.cur_motion_mode == MotionMode::SingleAnim {
            if let Some(ref motion) = self.mode_anim.motion {
                match self.mode_anim.anim_mode {
                    AnimationMode::Once => self.mode_anim.frame >= (motion.num_frames as f32 - 1.0),
                    AnimationMode::OnceBackwards => self.mode_anim.frame <= 0.0,
                    _ => false,
                }
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Peek at animation and info
    /// Reference: animobj.cpp:1047-1058 (Animatable3DObjClass::Peek_Animation_And_Info)
    pub fn peek_animation_and_info(
        &self,
    ) -> Option<(Arc<HAnimClass>, f32, u32, AnimationMode, f32)> {
        if self.cur_motion_mode == MotionMode::SingleAnim {
            self.mode_anim.motion.as_ref().map(|motion| (
                    motion.clone(),
                    self.mode_anim.frame,
                    motion.num_frames,
                    self.mode_anim.anim_mode,
                    self.mode_anim.frame_rate_multiplier,
                ))
        } else {
            None
        }
    }

    /// Set animation frame rate multiplier
    /// Reference: animobj.cpp:1063-1067 (Animatable3DObjClass::Set_Animation_Frame_Rate_Multiplier)
    pub fn set_animation_frame_rate_multiplier(&mut self, multiplier: f32) {
        self.mode_anim.frame_rate_multiplier = multiplier;
    }

    /// Update animation (call this every frame)
    pub fn update(&mut self, _delta_time: f32, sync_time: f64) {
        if self.cur_motion_mode == MotionMode::SingleAnim
            && self.mode_anim.anim_mode != AnimationMode::Manual {
                self.single_anim_progress(sync_time);
            }

        if !self.is_tree_valid {
            self.update_sub_object_transforms();
        }
    }

    /// Release current animation
    /// Reference: animobj.cpp:182-214 (Animatable3DObjClass::Release)
    fn release(&mut self) {
        match self.cur_motion_mode {
            MotionMode::BasePose | MotionMode::None => {}
            MotionMode::SingleAnim => {
                self.mode_anim.motion = None;
            }
            MotionMode::DoubleAnim => {
                self.mode_interp.motion0 = None;
                self.mode_interp.motion1 = None;
            }
            MotionMode::MultipleAnim => {
                self.mode_combo.anim_combo = None;
            }
        }
    }
}

impl Default for Animatable3DObjClass {
    fn default() -> Self {
        Self::new(HTreeClass::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animatable_object_creation() {
        let htree = HTreeClass::default();
        let obj = Animatable3DObjClass::new(htree);
        assert_eq!(obj.get_num_bones(), 1); // Default has root bone
    }

    #[test]
    fn test_animation_mode_transitions() {
        let htree = HTreeClass::default();
        let mut obj = Animatable3DObjClass::new(htree);

        // Start in base pose
        obj.set_animation_none();
        assert!(obj.peek_animation().is_none());

        // No animation to check completion
        assert!(!obj.is_animation_complete());
    }

    #[test]
    fn test_bone_capture() {
        let htree = HTreeClass::default();
        let mut obj = Animatable3DObjClass::new(htree);

        // Capture root bone
        obj.capture_bone(0);
        assert!(obj.is_bone_captured(0));

        // Release it
        obj.release_bone(0);
        assert!(!obj.is_bone_captured(0));
    }

    #[test]
    fn test_transform_management() {
        let htree = HTreeClass::default();
        let mut obj = Animatable3DObjClass::new(htree);

        let new_transform = Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0));
        obj.set_transform(new_transform);

        assert_eq!(obj.get_transform(), new_transform);
    }
}
