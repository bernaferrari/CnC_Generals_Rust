//! Shader Interface Trait
//!
//! This module defines the interface for actual shader instances that perform rendering.
//! It's the Rust equivalent of the C++ ShdInterfaceClass.

use crate::error::ShdResult;
use std::any::Any;

/// Maximum number of rendering passes allowed for any shader implementation
pub const MAX_PASSES: u32 = 4;

/// Shader Interface Trait
///
/// This trait defines the interface for all shader instances. A derived shader's job
/// is to set up the graphics API render states for a particular rendering operation.
/// Instances of shaders are created by an associated ShdDefClass.
pub trait ShdInterface: Send + Sync + std::fmt::Debug + Any {
    /// Get the runtime type identification class ID
    fn get_class_id(&self) -> u32;

    /// Get the number of rendering passes this shader requires
    fn get_pass_count(&self) -> u32;

    /// Check if this shader produces opaque geometry
    ///
    /// This property is used to determine whether geometric shadows should be cast
    /// from an object. Alpha-test shaders should return false.
    fn is_opaque(&self) -> bool {
        true
    }

    /// Apply shared render states for the specified pass
    ///
    /// This sets up render states that are shared across all instances of this shader,
    /// such as textures, samplers, and shader programs.
    fn apply_shared(&mut self, pass: u32, render_info: &RenderInfo) -> ShdResult<()> {
        let _ = (pass, render_info);
        Ok(())
    }

    /// Apply per-instance render states for the specified pass
    ///
    /// This sets up render states that are specific to the current object being rendered,
    /// such as transformation matrices and material constants.
    fn apply_instance(&mut self, pass: u32, render_info: &RenderInfo) -> ShdResult<()> {
        let _ = (pass, render_info);
        Ok(())
    }

    /// Compare shaders for sorting purposes
    ///
    /// For rendering efficiency, shaders should implement comparison that the renderer
    /// can use to sort meshes to minimize state changes.
    fn compare_for_sorting(&self, other: &dyn ShdInterface, _pass: u32) -> std::cmp::Ordering {
        // Default implementation compares class IDs
        self.get_class_id().cmp(&other.get_class_id())
    }

    /// Check if this shader is similar enough to another to batch together
    fn is_similar_enough(&self, other: &dyn ShdInterface, _pass: u32) -> bool {
        self.get_class_id() == other.get_class_id()
    }

    /// Get the number of vertex streams required by this shader
    fn get_vertex_stream_count(&self) -> u32 {
        1 // Most shaders use a single vertex stream
    }

    /// Get the size (in bytes) of a vertex in the specified stream
    fn get_vertex_size(&self, _stream: u32) -> u32 {
        // Default to common vertex size (position + normal + UV)
        32 // 3 floats (position) + 3 floats (normal) + 2 floats (UV) = 32 bytes
    }

    /// Check if this shader should use hardware vertex processing
    fn use_hardware_vertex_processing(&self) -> bool {
        true // Modern shaders typically use hardware vertex processing
    }

    /// Get the number of textures used by this shader
    fn get_texture_count(&self) -> u32 {
        1 // Default to single texture
    }

    /// Perform any per-frame setup required by this shader
    fn setup_frame(&mut self) -> ShdResult<()> {
        Ok(()) // Default implementation does nothing
    }

    /// Perform any cleanup required by this shader
    fn cleanup(&mut self) -> ShdResult<()> {
        Ok(()) // Default implementation does nothing
    }
}

/// Vertex stream structure for passing vertex data to shaders
///
/// This structure contains pointers to different vertex attributes that may be
/// needed by shaders. Attributes that are not available will be None, and the
/// shader should validate that it receives everything it needs.
#[derive(Debug)]
pub struct VertexStreams {
    /// Vertex positions (always required)
    pub positions: Option<Vec<glam::Vec3>>,

    /// Vertex normals (required for lighting)
    pub normals: Option<Vec<glam::Vec3>>,

    /// UV coordinates for different texture stages
    pub uv_coords: [Option<Vec<glam::Vec2>>; 8], // Support up to 8 UV channels

    /// Vertex colors (32-bit integer format)
    pub colors_int: Option<Vec<u32>>,

    /// Vertex colors (floating point format)
    pub colors_float: Option<Vec<glam::Vec4>>,

    /// Tangent vectors (for bump mapping)
    pub tangents: Option<Vec<glam::Vec3>>,

    /// Binormal vectors (for bump mapping)
    pub binormals: Option<Vec<glam::Vec3>>,

    /// Combined tangent-space cross product (S x T)
    pub tangent_cross: Option<Vec<glam::Vec3>>,
}

impl Default for VertexStreams {
    fn default() -> Self {
        Self {
            positions: None,
            normals: None,
            uv_coords: Default::default(),
            colors_int: None,
            colors_float: None,
            tangents: None,
            binormals: None,
            tangent_cross: None,
        }
    }
}

impl VertexStreams {
    /// Create a new empty vertex streams structure
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if positions are available
    pub fn has_positions(&self) -> bool {
        self.positions.is_some()
    }

    /// Check if normals are available
    pub fn has_normals(&self) -> bool {
        self.normals.is_some()
    }

    /// Check if UV coordinates for a specific channel are available
    pub fn has_uv_coords(&self, channel: usize) -> bool {
        channel < 8 && self.uv_coords[channel].is_some()
    }

