//! Light System - Complete lighting functionality
//!
//! This module implements the LightClass from the original C++ code,
//! providing comprehensive lighting with WGPU integration.
//!
//! Converted from:
//! - light.cpp/h (light class implementation)
//! - lightenvironment.cpp/h (light environment management)

use crate::core::error::{Error, Result};
use crate::render_object_system::RenderObjClass;
use crate::scene_system::scene::SceneClass;
use glam::{Mat4, Vec3, Vec4};
use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use ww3d_collision::bounding_volumes::{aabox::AABoxClass, sphere::SphereClass};

static LIGHT_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

fn next_light_id() -> u32 {
    LIGHT_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1
}

const DEFAULT_LIGHT_COLOR: Vec4 = Vec4::new(1.0, 1.0, 1.0, 1.0);
const DEFAULT_FAR_ATTENUATION_START: f32 = 50.0;
const DEFAULT_FAR_ATTENUATION_END: f32 = 100.0;
const DEFAULT_SPOT_ANGLE: f32 = std::f32::consts::FRAC_PI_4;
const DEFAULT_SPOT_DIRECTION: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const DEFAULT_NEAR_ATTENUATION_START: f32 = 0.0;
const DEFAULT_NEAR_ATTENUATION_END: f32 = 0.0;

const W3D_CHUNK_LIGHT: u32 = 0x0000_0460;
const W3D_CHUNK_LIGHT_INFO: u32 = 0x0000_0461;
const W3D_CHUNK_SPOT_LIGHT_INFO: u32 = 0x0000_0462;
const W3D_CHUNK_NEAR_ATTENUATION: u32 = 0x0000_0463;
const W3D_CHUNK_FAR_ATTENUATION: u32 = 0x0000_0464;
const W3D_CHUNK_SIZE_MASK: u32 = 0x7fff_ffff;
const W3D_CHUNK_HAS_SUBCHUNKS: u32 = 0x8000_0000;

const W3D_LIGHT_ATTRIBUTE_TYPE_MASK: u32 = 0x0000_00ff;
const W3D_LIGHT_ATTRIBUTE_POINT: u32 = 0x0000_0001;
const W3D_LIGHT_ATTRIBUTE_DIRECTIONAL: u32 = 0x0000_0002;
const W3D_LIGHT_ATTRIBUTE_SPOT: u32 = 0x0000_0003;
const W3D_LIGHT_ATTRIBUTE_CAST_SHADOWS: u32 = 0x0000_0100;

const LIGHT_FLAG_NEAR_ATTENUATION: u32 = 0;
const LIGHT_FLAG_FAR_ATTENUATION: u32 = 1;
const W3D_LIGHT_INFO_SIZE: usize = 24;
const W3D_SPOT_LIGHT_INFO_SIZE: usize = 20;
const W3D_LIGHT_ATTENUATION_SIZE: usize = 8;

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidData("truncated W3D u32".to_string()))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_f32_le(data: &[u8], offset: usize) -> Result<f32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidData("truncated W3D f32".to_string()))?;
    Ok(f32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_chunk<'a>(data: &'a [u8], cursor: &mut usize) -> Result<Option<(u32, &'a [u8])>> {
    if *cursor == data.len() {
        return Ok(None);
    }
    if data.len().saturating_sub(*cursor) < 8 {
        return Err(Error::InvalidData("truncated W3D chunk header".to_string()));
    }

    let chunk_type = read_u32_le(data, *cursor)?;
    let chunk_size = read_u32_le(data, *cursor + 4)? & W3D_CHUNK_SIZE_MASK;
    let payload_start = *cursor + 8;
    let payload_end = payload_start
        .checked_add(chunk_size as usize)
        .ok_or_else(|| Error::InvalidData("W3D chunk size overflow".to_string()))?;
    if payload_end > data.len() {
        return Err(Error::InvalidData(format!(
            "W3D chunk 0x{chunk_type:08x} overruns light payload"
        )));
    }

    *cursor = payload_end;
    Ok(Some((chunk_type, &data[payload_start..payload_end])))
}

fn write_chunk_bytes(out: &mut Vec<u8>, chunk_type: u32, payload: &[u8], has_subchunks: bool) {
    out.extend_from_slice(&chunk_type.to_le_bytes());
    let mut chunk_size = payload.len() as u32;
    if has_subchunks {
        chunk_size |= W3D_CHUNK_HAS_SUBCHUNKS;
    }
    out.extend_from_slice(&chunk_size.to_le_bytes());
    out.extend_from_slice(payload);
}

fn w3d_rgb_to_vec4(data: &[u8], offset: usize) -> Result<Vec4> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidData("truncated W3D RGB color".to_string()))?;
    Ok(Vec4::new(
        bytes[0] as f32 / 255.0,
        bytes[1] as f32 / 255.0,
        bytes[2] as f32 / 255.0,
        1.0,
    ))
}

fn push_w3d_rgb(out: &mut Vec<u8>, color: Vec4) {
    out.push((255.0 * color.x) as u8);
    out.push((255.0 * color.y) as u8);
    out.push((255.0 * color.z) as u8);
    out.push(0);
}

/// Light type enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightType {
    /// Point light
    Point = 0,
    /// Directional light
    Directional,
    /// Spot light
    Spot,
}

