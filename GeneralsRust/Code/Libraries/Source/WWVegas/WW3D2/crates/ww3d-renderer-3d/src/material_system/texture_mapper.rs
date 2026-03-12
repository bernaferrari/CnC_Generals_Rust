//! Texture Mapper System for WW3D2 Engine
//!
//! This module implements the complete texture coordinate transformation system
//! based on the C++ WW3D engine's mapper architecture. It provides 19 different
//! mapping types for UV coordinate manipulation.
//!
//! C++ References:
//! - gamemtl.h lines 74-92: Mapper type definitions (GAMEMTL_MAPPING_*)
//! - w3dmtl.cpp lines 514-558: Mapper type to attribute mapping
//! - Material pass mapper implementation in matpass.h/cpp

use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::PI;

/// Texture mapper type enumeration
///
/// C++ Reference: gamemtl.h lines 74-92
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum TextureMapperType {
    /// Standard UV mapping - use mesh UV coordinates directly
    /// C++ Reference: gamemtl.h line 74 (GAMEMTL_MAPPING_UV = 0)
    UV = 0,

    /// Environment mapping - sphere map reflection
    /// C++ Reference: gamemtl.h line 75 (GAMEMTL_MAPPING_ENV = 1)
    Environment = 1,

    /// Cheap environment mapping - simplified reflection
    /// C++ Reference: gamemtl.h line 76 (GAMEMTL_MAPPING_CHEAP_ENV = 2)
    CheapEnvironment = 2,

    /// Screen-space mapping - project to screen coordinates
    /// C++ Reference: gamemtl.h line 77 (GAMEMTL_MAPPING_SCREEN = 3)
    Screen = 3,

    /// Linear offset - translate UV coordinates over time
    /// C++ Reference: gamemtl.h line 78 (GAMEMTL_MAPPING_LINEAR_OFFSET = 4)
    LinearOffset = 4,

    /// Silhouette mapping - edge-based UV generation
    /// C++ Reference: gamemtl.h line 79 (GAMEMTL_MAPPING_SILHOUETTE = 5)
    Silhouette = 5,

    /// Scale mapping - scale UV coordinates
    /// C++ Reference: gamemtl.h line 80 (GAMEMTL_MAPPING_SCALE = 6)
    Scale = 6,

    /// Grid mapping - tile UV coordinates
    /// C++ Reference: gamemtl.h line 81 (GAMEMTL_MAPPING_GRID = 7)
    Grid = 7,

    /// Rotate mapping - rotate UV coordinates
    /// C++ Reference: gamemtl.h line 82 (GAMEMTL_MAPPING_ROTATE = 8)
    Rotate = 8,

    /// Sine linear offset - sinusoidal UV animation
    /// C++ Reference: gamemtl.h line 83 (GAMEMTL_MAPPING_SINE_LINEAR_OFFSET = 9)
    SineLinearOffset = 9,

    /// Step linear offset - stepped UV animation
    /// C++ Reference: gamemtl.h line 84 (GAMEMTL_MAPPING_STEP_LINEAR_OFFSET = 10)
    StepLinearOffset = 10,

    /// Zigzag linear offset - back-and-forth UV animation
    /// C++ Reference: gamemtl.h line 85 (GAMEMTL_MAPPING_ZIGZAG_LINEAR_OFFSET = 11)
    ZigZagLinearOffset = 11,

    /// Classic Westwood environment mapping
    /// C++ Reference: gamemtl.h line 86 (GAMEMTL_MAPPING_WS_CLASSIC_ENV = 12)
    WSClassicEnvironment = 12,

    /// Westwood environment mapping
    /// C++ Reference: gamemtl.h line 87 (GAMEMTL_MAPPING_WS_ENVIRONMENT = 13)
    WSEnvironment = 13,

    /// Grid with classic environment mapping
    /// C++ Reference: gamemtl.h line 88 (GAMEMTL_MAPPING_GRID_CLASSIC_ENV = 14)
    GridClassicEnvironment = 14,

    /// Grid with environment mapping
    /// C++ Reference: gamemtl.h line 89 (GAMEMTL_MAPPING_GRID_ENVIRONMENT = 15)
    GridEnvironment = 15,

    /// Random UV offset mapping
    /// C++ Reference: gamemtl.h line 90 (GAMEMTL_MAPPING_RANDOM = 16)
    Random = 16,

    /// Edge mapping - highlight edges
    /// C++ Reference: gamemtl.h line 91 (GAMEMTL_MAPPING_EDGE = 17)
    Edge = 17,

    /// Bump environment mapping
    /// C++ Reference: gamemtl.h line 92 (GAMEMTL_MAPPING_BUMPENV = 18)
    BumpEnvironment = 18,
}

