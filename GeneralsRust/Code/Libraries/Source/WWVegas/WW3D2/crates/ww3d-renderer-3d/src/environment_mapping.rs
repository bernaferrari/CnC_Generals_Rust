//! Environment Mapping System
//!
//! Implements reflection and chrome material effects including:
//! - EnvironmentMap (cube map reflections)
//! - Chrome (reflective surfaces with lighting)
//! - Reflected Shroud (special effect)
//!
//! These material types use cube maps to create reflection effects on geometry.

use glam::Vec3;
use std::sync::Arc;
use wgpu::{
    Device, Extent3d, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureView, TextureViewDescriptor, TextureViewDimension,
};

/// Result type for environment mapping operations
pub type EnvironmentMappingResult<T> = Result<T, EnvironmentMappingError>;

/// Error types for environment mapping
#[derive(Debug, Clone)]
pub enum EnvironmentMappingError {
    /// Cube map not loaded
    CubeMapNotLoaded(String),
    /// Invalid material configuration
    InvalidMaterialConfig(String),
    /// Reflection calculation failed
    ReflectionCalculationError(String),
    /// Cube map sampling error
    CubeSamplingError(String),
}

impl std::fmt::Display for EnvironmentMappingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvironmentMappingError::CubeMapNotLoaded(name) => {
                write!(f, "Cube map not loaded: {}", name)
            }
            EnvironmentMappingError::InvalidMaterialConfig(msg) => {
                write!(f, "Invalid material config: {}", msg)
            }
            EnvironmentMappingError::ReflectionCalculationError(msg) => {
                write!(f, "Reflection calculation error: {}", msg)
            }
            EnvironmentMappingError::CubeSamplingError(msg) => {
                write!(f, "Cube sampling error: {}", msg)
            }
        }
    }
}

impl std::error::Error for EnvironmentMappingError {}

/// Cube map face identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubeFace {
    /// Positive X face (right)
    PositiveX,
    /// Negative X face (left)
    NegativeX,
    /// Positive Y face (top)
    PositiveY,
    /// Negative Y face (bottom)
    NegativeY,
    /// Positive Z face (front)
    PositiveZ,
    /// Negative Z face (back)
    NegativeZ,
}

impl CubeFace {
    /// Get face as u32 for GPU indexing
    pub fn as_index(&self) -> u32 {
        match self {
            CubeFace::PositiveX => 0,
            CubeFace::NegativeX => 1,
            CubeFace::PositiveY => 2,
            CubeFace::NegativeY => 3,
            CubeFace::PositiveZ => 4,
            CubeFace::NegativeZ => 5,
        }
    }

    /// Determine which face a direction vector samples from
    pub fn from_direction(direction: Vec3) -> Self {
        let abs_x = direction.x.abs();
        let abs_y = direction.y.abs();
        let abs_z = direction.z.abs();

        if abs_x > abs_y && abs_x > abs_z {
            if direction.x > 0.0 {
                CubeFace::PositiveX
            } else {
                CubeFace::NegativeX
            }
        } else if abs_y > abs_x && abs_y > abs_z {
            if direction.y > 0.0 {
                CubeFace::PositiveY
            } else {
                CubeFace::NegativeY
            }
        } else if direction.z > 0.0 {
            CubeFace::PositiveZ
        } else {
            CubeFace::NegativeZ
        }
    }
}

/// Cube map texture data
pub struct CubeMap {
    /// Name identifier
    pub name: String,
    /// Face resolution (cube maps are square, single resolution)
    pub resolution: u32,
    /// Cube face data (0-5 in order: +X, -X, +Y, -Y, +Z, -Z)
    /// CPU-side texture data (RGBA8)
    pub faces: Vec<Option<Vec<u8>>>,
    /// Mipmap levels
    pub mip_levels: u32,
    /// GPU texture handle
    gpu_texture: Option<Arc<Texture>>,
    /// GPU texture view for sampling
    gpu_view: Option<Arc<TextureView>>,
}

impl CubeMap {
    /// Create a new cube map
    pub fn new(name: String, resolution: u32) -> Self {
        Self {
            name,
            resolution,
            faces: vec![None; 6],
            mip_levels: 1,
            gpu_texture: None,
            gpu_view: None,
        }
    }

    /// Check if all faces are loaded
    pub fn is_complete(&self) -> bool {
        self.faces.iter().all(|f| f.is_some())
    }