    /// Check if vertex colors are available
    pub fn has_vertex_colors(&self) -> bool {
        self.colors_int.is_some() || self.colors_float.is_some()
    }

    /// Check if tangent space vectors are available
    pub fn has_tangent_space(&self) -> bool {
        self.tangents.is_some() && self.binormals.is_some()
    }

    /// Get the vertex count (assumes all streams have the same length)
    pub fn get_vertex_count(&self) -> usize {
        if let Some(positions) = &self.positions {
            positions.len()
        } else {
            0
        }
    }
}

/// Render information structure passed to shaders
#[derive(Debug, Clone)]
pub struct RenderInfo {
    /// World transformation matrix
    pub world_matrix: glam::Mat4,

    /// View transformation matrix
    pub view_matrix: glam::Mat4,

    /// Projection transformation matrix
    pub projection_matrix: glam::Mat4,

    /// Combined world-view-projection matrix
    pub world_view_proj_matrix: glam::Mat4,

    /// Camera position in world space
    pub camera_position: glam::Vec3,

    /// Primary light direction
    pub light_direction: glam::Vec3,

    /// Primary light color
    pub light_color: glam::Vec3,

    /// Ambient light color
    pub ambient_color: glam::Vec3,

    /// Current time (for animated effects)
    pub time: f32,

    /// Frame delta time
    pub delta_time: f32,
}

impl Default for RenderInfo {
    fn default() -> Self {
        Self {
            world_matrix: glam::Mat4::IDENTITY,
            view_matrix: glam::Mat4::IDENTITY,
            projection_matrix: glam::Mat4::IDENTITY,
            world_view_proj_matrix: glam::Mat4::IDENTITY,
            camera_position: glam::Vec3::ZERO,
            light_direction: glam::Vec3::NEG_Z,
            light_color: glam::Vec3::ONE,
            ambient_color: glam::Vec3::splat(0.1),
            time: 0.0,
            delta_time: 0.016, // ~60 FPS
        }
    }
}

impl RenderInfo {
    /// Create a new render info structure
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the combined world-view-projection matrix
    pub fn update_combined_matrix(&mut self) {
        self.world_view_proj_matrix = self.projection_matrix * self.view_matrix * self.world_matrix;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ShdError;

    // Mock shader interface for testing
    #[derive(Debug)]
    struct MockShader {
        class_id: u32,
        pass_count: u32,
    }

    impl ShdInterface for MockShader {
        fn get_class_id(&self) -> u32 {
            self.class_id
        }

        fn get_pass_count(&self) -> u32 {
            self.pass_count
        }

        fn apply_shared(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
            Ok(())
        }

        fn apply_instance(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_shader_interface_basic_functionality() {
        let shader = MockShader {
            class_id: 456,
            pass_count: 2,
        };

        assert_eq!(shader.get_class_id(), 456);
        assert_eq!(shader.get_pass_count(), 2);
        assert!(shader.is_opaque());
        assert_eq!(shader.get_vertex_stream_count(), 1);
        assert_eq!(shader.get_vertex_size(0), 32);
        assert!(shader.use_hardware_vertex_processing());
        assert_eq!(shader.get_texture_count(), 1);
    }

    #[test]
    fn test_shader_comparison() {
        let shader1 = MockShader {
            class_id: 100,
            pass_count: 1,
        };
        let shader2 = MockShader {
            class_id: 200,
            pass_count: 1,
        };
        let shader3 = MockShader {
            class_id: 100,
            pass_count: 2,
        };

        assert_eq!(
            shader1.compare_for_sorting(&shader2, 0),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            shader2.compare_for_sorting(&shader1, 0),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            shader1.compare_for_sorting(&shader3, 0),
            std::cmp::Ordering::Equal
        );

        assert!(shader1.is_similar_enough(&shader3, 0));
        assert!(!shader1.is_similar_enough(&shader2, 0));
    }

    #[test]
    fn test_vertex_streams() {
        let mut streams = VertexStreams::new();

        assert!(!streams.has_positions());
        assert!(!streams.has_normals());
        assert!(!streams.has_uv_coords(0));
        assert!(!streams.has_vertex_colors());
        assert!(!streams.has_tangent_space());
        assert_eq!(streams.get_vertex_count(), 0);

        streams.positions = Some(vec![glam::Vec3::ZERO, glam::Vec3::ONE]);
        assert!(streams.has_positions());
        assert_eq!(streams.get_vertex_count(), 2);

        streams.uv_coords[0] = Some(vec![glam::Vec2::ZERO, glam::Vec2::ONE]);
        assert!(streams.has_uv_coords(0));
        assert!(!streams.has_uv_coords(1));

        streams.colors_int = Some(vec![0xFFFFFFFF, 0xFF000000]);
        assert!(streams.has_vertex_colors());
    }

    #[test]
    fn test_render_info() {
        let mut info = RenderInfo::new();

        assert_eq!(info.world_matrix, glam::Mat4::IDENTITY);
        assert_eq!(info.view_matrix, glam::Mat4::IDENTITY);
        assert_eq!(info.projection_matrix, glam::Mat4::IDENTITY);
        assert_eq!(info.camera_position, glam::Vec3::ZERO);
        assert_eq!(info.light_direction, glam::Vec3::NEG_Z);

        // Test matrix update
        info.world_matrix = glam::Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0));
        info.update_combined_matrix();

        // The combined matrix should include the translation
        assert_ne!(info.world_view_proj_matrix, glam::Mat4::IDENTITY);
    }
}
