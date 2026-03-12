//! Dazzle Effect System - Advanced lighting effects and glare
//!
//! This module implements the Dazzle system from the original C++ code,
//! providing sophisticated lighting effects including halos and dazzle effects.
//!
//! Converted from:
//! - dazzle.cpp/h (dazzle effect system)
//! - dazzle.ini (effect configurations)

use configparser::ini::Ini;
use glam::{Mat4, Vec3, Vec4};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use ww3d_core::errors::{W3DError, W3DResult};
use ww3d_renderer_3d::{
    core::error::{Error as RendererError, RendererResult},
    render_object_system::{
        AABoxClass, AABoxCollisionTestClass, AABoxIntersectionTestClass, DecalGeneratorClass,
        MaterialInfoClass, OBBoxCollisionTestClass, OBBoxIntersectionTestClass,
        RayCollisionTestClass, RenderInfoClass, RenderObjClass, RenderObjClassId,
        SpecialRenderInfoClass, SphereClass,
    },
    rendering::camera_system::camera::CameraClass,
};

type Result<T> = W3DResult<T>;

/// Dazzle type enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DazzleType {
    /// Standard dazzle effect
    Standard = 0,
    /// Halo effect
    Halo,
    /// Lens flare effect
    LensFlare,
    /// Star burst effect
    StarBurst,
    /// Custom effect
    Custom,
}

/// Dazzle render mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DazzleRenderMode {
    /// Additive blending
    Additive = 0,
    /// Screen blending
    Screen,
    /// Multiply blending
    Multiply,
    /// Alpha blending
    Alpha,
}

/// Dazzle configuration structure
#[derive(Debug, Clone)]
pub struct DazzleConfig {
    /// Dazzle name
    pub name: String,
    /// Dazzle type
    pub dazzle_type: DazzleType,
    /// Render mode
    pub render_mode: DazzleRenderMode,
    /// Texture filename
    pub texture_name: String,
    /// Halo texture filename
    pub halo_texture_name: String,
    /// Linked lens flare definition
    pub lensflare_name: Option<String>,
    /// Base intensity
    pub intensity: f32,
    /// Base size scale
    pub size: f32,
    /// Intensity power factor
    pub intensity_pow: f32,
    /// Size power factor
    pub size_pow: f32,
    /// Effect area (angle in radians)
    pub area: f32,
    /// Halo area (angle in radians)
    pub halo_area: f32,
    /// Scale factors
    pub scale_x: f32,
    pub scale_y: f32,
    /// Halo scale factors
    pub halo_scale_x: f32,
    pub halo_scale_y: f32,
    /// Direction area for directional effects
    pub direction_area: f32,
    /// Direction vector for directional effects
    pub direction: Vec3,
    /// Color tint
    pub color: Vec4,
    /// Halo color tint
    pub halo_color: Vec4,
    /// Animation parameters
    pub animation_speed: f32,
    pub animation_amplitude: f32,
    /// Fade out distances
    pub fadeout_start: f32,
    pub fadeout_end: f32,
    /// Historical smoothing weight
    pub history_weight: f32,
    /// Collision radius for occlusion tests
    pub radius: f32,
    /// Blink controls
    pub blink_period: f32,
    pub blink_on_time: f32,
    /// Whether to include camera translation when evaluating visibility
    pub use_camera_translation: bool,
}

impl Default for DazzleConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            dazzle_type: DazzleType::Standard,
            render_mode: DazzleRenderMode::Additive,
            texture_name: String::new(),
            halo_texture_name: String::new(),
            lensflare_name: None,
            intensity: 1.0,
            size: 1.0,
            intensity_pow: 2.0,
            size_pow: 1.0,
            area: 0.1,
            halo_area: 0.2,
            scale_x: 1.0,
            scale_y: 1.0,
            halo_scale_x: 1.0,
            halo_scale_y: 1.0,
            direction_area: 0.0,
            direction: Vec3::new(0.0, 0.0, -1.0),
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            halo_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            animation_speed: 0.0,
            animation_amplitude: 0.0,
            fadeout_start: 0.0,
            fadeout_end: 0.0,
            history_weight: 0.0,
            radius: 1.0,
            blink_period: 0.0,
            blink_on_time: 0.0,
            use_camera_translation: true,
        }
    }
}

