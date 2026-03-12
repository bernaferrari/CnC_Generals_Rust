//! Shader Renderer System
//!
//! This module provides the central rendering system for shader-based meshes.
//! It manages render nodes, batching, sorting, and the actual rendering pipeline.
//! The system is designed to be graphics API agnostic but includes a DirectX-style
//! implementation as the primary backend.

use once_cell::sync::Lazy;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
// Removed async_trait as we're making traits synchronous

use crate::error::{ShdError, ShdResult};
use crate::interface::{RenderInfo, ShdInterface, MAX_PASSES};
use crate::loader::MeshGeometry;
use crate::manager::ShdDefManager;

/// Maximum number of rendering passes supported
pub const SHD_MAX_PASSES: usize = MAX_PASSES as usize;

/// Vertex buffer abstraction
#[derive(Debug, Clone)]
pub struct VertexBuffer {
    pub data: Vec<u8>,
    pub vertex_count: u32,
    pub vertex_size: u32,
    pub usage_flags: VertexBufferUsage,
}

/// Index buffer abstraction
#[derive(Debug, Clone)]
pub struct IndexBuffer {
    pub data: Vec<u16>,
    pub index_count: u32,
    pub usage_flags: IndexBufferUsage,
}

/// Vertex buffer usage flags
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VertexBufferUsage {
    Default,
    Dynamic,
    SoftwareProcessing,
}

/// Index buffer usage flags  
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndexBufferUsage {
    Default,
    Dynamic,
    SoftwareProcessing,
}

/// Light environment for rendering
#[derive(Debug, Clone)]
pub struct LightEnvironment {
    pub ambient_color: glam::Vec3,
    pub primary_light_color: glam::Vec3,
    pub primary_light_direction: glam::Vec3,
    pub secondary_lights: Vec<PointLight>,
}

/// Point light definition
#[derive(Debug, Clone)]
pub struct PointLight {
    pub position: glam::Vec3,
    pub color: glam::Vec3,
    pub attenuation: glam::Vec3, // constant, linear, quadratic
}

impl Default for LightEnvironment {
    fn default() -> Self {
        Self {
            ambient_color: glam::Vec3::splat(0.1),
            primary_light_color: glam::Vec3::ONE,
            primary_light_direction: glam::Vec3::new(0.0, -1.0, -1.0).normalize(),
            secondary_lights: Vec::new(),
        }
    }
}

/// Render node trait - represents a renderable object
pub trait RenderNode: Send + Sync + std::fmt::Debug {
    /// Get the class ID of the associated shader
    fn get_shaderclass_id(&self) -> u32;

    /// Render this node for the specified pass
    fn render(&mut self, pass: u32, render_info: &RenderInfo) -> ShdResult<()>;

    /// Flush any pending rendering operations for this pass
    fn flush(&mut self, pass: u32) -> ShdResult<()>;

    /// Apply shared shader settings (called once per shader type per frame)
    fn apply_shared_shader_settings(
        &mut self,
        previous_node: Option<&dyn RenderNode>,
        pass: u32,
    ) -> ShdResult<()>;

    /// Compare this node with another for sorting purposes
    fn compare_for_sorting(&self, other: &dyn RenderNode, pass: u32) -> Ordering;

    /// Check if this node is similar enough to another to be batched together
    fn is_similar_enough(&self, other: &dyn RenderNode, pass: u32) -> bool;

    /// Connect this node to the visible list (mark as visible for this frame)
    fn connect(&mut self);

    /// Get the mesh geometry associated with this node
    fn get_mesh_geometry(&self) -> Arc<MeshGeometry>;
}

/// Container for managing render nodes of the same shader type
#[derive(Debug)]
pub struct RendererListContainer {
    pass: u32,
    linked_nodes: Vec<Arc<Mutex<dyn RenderNode>>>,
    visible_nodes: Vec<Arc<Mutex<dyn RenderNode>>>,
}

impl RendererListContainer {
    fn new(pass: u32) -> Self {
        Self {
            pass,
            linked_nodes: Vec::new(),
            visible_nodes: Vec::new(),
        }
    }

    /// Add a node to the visible list for this frame
    pub fn add_visible_node(&mut self, node: Arc<Mutex<dyn RenderNode>>) {
        self.visible_nodes.push(node);
    }

