/*!
 * Atlas generation and metadata structures
 */

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Result of atlas generation process
#[derive(Debug, Clone)]
pub struct AtlasResult {
    /// Unique identifier for this atlas
    pub id: Uuid,
    /// Name of the group/atlas
    pub group_name: String,
    /// Path to the generated atlas image
    pub atlas_path: PathBuf,
    /// Path to metadata file (if generated)
    pub metadata_path: Option<PathBuf>,
    /// Number of sprites packed into this atlas
    pub sprite_count: usize,
    /// Final size of the atlas (width, height)
    pub atlas_size: (u32, u32),
    /// When this atlas was created
    pub created_at: DateTime<Utc>,
}

/// Metadata for an atlas, compatible with C++ format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasMetadata {
    /// Atlas name
    pub name: String,
    /// Total texture size
    pub texture_size: (u32, u32),
    /// Map of sprite name to sprite information
    pub sprites: HashMap<String, SpriteInfo>,
    /// Output format
    pub format: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Information about a sprite in the atlas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpriteInfo {
    /// X position in atlas
    pub x: u32,
    /// Y position in atlas
    pub y: u32,
    /// Width in atlas
    pub w: u32,
    /// Height in atlas
    pub h: u32,
    /// Whether the sprite was rotated
    pub rotated: bool,
    /// Whether the sprite was trimmed
    pub trimmed: bool,
    /// Original source size (width, height)
    pub source_size: (u32, u32),
    /// Sprite source size info (x, y, w, h)
    pub sprite_source_size: (u32, u32, u32, u32),
}

impl AtlasMetadata {
    /// Export metadata in C++ compatible JSON format
    pub fn export_cpp_format(&self) -> serde_json::Value {
        use serde_json::json;

        let mut frames = serde_json::Map::new();
        for (name, sprite) in &self.sprites {
            frames.insert(name.clone(), json!({
                "frame": {
                    "x": sprite.x,
                    "y": sprite.y,
                    "w": sprite.w,
                    "h": sprite.h
                },
                "rotated": sprite.rotated,
                "trimmed": sprite.trimmed,
                "spriteSourceSize": {
                    "x": sprite.sprite_source_size.0,
                    "y": sprite.sprite_source_size.1,
                    "w": sprite.sprite_source_size.2,
                    "h": sprite.sprite_source_size.3
                },
                "sourceSize": {
                    "w": sprite.source_size.0,
                    "h": sprite.source_size.1
                }
            }));
        }

        json!({
            "frames": frames,
            "meta": {
                "app": "ImagePacker (Rust)",
                "version": "1.0",
                "image": format!("{}.{}", self.name, self.format.to_lowercase()),
                "format": self.format,
                "size": {
                    "w": self.texture_size.0,
                    "h": self.texture_size.1
                },
                "scale": "1"
            }
        })
    }
}

/// Atlas generation statistics
#[derive(Debug, Clone)]
pub struct AtlasStats {
    /// Total number of atlases created
    pub atlas_count: usize,
    /// Total number of sprites processed
    pub sprite_count: usize,
    /// Total area of all atlases
    pub total_atlas_area: u64,
    /// Total area of all sprites
    pub total_sprite_area: u64,
    /// Packing efficiency (sprite area / atlas area)
    pub packing_efficiency: f32,
    /// Processing time in seconds
    pub processing_time: f32,
}

impl AtlasStats {
    pub fn calculate(results: &[AtlasResult]) -> Self {
        let atlas_count = results.len();
        let sprite_count: usize = results.iter().map(|r| r.sprite_count).sum();
        let total_atlas_area: u64 = results.iter()
            .map(|r| r.atlas_size.0 as u64 * r.atlas_size.1 as u64)
            .sum();

        // Note: We'd need to calculate sprite area from the actual sprite data
        // For now, we'll estimate based on atlas area and packing efficiency
        let estimated_sprite_area = (total_atlas_area as f32 * 0.75) as u64; // Assume 75% efficiency
        let packing_efficiency = if total_atlas_area > 0 {
            estimated_sprite_area as f32 / total_atlas_area as f32
        } else {
            0.0
        };

        Self {
            atlas_count,
            sprite_count,
            total_atlas_area,
            total_sprite_area: estimated_sprite_area,
            packing_efficiency,
            processing_time: 0.0, // Would be calculated during actual processing
        }
    }
}