/// Dazzle instance bound to a render object or scene node.
#[derive(Debug, Clone)]
pub struct DazzleInstance {
    pub name: String,
    pub type_name: String,
    pub config: DazzleConfig,
    pub transform: Mat4,
    pub animation_phase: f32,
    pub visible: bool,
    pub computed_intensity: f32,
    pub computed_size: f32,
    last_update_frame: u64,
}

impl DazzleInstance {
    pub fn new(
        name: impl Into<String>,
        type_name: impl Into<String>,
        config: DazzleConfig,
        transform: Mat4,
        frame_index: u64,
    ) -> Self {
        let base_size = (config.scale_x + config.scale_y) * 0.5;
        let name = name.into();
        let type_original = type_name.into();
        let normalized_type = normalize_key(&type_original);
        Self {
            name,
            type_name: normalized_type,
            config,
            transform,
            animation_phase: 0.0,
            visible: false,
            computed_intensity: 0.0,
            computed_size: base_size,
            last_update_frame: frame_index,
        }
    }

    pub fn mark_updated(&mut self, frame_index: u64) {
        self.last_update_frame = frame_index;
    }

    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    pub fn evaluate(&mut self, camera: &CameraClass, delta_time: f32) {
        if self.config.animation_speed > 0.0 {
            self.animation_phase = (self.animation_phase
                + self.config.animation_speed * delta_time)
                % std::f32::consts::TAU;
        }

        let position = self.transform.w_axis.truncate();
        let camera_position = if self.config.use_camera_translation {
            camera.get_position()
        } else {
            Vec3::ZERO
        };
        let to_dazzle = position - camera_position;
        let distance = to_dazzle.length();

        if distance <= f32::EPSILON {
            self.visible = false;
            return;
        }

        let direction_to_dazzle = to_dazzle / distance;
        let camera_direction = camera.get_forward();

        if self.config.direction_area > 0.0 {
            let mut bone_direction = (-self.transform.z_axis.truncate()).normalize_or_zero();
            if bone_direction.length_squared() <= f32::EPSILON {
                bone_direction = self.config.direction.normalize_or_zero();
            }
            let direction_dot = bone_direction.dot(-direction_to_dazzle).clamp(-1.0, 1.0);
            if direction_dot < self.config.direction_area {
                self.visible = false;
                return;
            }
        }

        let angle = camera_direction
            .dot(direction_to_dazzle)
            .clamp(-1.0, 1.0)
            .acos();
        let normalized_angle = (angle / self.config.area.max(f32::EPSILON)).clamp(0.0, 1.0);

        if normalized_angle >= 1.0 {
            self.visible = false;
            return;
        }

        let intensity_factor = (1.0 - normalized_angle).powf(self.config.intensity_pow);
        let size_factor = (1.0 - normalized_angle).powf(self.config.size_pow);
        let animation_factor = if self.config.animation_amplitude > 0.0 {
            1.0 + self.config.animation_amplitude * (self.animation_phase * 2.0).sin()
        } else {
            1.0
        };

        self.computed_intensity = self.config.intensity * intensity_factor * animation_factor;
        let base_size = (self.config.scale_x + self.config.scale_y) * 0.5;
        self.computed_size = base_size * size_factor * animation_factor;

        if self.config.fadeout_end > self.config.fadeout_start && self.config.fadeout_end > 0.0 {
            let fade_range =
                (self.config.fadeout_end - self.config.fadeout_start).max(f32::EPSILON);
            let fade_factor = if distance <= self.config.fadeout_start {
                1.0
            } else if distance >= self.config.fadeout_end {
                0.0
            } else {
                1.0 - ((distance - self.config.fadeout_start) / fade_range)
            };
            self.computed_intensity *= fade_factor;
            self.computed_size *= fade_factor;
        }

        self.computed_size = self.computed_size.max(0.0);
        self.visible = self.computed_intensity > 0.01;
    }
}