    /// Register a render node with this container
    pub fn register_renderer(&mut self, node: Arc<Mutex<dyn RenderNode>>) {
        self.linked_nodes.push(node);
    }

    /// Remove all registered nodes
    pub fn unregister_all(&mut self) {
        self.linked_nodes.clear();
        self.visible_nodes.clear();
    }

    /// Flush all visible nodes in sorted order
    pub fn flush(&mut self) -> ShdResult<()> {
        if self.visible_nodes.is_empty() {
            return Ok(());
        }

        // Sort nodes for optimal rendering
        self.visible_nodes.sort_by(|a, b| {
            let node_a = a.lock().unwrap();
            let node_b = b.lock().unwrap();
            node_a.compare_for_sorting(&*node_b, self.pass)
        });

        // Process each node
        let mut previousclass_id: Option<u32> = None;

        for node_arc in &mut self.visible_nodes {
            let class_id = {
                let node = node_arc.lock().unwrap();
                node.get_shaderclass_id()
            };

            // Apply shared shader settings
            let needs_reset = if let Some(prev_id) = previousclass_id {
                prev_id != class_id
            } else {
                true
            };

            // Apply shared shader settings if needed
            if needs_reset {
                {
                    let mut node = node_arc.lock().unwrap();
                    node.apply_shared_shader_settings(None, self.pass)?;
                }
            }

            // Flush the node
            {
                let mut node = node_arc.lock().unwrap();
                node.flush(self.pass)?;
            }

            previousclass_id = Some(class_id);
        }

        // Clear visible nodes for next frame
        self.visible_nodes.clear();

        Ok(())
    }
}

/// Mesh container for managing nodes of a specific shader class
#[derive(Debug)]
#[allow(dead_code)]
struct MeshContainer {
    class_id: u32,
    renderer_lists: [Option<RendererListContainer>; SHD_MAX_PASSES],
}

impl MeshContainer {
    fn new(class_id: u32) -> Self {
        Self {
            class_id,
            renderer_lists: Default::default(),
        }
    }

    /// Register a mesh with this container
    fn register_mesh(
        &mut self,
        node: Arc<Mutex<dyn RenderNode>>,
        pass_count: u32,
    ) -> ShdResult<()> {
        for pass in 0..pass_count.min(SHD_MAX_PASSES as u32) {
            let pass_index = pass as usize;

            if self.renderer_lists[pass_index].is_none() {
                self.renderer_lists[pass_index] = Some(RendererListContainer::new(pass));
            }

            if let Some(container) = &mut self.renderer_lists[pass_index] {
                container.register_renderer(node.clone());
            }
        }

        Ok(())
    }

    /// Flush all passes for this container
    fn flush(&mut self) -> ShdResult<()> {
        for container in &mut self.renderer_lists {
            if let Some(cont) = container {
                cont.flush()?;
            }
        }
        Ok(())
    }
}

/// Abstract shader renderer interface
pub trait ShaderRenderer: Send + Sync {
    /// Initialize the renderer
    fn initialize(&mut self) -> ShdResult<()>;

    /// Shutdown the renderer
    fn shutdown(&mut self) -> ShdResult<()>;

    /// Register a mesh for rendering
    fn register_mesh(
        &mut self,
        mesh: Arc<MeshGeometry>,
        shader: Arc<Mutex<dyn ShdInterface>>,
    ) -> ShdResult<Arc<Mutex<dyn RenderNode>>>;

    /// Flush all registered meshes
    fn flush(&mut self) -> ShdResult<()>;

    /// Apply default render state
    fn apply_default_state(&mut self) -> ShdResult<()>;

    /// Set the current transformation matrix
    fn set_transform(&mut self, matrix: &glam::Mat4) -> ShdResult<()>;

    /// Set the current light environment
    fn setlight_environment(&mut self, lights: &LightEnvironment) -> ShdResult<()>;

    /// Draw triangles with the current state
    fn draw_triangles(
        &mut self,
        start_index: u32,
        triangle_count: u32,
        start_vertex: u32,
        vertex_count: u32,
    ) -> ShdResult<()>;
}

/// Default render node implementation
#[derive(Debug)]
#[allow(dead_code)]
pub struct DefaultRenderNode {
    mesh: Arc<MeshGeometry>,
    shader: Arc<Mutex<dyn ShdInterface>>,
    vertex_buffers: Vec<VertexBuffer>,
    index_buffer: Option<IndexBuffer>,
    light_environment: LightEnvironment,
    render_info: Option<RenderInfo>,
    is_visible: bool,
}