    /// Set a cube face
    pub fn set_face(&mut self, face: CubeFace, data: Vec<u8>) -> EnvironmentMappingResult<()> {
        let idx = face.as_index() as usize;
        if idx >= 6 {
            return Err(EnvironmentMappingError::CubeSamplingError(
                "Invalid cube face index".to_string(),
            ));
        }

        // Verify data size: resolution * resolution * 4 bytes (RGBA)
        let expected_size = (self.resolution * self.resolution * 4) as usize;
        if data.len() != expected_size {
            return Err(EnvironmentMappingError::CubeSamplingError(format!(
                "Face data size mismatch: expected {}, got {}",
                expected_size,
                data.len()
            )));
        }

        self.faces[idx] = Some(data);
        Ok(())
    }

    /// Get a cube face
    pub fn get_face(&self, face: CubeFace) -> Option<&Vec<u8>> {
        self.faces[face.as_index() as usize].as_ref()
    }

    /// Calculate mipmap levels (log2 of resolution)
    pub fn calculate_mip_levels(&mut self) {
        self.mip_levels = (self.resolution as f32).log2().ceil() as u32 + 1;
    }

    /// Upload cube map to GPU
    /// Matches C++ TextureClass::Init() for cube textures
    pub fn upload_to_gpu(
        &mut self,
        device: &Device,
        queue: &wgpu::Queue,
    ) -> EnvironmentMappingResult<()> {
        if !self.is_complete() {
            return Err(EnvironmentMappingError::CubeMapNotLoaded(
                "Not all faces loaded".to_string(),
            ));
        }

        // Create GPU cube texture
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(&format!("CubeMap_{}", self.name)),
            size: Extent3d {
                width: self.resolution,
                height: self.resolution,
                depth_or_array_layers: 6, // 6 faces for cube map
            },
            mip_level_count: self.mip_levels,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload each face
        for (face_idx, face_data) in self.faces.iter().enumerate() {
            if let Some(data) = face_data {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: face_idx as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * self.resolution),
                        rows_per_image: Some(self.resolution),
                    },
                    Extent3d {
                        width: self.resolution,
                        height: self.resolution,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }

        // Create texture view for cube sampling
        let view = texture.create_view(&TextureViewDescriptor {
            label: Some(&format!("CubeMapView_{}", self.name)),
            dimension: Some(TextureViewDimension::Cube),
            ..Default::default()
        });

        self.gpu_texture = Some(Arc::new(texture));
        self.gpu_view = Some(Arc::new(view));

        Ok(())
    }

    /// Get GPU texture view for binding
    pub fn get_gpu_view(&self) -> Option<&Arc<TextureView>> {
        self.gpu_view.as_ref()
    }

    /// Get GPU texture
    pub fn get_gpu_texture(&self) -> Option<&Arc<Texture>> {
        self.gpu_texture.as_ref()
    }

    /// Sample cube map (CPU-side, for material calculations)
    /// Uses bilinear filtering on closest face
    pub fn sample(&self, direction: Vec3) -> EnvironmentMappingResult<Vec3> {
        if !self.is_complete() {
            return Err(EnvironmentMappingError::CubeMapNotLoaded(
                "Cannot sample incomplete cube map".to_string(),
            ));
        }

        let face = CubeFace::from_direction(direction);
        let face_data = self.faces[face.as_index() as usize]
            .as_ref()
            .ok_or_else(|| {
                EnvironmentMappingError::CubeSamplingError("Face not loaded".to_string())
            })?;

        // Convert direction to UV coordinates on the cube face
        let (u, v) = Self::direction_to_uv(direction, face);

        // Clamp to [0, 1]
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        // Convert to pixel coordinates
        let x = (u * (self.resolution - 1) as f32) as usize;
        let y = (v * (self.resolution - 1) as f32) as usize;

        // Sample pixel (RGBA8)
        let pixel_index = (y * self.resolution as usize + x) * 4;
        let r = face_data[pixel_index] as f32 / 255.0;
        let g = face_data[pixel_index + 1] as f32 / 255.0;
        let b = face_data[pixel_index + 2] as f32 / 255.0;

        Ok(Vec3::new(r, g, b))
    }

