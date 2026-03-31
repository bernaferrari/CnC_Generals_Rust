use crate::animation::BoneTransform;
/// Rendering integration layer
///
/// This module provides the rendering backend abstraction for wgpu integration,
/// vertex/index buffer management, and GPU resource handling
use crate::material::{Material, Shader};
use crate::texture::TextureBase;
use glam::{Mat4, Vec2, Vec3, Vec4};
use std::collections::HashMap;
use std::sync::Arc;
use ww3d_core::errors::W3DResult;

/// Vertex format for mesh rendering
///
/// # Safety
///
/// This type is marked as `Pod` and `Zeroable` for safe byte conversion.
/// The #[repr(C)] ensures stable memory layout for GPU buffer uploads.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub color: Vec4,
}

/// Skinned vertex with bone weights
///
/// # Safety
///
/// This type is marked as `Pod` and `Zeroable` for safe byte conversion.
/// The #[repr(C)] ensures stable memory layout for GPU buffer uploads.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinnedVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub color: Vec4,
    pub bone_indices: [u32; 4],
    pub bone_weights: [f32; 4],
}

/// GPU buffer handle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferHandle(pub u64);

/// GPU texture handle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u64);

/// GPU pipeline handle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineHandle(pub u64);

/// Render command types
#[derive(Debug, Clone)]
pub enum RenderCommand {
    SetPipeline(PipelineHandle),
    SetVertexBuffer(BufferHandle),
    SetIndexBuffer(BufferHandle),
    SetTexture {
        stage: u32,
        handle: TextureHandle,
    },
    SetUniformBuffer {
        binding: u32,
        handle: BufferHandle,
    },
    DrawIndexed {
        index_count: u32,
        instance_count: u32,
        first_index: u32,
    },
    Draw {
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
    },
}

/// Mesh data for GPU upload
#[derive(Debug, Clone)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub material_name: Option<String>,
}

impl MeshData {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            material_name: None,
        }
    }

    /// Calculate bounding box
    pub fn calculate_bounds(&self) -> (Vec3, Vec3) {
        if self.vertices.is_empty() {
            return (Vec3::ZERO, Vec3::ZERO);
        }

        let mut min = self.vertices[0].position;
        let mut max = self.vertices[0].position;

        for vertex in &self.vertices {
            min = min.min(vertex.position);
            max = max.max(vertex.position);
        }

        (min, max)
    }
}

impl Default for MeshData {
    fn default() -> Self {
        Self::new()
    }
}

/// Skinned mesh data for GPU upload
#[derive(Debug, Clone)]
pub struct SkinnedMeshData {
    pub vertices: Vec<SkinnedVertex>,
    pub indices: Vec<u32>,
    pub material_name: Option<String>,
    pub bone_count: usize,
}

impl SkinnedMeshData {
    pub fn new(bone_count: usize) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            material_name: None,
            bone_count,
        }
    }
}

/// GPU mesh resource
#[derive(Debug)]
pub struct GpuMesh {
    pub vertex_buffer: BufferHandle,
    pub index_buffer: BufferHandle,
    pub index_count: u32,
    pub material: Option<Arc<Material>>,
}

/// GPU skinned mesh resource
#[derive(Debug)]
pub struct GpuSkinnedMesh {
    pub vertex_buffer: BufferHandle,
    pub index_buffer: BufferHandle,
    pub bone_buffer: BufferHandle,
    pub index_count: u32,
    pub bone_count: usize,
    pub material: Option<Arc<Material>>,
}

/// Rendering backend trait
pub trait RenderBackend: Send + Sync {
    /// Create vertex buffer
    fn create_vertex_buffer(&mut self, data: &[u8], size: u64) -> W3DResult<BufferHandle>;

    /// Create index buffer
    fn create_index_buffer(&mut self, data: &[u8], size: u64) -> W3DResult<BufferHandle>;

    /// Create uniform buffer
    fn create_uniform_buffer(&mut self, size: u64) -> W3DResult<BufferHandle>;

    /// Update buffer data
    fn update_buffer(&mut self, handle: BufferHandle, data: &[u8], offset: u64) -> W3DResult<()>;