impl TextureMapperType {
    /// Convert from u32 value
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::UV),
            1 => Some(Self::Environment),
            2 => Some(Self::CheapEnvironment),
            3 => Some(Self::Screen),
            4 => Some(Self::LinearOffset),
            5 => Some(Self::Silhouette),
            6 => Some(Self::Scale),
            7 => Some(Self::Grid),
            8 => Some(Self::Rotate),
            9 => Some(Self::SineLinearOffset),
            10 => Some(Self::StepLinearOffset),
            11 => Some(Self::ZigZagLinearOffset),
            12 => Some(Self::WSClassicEnvironment),
            13 => Some(Self::WSEnvironment),
            14 => Some(Self::GridClassicEnvironment),
            15 => Some(Self::GridEnvironment),
            16 => Some(Self::Random),
            17 => Some(Self::Edge),
            18 => Some(Self::BumpEnvironment),
            _ => None,
        }
    }

    /// Check if this mapper type requires world/camera transforms
    pub fn requires_transforms(&self) -> bool {
        matches!(
            self,
            Self::Environment
                | Self::CheapEnvironment
                | Self::Screen
                | Self::Silhouette
                | Self::WSClassicEnvironment
                | Self::WSEnvironment
                | Self::GridClassicEnvironment
                | Self::GridEnvironment
                | Self::BumpEnvironment
        )
    }

    /// Check if this mapper is time-animated
    pub fn is_animated(&self) -> bool {
        matches!(
            self,
            Self::LinearOffset
                | Self::SineLinearOffset
                | Self::StepLinearOffset
                | Self::ZigZagLinearOffset
                | Self::Rotate
        )
    }
}

/// Texture mapping context - provides all data needed for UV transformation
#[derive(Debug, Clone)]
pub struct TextureMappingContext {
    /// Vertex position in object space
    pub position: Vec3,

    /// Vertex normal in object space
    pub normal: Vec3,

    /// Original UV coordinates from mesh
    pub uv: Vec2,

    /// World transform matrix
    pub world_matrix: Mat4,

    /// View matrix (camera)
    pub view_matrix: Mat4,

    /// Projection matrix
    pub projection_matrix: Mat4,

    /// Current animation time in seconds
    pub time: f32,

    /// Camera position in world space
    pub camera_position: Vec3,

    /// Camera direction (view vector)
    pub camera_direction: Vec3,
}

impl Default for TextureMappingContext {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            normal: Vec3::Y,
            uv: Vec2::ZERO,
            world_matrix: Mat4::IDENTITY,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            time: 0.0,
            camera_position: Vec3::ZERO,
            camera_direction: -Vec3::Z,
        }
    }
}

/// Texture mapper parameters - configures mapper behavior
#[derive(Debug, Clone)]
pub struct TextureMapperParams {
    /// Mapper type
    pub mapper_type: TextureMapperType,

    /// Generic integer arguments (up to 4)
    /// Usage varies by mapper type
    pub args: [i32; 4],

    /// Generic float arguments for advanced control
    pub float_args: [f32; 4],

    /// UV channel to use (0-7)
    pub uv_channel: u32,
}

impl Default for TextureMapperParams {
    fn default() -> Self {
        Self {
            mapper_type: TextureMapperType::UV,
            args: [0, 0, 0, 0],
            float_args: [1.0, 1.0, 0.0, 0.0],
            uv_channel: 0,
        }
    }
}

/// Core texture mapper trait
pub trait TextureMapper: Send + Sync {
    /// Transform UV coordinates based on mapper type
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2;

    /// Get the mapper type
    fn mapper_type(&self) -> TextureMapperType;