/// Dazzle manager class - manages all dazzle effects
#[derive(Debug)]
pub struct DazzleManager {
    /// Dazzle configurations indexed by normalized name
    pub configs: HashMap<String, DazzleConfig>,
    /// Lens flare presets referenced by dazzles
    pub lensflare_configs: HashMap<String, LensFlareConfig>,
    /// Active dazzle instances keyed by render object name
    instances: HashMap<String, DazzleInstance>,
    /// Maximum number of dazzles to render
    pub max_dazzles: usize,
    /// Global dazzle intensity multiplier
    pub global_intensity: f32,
    /// Global dazzle size multiplier
    pub global_size: f32,
    /// Whether dazzle system is enabled
    pub enabled: bool,
    frame_index: u64,
    current_delta_time: f32,
}

impl DazzleManager {
    /// Create new dazzle manager
    pub fn new(_device: &wgpu::Device, _queue: &wgpu::Queue) -> Self {
        Self {
            configs: HashMap::new(),
            lensflare_configs: HashMap::new(),
            instances: HashMap::new(),
            max_dazzles: 64,
            global_intensity: 1.0,
            global_size: 1.0,
            enabled: true,
            frame_index: 0,
            current_delta_time: 0.0,
        }
    }

    /// Begin a new frame for the dazzle system
    pub fn begin_frame(&mut self, delta_time: f32) {
        self.frame_index = self.frame_index.wrapping_add(1);
        self.current_delta_time = delta_time;
    }

    /// Load dazzle configurations from an INI file
    pub fn load_configs_from_ini<P: AsRef<Path>>(&mut self, filename: P) -> Result<()> {
        let path = filename.as_ref();
        let mut ini = Ini::new();
        ini.load(&path.to_string_lossy())
            .map_err(W3DError::IoError)?;
        let map = ini.get_map_ref();

        self.configs.clear();
        self.lensflare_configs.clear();

        for lensflare_name in extract_list(map, "lensflares_list") {
            if let Some(config) = self.parse_lensflare_config(map, &lensflare_name)? {
                self.lensflare_configs
                    .insert(normalize_key(&lensflare_name), config);
            }
        }

        let mut loaded = false;
        for dazzle_name in extract_list(map, "dazzles_list") {
            let config = self.parse_dazzle_config(map, &dazzle_name)?;
            let key = normalize_key(&config.name);
            self.configs.insert(key, config);
            loaded = true;
        }

        if !loaded {
            let mut default_config = DazzleConfig::default();
            default_config.name = "DEFAULT".to_string();
            self.configs
                .insert(normalize_key(&default_config.name), default_config);
        }

        Ok(())
    }

    /// Queue a dazzle instance for this frame
    pub fn queue_dazzle_instance(
        &mut self,
        name: &str,
        type_name: &str,
        transform: Mat4,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let key = normalize_key(type_name);
        let config = self.configs.get(&key).cloned().ok_or_else(|| {
            W3DError::InvalidParameter(format!("Dazzle config '{}' not found", type_name))
        })?;

        let entry = self.instances.entry(name.to_string()).or_insert_with(|| {
            DazzleInstance::new(
                name,
                key.clone(),
                config.clone(),
                transform,
                self.frame_index,
            )
        });

        entry.type_name = key;
        entry.config = config;
        entry.set_transform(transform);
        entry.mark_updated(self.frame_index);
        Ok(())
    }

    /// Convenience helper for legacy callers
    pub fn create_dazzle(&mut self, config_name: &str, position: Vec3) -> Result<usize> {
        let handle = format!("{}_{}", normalize_key(config_name), self.instances.len());
        self.queue_dazzle_instance(&handle, config_name, Mat4::from_translation(position))?;
        Ok(self.instances.len())
    }

    /// Remove a dazzle instance by name
    pub fn remove_dazzle(&mut self, name: &str) {
        self.instances.remove(name);
    }

    /// Update frame timing (instances are evaluated during render)
    pub fn update(&mut self, _camera: &CameraClass, delta_time: f32) {
        if !self.enabled {
            return;
        }
        self.begin_frame(delta_time);
    }

    fn evaluate_instances(&mut self, camera: &CameraClass) {
        let frame = self.frame_index;
        let delta = self.current_delta_time;
        for instance in self.instances.values_mut() {
            if instance.last_update_frame == frame {
                instance.evaluate(camera, delta);
            } else {
                instance.visible = false;
            }
        }
    }

