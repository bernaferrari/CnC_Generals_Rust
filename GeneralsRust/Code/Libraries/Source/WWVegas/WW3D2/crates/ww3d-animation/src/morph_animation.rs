//! Morphing Animation System - HMorphAnim equivalent
//!
//! This module implements vertex morphing animations that were a major
//! feature of the original C++ WW3D2. Morphing animations deform mesh
//! geometry by interpolating between different vertex positions.

use crate::HTreeClass;
use glam::{Vec3, Mat4};
use std::collections::HashMap;

/// Morph target - a set of vertex positions for a specific pose
#[derive(Debug, Clone)]
pub struct MorphTarget {
    pub name: String,
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
}

/// Morph channel - controls interpolation between morph targets
#[derive(Debug, Clone)]
pub struct MorphChannel {
    pub target_indices: Vec<usize>, // Indices of morph targets to blend
    pub weights: Vec<f32>,         // Blend weights for each target
    pub times: Vec<f32>,          // Time keyframes for weight changes
}

/// Morphing animation data
#[derive(Debug)]
pub struct HMorphAnim {
    pub name: String,
    pub targets: Vec<MorphTarget>,
    pub channels: Vec<MorphChannel>,
    pub num_frames: u32,
    pub frame_rate: f32,
    pub base_vertex_count: usize,
}

impl HMorphAnim {
    /// Create a new morphing animation
    pub fn new(name: String, base_vertex_count: usize) -> Self {
        Self {
            name,
            targets: Vec::new(),
            channels: Vec::new(),
            num_frames: 0,
            frame_rate: 30.0,
            base_vertex_count,
        }
    }

    /// Add a morph target
    /// Returns error if target vertex count doesn't match base mesh
    pub fn add_target(&mut self, target: MorphTarget) -> Result<(), String> {
        // Validate that target has correct vertex count
        if target.vertices.len() != self.base_vertex_count {
            return Err(format!(
                "Morph target vertex count mismatch: expected {}, got {}",
                self.base_vertex_count, target.vertices.len()
            ));
        }
        if target.normals.len() != self.base_vertex_count {
            return Err(format!(
                "Morph target normal count mismatch: expected {}, got {}",
                self.base_vertex_count, target.normals.len()
            ));
        }
        self.targets.push(target);
        Ok(())
    }

    /// Add a morph channel
    pub fn add_channel(&mut self, channel: MorphChannel) {
        self.channels.push(channel);
    }

    /// Sample morph animation at a specific frame
    pub fn sample(&self, frame: f32) -> MorphResult {
        let mut vertex_positions = vec![Vec3::ZERO; self.base_vertex_count];
        let mut vertex_normals = vec![Vec3::ZERO; self.base_vertex_count];
        let mut total_weights = vec![0.0; self.base_vertex_count];

        // Process each morph channel
        for channel in &self.channels {
            let weights = self.sample_channel_weights(channel, frame);

            for (i, &weight) in weights.iter().enumerate() {
                if i >= channel.target_indices.len() {
                    continue;
                }

                let target_idx = channel.target_indices[i];
                if target_idx >= self.targets.len() {
                    continue;
                }

                let target = &self.targets[target_idx];

                // Blend vertex positions
                for j in 0..vertex_positions.len().min(target.vertices.len()) {
                    vertex_positions[j] += target.vertices[j] * weight;
                    total_weights[j] += weight;
                }

                // Blend vertex normals
                for j in 0..vertex_normals.len().min(target.normals.len()) {
                    vertex_normals[j] += target.normals[j] * weight;
                }
            }
        }

        // Normalize by total weights
        for i in 0..vertex_positions.len() {
            if total_weights[i] > 0.0 {
                vertex_positions[i] /= total_weights[i];
                vertex_normals[i] = vertex_normals[i].normalize_or_zero();
            }
        }

        MorphResult {
            vertices: vertex_positions,
            normals: vertex_normals,
        }
    }

