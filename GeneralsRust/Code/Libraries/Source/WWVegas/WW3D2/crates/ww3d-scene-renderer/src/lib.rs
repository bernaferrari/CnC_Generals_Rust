//! Scene Integration Module
//!
//! This crate provides integration between ww3d-scene and ww3d-renderer-3d,
//! enabling scene objects to be rendered through the rendering pipeline.

use glam::Mat4;
use std::sync::Arc;
use ww3d_core::errors::W3DResult;
use ww3d_renderer_3d::render_object_system::RenderInfoClass as RendererRenderInfoClass;
use ww3d_renderer_3d::rendering::mesh_system::MeshClass;
use ww3d_renderer_3d::Renderer;
use ww3d_scene::RenderInfoClass as SceneRenderInfoClass;

/// Bridge between ww3d-scene RenderObj and ww3d-renderer-3d rendering.
pub struct SceneRenderBridge {
    /// Cached mesh instances for scene objects.
    mesh_cache: std::collections::HashMap<String, Arc<MeshClass>>,
}

impl SceneRenderBridge {
    /// Create a new scene render bridge.
    pub fn new() -> Self {
        Self {
            mesh_cache: std::collections::HashMap::new(),
        }
    }

    /// Render a scene object through the renderer.
    pub fn render_scene_object(
        &mut self,
        renderer: &mut Renderer,
        obj_name: &str,
        transform: &Mat4,
        mesh: Option<Arc<MeshClass>>,
    ) -> W3DResult<()> {
        // Get or create mesh instance.
        let mesh_instance = if let Some(m) = mesh {
            m
        } else if let Some(cached) = self.mesh_cache.get(obj_name) {
            Arc::clone(cached)
        } else {
            // No mesh available.
            return Ok(());
        };

        // Create a transformed instance of the mesh.
        let mut instance = (*mesh_instance).clone();
        instance.transform = *transform;

        // Queue the mesh for rendering.
        renderer.queue_mesh(Arc::new(instance))?;

        Ok(())
    }

    /// Register a mesh for a scene object.
    pub fn register_mesh(&mut self, obj_name: String, mesh: Arc<MeshClass>) {
        self.mesh_cache.insert(obj_name, mesh);
    }

    /// Unregister a mesh.
    pub fn unregister_mesh(&mut self, obj_name: &str) -> bool {
        self.mesh_cache.remove(obj_name).is_some()
    }

    /// Clear all cached meshes.
    pub fn clear(&mut self) {
        self.mesh_cache.clear();
    }
}

impl Default for SceneRenderBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert ww3d-scene camera to renderer camera.
pub fn convert_scene_camera_to_renderer(
    scene_camera: &ww3d_scene::CameraClass,
) -> ww3d_renderer_3d::CameraClass {
    let mut renderer_camera = ww3d_renderer_3d::CameraClass::new();

    // Copy camera properties.
    renderer_camera.set_position(scene_camera.position);
    renderer_camera.set_view_matrix(scene_camera.view_matrix);
    renderer_camera.set_projection_matrix(scene_camera.projection_matrix);

    renderer_camera
}

/// Convert renderer camera to ww3d-scene camera.
pub fn convert_renderer_camera_to_scene(
    renderer_camera: &mut ww3d_renderer_3d::CameraClass,
) -> ww3d_scene::CameraClass {
    let mut scene_camera = ww3d_scene::CameraClass::new();

    // Copy camera properties.
    scene_camera.set_position(renderer_camera.get_position());
    scene_camera.set_view_matrix(renderer_camera.get_view_matrix());
    scene_camera.set_projection_matrix(renderer_camera.get_projection_matrix());

    scene_camera
}

/// Adapter to make ww3d-scene RenderInfoClass compatible with renderer RenderInfoClass.
pub fn convert_scene_render_info_to_renderer(
    scene_info: &SceneRenderInfoClass,
) -> RendererRenderInfoClass {
    let camera = convert_scene_camera_to_renderer(&scene_info.camera);
    let mut renderer_info = RendererRenderInfoClass::new(Arc::new(camera));

    // Copy rendering parameters.
    renderer_info.additional_alpha_multiplier = scene_info.alpha_override;

    renderer_info
}

/// Helper to render a complete ww3d-scene::SceneClass through the renderer.
pub struct SceneRenderer {
    bridge: SceneRenderBridge,
}

impl SceneRenderer {
    /// Create a new scene renderer.
    pub fn new() -> Self {
        Self {
            bridge: SceneRenderBridge::new(),
        }
    }

    /// Render an entire scene.
    pub fn render_scene(
        &mut self,
        scene: &ww3d_scene::SceneClass,
        renderer: &mut Renderer,
    ) -> W3DResult<()> {
        // Set up camera.
        let camera = convert_scene_camera_to_renderer(
            &scene
                .objects
                .first()
                .map(|_| {
                    // In a real implementation, we'd get the camera from the scene or render info.
                    ww3d_scene::CameraClass::new()
                })
                .unwrap_or_else(ww3d_scene::CameraClass::new),
        );

        renderer.set_camera(camera);

        // Render each object in the scene.
        for obj in &scene.objects {
            // Check visibility.
            let camera_pos = glam::Vec3::ZERO; // Would come from actual camera.
            if !obj.is_visible(camera_pos) {
                continue;
            }

            // Get object properties.
            let name = obj.get_name();
            let transform = obj.get_transform();

            // Render the object.
            // Note: In a real implementation, we'd need to extract mesh data from the RenderObj.
            // This is simplified for demonstration.
            self.bridge
                .render_scene_object(renderer, name, transform, None)?;
        }

        Ok(())
    }

    /// Get the underlying bridge.
    pub fn bridge_mut(&mut self) -> &mut SceneRenderBridge {
        &mut self.bridge
    }
}

impl Default for SceneRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_render_bridge_creation() {
        let bridge = SceneRenderBridge::new();
        assert_eq!(bridge.mesh_cache.len(), 0);
    }

    #[test]
    fn test_camera_conversion() {
        let scene_camera = ww3d_scene::CameraClass::new();
        let mut renderer_camera = convert_scene_camera_to_renderer(&scene_camera);
        let back_to_scene = convert_renderer_camera_to_scene(&mut renderer_camera);

        // Verify positions match.
        assert_eq!(scene_camera.position, back_to_scene.position);
    }
}