impl DefaultRenderNode {
    pub fn new(mesh: Arc<MeshGeometry>, shader: Arc<Mutex<dyn ShdInterface>>) -> ShdResult<Self> {
        let mut node = Self {
            mesh: mesh.clone(),
            shader: shader.clone(),
            vertex_buffers: Vec::new(),
            index_buffer: None,
            light_environment: LightEnvironment::default(),
            render_info: None,
            is_visible: false,
        };

        node.initialize_buffers()?;
        Ok(node)
    }

    fn initialize_buffers(&mut self) -> ShdResult<()> {
        // Create vertex buffer
        let shader = self.shader.lock().unwrap();
        let stream_count = shader.get_vertex_stream_count();

        self.vertex_buffers.clear();
        self.vertex_buffers.reserve(stream_count as usize);

        for stream in 0..stream_count {
            let vertex_size = shader.get_vertex_size(stream);
            let vertex_count = self.mesh.vertices.len() as u32;

            let usage = if shader.use_hardware_vertex_processing() {
                VertexBufferUsage::Default
            } else {
                VertexBufferUsage::SoftwareProcessing
            };

            // Create vertex data - this is simplified, in a real implementation
            // you would properly format the vertex data according to the shader's requirements
            let mut vertex_data = Vec::with_capacity((vertex_size * vertex_count) as usize);

            for vertex in &self.mesh.vertices {
                // Position (12 bytes)
                vertex_data.extend_from_slice(&vertex.position.x.to_le_bytes());
                vertex_data.extend_from_slice(&vertex.position.y.to_le_bytes());
                vertex_data.extend_from_slice(&vertex.position.z.to_le_bytes());

                // Normal (12 bytes)
                vertex_data.extend_from_slice(&vertex.normal.x.to_le_bytes());
                vertex_data.extend_from_slice(&vertex.normal.y.to_le_bytes());
                vertex_data.extend_from_slice(&vertex.normal.z.to_le_bytes());

                // UV coordinates (8 bytes)
                vertex_data.extend_from_slice(&vertex.uv.x.to_le_bytes());
                vertex_data.extend_from_slice(&vertex.uv.y.to_le_bytes());

                // Pad to vertex_size if needed
                while vertex_data.len() < (vertex_size as usize) {
                    vertex_data.push(0);
                }
            }

            self.vertex_buffers.push(VertexBuffer {
                data: vertex_data,
                vertex_count,
                vertex_size,
                usage_flags: usage,
            });
        }

        // Create index buffer
        if !self.mesh.indices.is_empty() {
            let index_count = (self.mesh.indices.len() * 3) as u32;
            let mut index_data = Vec::with_capacity(index_count as usize);

            for triangle in &self.mesh.indices {
                index_data.extend_from_slice(&triangle.indices);
            }

            let usage = if shader.use_hardware_vertex_processing() {
                IndexBufferUsage::Default
            } else {
                IndexBufferUsage::SoftwareProcessing
            };

            self.index_buffer = Some(IndexBuffer {
                data: index_data,
                index_count,
                usage_flags: usage,
            });
        }

        Ok(())
    }
}

impl RenderNode for DefaultRenderNode {
    fn get_shaderclass_id(&self) -> u32 {
        self.shader.lock().unwrap().get_class_id()
    }

    fn render(&mut self, _pass: u32, render_info: &RenderInfo) -> ShdResult<()> {
        self.render_info = Some(render_info.clone());
        self.is_visible = true;
        Ok(())
    }

    fn flush(&mut self, pass: u32) -> ShdResult<()> {
        if !self.is_visible {
            return Ok(());
        }

        let mut shader = self.shader.lock().unwrap();

        // Apply instance-specific rendering
        if let Some(ref render_info) = self.render_info {
            shader.apply_instance(pass, render_info)?;
        }

        self.is_visible = false;
        Ok(())
    }

    fn apply_shared_shader_settings(
        &mut self,
        previous_node: Option<&dyn RenderNode>,
        pass: u32,
    ) -> ShdResult<()> {
        let mut shader = self.shader.lock().unwrap();

        // Check if we need to reset state
        if let Some(prev) = previous_node {
            if prev.get_shaderclass_id() != self.get_shaderclass_id() {
                // Different shader type, need to reset to default state
                // In a real implementation, this would call the graphics API
            }
        }

        let render_info = self.render_info.as_ref().cloned().unwrap_or_default();
        shader.apply_shared(pass, &render_info)?;
        Ok(())
    }

