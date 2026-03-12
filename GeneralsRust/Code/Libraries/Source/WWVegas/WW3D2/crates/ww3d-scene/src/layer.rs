/// Layer System
/// This module implements the multi-layer rendering system from C++ layer.h/cpp
///
/// The layer system provides:
/// - Multi-layer rendering (UI, HUD, world, etc.)
/// - Independent cameras per layer
/// - Layer-specific clear operations
/// - Render order management
use crate::{CameraClass, SceneClass};
use glam::Vec3;

/// Layer class - represents a single rendering layer
///
/// Each layer has its own scene and camera, allowing for independent
/// rendering passes. Common use cases:
/// - Layer 0: 3D world
/// - Layer 1: HUD/UI overlay
/// - Layer 2: Debug visualization
pub struct Layer {
    /// Layer name
    pub name: String,
    /// Scene to render for this layer
    pub scene: Option<Box<SceneClass>>,
    /// Camera for this layer
    pub camera: Option<CameraClass>,
    /// Should clear color buffer before rendering?
    pub clear: bool,
    /// Should clear depth buffer before rendering?
    pub clear_z: bool,
    /// Clear color
    pub clear_color: Vec3,
    /// Is this layer enabled?
    pub enabled: bool,
    /// Render order priority (lower = earlier)
    pub priority: i32,
}

impl std::fmt::Debug for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Layer")
            .field("name", &self.name)
            .field("has_scene", &self.scene.is_some())
            .field("has_camera", &self.camera.is_some())
            .field("clear", &self.clear)
            .field("clear_z", &self.clear_z)
            .field("clear_color", &self.clear_color)
            .field("enabled", &self.enabled)
            .field("priority", &self.priority)
            .finish()
    }
}

impl Layer {
    /// Create a new layer
    pub fn new(name: String) -> Self {
        Self {
            name,
            scene: None,
            camera: None,
            clear: false,
            clear_z: true,
            clear_color: Vec3::ZERO,
            enabled: true,
            priority: 0,
        }
    }

    /// Set the scene for this layer
    pub fn set_scene(&mut self, scene: SceneClass) {
        self.scene = Some(Box::new(scene));
    }

    /// Get the scene for this layer
    pub fn get_scene(&self) -> Option<&SceneClass> {
        self.scene.as_ref().map(|s| &**s)
    }

    /// Get mutable scene reference
    pub fn get_scene_mut(&mut self) -> Option<&mut SceneClass> {
        self.scene.as_mut().map(|s| &mut **s)
    }

    /// Set the camera for this layer
    pub fn set_camera(&mut self, camera: CameraClass) {
        self.camera = Some(camera);
    }

    /// Get the camera for this layer
    pub fn get_camera(&self) -> Option<&CameraClass> {
        self.camera.as_ref()
    }

    /// Get mutable camera reference
    pub fn get_camera_mut(&mut self) -> Option<&mut CameraClass> {
        self.camera.as_mut()
    }

    /// Set clear parameters
    pub fn set_clear(&mut self, clear: bool, clear_z: bool, clear_color: Vec3) {
        self.clear = clear;
        self.clear_z = clear_z;
        self.clear_color = clear_color;
    }

    /// Set layer enabled state
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set layer priority
    pub fn set_priority(&mut self, priority: i32) {
        self.priority = priority;
    }

    /// Update the layer
    pub fn update(&mut self, dt: f32) {
        if let Some(scene) = &mut self.scene {
            scene.update(dt);
        }
    }

    /// Render the layer
    pub fn render(&self) {
        if !self.enabled {
            return;
        }

        if let (Some(scene), Some(camera)) = (&self.scene, &self.camera) {
            // Note: Clear operations (color/depth/stencil) are handled by the renderer backend
            // during render pass setup. This layer system controls logical rendering order.
            // C++ equivalent: LayerClass::Render applies clear flags to RenderContext
            scene.render(camera);
        }
    }
}

/// Layer manager - manages multiple rendering layers
#[derive(Debug)]
pub struct LayerManager {
    /// List of layers
    layers: Vec<Layer>,
}