/// Light class - Core lighting functionality
#[derive(Debug)]
pub struct LightClass {
    /// Base render object
    pub base: Option<Arc<dyn RenderObjClass>>,
    /// Light type
    pub light_type: LightType,
    /// C++ `Flags` bit field.
    pub flags: u32,
    /// Whether this light casts shadows.
    pub cast_shadows: bool,
    /// Mirrors C++ `Set_Force_Visible(true)` for directional lights.
    pub force_visible: bool,
    /// Light position
    pub position: Vec3,
    /// Light direction (for directional and spot lights)
    pub direction: Vec3,
    /// Ambient color
    pub ambient: Vec4,
    /// Diffuse color
    pub diffuse: Vec4,
    /// Specular color
    pub specular: Vec4,
    /// Light intensity
    pub intensity: f32,
    /// Spotlight inner angle (in radians)
    pub inner_cone_angle: f32,
    /// Spotlight outer angle (in radians)
    pub outer_cone_angle: f32,
    /// Spotlight falloff
    pub spot_falloff: f32,
    /// Near attenuation start distance
    pub near_attenuation_start: f32,
    /// Near attenuation end distance
    pub near_attenuation_end: f32,
    /// Attenuation start distance
    pub attenuation_start: f32,
    /// Attenuation end distance
    pub attenuation_end: f32,
    /// Light range (maximum distance)
    pub range: f32,
    /// Transform matrix
    pub transform: Mat4,
    /// Whether transform is dirty
    pub transform_dirty: bool,
    /// Light ID
    pub light_id: u32,
}

impl LightClass {
    /// Create a light with the same defaults as C++ LightClass::LightClass.
    pub fn new(light_type: LightType) -> Self {
        let light_id = next_light_id();
        let position = Vec3::ZERO;

        Self {
            base: None,
            light_type,
            flags: 0,
            cast_shadows: false,
            force_visible: light_type == LightType::Directional,
            position,
            direction: DEFAULT_SPOT_DIRECTION,
            ambient: DEFAULT_LIGHT_COLOR,
            diffuse: DEFAULT_LIGHT_COLOR,
            specular: DEFAULT_LIGHT_COLOR,
            intensity: 1.0,
            inner_cone_angle: DEFAULT_SPOT_ANGLE,
            outer_cone_angle: DEFAULT_SPOT_ANGLE,
            spot_falloff: 1.0,
            near_attenuation_start: DEFAULT_NEAR_ATTENUATION_START,
            near_attenuation_end: DEFAULT_NEAR_ATTENUATION_END,
            attenuation_start: DEFAULT_FAR_ATTENUATION_START,
            attenuation_end: DEFAULT_FAR_ATTENUATION_END,
            range: DEFAULT_FAR_ATTENUATION_END,
            transform: Mat4::from_translation(position),
            transform_dirty: false,
            light_id,
        }
    }

    /// Create new directional light
    pub fn new_directional(direction: Vec3, color: Vec4) -> Self {
        let mut light = Self::new(LightType::Directional);
        light.direction = direction.normalize_or_zero();
        light.diffuse = color;
        light.specular = color;
        light
    }

    /// Create new point light
    pub fn new_point(position: Vec3, color: Vec4, range: f32) -> Self {
        let mut light = Self::new(LightType::Point);
        light.position = position;
        light.direction = Vec3::ZERO;
        light.diffuse = color;
        light.specular = color;
        light.transform = Mat4::from_translation(position);
        light.set_range(range);
        light
    }

    /// Create new spot light
    pub fn new_spot(
        position: Vec3,
        direction: Vec3,
        color: Vec4,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
    ) -> Self {
        let mut light = Self::new(LightType::Spot);
        light.position = position;
        light.direction = direction.normalize_or_zero();
        light.diffuse = color;
        light.specular = color;
        light.inner_cone_angle = inner_angle;
        light.outer_cone_angle = outer_angle;
        light.transform = Mat4::from_translation(position);
        light.set_range(range);
        light
    }

    /// Clone light
    pub fn clone(&self) -> Self {
        self.copy_with_light_id(next_light_id())
    }

    fn copy_with_light_id(&self, light_id: u32) -> Self {
        Self {
            base: self.base.clone(),
            light_type: self.light_type,
            flags: self.flags,
            cast_shadows: self.cast_shadows,
            force_visible: self.force_visible,
            position: self.position,
            direction: self.direction,
            ambient: self.ambient,
            diffuse: self.diffuse,
            specular: self.specular,
            intensity: self.intensity,
            inner_cone_angle: self.inner_cone_angle,
            outer_cone_angle: self.outer_cone_angle,
            spot_falloff: self.spot_falloff,
            near_attenuation_start: self.near_attenuation_start,
            near_attenuation_end: self.near_attenuation_end,
            attenuation_start: self.attenuation_start,
            attenuation_end: self.attenuation_end,
            range: self.range,
            transform: self.transform,
            transform_dirty: self.transform_dirty,
            light_id,
        }
    }

    /// Set position
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
        self.transform = Mat4::from_translation(position);
        self.transform_dirty = false;

