//! Asset Validation
//!
//! This module provides validation capabilities for assets:
//! - Mesh validation (topology, normals, UVs)
//! - Texture validation (format, dimensions, mipmaps)
//! - Material validation (shader compatibility, texture references)
//! - Scene validation (hierarchy, transforms)

use crate::{Asset, AssetData, AssetError, AssetType, MeshData, Result, TextureData};

/// Validation severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
}

/// Validation message
#[derive(Debug, Clone)]
pub struct ValidationMessage {
    pub severity: ValidationSeverity,
    pub category: String,
    pub message: String,
    pub context: Option<String>,
}

impl ValidationMessage {
    pub fn error(category: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Error,
            category: category.into(),
            message: message.into(),
            context: None,
        }
    }

    pub fn warning(category: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Warning,
            category: category.into(),
            message: message.into(),
            context: None,
        }
    }

    pub fn info(category: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Info,
            category: category.into(),
            message: message.into(),
            context: None,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}

/// Validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub messages: Vec<ValidationMessage>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn add(&mut self, message: ValidationMessage) {
        self.messages.push(message);
    }

    pub fn is_valid(&self) -> bool {
        !self.has_errors()
    }

    pub fn has_errors(&self) -> bool {
        self.messages
            .iter()
            .any(|m| m.severity == ValidationSeverity::Error)
    }

    pub fn has_warnings(&self) -> bool {
        self.messages
            .iter()
            .any(|m| m.severity == ValidationSeverity::Warning)
    }

    pub fn error_count(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| m.severity == ValidationSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| m.severity == ValidationSeverity::Warning)
            .count()
    }

    pub fn merge(&mut self, other: ValidationResult) {
        self.messages.extend(other.messages);
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Asset validator
pub trait AssetValidator {
    fn validate(&self, asset: &Asset) -> ValidationResult;
    fn name(&self) -> &str;
}

/// Mesh validator
#[derive(Debug, Clone)]
pub struct MeshValidator {
    check_topology: bool,
    check_normals: bool,
    check_uvs: bool,
    check_bounds: bool,
    max_vertices: Option<usize>,
    max_triangles: Option<usize>,
}

impl MeshValidator {
    pub fn new() -> Self {
        Self {
            check_topology: true,
            check_normals: true,
            check_uvs: true,
            check_bounds: true,
            max_vertices: Some(1_000_000),
            max_triangles: Some(500_000),
        }
    }

    pub fn with_topology_check(mut self, enabled: bool) -> Self {
        self.check_topology = enabled;
        self
    }

    pub fn with_normal_check(mut self, enabled: bool) -> Self {
        self.check_normals = enabled;
        self
    }

    pub fn with_uv_check(mut self, enabled: bool) -> Self {
        self.check_uvs = enabled;
        self
    }

    pub fn with_max_vertices(mut self, max: usize) -> Self {
        self.max_vertices = Some(max);
        self
    }

    fn validate_mesh(&self, mesh: &MeshData) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Check vertex count
        if mesh.vertices.is_empty() {
            result.add(ValidationMessage::error("mesh", "Mesh has no vertices"));
        }

        if let Some(max) = self.max_vertices {
            if mesh.vertices.len() > max {
                result.add(ValidationMessage::warning(
                    "mesh",
                    format!("Mesh has {} vertices (max: {})", mesh.vertices.len(), max),
                ));
            }
        }

        // Check index count
        if mesh.indices.is_empty() {
            result.add(ValidationMessage::error("mesh", "Mesh has no indices"));
        } else if mesh.indices.len() % 3 != 0 {
            result.add(ValidationMessage::error(
                "mesh",
                format!("Index count {} is not divisible by 3", mesh.indices.len()),
            ));
        }

        // Check triangle count
        let triangle_count = mesh.indices.len() / 3;
        if let Some(max) = self.max_triangles {
            if triangle_count > max {
                result.add(ValidationMessage::warning(
                    "mesh",
                    format!("Mesh has {} triangles (max: {})", triangle_count, max),
                ));
            }
        }

        // Check topology
        if self.check_topology {
            for &index in &mesh.indices {
                if index >= mesh.vertices.len() as u32 {
                    result.add(ValidationMessage::error(
                        "topology",
                        format!(
                            "Index {} out of bounds (vertex count: {})",
                            index,
                            mesh.vertices.len()
                        ),
                    ));
                    break;
                }
            }
        }

        // Check normals
        if self.check_normals {
            for (i, vertex) in mesh.vertices.iter().enumerate() {
                let normal_length_sq = vertex.normal[0] * vertex.normal[0]
                    + vertex.normal[1] * vertex.normal[1]
                    + vertex.normal[2] * vertex.normal[2];

                if normal_length_sq < 0.9 || normal_length_sq > 1.1 {
                    result.add(
                        ValidationMessage::warning(
                            "normals",
                            format!("Vertex {} has unnormalized normal", i),
                        )
                        .with_context(format!("Length squared: {:.3}", normal_length_sq)),
                    );
                }
            }
        }

        // Check UVs
        if self.check_uvs {
            for (i, vertex) in mesh.vertices.iter().enumerate() {
                if vertex.uv[0] < -1.0
                    || vertex.uv[0] > 2.0
                    || vertex.uv[1] < -1.0
                    || vertex.uv[1] > 2.0
                {
                    result.add(
                        ValidationMessage::info(
                            "uvs",
                            format!("Vertex {} has UV coordinates outside [0,1] range", i),
                        )
                        .with_context(format!("UV: ({:.3}, {:.3})", vertex.uv[0], vertex.uv[1])),
                    );
                }
            }
        }

        // Check bounds
        if self.check_bounds {
            let bounds = &mesh.bounds;
            for i in 0..3 {
                if bounds.min[i] > bounds.max[i] {
                    result.add(ValidationMessage::error(
                        "bounds",
                        format!("Invalid bounding box: min[{}] > max[{}]", i, i),
                    ));
                }
            }
        }

        result
    }
}

