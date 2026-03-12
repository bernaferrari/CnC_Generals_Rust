//! INI parsing for Model definitions
//!
//! This module handles parsing Model entries from INI files.
//! Model entries define 3D models that can be used in the game.
//!
//! Author: Colin Day, November 2001
//! Rust port: 2025

use crate::common::ini::ini::INI;
use crate::common::resource_manager::ResourceManager;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// 3D coordinate representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn one() -> Self {
        Self {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        }
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(&mut self) {
        let len = self.length();
        if len > 0.0 {
            self.x /= len;
            self.y /= len;
            self.z /= len;
        }
    }

    pub fn normalized(&self) -> Vector3D {
        let mut result = *self;
        result.normalize();
        result
    }
}

impl Default for Vector3D {
    fn default() -> Self {
        Self::zero()
    }
}

/// Quaternion representation for rotations
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quaternion {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn identity() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }

    pub fn from_euler(x: f32, y: f32, z: f32) -> Self {
        let cx = (x * 0.5).cos();
        let sx = (x * 0.5).sin();
        let cy = (y * 0.5).cos();
        let sy = (y * 0.5).sin();
        let cz = (z * 0.5).cos();
        let sz = (z * 0.5).sin();

        Self {
            w: cx * cy * cz + sx * sy * sz,
            x: sx * cy * cz - cx * sy * sz,
            y: cx * sy * cz + sx * cy * sz,
            z: cx * cy * sz - sx * sy * cz,
        }
    }
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::identity()
    }
}

/// Bounding box representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub min: Vector3D,
    pub max: Vector3D,
}

impl BoundingBox {
    pub fn new(min: Vector3D, max: Vector3D) -> Self {
        Self { min, max }
    }

    pub fn zero() -> Self {
        Self {
            min: Vector3D::zero(),
            max: Vector3D::zero(),
        }
    }

    pub fn get_size(&self) -> Vector3D {
        Vector3D::new(
            self.max.x - self.min.x,
            self.max.y - self.min.y,
            self.max.z - self.min.z,
        )
    }

    pub fn get_center(&self) -> Vector3D {
        Vector3D::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    pub fn contains(&self, point: Vector3D) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self::zero()
    }
}

/// Animation information for a model
#[derive(Debug, Clone)]
pub struct ModelAnimation {
    /// Name of the animation
    pub name: String,
    /// File path to animation data
    pub file_path: String,
    /// Duration of the animation in seconds
    pub duration: f32,
    /// Whether the animation loops
    pub is_looped: bool,
    /// Playback speed multiplier
    pub speed: f32,
    /// Priority for animation blending
    pub priority: i32,
}

impl ModelAnimation {
    pub fn new(name: String) -> Self {
        Self {
            name,
            file_path: String::new(),
            duration: 1.0,
            is_looped: false,
            speed: 1.0,
            priority: 0,
        }
    }
}

impl Default for ModelAnimation {
    fn default() -> Self {
        Self {
            name: String::new(),
            file_path: String::new(),
            duration: 1.0,
            is_looped: false,
            speed: 1.0,
            priority: 0,
        }
    }
}

/// Level of Detail (LOD) information
#[derive(Debug, Clone)]
pub struct ModelLOD {
    /// Distance at which this LOD becomes active
    pub distance: f32,
    /// Model file for this LOD level
    pub model_file: String,
    /// Texture file for this LOD level
    pub texture_file: String,
    /// Polygon count for this LOD
    pub polygon_count: i32,
}

impl ModelLOD {
    pub fn new(distance: f32, model_file: String) -> Self {
        Self {
            distance,
            model_file,
            texture_file: String::new(),
            polygon_count: 0,
        }
    }
}

impl Default for ModelLOD {
    fn default() -> Self {
        Self {
            distance: 0.0,
            model_file: String::new(),
            texture_file: String::new(),
            polygon_count: 0,
        }
    }
}