    /// Convert 3D direction to UV coordinates on cube face
    /// Matches D3D cube map convention
    fn direction_to_uv(direction: Vec3, face: CubeFace) -> (f32, f32) {
        let dir = direction.normalize();

        match face {
            CubeFace::PositiveX => {
                // U = -Z, V = -Y
                let u = (-dir.z / dir.x) * 0.5 + 0.5;
                let v = (-dir.y / dir.x) * 0.5 + 0.5;
                (u, v)
            }
            CubeFace::NegativeX => {
                // U = Z, V = -Y
                let u = (dir.z / -dir.x) * 0.5 + 0.5;
                let v = (-dir.y / -dir.x) * 0.5 + 0.5;
                (u, v)
            }
            CubeFace::PositiveY => {
                // U = X, V = Z
                let u = (dir.x / dir.y) * 0.5 + 0.5;
                let v = (dir.z / dir.y) * 0.5 + 0.5;
                (u, v)
            }
            CubeFace::NegativeY => {
                // U = X, V = -Z
                let u = (dir.x / -dir.y) * 0.5 + 0.5;
                let v = (-dir.z / -dir.y) * 0.5 + 0.5;
                (u, v)
            }
            CubeFace::PositiveZ => {
                // U = X, V = -Y
                let u = (dir.x / dir.z) * 0.5 + 0.5;
                let v = (-dir.y / dir.z) * 0.5 + 0.5;
                (u, v)
            }
            CubeFace::NegativeZ => {
                // U = -X, V = -Y
                let u = (-dir.x / -dir.z) * 0.5 + 0.5;
                let v = (-dir.y / -dir.z) * 0.5 + 0.5;
                (u, v)
            }
        }
    }
}

/// Reflection calculation helper
pub struct ReflectionCalculator;

impl ReflectionCalculator {
    /// Calculate reflected direction (for specular reflections)
    ///
    /// Uses standard reflection formula: R = I - 2(I·N)N
    /// where I is incident direction (viewer to surface)
    pub fn calculate_reflection(incident: Vec3, normal: Vec3) -> Vec3 {
        let incident_norm = incident.normalize();
        let normal_norm = normal.normalize();
        let dot = incident_norm.dot(normal_norm);
        (incident_norm - 2.0 * dot * normal_norm).normalize()
    }

    /// Calculate Fresnel effect (variation with viewing angle)
    ///
    /// Fresnel = clamp((1 - dot(V, N))^5, 0, 1)
    /// This gives more reflection at grazing angles
    pub fn calculate_fresnel(view_direction: Vec3, normal: Vec3, power: f32) -> f32 {
        let view_norm = view_direction.normalize();
        let normal_norm = normal.normalize();
        let dot = view_norm.dot(normal_norm).clamp(0.0, 1.0);
        (1.0 - dot).powf(power)
    }

    /// Blend between surface color and reflection based on Fresnel
    pub fn blend_with_fresnel(surface_color: Vec3, reflection_color: Vec3, fresnel: f32) -> Vec3 {
        surface_color.lerp(reflection_color, fresnel)
    }
}

/// Environment map material (cube map reflection)
pub struct EnvironmentMapMaterial {
    /// Associated cube map
    pub cube_map: Arc<CubeMap>,
    /// Reflection intensity (0.0 = no reflection, 1.0 = full)
    pub intensity: f32,
    /// Use Fresnel effect
    pub use_fresnel: bool,
    /// Fresnel power exponent
    pub fresnel_power: f32,
}

impl EnvironmentMapMaterial {
    /// Create new environment map material
    pub fn new(cube_map: Arc<CubeMap>) -> Self {
        Self {
            cube_map,
            intensity: 1.0,
            use_fresnel: true,
            fresnel_power: 5.0,
        }
    }

    /// Calculate reflection color
    /// Matches C++ EnvironmentMapperClass behavior
    ///
    /// Implementation:
    /// 1. Calculate reflection vector from normal
    /// 2. Sample cube map in reflection direction
    /// 3. Apply Fresnel if enabled
    /// 4. Blend with surface color
    pub fn calculate_reflection(
        &self,
        surface_color: Vec3,
        normal: Vec3,
        view_direction: Vec3,
    ) -> EnvironmentMappingResult<Vec3> {
        // Calculate reflection vector: R = I - 2(I·N)N
        let reflection = ReflectionCalculator::calculate_reflection(view_direction, normal);

        // Sample cube map in reflection direction
        let reflection_color = self.cube_map.sample(reflection)?;

        // Apply Fresnel effect if enabled
        let blend_factor = if self.use_fresnel {
            ReflectionCalculator::calculate_fresnel(view_direction, normal, self.fresnel_power)
        } else {
            self.intensity
        };

        // Blend surface color with reflection based on Fresnel
        Ok(ReflectionCalculator::blend_with_fresnel(
            surface_color,
            reflection_color,
            blend_factor * self.intensity,
        ))
    }

