//! GPU Command Buffer and Encoder Management
//!
//! This module provides command buffer creation, encoding, and submission
//! functionality for recording and executing GPU commands.

use crate::*;
use std::sync::Arc;

/// Command encoder abstraction
#[derive(Debug)]
pub struct CommandEncoder {
    /// WGPU command encoder
    encoder: wgpu::CommandEncoder,
    /// Encoder label
    label: Option<String>,
    /// Whether the encoder is active
    is_active: bool,
}

impl CommandEncoder {
    /// Create a new command encoder
    pub fn new(device: &crate::device::GpuDevice, label: Option<&str>) -> Self {
        let encoder = device.create_command_encoder(label);

        Self {
            encoder,
            label: label.map(|s| s.to_string()),
            is_active: true,
        }
    }

    /// Wrap an existing wgpu::CommandEncoder
    pub fn wrap(encoder: wgpu::CommandEncoder, label: Option<String>) -> Self {
        Self {
            encoder,
            label,
            is_active: true,
        }
    }

    /// Get the underlying WGPU command encoder
    pub fn encoder(&self) -> &wgpu::CommandEncoder {
        &self.encoder
    }

    /// Get the encoder label
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Check if encoder is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Begin a render pass
    pub fn begin_render_pass<'a>(
        &'a mut self,
        desc: &wgpu::RenderPassDescriptor<'a>,
    ) -> RenderPass<'a> {
        let render_pass = self.encoder.begin_render_pass(desc);

        RenderPass { render_pass }
    }

    /// Begin a compute pass
    pub fn begin_compute_pass<'a>(
        &'a mut self,
        desc: &wgpu::ComputePassDescriptor<'a>,
    ) -> ComputePass<'a> {
        let compute_pass = self.encoder.begin_compute_pass(desc);

        ComputePass { compute_pass }
    }

    /// Copy buffer to buffer
    pub fn copy_buffer_to_buffer<'a>(
        &mut self,
        source: &'a crate::Buffer,
        source_offset: u64,
        destination: &'a crate::Buffer,
        destination_offset: u64,
        size: u64,
    ) {
        self.encoder.copy_buffer_to_buffer(
            &source.buffer,
            source_offset,
            &destination.buffer,
            destination_offset,
            size,
        );
    }

    /// Copy texture to texture
    pub fn copy_texture_to_texture(
        &mut self,
        source: wgpu::TexelCopyTextureInfo<'_>,
        destination: wgpu::TexelCopyTextureInfo<'_>,
        size: wgpu::Extent3d,
    ) {
        self.encoder
            .copy_texture_to_texture(source, destination, size);
    }

    /// Copy buffer to texture
    pub fn copy_buffer_to_texture(
        &mut self,
        source: wgpu::TexelCopyBufferInfo<'_>,
        destination: wgpu::TexelCopyTextureInfo<'_>,
        size: wgpu::Extent3d,
    ) {
        self.encoder
            .copy_buffer_to_texture(source, destination, size);
    }

    /// Copy texture to buffer
    pub fn copy_texture_to_buffer(
        &mut self,
        source: wgpu::TexelCopyTextureInfo<'_>,
        destination: wgpu::TexelCopyBufferInfo<'_>,
        size: wgpu::Extent3d,
    ) {
        self.encoder
            .copy_texture_to_buffer(source, destination, size);
    }

    /// Clear buffer
    pub fn clear_buffer(&mut self, buffer: &crate::Buffer, offset: u64, size: Option<u64>) {
        self.encoder.clear_buffer(&buffer.buffer, offset, size);
    }

    /// Insert debug marker
    pub fn insert_debug_marker(&mut self, label: &str) {
        // WGPU doesn't have debug markers in the same way
        // This is a placeholder for potential future extensions
        let _ = label;
    }

    /// Push debug group
    pub fn push_debug_group(&mut self, label: &str) {
        // WGPU doesn't have debug groups in the same way
        let _ = label;
    }

    /// Pop debug group
    pub fn pop_debug_group(&mut self) {
        // WGPU doesn't have debug groups in the same way
    }

    /// Finish encoding and return command buffer
    pub fn finish(mut self) -> CommandBuffer {
        let command_buffer = self.encoder.finish();
        self.is_active = false;

        CommandBuffer {
            command_buffer,
            label: self.label,
        }
    }
}

/// Command buffer abstraction
#[derive(Debug)]
pub struct CommandBuffer {
    /// WGPU command buffer
    command_buffer: wgpu::CommandBuffer,
    /// Command buffer label
    label: Option<String>,
}

impl CommandBuffer {
    /// Get the underlying WGPU command buffer
    pub fn command_buffer(&self) -> &wgpu::CommandBuffer {
        &self.command_buffer
    }