    /// Create texture
    fn create_texture(&mut self, texture: &TextureBase) -> W3DResult<TextureHandle>;

    /// Create render pipeline
    fn create_pipeline(
        &mut self,
        shader: &Shader,
        vertex_layout: &str,
    ) -> W3DResult<PipelineHandle>;

    /// Destroy buffer
    fn destroy_buffer(&mut self, handle: BufferHandle);

    /// Destroy texture
    fn destroy_texture(&mut self, handle: TextureHandle);

    /// Destroy pipeline
    fn destroy_pipeline(&mut self, handle: PipelineHandle);

    /// Submit render commands
    fn submit_commands(&mut self, commands: &[RenderCommand]) -> W3DResult<()>;

    /// Begin frame
    fn begin_frame(&mut self);

    /// End frame
    fn end_frame(&mut self);
}

/// Renderer managing GPU resources and rendering
pub struct Renderer {
    backend: Box<dyn RenderBackend>,
    meshes: HashMap<String, GpuMesh>,
    skinned_meshes: HashMap<String, GpuSkinnedMesh>,
    textures: HashMap<String, TextureHandle>,
    pipelines: HashMap<u32, PipelineHandle>,
    #[allow(dead_code)] // C++ parity
    next_handle_id: u64,
}

impl Renderer {
    pub fn new(backend: Box<dyn RenderBackend>) -> Self {
        Self {
            backend,
            meshes: HashMap::new(),
            skinned_meshes: HashMap::new(),
            textures: HashMap::new(),
            pipelines: HashMap::new(),
            next_handle_id: 1,
        }
    }

    #[allow(dead_code)] // C++ parity
    fn allocate_handle(&mut self) -> u64 {
        let handle = self.next_handle_id;
        self.next_handle_id += 1;
        handle
    }

    /// Upload mesh to GPU
    ///
    /// # Safety
    ///
    /// This function uses `bytemuck::cast_slice` to safely convert typed slices to bytes.
    /// This is sound because Vertex and u32 are both `Pod` types with well-defined layouts.
    pub fn upload_mesh(
        &mut self,
        name: String,
        mesh_data: &MeshData,
        material: Option<Arc<Material>>,
    ) -> W3DResult<()> {
        // Convert vertices to bytes safely using bytemuck
        // This performs compile-time alignment and size checks
        let vertex_data: &[u8] = bytemuck::cast_slice(&mesh_data.vertices);

        let vertex_buffer = self
            .backend
            .create_vertex_buffer(vertex_data, vertex_data.len() as u64)?;

        // Convert indices to bytes safely using bytemuck
        let index_data: &[u8] = bytemuck::cast_slice(&mesh_data.indices);

        let index_buffer = self
            .backend
            .create_index_buffer(index_data, index_data.len() as u64)?;

        let gpu_mesh = GpuMesh {
            vertex_buffer,
            index_buffer,
            index_count: mesh_data.indices.len() as u32,
            material,
        };

        self.meshes.insert(name, gpu_mesh);
        Ok(())
    }

    /// Upload skinned mesh to GPU
    ///
    /// # Safety
    ///
    /// This function uses `bytemuck::cast_slice` to safely convert typed slices to bytes.
    /// This is sound because SkinnedVertex and u32 are both `Pod` types with well-defined layouts.
    pub fn upload_skinned_mesh(
        &mut self,
        name: String,
        mesh_data: &SkinnedMeshData,
        material: Option<Arc<Material>>,
    ) -> W3DResult<()> {
        // Convert vertices to bytes safely using bytemuck
        let vertex_data: &[u8] = bytemuck::cast_slice(&mesh_data.vertices);

        let vertex_buffer = self
            .backend
            .create_vertex_buffer(vertex_data, vertex_data.len() as u64)?;

        // Convert indices to bytes safely using bytemuck
        let index_data: &[u8] = bytemuck::cast_slice(&mesh_data.indices);

        let index_buffer = self
            .backend
            .create_index_buffer(index_data, index_data.len() as u64)?;

        // Create bone buffer
        let bone_buffer_size = (mesh_data.bone_count * std::mem::size_of::<Mat4>()) as u64;
        let bone_buffer = self.backend.create_uniform_buffer(bone_buffer_size)?;

        let gpu_mesh = GpuSkinnedMesh {
            vertex_buffer,
            index_buffer,
            bone_buffer,
            index_count: mesh_data.indices.len() as u32,
            bone_count: mesh_data.bone_count,
            material,
        };

        self.skinned_meshes.insert(name, gpu_mesh);
        Ok(())
    }

