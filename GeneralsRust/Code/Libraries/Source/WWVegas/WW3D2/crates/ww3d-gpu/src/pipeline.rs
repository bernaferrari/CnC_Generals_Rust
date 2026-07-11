//! GPU Pipeline Management
//!
//! This module provides render and compute pipeline creation, management,
//! and caching functionality for efficient GPU resource usage.

use crate::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Render pipeline abstraction
#[derive(Debug)]
pub struct RenderPipeline {
    /// WGPU render pipeline
    pipeline: wgpu::RenderPipeline,
    /// Pipeline layout
    layout: wgpu::PipelineLayout,
    /// Compiled shader
    compiled_shader: Arc<crate::shader::CompiledShader>,
    /// Pipeline label
    label: Option<String>,
}

impl RenderPipeline {
    /// Create a new render pipeline
    pub fn new(
        device: &crate::device::GpuDevice,
        layout: &wgpu::PipelineLayoutDescriptor,
        compiled_shader: Arc<crate::shader::CompiledShader>,
        primitive: wgpu::PrimitiveState,
        multisample: wgpu::MultisampleState,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let pipeline_layout = device.create_pipeline_layout(layout);

        let color_targets = &[Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            blend: compiled_shader
                .blend_state
                .or(Some(wgpu::BlendState::REPLACE)),
            write_mask: wgpu::ColorWrites::ALL,
        })];

        let desc = wgpu::RenderPipelineDescriptor {
            label,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &compiled_shader.vertex_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive,
            depth_stencil: compiled_shader.depth_stencil.clone(),
            multisample,
            fragment: Some(wgpu::FragmentState {
                module: &compiled_shader.fragment_module,
                entry_point: Some("fs_main"),
                targets: color_targets,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview: None,
            cache: None,
        };

        let pipeline = device.create_render_pipeline(&desc);

        Ok(Self {
            pipeline,
            layout: pipeline_layout,
            compiled_shader,
            label: label.map(|s| s.to_string()),
        })
    }

    /// Create a basic render pipeline
    pub fn create_basic(
        device: &crate::device::GpuDevice,
        compiled_shader: Arc<crate::shader::CompiledShader>,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let layout_desc = wgpu::PipelineLayoutDescriptor {
            label,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        };

        Self::new(
            device,
            &layout_desc,
            compiled_shader,
            wgpu::PrimitiveState::default(),
            wgpu::MultisampleState::default(),
            label,
        )
    }

    /// Get the WGPU render pipeline
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    /// Get the pipeline layout
    pub fn layout(&self) -> &wgpu::PipelineLayout {
        &self.layout
    }

    /// Get the compiled shader
    pub fn compiled_shader(&self) -> &Arc<crate::shader::CompiledShader> {
        &self.compiled_shader
    }

    /// Get the pipeline label
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Check if pipeline has depth testing
    pub fn has_depth(&self) -> bool {
        self.compiled_shader.depth_stencil.is_some()
    }

    /// Check if pipeline has fragment shader
    pub fn has_fragment_shader(&self) -> bool {
        true // Always has fragment shader in our design
    }
}

/// Compute pipeline abstraction - stub for future implementation
#[derive(Debug)]
pub struct ComputePipeline {
    /// WGPU compute pipeline
    pipeline: wgpu::ComputePipeline,
    /// Pipeline layout
    layout: wgpu::PipelineLayout,
    /// Pipeline label
    label: Option<String>,
}

impl ComputePipeline {
    /// Create a new compute pipeline (not yet implemented for WW3D2)
    pub fn new(
        _device: &crate::device::GpuDevice,
        _layout: &wgpu::PipelineLayoutDescriptor,
        _label: Option<&str>,
    ) -> Result<Self, GpuError> {
        Err(GpuError::UnsupportedFeature(
            "Compute pipelines not yet implemented for WW3D2".to_string(),
        ))
    }

    /// Get the WGPU compute pipeline
    pub fn pipeline(&self) -> &wgpu::ComputePipeline {
        &self.pipeline
    }

    /// Get the pipeline layout
    pub fn layout(&self) -> &wgpu::PipelineLayout {
        &self.layout
    }

    /// Get the pipeline label
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

/// Pipeline manager for handling multiple pipelines
#[derive(Debug)]
pub struct PipelineManager {
    /// GPU device reference
    device: Arc<crate::device::GpuDevice>,
    /// Render pipelines
    render_pipelines: Vec<Arc<RenderPipeline>>,
    /// Compute pipelines
    compute_pipelines: Vec<Arc<ComputePipeline>>,
    /// Pipeline cache for reuse
    cache: HashMap<String, PipelineCacheEntry>,
    /// Pipeline statistics
    stats: PipelineStats,
}

impl PipelineManager {
    /// Create a new pipeline manager
    pub fn new(device: Arc<crate::device::GpuDevice>) -> Self {
        Self {
            device,
            render_pipelines: Vec::new(),
            compute_pipelines: Vec::new(),
            cache: HashMap::new(),
            stats: PipelineStats::default(),
        }
    }