    /// Consume self and return the inner WGPU command buffer
    pub fn into_wgpu_command_buffer(self) -> wgpu::CommandBuffer {
        self.command_buffer
    }

    /// Get the command buffer label
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

/// Render pass abstraction
#[derive(Debug)]
pub struct RenderPass<'a> {
    /// WGPU render pass
    render_pass: wgpu::RenderPass<'a>,
}

impl<'a> RenderPass<'a> {
    /// Get the inner WGPU render pass
    pub fn inner(&mut self) -> &mut wgpu::RenderPass<'a> {
        &mut self.render_pass
    }

    pub fn wrap(render_pass: wgpu::RenderPass<'a>) -> Self {
        Self { render_pass }
    }

    /// Set the render pipeline
    pub fn set_pipeline<'b>(&mut self, pipeline: &'b crate::pipeline::RenderPipeline)
    where
        'b: 'a,
    {
        self.render_pass.set_pipeline(pipeline.pipeline());
    }

    /// Set vertex buffer
    pub fn set_vertex_buffer<'b>(&mut self, slot: u32, buffer: &'b crate::Buffer, offset: u64)
    where
        'b: 'a,
    {
        self.render_pass
            .set_vertex_buffer(slot, buffer.buffer.slice(offset..));
    }

    /// Set index buffer
    pub fn set_index_buffer<'b>(
        &mut self,
        buffer: &'b crate::Buffer,
        offset: u64,
        format: wgpu::IndexFormat,
    ) where
        'b: 'a,
    {
        self.render_pass
            .set_index_buffer(buffer.buffer.slice(offset..), format);
    }

    /// Set bind group
    pub fn set_bind_group<'b>(
        &mut self,
        index: u32,
        bind_group: &'b wgpu::BindGroup,
        offsets: &[u32],
    ) where
        'b: 'a,
    {
        self.render_pass.set_bind_group(index, bind_group, offsets);
    }

    /// Set push constants
    pub fn set_push_constants(&mut self, stages: wgpu::ShaderStages, offset: u32, data: &[u8]) {
        self.render_pass.set_push_constants(stages, offset, data);
    }

    /// Set viewport
    pub fn set_viewport(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) {
        self.render_pass
            .set_viewport(x, y, width, height, min_depth, max_depth);
    }

    /// Set scissor rectangle
    pub fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.render_pass.set_scissor_rect(x, y, width, height);
    }

    /// Set blend constant
    pub fn set_blend_constant(&mut self, color: wgpu::Color) {
        self.render_pass.set_blend_constant(color);
    }

    /// Set stencil reference
    pub fn set_stencil_reference(&mut self, reference: u32) {
        self.render_pass.set_stencil_reference(reference);
    }

    /// Draw vertices
    pub fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        self.render_pass.draw(vertices, instances);
    }

    /// Draw indexed vertices
    pub fn draw_indexed(
        &mut self,
        indices: std::ops::Range<u32>,
        base_vertex: i32,
        instances: std::ops::Range<u32>,
    ) {
        self.render_pass
            .draw_indexed(indices, base_vertex, instances);
    }

    /// Draw indirect
    pub fn draw_indirect<'b>(&mut self, indirect_buffer: &'b crate::Buffer, offset: u64)
    where
        'b: 'a,
    {
        self.render_pass
            .draw_indirect(&indirect_buffer.buffer, offset);
    }

    /// Draw indexed indirect
    pub fn draw_indexed_indirect<'b>(&mut self, indirect_buffer: &'b crate::Buffer, offset: u64)
    where
        'b: 'a,
    {
        self.render_pass
            .draw_indexed_indirect(&indirect_buffer.buffer, offset);
    }

    /// Insert debug marker
    pub fn insert_debug_marker(&mut self, label: &str) {
        // WGPU doesn't have render pass debug markers
        let _ = label;
    }

    /// Push debug group
    pub fn push_debug_group(&mut self, label: &str) {
        // WGPU doesn't have render pass debug groups
        let _ = label;
    }

    /// Pop debug group
    pub fn pop_debug_group(&mut self) {
        // WGPU doesn't have render pass debug groups
    }
}

/// Compute pass abstraction
#[derive(Debug)]
pub struct ComputePass<'a> {
    /// WGPU compute pass
    compute_pass: wgpu::ComputePass<'a>,
}

impl<'a> ComputePass<'a> {
    /// Get the inner WGPU compute pass
    pub fn inner(&mut self) -> &mut wgpu::ComputePass<'a> {
        &mut self.compute_pass
    }
}