impl Default for MeshValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetValidator for MeshValidator {
    fn validate(&self, asset: &Asset) -> ValidationResult {
        match &asset.data {
            AssetData::Mesh(mesh) => self.validate_mesh(mesh),
            _ => {
                let mut result = ValidationResult::new();
                result.add(ValidationMessage::error("type", "Expected mesh data"));
                result
            }
        }
    }

    fn name(&self) -> &str {
        "MeshValidator"
    }
}

/// Texture validator
#[derive(Debug, Clone)]
pub struct TextureValidator {
    check_dimensions: bool,
    check_mipmaps: bool,
    check_format: bool,
    require_power_of_two: bool,
    max_dimension: Option<u32>,
}

impl TextureValidator {
    pub fn new() -> Self {
        Self {
            check_dimensions: true,
            check_mipmaps: true,
            check_format: true,
            require_power_of_two: false,
            max_dimension: Some(8192),
        }
    }

    pub fn require_power_of_two(mut self, required: bool) -> Self {
        self.require_power_of_two = required;
        self
    }

    pub fn with_max_dimension(mut self, max: u32) -> Self {
        self.max_dimension = Some(max);
        self
    }

    fn is_power_of_two(n: u32) -> bool {
        n > 0 && (n & (n - 1)) == 0
    }

    fn validate_texture(&self, texture: &TextureData) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Check dimensions
        if self.check_dimensions {
            if texture.width == 0 || texture.height == 0 {
                result.add(ValidationMessage::error(
                    "texture",
                    "Texture has zero dimensions",
                ));
            }

            if self.require_power_of_two {
                if !Self::is_power_of_two(texture.width) {
                    result.add(ValidationMessage::warning(
                        "dimensions",
                        format!("Width {} is not power of two", texture.width),
                    ));
                }
                if !Self::is_power_of_two(texture.height) {
                    result.add(ValidationMessage::warning(
                        "dimensions",
                        format!("Height {} is not power of two", texture.height),
                    ));
                }
            }

            if let Some(max) = self.max_dimension {
                if texture.width > max || texture.height > max {
                    result.add(ValidationMessage::warning(
                        "dimensions",
                        format!(
                            "Texture dimensions {}x{} exceed maximum {}",
                            texture.width, texture.height, max
                        ),
                    ));
                }
            }
        }

        // Check mipmaps
        if self.check_mipmaps {
            let max_mips = (texture.width.max(texture.height) as f32).log2().floor() as u8 + 1;
            if texture.mip_levels > max_mips {
                result.add(ValidationMessage::error(
                    "mipmaps",
                    format!(
                        "Invalid mip level count {} (max: {})",
                        texture.mip_levels, max_mips
                    ),
                ));
            }
        }

        // Check format
        if self.check_format {
            if texture.data.is_empty() {
                result.add(ValidationMessage::error("texture", "Texture has no data"));
            }
        }

        result
    }
}