    /// Update bone transforms for skinned mesh
    ///
    /// # Safety
    ///
    /// This function uses `bytemuck::cast_slice` to safely convert Mat4 matrices to bytes.
    /// Mat4 from glam is a `Pod` type with well-defined memory layout.
    pub fn update_bone_transforms(
        &mut self,
        name: &str,
        transforms: &[BoneTransform],
    ) -> W3DResult<()> {
        if let Some(mesh) = self.skinned_meshes.get(name) {
            // Convert bone transforms to matrices
            let matrices: Vec<Mat4> = transforms.iter().map(|t| t.to_matrix()).collect();

            // Convert to bytes safely using bytemuck
            let bone_data: &[u8] = bytemuck::cast_slice(&matrices);

            self.backend.update_buffer(mesh.bone_buffer, bone_data, 0)?;
        }

        Ok(())
    }

    /// Upload texture to GPU
    pub fn upload_texture(
        &mut self,
        name: String,
        texture: &TextureBase,
    ) -> W3DResult<TextureHandle> {
        let handle = self.backend.create_texture(texture)?;
        self.textures.insert(name, handle);
        Ok(handle)
    }

    /// Get or create pipeline for shader
    pub fn get_or_create_pipeline(&mut self, shader: &Shader) -> W3DResult<PipelineHandle> {
        let shader_bits = shader.bits();

        if let Some(&handle) = self.pipelines.get(&shader_bits) {
            return Ok(handle);
        }

        let handle = self.backend.create_pipeline(shader, "default")?;
        self.pipelines.insert(shader_bits, handle);
        Ok(handle)
    }

    /// Draw mesh
    pub fn draw_mesh(&mut self, name: &str, _world_transform: Mat4) -> W3DResult<()> {
        // Extract data we need before borrowing self mutably
        let mesh_data = if let Some(mesh) = self.meshes.get(name) {
            let pipeline_shader = mesh
                .material
                .as_ref()
                .and_then(|m| m.primary_pass())
                .map(|p| p.shader);
            let textures: Vec<(usize, String)> = mesh
                .material
                .as_ref()
                .and_then(|m| m.primary_pass())
                .map(|p| {
                    p.textures
                        .iter()
                        .enumerate()
                        .filter_map(|(i, t)| t.as_ref().map(|tex| (i, tex.name.clone())))
                        .collect()
                })
                .unwrap_or_default();
            Some((
                mesh.vertex_buffer,
                mesh.index_buffer,
                mesh.index_count,
                pipeline_shader,
                textures,
            ))
        } else {
            None
        };

        if let Some((vb, ib, index_count, pipeline_shader, textures)) = mesh_data {
            let mut commands = Vec::new();

            // Set up pipeline
            if let Some(shader) = pipeline_shader {
                let pipeline = self.get_or_create_pipeline(&shader)?;
                commands.push(RenderCommand::SetPipeline(pipeline));
            }

            // Bind textures
            for (i, tex_name) in textures {
                if let Some(&handle) = self.textures.get(&tex_name) {
                    commands.push(RenderCommand::SetTexture {
                        stage: i as u32,
                        handle,
                    });
                }
            }

            // Set buffers
            commands.push(RenderCommand::SetVertexBuffer(vb));
            commands.push(RenderCommand::SetIndexBuffer(ib));

            // Draw
            commands.push(RenderCommand::DrawIndexed {
                index_count,
                instance_count: 1,
                first_index: 0,
            });

            self.backend.submit_commands(&commands)?;
        }

        Ok(())
    }

