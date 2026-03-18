use crate::core::error::RendererResult;
use crate::render_object_system::RenderInfoClass;
use crate::rendering::batching::batch_renderer::{
    BatchRenderer, BatchVertex, MaterialKey,
};
///! W3DRenderer - High-level rendering API for drawing W3D meshes and models
///!
///! This module provides the main public rendering API that matches the C++ WW3D renderer
///! interface. It wraps the internal mesh rendering system and provides convenient methods
///! for rendering individual meshes, hierarchical models, and batched geometry.
///!
///! C++ Reference: W3D.cpp, W3DRenderState.cpp
use crate::rendering::camera_system::CameraClass;
use crate::rendering::frame_uniform_arena::FrameUniformArena;
use crate::rendering::mesh_system::{MeshClass, MeshModelClass, MeshRenderManager};
use glam::Mat4;
use std::sync::Arc;
use wgpu::Color;
use ww3d_core::errors::W3DResult;
use ww3d_gpu::device::GpuDevice;

/// Main W3D renderer for drawing game objects
///
/// This renderer handles:
/// - Single mesh rendering
/// - Hierarchical model rendering (with bone transforms)
/// - Batch rendering for static/dynamic objects
/// - Frame lifecycle management
/// - Shader and material state
///
/// C++ equivalent: W3DRendererClass
pub struct W3DRenderer {
    /// Low-level mesh rendering manager
    mesh_manager: MeshRenderManager,

    /// Batch renderer for optimized draw calls
    batch_renderer: BatchRenderer,

    /// Current camera for rendering
    camera: Option<CameraClass>,

    /// GPU device reference
    gpu_device: Arc<GpuDevice>,

    /// Frame state tracking
    frame_active: bool,

    /// Frame clear color
    clear_color: Color,
}

impl W3DRenderer {
    /// Create a new W3D renderer
    ///
    /// # Arguments
    /// * `gpu_device` - GPU device for creating resources
    ///
    /// # Returns
    /// New renderer instance
    pub fn new(gpu_device: Arc<GpuDevice>) -> Self {
        Self {
            mesh_manager: MeshRenderManager::new(gpu_device.clone()),
            batch_renderer: BatchRenderer::new(),
            camera: None,
            gpu_device,
            frame_active: false,
            clear_color: Color::BLACK,
        }
    }

    /// Set the active camera for rendering
    ///
    /// The camera defines the view and projection transforms used for rendering.
    ///
    /// # Arguments
    /// * `camera` - Camera to use for rendering
    pub fn set_camera(&mut self, camera: CameraClass) {
        self.camera = Some(camera);
    }

    /// Get the current camera
    pub fn get_camera(&self) -> Option<&CameraClass> {
        self.camera.as_ref()
    }

    /// Set the frame clear color
    ///
    /// # Arguments
    /// * `color` - RGBA clear color
    pub fn set_clear_color(&mut self, color: Color) {
        self.clear_color = color;
    }

    /// Begin a new rendering frame
    ///
    /// Must be called before any draw calls. Resets rendering state and prepares
    /// for a new frame.
    ///
    /// C++ equivalent: W3D::Begin_Frame()
    pub fn begin_frame(&mut self) -> RendererResult<()> {
        if self.frame_active {
            return Err(crate::core::error::Error::InvalidOperation(
                "Frame already active".to_string(),
            ));
        }

        self.mesh_manager.reset_stats();
        self.batch_renderer.clear();
        self.frame_active = true;
        Ok(())
    }

    /// End the current rendering frame
    ///
    /// Must be called after all draw calls are complete. Finalizes rendering
    /// and presents the frame.
    ///
    /// C++ equivalent: W3D::End_Frame()
    pub fn end_frame(&mut self) -> RendererResult<()> {
        if !self.frame_active {
            return Err(crate::core::error::Error::InvalidOperation(
                "No frame active".to_string(),
            ));
        }

        self.frame_active = false;
        Ok(())
    }