    /// Set reflection intensity
    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity.clamp(0.0, 1.0);
    }

    /// Enable/disable Fresnel effect
    pub fn set_fresnel(&mut self, enabled: bool, power: f32) {
        self.use_fresnel = enabled;
        self.fresnel_power = power.clamp(0.1, 10.0);
    }
}

/// Chrome material (reflective with lighting)
pub struct ChromeMaterial {
    /// Associated cube map for reflections
    pub cube_map: Arc<CubeMap>,
    /// Chrome color tint
    pub color: Vec3,
    /// Specularity (0.0 = dull, 1.0 = mirror)
    pub specularity: f32,
    /// Self-illumination (for glow effect)
    pub self_illumination: f32,
}

impl ChromeMaterial {
    /// Create new chrome material
    pub fn new(cube_map: Arc<CubeMap>) -> Self {
        Self {
            cube_map,
            color: Vec3::ONE,
            specularity: 1.0,
            self_illumination: 0.0,
        }
    }

    /// Calculate chrome shading
    /// Matches C++ chrome material rendering
    ///
    /// Chrome = reflection + lighting + self-illumination
    pub fn calculate_chrome_shading(
        &self,
        reflection_color: Vec3,
        light_color: Vec3,
        normal: Vec3,
        light_direction: Vec3,
    ) -> EnvironmentMappingResult<Vec3> {
        // Calculate diffuse lighting (N · L)
        let n_dot_l = normal.dot(light_direction).max(0.0);
        let diffuse = light_color * n_dot_l;

        // Blend reflection with diffuse lighting based on specularity
        // Higher specularity = more reflection, less diffuse
        let base_color = reflection_color * self.specularity + diffuse * (1.0 - self.specularity);

        // Apply chrome color tint
        let tinted = Vec3::new(
            base_color.x * self.color.x,
            base_color.y * self.color.y,
            base_color.z * self.color.z,
        );

        // Add self-illumination (emissive)
        let final_color = tinted + self.color * self.self_illumination;

        Ok(final_color)
    }

    /// Set specularity
    pub fn set_specularity(&mut self, specularity: f32) {
        self.specularity = specularity.clamp(0.0, 1.0);
    }

    /// Set self-illumination
    pub fn set_self_illumination(&mut self, self_illum: f32) {
        self.self_illumination = self_illum.clamp(0.0, 1.0);
    }
}

/// Reflected shroud material (special effect for shroud revealing)
pub struct ReflectedShroudMaterial {
    /// Cube map for environment
    pub cube_map: Arc<CubeMap>,
    /// Opacity of shroud (0.0 = invisible, 1.0 = opaque)
    pub opacity: f32,
    /// Shroud color
    pub shroud_color: Vec3,
}

impl ReflectedShroudMaterial {
    /// Create new reflected shroud material
    pub fn new(cube_map: Arc<CubeMap>) -> Self {
        Self {
            cube_map,
            opacity: 0.5,
            shroud_color: Vec3::new(0.3, 0.3, 0.3),
        }
    }

    /// Calculate shroud effect
    /// Implements fog-of-war blending for reflections
    ///
    /// Blends environment reflection with shroud color based on opacity
    /// Higher opacity = more shroud (darker), lower opacity = more reflection (visible)
    pub fn calculate_shroud_effect(
        &self,
        reflection_color: Vec3,
        view_direction: Vec3,
    ) -> EnvironmentMappingResult<Vec3> {
        // Sample cube map for base reflection
        let env_reflection = self.cube_map.sample(view_direction)?;

        // Blend environment reflection with reflection_color (world reflection)
        let combined_reflection = env_reflection * 0.5 + reflection_color * 0.5;

        // Blend with shroud color based on opacity
        // opacity = 0: full reflection visible
        // opacity = 1: full shroud (dark/fog)
        let result = combined_reflection * (1.0 - self.opacity) + self.shroud_color * self.opacity;

        Ok(result)
    }

    /// Set opacity
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }
}

/// Environment map cache
pub struct EnvironmentMapCache {
    cube_maps: std::collections::HashMap<String, Arc<CubeMap>>,
}