impl<'a> ComputePass<'a> {
    /// Set the compute pipeline
    pub fn set_pipeline<'b>(&mut self, pipeline: &'b crate::pipeline::ComputePipeline)
    where
        'b: 'a,
    {
        self.compute_pass.set_pipeline(pipeline.pipeline());
    }

    /// Set bind group
    pub fn set_bind_group<'b>(
        &mut self,
        index: u32,
        bind_group: &'b wgpu::BindGroup,
        offsets: &[u32],
    ) where
        'b: 'a,
    {
        self.compute_pass.set_bind_group(index, bind_group, offsets);
    }

    /// Set push constants
    pub fn set_push_constants(&mut self, offset: u32, data: &[u8]) {
        self.compute_pass.set_push_constants(offset, data);
    }

    /// Dispatch compute workgroups
    pub fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32) {
        self.compute_pass.dispatch_workgroups(x, y, z);
    }

    /// Dispatch compute workgroups indirect
    pub fn dispatch_workgroups_indirect<'b>(
        &mut self,
        indirect_buffer: &'b crate::Buffer,
        offset: u64,
    ) where
        'b: 'a,
    {
        self.compute_pass
            .dispatch_workgroups_indirect(&indirect_buffer.buffer, offset);
    }

    /// Insert debug marker
    pub fn insert_debug_marker(&mut self, label: &str) {
        // WGPU doesn't have compute pass debug markers
        let _ = label;
    }

    /// Push debug group
    pub fn push_debug_group(&mut self, label: &str) {
        // WGPU doesn't have compute pass debug groups
        let _ = label;
    }

    /// Pop debug group
    pub fn pop_debug_group(&mut self) {
        // WGPU doesn't have compute pass debug groups
    }
}

/// Command buffer manager
#[derive(Debug)]
pub struct CommandBufferManager {
    /// GPU device reference
    device: Arc<crate::device::GpuDevice>,
    /// Completed command buffers
    command_buffers: Vec<CommandBuffer>,
    /// Command statistics
    stats: CommandStats,
}

impl CommandBufferManager {
    /// Create a new command buffer manager
    pub fn new(device: Arc<crate::device::GpuDevice>) -> Self {
        Self {
            device,
            command_buffers: Vec::new(),
            stats: CommandStats::default(),
        }
    }

    /// Create a new command encoder
    pub fn create_encoder(&self, label: Option<&str>) -> CommandEncoder {
        CommandEncoder::new(&self.device, label)
    }

    /// Submit all pending command buffers
    pub fn submit_all(&mut self) {
        let command_buffers: Vec<_> = self.command_buffers.drain(..).collect();

        if !command_buffers.is_empty() {
            let raw_buffers = command_buffers
                .into_iter()
                .map(|cb| cb.command_buffer)
                .collect();
            self.device.submit(raw_buffers);
            self.update_stats();
        }
    }

    /// Submit specific command buffers
    pub fn submit(&mut self, command_buffers: Vec<CommandBuffer>) {
        let wgpu_buffers: Vec<_> = command_buffers
            .into_iter()
            .map(|cb| cb.command_buffer)
            .collect();

        if !wgpu_buffers.is_empty() {
            self.device.submit(wgpu_buffers);
            self.update_stats();
        }
    }

    /// Finish encoding a command encoder and get the command buffer
    pub fn finish_encoder(&mut self, encoder: CommandEncoder) {
        let command_buffer = encoder.finish();
        self.command_buffers.push(command_buffer);
        self.update_stats();
    }

    /// Get command statistics
    pub fn stats(&self) -> &CommandStats {
        &self.stats
    }

    /// Update statistics
    fn update_stats(&mut self) {
        self.stats.encoder_count = 0;
        self.stats.command_buffer_count = self.command_buffers.len();
        // In a real implementation, you'd track more detailed statistics
    }

    /// Cleanup completed command buffers
    pub fn cleanup(&mut self) {
        // Remove completed command buffers that have been submitted
        self.command_buffers.clear();
        self.update_stats();
    }
}

/// Command statistics
#[derive(Debug, Clone, Default)]
pub struct CommandStats {
    pub encoder_count: usize,
    pub command_buffer_count: usize,
    pub total_commands: usize,
    pub render_passes: usize,
    pub compute_passes: usize,
}

/// Command builder for fluent API
#[derive(Debug)]
pub struct CommandBuilder<'a> {
    encoder: &'a mut CommandEncoder,
}

impl<'a> CommandBuilder<'a> {
    /// Create a new command builder
    pub fn new(encoder: &'a mut CommandEncoder) -> Self {
        Self { encoder }
    }

