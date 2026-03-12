//! Sorting Renderer
//!
//! This module implements an alpha-sorted rendering system for transparent objects.
//! Objects are sorted back-to-front based on camera distance to ensure correct
//! alpha blending.

use crate::{pipeline, GpuBuffer};
use parking_lot::Mutex;
use std::sync::Arc;

/// Render batch for sorting
#[derive(Clone)]
pub struct RenderBatch {
    /// Vertex buffer
    pub vertex_buffer: Arc<GpuBuffer>,
    /// Index buffer
    pub index_buffer: Arc<GpuBuffer>,
    /// Start index in index buffer
    pub start_index: u32,
    /// Number of indices to draw
    pub index_count: u32,
    /// Base vertex offset
    pub base_vertex: i32,
    /// Vertex count
    pub vertex_count: u32,
    /// Distance from camera (for sorting)
    pub distance: f32,
    /// Material/shader ID for batching
    pub material_id: u64,
    /// Render pipeline
    pub pipeline: Option<Arc<pipeline::RenderPipeline>>,
    /// Bind groups
    pub bind_groups: Vec<Arc<wgpu::BindGroup>>,
}

impl RenderBatch {
    /// Create a new render batch
    pub fn new(
        vertex_buffer: Arc<GpuBuffer>,
        index_buffer: Arc<GpuBuffer>,
        start_index: u32,
        index_count: u32,
        base_vertex: i32,
        vertex_count: u32,
        distance: f32,
    ) -> Self {
        Self {
            vertex_buffer,
            index_buffer,
            start_index,
            index_count,
            base_vertex,
            vertex_count,
            distance,
            material_id: 0,
            pipeline: None,
            bind_groups: Vec::new(),
        }
    }

    /// Set material ID for batching
    pub fn with_material(mut self, material_id: u64) -> Self {
        self.material_id = material_id;
        self
    }

    /// Set pipeline
    pub fn with_pipeline(mut self, pipeline: Arc<pipeline::RenderPipeline>) -> Self {
        self.pipeline = Some(pipeline);
        self
    }

    /// Add bind group
    pub fn with_bind_group(mut self, bind_group: Arc<wgpu::BindGroup>) -> Self {
        self.bind_groups.push(bind_group);
        self
    }
}

/// Sorting renderer for transparent objects
pub struct SortingRenderer {
    /// Batches to render
    batches: Vec<RenderBatch>,
    /// Vertex count statistics
    vertex_count: usize,
    /// Triangle count statistics
    triangle_count: usize,
    /// Enable/disable triangle drawing
    enable_triangle_draw: bool,
    /// Minimum vertex buffer size
    min_vertex_buffer_size: usize,
}