    fn compare_for_sorting(&self, other: &dyn RenderNode, _pass: u32) -> Ordering {
        let _shader = self.shader.lock().unwrap();
        let otherclass_id = other.get_shaderclass_id();

        // Compare by shader class ID first
        self.get_shaderclass_id().cmp(&otherclass_id)
    }

    fn is_similar_enough(&self, other: &dyn RenderNode, _pass: u32) -> bool {
        self.get_shaderclass_id() == other.get_shaderclass_id()
    }

    fn connect(&mut self) {
        self.is_visible = true;
    }

    fn get_mesh_geometry(&self) -> Arc<MeshGeometry> {
        self.mesh.clone()
    }
}

/// Main shader renderer implementation
#[derive(Debug)]
pub struct ShdRenderer {
    mesh_containers: HashMap<u32, MeshContainer>,
    current_transform: glam::Mat4,
    current_light_environment: LightEnvironment,
    is_initialized: bool,
}

impl ShdRenderer {
    pub fn new() -> Self {
        Self {
            mesh_containers: HashMap::new(),
            current_transform: glam::Mat4::IDENTITY,
            current_light_environment: LightEnvironment::default(),
            is_initialized: false,
        }
    }
}

impl ShaderRenderer for ShdRenderer {
    fn initialize(&mut self) -> ShdResult<()> {
        if self.is_initialized {
            return Err(ShdError::InvalidConfig(
                "Renderer already initialized".to_string(),
            ));
        }

        // Initialize all shader definitions
        self.initialize_shaders()?;

        self.is_initialized = true;
        Ok(())
    }

    fn shutdown(&mut self) -> ShdResult<()> {
        if !self.is_initialized {
            return Ok(());
        }

        // Shutdown all shader definitions
        self.shutdown_shaders()?;

        // Clear all containers
        self.mesh_containers.clear();

        self.is_initialized = false;
        Ok(())
    }

    fn register_mesh(
        &mut self,
        mesh: Arc<MeshGeometry>,
        shader: Arc<Mutex<dyn ShdInterface>>,
    ) -> ShdResult<Arc<Mutex<dyn RenderNode>>> {
        let node = Arc::new(Mutex::new(DefaultRenderNode::new(mesh, shader.clone())?));

        let class_id = {
            let shader_guard = shader.lock().unwrap();
            shader_guard.get_class_id()
        };

        // Get or create mesh container for this shader class
        if !self.mesh_containers.contains_key(&class_id) {
            self.mesh_containers
                .insert(class_id, MeshContainer::new(class_id));
        }

        let container = self.mesh_containers.get_mut(&class_id).unwrap();
        let pass_count = {
            let shader_guard = shader.lock().unwrap();
            shader_guard.get_pass_count()
        };

        container.register_mesh(node.clone(), pass_count)?;

        Ok(node)
    }

    fn flush(&mut self) -> ShdResult<()> {
        // Apply default state
        self.apply_default_state()?;

        // Flush all containers in class ID order for consistent rendering
        let mut class_ids: Vec<_> = self.mesh_containers.keys().cloned().collect();
        class_ids.sort();

        for class_id in class_ids {
            if let Some(container) = self.mesh_containers.get_mut(&class_id) {
                container.flush()?;
            }
        }

        // Reset state after rendering
        self.apply_default_state()?;

        Ok(())
    }

    fn apply_default_state(&mut self) -> ShdResult<()> {
        // In a real implementation, this would reset graphics API state
        // For now, just reset our internal state
        self.current_transform = glam::Mat4::IDENTITY;
        Ok(())
    }

    fn set_transform(&mut self, matrix: &glam::Mat4) -> ShdResult<()> {
        self.current_transform = *matrix;
        Ok(())
    }

    fn setlight_environment(&mut self, lights: &LightEnvironment) -> ShdResult<()> {
        self.current_light_environment = lights.clone();
        Ok(())
    }

