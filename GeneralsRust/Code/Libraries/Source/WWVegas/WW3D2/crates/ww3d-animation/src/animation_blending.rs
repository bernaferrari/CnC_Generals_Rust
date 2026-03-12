//! Animation Blending System
//!
//! This module implements the sophisticated animation blending and state
//! machine system that was a major feature of the original C++ WW3D2.
//! It allows smooth transitions between different animations and complex
//! animation state management.

use crate::combo::{HAnimCombo, HAnimComboData};
use crate::manager::HAnimManager;
use crate::{
    embedded_sound_bone, has_embedded_sounds, trigger_sound, AnimationChannel, HAnimClass,
    HTreeClass,
};
use glam::Mat4;
#[cfg(feature = "renderer-3d")]
use log::warn;
#[cfg(feature = "renderer-3d")]
use ww3d_renderer_3d::rendering::mesh_system::MeshClass;
#[cfg(not(feature = "renderer-3d"))]
#[derive(Debug)]
pub struct MeshClass;

#[allow(dead_code)]
const MAX_SKINNING_MATRICES: usize = 64;
use std::collections::HashMap;
use std::sync::Arc;

/// Animation blend mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendMode {
    /// Override previous animation completely
    Override,
    /// Additive blending
    Additive,
    /// Multiplicative blending
    Multiplicative,
    /// Lerp between animations
    Lerp,
}

/// Animation layer for blending multiple animations
#[derive(Debug, Clone)]
pub struct AnimationLayer {
    pub name: String,
    pub animation: Option<Arc<HAnimClass>>,
    pub weight: f32,
    pub blend_mode: BlendMode,
    pub playback_rate: f32,
    pub current_time: f32,
    pub loop_animation: bool,
    pub is_active: bool,
    pub has_sound_data: bool,
    pub sound_bone_name: Option<String>,
    pub sound_bone_index: Option<usize>,
    pub prev_sound_frame: f32,
}

/// Animation state in a state machine
#[derive(Debug, Clone)]
pub struct AnimationState {
    pub name: String,
    pub animation_name: String,
    pub playback_rate: f32,
    pub loop_animation: bool,
    pub transitions: Vec<AnimationTransition>,
}

/// Transition between animation states
#[derive(Debug, Clone)]
pub struct AnimationTransition {
    pub target_state: String,
    pub condition: TransitionCondition,
    pub blend_time: f32,
}

/// Conditions for state transitions
#[derive(Debug, Clone)]
pub enum TransitionCondition {
    /// Transition after time elapsed
    TimeElapsed(f32),
    /// Transition based on parameter value
    ParameterValue(String, f32),
    /// Transition on animation end
    AnimationEnd,
    /// Manual transition trigger
    Manual(String),
}

/// Animation parameters for state machine
#[derive(Debug, Clone)]
pub enum AnimationParameter {
    Float(String, f32),
    Bool(String, bool),
    Trigger(String),
}

/// Helper methods for AnimationLayer
impl AnimationLayer {
    /// Create a new animation layer with default values
    pub fn new(name: String) -> Self {
        Self {
            name,
            animation: None,
            weight: 1.0,
            blend_mode: BlendMode::Lerp,
            playback_rate: 1.0,
            current_time: 0.0,
            loop_animation: true,
            is_active: true,
            has_sound_data: false,
            sound_bone_name: None,
            sound_bone_index: None,
            prev_sound_frame: 0.0,
        }
    }
}

/// Helper methods for AnimationState
impl AnimationState {
    /// Create a new animation state
    pub fn new(
        name: String,
        animation_name: String,
        playback_rate: f32,
        loop_animation: bool,
    ) -> Self {
        Self {
            name,
            animation_name,
            playback_rate,
            loop_animation,
            transitions: Vec::new(),
        }
    }

    /// Add a transition to this state
    pub fn add_transition(&mut self, transition: AnimationTransition) {
        self.transitions.push(transition);
    }
}