    /// Sample weights for a morph channel at a specific frame
    fn sample_channel_weights(&self, channel: &MorphChannel, frame: f32) -> Vec<f32> {
        if channel.times.is_empty() {
            return vec![0.0; channel.weights.len()];
        }

        // Find the appropriate time segment
        let mut left = 0;
        let mut right = channel.times.len() - 1;

        while left < right {
            let mid = (left + right) / 2;
            if channel.times[mid] < frame {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        if left == 0 {
            // Before first keyframe
            channel.weights.clone()
        } else if left >= channel.times.len() {
            // After last keyframe
            vec![0.0; channel.weights.len()]
        } else {
            // Interpolate between keyframes
            let time1 = channel.times[left - 1];
            let time2 = channel.times[left];
            let weights1 = &channel.weights[(left - 1) * channel.target_indices.len()..left * channel.target_indices.len()];
            let weights2 = &channel.weights[left * channel.target_indices.len()..(left + 1) * channel.target_indices.len()];

            if (time2 - time1).abs() < f32::EPSILON {
                weights1.to_vec()
            } else {
                let factor = (frame - time1) / (time2 - time1);
                let mut result = Vec::new();

                for i in 0..weights1.len().min(weights2.len()) {
                    let weight = weights1[i] + (weights2[i] - weights1[i]) * factor;
                    result.push(weight);
                }

                result
            }
        }
    }

    /// Get morph target by name
    pub fn get_target(&self, name: &str) -> Option<&MorphTarget> {
        self.targets.iter().find(|t| t.name == name)
    }

    /// Get morph target by index
    pub fn get_target_by_index(&self, index: usize) -> Option<&MorphTarget> {
        self.targets.get(index)
    }
}

/// Result of morphing animation sampling
#[derive(Debug, Clone)]
pub struct MorphResult {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
}

/// Morphing animation manager
pub struct HMorphAnimManager {
    animations: HashMap<String, HMorphAnim>,
    base_meshes: HashMap<String, Vec<Vec3>>, // Base mesh vertex positions
}

impl HMorphAnimManager {
    pub fn new() -> Self {
        Self {
            animations: HashMap::new(),
            base_meshes: HashMap::new(),
        }
    }

    /// Register a base mesh for morphing
    pub fn register_base_mesh(&mut self, name: String, vertices: Vec<Vec3>) {
        self.base_meshes.insert(name, vertices);
    }

    /// Load a morphing animation
    pub fn load_animation(&mut self, anim: HMorphAnim) {
        self.animations.insert(anim.name.clone(), anim);
    }

    /// Get animation by name
    pub fn get_animation(&self, name: &str) -> Option<&HMorphAnim> {
        self.animations.get(name)
    }

    /// Apply morphing animation to base mesh
    pub fn apply_morph(&self, anim_name: &str, base_mesh_name: &str, frame: f32) -> Option<MorphResult> {
        let anim = self.animations.get(anim_name)?;
        let base_vertices = self.base_meshes.get(base_mesh_name)?;

        // Validate vertex count
        if anim.base_vertex_count != base_vertices.len() {
            return None;
        }

        let mut result = anim.sample(frame);

        // Add base mesh positions
        for i in 0..result.vertices.len() {
            result.vertices[i] += base_vertices[i];
        }

        Some(result)
    }
}

/// Facial animation system - specialized morphing for faces
pub struct FacialAnimationSystem {
    morph_anim: HMorphAnim,
    expression_weights: HashMap<String, f32>,
    phoneme_weights: HashMap<String, f32>,
}

impl FacialAnimationSystem {
    /// Create a new facial animation system
    pub fn new(base_vertex_count: usize) -> Self {
        Self {
            morph_anim: HMorphAnim::new("facial".to_string(), base_vertex_count),
            expression_weights: HashMap::new(),
            phoneme_weights: HashMap::new(),
        }
    }

    /// Add a facial expression morph target
    pub fn add_expression(&mut self, name: &str, vertices: Vec<Vec3>, normals: Vec<Vec3>) {
        let target = MorphTarget {
            name: format!("expression_{}", name),
            vertices,
            normals,
        };
        self.morph_anim.add_target(target);
        self.expression_weights.insert(name.to_string(), 0.0);
    }

    /// Add a phoneme morph target for lip sync
    pub fn add_phoneme(&mut self, phoneme: &str, vertices: Vec<Vec3>, normals: Vec<Vec3>) {
        let target = MorphTarget {
            name: format!("phoneme_{}", phoneme),
            vertices,
            normals,
        };
        self.morph_anim.add_target(target);
        self.phoneme_weights.insert(phoneme.to_string(), 0.0);
    }

    /// Set expression weight
    pub fn set_expression_weight(&mut self, expression: &str, weight: f32) {
        if let Some(w) = self.expression_weights.get_mut(expression) {
            *w = weight.clamp(0.0, 1.0);
        }
    }

    /// Set phoneme weight
    pub fn set_phoneme_weight(&mut self, phoneme: &str, weight: f32) {
        if let Some(w) = self.phoneme_weights.get_mut(phoneme) {
            *w = weight.clamp(0.0, 1.0);
        }
    }

    /// Update morph channels based on current weights
    pub fn update_morph_channels(&mut self) {
        self.morph_anim.channels.clear();

        // Create expression channels
        for (expr_name, &weight) in &self.expression_weights {
            let target_name = format!("expression_{}", expr_name);
            if let Some(target_idx) = self.morph_anim.targets.iter().position(|t| t.name == target_name) {
                let mut channel = MorphChannel {
                    target_indices: vec![target_idx],
                    weights: vec![weight],
                    times: vec![0.0], // Static weight
                };
                self.morph_anim.add_channel(channel);
            }
        }

        // Create phoneme channels
        for (phoneme_name, &weight) in &self.phoneme_weights {
            let target_name = format!("phoneme_{}", phoneme_name);
            if let Some(target_idx) = self.morph_anim.targets.iter().position(|t| t.name == target_name) {
                let mut channel = MorphChannel {
                    target_indices: vec![target_idx],
                    weights: vec![weight],
                    times: vec![0.0], // Static weight
                };
                self.morph_anim.add_channel(channel);
            }
        }
    }

    /// Sample facial animation
    pub fn sample(&self, frame: f32) -> MorphResult {
        self.morph_anim.sample(frame)
    }

    /// Get all expression names
    pub fn get_expression_names(&self) -> Vec<&String> {
        self.expression_weights.keys().collect()
    }

    /// Get all phoneme names
    pub fn get_phoneme_names(&self) -> Vec<&String> {
        self.phoneme_weights.keys().collect()
    }

    /// Set multiple expression weights at once
    pub fn set_expression_blend(&mut self, weights: &HashMap<String, f32>) {
        for (name, &weight) in weights {
            self.set_expression_weight(name, weight);
        }
        self.update_morph_channels();
    }

    /// Clear all expression weights
    pub fn clear_expressions(&mut self) {
        for (_, weight) in self.expression_weights.iter_mut() {
            *weight = 0.0;
        }
        self.update_morph_channels();
    }
}

/// Advanced morph target blending system
/// Supports blending between multiple morph targets with different blend modes
pub struct AdvancedMorphBlender {
    base_vertices: Vec<Vec3>,
    base_normals: Vec<Vec3>,
    morph_targets: Vec<MorphTarget>,
    blend_weights: Vec<f32>,
    blend_modes: Vec<MorphBlendMode>,
}

/// Morph target blend modes
#[derive(Debug, Clone, Copy)]
pub enum MorphBlendMode {
    /// Replace base mesh
    Replace,
    /// Add to base mesh
    Additive,
    /// Multiply with base mesh
    Multiplicative,
    /// Linear interpolation
    Lerp,
}

impl AdvancedMorphBlender {
    /// Create a new morph blender
    pub fn new(base_vertices: Vec<Vec3>, base_normals: Vec<Vec3>) -> Self {
        Self {
            base_vertices,
            base_normals,
            morph_targets: Vec::new(),
            blend_weights: Vec::new(),
            blend_modes: Vec::new(),
        }
    }

    /// Add a morph target with blend mode
    /// Returns error if target vertex count doesn't match base mesh
    pub fn add_morph_target(&mut self, target: MorphTarget, weight: f32, blend_mode: MorphBlendMode) -> Result<(), String> {
        // Validate vertex count
        if target.vertices.len() != self.base_vertices.len() {
            return Err(format!(
                "Morph target vertex count mismatch: expected {}, got {}",
                self.base_vertices.len(), target.vertices.len()
            ));
        }

        self.morph_targets.push(target);
        self.blend_weights.push(weight);
        self.blend_modes.push(blend_mode);
        Ok(())
    }

    /// Set weight for a morph target
    pub fn set_weight(&mut self, target_index: usize, weight: f32) {
        if let Some(w) = self.blend_weights.get_mut(target_index) {
            *w = weight.clamp(0.0, 1.0);
        }
    }

    /// Blend all morph targets
    pub fn blend(&self) -> MorphResult {
        let mut result_vertices = self.base_vertices.clone();
        let mut result_normals = self.base_normals.clone();

        for (i, target) in self.morph_targets.iter().enumerate() {
            let weight = self.blend_weights[i];
            let blend_mode = self.blend_modes[i];

            if weight <= 0.0 {
                continue;
            }

            match blend_mode {
                MorphBlendMode::Replace => {
                    // Replace based on weight
                    for j in 0..result_vertices.len() {
                        result_vertices[j] = result_vertices[j].lerp(target.vertices[j], weight);
                        result_normals[j] = result_normals[j].lerp(target.normals[j], weight).normalize_or_zero();
                    }
                }
                MorphBlendMode::Additive => {
                    // Add scaled morph target
                    for j in 0..result_vertices.len() {
                        result_vertices[j] += target.vertices[j] * weight;
                        result_normals[j] = (result_normals[j] + target.normals[j] * weight).normalize_or_zero();
                    }
                }
                MorphBlendMode::Multiplicative => {
                    // Multiply by morph target (scaled)
                    for j in 0..result_vertices.len() {
                        let scale_factor = 1.0 + (target.vertices[j].length() * weight);
                        result_vertices[j] *= scale_factor;
                        result_normals[j] = result_normals[j].normalize_or_zero();
                    }
                }
                MorphBlendMode::Lerp => {
                    // Linear interpolation
                    for j in 0..result_vertices.len() {
                        result_vertices[j] = result_vertices[j].lerp(target.vertices[j], weight);
                        result_normals[j] = result_normals[j].lerp(target.normals[j], weight).normalize_or_zero();
                    }
                }
            }
        }

        MorphResult {
            vertices: result_vertices,
            normals: result_normals,
        }
    }

    /// Get number of morph targets
    pub fn target_count(&self) -> usize {
        self.morph_targets.len()
    }

    /// Clear all morph targets
    pub fn clear_targets(&mut self) {
        self.morph_targets.clear();
        self.blend_weights.clear();
        self.blend_modes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_morph_animation_basic() {
        let mut anim = HMorphAnim::new("test_morph".to_string(), 3);

        // Add a simple morph target
        let target = MorphTarget {
            name: "smile".to_string(),
            vertices: vec![
                Vec3::new(0.1, 0.0, 0.0),
                Vec3::new(0.0, 0.1, 0.0),
                Vec3::new(-0.1, 0.0, 0.0),
            ],
            normals: vec![
                Vec3::X,
                Vec3::Y,
                Vec3::Z,
            ],
        };

        anim.add_target(target);

        // Add a channel
        let channel = MorphChannel {
            target_indices: vec![0],
            weights: vec![1.0],
            times: vec![0.0],
        };

        anim.add_channel(channel);

        // Sample animation
        let result = anim.sample(0.0);

        // Should have morphed vertices
        assert_eq!(result.vertices.len(), 3);
        assert!(result.vertices[0].x > 0.0); // Should be morphed from base
    }

    #[test]
    fn test_facial_animation_system() {
        let mut facial = FacialAnimationSystem::new(10);

        let vertices = vec![Vec3::new(0.0, 0.0, 0.0); 10];
        let normals = vec![Vec3::Y; 10];

        facial.add_expression("happy", vertices.clone(), normals.clone());
        facial.set_expression_weight("happy", 0.5);
        facial.update_morph_channels();

        let result = facial.sample(0.0);
        assert_eq!(result.vertices.len(), 10);
    }
}