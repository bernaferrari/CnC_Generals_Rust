//! 3D viewport implementation for game development tools

use crate::UIError;
use anyhow::Result;
use eframe::egui;
use glam::{Mat4, Quat, Vec3};
use parking_lot::RwLock;
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// 3D viewport for real-time scene editing and preview
pub struct Viewport3D {
    render_context: Option<RenderContext>,
    camera: Camera,
    scene: Scene,
    input_state: InputState,
    viewport_size: [u32; 2],
    is_active: bool,
}

impl Viewport3D {
    pub fn new() -> Self {
        Self {
            render_context: None,
            camera: Camera::new(),
            scene: Scene::new(),
            input_state: InputState::new(),
            viewport_size: [800, 600],
            is_active: false,
        }
    }

    /// Initialize the 3D rendering context
    pub async fn initialize(
        &mut self,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
    ) -> Result<()> {
        self.render_context = Some(RenderContext::new(device, queue).await?);
        Ok(())
    }

    /// Update the viewport (called every frame)
    pub fn update(&mut self, ui: &mut egui::Ui) -> Result<()> {
        let available_size = ui.available_size();
        let new_size = [available_size.x as u32, available_size.y as u32];

        if new_size != self.viewport_size {
            self.viewport_size = new_size;
            self.resize_viewport(new_size[0], new_size[1])?;
        }

        // Handle input
        self.handle_input(ui)?;

        // Render the 3D scene
        self.render()?;

        // Display the rendered frame in egui
        if let Some(texture_id) = self.get_egui_texture_id() {
            let response = ui.image((texture_id, available_size));

            // Track mouse interaction
            if response.hovered() {
                self.is_active = true;
            } else {
                self.is_active = false;
            }
        } else {
            // Fallback: show a placeholder
            ui.centered_and_justified(|ui| {
                ui.label("3D Viewport (Initializing...)");
            });
        }

        Ok(())
    }

    /// Add an object to the scene
    pub fn add_object(&mut self, object: SceneObject) {
        self.scene.add_object(object);
    }

    /// Remove an object from the scene
    pub fn remove_object(&mut self, id: u32) {
        self.scene.remove_object(id);
    }

    /// Get camera reference for external manipulation
    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    /// Set camera position and target
    pub fn set_camera(&mut self, position: Vec3, target: Vec3) {
        self.camera.set_position(position);
        self.camera.set_target(target);
    }

    fn handle_input(&mut self, ui: &mut egui::Ui) -> Result<()> {
        if !self.is_active {
            return Ok(());
        }

        let ctx = ui.ctx();

        // Camera controls
        if ctx.input(|i| i.key_down(egui::Key::W)) {
            self.camera.move_forward(0.1);
        }
        if ctx.input(|i| i.key_down(egui::Key::S)) {
            self.camera.move_backward(0.1);
        }
        if ctx.input(|i| i.key_down(egui::Key::A)) {
            self.camera.strafe_left(0.1);
        }
        if ctx.input(|i| i.key_down(egui::Key::D)) {
            self.camera.strafe_right(0.1);
        }

        // Mouse look
        if ctx.input(|i| i.pointer.button_down(egui::PointerButton::Middle)) {
            let delta = ctx.input(|i| i.pointer.delta());
            if delta != egui::Vec2::ZERO {
                self.camera.rotate(delta.x * 0.005, delta.y * 0.005);
            }
        }

        // Mouse zoom
        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 {
            self.camera.zoom(scroll_delta * 0.01);
        }

        Ok(())
    }

    fn render(&mut self) -> Result<()> {
        if let Some(ctx) = &mut self.render_context {
            ctx.render(&self.camera, &self.scene, self.viewport_size)?;
        }
        Ok(())
    }

    fn resize_viewport(&mut self, width: u32, height: u32) -> Result<()> {
        if let Some(ctx) = &mut self.render_context {
            ctx.resize(width, height)?;
        }
        self.camera.set_aspect_ratio(width as f32 / height as f32);
        Ok(())
    }

    fn get_egui_texture_id(&self) -> Option<egui::TextureId> {
        self.render_context.as_ref()?.get_egui_texture_id()
    }
}

/// 3D camera for viewport navigation
pub struct Camera {
    position: Vec3,
    target: Vec3,
    up: Vec3,
    fov: f32,
    aspect_ratio: f32,
    near: f32,
    far: f32,
    view_matrix: Mat4,
    projection_matrix: Mat4,
}