        if self.light_type == LightType::Directional {
            // For directional lights, position doesn't affect lighting
            // but we still update it for consistency
        }
    }

    /// Get position
    pub fn get_position(&self) -> Vec3 {
        self.position
    }

    /// Set direction (for directional and spot lights)
    pub fn set_direction(&mut self, direction: Vec3) {
        self.direction = direction.normalize();
    }

    /// Get direction
    pub fn get_direction(&self) -> Vec3 {
        self.direction
    }

    /// Set diffuse color
    pub fn set_diffuse(&mut self, color: Vec4) {
        self.diffuse = color;
    }

    /// Get diffuse color
    pub fn get_diffuse(&self) -> Vec4 {
        self.diffuse
    }

    /// Set specular color
    pub fn set_specular(&mut self, color: Vec4) {
        self.specular = color;
    }

    /// Get specular color
    pub fn get_specular(&self) -> Vec4 {
        self.specular
    }

    /// Set ambient color
    pub fn set_ambient(&mut self, color: Vec4) {
        self.ambient = color;
    }

    /// Get ambient color
    pub fn get_ambient(&self) -> Vec4 {
        self.ambient
    }

    /// Set intensity
    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity.clamp(0.0, f32::INFINITY);
    }

    /// Get intensity
    pub fn get_intensity(&self) -> f32 {
        self.intensity
    }

    /// Set a C++ light flag. The values intentionally mirror `FlagsType`.
    pub fn set_flag(&mut self, flag: u32, onoff: bool) {
        if onoff {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    /// Get a C++ light flag. `NEAR_ATTENUATION` is zero in the original enum,
    /// so this method preserves the original `(Flags & flag) != 0` behavior.
    pub fn get_flag(&self, flag: u32) -> bool {
        (self.flags & flag) != 0
    }

    /// Enable or disable shadow casting.
    pub fn enable_shadows(&mut self, onoff: bool) {
        self.cast_shadows = onoff;
    }

    /// Check whether shadow casting is enabled.
    pub fn are_shadows_enabled(&self) -> bool {
        self.cast_shadows
    }

    /// Check whether culling should force this light visible.
    pub fn is_force_visible(&self) -> bool {
        self.force_visible
    }

    /// Set range (for point and spot lights)
    pub fn set_range(&mut self, range: f32) {
        self.range = range;
        self.attenuation_start =
            range * (DEFAULT_FAR_ATTENUATION_START / DEFAULT_FAR_ATTENUATION_END);
        self.attenuation_end = range;
    }

    /// Get range
    pub fn get_range(&self) -> f32 {
        self.range
    }

    /// Set near attenuation parameters.
    pub fn set_near_attenuation(&mut self, start: f32, end: f32) {
        self.near_attenuation_start = start;
        self.near_attenuation_end = end;
    }

    /// Get near attenuation parameters.
    pub fn get_near_attenuation(&self) -> (f32, f32) {
        (self.near_attenuation_start, self.near_attenuation_end)
    }

    /// Set spot angles (for spot lights)
    pub fn set_spot_angles(&mut self, inner: f32, outer: f32) {
        self.inner_cone_angle = inner;
        self.outer_cone_angle = outer;
    }

    /// Get spot angles
    pub fn get_spot_angles(&self) -> (f32, f32) {
        (self.inner_cone_angle, self.outer_cone_angle)
    }

    /// Set attenuation parameters
    pub fn set_attenuation(&mut self, start: f32, end: f32) {
        self.attenuation_start = start;
        self.attenuation_end = end;
        self.range = end;
    }

    /// Get attenuation parameters
    pub fn get_attenuation(&self) -> (f32, f32) {
        (self.attenuation_start, self.attenuation_end)
    }

    /// Set transform
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        // Extract position from transform
        self.position = transform.w_axis.truncate();
        self.transform_dirty = false;
    }

    /// Get transform
    pub fn get_transform(&self) -> Mat4 {
        self.transform
    }

    /// Get attenuation range
    pub fn get_attenuation_range(&self) -> f32 {
        self.attenuation_end
    }

    /// Get object space bounding sphere
    pub fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        SphereClass::new(Vec3::ZERO, self.get_attenuation_range())
    }

    /// Get object space bounding box
    pub fn get_obj_space_bounding_box(&self) -> AABoxClass {
        let range = self.get_attenuation_range();
        AABoxClass::from_center_and_extent(Vec3::ZERO, Vec3::new(range, range, range))
    }

    /// Push light to vertex processor
    pub fn vertex_processor_push(&self) -> Result<()> {
        let mut env = get_light_environment().ok_or_else(|| {
            Error::NotInitialized("Light environment not initialized".to_string())
        })?;
        env.remove_light(self.light_id);
        env.add_light(Arc::new(self.copy_with_light_id(self.light_id)))
    }

    /// Pop light from vertex processor
    pub fn vertex_processor_pop(&self) -> Result<()> {
        let mut env = get_light_environment().ok_or_else(|| {
            Error::NotInitialized("Light environment not initialized".to_string())
        })?;
        env.remove_light(self.light_id);
        Ok(())
    }

    /// Notify when light is added to scene
    pub fn notify_added(&mut self, scene: &mut SceneClass) -> Result<()> {
        scene.register_light_object(self.light_id as usize);
        Ok(())
    }

    /// Notify when light is removed from scene
    pub fn notify_removed(&mut self, scene: &mut SceneClass) -> Result<()> {
        scene.unregister_light_object(self.light_id as usize);
        Ok(())
    }

    /// Load light from W3D file
    pub fn load_w3d(&mut self, data: &[u8]) -> Result<()> {
        let mut outer_cursor = 0;
        let payload = match read_chunk(data, &mut outer_cursor)? {
            Some((W3D_CHUNK_LIGHT, payload)) if outer_cursor == data.len() => payload,
            _ => data,
        };

        let mut cursor = 0;
        let (chunk_type, light_info) = read_chunk(payload, &mut cursor)?
            .ok_or_else(|| Error::InvalidData("missing W3D light info chunk".to_string()))?;
        if chunk_type != W3D_CHUNK_LIGHT_INFO {
            return Err(Error::InvalidData(format!(
                "expected W3D_CHUNK_LIGHT_INFO, got 0x{chunk_type:08x}"
            )));
        }
        if light_info.len() < W3D_LIGHT_INFO_SIZE {
            return Err(Error::InvalidData("truncated W3D light info".to_string()));
        }

        let attributes = read_u32_le(light_info, 0)?;
        match attributes & W3D_LIGHT_ATTRIBUTE_TYPE_MASK {
            W3D_LIGHT_ATTRIBUTE_POINT => self.light_type = LightType::Point,
            W3D_LIGHT_ATTRIBUTE_DIRECTIONAL => self.light_type = LightType::Directional,
            W3D_LIGHT_ATTRIBUTE_SPOT => self.light_type = LightType::Spot,
            _ => {}
        }
        self.force_visible = self.light_type == LightType::Directional;

        self.enable_shadows((attributes & W3D_LIGHT_ATTRIBUTE_CAST_SHADOWS) != 0);
        self.ambient = w3d_rgb_to_vec4(light_info, 8)?;
        self.diffuse = w3d_rgb_to_vec4(light_info, 12)?;
        self.specular = w3d_rgb_to_vec4(light_info, 16)?;
        self.intensity = read_f32_le(light_info, 20)?;

        while let Some((chunk_type, chunk)) = read_chunk(payload, &mut cursor)? {
            match chunk_type {
                W3D_CHUNK_SPOT_LIGHT_INFO => {
                    if chunk.len() < W3D_SPOT_LIGHT_INFO_SIZE {
                        return Err(Error::InvalidData(
                            "truncated W3D spot light info".to_string(),
                        ));
                    }
                    self.direction = Vec3::new(
                        read_f32_le(chunk, 0)?,
                        read_f32_le(chunk, 4)?,
                        read_f32_le(chunk, 8)?,
                    );
                    let spot_angle = read_f32_le(chunk, 12)?;
                    self.inner_cone_angle = spot_angle;
                    self.outer_cone_angle = spot_angle;
                    self.spot_falloff = read_f32_le(chunk, 16)?;
                }
                W3D_CHUNK_NEAR_ATTENUATION => {
                    if chunk.len() < W3D_LIGHT_ATTENUATION_SIZE {
                        return Err(Error::InvalidData(
                            "truncated W3D near attenuation".to_string(),
                        ));
                    }
                    self.set_flag(LIGHT_FLAG_NEAR_ATTENUATION, true);
                    self.set_near_attenuation(read_f32_le(chunk, 0)?, read_f32_le(chunk, 4)?);
                }
                W3D_CHUNK_FAR_ATTENUATION => {
                    if chunk.len() < W3D_LIGHT_ATTENUATION_SIZE {
                        return Err(Error::InvalidData(
                            "truncated W3D far attenuation".to_string(),
                        ));
                    }
                    self.set_flag(LIGHT_FLAG_FAR_ATTENUATION, true);
                    self.set_attenuation(read_f32_le(chunk, 0)?, read_f32_le(chunk, 4)?);
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Save light to W3D file
    pub fn save_w3d(&self, writer: &mut dyn Write) -> Result<()> {
        let mut light_body = Vec::new();
        let mut light_info = Vec::with_capacity(W3D_LIGHT_INFO_SIZE);

        let mut attributes = match self.light_type {
            LightType::Point => W3D_LIGHT_ATTRIBUTE_POINT,
            LightType::Directional => W3D_LIGHT_ATTRIBUTE_DIRECTIONAL,
            LightType::Spot => W3D_LIGHT_ATTRIBUTE_SPOT,
        };
        if self.are_shadows_enabled() {
            attributes |= W3D_LIGHT_ATTRIBUTE_CAST_SHADOWS;
        }

        light_info.extend_from_slice(&attributes.to_le_bytes());
        light_info.extend_from_slice(&0u32.to_le_bytes());
        push_w3d_rgb(&mut light_info, self.ambient);
        push_w3d_rgb(&mut light_info, self.diffuse);
        push_w3d_rgb(&mut light_info, self.specular);
        light_info.extend_from_slice(&self.intensity.to_le_bytes());
        write_chunk_bytes(&mut light_body, W3D_CHUNK_LIGHT_INFO, &light_info, false);

        if self.light_type == LightType::Spot {
            let mut spot_info = Vec::with_capacity(W3D_SPOT_LIGHT_INFO_SIZE);
            spot_info.extend_from_slice(&self.direction.x.to_le_bytes());
            spot_info.extend_from_slice(&self.direction.y.to_le_bytes());
            spot_info.extend_from_slice(&self.direction.z.to_le_bytes());
            spot_info.extend_from_slice(&self.outer_cone_angle.to_le_bytes());
            spot_info.extend_from_slice(&self.spot_falloff.to_le_bytes());
            write_chunk_bytes(
                &mut light_body,
                W3D_CHUNK_SPOT_LIGHT_INFO,
                &spot_info,
                false,
            );
        }

        if self.get_flag(LIGHT_FLAG_NEAR_ATTENUATION) {
            let mut attenuation = Vec::with_capacity(W3D_LIGHT_ATTENUATION_SIZE);
            attenuation.extend_from_slice(&self.near_attenuation_start.to_le_bytes());
            attenuation.extend_from_slice(&self.near_attenuation_end.to_le_bytes());
            write_chunk_bytes(
                &mut light_body,
                W3D_CHUNK_NEAR_ATTENUATION,
                &attenuation,
                false,
            );
        }

        if self.get_flag(LIGHT_FLAG_FAR_ATTENUATION) {
            let mut attenuation = Vec::with_capacity(W3D_LIGHT_ATTENUATION_SIZE);
            attenuation.extend_from_slice(&self.attenuation_start.to_le_bytes());
            attenuation.extend_from_slice(&self.attenuation_end.to_le_bytes());
            write_chunk_bytes(
                &mut light_body,
                W3D_CHUNK_FAR_ATTENUATION,
                &attenuation,
                false,
            );
        }

        let mut out = Vec::with_capacity(light_body.len() + 8);
        write_chunk_bytes(&mut out, W3D_CHUNK_LIGHT, &light_body, true);
        writer
            .write_all(&out)
            .map_err(|e| Error::Generic(format!("failed to write W3D light: {e}")))?;
        Ok(())
    }

    /// Get light type
    pub fn get_type(&self) -> LightType {
        self.light_type
    }

    /// Check if light is directional
    pub fn is_directional(&self) -> bool {
        self.light_type == LightType::Directional
    }

    /// Check if light is point light
    pub fn is_point(&self) -> bool {
        self.light_type == LightType::Point
    }

    /// Check if light is spot light
    pub fn is_spot(&self) -> bool {
        self.light_type == LightType::Spot
    }

    /// Get light color (diffuse)
    pub fn get_color(&self) -> Vec4 {
        self.diffuse
    }

    /// Set light color
    pub fn set_color(&mut self, color: Vec4) {
        self.diffuse = color;
        self.specular = color;
    }

    /// Calculate light intensity at distance
    pub fn calculate_intensity_at_distance(&self, distance: f32) -> f32 {
        if self.light_type == LightType::Directional {
            return self.intensity;
        }

        if distance <= self.attenuation_start {
            return self.intensity;
        }

        if distance >= self.attenuation_end {
            return 0.0;
        }

        // Linear attenuation
        let factor = 1.0
            - ((distance - self.attenuation_start)
                / (self.attenuation_end - self.attenuation_start));
        self.intensity * factor
    }

    /// Calculate spot light factor
    pub fn calculate_spot_factor(&self, direction_to_light: Vec3) -> f32 {
        if self.light_type != LightType::Spot {
            return 1.0;
        }

        let cos_angle = self.direction.dot(-direction_to_light);
        let cos_inner = self.inner_cone_angle.cos();
        let cos_outer = self.outer_cone_angle.cos();

        if cos_angle > cos_inner {
            return 1.0; // Inside inner cone
        }

        if cos_angle < cos_outer {
            return 0.0; // Outside outer cone
        }

        // Between inner and outer cone - smooth falloff
        let factor = (cos_angle - cos_outer) / (cos_inner - cos_outer);
        factor.powf(self.spot_falloff)
    }

    /// Get light contribution at point
    pub fn get_contribution(&self, point: Vec3, normal: Vec3, view_dir: Vec3) -> LightContribution {
        let mut contribution = LightContribution {
            ambient: Vec4::ZERO,
            diffuse: Vec4::ZERO,
            specular: Vec4::ZERO,
        };

        match self.light_type {
            LightType::Directional => {
                let light_dir = -self.direction.normalize();
                let intensity = self.intensity;

                // Ambient
                contribution.ambient = self.ambient * intensity;

                // Diffuse
                let n_dot_l = normal.dot(light_dir).max(0.0);
                contribution.diffuse = self.diffuse * intensity * n_dot_l;

                // Specular (simplified Blinn-Phong)
                let half_vector = (light_dir + view_dir).normalize();
                let n_dot_h = normal.dot(half_vector).max(0.0);
                let specular_intensity = n_dot_h.powf(32.0); // Hardcoded shininess
                contribution.specular = self.specular * intensity * specular_intensity;
            }

            LightType::Point => {
                let to_light = self.position - point;
                let distance = to_light.length();
                let light_dir = to_light.normalize();

                let intensity = self.calculate_intensity_at_distance(distance);

                // Ambient
                contribution.ambient = self.ambient * intensity;

                // Diffuse
                let n_dot_l = normal.dot(light_dir).max(0.0);
                contribution.diffuse = self.diffuse * intensity * n_dot_l;

                // Specular
                let half_vector = (light_dir + view_dir).normalize();
                let n_dot_h = normal.dot(half_vector).max(0.0);
                let specular_intensity = n_dot_h.powf(32.0);
                contribution.specular = self.specular * intensity * specular_intensity;
            }

            LightType::Spot => {
                let to_light = self.position - point;
                let distance = to_light.length();
                let light_dir = to_light.normalize();

                let intensity = self.calculate_intensity_at_distance(distance);
                let spot_factor = self.calculate_spot_factor(light_dir);

                let final_intensity = intensity * spot_factor;

                // Ambient
                contribution.ambient = self.ambient * final_intensity;

                // Diffuse
                let n_dot_l = normal.dot(light_dir).max(0.0);
                contribution.diffuse = self.diffuse * final_intensity * n_dot_l;

                // Specular
                let half_vector = (light_dir + view_dir).normalize();
                let n_dot_h = normal.dot(half_vector).max(0.0);
                let specular_intensity = n_dot_h.powf(32.0);
                contribution.specular = self.specular * final_intensity * specular_intensity;
            }
        }

        contribution
    }
}

/// Light contribution structure
#[derive(Debug, Clone, Copy)]
pub struct LightContribution {
    /// Ambient contribution
    pub ambient: Vec4,
    /// Diffuse contribution
    pub diffuse: Vec4,
    /// Specular contribution
    pub specular: Vec4,
}

impl std::ops::Add for LightContribution {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            ambient: self.ambient + other.ambient,
            diffuse: self.diffuse + other.diffuse,
            specular: self.specular + other.specular,
        }
    }
}

impl std::ops::AddAssign for LightContribution {
    fn add_assign(&mut self, other: Self) {
        self.ambient += other.ambient;
        self.diffuse += other.diffuse;
        self.specular += other.specular;
    }
}

/// Light environment class - manages multiple lights
#[derive(Debug)]
pub struct LightEnvironmentClass {
    /// Ambient light color
    pub ambient: Vec4,
    /// Lights in the environment
    pub lights: Vec<Arc<LightClass>>,
    /// Maximum number of lights
    pub max_lights: usize,
    /// Whether environment is enabled
    pub enabled: bool,
}

impl LightEnvironmentClass {
    /// Create new light environment
    pub fn new() -> Self {
        Self {
            ambient: Vec4::new(0.2, 0.2, 0.2, 1.0),
            lights: Vec::new(),
            max_lights: 8, // Common limit for shader-based lighting
            enabled: true,
        }
    }

    /// Add light to environment
    pub fn add_light(&mut self, light: Arc<LightClass>) -> Result<()> {
        if self.lights.len() >= self.max_lights {
            return Err(Error::InvalidParameter(format!(
                "Maximum number of lights ({}) exceeded",
                self.max_lights
            )));
        }

        self.lights.push(light);
        Ok(())
    }

    /// Remove light from environment
    pub fn remove_light(&mut self, light_id: u32) -> bool {
        let old_len = self.lights.len();
        self.lights.retain(|light| light.light_id != light_id);
        self.lights.len() != old_len
    }

    /// Clear all lights
    pub fn clear_lights(&mut self) {
        self.lights.clear();
    }

    /// Get light contribution at point
    pub fn get_contribution(&self, point: Vec3, normal: Vec3, view_dir: Vec3) -> LightContribution {
        if !self.enabled {
            return LightContribution {
                ambient: self.ambient,
                diffuse: Vec4::ZERO,
                specular: Vec4::ZERO,
            };
        }

        let mut total_contribution = LightContribution {
            ambient: self.ambient,
            diffuse: Vec4::ZERO,
            specular: Vec4::ZERO,
        };

        for light in &self.lights {
            let contribution = light.get_contribution(point, normal, view_dir);
            total_contribution += contribution;
        }

        total_contribution
    }

    /// Get number of lights
    pub fn get_light_count(&self) -> usize {
        self.lights.len()
    }

    /// Get light by index
    pub fn get_light(&self, index: usize) -> Option<Arc<LightClass>> {
        self.lights.get(index).map(|light| Arc::clone(light))
    }

    /// Set ambient color
    pub fn set_ambient(&mut self, ambient: Vec4) {
        self.ambient = ambient;
    }

    /// Get ambient color
    pub fn get_ambient(&self) -> Vec4 {
        self.ambient
    }

    /// Set maximum lights
    pub fn set_max_lights(&mut self, max: usize) {
        self.max_lights = max;
        // Remove excess lights if any
        while self.lights.len() > self.max_lights {
            self.lights.pop();
        }
    }

    /// Enable/disable environment
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for LightEnvironmentClass {
    fn default() -> Self {
        Self::new()
    }
}

fn light_environment_slot() -> &'static Mutex<Option<LightEnvironmentClass>> {
    static SLOT: OnceLock<Mutex<Option<LightEnvironmentClass>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn lock_light_environment_slot() -> MutexGuard<'static, Option<LightEnvironmentClass>> {
    match light_environment_slot().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Handle used to access the shared light environment safely.
pub struct LightEnvironmentHandle<'a> {
    guard: MutexGuard<'a, Option<LightEnvironmentClass>>,
}

impl<'a> Deref for LightEnvironmentHandle<'a> {
    type Target = LightEnvironmentClass;

    fn deref(&self) -> &Self::Target {
        self.guard
            .as_ref()
            .expect("light environment must be initialized before use")
    }
}

impl<'a> DerefMut for LightEnvironmentHandle<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard
            .as_mut()
            .expect("light environment must be initialized before use")
    }
}