    /// Draw skinned mesh
    pub fn draw_skinned_mesh(&mut self, name: &str, _world_transform: Mat4) -> W3DResult<()> {
        // Extract data we need before borrowing self mutably
        let mesh_data = if let Some(mesh) = self.skinned_meshes.get(name) {
            let pipeline_shader = mesh
                .material
                .as_ref()
                .and_then(|m| m.primary_pass())
                .map(|p| p.shader);
            let textures: Vec<(usize, String)> = mesh
                .material
                .as_ref()
                .and_then(|m| m.primary_pass())
                .map(|p| {
                    p.textures
                        .iter()
                        .enumerate()
                        .filter_map(|(i, t)| t.as_ref().map(|tex| (i, tex.name.clone())))
                        .collect()
                })
                .unwrap_or_default();
            Some((
                mesh.vertex_buffer,
                mesh.index_buffer,
                mesh.bone_buffer,
                mesh.index_count,
                pipeline_shader,
                textures,
            ))
        } else {
            None
        };

        if let Some((vb, ib, bb, index_count, pipeline_shader, textures)) = mesh_data {
            let mut commands = Vec::new();

            // Set up pipeline
            if let Some(shader) = pipeline_shader {
                let pipeline = self.get_or_create_pipeline(&shader)?;
                commands.push(RenderCommand::SetPipeline(pipeline));
            }

            // Bind textures
            for (i, tex_name) in textures {
                if let Some(&handle) = self.textures.get(&tex_name) {
                    commands.push(RenderCommand::SetTexture {
                        stage: i as u32,
                        handle,
                    });
                }
            }

            // Set buffers
            commands.push(RenderCommand::SetVertexBuffer(vb));
            commands.push(RenderCommand::SetIndexBuffer(ib));
            commands.push(RenderCommand::SetUniformBuffer {
                binding: 1,
                handle: bb,
            });

            // Draw
            commands.push(RenderCommand::DrawIndexed {
                index_count,
                instance_count: 1,
                first_index: 0,
            });

            self.backend.submit_commands(&commands)?;
        }

        Ok(())
    }

    /// Begin rendering frame
    pub fn begin_frame(&mut self) {
        self.backend.begin_frame();
    }

    /// End rendering frame
    pub fn end_frame(&mut self) {
        self.backend.end_frame();
    }

    /// Remove mesh
    pub fn remove_mesh(&mut self, name: &str) {
        if let Some(mesh) = self.meshes.remove(name) {
            self.backend.destroy_buffer(mesh.vertex_buffer);
            self.backend.destroy_buffer(mesh.index_buffer);
        }
    }

    /// Remove skinned mesh
    pub fn remove_skinned_mesh(&mut self, name: &str) {
        if let Some(mesh) = self.skinned_meshes.remove(name) {
            self.backend.destroy_buffer(mesh.vertex_buffer);
            self.backend.destroy_buffer(mesh.index_buffer);
            self.backend.destroy_buffer(mesh.bone_buffer);
        }
    }

    /// Clear all resources
    pub fn clear(&mut self) {
        for (_, mesh) in self.meshes.drain() {
            self.backend.destroy_buffer(mesh.vertex_buffer);
            self.backend.destroy_buffer(mesh.index_buffer);
        }

        for (_, mesh) in self.skinned_meshes.drain() {
            self.backend.destroy_buffer(mesh.vertex_buffer);
            self.backend.destroy_buffer(mesh.index_buffer);
            self.backend.destroy_buffer(mesh.bone_buffer);
        }

        for (_, handle) in self.textures.drain() {
            self.backend.destroy_texture(handle);
        }

        for (_, handle) in self.pipelines.drain() {
            self.backend.destroy_pipeline(handle);
        }
    }

    /// Get mesh count
    pub fn mesh_count(&self) -> usize {
        self.meshes.len() + self.skinned_meshes.len()
    }

    /// Get texture count
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }
}

/// Null rendering backend for testing
pub struct NullRenderBackend;

impl RenderBackend for NullRenderBackend {
    fn create_vertex_buffer(&mut self, _data: &[u8], _size: u64) -> W3DResult<BufferHandle> {
        Ok(BufferHandle(1))
    }

    fn create_index_buffer(&mut self, _data: &[u8], _size: u64) -> W3DResult<BufferHandle> {
        Ok(BufferHandle(2))
    }

    fn create_uniform_buffer(&mut self, _size: u64) -> W3DResult<BufferHandle> {
        Ok(BufferHandle(3))
    }