    fn finalize_frame(&mut self) {
        let frame = self.frame_index;
        self.instances
            .retain(|_, instance| instance.last_update_frame == frame);
    }

    /// Render all dazzle instances for the current frame
    pub fn render(&mut self, render_info: &RenderInfoClass) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        self.evaluate_instances(&render_info.camera);

        let mut keys: Vec<String> = self.instances.keys().cloned().collect();
        keys.sort();

        let mut rendered = 0;
        for key in keys {
            if rendered >= self.max_dazzles {
                break;
            }
            if let Some(instance) = self.instances.get_mut(&key) {
                if instance.visible {
                    instance.computed_intensity *= self.global_intensity;
                    instance.computed_size *= self.global_size;
                    rendered += 1;
                }
            }
        }

        self.finalize_frame();
        Ok(())
    }

    /// Render helper for camera-only contexts
    pub fn render_with_camera(&mut self, camera: &CameraClass) {
        if !self.enabled {
            return;
        }
        self.evaluate_instances(camera);
        let mut keys: Vec<String> = self.instances.keys().cloned().collect();
        keys.sort();
        let mut rendered = 0;
        for key in keys {
            if rendered >= self.max_dazzles {
                break;
            }
            if let Some(instance) = self.instances.get_mut(&key) {
                if instance.visible {
                    rendered += 1;
                }
            }
        }
        self.finalize_frame();
    }

    /// Convenience helper to flash the screen with a dazzle
    pub fn create_screen_flash(&mut self, color: Vec3, intensity: f32, _duration: f32) {
        let mut config = DazzleConfig::default();
        config.name = format!("SCREEN_FLASH_{}", self.configs.len());
        config.intensity = intensity;
        config.scale_x = 1.0;
        config.scale_y = 1.0;
        config.size = 1.0;
        config.color = Vec4::new(color.x, color.y, color.z, 1.0);
        config.halo_color = Vec4::new(color.x, color.y, color.z, 1.0);
        config.dazzle_type = DazzleType::Custom;

        let config_key = normalize_key(&config.name);
        self.configs.insert(config_key.clone(), config);

        let _ = self.create_dazzle(&config_key, Vec3::ZERO);
    }

    /// Clear all dazzle instances
    pub fn clear(&mut self) {
        self.instances.clear();
    }

    /// Set maximum number of dazzles to render
    pub fn set_max_dazzles(&mut self, max: usize) {
        self.max_dazzles = max;
    }

    /// Set global intensity multiplier
    pub fn set_global_intensity(&mut self, intensity: f32) {
        self.global_intensity = intensity;
    }

    /// Set global size multiplier
    pub fn set_global_size(&mut self, size: f32) {
        self.global_size = size;
    }

    /// Enable/disable dazzle system
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get dazzle statistics
    pub fn get_stats(&self) -> DazzleStats {
        let visible_instances = self.instances.values().filter(|i| i.visible).count();

        DazzleStats {
            total_instances: self.instances.len(),
            visible_instances,
            max_instances: self.max_dazzles,
            global_intensity: self.global_intensity,
            global_size: self.global_size,
            enabled: self.enabled,
        }
    }

    fn parse_lensflare_config(
        &self,
        map: &HashMap<String, HashMap<String, Option<String>>>,
        name: &str,
    ) -> Result<Option<LensFlareConfig>> {
        let props = match map.get(&name.to_ascii_lowercase()) {
            Some(props) => props,
            None => return Ok(None),
        };

        let mut config = LensFlareConfig::default();
        config.name = name.to_string();
        config.texture_name = get_prop(props, "TextureName")
            .map(|s| s.to_string())
            .unwrap_or_default();

        let flare_count = parse_i32(get_prop(props, "FlareCount"), 0).max(0) as usize;
        let mut elements = Vec::with_capacity(flare_count);
        for index in 1..=flare_count {
            let location = parse_f32(get_prop(props, &format!("FlareLocation{}", index)), 0.0);
            let size = parse_f32(get_prop(props, &format!("FlareSize{}", index)), 0.1);
            let color = parse_vec3(
                get_prop(props, &format!("FlareColor{}", index)),
                Vec3::new(1.0, 1.0, 1.0),
            );
            let uv = parse_vec4(
                get_prop(props, &format!("FlareUV{}", index)),
                Vec4::new(0.0, 0.0, 1.0, 1.0),
            );

            elements.push(LensFlareElement {
                location,
                size,
                color,
                uv,
            });
        }

        config.elements = elements;
        Ok(Some(config))
    }

    fn parse_dazzle_config(
        &self,
        map: &HashMap<String, HashMap<String, Option<String>>>,
        name: &str,
    ) -> Result<DazzleConfig> {
        let props = map.get(&name.to_ascii_lowercase()).ok_or_else(|| {
            W3DError::InvalidParameter(format!("Dazzle '{}' missing section", name))
        })?;

        let mut config = DazzleConfig::default();
        config.name = name.to_string();
        config.texture_name = get_prop(props, "DazzleTextureName")
            .map(|s| s.to_string())
            .unwrap_or_default();
        config.halo_texture_name = get_prop(props, "HaloTextureName")
            .map(|s| s.to_string())
            .unwrap_or_default();
        config.lensflare_name = get_prop(props, "LensflareName")
            .map(normalize_key)
            .filter(|s| !s.is_empty());

        config.intensity = parse_f32(get_prop(props, "DazzleIntensity"), config.intensity);
        config.intensity_pow =
            parse_f32(get_prop(props, "DazzleIntensityPow"), config.intensity_pow);
        config.size_pow = parse_f32(get_prop(props, "DazzleSizePow"), config.size_pow);
        config.area = parse_f32(get_prop(props, "DazzleArea"), config.area);
        config.scale_x = parse_f32(get_prop(props, "DazzleScaleX"), config.scale_x);
        config.scale_y = parse_f32(get_prop(props, "DazzleScaleY"), config.scale_y);
        config.size = (config.scale_x + config.scale_y) * 0.5;
        config.color = parse_vec4(get_prop(props, "DazzleColor"), config.color);
        config.direction_area = parse_f32(
            get_prop(props, "DazzleDirectionArea"),
            config.direction_area,
        );
        config.direction = parse_vec3(get_prop(props, "DazzleDirection"), config.direction);
        config.animation_speed =
            parse_f32(get_prop(props, "AnimationSpeed"), config.animation_speed);
        config.animation_amplitude = parse_f32(
            get_prop(props, "AnimationAmplitude"),
            config.animation_amplitude,
        );

        config.halo_area = parse_f32(get_prop(props, "HaloArea"), config.halo_area);
        config.halo_scale_x = parse_f32(get_prop(props, "HaloScaleX"), config.halo_scale_x);
        config.halo_scale_y = parse_f32(get_prop(props, "HaloScaleY"), config.halo_scale_y);
        config.halo_color = parse_vec4(get_prop(props, "HaloColor"), config.halo_color);

        config.fadeout_start = parse_f32(get_prop(props, "FadeoutStart"), config.fadeout_start);
        config.fadeout_end = parse_f32(get_prop(props, "FadeoutEnd"), config.fadeout_end);
        config.history_weight = parse_f32(get_prop(props, "HistoryWeight"), config.history_weight);
        config.radius = parse_f32(get_prop(props, "Radius"), config.radius);
        config.blink_period = parse_f32(get_prop(props, "BlinkPeriod"), config.blink_period);
        config.blink_on_time = parse_f32(get_prop(props, "BlinkOnTime"), config.blink_on_time);
        config.use_camera_translation = parse_bool(
            get_prop(props, "UseCameraTranslation"),
            config.use_camera_translation,
        );

        Ok(config)
    }
}

