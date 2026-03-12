//! Shader Definition Trait
//!
//! This module defines the core shader definition trait that all shader types must implement.
//! It's the Rust equivalent of the C++ ShdDefClass.

use crate::error::ShdResult;
use crate::interface::ShdInterface;
use std::any::Any;

/// Shader Definition Trait
///
/// This trait represents the interface for all shader "definition" objects.
/// A shader definition object has two main responsibilities:
///
/// 1. It contains a generic description of all user-settable parameters used by
///    an instance of this type of shader (textures, colors, etc.)
///
/// 2. It contains a factory method which can create an actual shader instance
///    compatible with the current hardware the application is running on.
pub trait ShdDefClass: Send + Sync {
    /// Get the runtime type identification class ID
    fn get_class_id(&self) -> u32;

    /// Get the shader name
    fn get_name(&self) -> &str;

    /// Set the shader name
    fn set_name(&mut self, name: String);

    /// Get the surface type (used for decal, sound, and emitter creation)
    fn get_surface_type(&self) -> i32;

    /// Set the surface type
    fn set_surface_type(&mut self, surface_type: i32);

    /// Clone this shader definition
    fn clone_def(&self) -> Box<dyn ShdDefClass>;

    /// Create a shader instance compatible with the current hardware/API
    fn create_shader(&self) -> ShdResult<Box<dyn ShdInterface>>;

    /// Validate the current shader configuration
    fn is_valid_config(&self) -> ShdResult<()>;

    // Requirements - used to determine what vertex data the shader needs

    /// Check if this shader uses vertex alpha
    fn uses_vertex_alpha(&self) -> bool {
        false
    }

    /// Check if this shader uses a specific UV channel
    fn uses_uv_channel(&self, channel: u32) -> bool {
        channel == 0 // By default, only use UV channel 0
    }

    /// Check if this shader uses vertex colors
    fn uses_vertex_colors(&self) -> bool {
        false
    }

    /// Check if this shader requires normals
    fn requires_normals(&self) -> bool {
        false
    }

    /// Check if this shader requires tangent space vectors (for bump mapping)
    fn requires_tangent_space_vectors(&self) -> bool {
        false
    }

    /// Check if this shader requires sorting (for transparency)
    fn requires_sorting(&self) -> bool {
        false
    }

    /// Get the static sort index (for render order)
    fn static_sort_index(&self) -> i32 {
        0
    }

    // Serialization methods

    /// Save shader definition to binary format
    fn save(&self) -> ShdResult<Vec<u8>>;

    /// Load shader definition from binary format
    fn load(&mut self, data: &[u8]) -> ShdResult<()>;

    /// Downcast helper for backend-specific handling
    fn as_any(&self) -> &dyn Any;

    /// Mutable downcast helper for backend-specific handling
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

// Helper trait for shader definition factories
pub trait ShdDefFactory: Send + Sync + std::fmt::Debug {
    /// Create a new shader definition instance
    fn create_definition(&self, class_id: u32) -> ShdResult<Box<dyn ShdDefClass>>;

    /// Get the display name for this shader type
    fn get_display_name(&self) -> &str;

    /// Get the class ID for this shader type
    fn get_class_id(&self) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ShdError;

    // Mock shader definition for testing
    #[derive(Debug, Clone)]
    struct MockShaderDef {
        name: String,
        class_id: u32,
        surface_type: i32,
    }

    impl ShdDefClass for MockShaderDef {
        fn get_class_id(&self) -> u32 {
            self.class_id
        }

        fn get_name(&self) -> &str {
            &self.name
        }

        fn set_name(&mut self, name: String) {
            self.name = name;
        }

        fn get_surface_type(&self) -> i32 {
            self.surface_type
        }

        fn set_surface_type(&mut self, surface_type: i32) {
            self.surface_type = surface_type;
        }

        fn clone_def(&self) -> Box<dyn ShdDefClass> {
            Box::new(self.clone())
        }

        fn create_shader(&self) -> ShdResult<Box<dyn ShdInterface>> {
            Err(ShdError::InvalidConfig(
                "Mock shader cannot create instances".to_string(),
            ))
        }

        fn is_valid_config(&self) -> ShdResult<()> {
            Ok(())
        }

        fn save(&self) -> ShdResult<Vec<u8>> {
            Ok(vec![1, 2, 3, 4]) // Mock data
        }

        fn load(&mut self, _data: &[u8]) -> ShdResult<()> {
            Ok(())
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn test_shader_def_basic_functionality() {
        let mut shader_def = MockShaderDef {
            name: "TestShader".to_string(),
            class_id: 123,
            surface_type: 0,
        };

        assert_eq!(shader_def.get_class_id(), 123);
        assert_eq!(shader_def.get_name(), "TestShader");
        assert_eq!(shader_def.get_surface_type(), 0);

        shader_def.set_name("NewName".to_string());
        assert_eq!(shader_def.get_name(), "NewName");

        shader_def.set_surface_type(5);
        assert_eq!(shader_def.get_surface_type(), 5);
    }

    #[test]
    fn test_shader_def_default_requirements() {
        let shader_def = MockShaderDef {
            name: "TestShader".to_string(),
            class_id: 123,
            surface_type: 0,
        };

        assert!(!shader_def.uses_vertex_alpha());
        assert!(!shader_def.uses_vertex_colors());
        assert!(!shader_def.requires_normals());
        assert!(!shader_def.requires_tangent_space_vectors());
        assert!(!shader_def.requires_sorting());
        assert_eq!(shader_def.static_sort_index(), 0);

        assert!(shader_def.uses_uv_channel(0));
        assert!(!shader_def.uses_uv_channel(1));
    }

    #[test]
    fn test_shader_def_clone() {
        let original = MockShaderDef {
            name: "Original".to_string(),
            class_id: 456,
            surface_type: 1,
        };

        let cloned = original.clone_def();
        assert_eq!(cloned.get_name(), "Original");
        assert_eq!(cloned.get_class_id(), 456);
        assert_eq!(cloned.get_surface_type(), 1);
    }

    #[test]
    fn test_shader_def_serialization() {
        let shader_def = MockShaderDef {
            name: "SerializationTest".to_string(),
            class_id: 789,
            surface_type: 2,
        };

        let data = shader_def.save().unwrap();
        assert_eq!(data, vec![1, 2, 3, 4]);

        let mut loaded_shader = MockShaderDef {
            name: String::new(),
            class_id: 0,
            surface_type: 0,
        };

        assert!(loaded_shader.load(&data).is_ok());
    }
}