impl LayerManager {
    /// Create a new layer manager
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Add a layer
    pub fn add_layer(&mut self, layer: Layer) {
        self.layers.push(layer);
        self.sort_layers();
    }

    /// Remove a layer by name
    pub fn remove_layer(&mut self, name: &str) -> bool {
        let initial_len = self.layers.len();
        self.layers.retain(|l| l.name != name);
        self.layers.len() < initial_len
    }

    /// Get a layer by name
    pub fn get_layer(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// Get a mutable layer by name
    pub fn get_layer_mut(&mut self, name: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }

    /// Get a layer by index
    pub fn get_layer_by_index(&self, index: usize) -> Option<&Layer> {
        self.layers.get(index)
    }

    /// Get a mutable layer by index
    pub fn get_layer_by_index_mut(&mut self, index: usize) -> Option<&mut Layer> {
        self.layers.get_mut(index)
    }

    /// Get the number of layers
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Sort layers by priority
    fn sort_layers(&mut self) {
        self.layers.sort_by_key(|l| l.priority);
    }

    /// Update all layers
    pub fn update(&mut self, dt: f32) {
        for layer in &mut self.layers {
            layer.update(dt);
        }
    }

    /// Render all enabled layers in priority order
    pub fn render(&self) {
        for layer in &self.layers {
            if layer.enabled {
                layer.render();
            }
        }
    }

    /// Clear all layers
    pub fn clear(&mut self) {
        self.layers.clear();
    }
}

impl Default for LayerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_creation() {
        let layer = Layer::new("WorldLayer".to_string());
        assert_eq!(layer.name, "WorldLayer");
        assert!(layer.enabled);
        assert_eq!(layer.priority, 0);
    }

    #[test]
    fn test_layer_scene() {
        let mut layer = Layer::new("TestLayer".to_string());
        let scene = SceneClass::new();
        layer.set_scene(scene);

        assert!(layer.get_scene().is_some());
    }

    #[test]
    fn test_layer_camera() {
        let mut layer = Layer::new("TestLayer".to_string());
        let camera = CameraClass::new();
        layer.set_camera(camera);

        assert!(layer.get_camera().is_some());
    }

    #[test]
    fn test_layer_manager() {
        let mut manager = LayerManager::new();

        let layer1 = Layer::new("Layer1".to_string());
        let layer2 = Layer::new("Layer2".to_string());

        manager.add_layer(layer1);
        manager.add_layer(layer2);

        assert_eq!(manager.layer_count(), 2);
        assert!(manager.get_layer("Layer1").is_some());
    }

    #[test]
    fn test_layer_priority_sorting() {
        let mut manager = LayerManager::new();

        let mut layer1 = Layer::new("Background".to_string());
        layer1.set_priority(10);

        let mut layer2 = Layer::new("Foreground".to_string());
        layer2.set_priority(0);

        manager.add_layer(layer1);
        manager.add_layer(layer2);

        // Foreground should be first (lower priority = earlier)
        assert_eq!(manager.get_layer_by_index(0).unwrap().name, "Foreground");
        assert_eq!(manager.get_layer_by_index(1).unwrap().name, "Background");
    }

    #[test]
    fn test_layer_removal() {
        let mut manager = LayerManager::new();

        let layer = Layer::new("TestLayer".to_string());
        manager.add_layer(layer);

        assert_eq!(manager.layer_count(), 1);
        assert!(manager.remove_layer("TestLayer"));
        assert_eq!(manager.layer_count(), 0);
    }

    #[test]
    fn test_layer_enable_disable() {
        let mut layer = Layer::new("TestLayer".to_string());
        assert!(layer.enabled);

        layer.set_enabled(false);
        assert!(!layer.enabled);
    }

    #[test]
    fn test_clear_settings() {
        let mut layer = Layer::new("TestLayer".to_string());
        layer.set_clear(true, true, Vec3::new(0.5, 0.5, 0.5));

        assert!(layer.clear);
        assert!(layer.clear_z);
        assert_eq!(layer.clear_color, Vec3::new(0.5, 0.5, 0.5));
    }
}
