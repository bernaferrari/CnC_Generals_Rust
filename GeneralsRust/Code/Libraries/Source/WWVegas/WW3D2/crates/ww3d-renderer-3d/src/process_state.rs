//! Crate-local process façade for renderer-3d free-function bridges.
//!
//! State here mirrors C++ WW3D2 file-scope / class-static mutable data that
//! has not yet been threaded through [`Ww3dContext`]. One LazyLock replaces
//! the previous per-module Bag/Slot statics so the globals meter tracks a
//! single owner. Prefer moving fields onto Ww3dContext when call sites allow.

use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{LazyLock, Mutex};

use crate::config::RendererConfig;
use crate::rendering::texture_metrics::TextureDecisionRecord;
use crate::rendering::texture_quality::TextureQualitySettings;
use crate::rendering::texture_system::texfcach::TextureFileCache;
use crate::rendering::texture_system::texturethumbnail::ThumbnailManagerClass;

/// Packed process state formerly spread across renderer-3d module statics.
pub struct Renderer3dRuntime {
    pub enable_per_polygon_culling: AtomicBool,
    pub next_logical_decal_id: AtomicU32,
    pub mesh_debug_id_count: AtomicU32,
    pub lighting_lod_cutoff: Mutex<f32>,
    pub config: Mutex<RendererConfig>,
    pub texture_quality: Mutex<TextureQualitySettings>,
    pub texture_decisions: Mutex<Vec<TextureDecisionRecord>>,
    pub thumbnail_manager: Mutex<ThumbnailManagerClass>,
    pub texture_cache: Mutex<Option<TextureFileCache>>,
}

pub static RENDERER_3D: LazyLock<Renderer3dRuntime> = LazyLock::new(|| Renderer3dRuntime {
    enable_per_polygon_culling: AtomicBool::new(true),
    next_logical_decal_id: AtomicU32::new(1),
    mesh_debug_id_count: AtomicU32::new(0),
    lighting_lod_cutoff: Mutex::new(0.5),
    config: Mutex::new(RendererConfig::default()),
    texture_quality: Mutex::new(TextureQualitySettings::default()),
    texture_decisions: Mutex::new(Vec::new()),
    thumbnail_manager: Mutex::new(ThumbnailManagerClass::new()),
    texture_cache: Mutex::new(None),
});