/// Initialize light system
pub fn init_light_system() -> Result<()> {
    let mut guard = lock_light_environment_slot();
    *guard = Some(LightEnvironmentClass::default());
    Ok(())
}

/// Shutdown light system
pub fn shutdown_light_system() {
    let mut guard = lock_light_environment_slot();
    *guard = None;
}

/// Get light environment instance
pub fn get_light_environment() -> Option<LightEnvironmentHandle<'static>> {
    let guard = lock_light_environment_slot();
    if guard.is_none() {
        None
    } else {
        Some(LightEnvironmentHandle { guard })
    }
}

/// Quick light creation functions
pub fn create_directional_light(direction: Vec3, color: Vec4) -> LightClass {
    LightClass::new_directional(direction, color)
}

pub fn create_point_light(position: Vec3, color: Vec4, range: f32) -> LightClass {
    LightClass::new_point(position, color, range)
}

pub fn create_spot_light(
    position: Vec3,
    direction: Vec3,
    color: Vec4,
    range: f32,
    inner_angle: f32,
    outer_angle: f32,
) -> LightClass {
    LightClass::new_spot(position, direction, color, range, inner_angle, outer_angle)
}

/// Quick light environment functions
pub fn add_light_to_environment(light: LightClass) -> Result<()> {
    let mut env = get_light_environment()
        .ok_or_else(|| Error::NotInitialized("Light environment not initialized".to_string()))?;

    env.add_light(Arc::new(light))
}