/// Advanced animatable object with blending and state machine
#[derive(Debug)]
#[allow(dead_code)]
pub struct AdvancedAnimatable3DObj {
    pub htree: HTreeClass,
    pub layers: Vec<AnimationLayer>,
    pub current_state: Option<String>,
    pub states: HashMap<String, AnimationState>,
    pub parameters: HashMap<String, AnimationParameter>,
    pub bone_matrices: Vec<Mat4>,
    pub animation_channels: Vec<AnimationChannel>, // Animation data storage
    combo: HAnimCombo,
    root_transform: Mat4,
    #[cfg(feature = "renderer-3d")]
    bound_meshes: Vec<MeshBinding>,
    #[cfg(not(feature = "renderer-3d"))]
    bound_meshes: (),
}

#[cfg(feature = "renderer-3d")]
#[derive(Debug, Clone)]
struct MeshBinding {
    mesh_ptr: usize,
    influences_initialized: bool,
}

impl AdvancedAnimatable3DObj {
    /// Create a new advanced animatable object
    pub fn new(htree: HTreeClass) -> Self {
        let bone_count = htree.num_pivots();
        Self {
            htree,
            layers: Vec::new(),
            current_state: None,
            states: HashMap::new(),
            parameters: HashMap::new(),
            bone_matrices: vec![Mat4::IDENTITY; bone_count],
            animation_channels: Vec::new(),
            combo: HAnimCombo::new(),
            root_transform: Mat4::IDENTITY,
            #[cfg(feature = "renderer-3d")]
            bound_meshes: Vec::new(),
            #[cfg(not(feature = "renderer-3d"))]
            bound_meshes: (),
        }
    }

    /// Add an animation layer
    pub fn add_layer(&mut self, mut layer: AnimationLayer) {
        self.reset_layer_sound_state(&mut layer);
        if layer.animation.is_some() {
            self.refresh_layer_sound_metadata(&mut layer);
        }
        self.layers.push(layer);
    }

    pub fn set_root_transform(&mut self, transform: Mat4) {
        self.root_transform = transform;
    }

    /// Set layer animation
    pub fn set_layer_animation(&mut self, layer_index: usize, animation: Arc<HAnimClass>) {
        if let Some(layer) = self.layers.get_mut(layer_index) {
            layer.animation = Some(animation);
            layer.current_time = 0.0;
            // Reset sound state directly on the layer
            layer.prev_sound_frame = 0.0;
            layer.has_sound_data = false;
            layer.sound_bone_name = None;
            layer.sound_bone_index = None;

            // Refresh sound metadata
            if let Some(animation) = layer.animation.as_ref() {
                if has_embedded_sounds(animation) {
                    layer.has_sound_data = true;
                    if let Some(bone) = embedded_sound_bone(animation) {
                        layer.sound_bone_name = Some(bone.clone());
                    }
                }
            }
        }
    }

    /// Set layer weight
    pub fn set_layer_weight(&mut self, layer_index: usize, weight: f32) {
        if let Some(layer) = self.layers.get_mut(layer_index) {
            layer.weight = weight.clamp(0.0, 1.0);
        }
    }

    /// Add animation state
    pub fn add_state(&mut self, state: AnimationState) {
        self.states.insert(state.name.clone(), state);
    }

    /// Set current animation state
    pub fn set_state(&mut self, state_name: &str) -> Result<(), String> {
        if let Some(_state) = self.states.get(state_name) {
            self.current_state = Some(state_name.to_string());

            // Set base layer animation
            if let Some(layer) = self.layers.get_mut(0) {
                // In a real implementation, we'd load the animation by name
                // For now, we'll just mark it as active
                layer.is_active = true;
            }

            Ok(())
        } else {
            Err(format!("Animation state '{}' not found", state_name))
        }
    }

    /// Set animation parameter
    pub fn set_parameter(&mut self, name: &str, value: AnimationParameter) {
        self.parameters.insert(name.to_string(), value);
    }

    /// Set float parameter
    pub fn set_float_parameter(&mut self, name: &str, value: f32) {
        self.set_parameter(name, AnimationParameter::Float(name.to_string(), value));
    }

    /// Set bool parameter
    pub fn set_bool_parameter(&mut self, name: &str, value: bool) {
        self.set_parameter(name, AnimationParameter::Bool(name.to_string(), value));
    }

    /// Trigger a named event
    pub fn trigger(&mut self, name: &str) {
        self.set_parameter(name, AnimationParameter::Trigger(name.to_string()));
    }