/// Dazzle statistics structure
#[derive(Debug, Clone)]
pub struct DazzleStats {
    /// Total number of dazzle instances
    pub total_instances: usize,
    /// Number of visible dazzle instances
    pub visible_instances: usize,
    /// Maximum number of dazzles to render
    pub max_instances: usize,
    /// Global intensity multiplier
    pub global_intensity: f32,
    /// Global size multiplier
    pub global_size: f32,
    /// Whether dazzle system is enabled
    pub enabled: bool,
}

/// Lens flare element structure
#[derive(Debug, Clone)]
pub struct LensFlareElement {
    /// Normalized location along the flare line
    pub location: f32,
    /// Size multiplier
    pub size: f32,
    /// Colour multiplier
    pub color: Vec3,
    /// UV rectangle within the flare texture
    pub uv: Vec4,
}

/// Lens flare configuration
#[derive(Debug, Clone, Default)]
pub struct LensFlareConfig {
    pub name: String,
    pub texture_name: String,
    pub elements: Vec<LensFlareElement>,
}

#[derive(Debug, Clone)]
pub struct DazzleRenderObj {
    name: String,
    type_name: String,
    transform: Mat4,
    radius: f32,
    sort_level: i32,
}

impl DazzleRenderObj {
    pub fn new(name: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            transform: Mat4::IDENTITY,
            radius: 1.0,
            sort_level: 0,
        }
    }

    pub fn with_radius(name: impl Into<String>, type_name: impl Into<String>, radius: f32) -> Self {
        let mut obj = Self::new(name, type_name);
        obj.radius = radius.max(0.0);
        obj
    }

    pub fn set_type_name(&mut self, type_name: impl Into<String>) {
        self.type_name = type_name.into();
    }

    pub fn set_radius(&mut self, radius: f32) {
        self.radius = radius.max(0.0);
    }
}