    /// Begin a render pass
    pub fn begin_render_pass(self, desc: &wgpu::RenderPassDescriptor<'a>) -> RenderPassBuilder<'a> {
        let render_pass_wrapper = self.encoder.begin_render_pass(desc);
        RenderPassBuilder {
            render_pass: render_pass_wrapper.render_pass,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Begin a compute pass
    pub fn begin_compute_pass(
        self,
        desc: &wgpu::ComputePassDescriptor<'a>,
    ) -> ComputePassBuilder<'a> {
        let compute_pass_wrapper = self.encoder.begin_compute_pass(desc);
        ComputePassBuilder {
            compute_pass: compute_pass_wrapper.compute_pass,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Copy buffer to buffer
    pub fn copy_buffer_to_buffer(
        self,
        source: &crate::Buffer,
        source_offset: u64,
        destination: &crate::Buffer,
        destination_offset: u64,
        size: u64,
    ) -> Self {
        self.encoder.copy_buffer_to_buffer(
            source,
            source_offset,
            destination,
            destination_offset,
            size,
        );
        self
    }

    /// Finish building and return the encoder
    pub fn finish(self) -> &'a mut CommandEncoder {
        self.encoder
    }
}

/// Render pass builder for fluent API
#[derive(Debug)]
pub struct RenderPassBuilder<'a> {
    render_pass: wgpu::RenderPass<'a>,
    _phantom: std::marker::PhantomData<&'a mut CommandEncoder>,
}

impl<'a> RenderPassBuilder<'a> {
    /// Set pipeline
    pub fn set_pipeline<'b>(mut self, pipeline: &'b crate::pipeline::RenderPipeline) -> Self
    where
        'b: 'a,
    {
        self.render_pass.set_pipeline(pipeline.pipeline());
        self
    }

    /// Set vertex buffer
    pub fn set_vertex_buffer<'b>(
        mut self,
        slot: u32,
        buffer: &'b crate::Buffer,
        offset: u64,
    ) -> Self
    where
        'b: 'a,
    {
        self.render_pass
            .set_vertex_buffer(slot, buffer.buffer.slice(offset..));
        self
    }

    /// Set index buffer
    pub fn set_index_buffer<'b>(
        mut self,
        buffer: &'b crate::Buffer,
        offset: u64,
        format: wgpu::IndexFormat,
    ) -> Self
    where
        'b: 'a,
    {
        self.render_pass
            .set_index_buffer(buffer.buffer.slice(offset..), format);
        self
    }

    /// Set bind group
    pub fn set_bind_group<'b>(
        mut self,
        index: u32,
        bind_group: &'b wgpu::BindGroup,
        offsets: &[u32],
    ) -> Self
    where
        'b: 'a,
    {
        self.render_pass.set_bind_group(index, bind_group, offsets);
        self
    }

    /// Draw
    pub fn draw(mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) -> Self {
        self.render_pass.draw(vertices, instances);
        self
    }

    /// Draw indexed
    pub fn draw_indexed(
        mut self,
        indices: std::ops::Range<u32>,
        base_vertex: i32,
        instances: std::ops::Range<u32>,
    ) -> Self {
        self.render_pass
            .draw_indexed(indices, base_vertex, instances);
        self
    }
}

/// Compute pass builder for fluent API
#[derive(Debug)]
pub struct ComputePassBuilder<'a> {
    compute_pass: wgpu::ComputePass<'a>,
    _phantom: std::marker::PhantomData<&'a mut CommandEncoder>,
}

impl<'a> ComputePassBuilder<'a> {
    /// Set pipeline
    pub fn set_pipeline<'b>(mut self, pipeline: &'b crate::pipeline::ComputePipeline) -> Self
    where
        'b: 'a,
    {
        self.compute_pass.set_pipeline(pipeline.pipeline());
        self
    }

    /// Set bind group
    pub fn set_bind_group<'b>(
        mut self,
        index: u32,
        bind_group: &'b wgpu::BindGroup,
        offsets: &[u32],
    ) -> Self
    where
        'b: 'a,
    {
        self.compute_pass.set_bind_group(index, bind_group, offsets);
        self
    }

    /// Dispatch
    pub fn dispatch(mut self, x: u32, y: u32, z: u32) -> Self {
        self.compute_pass.dispatch_workgroups(x, y, z);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_stats() {
        let stats = CommandStats::default();
        assert_eq!(stats.encoder_count, 0);
        assert_eq!(stats.command_buffer_count, 0);
        assert_eq!(stats.total_commands, 0);
    }

    #[test]
    fn test_command_buffer_manager() {
        // Test the stats structure independently
        let stats = CommandStats::default();

        assert_eq!(stats.encoder_count, 0);
        assert_eq!(stats.command_buffer_count, 0);
        assert_eq!(stats.total_commands, 0);
        assert_eq!(stats.render_passes, 0);
        assert_eq!(stats.compute_passes, 0);

        // Note: Full CommandBufferManager testing would require a real GPU device
        // which is not available in unit tests
    }
}