/// Material properties for a model
#[derive(Debug, Clone)]
pub struct ModelMaterial {
    /// Name of the material
    pub name: String,
    /// Diffuse texture file
    pub diffuse_texture: String,
    /// Normal map texture file
    pub normal_texture: String,
    /// Specular map texture file
    pub specular_texture: String,
    /// Emissive color
    pub emissive_color: Vector3D,
    /// Transparency value (0.0 = opaque, 1.0 = transparent)
    pub transparency: f32,
    /// Shininess factor
    pub shininess: f32,
    /// Whether this material uses alpha blending
    pub uses_alpha: bool,
}

impl ModelMaterial {
    pub fn new(name: String) -> Self {
        Self {
            name,
            diffuse_texture: String::new(),
            normal_texture: String::new(),
            specular_texture: String::new(),
            emissive_color: Vector3D::zero(),
            transparency: 0.0,
            shininess: 32.0,
            uses_alpha: false,
        }
    }
}

impl Default for ModelMaterial {
    fn default() -> Self {
        Self {
            name: String::new(),
            diffuse_texture: String::new(),
            normal_texture: String::new(),
            specular_texture: String::new(),
            emissive_color: Vector3D::zero(),
            transparency: 0.0,
            shininess: 32.0,
            uses_alpha: false,
        }
    }
}

/// Model definition for 3D models
///
/// Contains all information needed to load and use a 3D model in the game.
/// The original C++ file was empty, but we provide a complete structure
/// for future expansion and actual model loading functionality.
#[derive(Debug, Clone)]
pub struct Model {
    /// Model name/identifier
    pub name: String,
    /// Display name for UI
    pub display_name: String,
    /// Description of the model
    pub description: String,

    /// File paths
    pub model_file: String,
    pub texture_file: String,
    pub animation_file: String,

    /// Transform properties
    pub scale: Vector3D,
    pub rotation: Quaternion,
    pub position: Vector3D,

    /// Bounding information
    pub bounding_box: BoundingBox,
    pub bounding_radius: f32,

    /// Rendering properties
    pub materials: Vec<ModelMaterial>,
    pub lod_levels: Vec<ModelLOD>,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
    pub is_transparent: bool,
    pub render_priority: i32,

    /// Animation properties
    pub animations: Vec<ModelAnimation>,
    pub default_animation: String,
    pub animation_speed: f32,

    /// Performance and optimization
    pub polygon_count: i32,
    pub texture_memory_usage: u32,
    pub use_hardware_skinning: bool,
    pub max_bones: i32,

    /// Gameplay properties
    pub collision_enabled: bool,
    pub pickable: bool,
    pub selectable: bool,
    pub mass: f32,

    /// Custom properties
    pub custom_properties: HashMap<String, String>,

    /// Loading state
    pub is_loaded: bool,
    pub is_loading: bool,
    pub load_priority: i32,
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

impl Model {
    /// Create a new Model with default values
    pub fn new() -> Self {
        Self {
            name: String::new(),
            display_name: String::new(),
            description: String::new(),

            model_file: String::new(),
            texture_file: String::new(),
            animation_file: String::new(),

            scale: Vector3D::one(),
            rotation: Quaternion::identity(),
            position: Vector3D::zero(),

            bounding_box: BoundingBox::zero(),
            bounding_radius: 1.0,

            materials: Vec::new(),
            lod_levels: Vec::new(),
            cast_shadows: true,
            receive_shadows: true,
            is_transparent: false,
            render_priority: 0,

            animations: Vec::new(),
            default_animation: String::new(),
            animation_speed: 1.0,

            polygon_count: 0,
            texture_memory_usage: 0,
            use_hardware_skinning: true,
            max_bones: 64,

            collision_enabled: true,
            pickable: true,
            selectable: true,
            mass: 1.0,

            custom_properties: HashMap::new(),

            is_loaded: false,
            is_loading: false,
            load_priority: 0,
        }
    }

    /// Set the model name and display name
    pub fn set_name(&mut self, name: String) {
        self.name = name.clone();
        if self.display_name.is_empty() {
            self.display_name = name;
        }
    }