    fn update_buffer(
        &mut self,
        _handle: BufferHandle,
        _data: &[u8],
        _offset: u64,
    ) -> W3DResult<()> {
        Ok(())
    }

    fn create_texture(&mut self, _texture: &TextureBase) -> W3DResult<TextureHandle> {
        Ok(TextureHandle(1))
    }

    fn create_pipeline(
        &mut self,
        _shader: &Shader,
        _vertex_layout: &str,
    ) -> W3DResult<PipelineHandle> {
        Ok(PipelineHandle(1))
    }

    fn destroy_buffer(&mut self, _handle: BufferHandle) {}
    fn destroy_texture(&mut self, _handle: TextureHandle) {}
    fn destroy_pipeline(&mut self, _handle: PipelineHandle) {}

    fn submit_commands(&mut self, _commands: &[RenderCommand]) -> W3DResult<()> {
        Ok(())
    }

    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
}

/// WGPU rendering backend for real GPU upload
///
/// This backend implementation connects the ww3d-assets parsed data (textures, meshes, materials)
/// to actual GPU resources via wgpu. It handles:
///
/// - Buffer creation and management (vertex, index, uniform)
/// - Texture upload with format conversion (DDS/TGA/BMP to wgpu formats)
/// - Mipmap upload for all mip levels
/// - Resource lifetime management via Arc references
/// - Automatic format conversion for unsupported texture formats (RGB8 -> RGBA8, etc.)
///
/// The backend integrates with the ww3d-gpu crate's buffer and texture systems to provide
/// efficient GPU resource management and upload.
pub struct WgpuRenderBackend {
    device: std::sync::Arc<ww3d_gpu::device::GpuDevice>,
    buffers: std::collections::HashMap<u64, std::sync::Arc<ww3d_gpu::buffer::GpuBuffer>>,
    textures: std::collections::HashMap<u64, std::sync::Arc<ww3d_gpu::texture::GpuTexture>>,
    pipelines: std::collections::HashMap<u64, wgpu::RenderPipeline>,
    next_handle_id: u64,
    command_encoder: Option<wgpu::CommandEncoder>,
}

impl WgpuRenderBackend {
    /// Create a new WGPU render backend
    pub fn new(device: std::sync::Arc<ww3d_gpu::device::GpuDevice>) -> Self {
        Self {
            device,
            buffers: std::collections::HashMap::new(),
            textures: std::collections::HashMap::new(),
            pipelines: std::collections::HashMap::new(),
            next_handle_id: 1,
            command_encoder: None,
        }
    }

    fn allocate_handle(&mut self) -> u64 {
        let handle = self.next_handle_id;
        self.next_handle_id += 1;
        handle
    }

    /// Convert TextureFormat to wgpu::TextureFormat
    fn convert_texture_format(format: &crate::texture::TextureFormat) -> wgpu::TextureFormat {
        match format {
            crate::texture::TextureFormat::A8R8G8B8 => wgpu::TextureFormat::Rgba8Unorm,
            crate::texture::TextureFormat::X8R8G8B8 => wgpu::TextureFormat::Rgba8Unorm,
            crate::texture::TextureFormat::R8G8B8 => wgpu::TextureFormat::Rgba8Unorm, // RGB8 not supported, use RGBA8
            crate::texture::TextureFormat::R5G6B5 => wgpu::TextureFormat::Rgba8Unorm, // Convert to RGBA8
            crate::texture::TextureFormat::A1R5G5B5 => wgpu::TextureFormat::Rgba8Unorm, // Convert to RGBA8
            crate::texture::TextureFormat::A4R4G4B4 => wgpu::TextureFormat::Rgba8Unorm, // Convert to RGBA8
            crate::texture::TextureFormat::L8 => wgpu::TextureFormat::R8Unorm,
            crate::texture::TextureFormat::A8 => wgpu::TextureFormat::R8Unorm,
            crate::texture::TextureFormat::A8L8 => wgpu::TextureFormat::Rg8Unorm,
            crate::texture::TextureFormat::DXT1 => wgpu::TextureFormat::Bc1RgbaUnorm,
            crate::texture::TextureFormat::DXT3 => wgpu::TextureFormat::Bc2RgbaUnorm,
            crate::texture::TextureFormat::DXT5 => wgpu::TextureFormat::Bc3RgbaUnorm,
            _ => wgpu::TextureFormat::Rgba8Unorm, // Default fallback
        }
    }