impl RenderObjClass for DazzleRenderObj {
    fn clone_obj(&self) -> Box<dyn RenderObjClass> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn class_id(&self) -> RenderObjClassId {
        RenderObjClassId::Dazzle
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    fn get_num_polys(&self) -> usize {
        0
    }

    fn render(&self, _rinfo: &RenderInfoClass) -> RendererResult<()> {
        if let Some(result) = with_dazzle_manager_mut(|manager| {
            manager
                .queue_dazzle_instance(&self.name, &self.type_name, self.transform)
                .map_err(RendererError::from)
        }) {
            result?;
        }
        Ok(())
    }

    fn special_render(&self, _rinfo: &SpecialRenderInfoClass) -> RendererResult<()> {
        Ok(())
    }

    fn cast_ray(&self, _raytest: &mut RayCollisionTestClass) -> bool {
        false
    }

    fn cast_aabox(&self, _boxtest: &mut AABoxCollisionTestClass) -> bool {
        false
    }

    fn cast_obbox(&self, _boxtest: &mut OBBoxCollisionTestClass) -> bool {
        false
    }

    fn intersect_aabox(&self, _boxtest: &AABoxIntersectionTestClass) -> bool {
        false
    }

    fn intersect_obbox(&self, _boxtest: &OBBoxIntersectionTestClass) -> bool {
        false
    }

    fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        SphereClass::new(Vec3::ZERO, self.radius)
    }

    fn get_obj_space_bounding_box(&self) -> AABoxClass {
        let extent = Vec3::splat(self.radius);
        AABoxClass::from_center_and_extent(Vec3::ZERO, extent)
    }

    fn scale(&mut self, scale: f32) {
        self.radius *= scale.abs();
    }

    fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        let avg = (scalex.abs() + scaley.abs() + scalez.abs()) / 3.0;
        self.radius *= avg.max(f32::EPSILON);
    }

    fn get_material_info(&self) -> Option<&MaterialInfoClass> {
        None
    }

    fn get_sort_level(&self) -> i32 {
        self.sort_level
    }

    fn set_sort_level(&mut self, level: i32) {
        self.sort_level = level;
    }

    fn create_decal(&mut self, _generator: &mut DecalGeneratorClass) {}

    fn delete_decal(&mut self, _decal_id: u32) {}

    fn transform(&self) -> &Mat4 {
        &self.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

fn get_prop<'a>(props: &'a HashMap<String, Option<String>>, key: &str) -> Option<&'a str> {
    props
        .get(&key.to_ascii_lowercase())
        .and_then(|value| value.as_deref())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

fn parse_f32(value: Option<&str>, default: f32) -> f32 {
    value
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(default)
}

fn parse_i32(value: Option<&str>, default: i32) -> i32 {
    value
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(default)
}

fn parse_bool(value: Option<&str>, default: bool) -> bool {
    value
        .map(|s| match s.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        })
        .unwrap_or(default)
}