    /// Add a material to the model
    pub fn add_material(&mut self, material: ModelMaterial) {
        self.materials.push(material);
    }

    /// Add a LOD level to the model
    pub fn add_lod_level(&mut self, lod: ModelLOD) {
        self.lod_levels.push(lod);
        // Keep LOD levels sorted by distance
        self.lod_levels
            .sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
    }

    /// Add an animation to the model
    pub fn add_animation(&mut self, animation: ModelAnimation) {
        self.animations.push(animation);
    }

    /// Get an animation by name
    pub fn get_animation(&self, name: &str) -> Option<&ModelAnimation> {
        self.animations.iter().find(|anim| anim.name == name)
    }

    /// Get a material by name
    pub fn get_material(&self, name: &str) -> Option<&ModelMaterial> {
        self.materials.iter().find(|mat| mat.name == name)
    }

    /// Get the appropriate LOD level for a given distance
    pub fn get_lod_for_distance(&self, distance: f32) -> Option<&ModelLOD> {
        self.lod_levels
            .iter()
            .rev() // Start from highest LOD
            .find(|lod| distance >= lod.distance)
    }

    /// Set a custom property
    pub fn set_custom_property(&mut self, key: String, value: String) {
        self.custom_properties.insert(key, value);
    }

    /// Get a custom property
    pub fn get_custom_property(&self, key: &str) -> Option<&String> {
        self.custom_properties.get(key)
    }

    /// Check if this is a valid model configuration
    pub fn is_valid(&self) -> bool {
        !self.name.is_empty() && !self.model_file.is_empty()
    }

    /// Calculate total texture memory usage
    pub fn calculate_texture_memory(&self) -> u32 {
        // This would calculate actual texture memory usage in a real implementation
        self.texture_memory_usage
    }

    /// Check if model supports animations
    pub fn has_animations(&self) -> bool {
        !self.animations.is_empty()
    }

    /// Check if model has multiple LOD levels
    pub fn has_lod(&self) -> bool {
        self.lod_levels.len() > 1
    }

    /// Get estimated loading time (placeholder)
    pub fn get_estimated_load_time(&self) -> f32 {
        // Basic estimation based on polygon count and texture usage
        let base_time = 0.1; // Base load time in seconds
        let poly_factor = self.polygon_count as f32 * 0.00001;
        let texture_factor = self.texture_memory_usage as f32 * 0.000001;
        base_time + poly_factor + texture_factor
    }

    /// Load the model using the resource manager.
    pub fn load(&mut self) -> Result<(), String> {
        if self.is_loaded {
            return Ok(());
        }

        if !self.is_valid() {
            return Err("Invalid model configuration".to_string());
        }

        self.is_loading = true;

        let resource_manager = ResourceManager::new();
        if self.model_file.is_empty() {
            self.is_loading = false;
            return Err("Model file not specified".to_string());
        }

        let model_resource = resource_manager
            .load_resource(&self.model_file)
            .map_err(|err| format!("Failed to load model '{}': {}", self.model_file, err))?;

        if !self.texture_file.is_empty() {
            if let Ok(texture_resource) = resource_manager.load_resource(&self.texture_file) {
                self.texture_memory_usage = texture_resource.data.len() as u32;
            }
        }

        // Basic load bookkeeping using actual resource payload sizes.
        self.polygon_count = self.polygon_count.max(0);
        self.is_loaded = true;
        self.is_loading = false;

        let _ = model_resource;
        Ok(())
    }

    /// Unload the model
    pub fn unload(&mut self) {
        if self.is_loaded {
            println!("Unloading model: {}", self.name);
            self.is_loaded = false;
        }
    }