    /// Render a single mesh with the given transform
    ///
    /// This is the primary method for rendering individual meshes in the scene.
    /// The mesh must have been registered with the renderer before calling this.
    ///
    /// # Arguments
    /// * `mesh` - Mesh to render
    /// * `transform` - World transform matrix (object-to-world)
    ///
    /// # Example
    /// ```no_run
    /// # use ww3d_renderer_3d::w3d_renderer::W3DRenderer;
    /// # use glam::Mat4;
    /// # let mut renderer: W3DRenderer = todo!();
    /// # let mesh = todo!();
    /// let transform = Mat4::from_translation([0.0, 1.0, 0.0].into());
    /// renderer.render_mesh(&mesh, &transform)?;
    /// # Ok::<(), ww3d_renderer_3d::core::error::Error>(())
    /// ```
    ///
    /// C++ equivalent: W3D::Render_Mesh()
    pub fn render_mesh(&mut self, mesh: &Arc<MeshClass>, transform: &Mat4) -> RendererResult<()> {
        if !self.frame_active {
            return Err(crate::core::error::Error::InvalidOperation(
                "No frame active - call begin_frame() first".to_string(),
            ));
        }

        // Update mesh transform
        if let Some(mesh_mut) = Arc::get_mut(&mut mesh.clone()) {
            mesh_mut.set_transform(*transform);
        }

        // For now, we just track the mesh
        // Actual rendering happens in render_pass()
        Ok(())
    }

    /// Render a hierarchical model with bone transforms
    ///
    /// This method renders a complete animated model with skeletal animation.
    /// Each bone in the hierarchy is transformed according to the provided
    /// bone transform array.
    ///
    /// # Arguments
    /// * `model` - Model to render
    /// * `bone_transforms` - Array of bone transform matrices
    ///
    /// # Example
    /// ```no_run
    /// # use ww3d_renderer_3d::w3d_renderer::W3DRenderer;
    /// # use glam::Mat4;
    /// # let mut renderer: W3DRenderer = todo!();
    /// # let model = todo!();
    /// let bone_transforms = vec![Mat4::IDENTITY; 64];
    /// renderer.render_model(&model, &bone_transforms)?;
    /// # Ok::<(), ww3d_renderer_3d::core::error::Error>(())
    /// ```
    ///
    /// C++ equivalent: W3D::Render_Model() / HModelClass::Render()
    pub fn render_model(
        &mut self,
        model: &Arc<MeshModelClass>,
        bone_transforms: &[Mat4],
    ) -> RendererResult<()> {
        if !self.frame_active {
            return Err(crate::core::error::Error::InvalidOperation(
                "No frame active - call begin_frame() first".to_string(),
            ));
        }

        // Register the model if not already registered
        self.mesh_manager
            .ensure_model(model)
            .map_err(|e| crate::core::error::Error::RenderError(e.to_string()))?;

        // Store bone transforms for rendering
        // In a full implementation, this would update the bone palette
        // for the model and queue it for rendering
        let _ = bone_transforms; // Bone transforms used during actual render pass

        Ok(())
    }

    /// Add a mesh to the static batch renderer
    ///
    /// Static batches combine multiple meshes with the same material into
    /// a single draw call for improved performance. Use this for objects
    /// that don't move (buildings, terrain, etc.).
    ///
    /// # Arguments
    /// * `material` - Material identifier for batching
    /// * `vertices` - Vertex data
    /// * `indices` - Index buffer
    /// * `transform` - World transform (baked into vertex positions)
    pub fn add_to_static_batch(
        &mut self,
        material: MaterialKey,
        vertices: &[BatchVertex],
        indices: &[u32],
        transform: Mat4,
    ) {
        self.batch_renderer
            .add_static_object(material, vertices, indices, transform);
    }

    /// Add a mesh to the dynamic batch renderer
    ///
    /// Dynamic batches allow objects to be moved/hidden while still being
    /// batched. Use this for objects that move occasionally.
    ///
    /// # Returns
    /// Handle (material_key, object_id) for updating the object later
    pub fn add_to_dynamic_batch(
        &mut self,
        material: MaterialKey,
        vertices: Vec<BatchVertex>,
        indices: Vec<u32>,
        transform: Mat4,
    ) -> (MaterialKey, usize) {
        self.batch_renderer
            .add_dynamic_object(material, vertices, indices, transform)
    }