    /// Check if this mapper needs per-frame updates
    fn is_animated(&self) -> bool {
        self.mapper_type().is_animated()
    }
}

// ============================================================================
// FOUNDATION MAPPERS - Most commonly used mappers
// ============================================================================

/// Standard UV Mapper - passes through mesh UV coordinates
///
/// C++ Reference: gamemtl.h line 74
pub struct UVMapper;

impl TextureMapper for UVMapper {
    fn map_texture(&self, context: &TextureMappingContext, _params: &TextureMapperParams) -> Vec2 {
        context.uv
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::UV
    }
}

/// Linear Offset Mapper - scrolls UVs at constant velocity
///
/// Args:
/// - args[0]: U speed (units per second * 1000)
/// - args[1]: V speed (units per second * 1000)
///
/// C++ Reference: gamemtl.h line 78
pub struct LinearOffsetMapper;

impl TextureMapper for LinearOffsetMapper {
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2 {
        let u_speed = params.args[0] as f32 / 1000.0;
        let v_speed = params.args[1] as f32 / 1000.0;

        Vec2::new(
            context.uv.x + u_speed * context.time,
            context.uv.y + v_speed * context.time,
        )
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::LinearOffset
    }
}

/// Grid Mapper - tiles UVs into repeating grid
///
/// Args:
/// - args[0]: U tiles (columns)
/// - args[1]: V tiles (rows)
/// - args[2]: Optional U offset
/// - args[3]: Optional V offset
///
/// C++ Reference: gamemtl.h line 81
pub struct GridMapper;

impl TextureMapper for GridMapper {
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2 {
        let u_tiles = (params.args[0].max(1)) as f32;
        let v_tiles = (params.args[1].max(1)) as f32;
        let u_offset = params.args[2] as f32 / 1000.0;
        let v_offset = params.args[3] as f32 / 1000.0;

        Vec2::new(
            (context.uv.x * u_tiles) + u_offset,
            (context.uv.y * v_tiles) + v_offset,
        )
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::Grid
    }
}

/// Rotate Mapper - rotates UVs around center point
///
/// Args:
/// - args[0]: Rotation speed (degrees per second * 100)
/// - args[1]: Center U coordinate * 1000
/// - args[2]: Center V coordinate * 1000
///
/// C++ Reference: gamemtl.h line 82
pub struct RotateMapper;

impl TextureMapper for RotateMapper {
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2 {
        let rotation_speed = (params.args[0] as f32 / 100.0) * PI / 180.0; // Convert to radians
        let center_u = params.args[1] as f32 / 1000.0;
        let center_v = params.args[2] as f32 / 1000.0;

        let angle = rotation_speed * context.time;
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();

        // Translate to origin
        let u = context.uv.x - center_u;
        let v = context.uv.y - center_v;

        // Rotate
        let rotated_u = u * cos_angle - v * sin_angle;
        let rotated_v = u * sin_angle + v * cos_angle;

        // Translate back
        Vec2::new(rotated_u + center_u, rotated_v + center_v)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::Rotate
    }
}

/// Scale Mapper - scales UV coordinates
///
/// Args:
/// - args[0]: U scale * 1000 (1000 = 1.0x scale)
/// - args[1]: V scale * 1000 (1000 = 1.0x scale)
///
/// C++ Reference: gamemtl.h line 80
pub struct ScaleMapper;

impl TextureMapper for ScaleMapper {
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2 {
        let u_scale = if params.args[0] != 0 {
            params.args[0] as f32 / 1000.0
        } else {
            1.0
        };
        let v_scale = if params.args[1] != 0 {
            params.args[1] as f32 / 1000.0
        } else {
            1.0
        };

        Vec2::new(context.uv.x * u_scale, context.uv.y * v_scale)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::Scale
    }
}

// ============================================================================
// ENVIRONMENT MAPPERS - Reflection mapping
// ============================================================================

/// Environment Mapper - sphere map reflection
///
/// Generates UV coordinates based on the reflection vector
///
/// C++ Reference: gamemtl.h line 75
pub struct EnvironmentMapper;