    /// Parse model fields from an INI block.
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), String> {
        loop {
            ini.read_line().map_err(|error| error.to_string())?;
            if ini.is_eof() {
                return Err("Unexpected EOF while parsing Model block".to_string());
            }

            let tokens = ini.get_line_tokens();
            let Some(key) = tokens.first().copied() else {
                continue;
            };
            if key.eq_ignore_ascii_case("End") {
                break;
            }

            let values: Vec<&str> = tokens
                .iter()
                .skip(1)
                .copied()
                .filter(|token| *token != "=")
                .collect();
            if values.is_empty() {
                continue;
            }

            let key_lc = key.to_ascii_lowercase();
            match key_lc.as_str() {
                "name" => self.set_name(values.join(" ")),
                "displayname" => self.display_name = values.join(" "),
                "description" => self.description = values.join(" "),
                "modelfile" | "model" | "filename" => self.model_file = values.join(" "),
                "texturefile" | "texture" => self.texture_file = values.join(" "),
                "animationfile" => self.animation_file = values.join(" "),
                "defaultanimation" => self.default_animation = values.join(" "),
                "castshadows" => {
                    if let Ok(v) = INI::parse_bool(values[0]) {
                        self.cast_shadows = v;
                    }
                }
                "receiveshadows" => {
                    if let Ok(v) = INI::parse_bool(values[0]) {
                        self.receive_shadows = v;
                    }
                }
                "istransparent" => {
                    if let Ok(v) = INI::parse_bool(values[0]) {
                        self.is_transparent = v;
                    }
                }
                "collisionenabled" => {
                    if let Ok(v) = INI::parse_bool(values[0]) {
                        self.collision_enabled = v;
                    }
                }
                "pickable" => {
                    if let Ok(v) = INI::parse_bool(values[0]) {
                        self.pickable = v;
                    }
                }
                "selectable" => {
                    if let Ok(v) = INI::parse_bool(values[0]) {
                        self.selectable = v;
                    }
                }
                "renderpriority" => {
                    if let Ok(v) = INI::parse_int(values[0]) {
                        self.render_priority = v;
                    }
                }
                "polygoncount" => {
                    if let Ok(v) = INI::parse_int(values[0]) {
                        self.polygon_count = v.max(0);
                    }
                }
                "texturememoryusage" => {
                    if let Ok(v) = INI::parse_unsigned_int(values[0]) {
                        self.texture_memory_usage = v;
                    }
                }
                "animationspeed" => {
                    if let Ok(v) = INI::parse_real(values[0]) {
                        self.animation_speed = v.max(0.0);
                    }
                }
                "boundingradius" => {
                    if let Ok(v) = INI::parse_real(values[0]) {
                        self.bounding_radius = v.max(0.0);
                    }
                }
                "mass" => {
                    if let Ok(v) = INI::parse_real(values[0]) {
                        self.mass = v.max(0.0);
                    }
                }
                "loadpriority" => {
                    if let Ok(v) = INI::parse_int(values[0]) {
                        self.load_priority = v;
                    }
                }
                "maxbones" => {
                    if let Ok(v) = INI::parse_int(values[0]) {
                        self.max_bones = v.max(0);
                    }
                }
                "scale" => {
                    if let Ok((x, y, z)) = INI::parse_coord_3d(&values) {
                        self.scale = Vector3D::new(x, y, z);
                    }
                }
                "position" => {
                    if let Ok((x, y, z)) = INI::parse_coord_3d(&values) {
                        self.position = Vector3D::new(x, y, z);
                    }
                }
                _ => {
                    // Keep parity with C++ loader behavior by ignoring unknown fields.
                }
            }
        }

        Ok(())
    }
}