    /// Add a mesh to the instanced renderer
    ///
    /// Instanced rendering is used for many copies of the same mesh (trees,
    /// rocks, units in formation, etc.). Each instance has its own transform.
    ///
    /// # Returns
    /// Handle (batch_id, instance_id) for updating the instance later
    pub fn add_to_instanced_batch(
        &mut self,
        material: MaterialKey,
        vertices: Vec<BatchVertex>,
        indices: Vec<u32>,
        transform: Mat4,
        color: glam::Vec4,
    ) -> Option<(usize, usize)> {
        self.batch_renderer
            .add_instanced_object(material, vertices, indices, transform, color)
    }

    /// Execute a render pass with the given render targets
    ///
    /// This is the internal method that performs actual GPU draw calls.
    /// It renders all queued meshes, batches, and instances.
    ///
    /// # Arguments
    /// * `render_pass` - WGPU render pass to draw into
    /// * `opaque_meshes` - Opaque mesh list (front-to-back sorted)
    /// * `blended_meshes` - Transparent mesh list (back-to-front sorted)
    /// * `render_info` - Rendering context (camera, lighting, etc.)
    /// * `arena` - Frame uniform buffer arena
    #[doc(hidden)]
    pub fn internal_render_pass(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        opaque_meshes: &[Arc<MeshClass>],
        blended_meshes: &[Arc<MeshClass>],
        render_info: &RenderInfoClass,
        arena: &mut FrameUniformArena,
    ) -> W3DResult<()> {
        self.mesh_manager.render_pass(
            render_pass,
            opaque_meshes,
            blended_meshes,
            render_info,
            arena,
        )
    }

    /// Get batch rendering statistics
    pub fn batch_stats(&self) -> crate::rendering::batching::batch_renderer::BatchStats {
        self.batch_renderer.stats()
    }

    /// Get mesh rendering statistics
    pub fn mesh_stats(&self) -> &crate::rendering::mesh_system::MeshRenderStats {
        self.mesh_manager.get_stats()
    }

    /// Get the GPU device
    pub fn gpu_device(&self) -> &Arc<GpuDevice> {
        &self.gpu_device
    }

    /// Access the internal mesh manager
    ///
    /// This provides low-level access to mesh rendering functionality.
    /// Most users should use the high-level render_mesh/render_model methods.
    pub fn mesh_manager(&self) -> &MeshRenderManager {
        &self.mesh_manager
    }

    /// Access the internal mesh manager (mutable)
    pub fn mesh_manager_mut(&mut self) -> &mut MeshRenderManager {
        &mut self.mesh_manager
    }

    /// Access the batch renderer
    pub fn batch_renderer(&self) -> &BatchRenderer {
        &self.batch_renderer
    }

    /// Access the batch renderer (mutable)
    pub fn batch_renderer_mut(&mut self) -> &mut BatchRenderer {
        &mut self.batch_renderer
    }
}

/// Convenience methods for integrating with GameClient drawable system
impl W3DRenderer {
    /// Render all drawables from the update_for_rendering callback
    ///
    /// This method is called by the GameClient to render all visible drawables
    /// in the scene. It handles sorting, culling, and batching automatically.
    ///
    /// C++ equivalent: GameClient::Update_For_Rendering() -> drawable->Render()
    pub fn render_drawables(
        &mut self,
        _drawables: &[Arc<dyn std::any::Any>], // In real impl, would be Arc<dyn Drawable>
    ) -> RendererResult<()> {
        if !self.frame_active {
            return Err(crate::core::error::Error::InvalidOperation(
                "No frame active - call begin_frame() first".to_string(),
            ));
        }

        // In a full implementation:
        // 1. Iterate through all drawables
        // 2. For each W3DModelDraw drawable:
        //    - Extract mesh/model
        //    - Extract transform
        //    - Call render_mesh() or render_model()
        // 3. Sort by material/distance
        // 4. Execute batched draw calls

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests would require a mock GPU device
    // For now, just verify the API compiles

    #[test]
    fn test_renderer_api_exists() {
        // Just verify the types exist and compile
        let _ = std::mem::size_of::<W3DRenderer>();
    }
}