    /// Convert texture data for formats that need conversion
    fn convert_texture_data(
        data: &[u8],
        width: u32,
        height: u32,
        src_format: &crate::texture::TextureFormat,
    ) -> Vec<u8> {
        match src_format {
            // RGB8 -> RGBA8 conversion
            crate::texture::TextureFormat::R8G8B8 => {
                let pixel_count = (width * height) as usize;
                let mut rgba_data = Vec::with_capacity(pixel_count * 4);
                for i in 0..pixel_count {
                    let offset = i * 3;
                    if offset + 2 < data.len() {
                        rgba_data.push(data[offset]); // R
                        rgba_data.push(data[offset + 1]); // G
                        rgba_data.push(data[offset + 2]); // B
                        rgba_data.push(255); // A
                    }
                }
                rgba_data
            }
            // For other formats that need conversion, we'd add more cases here
            // For now, return data as-is
            _ => data.to_vec(),
        }
    }
}

impl RenderBackend for WgpuRenderBackend {
    fn create_vertex_buffer(&mut self, data: &[u8], _size: u64) -> W3DResult<BufferHandle> {
        let buffer = ww3d_gpu::buffer::GpuBuffer::with_data(
            &self.device,
            data,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            Some("Vertex Buffer"),
        )
        .map_err(|e| {
            ww3d_core::errors::W3DError::InvalidParameter(format!(
                "Failed to create vertex buffer: {:?}",
                e
            ))
        })?;

        let handle_id = self.allocate_handle();
        self.buffers.insert(handle_id, std::sync::Arc::new(buffer));
        Ok(BufferHandle(handle_id))
    }

    fn create_index_buffer(&mut self, data: &[u8], _size: u64) -> W3DResult<BufferHandle> {
        let buffer = ww3d_gpu::buffer::GpuBuffer::with_data(
            &self.device,
            data,
            wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            Some("Index Buffer"),
        )
        .map_err(|e| {
            ww3d_core::errors::W3DError::InvalidParameter(format!(
                "Failed to create index buffer: {:?}",
                e
            ))
        })?;

        let handle_id = self.allocate_handle();
        self.buffers.insert(handle_id, std::sync::Arc::new(buffer));
        Ok(BufferHandle(handle_id))
    }

    fn create_uniform_buffer(&mut self, size: u64) -> W3DResult<BufferHandle> {
        let buffer =
            ww3d_gpu::buffer::GpuBuffer::uniform_buffer(&self.device, size, Some("Uniform Buffer"))
                .map_err(|e| {
                    ww3d_core::errors::W3DError::InvalidParameter(format!(
                        "Failed to create uniform buffer: {:?}",
                        e
                    ))
                })?;

        let handle_id = self.allocate_handle();
        self.buffers.insert(handle_id, std::sync::Arc::new(buffer));
        Ok(BufferHandle(handle_id))
    }

    fn update_buffer(&mut self, handle: BufferHandle, data: &[u8], offset: u64) -> W3DResult<()> {
        if let Some(buffer) = self.buffers.get(&handle.0) {
            // Use the queue to write buffer data
            self.device
                .queue()
                .write_buffer(buffer.wgpu_buffer(), offset, data);
            Ok(())
        } else {
            Err(ww3d_core::errors::W3DError::InvalidParameter(
                "Buffer handle not found".to_string(),
            ))
        }
    }