impl EnvironmentMapCache {
    /// Create new cache
    pub fn new() -> Self {
        Self {
            cube_maps: std::collections::HashMap::new(),
        }
    }

    /// Add cube map to cache
    pub fn add_cube_map(&mut self, cube_map: CubeMap) {
        self.cube_maps
            .insert(cube_map.name.clone(), Arc::new(cube_map));
    }

    /// Get cube map from cache
    pub fn get_cube_map(&self, name: &str) -> Option<Arc<CubeMap>> {
        self.cube_maps.get(name).map(Arc::clone)
    }

    /// Check if cube map exists
    pub fn has_cube_map(&self, name: &str) -> bool {
        self.cube_maps.contains_key(name)
    }

    /// Remove cube map from cache
    pub fn remove_cube_map(&mut self, name: &str) -> Option<Arc<CubeMap>> {
        self.cube_maps.remove(name)
    }

    /// Clear all cube maps
    pub fn clear(&mut self) {
        self.cube_maps.clear();
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let memory_usage: usize = self
            .cube_maps
            .values()
            .map(|cm| {
                // Estimate: 6 faces * resolution^2 * 4 bytes + overhead
                (6 * cm.resolution * cm.resolution * 4) as usize + 256
            })
            .sum();

        CacheStats {
            cube_map_count: self.cube_maps.len(),
            total_memory_bytes: memory_usage,
        }
    }
}

impl Default for EnvironmentMapCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub cube_map_count: usize,
    pub total_memory_bytes: usize,
}

/// Environment probe for static reflections
/// Probes capture the scene at a fixed position and store it as a cube map
pub struct EnvironmentProbe {
    /// Probe position in world space
    pub position: Vec3,
    /// Cube map containing captured environment
    pub cube_map: Arc<CubeMap>,
    /// Influence radius (objects within this distance use this probe)
    pub radius: f32,
    /// Probe priority (higher priority probes take precedence)
    pub priority: i32,
    /// Is this probe static (pre-baked) or dynamic (updated each frame)?
    pub is_static: bool,
    /// Update frequency for dynamic probes (frames between updates)
    pub update_frequency: u32,
    /// Frame counter for update timing
    frames_since_update: u32,
}

impl EnvironmentProbe {
    /// Create a new static environment probe
    pub fn new_static(position: Vec3, cube_map: Arc<CubeMap>, radius: f32) -> Self {
        Self {
            position,
            cube_map,
            radius,
            priority: 0,
            is_static: true,
            update_frequency: 0,
            frames_since_update: 0,
        }
    }

    /// Create a new dynamic environment probe
    pub fn new_dynamic(
        position: Vec3,
        cube_map: Arc<CubeMap>,
        radius: f32,
        update_frequency: u32,
    ) -> Self {
        Self {
            position,
            cube_map,
            radius,
            priority: 0,
            is_static: false,
            update_frequency,
            frames_since_update: 0,
        }
    }

    /// Check if this probe influences a given position
    pub fn influences(&self, position: Vec3) -> bool {
        self.position.distance(position) <= self.radius
    }

    /// Get influence strength at a position (0.0 = no influence, 1.0 = full influence)
    /// Uses inverse square falloff
    pub fn influence_strength(&self, position: Vec3) -> f32 {
        let distance = self.position.distance(position);
        if distance >= self.radius {
            return 0.0;
        }

        // Inverse square falloff with smooth edge
        let normalized = distance / self.radius;
        let falloff = 1.0 - normalized * normalized;
        falloff.clamp(0.0, 1.0)
    }

    /// Check if probe needs update this frame
    pub fn needs_update(&mut self) -> bool {
        if self.is_static {
            return false;
        }

        self.frames_since_update += 1;
        if self.frames_since_update >= self.update_frequency {
            self.frames_since_update = 0;
            true
        } else {
            false
        }
    }
}

/// Environment probe system
/// Manages multiple environment probes and selects the best one for each object
pub struct EnvironmentProbeSystem {
    /// All environment probes
    probes: Vec<EnvironmentProbe>,
    /// Default fallback cube map (used when no probes influence a position)
    default_cube_map: Option<Arc<CubeMap>>,
}

impl EnvironmentProbeSystem {
    /// Create a new probe system
    pub fn new() -> Self {
        Self {
            probes: Vec::new(),
            default_cube_map: None,
        }
    }