    /// Clear triggers after one frame
    pub fn clear_triggers(&mut self) {
        let trigger_names: Vec<String> = self
            .parameters
            .iter()
            .filter_map(|(key, value)| {
                if matches!(value, AnimationParameter::Trigger(_)) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        for name in trigger_names {
            self.parameters.remove(&name);
        }
    }

    /// Get parameter value
    pub fn get_parameter_float(&self, name: &str) -> Option<f32> {
        if let Some(AnimationParameter::Float(_, value)) = self.parameters.get(name) {
            Some(*value)
        } else {
            None
        }
    }

    /// Update animation state machine
    pub fn update_state_machine(&mut self, delta_time: f32) {
        // Update any active transitions first
        self.update_transitions(delta_time);

        // Clone state name and transitions to avoid borrowing conflicts
        if let Some(current_state_name) = self.current_state.clone() {
            let transitions = self
                .states
                .get(&current_state_name)
                .map(|state| state.transitions.clone());

            if let Some(transitions) = transitions {
                // Check transitions
                for transition in &transitions {
                    if self.evaluate_transition(&transition.condition, delta_time) {
                        // Perform transition with blend time
                        if transition.blend_time > 0.0 {
                            self.start_transition_blend(
                                &transition.target_state,
                                transition.blend_time,
                            )
                            .ok();
                        } else {
                            self.set_state(&transition.target_state).ok();
                        }
                        break;
                    }
                }
            }
        }
    }

    /// Evaluate transition condition
    fn evaluate_transition(&self, condition: &TransitionCondition, _delta_time: f32) -> bool {
        match condition {
            TransitionCondition::TimeElapsed(duration) => {
                // Check if enough time has passed in current state
                if let Some(layer) = self.layers.get(0) {
                    layer.current_time >= *duration
                } else {
                    false
                }
            }
            TransitionCondition::ParameterValue(param_name, target_value) => {
                if let Some(value) = self.get_parameter_float(param_name) {
                    (value - *target_value).abs() < 0.01
                } else {
                    false
                }
            }
            TransitionCondition::AnimationEnd => {
                // Check if current animation has ended
                self.layers
                    .get(0)
                    .and_then(|layer| layer.animation.as_ref())
                    .map(|anim| {
                        let current_time = self.layers.get(0).unwrap().current_time;
                        current_time >= anim.num_frames as f32 / anim.frame_rate
                    })
                    .unwrap_or(false)
            }
            TransitionCondition::Manual(trigger_name) => {
                // Check if manual trigger was activated
                matches!(
                    self.parameters.get(trigger_name),
                    Some(AnimationParameter::Trigger(_))
                )
            }
        }
    }

    /// Update all animation layers and blend them
    pub fn update_animation(&mut self, delta_time: f32) {
        // Update state machine
        self.update_state_machine(delta_time);

        // Update each layer
        for layer in &mut self.layers {
            if layer.is_active {
                Self::update_layer(layer, delta_time);
            }
        }

        // Blend all layers
        self.blend_layers();

        // Apply hierarchy
        self.apply_hierarchy();

        // After computing the hierarchy transforms, propagate them to bound meshes.
        self.push_skinning_to_meshes();
    }

    /// Update a single animation layer
    fn update_layer(layer: &mut AnimationLayer, delta_time: f32) {
        if let Some(ref animation) = layer.animation {
            // Update playback time
            layer.current_time += delta_time * layer.playback_rate;

            // Handle looping
            if layer.loop_animation {
                let duration = animation.num_frames as f32 / animation.frame_rate;
                if layer.current_time >= duration {
                    layer.current_time = layer.current_time % duration;
                }
            } else {
                let duration = animation.num_frames as f32 / animation.frame_rate;
                layer.current_time = layer.current_time.min(duration);
            }
        }
    }

    /// Blend all animation layers
    fn blend_layers(&mut self) {
        self.combo.clear();

        for layer in &mut self.layers {
            if !layer.is_active {
                continue;
            }

            let Some(animation) = layer.animation.as_ref() else {
                continue;
            };
            if layer.weight <= 0.0 {
                continue;
            }

            let current_frame = layer.current_time * animation.get_frame_rate();

            if layer.has_sound_data && layer.sound_bone_index.is_none() {
                if let Some(ref bone_name) = layer.sound_bone_name {
                    layer.sound_bone_index = self.htree.find_pivot_index(bone_name);
                }
            }

            let mut entry = HAnimComboData::new(false);
            entry.set_motion(Some(animation.clone()));
            entry.set_weight(layer.weight);
            entry.set_frame(current_frame);
            self.combo.append_entry(entry);
        }

        if self.combo.num_anims() == 0 {
            return;
        }

        let _ = self.combo.normalize_weights(Some(&self.htree));
        self.htree.combo_update(self.root_transform, &self.combo);

        for i in 0..self.htree.num_pivots() {
            if let Some(transform) = self.htree.transform(i) {
                self.bone_matrices[i] = transform;
            }
        }

        for layer in &mut self.layers {
            if !layer.has_sound_data {
                continue;
            }
            if let Some(animation) = layer.animation.as_ref() {
                let current_frame = layer.current_time * animation.get_frame_rate();
                let mut prev_frame = layer.prev_sound_frame;
                if current_frame < prev_frame {
                    prev_frame = 0.0;
                }

                let transform_matrix = layer
                    .sound_bone_index
                    .and_then(|idx| self.htree.transform(idx))
                    .unwrap_or_else(|| self.htree.transform(0).unwrap_or(Mat4::IDENTITY));

                layer.prev_sound_frame =
                    trigger_sound(animation, prev_frame, current_frame, &transform_matrix);
            }
        }
    }

    fn reset_layer_sound_state(&self, layer: &mut AnimationLayer) {
        layer.prev_sound_frame = 0.0;
        layer.has_sound_data = false;
        layer.sound_bone_name = None;
        layer.sound_bone_index = None;
    }

    fn refresh_layer_sound_metadata(&self, layer: &mut AnimationLayer) {
        if let Some(animation) = layer.animation.as_ref() {
            if has_embedded_sounds(animation) {
                layer.has_sound_data = true;
                if let Some(bone) = embedded_sound_bone(animation) {
                    layer.sound_bone_name = Some(bone.clone());
                    if let Some(ref bone_name) = layer.sound_bone_name {
                        layer.sound_bone_index = self.htree.find_pivot_index(bone_name);
                    }
                }
            }
        }
    }

    /// Linear interpolation between two matrices
    /// Properly interpolates translation, rotation, and scale components
    fn lerp_matrix(a: &Mat4, b: &Mat4, t: f32) -> Mat4 {
        // Decompose matrices into transform components
        let (a_scale, a_rotation, a_translation) = a.to_scale_rotation_translation();
        let (b_scale, b_rotation, b_translation) = b.to_scale_rotation_translation();

        // Interpolate each component
        let lerped_translation = a_translation.lerp(b_translation, t);
        let lerped_rotation = a_rotation.slerp(b_rotation, t);
        let lerped_scale = a_scale.lerp(b_scale, t);

        // Reconstruct matrix
        Mat4::from_scale_rotation_translation(lerped_scale, lerped_rotation, lerped_translation)
    }

    /// Apply hierarchical transformations
    fn apply_hierarchy(&mut self) {
        // Bone matrices are already in world space after blending
        // No additional hierarchy application needed
    }

    /// Attach a mesh so that it receives bone palette updates after each animation tick.
    #[cfg(feature = "renderer-3d")]
    pub fn bind_mesh(&mut self, mesh: *mut MeshClass) {
        if mesh.is_null() {
            return;
        }
        let addr = mesh as usize;
        if self
            .bound_meshes
            .iter()
            .any(|binding| binding.mesh_ptr == addr)
        {
            return;
        }
        self.bound_meshes.push(MeshBinding {
            mesh_ptr: addr,
            influences_initialized: false,
        });
    }

    /// Detach a previously bound mesh.
    #[cfg(feature = "renderer-3d")]
    pub fn unbind_mesh(&mut self, mesh: *const MeshClass) {
        let addr = mesh as usize;
        self.bound_meshes.retain(|binding| binding.mesh_ptr != addr);
    }

    #[cfg(not(feature = "renderer-3d"))]
    pub fn bind_mesh(&mut self, _mesh: *mut MeshClass) {}

    #[cfg(not(feature = "renderer-3d"))]
    pub fn unbind_mesh(&mut self, _mesh: *const MeshClass) {}

    #[cfg(feature = "renderer-3d")]
    fn push_skinning_to_meshes(&mut self) {
        if self.bound_meshes.is_empty() {
            return;
        }

        let bone_count = self
            .htree
            .num_pivots()
            .min(self.bone_matrices.len())
            .min(MAX_SKINNING_MATRICES);
        let palette = &self.bone_matrices[..bone_count];

        self.bound_meshes.retain_mut(|binding| {
            // Validate mesh pointer before dereferencing
            // Address 0 is reserved as null, small addresses are likely invalid
            if binding.mesh_ptr == 0 || binding.mesh_ptr < std::mem::size_of::<MeshClass>() {
                return false; // Remove invalid binding
            }

            let mesh_ptr = binding.mesh_ptr as *mut MeshClass;

            // Additional safety: only dereference if pointer is properly aligned
            if mesh_ptr as usize % std::mem::align_of::<MeshClass>() != 0 {
                return false; // Remove misaligned binding
            }

            unsafe {
                if let Some(mesh) = mesh_ptr.as_mut() {
                    if !binding.influences_initialized {
                        if let Some(model) = mesh.model.as_ref() {
                            if let Some(influences) = model.vertex_influences() {
                                if mesh
                                    .vertex_bone_links()
                                    .map_or(true, |links| links.len() != influences.len())
                                {
                                    mesh.set_vertex_influences(influences.to_vec());
                                }
                            } else if let Some(bone_links) = model.vertex_bone_links() {
                                if mesh
                                    .vertex_bone_links()
                                    .map_or(true, |links| links.len() != bone_links.len())
                                {
                                    mesh.set_vertex_bone_links(bone_links.to_vec());
                                }
                            }
                        }
                        binding.influences_initialized = true;
                    }

                    mesh.set_bone_palette_slice(palette);
                    true
                } else {
                    warn!("animation object lost bound mesh reference at {mesh_ptr:p}");
                    false
                }
            }
        });
    }

    #[cfg(not(feature = "renderer-3d"))]
    fn push_skinning_to_meshes(&mut self) {}

    /// Get bone transformation matrix
    pub fn get_bone_transform(&self, bone_idx: usize) -> Option<&Mat4> {
        self.bone_matrices.get(bone_idx)
    }

    /// Get current animation state
    pub fn get_current_state(&self) -> Option<&str> {
        self.current_state.as_deref()
    }

    /// Check if animation is playing
    pub fn is_animation_playing(&self, layer_index: usize) -> bool {
        self.layers
            .get(layer_index)
            .map(|layer| layer.is_active && layer.animation.is_some())
            .unwrap_or(false)
    }

    /// Blend two animations using different blend modes
    /// This matches the C++ Blend_Update functionality
    pub fn blend_animations(
        &mut self,
        animation0: &HAnimClass,
        frame0: f32,
        animation1: &HAnimClass,
        frame1: f32,
        blend_factor: f32,
        root_transform: Mat4,
    ) {
        // Sample both animations
        let mut transforms0 = Vec::new();
        let mut transforms1 = Vec::new();

        for i in 0..self.htree.num_pivots() {
            let trans0 = animation0.get_translation(i, frame0);
            let rot0 = animation0.get_orientation(i, frame0);

            let trans1 = animation1.get_translation(i, frame1);
            let rot1 = animation1.get_orientation(i, frame1);

            transforms0.push(Mat4::from_rotation_translation(rot0, trans0));
            transforms1.push(Mat4::from_rotation_translation(rot1, trans1));
        }

        // Blend the transformations
        for i in 0..self.htree.num_pivots() {
            let blended = Self::lerp_matrix(&transforms0[i], &transforms1[i], blend_factor);

            // Apply to hierarchy
            if let Some(pivot) = self.htree.get_pivot(i) {
                self.bone_matrices[i] = pivot.base_transform * blended;
            } else {
                self.bone_matrices[i] = blended;
            }
        }

        // Apply hierarchy transformations
        self.apply_hierarchy_transforms(root_transform);
    }

    /// Apply hierarchical transformations to bone matrices
    /// Matches C++ hierarchy update logic
    fn apply_hierarchy_transforms(&mut self, root_transform: Mat4) {
        // Set root transform
        if !self.bone_matrices.is_empty() {
            self.bone_matrices[0] = root_transform;
        }

        // Apply parent-child relationships
        for i in 1..self.htree.num_pivots() {
            if let Some(parent_idx) = self.htree.get_parent_index(i) {
                if parent_idx >= 0 && (parent_idx as usize) < self.bone_matrices.len() {
                    let parent_transform = self.bone_matrices[parent_idx as usize];
                    self.bone_matrices[i] = parent_transform * self.bone_matrices[i];
                }
            }
        }
    }

    /// Update transition blending weights and timing
    fn update_transitions(&mut self, delta_time: f32) {
        // Update transition layers
        let mut completed_transitions = Vec::new();

        for (i, layer) in self.layers.iter_mut().enumerate() {
            if layer.name.starts_with("transition_to_") && layer.is_active {
                // Update transition weight based on time
                layer.current_time += delta_time * layer.playback_rate;

                if layer.current_time >= 1.0 {
                    // Transition complete
                    layer.weight = 1.0;
                    layer.is_active = false;
                    completed_transitions.push(i);
                } else {
                    // Update weight based on easing curve (linear for now)
                    layer.weight = layer.current_time;
                }
            }
        }

        // Remove completed transitions
        for &index in completed_transitions.iter().rev() {
            self.layers.remove(index);
        }
    }

    /// Start a blended transition to a new state
    fn start_transition_blend(
        &mut self,
        target_state: &str,
        blend_time: f32,
    ) -> Result<(), String> {
        if !self.states.contains_key(target_state) {
            return Err(format!("Target state '{}' not found", target_state));
        }

        // Reduce weight of current layers
        for layer in &mut self.layers {
            if layer.is_active {
                layer.weight *= 0.5; // Fade out current layers
            }
        }

        // Create transition layer
        let transition_layer = AnimationLayer {
            name: format!("transition_to_{}", target_state),
            animation: None, // Will be set when animation is loaded
            weight: 0.0,
            blend_mode: BlendMode::Lerp,
            playback_rate: 1.0 / blend_time,
            current_time: 0.0,
            loop_animation: false,
            is_active: true,
            has_sound_data: false,
            sound_bone_name: None,
            sound_bone_index: None,
            prev_sound_frame: 0.0,
        };

        self.layers.push(transition_layer);
        self.current_state = Some(target_state.to_string());
        Ok(())
    }

    /// Create smooth transition between two animation states
    pub fn transition_to_state(
        &mut self,
        target_state: &str,
        transition_time: f32,
    ) -> Result<(), String> {
        if self.states.contains_key(target_state) {
            // Create transition layer
            let transition_layer = AnimationLayer {
                name: format!("transition_to_{}", target_state),
                animation: None, // Will be set when transition animation is loaded
                weight: 0.0,     // Start at 0, animate to 1
                blend_mode: BlendMode::Lerp,
                playback_rate: 1.0 / transition_time, // Complete transition in specified time
                current_time: 0.0,
                loop_animation: false,
                is_active: true,
                has_sound_data: false,
                sound_bone_name: None,
                sound_bone_index: None,
                prev_sound_frame: 0.0,
            };

            self.layers.push(transition_layer);
            self.current_state = Some(target_state.to_string());
            Ok(())
        } else {
            Err(format!("Target state '{}' not found", target_state))
        }
    }
}

// Helper functions removed - using glam's built-in methods instead

/// Animation controller for managing complex animation state
pub struct AnimationController {
    pub animatable_objects: HashMap<String, AdvancedAnimatable3DObj>,
    pub global_parameters: HashMap<String, AnimationParameter>,
    anim_manager: HAnimManager,
}

impl AnimationController {
    /// Create a new animation controller
    pub fn new() -> Self {
        Self {
            animatable_objects: HashMap::new(),
            global_parameters: HashMap::new(),
            anim_manager: HAnimManager::new(),
        }
    }

    /// Register an animatable object
    pub fn register_object(&mut self, name: String, object: AdvancedAnimatable3DObj) {
        self.animatable_objects.insert(name, object);
    }

    /// Bind a mesh instance to a registered animatable object so it receives skinning updates.
    #[cfg(feature = "renderer-3d")]
    pub fn bind_mesh_to_object(
        &mut self,
        object_name: &str,
        mesh: *mut MeshClass,
    ) -> Result<(), String> {
        let object = self
            .animatable_objects
            .get_mut(object_name)
            .ok_or_else(|| format!("Object '{}' not registered", object_name))?;
        object.bind_mesh(mesh);
        Ok(())
    }

    /// Remove a mesh binding from an animatable object.
    #[cfg(feature = "renderer-3d")]
    pub fn unbind_mesh_from_object(&mut self, object_name: &str, mesh: *const MeshClass) {
        if let Some(object) = self.animatable_objects.get_mut(object_name) {
            object.unbind_mesh(mesh);
        }
    }

    #[cfg(not(feature = "renderer-3d"))]
    pub fn bind_mesh_to_object(
        &mut self,
        _object_name: &str,
        _mesh: *mut MeshClass,
    ) -> Result<(), String> {
        Ok(())
    }

    #[cfg(not(feature = "renderer-3d"))]
    pub fn unbind_mesh_from_object(&mut self, _object_name: &str, _mesh: *const MeshClass) {}

    /// Add animation to the shared manager
    pub fn add_animation(&mut self, animation: HAnimClass) {
        self.anim_manager.add_anim(Arc::new(animation));
    }

    pub fn add_animation_arc(&mut self, animation: Arc<HAnimClass>) {
        self.anim_manager.add_anim(animation);
    }

    pub fn load_animation_prototype(
        &mut self,
        proto: &ww3d_assets::prototypes::AnimationPrototype,
    ) -> Arc<HAnimClass> {
        self.anim_manager.load_prototype(proto)
    }

    pub fn set_object_animation(
        &mut self,
        object_name: &str,
        layer_index: usize,
        animation_name: &str,
    ) -> Result<(), String> {
        let animation = self
            .anim_manager
            .get_anim(animation_name)
            .ok_or_else(|| format!("Animation '{}' not found", animation_name))?;

        let object = self
            .animatable_objects
            .get_mut(object_name)
            .ok_or_else(|| format!("Object '{}' not registered", object_name))?;

        object.set_layer_animation(layer_index, animation);
        Ok(())
    }

    /// Set global parameter
    pub fn set_global_parameter(&mut self, name: &str, value: AnimationParameter) {
        // Clone value for propagation before moving it into the map
        let value_clone = value.clone();
        self.global_parameters.insert(name.to_string(), value);

        // Propagate to all objects
        for object in self.animatable_objects.values_mut() {
            object.set_parameter(name, value_clone.clone());
        }
    }

    /// Update all animatable objects
    pub fn update(&mut self, delta_time: f32) {
        for object in self.animatable_objects.values_mut() {
            object.update_animation(delta_time);
        }
    }

    /// Get object by name
    pub fn get_object(&self, name: &str) -> Option<&AdvancedAnimatable3DObj> {
        self.animatable_objects.get(name)
    }

    /// Get mutable object by name
    pub fn get_object_mut(&mut self, name: &str) -> Option<&mut AdvancedAnimatable3DObj> {
        self.animatable_objects.get_mut(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_layer_blending() {
        let htree = HTreeClass::new();
        let mut animatable = AdvancedAnimatable3DObj::new(htree);

        // Add a layer
        let layer = AnimationLayer {
            name: "base".to_string(),
            animation: None,
            weight: 1.0,
            blend_mode: BlendMode::Override,
            playback_rate: 1.0,
            current_time: 0.0,
            loop_animation: true,
            is_active: true,
            has_sound_data: false,
            sound_bone_name: None,
            sound_bone_index: None,
            prev_sound_frame: 0.0,
        };

        animatable.add_layer(layer);

        // Should have one layer
        assert_eq!(animatable.layers.len(), 1);
        assert!(animatable.layers[0].is_active);
    }

    #[test]
    fn test_animation_state_machine() {
        let htree = HTreeClass::new();
        let mut animatable = AdvancedAnimatable3DObj::new(htree);

        // Add states
        let state1 = AnimationState {
            name: "idle".to_string(),
            animation_name: "idle_anim".to_string(),
            playback_rate: 1.0,
            loop_animation: true,
            transitions: vec![],
        };

        let state2 = AnimationState {
            name: "walk".to_string(),
            animation_name: "walk_anim".to_string(),
            playback_rate: 1.0,
            loop_animation: true,
            transitions: vec![],
        };

        animatable.add_state(state1);
        animatable.add_state(state2);

        // Set initial state
        animatable.set_state("idle").unwrap();
        assert_eq!(animatable.get_current_state(), Some("idle"));
    }
}