impl Camera {
    pub fn new() -> Self {
        let mut camera = Self {
            position: Vec3::new(0.0, 5.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov: 45.0_f32.to_radians(),
            aspect_ratio: 16.0 / 9.0,
            near: 0.1,
            far: 1000.0,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
        };
        camera.update_matrices();
        camera
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
        self.update_view_matrix();
    }

    pub fn set_target(&mut self, target: Vec3) {
        self.target = target;
        self.update_view_matrix();
    }

    pub fn set_aspect_ratio(&mut self, aspect: f32) {
        self.aspect_ratio = aspect;
        self.update_projection_matrix();
    }

    pub fn move_forward(&mut self, distance: f32) {
        let forward = (self.target - self.position).normalize();
        self.position += forward * distance;
        self.target += forward * distance;
        self.update_view_matrix();
    }

    pub fn move_backward(&mut self, distance: f32) {
        self.move_forward(-distance);
    }

    pub fn strafe_left(&mut self, distance: f32) {
        let forward = (self.target - self.position).normalize();
        let right = forward.cross(self.up).normalize();
        self.position -= right * distance;
        self.target -= right * distance;
        self.update_view_matrix();
    }

    pub fn strafe_right(&mut self, distance: f32) {
        self.strafe_left(-distance);
    }

    pub fn rotate(&mut self, yaw: f32, pitch: f32) {
        let forward = self.target - self.position;
        let distance = forward.length();

        let rotation = Quat::from_axis_angle(self.up, yaw) * Quat::from_axis_angle(Vec3::X, pitch);

        let new_forward = rotation * forward.normalize();
        self.target = self.position + new_forward * distance;
        self.update_view_matrix();
    }

    pub fn zoom(&mut self, delta: f32) {
        let forward = (self.target - self.position).normalize();
        let new_distance = (self.target - self.position).length() + delta;
        let clamped_distance = new_distance.clamp(1.0, 100.0);

        self.position = self.target - forward * clamped_distance;
        self.update_view_matrix();
    }

    pub fn view_matrix(&self) -> &Mat4 {
        &self.view_matrix
    }

    pub fn projection_matrix(&self) -> &Mat4 {
        &self.projection_matrix
    }

    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix * self.view_matrix
    }

    fn update_matrices(&mut self) {
        self.update_view_matrix();
        self.update_projection_matrix();
    }

    fn update_view_matrix(&mut self) {
        self.view_matrix = Mat4::look_at_rh(self.position, self.target, self.up);
    }

    fn update_projection_matrix(&mut self) {
        self.projection_matrix =
            Mat4::perspective_rh(self.fov, self.aspect_ratio, self.near, self.far);
    }
}

/// 3D scene containing objects to render
pub struct Scene {
    objects: Vec<SceneObject>,
    lights: Vec<Light>,
    grid: GridRenderer,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            lights: vec![Light::default()],
            grid: GridRenderer::new(),
        }
    }

    pub fn add_object(&mut self, object: SceneObject) {
        self.objects.push(object);
    }

    pub fn remove_object(&mut self, id: u32) {
        self.objects.retain(|obj| obj.id != id);
    }

    pub fn objects(&self) -> &[SceneObject] {
        &self.objects
    }

    pub fn lights(&self) -> &[Light] {
        &self.lights
    }

    pub fn grid(&self) -> &GridRenderer {
        &self.grid
    }
}

/// Object in the 3D scene
#[derive(Debug, Clone)]
pub struct SceneObject {
    pub id: u32,
    pub name: String,
    pub transform: Transform,
    pub mesh: Option<MeshHandle>,
    pub material: Option<MaterialHandle>,
    pub visible: bool,
}

impl SceneObject {
    pub fn new(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            transform: Transform::default(),
            mesh: None,
            material: None,
            visible: true,
        }
    }
}

/// 3D transformation
#[derive(Debug, Clone)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

/// Light in the scene
#[derive(Debug, Clone)]
pub struct Light {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub light_type: LightType,
}

impl Default for Light {
    fn default() -> Self {
        Self {
            position: Vec3::new(5.0, 10.0, 5.0),
            color: Vec3::ONE,
            intensity: 1.0,
            light_type: LightType::Directional,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LightType {
    Directional,
    Point,
    Spot,
}

/// Grid renderer for scene reference
pub struct GridRenderer {
    visible: bool,
    size: f32,
    divisions: u32,
}

impl GridRenderer {
    pub fn new() -> Self {
        Self {
            visible: true,
            size: 100.0,
            divisions: 100,
        }
    }
}

/// Handle to a mesh resource
#[derive(Debug, Clone, Copy)]
pub struct MeshHandle(pub u32);

/// Handle to a material resource
#[derive(Debug, Clone, Copy)]
pub struct MaterialHandle(pub u32);

/// Input state tracking
struct InputState {
    mouse_delta: egui::Vec2,
    keys_pressed: std::collections::HashSet<egui::Key>,
}

impl InputState {
    fn new() -> Self {
        Self {
            mouse_delta: egui::Vec2::ZERO,
            keys_pressed: std::collections::HashSet::new(),
        }
    }
}

/// Rendering context for the 3D viewport
struct RenderContext {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface_texture: Option<wgpu::Texture>,
    surface_view: Option<wgpu::TextureView>,
}

impl RenderContext {
    async fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Result<Self> {
        Ok(Self {
            device,
            queue,
            surface_texture: None,
            surface_view: None,
        })
    }

    fn render(&mut self, camera: &Camera, scene: &Scene, size: [u32; 2]) -> Result<()> {
        // TODO: Implement actual 3D rendering
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        // TODO: Implement viewport resizing
        Ok(())
    }

    fn get_egui_texture_id(&self) -> Option<egui::TextureId> {
        // TODO: Return actual texture ID for egui
        None
    }
}
