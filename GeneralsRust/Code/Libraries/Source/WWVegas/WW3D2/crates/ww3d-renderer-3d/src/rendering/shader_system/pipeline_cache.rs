use std::collections::HashMap;
use std::sync::Arc;

use wgpu::{Device, SurfaceConfiguration, TextureFormat};

use super::shader::ShaderClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VertexLayoutKind {
    Rigid,
    Skinned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PipelineKey {
    shader_bits: u32,
    surface_format: TextureFormat,
    sample_count: u32,
    vertex_layout: VertexLayoutKind,
}

/// Cache that maps legacy shader bitfields to concrete WGPU render pipelines.
///
/// The DX8 renderer baked render state into a single `ShaderClass` bitfield. We mimic that here by
/// using the bit pattern plus the active surface format as the cache key, ensuring that identical
/// shader/material combinations reuse the same pipeline instance.
#[derive(Debug, Default)]
pub struct ShaderPipelineCache {
    pipelines: HashMap<PipelineKey, Arc<wgpu::RenderPipeline>>,
}

impl ShaderPipelineCache {
    /// Construct an empty cache.
    pub fn new() -> Self {
        Self {
            pipelines: HashMap::new(),
        }
    }

    /// Retrieve a pipeline for the provided shader configuration, building and caching it on demand.
    pub fn get_or_create(
        &mut self,
        shader: &ShaderClass,
        device: &Device,
        config: &SurfaceConfiguration,
        vertex_layout: VertexLayoutKind,
        sample_count: u32,
    ) -> Arc<wgpu::RenderPipeline> {
        let key = PipelineKey {
            shader_bits: shader.get_bits(),
            surface_format: config.format,
            sample_count,
            vertex_layout,
        };

        if let Some(pipeline) = self.pipelines.get(&key) {
            return pipeline.clone();
        }

        let pipeline = shader.create_pipeline(device, config, vertex_layout, sample_count);
        let arc = Arc::new(pipeline);
        self.pipelines.insert(key, arc.clone());
        arc
    }

    /// Drop all cached pipelines. This should be invoked whenever the surface format changes or the
    /// underlying device is recreated.
    pub fn clear(&mut self) {
        self.pipelines.clear();
    }
}