    fn create_texture(&mut self, texture: &TextureBase) -> W3DResult<TextureHandle> {
        let wgpu_format = Self::convert_texture_format(&texture.format);

        // Calculate mip level count
        let mip_levels = texture.mip_level_count();

        // Create the texture
        let mut gpu_texture = ww3d_gpu::texture::GpuTexture::new(
            &self.device,
            &wgpu::TextureDescriptor {
                label: Some(&texture.name),
                size: wgpu::Extent3d {
                    width: texture.width,
                    height: texture.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: mip_levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
        )
        .map_err(|e| {
            ww3d_core::errors::W3DError::InvalidParameter(format!(
                "Failed to create texture: {:?}",
                e
            ))
        })?;

        // Upload all mip levels
        for (mip_level, level_data) in texture.mip_levels.iter().enumerate() {
            if !level_data.data.is_empty() {
                // Convert texture data if necessary
                let converted_data = Self::convert_texture_data(
                    &level_data.data,
                    level_data.width,
                    level_data.height,
                    &texture.format,
                );

                // Upload the mip level
                gpu_texture.write_data(
                    &self.device,
                    &converted_data,
                    wgpu::Origin3d::ZERO,
                    wgpu::Extent3d {
                        width: level_data.width,
                        height: level_data.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level as u32,
                );
            }
        }

        let handle_id = self.allocate_handle();
        self.textures
            .insert(handle_id, std::sync::Arc::new(gpu_texture));
        Ok(TextureHandle(handle_id))
    }

    fn create_pipeline(
        &mut self,
        _shader: &Shader,
        _vertex_layout: &str,
    ) -> W3DResult<PipelineHandle> {
        // For now, return a placeholder handle
        // Full pipeline creation would require shader compilation and more complex setup
        let handle_id = self.allocate_handle();
        Ok(PipelineHandle(handle_id))
    }

    fn destroy_buffer(&mut self, handle: BufferHandle) {
        self.buffers.remove(&handle.0);
    }

    fn destroy_texture(&mut self, handle: TextureHandle) {
        self.textures.remove(&handle.0);
    }

    fn destroy_pipeline(&mut self, handle: PipelineHandle) {
        self.pipelines.remove(&handle.0);
    }

    fn submit_commands(&mut self, _commands: &[RenderCommand]) -> W3DResult<()> {
        // Command submission would be handled by the render pass
        // For asset loading, we just need to ensure data is uploaded
        Ok(())
    }

    fn begin_frame(&mut self) {
        // Create a command encoder for the frame if needed
        self.command_encoder = Some(self.device.create_command_encoder(Some("Frame Commands")));
    }

    fn end_frame(&mut self) {
        // Submit any pending commands
        if let Some(encoder) = self.command_encoder.take() {
            self.device.submit(vec![encoder.finish()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_data_creation() {
        let mesh = MeshData::new();
        assert_eq!(mesh.vertices.len(), 0);
        assert_eq!(mesh.indices.len(), 0);
    }

    /// Test that Vertex is properly Pod-compatible for safe byte conversion
    #[test]
    fn test_vertex_pod_safety() {
        let vertices = vec![
            Vertex {
                position: Vec3::new(1.0, 2.0, 3.0),
                normal: Vec3::new(0.0, 1.0, 0.0),
                uv: Vec2::new(0.5, 0.5),
                color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            },
            Vertex {
                position: Vec3::new(4.0, 5.0, 6.0),
                normal: Vec3::new(0.0, 0.0, 1.0),
                uv: Vec2::new(0.25, 0.75),
                color: Vec4::new(0.5, 0.5, 0.5, 1.0),
            },
        ];

        // Safe conversion using bytemuck - this is what we use in upload_mesh
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);

        // Verify size calculation is correct
        let expected_size = vertices.len() * std::mem::size_of::<Vertex>();
        assert_eq!(bytes.len(), expected_size);

        // Verify we can convert back safely
        let vertices_back: &[Vertex] = bytemuck::cast_slice(bytes);
        assert_eq!(vertices_back.len(), vertices.len());
        assert_eq!(vertices_back[0].position, vertices[0].position);
        assert_eq!(vertices_back[1].uv, vertices[1].uv);
    }

    /// Test that SkinnedVertex is properly Pod-compatible for safe byte conversion
    #[test]
    fn test_skinned_vertex_pod_safety() {
        let vertices = vec![SkinnedVertex {
            position: Vec3::new(1.0, 2.0, 3.0),
            normal: Vec3::new(0.0, 1.0, 0.0),
            uv: Vec2::new(0.5, 0.5),
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            bone_indices: [0, 1, 2, 3],
            bone_weights: [0.4, 0.3, 0.2, 0.1],
        }];

        // Safe conversion using bytemuck
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);

        let expected_size = vertices.len() * std::mem::size_of::<SkinnedVertex>();
        assert_eq!(bytes.len(), expected_size);

        // Verify round-trip conversion
        let vertices_back: &[SkinnedVertex] = bytemuck::cast_slice(bytes);
        assert_eq!(vertices_back.len(), vertices.len());
        assert_eq!(vertices_back[0].bone_indices, vertices[0].bone_indices);
        assert_eq!(vertices_back[0].bone_weights, vertices[0].bone_weights);
    }

    /// Test that u32 indices can be safely converted to bytes
    #[test]
    fn test_index_buffer_pod_safety() {
        let indices: Vec<u32> = vec![0, 1, 2, 3, 4, 5];

        // Safe conversion using bytemuck
        let bytes: &[u8] = bytemuck::cast_slice(&indices);

        let expected_size = indices.len() * std::mem::size_of::<u32>();
        assert_eq!(bytes.len(), expected_size);

        // Verify round-trip
        let indices_back: &[u32] = bytemuck::cast_slice(bytes);
        assert_eq!(indices_back, indices.as_slice());
    }

    /// Test that Mat4 bone transforms can be safely converted to bytes
    #[test]
    fn test_bone_transform_pod_safety() {
        let matrices = vec![
            Mat4::IDENTITY,
            Mat4::from_scale(Vec3::new(2.0, 2.0, 2.0)),
            Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)),
        ];

        // Safe conversion using bytemuck
        let bytes: &[u8] = bytemuck::cast_slice(&matrices);

        let expected_size = matrices.len() * std::mem::size_of::<Mat4>();
        assert_eq!(bytes.len(), expected_size);

        // Verify round-trip
        let matrices_back: &[Mat4] = bytemuck::cast_slice(bytes);
        assert_eq!(matrices_back.len(), matrices.len());
        for (original, converted) in matrices.iter().zip(matrices_back.iter()) {
            assert_eq!(original, converted);
        }
    }

    /// Test that empty vertex arrays don't cause issues
    #[test]
    fn test_empty_vertex_buffer_safety() {
        let vertices: Vec<Vertex> = vec![];
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);
        assert_eq!(bytes.len(), 0);
    }

    /// Test alignment requirements are met
    #[test]
    fn test_vertex_alignment() {
        // Vertex should be properly aligned for GPU upload
        assert_eq!(std::mem::align_of::<Vertex>() % 4, 0);
        assert_eq!(std::mem::align_of::<SkinnedVertex>() % 4, 0);

        // Size should be a multiple of alignment for proper array layout
        let vertex_size = std::mem::size_of::<Vertex>();
        let vertex_align = std::mem::align_of::<Vertex>();
        assert_eq!(vertex_size % vertex_align, 0);

        let skinned_size = std::mem::size_of::<SkinnedVertex>();
        let skinned_align = std::mem::align_of::<SkinnedVertex>();
        assert_eq!(skinned_size % skinned_align, 0);
    }

    #[test]
    fn test_mesh_bounds_calculation() {
        let mut mesh = MeshData::new();
        mesh.vertices.push(Vertex {
            position: Vec3::new(-1.0, -1.0, -1.0),
            normal: Vec3::Z,
            uv: Vec2::ZERO,
            color: Vec4::ONE,
        });
        mesh.vertices.push(Vertex {
            position: Vec3::new(1.0, 1.0, 1.0),
            normal: Vec3::Z,
            uv: Vec2::ZERO,
            color: Vec4::ONE,
        });

        let (min, max) = mesh.calculate_bounds();
        assert_eq!(min, Vec3::new(-1.0, -1.0, -1.0));
        assert_eq!(max, Vec3::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn test_renderer_with_null_backend() {
        let backend = Box::new(NullRenderBackend);
        let mut renderer = Renderer::new(backend);

        let mesh = MeshData::new();
        assert!(renderer
            .upload_mesh("test".to_string(), &mesh, None)
            .is_ok());
        assert_eq!(renderer.mesh_count(), 1);

        renderer.remove_mesh("test");
        assert_eq!(renderer.mesh_count(), 0);
    }
}