    fn draw_triangles(
        &mut self,
        _start_index: u32,
        _triangle_count: u32,
        _start_vertex: u32,
        _vertex_count: u32,
    ) -> ShdResult<()> {
        // In a real implementation, this would call the graphics API to draw
        Ok(())
    }
}

impl ShdRenderer {
    /// Initialize all shader definitions
    fn initialize_shaders(&mut self) -> ShdResult<()> {
        // This would iterate through all available shader class IDs and initialize them
        // For now, we'll use a simplified approach

        let factories = ShdDefManager::get_all_factories()?;
        for factory in factories {
            let class_id = factory.get_class_id();
            if let Ok(shader_def) = factory.create_definition(class_id) {
                // Initialize the shader definition
                let _ = shader_def.is_valid_config();
            }
        }

        Ok(())
    }

    /// Shutdown all shader definitions
    fn shutdown_shaders(&mut self) -> ShdResult<()> {
        // Cleanup shader definitions
        // In a real implementation, this would properly cleanup graphics resources
        Ok(())
    }
}

/// Global renderer instance
static GLOBAL_RENDERER: Lazy<RwLock<Option<Box<dyn ShaderRenderer>>>> =
    Lazy::new(|| RwLock::new(None));

/// Shader renderer manager - provides global access to the renderer
pub struct ShdRendererManager;

impl ShdRendererManager {
    /// Initialize the global renderer
    pub fn initialize() -> ShdResult<()> {
        let mut renderer = Box::new(ShdRenderer::new());
        renderer.initialize()?;

        let mut global = GLOBAL_RENDERER
            .write()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire renderer lock".to_string()))?;

        *global = Some(renderer);
        Ok(())
    }

    /// Shutdown the global renderer
    pub fn shutdown() -> ShdResult<()> {
        let mut global = GLOBAL_RENDERER
            .write()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire renderer lock".to_string()))?;

        if let Some(renderer) = global.as_mut() {
            renderer.shutdown()?;
        }

        *global = None;
        Ok(())
    }

    /// Get access to the global renderer
    pub fn with_renderer<F, R>(f: F) -> ShdResult<R>
    where
        F: FnOnce(&mut dyn ShaderRenderer) -> ShdResult<R>,
    {
        let mut global = GLOBAL_RENDERER
            .write()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire renderer lock".to_string()))?;

        if let Some(renderer) = global.as_mut() {
            f(renderer.as_mut())
        } else {
            Err(ShdError::InvalidConfig(
                "Renderer not initialized".to_string(),
            ))
        }
    }