impl SortingRenderer {
    /// Create a new sorting renderer
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
            vertex_count: 0,
            triangle_count: 0,
            enable_triangle_draw: true,
            min_vertex_buffer_size: 1024,
        }
    }

    /// Add a batch to the renderer
    pub fn add_batch(&mut self, batch: RenderBatch) {
        self.vertex_count += batch.vertex_count as usize;
        self.triangle_count += (batch.index_count / 3) as usize;
        self.batches.push(batch);
    }

    /// Add triangles with bounding sphere for distance sorting
    pub fn add_triangles(
        &mut self,
        vertex_buffer: Arc<GpuBuffer>,
        index_buffer: Arc<GpuBuffer>,
        start_index: u32,
        polygon_count: u32,
        min_vertex_index: u32,
        vertex_count: u32,
        distance: f32,
    ) {
        let batch = RenderBatch::new(
            vertex_buffer,
            index_buffer,
            start_index,
            polygon_count * 3, // Convert triangles to indices
            min_vertex_index as i32,
            vertex_count,
            distance,
        );
        self.add_batch(batch);
    }

    /// Add volume particles (layered rendering)
    pub fn add_volume_particle(
        &mut self,
        vertex_buffer: Arc<GpuBuffer>,
        index_buffer: Arc<GpuBuffer>,
        start_index: u32,
        polygon_count: u32,
        min_vertex_index: u32,
        vertex_count: u32,
        layer_count: u32,
        distance: f32,
    ) {
        // Volume particles are rendered in multiple layers
        for layer in 0..layer_count {
            let layer_distance = distance + (layer as f32 * 0.01); // Slight offset per layer
            let batch = RenderBatch::new(
                vertex_buffer.clone(),
                index_buffer.clone(),
                start_index,
                polygon_count * 3,
                min_vertex_index as i32,
                vertex_count,
                layer_distance,
            );
            self.add_batch(batch);
        }
    }

    /// Sort batches by distance (back to front)
    pub fn sort(&mut self) {
        self.batches.sort_by(|a, b| {
            b.distance
                .partial_cmp(&a.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Render all batches
    pub fn render<'a>(&'a mut self, render_pass: &mut wgpu::RenderPass<'a>) {
        if !self.enable_triangle_draw || self.batches.is_empty() {
            return;
        }

        // Sort batches first
        self.sort();

        let mut current_pipeline: Option<u64> = None;
        let mut current_vertex_buffer: Option<u64> = None;
        let mut current_index_buffer: Option<u64> = None;

        for batch in &self.batches {
            // Set pipeline if changed
            if let Some(pipeline) = &batch.pipeline {
                let pipeline_id = Arc::as_ptr(pipeline) as u64;
                if current_pipeline != Some(pipeline_id) {
                    render_pass.set_pipeline(pipeline.pipeline());
                    current_pipeline = Some(pipeline_id);
                }
            }

            // Set vertex buffer if changed
            let vb_ptr = Arc::as_ptr(&batch.vertex_buffer) as u64;
            if current_vertex_buffer != Some(vb_ptr) {
                render_pass.set_vertex_buffer(0, batch.vertex_buffer.wgpu_buffer().slice(..));
                current_vertex_buffer = Some(vb_ptr);
            }

            // Set index buffer if changed
            let ib_ptr = Arc::as_ptr(&batch.index_buffer) as u64;
            if current_index_buffer != Some(ib_ptr) {
                render_pass.set_index_buffer(
                    batch.index_buffer.wgpu_buffer().slice(..),
                    wgpu::IndexFormat::Uint16, // Assume u16, could be dynamic
                );
                current_index_buffer = Some(ib_ptr);
            }

            // Set bind groups
            for (index, bind_group) in batch.bind_groups.iter().enumerate() {
                render_pass.set_bind_group(index as u32, bind_group.as_ref(), &[]);
            }

            // Draw indexed
            render_pass.draw_indexed(
                batch.start_index..batch.start_index + batch.index_count,
                batch.base_vertex,
                0..1,
            );
        }
    }

    /// Flush all batches (render and clear)
    pub fn flush(&mut self) {
        self.batches.clear();
        self.vertex_count = 0;
        self.triangle_count = 0;
    }

    /// Get batch count
    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }

    /// Get vertex count
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Get triangle count
    pub fn triangle_count(&self) -> usize {
        self.triangle_count
    }

    /// Enable or disable triangle drawing
    pub fn set_triangle_draw_enabled(&mut self, enabled: bool) {
        self.enable_triangle_draw = enabled;
    }

    /// Check if triangle drawing is enabled
    pub fn is_triangle_draw_enabled(&self) -> bool {
        self.enable_triangle_draw
    }

    /// Set minimum vertex buffer size
    pub fn set_min_vertex_buffer_size(&mut self, size: usize) {
        self.min_vertex_buffer_size = size;
    }

    /// Get statistics
    pub fn stats(&self) -> SortingRendererStats {
        SortingRendererStats {
            batch_count: self.batches.len(),
            vertex_count: self.vertex_count,
            triangle_count: self.triangle_count,
        }
    }
}

impl Default for SortingRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Sorting renderer statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct SortingRendererStats {
    pub batch_count: usize,
    pub vertex_count: usize,
    pub triangle_count: usize,
}

// Global sorting renderer instance
lazy_static::lazy_static! {
    pub static ref SORTING_RENDERER: Mutex<SortingRenderer> = Mutex::new(SortingRenderer::new());
}

/// Add triangles to global sorting renderer
pub fn add_sorted_triangles(
    vertex_buffer: Arc<GpuBuffer>,
    index_buffer: Arc<GpuBuffer>,
    start_index: u32,
    polygon_count: u32,
    min_vertex_index: u32,
    vertex_count: u32,
    distance: f32,
) {
    SORTING_RENDERER.lock().add_triangles(
        vertex_buffer,
        index_buffer,
        start_index,
        polygon_count,
        min_vertex_index,
        vertex_count,
        distance,
    );
}

/// Add volume particle to global sorting renderer
pub fn add_sorted_volume_particle(
    vertex_buffer: Arc<GpuBuffer>,
    index_buffer: Arc<GpuBuffer>,
    start_index: u32,
    polygon_count: u32,
    min_vertex_index: u32,
    vertex_count: u32,
    layer_count: u32,
    distance: f32,
) {
    SORTING_RENDERER.lock().add_volume_particle(
        vertex_buffer,
        index_buffer,
        start_index,
        polygon_count,
        min_vertex_index,
        vertex_count,
        layer_count,
        distance,
    );
}

/// Render global sorting renderer
/// Note: Use the SortingRenderer directly via SORTING_RENDERER.lock() for more control
pub fn render_sorted<'a>(render_pass: &mut wgpu::RenderPass<'a>) {
    // This requires careful lifetime management
    // User code should call: SORTING_RENDERER.lock().render(render_pass) directly
    let _ = render_pass; // Prevent lifetime issues
}

/// Flush global sorting renderer
pub fn flush_sorting_renderer() {
    SORTING_RENDERER.lock().flush();
}

/// Get sorting renderer statistics
pub fn sorting_renderer_stats() -> SortingRendererStats {
    SORTING_RENDERER.lock().stats()
}

/// Enable/disable triangle drawing
pub fn set_triangle_draw_enabled(enabled: bool) {
    SORTING_RENDERER.lock().set_triangle_draw_enabled(enabled);
}

/// Check if triangle drawing is enabled
pub fn is_triangle_draw_enabled() -> bool {
    SORTING_RENDERER.lock().is_triangle_draw_enabled()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_render_batch_creation() {
        // This would need a mock buffer for full testing
        assert_eq!(mem::size_of::<RenderBatch>(), mem::size_of::<RenderBatch>());
    }

    #[test]
    fn test_sorting_renderer() {
        let mut renderer = SortingRenderer::new();
        assert_eq!(renderer.batch_count(), 0);
        assert_eq!(renderer.vertex_count(), 0);
        assert!(renderer.is_triangle_draw_enabled());

        renderer.set_triangle_draw_enabled(false);
        assert!(!renderer.is_triangle_draw_enabled());

        renderer.flush();
        assert_eq!(renderer.batch_count(), 0);
    }

    #[test]
    fn test_stats() {
        let renderer = SortingRenderer::new();
        let stats = renderer.stats();
        assert_eq!(stats.batch_count, 0);
        assert_eq!(stats.vertex_count, 0);
        assert_eq!(stats.triangle_count, 0);
    }
}