impl TextureMapper for EnvironmentMapper {
    fn map_texture(&self, context: &TextureMappingContext, _params: &TextureMapperParams) -> Vec2 {
        // Transform normal to world space
        let world_normal = (context.world_matrix * context.normal.extend(0.0))
            .truncate()
            .normalize();

        // Calculate view vector
        let view_dir = (context.camera_position - context.position).normalize();

        // Calculate reflection vector
        let reflection = view_dir - 2.0 * world_normal.dot(view_dir) * world_normal;

        // Convert reflection to sphere map coordinates
        let m = 2.0 * (reflection.z + 1.0).sqrt();

        Vec2::new(reflection.x / m + 0.5, reflection.y / m + 0.5)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::Environment
    }
}

/// Cheap Environment Mapper - simplified reflection
///
/// Uses vertex normal directly without accurate reflection calculation
///
/// C++ Reference: gamemtl.h line 76
pub struct CheapEnvironmentMapper;

impl TextureMapper for CheapEnvironmentMapper {
    fn map_texture(&self, context: &TextureMappingContext, _params: &TextureMapperParams) -> Vec2 {
        // Transform normal to view space
        let view_normal = (context.view_matrix * context.world_matrix * context.normal.extend(0.0))
            .truncate()
            .normalize();

        // Simple sphere map approximation
        Vec2::new(view_normal.x * 0.5 + 0.5, view_normal.y * 0.5 + 0.5)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::CheapEnvironment
    }
}

/// Classic Westwood Environment Mapper
///
/// Legacy environment mapping used in older Westwood games
///
/// C++ Reference: gamemtl.h line 86
pub struct WSClassicEnvironmentMapper;

impl TextureMapper for WSClassicEnvironmentMapper {
    fn map_texture(&self, context: &TextureMappingContext, _params: &TextureMapperParams) -> Vec2 {
        // Classic WW environment mapping using camera-space normal
        let view_normal = (context.view_matrix * context.world_matrix * context.normal.extend(0.0))
            .truncate()
            .normalize();

        // Classic mapping formula
        Vec2::new(view_normal.x + 0.5, -view_normal.y + 0.5)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::WSClassicEnvironment
    }
}

/// Westwood Environment Mapper
///
/// Improved environment mapping for WW3D
///
/// C++ Reference: gamemtl.h line 87
pub struct WSEnvironmentMapper;

impl TextureMapper for WSEnvironmentMapper {
    fn map_texture(&self, context: &TextureMappingContext, _params: &TextureMapperParams) -> Vec2 {
        // Similar to standard environment mapping but with WW-specific tweaks
        let world_normal = (context.world_matrix * context.normal.extend(0.0))
            .truncate()
            .normalize();
        let view_dir = (context.camera_position - context.position).normalize();
        let reflection = view_dir - 2.0 * world_normal.dot(view_dir) * world_normal;

        // WW-specific sphere map formula
        let m = (reflection.x * reflection.x
            + reflection.y * reflection.y
            + (reflection.z + 1.0).powi(2))
        .sqrt();

        Vec2::new(reflection.x / m + 0.5, -reflection.y / m + 0.5)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::WSEnvironment
    }
}

// ============================================================================
// SCREEN MAPPER - Screen-space projection
// ============================================================================

/// Screen Mapper - projects texture to screen space
///
/// Useful for HUD overlays, decals, and projected textures
///
/// C++ Reference: gamemtl.h line 77
pub struct ScreenMapper;