    /// Create a render pipeline
    pub fn create_render_pipeline(
        &mut self,
        layout_desc: &wgpu::PipelineLayoutDescriptor,
        compiled_shader: Arc<crate::shader::CompiledShader>,
        primitive: wgpu::PrimitiveState,
        multisample: wgpu::MultisampleState,
        label: Option<&str>,
    ) -> Result<Arc<RenderPipeline>, GpuError> {
        // Check cache first
        if let Some(label) = label {
            if let Some(cached) = self.cache.get(label) {
                if let PipelineCacheEntry::Render(pipeline) = cached {
                    return Ok(pipeline.clone());
                }
            }
        }

        let pipeline = RenderPipeline::new(
            &self.device,
            layout_desc,
            compiled_shader,
            primitive,
            multisample,
            label,
        )?;

        let pipeline_arc = Arc::new(pipeline);
        self.render_pipelines.push(pipeline_arc.clone());

        if let Some(label) = label {
            self.cache.insert(
                label.to_string(),
                PipelineCacheEntry::Render(pipeline_arc.clone()),
            );
        }

        self.update_stats();
        Ok(pipeline_arc)
    }

    /// Create a basic render pipeline
    pub fn create_basic_render_pipeline(
        &mut self,
        compiled_shader: Arc<crate::shader::CompiledShader>,
        label: Option<&str>,
    ) -> Result<Arc<RenderPipeline>, GpuError> {
        let layout_desc = wgpu::PipelineLayoutDescriptor {
            label,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        };

        self.create_render_pipeline(
            &layout_desc,
            compiled_shader,
            wgpu::PrimitiveState::default(),
            wgpu::MultisampleState::default(),
            label,
        )
    }

    /// Create a compute pipeline (not yet implemented for WW3D2)
    pub fn create_compute_pipeline(
        &mut self,
        _layout_desc: &wgpu::PipelineLayoutDescriptor,
        _label: Option<&str>,
    ) -> Result<Arc<ComputePipeline>, GpuError> {
        Err(GpuError::UnsupportedFeature(
            "Compute pipelines not yet implemented for WW3D2".to_string(),
        ))
    }

    /// Get cached pipeline
    pub fn get_cached(&self, name: &str) -> Option<&PipelineCacheEntry> {
        self.cache.get(name)
    }

    /// Remove pipeline from cache
    pub fn remove_cached(&mut self, name: &str) -> bool {
        self.cache.remove(name).is_some()
    }

    /// Clear pipeline cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get pipeline statistics
    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }

    /// Update statistics
    fn update_stats(&mut self) {
        self.stats.render_pipeline_count = self.render_pipelines.len();
        self.stats.compute_pipeline_count = self.compute_pipelines.len();
        self.stats.cached_pipelines = self.cache.len();
    }

    /// Cleanup unused pipelines
    pub fn cleanup(&mut self) {
        // In a real implementation, you'd track usage and remove unused pipelines
        // For now, just update stats
        self.update_stats();
    }
}

/// Pipeline cache entry
#[derive(Debug, Clone)]
pub enum PipelineCacheEntry {
    Render(Arc<RenderPipeline>),
    Compute(Arc<ComputePipeline>),
}

/// Pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    pub render_pipeline_count: usize,
    pub compute_pipeline_count: usize,
    pub cached_pipelines: usize,
}

/// Pipeline configuration helpers
pub struct PipelineConfig;

impl PipelineConfig {
    /// Create a default render pipeline configuration
    #[allow(unreachable_code)]
    pub fn default_render() -> wgpu::RenderPipelineDescriptor<'static> {
        wgpu::RenderPipelineDescriptor {
            label: Some("Default Render Pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: panic!("Vertex shader required"), // This would be set by caller
                entry_point: Some("main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: None,
            multiview: None,
            cache: None,
        }
    }

    /// Create a compute pipeline configuration
    #[allow(unreachable_code)]
    pub fn default_compute() -> wgpu::ComputePipelineDescriptor<'static> {
        wgpu::ComputePipelineDescriptor {
            label: Some("Default Compute Pipeline"),
            layout: None,
            module: panic!("Compute shader required"), // This would be set by caller
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        }
    }

    /// Create a wireframe render pipeline configuration
    pub fn wireframe() -> wgpu::PrimitiveState {
        wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Line,
            conservative: false,
        }
    }

    /// Create a transparent render pipeline configuration
    pub fn transparent() -> wgpu::ColorTargetState {
        wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_stats() {
        let stats = PipelineStats::default();
        assert_eq!(stats.render_pipeline_count, 0);
        assert_eq!(stats.compute_pipeline_count, 0);
        assert_eq!(stats.cached_pipelines, 0);
    }

    #[test]
    fn test_pipeline_config_wireframe() {
        let wireframe = PipelineConfig::wireframe();
        assert_eq!(wireframe.topology, wgpu::PrimitiveTopology::LineList);
        assert_eq!(wireframe.polygon_mode, wgpu::PolygonMode::Line);
        assert_eq!(wireframe.cull_mode, None);
    }

    #[test]
    fn test_pipeline_config_transparent() {
        let transparent = PipelineConfig::transparent();
        assert_eq!(transparent.format, wgpu::TextureFormat::Bgra8UnormSrgb);

        if let Some(blend) = transparent.blend {
            assert_eq!(blend.color.src_factor, wgpu::BlendFactor::SrcAlpha);
            assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::OneMinusSrcAlpha);
        } else {
            panic!("Transparent pipeline should have blend state");
        }
    }
}