/// Model manager for loading and managing 3D models
#[derive(Debug)]
pub struct ModelManager {
    /// Map of loaded models by name
    models: HashMap<String, Model>,
    /// Loading queue
    loading_queue: Vec<String>,
    /// Maximum number of models to keep loaded
    max_loaded_models: usize,
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelManager {
    /// Create a new ModelManager
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            loading_queue: Vec::new(),
            max_loaded_models: 1000,
        }
    }

    /// Initialize the model manager
    pub fn init(&mut self) {
        self.models.clear();
        self.loading_queue.clear();
        println!("ModelManager initialized");
    }

    /// Reset the model manager
    pub fn reset(&mut self) {
        // Unload all models
        for model in self.models.values_mut() {
            model.unload();
        }
        self.models.clear();
        self.loading_queue.clear();
        println!("ModelManager reset");
    }

    /// Update the model manager (called per frame)
    pub fn update(&mut self) {
        // Process loading queue
        if !self.loading_queue.is_empty() {
            let model_name = self.loading_queue.remove(0);
            if let Some(model) = self.models.get_mut(&model_name) {
                if let Err(e) = model.load() {
                    println!("Failed to load model {}: {}", model_name, e);
                }
            }
        }

        // Manage memory usage if needed
        if self.models.len() > self.max_loaded_models {
            self.cleanup_unused_models();
        }
    }

    /// Add a model to the manager
    pub fn add_model(&mut self, model: Model) {
        let name = model.name.clone();
        self.models.insert(name, model);
    }

    /// Clear all tracked models and pending loads
    pub fn clear(&mut self) {
        self.models.clear();
        self.loading_queue.clear();
    }

    /// Get a model by name
    pub fn get_model(&self, name: &str) -> Option<&Model> {
        self.models.get(name)
    }

    /// Get a mutable model by name
    pub fn get_model_mut(&mut self, name: &str) -> Option<&mut Model> {
        self.models.get_mut(name)
    }

    /// Load a model (add to loading queue if not immediate)
    pub fn load_model(&mut self, name: &str, immediate: bool) -> Result<(), String> {
        if let Some(model) = self.models.get_mut(name) {
            if immediate {
                model.load()
            } else {
                if !model.is_loaded && !model.is_loading {
                    self.loading_queue.push(name.to_string());
                }
                Ok(())
            }
        } else {
            Err(format!("Model '{}' not found", name))
        }
    }

    /// Unload a model
    pub fn unload_model(&mut self, name: &str) {
        if let Some(model) = self.models.get_mut(name) {
            model.unload();
        }
    }

    /// Remove a model from the manager
    pub fn remove_model(&mut self, name: &str) -> Option<Model> {
        self.models.remove(name)
    }

    /// Get all model names
    pub fn get_model_names(&self) -> Vec<&String> {
        self.models.keys().collect()
    }

    /// Get loaded model count
    pub fn get_loaded_count(&self) -> usize {
        self.models.values().filter(|m| m.is_loaded).count()
    }

    /// Get total memory usage
    pub fn get_memory_usage(&self) -> u32 {
        self.models
            .values()
            .filter(|m| m.is_loaded)
            .map(|m| m.calculate_texture_memory())
            .sum()
    }

    /// Cleanup unused models
    fn cleanup_unused_models(&mut self) {
        let mut models_to_unload: Vec<String> = Vec::new();

        for (name, model) in &self.models {
            if model.is_loaded && model.load_priority < 0 {
                models_to_unload.push(name.clone());
            }
        }

        // Unload lowest priority models
        for name in models_to_unload {
            self.unload_model(&name);
        }
    }

    /// Get models by criteria
    pub fn find_models_with_animation(&self, animation_name: &str) -> Vec<&Model> {
        self.models
            .values()
            .filter(|model| model.get_animation(animation_name).is_some())
            .collect()
    }

    /// Get models with LOD support
    pub fn get_lod_models(&self) -> Vec<&Model> {
        self.models
            .values()
            .filter(|model| model.has_lod())
            .collect()
    }
}

/// Global model manager instance (thread-safe)
static MODEL_MANAGER: OnceCell<Arc<RwLock<ModelManager>>> = OnceCell::new();

/// Ensure the model manager exists and return a handle to it
pub fn ensure_model_manager() -> Arc<RwLock<ModelManager>> {
    MODEL_MANAGER
        .get_or_init(|| {
            let mut manager = ModelManager::new();
            manager.init();
            Arc::new(RwLock::new(manager))
        })
        .clone()
}