fn parse_vec3(value: Option<&str>, default: Vec3) -> Vec3 {
    if let Some(raw) = value {
        let mut parts = raw.split(',').filter_map(|p| p.trim().parse::<f32>().ok());
        if let (Some(x), Some(y), Some(z)) = (parts.next(), parts.next(), parts.next()) {
            return Vec3::new(x, y, z);
        }
    }
    default
}

fn parse_vec4(value: Option<&str>, default: Vec4) -> Vec4 {
    if let Some(raw) = value {
        let mut parts = raw.split(',').filter_map(|p| p.trim().parse::<f32>().ok());
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(x), Some(y), Some(z), Some(w)) => return Vec4::new(x, y, z, w),
            (Some(x), Some(y), Some(z), None) => return Vec4::new(x, y, z, default.w),
            _ => {}
        }
    }
    default
}

fn extract_list(
    map: &HashMap<String, HashMap<String, Option<String>>>,
    section: &str,
) -> Vec<String> {
    map.get(&section.to_ascii_lowercase())
        .map(|props| {
            let mut entries: Vec<(i32, String)> = props
                .iter()
                .filter_map(|(key, value)| {
                    let index = key.trim().parse::<i32>().ok()?;
                    let name = value.as_deref()?.trim();
                    if name.is_empty() {
                        None
                    } else {
                        Some((index, name.to_string()))
                    }
                })
                .collect();
            entries.sort_by_key(|(idx, _)| *idx);
            entries.into_iter().map(|(_, name)| name).collect()
        })
        .unwrap_or_default()
}

/// Global dazzle manager storage
fn dazzle_manager_store() -> &'static Mutex<Option<DazzleManager>> {
    static STORE: OnceLock<Mutex<Option<DazzleManager>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(None))
}

fn with_dazzle_manager_mut<R, F>(f: F) -> Option<R>
where
    F: FnOnce(&mut DazzleManager) -> R,
{
    let mut slot = dazzle_manager_store().lock().ok()?;
    let manager = slot.as_mut()?;
    Some(f(manager))
}

/// Initialize dazzle system
pub fn init_dazzle_system(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
    let mut slot = dazzle_manager_store()
        .lock()
        .expect("dazzle manager lock poisoned");
    *slot = Some(DazzleManager::new(device, queue));
    Ok(())
}

/// Shutdown dazzle system
pub fn shutdown_dazzle_system() {
    if let Ok(mut slot) = dazzle_manager_store().lock() {
        *slot = None;
    }
}

/// Quick dazzle creation function
pub fn create_dazzle_at_position(position: Vec3, intensity: f32, size: f32) -> Result<usize> {
    with_dazzle_manager_mut(|manager| {
        let mut config = DazzleConfig::default();
        config.name = format!("quick_dazzle_{}", manager.instances.len());
        config.intensity = intensity;
        config.scale_x = size;
        config.scale_y = size;
        config.size = size;
        config.color = Vec4::new(1.0, 1.0, 1.0, 1.0);
        config.halo_color = Vec4::new(1.0, 1.0, 1.0, 1.0);

        let config_key = normalize_key(&config.name);
        manager.configs.insert(config_key.clone(), config);
        manager.create_dazzle(&config_key, position)
    })
    .unwrap_or_else(|| {
        Err(W3DError::NotInitialized(
            "Dazzle manager not initialized".to_string(),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dazzle_config_creation() {
        let config = DazzleConfig::default();
        assert_eq!(config.intensity, 1.0);
        assert_eq!(config.size, 1.0);
        assert_eq!(config.dazzle_type, DazzleType::Standard);
        assert!(config.use_camera_translation);
    }

    #[test]
    fn test_dazzle_instance_creation() {
        let config = DazzleConfig::default();
        let transform = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let instance = DazzleInstance::new("Test", "Default", config, transform, 1);

        assert_eq!(instance.name, "Test");
        assert_eq!(instance.type_name, "DEFAULT");
        assert_eq!(instance.transform, transform);
        assert!(!instance.visible);
    }

    #[test]
    fn test_lens_flare_config() {
        let flare_config = LensFlareConfig::default();
        assert!(flare_config.elements.is_empty());
    }
}