impl TextureMapper for ScreenMapper {
    fn map_texture(&self, context: &TextureMappingContext, _params: &TextureMapperParams) -> Vec2 {
        // Transform position to clip space
        let world_pos = context.world_matrix * context.position.extend(1.0);
        let clip_pos = context.projection_matrix * context.view_matrix * world_pos;

        // Perspective divide
        let ndc = clip_pos.truncate() / clip_pos.w;

        // Convert to UV coordinates (0-1 range)
        Vec2::new(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::Screen
    }
}

// ============================================================================
// ANIMATED MAPPERS - Time-based UV animations
// ============================================================================

/// Sine Linear Offset Mapper - sinusoidal UV scrolling
///
/// Args:
/// - args[0]: U amplitude * 1000
/// - args[1]: V amplitude * 1000
/// - args[2]: Frequency (cycles per second * 100)
/// - args[3]: Phase offset * 100
///
/// C++ Reference: gamemtl.h line 83
pub struct SineLinearOffsetMapper;

impl TextureMapper for SineLinearOffsetMapper {
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2 {
        let u_amp = params.args[0] as f32 / 1000.0;
        let v_amp = params.args[1] as f32 / 1000.0;
        let frequency = params.args[2] as f32 / 100.0;
        let phase = (params.args[3] as f32 / 100.0) * PI / 180.0;

        let angle = 2.0 * PI * frequency * context.time + phase;
        let wave = angle.sin();

        Vec2::new(context.uv.x + u_amp * wave, context.uv.y + v_amp * wave)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::SineLinearOffset
    }
}

/// Step Linear Offset Mapper - stepped UV scrolling
///
/// Args:
/// - args[0]: U step size * 1000
/// - args[1]: V step size * 1000
/// - args[2]: Steps per second * 100
///
/// C++ Reference: gamemtl.h line 84
pub struct StepLinearOffsetMapper;

impl TextureMapper for StepLinearOffsetMapper {
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2 {
        let u_step = params.args[0] as f32 / 1000.0;
        let v_step = params.args[1] as f32 / 1000.0;
        let steps_per_sec = (params.args[2] as f32 / 100.0).max(0.001);

        let step_count = (context.time * steps_per_sec).floor();

        Vec2::new(
            context.uv.x + u_step * step_count,
            context.uv.y + v_step * step_count,
        )
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::StepLinearOffset
    }
}

/// ZigZag Linear Offset Mapper - back-and-forth UV animation
///
/// Args:
/// - args[0]: U range * 1000
/// - args[1]: V range * 1000
/// - args[2]: Cycle time (seconds * 100)
///
/// C++ Reference: gamemtl.h line 85
pub struct ZigZagLinearOffsetMapper;

impl TextureMapper for ZigZagLinearOffsetMapper {
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2 {
        let u_range = params.args[0] as f32 / 1000.0;
        let v_range = params.args[1] as f32 / 1000.0;
        let cycle_time = (params.args[2] as f32 / 100.0).max(0.001);

        // Calculate position in cycle (0.0 to 1.0)
        let cycle_pos = (context.time % cycle_time) / cycle_time;

        // ZigZag: 0->1->0
        let zigzag = if cycle_pos < 0.5 {
            cycle_pos * 2.0
        } else {
            2.0 - cycle_pos * 2.0
        };

        Vec2::new(
            context.uv.x + u_range * zigzag,
            context.uv.y + v_range * zigzag,
        )
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::ZigZagLinearOffset
    }
}

// ============================================================================
// GRID ENVIRONMENT MAPPERS - Combines grid tiling with environment mapping
// ============================================================================

/// Grid Classic Environment Mapper
///
/// Applies grid tiling to classic environment map coordinates
///
/// C++ Reference: gamemtl.h line 88
pub struct GridClassicEnvironmentMapper;

impl TextureMapper for GridClassicEnvironmentMapper {
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2 {
        // First apply classic environment mapping
        let env_mapper = WSClassicEnvironmentMapper;
        let env_uv = env_mapper.map_texture(context, params);

        // Then apply grid tiling
        let u_tiles = (params.args[0].max(1)) as f32;
        let v_tiles = (params.args[1].max(1)) as f32;

        Vec2::new(env_uv.x * u_tiles, env_uv.y * v_tiles)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::GridClassicEnvironment
    }
}

/// Grid Environment Mapper
///
/// Applies grid tiling to environment map coordinates
///
/// C++ Reference: gamemtl.h line 89
pub struct GridEnvironmentMapper;

impl TextureMapper for GridEnvironmentMapper {
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2 {
        // First apply WS environment mapping
        let env_mapper = WSEnvironmentMapper;
        let env_uv = env_mapper.map_texture(context, params);

        // Then apply grid tiling
        let u_tiles = (params.args[0].max(1)) as f32;
        let v_tiles = (params.args[1].max(1)) as f32;

        Vec2::new(env_uv.x * u_tiles, env_uv.y * v_tiles)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::GridEnvironment
    }
}

// ============================================================================
// ADVANCED MAPPERS - Specialized effects
// ============================================================================

/// Silhouette Mapper - edge-based UV generation
///
/// Generates UVs based on the angle between view direction and surface normal
/// Useful for rim lighting and edge effects
///
/// C++ Reference: gamemtl.h line 79
pub struct SilhouetteMapper;

impl TextureMapper for SilhouetteMapper {
    fn map_texture(&self, context: &TextureMappingContext, _params: &TextureMapperParams) -> Vec2 {
        let world_normal = (context.world_matrix * context.normal.extend(0.0))
            .truncate()
            .normalize();
        let view_dir = (context.camera_position - context.position).normalize();

        // Calculate edge intensity (1.0 at edges, 0.0 facing camera)
        let edge_factor = 1.0 - world_normal.dot(view_dir).abs();

        Vec2::new(edge_factor, context.uv.y)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::Silhouette
    }
}

/// Random Mapper - randomized UV coordinates
///
/// Args:
/// - args[0]: Random seed
/// - args[1]: U variance * 1000
/// - args[2]: V variance * 1000
///
/// C++ Reference: gamemtl.h line 90
pub struct RandomMapper;

impl TextureMapper for RandomMapper {
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2 {
        let seed = params.args[0] as u32;
        let u_variance = params.args[1] as f32 / 1000.0;
        let v_variance = params.args[2] as f32 / 1000.0;

        // Simple hash function for pseudo-random values
        let hash = |x: u32| -> f32 {
            let x = x.wrapping_mul(0x9E3779B9);
            let x = x ^ (x >> 16);
            let x = x.wrapping_mul(0x85EBCA6B);
            let x = x ^ (x >> 13);
            (x as f32 / u32::MAX as f32) * 2.0 - 1.0
        };

        // Generate random offsets based on position and seed
        let pos_hash = (context.position.x * 1000.0) as u32
            ^ (context.position.y * 2000.0) as u32
            ^ (context.position.z * 3000.0) as u32;

        let u_offset = hash(seed.wrapping_add(pos_hash)) * u_variance;
        let v_offset = hash(seed.wrapping_add(pos_hash).wrapping_add(1)) * v_variance;

        Vec2::new(context.uv.x + u_offset, context.uv.y + v_offset)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::Random
    }
}

/// Edge Mapper - highlights polygon edges
///
/// Generates UVs based on proximity to polygon edges
///
/// C++ Reference: gamemtl.h line 91
pub struct EdgeMapper;

impl TextureMapper for EdgeMapper {
    fn map_texture(&self, context: &TextureMappingContext, _params: &TextureMapperParams) -> Vec2 {
        // Distance to nearest edge in UV space
        let u_edge = (context.uv.x - 0.5).abs() * 2.0;
        let v_edge = (context.uv.y - 0.5).abs() * 2.0;
        let edge_dist = u_edge.max(v_edge);

        Vec2::new(edge_dist, edge_dist)
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::Edge
    }
}

/// Bump Environment Mapper - environment mapping with bump offset
///
/// Applies environment mapping with perturbation from bump map
///
/// C++ Reference: gamemtl.h line 92
pub struct BumpEnvironmentMapper;

impl TextureMapper for BumpEnvironmentMapper {
    fn map_texture(&self, context: &TextureMappingContext, params: &TextureMapperParams) -> Vec2 {
        // Start with standard environment mapping
        let env_mapper = EnvironmentMapper;
        let mut env_uv = env_mapper.map_texture(context, params);

        // Apply bump offset (would need bump map sample in real implementation)
        let bump_strength = params.float_args[0];

        // For now, use UV coordinates as bump approximation
        env_uv.x += (context.uv.x - 0.5) * bump_strength * 0.1;
        env_uv.y += (context.uv.y - 0.5) * bump_strength * 0.1;

        env_uv
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::BumpEnvironment
    }
}

// ============================================================================
// MAPPER FACTORY - Creates appropriate mapper for type
// ============================================================================

/// Factory for creating texture mappers
pub struct TextureMapperFactory;

impl TextureMapperFactory {
    /// Create a mapper instance for the given type
    pub fn create_mapper(mapper_type: TextureMapperType) -> Box<dyn TextureMapper> {
        match mapper_type {
            TextureMapperType::UV => Box::new(UVMapper),
            TextureMapperType::LinearOffset => Box::new(LinearOffsetMapper),
            TextureMapperType::Grid => Box::new(GridMapper),
            TextureMapperType::Rotate => Box::new(RotateMapper),
            TextureMapperType::Scale => Box::new(ScaleMapper),
            TextureMapperType::Environment => Box::new(EnvironmentMapper),
            TextureMapperType::CheapEnvironment => Box::new(CheapEnvironmentMapper),
            TextureMapperType::WSClassicEnvironment => Box::new(WSClassicEnvironmentMapper),
            TextureMapperType::WSEnvironment => Box::new(WSEnvironmentMapper),
            TextureMapperType::Screen => Box::new(ScreenMapper),
            TextureMapperType::SineLinearOffset => Box::new(SineLinearOffsetMapper),
            TextureMapperType::StepLinearOffset => Box::new(StepLinearOffsetMapper),
            TextureMapperType::ZigZagLinearOffset => Box::new(ZigZagLinearOffsetMapper),
            TextureMapperType::GridClassicEnvironment => Box::new(GridClassicEnvironmentMapper),
            TextureMapperType::GridEnvironment => Box::new(GridEnvironmentMapper),
            TextureMapperType::Silhouette => Box::new(SilhouetteMapper),
            TextureMapperType::Random => Box::new(RandomMapper),
            TextureMapperType::Edge => Box::new(EdgeMapper),
            TextureMapperType::BumpEnvironment => Box::new(BumpEnvironmentMapper),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uv_mapper() {
        let mapper = UVMapper;
        let context = TextureMappingContext {
            uv: Vec2::new(0.5, 0.7),
            ..Default::default()
        };
        let params = TextureMapperParams::default();

        let result = mapper.map_texture(&context, &params);
        assert_eq!(result, Vec2::new(0.5, 0.7));
    }

    #[test]
    fn test_linear_offset_mapper() {
        let mapper = LinearOffsetMapper;
        let mut context = TextureMappingContext {
            uv: Vec2::new(0.0, 0.0),
            time: 1.0,
            ..Default::default()
        };
        let params = TextureMapperParams {
            args: [1000, 2000, 0, 0], // 1.0 u/s, 2.0 v/s
            ..Default::default()
        };

        let result = mapper.map_texture(&context, &params);
        assert!((result.x - 1.0).abs() < 0.001);
        assert!((result.y - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_grid_mapper() {
        let mapper = GridMapper;
        let context = TextureMappingContext {
            uv: Vec2::new(0.5, 0.5),
            ..Default::default()
        };
        let params = TextureMapperParams {
            args: [2, 3, 0, 0], // 2x3 grid
            ..Default::default()
        };

        let result = mapper.map_texture(&context, &params);
        assert_eq!(result, Vec2::new(1.0, 1.5));
    }

    #[test]
    fn test_scale_mapper() {
        let mapper = ScaleMapper;
        let context = TextureMappingContext {
            uv: Vec2::new(0.5, 0.5),
            ..Default::default()
        };
        let params = TextureMapperParams {
            args: [2000, 500, 0, 0], // 2.0x, 0.5x
            ..Default::default()
        };

        let result = mapper.map_texture(&context, &params);
        assert_eq!(result, Vec2::new(1.0, 0.25));
    }

    #[test]
    fn test_mapper_type_conversion() {
        assert_eq!(TextureMapperType::from_u32(0), Some(TextureMapperType::UV));
        assert_eq!(
            TextureMapperType::from_u32(4),
            Some(TextureMapperType::LinearOffset)
        );
        assert_eq!(
            TextureMapperType::from_u32(7),
            Some(TextureMapperType::Grid)
        );
        assert_eq!(TextureMapperType::from_u32(99), None);
    }

    #[test]
    fn test_mapper_properties() {
        assert!(TextureMapperType::LinearOffset.is_animated());
        assert!(!TextureMapperType::UV.is_animated());
        assert!(TextureMapperType::Environment.requires_transforms());
        assert!(!TextureMapperType::Grid.requires_transforms());
    }
}