    /// Set default fallback cube map
    pub fn set_default_cube_map(&mut self, cube_map: Arc<CubeMap>) {
        self.default_cube_map = Some(cube_map);
    }

    /// Add a probe to the system
    pub fn add_probe(&mut self, probe: EnvironmentProbe) -> usize {
        self.probes.push(probe);
        self.probes.len() - 1
    }

    /// Remove a probe
    pub fn remove_probe(&mut self, index: usize) -> Option<EnvironmentProbe> {
        if index < self.probes.len() {
            Some(self.probes.remove(index))
        } else {
            None
        }
    }

    /// Get the best probe for a given position
    /// Returns the probe with highest influence and priority
    pub fn get_best_probe(&self, position: Vec3) -> Option<&EnvironmentProbe> {
        let mut best_probe: Option<&EnvironmentProbe> = None;
        let mut best_score = -1.0;

        for probe in &self.probes {
            let strength = probe.influence_strength(position);
            if strength > 0.0 {
                // Score = influence_strength + priority_bonus
                let score = strength + (probe.priority as f32) * 0.1;
                if score > best_score {
                    best_score = score;
                    best_probe = Some(probe);
                }
            }
        }

        best_probe
    }

    /// Get cube map for a position (best probe or default)
    pub fn get_cube_map_for_position(&self, position: Vec3) -> Option<Arc<CubeMap>> {
        if let Some(probe) = self.get_best_probe(position) {
            Some(Arc::clone(&probe.cube_map))
        } else {
            self.default_cube_map.as_ref().map(Arc::clone)
        }
    }

    /// Update all dynamic probes
    pub fn update(&mut self) {
        for probe in &mut self.probes {
            probe.needs_update();
        }
    }

