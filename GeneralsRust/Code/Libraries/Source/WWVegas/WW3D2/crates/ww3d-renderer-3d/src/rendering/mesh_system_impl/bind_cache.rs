//! Bind-group and static-buffer reuse for the W3D mesh path (renderer plan R5).
//!
//! Every mesh pipeline still owns private bind-group layouts, so a bind group
//! can only be reused with the pipeline that created it. Keys therefore carry
//! the pipeline identity, and each entry keeps the pipeline and every bound
//! resource alive: an address in a key can never be recycled for a different
//! object while the entry exists.
//!
//! * Persistent entries (texture groups, vertex colors) survive across frames
//!   and are evicted after [`IDLE_FRAMES`] frames without a hit.
//! * Per-pass entries (camera, UV transform) point into the frame uniform
//!   arena and are dropped by [`MeshBindCache::begin_pass`].

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

/// Frames a persistent entry may stay unused before it is evicted.
const IDLE_FRAMES: u64 = 120;

/// Address identity of an `Arc`-held GPU object.
pub(super) fn addr<T>(arc: &Arc<T>) -> usize {
    Arc::as_ptr(arc).cast::<()>().addr()
}

/// 64-bit content hash of plain bytes.
pub(super) fn content_hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// Key of one texture-stage bind group: pipeline, group slot, and the exact
/// view/sampler objects bound in it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextureGroupKey {
    pub pipeline: usize,
    pub group: u32,
    pub resources: Vec<usize>,
}

/// Key of a vertex-color bind group: pipeline, slot, and the color contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct VertexColorKey {
    pub pipeline: usize,
    pub group: u32,
    pub diffuse: Option<(u64, usize)>,
    pub illumination: Option<(u64, usize)>,
}

/// Key of a per-pass uniform bind group (camera / UV transform).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct UniformGroupKey {
    pub pipeline: usize,
    pub group: u32,
    pub contents: u64,
}

/// Sampler state key: every `wgpu::SamplerDescriptor` field except the label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct SamplerKey {
    address: [wgpu::AddressMode; 3],
    filters: [wgpu::FilterMode; 2],
    mipmap: wgpu::MipmapFilterMode,
    lod_clamp: [u32; 2],
    compare: Option<wgpu::CompareFunction>,
    anisotropy: u16,
    border: Option<wgpu::SamplerBorderColor>,
}

impl SamplerKey {
    pub(super) fn of(desc: &wgpu::SamplerDescriptor<'_>) -> Self {
        Self {
            address: [desc.address_mode_u, desc.address_mode_v, desc.address_mode_w],
            filters: [desc.mag_filter, desc.min_filter],
            mipmap: desc.mipmap_filter,
            lod_clamp: [desc.lod_min_clamp.to_bits(), desc.lod_max_clamp.to_bits()],
            compare: desc.compare,
            anisotropy: desc.anisotropy_clamp,
            border: desc.border_color,
        }
    }
}

struct Persistent<V> {
    value: V,
    last_used: u64,
    _pipeline: Arc<wgpu::RenderPipeline>,
    _views: Vec<Arc<wgpu::TextureView>>,
    _samplers: Vec<Arc<wgpu::Sampler>>,
}

/// A cached bind group plus the buffers it reads.
#[derive(Clone)]
pub(super) struct CachedGroup {
    pub bind_group: Arc<wgpu::BindGroup>,
    pub buffers: Vec<Arc<wgpu::Buffer>>,
}

/// Counters for one frame (hits/misses), surfaced through `MeshRenderStats`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MeshBindCacheStats {
    pub hits: u32,
    pub misses: u32,
}

/// Bind-group cache owned by one `MeshRenderManager`.
#[derive(Default)]
pub(super) struct MeshBindCache {
    frame: u64,
    textures: HashMap<TextureGroupKey, Persistent<Arc<wgpu::BindGroup>>>,
    vertex_colors: HashMap<VertexColorKey, Persistent<CachedGroup>>,
    color_buffers: HashMap<(u64, usize), (Arc<wgpu::Buffer>, u64)>,
    per_pass: HashMap<UniformGroupKey, CachedGroup>,
    /// One sampler per distinct sampler state (C++ sets sampler states on the
    /// device; nothing needs a per-draw object).
    samplers: HashMap<SamplerKey, Arc<wgpu::Sampler>>,
    /// Default views of textures that carry their own wgpu texture.
    texture_views: HashMap<wgpu::Texture, (Arc<wgpu::TextureView>, u64)>,
    pub stats: MeshBindCacheStats,
}