/// Initialize (or reinitialize) the global model manager
pub fn init_global_model_manager() {
    let manager = ensure_model_manager();
    manager.write().init();
}

/// Get a handle to the model manager if it has been initialized
pub fn get_model_manager() -> Option<Arc<RwLock<ModelManager>>> {
    MODEL_MANAGER.get().cloned()
}

/// INI parsing function for Model definition (matches C++ interface)
///
/// This is the main entry point for parsing Model definitions from INI files.
pub fn parse_model_definition(ini: &mut INI) -> Result<(), String> {
    let tokens = ini.get_line_tokens();
    let model_name = tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .map(|token| token.to_string())
        .unwrap_or_default();

    let mut model = Model::new();
    if !model_name.is_empty() {
        model.set_name(model_name);
    }
    model.parse_from_ini(ini)?;

    let manager = ensure_model_manager();
    manager.write().add_model(model);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector3d() {
        let v1 = Vector3D::new(3.0, 4.0, 0.0);
        assert_eq!(v1.length(), 5.0);

        let v2 = v1.normalized();
        assert!((v2.length() - 1.0).abs() < 0.001);

        let zero = Vector3D::zero();
        assert_eq!(zero.x, 0.0);
        assert_eq!(zero.y, 0.0);
        assert_eq!(zero.z, 0.0);

        let one = Vector3D::one();
        assert_eq!(one.x, 1.0);
        assert_eq!(one.y, 1.0);
        assert_eq!(one.z, 1.0);
    }

    #[test]
    fn test_quaternion() {
        let identity = Quaternion::identity();
        assert_eq!(identity.w, 1.0);
        assert_eq!(identity.x, 0.0);

        let euler_quat = Quaternion::from_euler(0.0, 0.0, std::f32::consts::PI / 2.0);
        assert!((euler_quat.z - 0.707).abs() < 0.01);
    }

    #[test]
    fn test_bounding_box() {
        let bbox = BoundingBox::new(
            Vector3D::new(-1.0, -1.0, -1.0),
            Vector3D::new(1.0, 1.0, 1.0),
        );

        let size = bbox.get_size();
        assert_eq!(size.x, 2.0);
        assert_eq!(size.y, 2.0);
        assert_eq!(size.z, 2.0);

        let center = bbox.get_center();
        assert_eq!(center.x, 0.0);
        assert_eq!(center.y, 0.0);
        assert_eq!(center.z, 0.0);

        assert!(bbox.contains(Vector3D::zero()));
        assert!(!bbox.contains(Vector3D::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn test_model_creation() {
        let mut model = Model::new();
        assert!(model.name.is_empty());
        assert!(!model.is_valid());

        model.set_name("test_model".to_string());
        model.model_file = "test_model.w3d".to_string();

        assert_eq!(model.name, "test_model");
        assert_eq!(model.display_name, "test_model");
        assert!(model.is_valid());
        assert!(!model.is_loaded);
    }

    #[test]
    fn test_model_materials() {
        let mut model = Model::new();
        let material = ModelMaterial::new("test_material".to_string());

        model.add_material(material);
        assert_eq!(model.materials.len(), 1);

        let found_material = model.get_material("test_material");
        assert!(found_material.is_some());
        assert_eq!(found_material.unwrap().name, "test_material");

        assert!(model.get_material("nonexistent").is_none());
    }

    #[test]
    fn test_model_lod() {
        let mut model = Model::new();
        assert!(!model.has_lod());

        model.add_lod_level(ModelLOD::new(100.0, "model_lod0.w3d".to_string()));
        model.add_lod_level(ModelLOD::new(50.0, "model_lod1.w3d".to_string()));
        model.add_lod_level(ModelLOD::new(200.0, "model_lod2.w3d".to_string()));

        assert!(model.has_lod());
        assert_eq!(model.lod_levels.len(), 3);

        // Should be sorted by distance
        assert_eq!(model.lod_levels[0].distance, 50.0);
        assert_eq!(model.lod_levels[1].distance, 100.0);
        assert_eq!(model.lod_levels[2].distance, 200.0);

        // Test LOD selection
        let lod = model.get_lod_for_distance(75.0);
        assert!(lod.is_some());
        assert_eq!(lod.unwrap().distance, 50.0);

        let lod = model.get_lod_for_distance(150.0);
        assert!(lod.is_some());
        assert_eq!(lod.unwrap().distance, 100.0);
    }

    #[test]
    fn test_model_animations() {
        let mut model = Model::new();
        assert!(!model.has_animations());

        let mut animation = ModelAnimation::new("walk".to_string());
        animation.duration = 2.0;
        animation.is_looped = true;

        model.add_animation(animation);
        assert!(model.has_animations());
        assert_eq!(model.animations.len(), 1);

        let found_anim = model.get_animation("walk");
        assert!(found_anim.is_some());
        assert_eq!(found_anim.unwrap().duration, 2.0);
        assert!(found_anim.unwrap().is_looped);

        assert!(model.get_animation("run").is_none());
    }

    #[test]
    fn test_model_custom_properties() {
        let mut model = Model::new();

        model.set_custom_property("faction".to_string(), "USA".to_string());
        model.set_custom_property("category".to_string(), "vehicle".to_string());

        assert_eq!(
            model.get_custom_property("faction"),
            Some(&"USA".to_string())
        );
        assert_eq!(
            model.get_custom_property("category"),
            Some(&"vehicle".to_string())
        );
        assert_eq!(model.get_custom_property("nonexistent"), None);
    }

    #[test]
    fn test_model_loading() {
        let mut model = Model::new();
        model.set_name("test_model".to_string());
        model.model_file = "test.w3d".to_string();

        assert!(!model.is_loaded);
        assert!(!model.is_loading);

        let result = model.load();
        assert!(result.is_ok());
        assert!(model.is_loaded);
        assert!(!model.is_loading);

        model.unload();
        assert!(!model.is_loaded);
    }

    #[test]
    fn test_model_manager() {
        let mut manager = ModelManager::new();
        manager.init();

        assert_eq!(manager.get_loaded_count(), 0);
        assert!(manager.get_model_names().is_empty());

        let mut model = Model::new();
        model.set_name("test_model".to_string());
        model.model_file = "test.w3d".to_string();

        manager.add_model(model);
        assert_eq!(manager.get_model_names().len(), 1);

        let retrieved = manager.get_model("test_model");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test_model");

        let result = manager.load_model("test_model", true);
        assert!(result.is_ok());
        assert_eq!(manager.get_loaded_count(), 1);

        manager.unload_model("test_model");
        assert_eq!(manager.get_loaded_count(), 0);
    }

    #[test]
    fn test_model_manager_queries() {
        let mut manager = ModelManager::new();

        let mut model1 = Model::new();
        model1.set_name("model1".to_string());
        model1.add_animation(ModelAnimation::new("walk".to_string()));

        let mut model2 = Model::new();
        model2.set_name("model2".to_string());
        model2.add_lod_level(ModelLOD::new(50.0, "lod0.w3d".to_string()));
        model2.add_lod_level(ModelLOD::new(100.0, "lod1.w3d".to_string()));

        manager.add_model(model1);
        manager.add_model(model2);

        let animated_models = manager.find_models_with_animation("walk");
        assert_eq!(animated_models.len(), 1);
        assert_eq!(animated_models[0].name, "model1");

        let lod_models = manager.get_lod_models();
        assert_eq!(lod_models.len(), 1);
        assert_eq!(lod_models[0].name, "model2");
    }

    #[test]
    fn test_global_model_manager() {
        init_global_model_manager();

        let manager_handle = ensure_model_manager();
        {
            let mut manager = manager_handle.write();
            manager.clear();

            let mut model = Model::new();
            model.set_name("global_test".to_string());
            manager.add_model(model);
        }

        let manager = manager_handle.read();
        assert_eq!(manager.get_model_names().len(), 1);
        assert!(manager.get_model("global_test").is_some());
    }
}