    /// Get probes that need updating this frame
    pub fn get_probes_needing_update(&mut self) -> Vec<usize> {
        self.probes
            .iter_mut()
            .enumerate()
            .filter_map(|(idx, probe)| {
                if probe.needs_update() {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get statistics
    pub fn get_stats(&self) -> ProbeSystemStats {
        let static_count = self.probes.iter().filter(|p| p.is_static).count();
        let dynamic_count = self.probes.len() - static_count;

        ProbeSystemStats {
            total_probes: self.probes.len(),
            static_probes: static_count,
            dynamic_probes: dynamic_count,
        }
    }
}

impl Default for EnvironmentProbeSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Probe system statistics
#[derive(Debug, Clone)]
pub struct ProbeSystemStats {
    pub total_probes: usize,
    pub static_probes: usize,
    pub dynamic_probes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cube_face_indexing() {
        assert_eq!(CubeFace::PositiveX.as_index(), 0);
        assert_eq!(CubeFace::NegativeX.as_index(), 1);
        assert_eq!(CubeFace::PositiveY.as_index(), 2);
        assert_eq!(CubeFace::NegativeY.as_index(), 3);
        assert_eq!(CubeFace::PositiveZ.as_index(), 4);
        assert_eq!(CubeFace::NegativeZ.as_index(), 5);
    }

    #[test]
    fn test_cube_face_from_direction() {
        // Test positive X
        let face = CubeFace::from_direction(Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(face, CubeFace::PositiveX);

        // Test negative Z
        let face = CubeFace::from_direction(Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(face, CubeFace::NegativeZ);
    }

    #[test]
    fn test_cube_map_creation() {
        let cube_map = CubeMap::new("test_env".to_string(), 512);
        assert_eq!(cube_map.name, "test_env");
        assert_eq!(cube_map.resolution, 512);
        assert!(!cube_map.is_complete());
    }

    #[test]
    fn test_environment_map_material() {
        let cube_map = Arc::new(CubeMap::new("env".to_string(), 256));
        let mut material = EnvironmentMapMaterial::new(cube_map);

        material.set_intensity(0.8);
        assert_eq!(material.intensity, 0.8);

        material.set_fresnel(false, 3.0);
        assert!(!material.use_fresnel);
        assert_eq!(material.fresnel_power, 3.0);
    }

    #[test]
    fn test_chrome_material() {
        let cube_map = Arc::new(CubeMap::new("chrome_env".to_string(), 256));
        let mut material = ChromeMaterial::new(cube_map);

        material.set_specularity(0.5);
        assert_eq!(material.specularity, 0.5);

        material.set_self_illumination(0.3);
        assert_eq!(material.self_illumination, 0.3);
    }

    #[test]
    fn test_environment_cache() {
        let mut cache = EnvironmentMapCache::new();
        let cube_map = CubeMap::new("test".to_string(), 256);

        cache.add_cube_map(cube_map);
        assert!(cache.has_cube_map("test"));
        assert!(cache.get_cube_map("test").is_some());

        cache.clear();
        assert!(!cache.has_cube_map("test"));
    }

    #[test]
    fn test_reflection_calculator() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let incident = Vec3::new(1.0, -1.0, 0.0).normalize();

        let reflection = ReflectionCalculator::calculate_reflection(incident, normal);
        assert!(reflection.length() > 0.0);

        let fresnel = ReflectionCalculator::calculate_fresnel(incident, normal, 5.0);
        assert!((0.0..=1.0).contains(&fresnel));
    }

    #[test]
    fn test_error_display() {
        let err = EnvironmentMappingError::CubeMapNotLoaded("missing.dds".to_string());
        assert_eq!(format!("{}", err), "Cube map not loaded: missing.dds");
    }

    #[test]
    fn test_cube_map_sampling() {
        let mut cube_map = CubeMap::new("test_cube".to_string(), 2);

        // Create simple test data for one face (2x2 pixels, RGBA)
        let face_data = vec![
            255, 0, 0, 255, // Red pixel
            0, 255, 0, 255, // Green pixel
            0, 0, 255, 255, // Blue pixel
            255, 255, 0, 255, // Yellow pixel
        ];

        // Set all faces to the same data for testing
        for face in 0..6 {
            let _ = cube_map.set_face(
                match face {
                    0 => CubeFace::PositiveX,
                    1 => CubeFace::NegativeX,
                    2 => CubeFace::PositiveY,
                    3 => CubeFace::NegativeY,
                    4 => CubeFace::PositiveZ,
                    _ => CubeFace::NegativeZ,
                },
                face_data.clone(),
            );
        }

        // Test sampling
        let direction = Vec3::new(1.0, 0.0, 0.0);
        let result = cube_map.sample(direction);
        assert!(result.is_ok());
    }

    #[test]
    fn test_environment_probe_influence() {
        let cube_map = Arc::new(CubeMap::new("test".to_string(), 256));
        let probe = EnvironmentProbe::new_static(Vec3::new(0.0, 0.0, 0.0), cube_map, 10.0);

        // Test point inside radius
        assert!(probe.influences(Vec3::new(5.0, 0.0, 0.0)));

        // Test point outside radius
        assert!(!probe.influences(Vec3::new(15.0, 0.0, 0.0)));

        // Test influence strength
        let strength = probe.influence_strength(Vec3::new(0.0, 0.0, 0.0));
        assert!(strength > 0.9); // Very close to center

        let edge_strength = probe.influence_strength(Vec3::new(9.0, 0.0, 0.0));
        assert!(edge_strength > 0.0 && edge_strength < 0.5); // Near edge
    }

    #[test]
    fn test_probe_system() {
        let mut system = EnvironmentProbeSystem::new();

        let cube_map1 = Arc::new(CubeMap::new("probe1".to_string(), 256));
        let cube_map2 = Arc::new(CubeMap::new("probe2".to_string(), 256));

        let probe1 = EnvironmentProbe::new_static(Vec3::new(0.0, 0.0, 0.0), cube_map1, 10.0);
        let probe2 = EnvironmentProbe::new_static(Vec3::new(20.0, 0.0, 0.0), cube_map2, 10.0);

        system.add_probe(probe1);
        system.add_probe(probe2);

        // Test best probe selection
        let best = system.get_best_probe(Vec3::new(5.0, 0.0, 0.0));
        assert!(best.is_some());
        assert_eq!(best.unwrap().cube_map.name, "probe1");

        let best2 = system.get_best_probe(Vec3::new(25.0, 0.0, 0.0));
        assert!(best2.is_some());
        assert_eq!(best2.unwrap().cube_map.name, "probe2");
    }

    #[test]
    fn test_dynamic_probe_updates() {
        let cube_map = Arc::new(CubeMap::new("dynamic".to_string(), 256));
        let mut probe = EnvironmentProbe::new_dynamic(Vec3::ZERO, cube_map, 10.0, 5);

        // Should not need update initially
        assert!(!probe.is_static);

        // After update_frequency frames, should need update
        for _ in 0..4 {
            assert!(!probe.needs_update());
        }
        assert!(probe.needs_update());
    }
}