    /// Flush the global renderer
    pub fn flush() -> ShdResult<()> {
        let mut global = GLOBAL_RENDERER
            .write()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire renderer lock".to_string()))?;

        if let Some(renderer) = global.as_mut() {
            renderer.flush()
        } else {
            Err(ShdError::InvalidConfig(
                "Renderer not initialized".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def::ShdDefClass;
    use crate::interface::RenderInfo;
    use crate::loader::Vertex3D;

    // Mock shader for testing
    #[derive(Debug)]
    struct MockShader {
        class_id: u32,
        pass_count: u32,
    }

    impl ShdInterface for MockShader {
        fn get_class_id(&self) -> u32 {
            self.class_id
        }

        fn get_pass_count(&self) -> u32 {
            self.pass_count
        }

        fn apply_shared(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
            Ok(())
        }

        fn apply_instance(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
            Ok(())
        }
    }

    fn create_test_mesh() -> Arc<MeshGeometry> {
        let mut mesh = MeshGeometry::new("TestMesh".to_string());

        mesh.vertices.push(Vertex3D {
            position: glam::Vec3::new(0.0, 0.0, 0.0),
            normal: glam::Vec3::Y,
            uv: glam::Vec2::new(0.0, 0.0),
            color: 0xFFFFFFFF,
            tangent: None,
            binormal: None,
        });

        mesh.vertices.push(Vertex3D {
            position: glam::Vec3::new(1.0, 0.0, 0.0),
            normal: glam::Vec3::Y,
            uv: glam::Vec2::new(1.0, 0.0),
            color: 0xFFFFFFFF,
            tangent: None,
            binormal: None,
        });

        mesh.vertices.push(Vertex3D {
            position: glam::Vec3::new(0.0, 1.0, 0.0),
            normal: glam::Vec3::Y,
            uv: glam::Vec2::new(0.0, 1.0),
            color: 0xFFFFFFFF,
            tangent: None,
            binormal: None,
        });

        mesh.indices.push(crate::loader::TriIndex::new(0, 1, 2));

        Arc::new(mesh)
    }

    #[test]
    fn testlight_environment_creation() {
        let lights = LightEnvironment::default();
        assert_eq!(lights.ambient_color, glam::Vec3::splat(0.1));
        assert_eq!(lights.primary_light_color, glam::Vec3::ONE);
        assert!(lights.secondary_lights.is_empty());
    }

    #[test]
    fn test_vertex_buffer_creation() {
        let buffer = VertexBuffer {
            data: vec![1, 2, 3, 4],
            vertex_count: 1,
            vertex_size: 32,
            usage_flags: VertexBufferUsage::Default,
        };

        assert_eq!(buffer.vertex_count, 1);
        assert_eq!(buffer.vertex_size, 32);
        assert_eq!(buffer.usage_flags, VertexBufferUsage::Default);
    }

    #[test]
    fn test_index_buffer_creation() {
        let buffer = IndexBuffer {
            data: vec![0, 1, 2],
            index_count: 3,
            usage_flags: IndexBufferUsage::Default,
        };

        assert_eq!(buffer.index_count, 3);
        assert_eq!(buffer.usage_flags, IndexBufferUsage::Default);
    }

    #[test]
    fn test_default_render_node_creation() {
        let mesh = create_test_mesh();
        let shader = Arc::new(Mutex::new(MockShader {
            class_id: 100,
            pass_count: 1,
        }));

        let node = DefaultRenderNode::new(mesh.clone(), shader.clone());
        assert!(node.is_ok());

        let node = node.unwrap();
        assert_eq!(node.get_shaderclass_id(), 100);
        assert!(!node.vertex_buffers.is_empty());
        assert!(node.index_buffer.is_some());
    }

    #[test]
    fn test_renderer_list_container() {
        let mut container = RendererListContainer::new(0);
        assert_eq!(container.pass, 0);
        assert!(container.linked_nodes.is_empty());
        assert!(container.visible_nodes.is_empty());

        let mesh = create_test_mesh();
        let shader = Arc::new(Mutex::new(MockShader {
            class_id: 200,
            pass_count: 1,
        }));
        let node = Arc::new(Mutex::new(DefaultRenderNode::new(mesh, shader).unwrap()));

        container.register_renderer(node.clone());
        assert_eq!(container.linked_nodes.len(), 1);

        container.add_visible_node(node);
        assert_eq!(container.visible_nodes.len(), 1);

        // Test flush
        let result = container.flush();
        assert!(result.is_ok());
        assert!(container.visible_nodes.is_empty()); // Should be cleared after flush
    }

    #[test]
    fn test_shd_renderer_creation() {
        let mut renderer = ShdRenderer::new();
        assert!(!renderer.is_initialized);
        assert!(renderer.mesh_containers.is_empty());

        // Test initialization
        let result = renderer.initialize();
        assert!(result.is_ok());
        assert!(renderer.is_initialized);

        // Test double initialization fails
        let result = renderer.initialize();
        assert!(result.is_err());

        // Test shutdown
        let result = renderer.shutdown();
        assert!(result.is_ok());
        assert!(!renderer.is_initialized);
    }

    #[test]
    fn test_mesh_registration() {
        let mut renderer = ShdRenderer::new();
        renderer.initialize().unwrap();

        let mesh = create_test_mesh();
        let shader = Arc::new(Mutex::new(MockShader {
            class_id: 300,
            pass_count: 1,
        }));

        let node = renderer.register_mesh(mesh, shader);
        assert!(node.is_ok());

        let node = node.unwrap();
        assert_eq!(node.lock().unwrap().get_shaderclass_id(), 300);

        // Check that container was created
        assert!(renderer.mesh_containers.contains_key(&300));

        renderer.shutdown().unwrap();
    }

    #[test]
    fn test_renderer_flush() {
        let mut renderer = ShdRenderer::new();
        renderer.initialize().unwrap();

        let mesh = create_test_mesh();
        let shader = Arc::new(Mutex::new(MockShader {
            class_id: 400,
            pass_count: 1,
        }));

        let _node = renderer.register_mesh(mesh, shader).unwrap();

        let result = renderer.flush();
        assert!(result.is_ok());

        renderer.shutdown().unwrap();
    }
}