pub fn get_light_contribution(point: Vec3, normal: Vec3, view_dir: Vec3) -> LightContribution {
    if let Some(env) = get_light_environment() {
        env.get_contribution(point, normal, view_dir)
    } else {
        LightContribution {
            ambient: Vec4::new(0.2, 0.2, 0.2, 1.0),
            diffuse: Vec4::ZERO,
            specular: Vec4::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use glam::Quat;

    fn assert_vec4_close(actual: Vec4, expected: Vec4) {
        assert!((actual.x - expected.x).abs() <= (1.0 / 255.0));
        assert!((actual.y - expected.y).abs() <= (1.0 / 255.0));
        assert!((actual.z - expected.z).abs() <= (1.0 / 255.0));
        assert!((actual.w - expected.w).abs() <= f32::EPSILON);
    }

    #[test]
    fn test_light_type_order_matches_cpp() {
        assert_eq!(LightType::Point as u32, 0);
        assert_eq!(LightType::Directional as u32, 1);
        assert_eq!(LightType::Spot as u32, 2);
    }

    #[test]
    fn test_light_constructor_matches_cpp_defaults() {
        let light = LightClass::new(LightType::Spot);

        assert_eq!(light.light_type, LightType::Spot);
        assert_eq!(light.flags, 0);
        assert!(!light.cast_shadows);
        assert!(!light.is_force_visible());
        assert_eq!(light.ambient, Vec4::ONE);
        assert_eq!(light.diffuse, Vec4::ONE);
        assert_eq!(light.specular, Vec4::ONE);
        assert_eq!(light.intensity, 1.0);
        assert_eq!(light.near_attenuation_start, 0.0);
        assert_eq!(light.near_attenuation_end, 0.0);
        assert_eq!(light.attenuation_start, 50.0);
        assert_eq!(light.attenuation_end, 100.0);
        assert_eq!(light.range, 100.0);
        assert!((light.inner_cone_angle - std::f32::consts::FRAC_PI_4).abs() < f32::EPSILON);
        assert!((light.outer_cone_angle - std::f32::consts::FRAC_PI_4).abs() < f32::EPSILON);
        assert_eq!(light.spot_falloff, 1.0);
        assert_eq!(light.direction, Vec3::Z);
    }

    #[test]
    fn test_light_w3d_save_load_round_trip_matches_cpp_chunks() {
        let mut light = LightClass::new(LightType::Spot);
        light.enable_shadows(true);
        light.set_ambient(Vec4::new(0.25, 0.5, 0.75, 1.0));
        light.set_diffuse(Vec4::new(1.0, 0.5, 0.25, 1.0));
        light.set_specular(Vec4::new(0.1, 0.2, 0.3, 1.0));
        light.set_intensity(2.5);
        light.set_direction(Vec3::new(0.0, 1.0, 0.0));
        light.set_spot_angles(0.4, 0.6);
        light.spot_falloff = 3.0;
        light.set_attenuation(20.0, 80.0);
        light.set_flag(LIGHT_FLAG_FAR_ATTENUATION, true);

        let mut bytes = Vec::new();
        light.save_w3d(&mut bytes).unwrap();

        assert_eq!(read_u32_le(&bytes, 0).unwrap(), W3D_CHUNK_LIGHT);
        assert_ne!(read_u32_le(&bytes, 4).unwrap() & W3D_CHUNK_HAS_SUBCHUNKS, 0);
        assert_eq!(read_u32_le(&bytes, 8).unwrap(), W3D_CHUNK_LIGHT_INFO);

        let mut loaded = LightClass::new(LightType::Point);
        loaded.load_w3d(&bytes).unwrap();

        assert_eq!(loaded.light_type, LightType::Spot);
        assert!(loaded.are_shadows_enabled());
        assert_vec4_close(loaded.ambient, Vec4::new(0.25, 0.5, 0.75, 1.0));
        assert_vec4_close(loaded.diffuse, Vec4::new(1.0, 0.5, 0.25, 1.0));
        assert_vec4_close(loaded.specular, Vec4::new(0.1, 0.2, 0.3, 1.0));
        assert_eq!(loaded.intensity, 2.5);
        assert_eq!(loaded.direction, Vec3::Y);
        assert_eq!(loaded.inner_cone_angle, 0.6);
        assert_eq!(loaded.outer_cone_angle, 0.6);
        assert_eq!(loaded.spot_falloff, 3.0);
        assert_eq!(loaded.get_attenuation(), (20.0, 80.0));
        assert_eq!(loaded.get_range(), 80.0);
        assert!(loaded.get_flag(LIGHT_FLAG_FAR_ATTENUATION));
    }

    #[test]
    fn test_light_w3d_load_accepts_open_light_payload() {
        let mut bytes = Vec::new();
        LightClass::new(LightType::Directional)
            .save_w3d(&mut bytes)
            .unwrap();

        let payload_size = (read_u32_le(&bytes, 4).unwrap() & W3D_CHUNK_SIZE_MASK) as usize;
        let payload = &bytes[8..8 + payload_size];
        let mut loaded = LightClass::new(LightType::Point);

        loaded.load_w3d(payload).unwrap();

        assert_eq!(loaded.light_type, LightType::Directional);
        assert_eq!(loaded.ambient, Vec4::ONE);
        assert_eq!(loaded.diffuse, Vec4::ONE);
        assert_eq!(loaded.specular, Vec4::ONE);
        assert!(loaded.is_force_visible());
    }

    #[test]
    fn test_directional_light_creation() {
        let direction = Vec3::new(0.0, -1.0, 0.0);
        let color = Vec4::new(1.0, 1.0, 1.0, 1.0);
        let light = LightClass::new_directional(direction, color);

        assert_eq!(light.light_type, LightType::Directional);
        assert_eq!(light.direction, direction.normalize());
        assert_eq!(light.ambient, Vec4::ONE);
        assert_eq!(light.diffuse, color);
        assert!(light.is_force_visible());
    }

    #[test]
    fn test_point_light_creation() {
        let position = Vec3::new(10.0, 0.0, 0.0);
        let color = Vec4::new(1.0, 0.0, 0.0, 1.0);
        let range = 100.0;
        let light = LightClass::new_point(position, color, range);

        assert_eq!(light.light_type, LightType::Point);
        assert_eq!(light.position, position);
        assert_eq!(light.range, range);
        assert_eq!(light.diffuse, color);
    }

    #[test]
    fn test_spot_light_creation() {
        let position = Vec3::new(0.0, 0.0, 10.0);
        let direction = Vec3::new(0.0, 0.0, -1.0);
        let color = Vec4::new(0.0, 1.0, 0.0, 1.0);
        let range = 50.0;
        let inner_angle = 0.1;
        let outer_angle = 0.5;
        let light =
            LightClass::new_spot(position, direction, color, range, inner_angle, outer_angle);

        assert_eq!(light.light_type, LightType::Spot);
        assert_eq!(light.position, position);
        assert_eq!(light.direction, direction.normalize());
        assert_eq!(light.inner_cone_angle, inner_angle);
        assert_eq!(light.outer_cone_angle, outer_angle);
    }

    #[test]
    fn test_set_transform_uses_glam_translation_axis() {
        let mut light = LightClass::new_point(Vec3::ZERO, Vec4::ONE, 100.0);
        let position = Vec3::new(12.0, -4.0, 7.5);
        let transform = Mat4::from_rotation_translation(Quat::from_rotation_y(0.5), position);

        light.set_transform(transform);

        assert_eq!(light.get_position(), position);
        assert_eq!(light.get_transform(), transform);
    }

    #[test]
    fn test_notify_added_removed_registers_scene_light_like_cpp() {
        let mut scene = SceneClass::new();
        let mut light = LightClass::new_point(Vec3::ZERO, Vec4::ONE, 100.0);
        let id = light.light_id as usize;

        light.notify_added(&mut scene).unwrap();
        light.notify_added(&mut scene).unwrap();

        assert!(scene.is_light_registered(id));
        assert_eq!(scene.registered_light_count(), 1);

        light.notify_removed(&mut scene).unwrap();

        assert!(!scene.is_light_registered(id));
        assert_eq!(scene.registered_light_count(), 0);
    }

    #[test]
    fn test_light_push_pop_updates_environment() {
        init_light_system().unwrap();
        let light = LightClass::new_point(Vec3::new(1.0, 2.0, 3.0), Vec4::ONE, 100.0);
        let light_id = light.light_id;

        light.vertex_processor_push().unwrap();
        {
            let env = get_light_environment().unwrap();
            assert_eq!(env.get_light_count(), 1);
            assert_eq!(env.get_light(0).unwrap().light_id, light_id);
        }

        light.vertex_processor_push().unwrap();
        {
            let env = get_light_environment().unwrap();
            assert_eq!(env.get_light_count(), 1);
            assert_eq!(env.get_light(0).unwrap().light_id, light_id);
        }

        light.vertex_processor_pop().unwrap();
        {
            let env = get_light_environment().unwrap();
            assert_eq!(env.get_light_count(), 0);
        }
        shutdown_light_system();
    }

    #[test]
    fn test_light_intensity_calculation() {
        let light = LightClass::new_point(Vec3::ZERO, Vec4::ONE, 10.0);

        assert_eq!(light.calculate_intensity_at_distance(0.0), 1.0);
        assert_eq!(light.calculate_intensity_at_distance(5.0), 1.0); // Within attenuation start
        assert_eq!(light.calculate_intensity_at_distance(10.0), 0.0); // At attenuation end
        assert_eq!(light.calculate_intensity_at_distance(15.0), 0.0); // Beyond attenuation end
    }

    #[test]
    fn test_spot_light_factor() {
        let light = LightClass::new_spot(Vec3::ZERO, Vec3::Z, Vec4::ONE, 10.0, 0.1, 0.5);

        // Light direction is +Z, so -Z direction should have full intensity
        let factor = light.calculate_spot_factor(-Vec3::Z);
        assert_eq!(factor, 1.0);

        // Perpendicular direction should have zero intensity
        let factor = light.calculate_spot_factor(Vec3::X);
        assert_eq!(factor, 0.0);
    }

    #[test]
    fn test_light_environment() {
        let mut env = LightEnvironmentClass::new();
        assert_eq!(env.get_light_count(), 0);

        let light = Arc::new(LightClass::new_directional(Vec3::Y, Vec4::ONE));
        env.add_light(Arc::clone(&light)).unwrap();
        assert_eq!(env.get_light_count(), 1);

        env.clear_lights();
        assert_eq!(env.get_light_count(), 0);
    }

    #[test]
    fn test_light_contribution() {
        let light = LightClass::new_directional(-Vec3::Z, Vec4::ONE);
        let point = Vec3::ZERO;
        let normal = Vec3::Z;
        let view_dir = Vec3::Z;

        let contribution = light.get_contribution(point, normal, view_dir);

        // For directional light pointing down -Z, hitting surface with +Z normal
        // should give full diffuse contribution
        assert_eq!(contribution.diffuse, Vec4::ONE);
        assert_eq!(contribution.specular, Vec4::ONE);
    }

    #[test]
    fn test_light_clone() {
        let original = LightClass::new_point(Vec3::ONE, Vec4::new(1.0, 0.0, 0.0, 1.0), 50.0);
        let cloned = original.clone();

        assert_eq!(original.light_type, cloned.light_type);
        assert_eq!(original.position, cloned.position);
        assert_eq!(original.diffuse, cloned.diffuse);
        assert_ne!(original.light_id, cloned.light_id); // IDs should be different
    }
}