impl MeshBindCache {
    /// Start a new frame: age persistent entries and drop idle ones.
    pub(super) fn begin_frame(&mut self) {
        self.frame += 1;
        let frame = self.frame;
        let live = |last_used: u64| frame.saturating_sub(last_used) <= IDLE_FRAMES;
        self.textures.retain(|_, entry| live(entry.last_used));
        self.vertex_colors.retain(|_, entry| live(entry.last_used));
        self.color_buffers.retain(|_, (_, last_used)| live(*last_used));
        self.texture_views.retain(|_, (_, last_used)| live(*last_used));
        self.stats = MeshBindCacheStats::default();
    }

    /// Start a mesh render pass: arena-backed groups from an earlier pass are
    /// no longer valid.
    pub(super) fn begin_pass(&mut self) {
        self.per_pass.clear();
    }

    pub(super) fn texture_group(
        &mut self,
        key: TextureGroupKey,
        pipeline: &Arc<wgpu::RenderPipeline>,
        views: Vec<Arc<wgpu::TextureView>>,
        samplers: Vec<Arc<wgpu::Sampler>>,
        create: impl FnOnce() -> wgpu::BindGroup,
    ) -> Arc<wgpu::BindGroup> {
        let frame = self.frame;
        if let Some(entry) = self.textures.get_mut(&key) {
            entry.last_used = frame;
            self.stats.hits += 1;
            return Arc::clone(&entry.value);
        }
        self.stats.misses += 1;
        let value = Arc::new(create());
        self.textures.insert(
            key,
            Persistent {
                value: Arc::clone(&value),
                last_used: frame,
                _pipeline: Arc::clone(pipeline),
                _views: views,
                _samplers: samplers,
            },
        );
        value
    }

    /// The shared sampler for `desc`'s state.
    pub(super) fn sampler(
        &mut self,
        desc: &wgpu::SamplerDescriptor<'_>,
        create: impl FnOnce() -> wgpu::Sampler,
    ) -> Arc<wgpu::Sampler> {
        Arc::clone(
            self.samplers
                .entry(SamplerKey::of(desc))
                .or_insert_with(|| Arc::new(create())),
        )
    }

    /// The default view of `texture`, reusing the owner's view when available.
    pub(super) fn texture_view(&mut self, texture: &wgpu::Texture, existing: Option<&wgpu::TextureView>) -> Arc<wgpu::TextureView> {
        let frame = self.frame;
        let entry = self.texture_views.entry(texture.clone()).or_insert_with(|| {
            (
                Arc::new(existing.cloned().unwrap_or_else(|| texture.create_view(&wgpu::TextureViewDescriptor::default()))),
                frame,
            )
        });
        entry.1 = frame;
        Arc::clone(&entry.0)
    }

    /// Static vertex-color storage buffer for `bytes`, shared by content.
    pub(super) fn color_buffer(
        &mut self,
        bytes: &[u8],
        create: impl FnOnce() -> wgpu::Buffer,
    ) -> Arc<wgpu::Buffer> {
        let key = (content_hash(bytes), bytes.len());
        let frame = self.frame;
        let entry = self
            .color_buffers
            .entry(key)
            .or_insert_with(|| (Arc::new(create()), frame));
        entry.1 = frame;
        Arc::clone(&entry.0)
    }

    pub(super) fn vertex_color_group(
        &mut self,
        key: VertexColorKey,
        pipeline: &Arc<wgpu::RenderPipeline>,
        create: impl FnOnce(&mut Self) -> CachedGroup,
    ) -> CachedGroup {
        let frame = self.frame;
        if let Some(entry) = self.vertex_colors.get_mut(&key) {
            entry.last_used = frame;
            self.stats.hits += 1;
            return entry.value.clone();
        }
        self.stats.misses += 1;
        let value = create(self);
        self.vertex_colors.insert(
            key,
            Persistent {
                value: value.clone(),
                last_used: frame,
                _pipeline: Arc::clone(pipeline),
                _views: Vec::new(),
                _samplers: Vec::new(),
            },
        );
        value
    }

    /// Arena-backed group valid for the current mesh render pass only.
    pub(super) fn per_pass_group<E>(
        &mut self,
        key: UniformGroupKey,
        create: impl FnOnce() -> Result<CachedGroup, E>,
    ) -> Result<CachedGroup, E> {
        if let Some(group) = self.per_pass.get(&key) {
            self.stats.hits += 1;
            return Ok(group.clone());
        }
        self.stats.misses += 1;
        let group = create()?;
        self.per_pass.insert(key, group.clone());
        Ok(group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_separates_contents_and_matches_equal_bytes() {
        assert_eq!(content_hash(&[1, 2, 3]), content_hash(&[1, 2, 3]));
        assert_ne!(content_hash(&[1, 2, 3]), content_hash(&[1, 2, 4]));
    }
}
