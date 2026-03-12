//! Shader Class IDs
//!
//! This module defines the class IDs used for runtime type identification
//! of different shader types. These correspond to the original C++ enum values.

/// Class IDs for Shader Definitions
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShdDefClassId {
    /// Dummy/placeholder shader
    Dummy = 0,
    /// Simple texture shader
    Simple = 1,
    /// Gloss mask shader
    GlossMask = 2,
    /// Bump mapping with specular highlights
    BumpSpec = 3,
    /// Bump mapping with diffuse lighting only
    BumpDiff = 4,
    /// Cube map reflection shader
    CubeMap = 5,
    /// Legacy W3D compatibility shader
    LegacyW3D = 6,
}

/// Class IDs for actual Shader Implementations
///
/// These are used for specific hardware implementations of shaders.
/// Typically there will be several for each "type", one for each
/// hardware configuration.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShdClassId {
    /// Dummy/placeholder shader implementation
    Dummy = 0,
}

// Constants for backward compatibility with the original C++ code
pub const SHDDEF_CLASSID_DUMMY: u32 = ShdDefClassId::Dummy as u32;
pub const SHDDEF_CLASSID_SIMPLE: u32 = ShdDefClassId::Simple as u32;
pub const SHDDEF_CLASSID_GLOSSMASK: u32 = ShdDefClassId::GlossMask as u32;
pub const SHDDEF_CLASSID_BUMPSPEC: u32 = ShdDefClassId::BumpSpec as u32;
pub const SHDDEF_CLASSID_BUMPDIFF: u32 = ShdDefClassId::BumpDiff as u32;
pub const SHDDEF_CLASSID_CUBEMAP: u32 = ShdDefClassId::CubeMap as u32;
pub const SHDDEF_CLASSID_LEGACYW3D: u32 = ShdDefClassId::LegacyW3D as u32;

pub const SHD_CLASSID_DUMMY: u32 = ShdClassId::Dummy as u32;

impl ShdDefClassId {
    /// Get the display name for this shader definition class
    pub fn display_name(self) -> &'static str {
        match self {
            ShdDefClassId::Dummy => "Dummy",
            ShdDefClassId::Simple => "Simple",
            ShdDefClassId::GlossMask => "Gloss Mask",
            ShdDefClassId::BumpSpec => "Bump Specular",
            ShdDefClassId::BumpDiff => "Bump Diffuse",
            ShdDefClassId::CubeMap => "Cube Map",
            ShdDefClassId::LegacyW3D => "Legacy W3D",
        }
    }

    /// Convert from a u32 class ID to the enum variant
    pub fn from_u32(id: u32) -> Option<Self> {
        match id {
            0 => Some(ShdDefClassId::Dummy),
            1 => Some(ShdDefClassId::Simple),
            2 => Some(ShdDefClassId::GlossMask),
            3 => Some(ShdDefClassId::BumpSpec),
            4 => Some(ShdDefClassId::BumpDiff),
            5 => Some(ShdDefClassId::CubeMap),
            6 => Some(ShdDefClassId::LegacyW3D),
            _ => None,
        }
    }
}

impl TryFrom<u32> for ShdDefClassId {
    type Error = crate::ShdError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::from_u32(value).ok_or_else(|| {
            crate::ShdError::InvalidConfig(format!("Unknown shader class ID: {}", value))
        })
    }
}

impl From<ShdDefClassId> for u32 {
    fn from(id: ShdDefClassId) -> u32 {
        id as u32
    }
}

impl ShdDefClassId {
    /// Get all valid shader definition class IDs
    pub fn all() -> &'static [ShdDefClassId] {
        &[
            ShdDefClassId::Dummy,
            ShdDefClassId::Simple,
            ShdDefClassId::GlossMask,
            ShdDefClassId::BumpSpec,
            ShdDefClassId::BumpDiff,
            ShdDefClassId::CubeMap,
            ShdDefClassId::LegacyW3D,
        ]
    }
}

impl ShdClassId {
    /// Get the display name for this shader implementation class
    pub fn display_name(self) -> &'static str {
        match self {
            ShdClassId::Dummy => "Dummy Implementation",
        }
    }

    /// Convert from a u32 class ID to the enum variant
    pub fn from_u32(id: u32) -> Option<Self> {
        match id {
            0 => Some(ShdClassId::Dummy),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_def_class_id_conversion() {
        assert_eq!(ShdDefClassId::BumpDiff as u32, 4);
        assert_eq!(ShdDefClassId::BumpSpec as u32, 3);

        assert_eq!(ShdDefClassId::from_u32(4), Some(ShdDefClassId::BumpDiff));
        assert_eq!(ShdDefClassId::from_u32(3), Some(ShdDefClassId::BumpSpec));
        assert_eq!(ShdDefClassId::from_u32(999), None);
    }

    #[test]
    fn test_shader_def_class_id_display_names() {
        assert_eq!(ShdDefClassId::BumpDiff.display_name(), "Bump Diffuse");
        assert_eq!(ShdDefClassId::BumpSpec.display_name(), "Bump Specular");
    }

    #[test]
    fn test_constants_compatibility() {
        assert_eq!(SHDDEF_CLASSID_BUMPDIFF, 4);
        assert_eq!(SHDDEF_CLASSID_BUMPSPEC, 3);
        assert_eq!(SHDDEF_CLASSID_SIMPLE, 1);
    }

    #[test]
    fn test_all_shader_def_class_ids() {
        let all_ids = ShdDefClassId::all();
        assert_eq!(all_ids.len(), 7);
        assert!(all_ids.contains(&ShdDefClassId::BumpDiff));
        assert!(all_ids.contains(&ShdDefClassId::BumpSpec));
    }
}