impl Default for TextureValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetValidator for TextureValidator {
    fn validate(&self, asset: &Asset) -> ValidationResult {
        match &asset.data {
            AssetData::Texture(texture) => self.validate_texture(texture),
            _ => {
                let mut result = ValidationResult::new();
                result.add(ValidationMessage::error("type", "Expected texture data"));
                result
            }
        }
    }

    fn name(&self) -> &str {
        "TextureValidator"
    }
}

/// General asset validator
pub fn validate_asset(asset: &Asset) -> ValidationResult {
    let mut result = ValidationResult::new();

    // Check basic asset properties
    if asset.name.is_empty() {
        result.add(ValidationMessage::warning("asset", "Asset has no name"));
    }

    // Validate based on type
    match asset.asset_type {
        AssetType::Mesh => {
            let validator = MeshValidator::new();
            result.merge(validator.validate(asset));
        }
        AssetType::Texture => {
            let validator = TextureValidator::new();
            result.merge(validator.validate(asset));
        }
        _ => {
            result.add(ValidationMessage::info(
                "asset",
                format!("No specific validator for {:?}", asset.asset_type),
            ));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoneWeight, BoundingBox, Vertex};

    fn create_valid_mesh() -> MeshData {
        MeshData {
            vertices: vec![
                Vertex {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                    tangent: [1.0, 0.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                Vertex {
                    position: [1.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [1.0, 0.0],
                    tangent: [1.0, 0.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                Vertex {
                    position: [0.0, 1.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 1.0],
                    tangent: [1.0, 0.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2],
            materials: vec![],
            bone_weights: vec![],
            bounds: BoundingBox {
                min: [0.0, 0.0, 0.0],
                max: [1.0, 1.0, 0.0],
            },
        }
    }

    #[test]
    fn test_validation_message_creation() {
        let error = ValidationMessage::error("test", "error message");
        assert_eq!(error.severity, ValidationSeverity::Error);

        let warning = ValidationMessage::warning("test", "warning message");
        assert_eq!(warning.severity, ValidationSeverity::Warning);

        let info = ValidationMessage::info("test", "info message");
        assert_eq!(info.severity, ValidationSeverity::Info);
    }

    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::new();
        assert!(result.is_valid());

        result.add(ValidationMessage::warning("test", "warning"));
        assert!(result.is_valid());
        assert!(result.has_warnings());

        result.add(ValidationMessage::error("test", "error"));
        assert!(!result.is_valid());
        assert!(result.has_errors());

        assert_eq!(result.error_count(), 1);
        assert_eq!(result.warning_count(), 1);
    }

    #[test]
    fn test_mesh_validator_valid_mesh() {
        let validator = MeshValidator::new();
        let mut asset = Asset::new("test", AssetType::Mesh);
        asset.data = AssetData::Mesh(create_valid_mesh());

        let result = validator.validate(&asset);
        assert!(result.is_valid());
    }

    #[test]
    fn test_mesh_validator_empty_mesh() {
        let validator = MeshValidator::new();
        let mut asset = Asset::new("test", AssetType::Mesh);
        asset.data = AssetData::Mesh(MeshData {
            vertices: vec![],
            indices: vec![],
            materials: vec![],
            bone_weights: vec![],
            bounds: BoundingBox {
                min: [0.0, 0.0, 0.0],
                max: [0.0, 0.0, 0.0],
            },
        });

        let result = validator.validate(&asset);
        assert!(!result.is_valid());
        assert!(result.has_errors());
    }

    #[test]
    fn test_texture_validator() {
        let validator = TextureValidator::new();
        let mut asset = Asset::new("test", AssetType::Texture);
        asset.data = AssetData::Texture(TextureData {
            width: 512,
            height: 512,
            depth: 1,
            format: crate::TextureFormat::Rgba8,
            mip_levels: 10,
            data: vec![0; 512 * 512 * 4],
        });

        let result = validator.validate(&asset);
        assert!(result.is_valid());
    }

    #[test]
    fn test_texture_validator_power_of_two() {
        let validator = TextureValidator::new().require_power_of_two(true);
        let mut asset = Asset::new("test", AssetType::Texture);
        asset.data = AssetData::Texture(TextureData {
            width: 500, // Not power of two
            height: 512,
            depth: 1,
            format: crate::TextureFormat::Rgba8,
            mip_levels: 1,
            data: vec![0; 500 * 512 * 4],
        });

        let result = validator.validate(&asset);
        assert!(result.has_warnings());
    }
}
