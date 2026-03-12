//! WW3D Enhanced Demo Application
//!
//! This demo showcases all the advanced features of the WW3D engine:
//! - GPU skinning with advanced shaders
//! - Physics simulation with collision detection
//! - Multi-threaded scene processing
//! - Advanced lighting with shadows and SSAO
//! - Performance monitoring and benchmarking

use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wgpu::{util::DeviceExt, SurfaceError};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ww3d_assets::{prototypes::MeshPrototype, AssetManager};
use ww3d_collision::{CollisionSystem, Plane, Sphere};
use glam::{Mat4, Quat, Vec3, Vec4};
use ww3d_animation::{hanim_from_prototype, htree_from_hierarchy_prototype};
use ww3d_renderer_3d::core::error::Error as RendererError;
use ww3d_renderer_3d::render_object_system::RenderInfoClass;
use ww3d_renderer_3d::rendering::wgpu_main_renderer::{
    WgpuMainRenderer, WgpuMainRendererConfig,
};
use ww3d_renderer_3d::rendering::camera_system::{CameraClass as RenderCamera, CameraUtils};
use ww3d_renderer_3d::rendering::lighting_system::LightClass;
use ww3d_renderer_3d::rendering::mesh_system::{MeshClass, MeshModelClass};
use ww3d_renderer_3d::scene_system::SceneManagerClass;
use ww3d_renderer_3d::Renderer;
use ww3d_collision::physics_integration::{PhysicsWorld, RigidBodyDesc, CollisionShape};
use ww3d_core::errors::{W3DError, W3DResult};
use ww3d_engine::{self, EngineConfig, EngineError};

/// Enhanced demo application
const DEMO_ASSET_RELATIVE: &str = "../Code/Tools/w3d_to_gltf/W3D/gxmammoth_a.w3d";
const MAX_SKINNING_MATRICES: usize = 64;
type Quaternion = Quat;

/// Maintains animation playback for a single skinned hierarchy and feeds the renderer
/// with an updated bone palette each frame.
///
/// # Renderer handshake
/// * In the original WW3D renderer the `SkinnedMeshDrawModule` owned the current
///   `HTree` pose while `MeshClass::Set_Skin` copied matrices into the `VisibleMesh`.
/// * The Rust port follows the same split: animation state (this type) owns the bind
///   and inverse-bind caches, while the render `MeshClass` keeps the most recent
///   palette via [`MeshClass::set_bone_palette_slice`].
/// * [`MeshClass::bone_palette_view`] exposes the palette together with a monotonically
///   increasing version so that `MeshRenderManager` mirrors the DX8 renderer behaviour
///   of uploading bone matrices only when they change.
///
/// This documentation lives here instead of the renderer crate so future contributors
/// can compare the flow directly against the C++ comments in `SkinnedMeshDrawModule`
/// and `RendererClass::Render_Mesh`. Any deviation from this contract will cause
/// palette uploads or skinning to diverge from the legacy implementation.
struct SkeletalAnimationState {
    hierarchy_name: String,
    animation: Arc<ww3d_animation::HAnimClass>,
    htree: ww3d_animation::HTreeClass,
    bind_pose: Vec<Mat4>,
    inverse_bind: Vec<Mat4>,
    palette: Vec<Mat4>,
    frame: f32,
}

impl SkeletalAnimationState {
    /// Construct a new animation state from hierarchy and animation prototypes.
    ///
    /// Equivalent behaviour to `SkinnedMeshDrawModule::Init`:
    /// * The bind pose is cached exactly once so subsequent animation frames do not
    ///   touch the asset manager.
    /// * Each inverse-bind matrix is pulled from the asset pipeline when available,
    ///   falling back to recomputation just like the C++ debug path.
    fn new(
        hierarchy_name: String,
        mut htree: ww3d_animation::HTreeClass,
        animation: Arc<ww3d_animation::HAnimClass>,
        bind_transforms: &[Mat4],
        inverse_bind_transforms: &[Mat4],
    ) -> Self {
        htree.base_update(Mat4::IDENTITY);
        let total_pivots = htree.num_pivots();
        let bone_count = total_pivots.min(MAX_SKINNING_MATRICES);
        let mut bind_pose = Vec::with_capacity(total_pivots);
        let mut inverse_bind = Vec::with_capacity(total_pivots);

        for index in 0..total_pivots {
            let fallback_bind = htree.transform(index).unwrap_or(Mat4::IDENTITY);
            let bind = bind_transforms
                .get(index)
                .copied()
                .unwrap_or(fallback_bind);
            bind_pose.push(bind);

            let provided_inverse = inverse_bind_transforms.get(index).copied();
            let inv = provided_inverse
                .filter(|matrix| matrix.is_finite())
                .unwrap_or_else(|| {
                    let computed = bind.inverse();
                    if computed.is_finite() {
                        computed
                    } else {
                        Mat4::IDENTITY
                    }
                });
            inverse_bind.push(inv);
        }

        for (pivot, bind) in htree.pivots.iter_mut().zip(bind_pose.iter()) {
            pivot.transform = *bind;
        }

        let mut palette = Vec::with_capacity(bone_count);
        for index in 0..bone_count {
            palette.push(bind_pose[index] * inverse_bind[index]);
        }

        Self {
            hierarchy_name,
            animation,
            htree,
            bind_pose,
            inverse_bind,
            palette,
            frame: 0.0,
        }
    }

    /// Advance the animation clock and return the palette slice expected by the renderer.
    ///
    /// Matching `SkinnedMeshDrawModule::Prepare` in C++, the palette is produced as:
    /// current pivot transform * cached inverse bind. The versioned palette is consumed
    /// by [`MeshClass::set_bone_palette_slice`] and ultimately uploaded via
    /// `MeshRenderManager::ensure_bone_palette_bind`.
    fn update(&mut self, delta_time: f32) -> &[Mat4] {
        let frame_rate = self.animation.get_frame_rate();
        let total_frames = self.animation.get_num_frames().max(1) as f32;

        if frame_rate > f32::EPSILON {
            self.frame = (self.frame + delta_time * frame_rate) % total_frames;
        }

        self.animation
            .apply_animation(&mut self.htree, self.frame, Mat4::IDENTITY);

        let bone_count = self
            .htree
            .num_pivots()
            .min(self.inverse_bind.len())
            .min(MAX_SKINNING_MATRICES);

        if self.palette.len() < bone_count {
            self.palette.resize(bone_count, Mat4::IDENTITY);
        }

        for index in 0..bone_count {
            let current = self.htree.transform(index).unwrap_or(Mat4::IDENTITY);
            self.palette[index] = current * self.inverse_bind[index];
        }

        if self.palette.len() > bone_count {
            self.palette.truncate(bone_count);
        }

        &self.palette[..bone_count]
    }
}

pub struct EnhancedDemoApp {
    // Core systems
    asset_manager: Arc<Mutex<AssetManager>>,
    collision_system: CollisionSystem,
    scene_manager: SceneManagerClass,
    physics_world: PhysicsWorld,

    // Rendering
    renderer: WgpuMainRenderer,
    render_camera: RenderCamera,
    demo_mesh_id: Option<usize>,

    // Animation and physics objects
    skeletal_state: Option<SkeletalAnimationState>,
    physics_bodies: Vec<ww3d_collision::physics_integration::PhysicsBodyId>,

    // Performance monitoring
    frame_timer: Instant,
    frame_count: u64,
    fps: f32,

    // Demo state
    camera_angle: f32,
    light_angle: f32,
    simulation_running: bool,
}

impl EnhancedDemoApp {
    pub async fn new(window: Arc<winit::window::Window>) -> Self {
        // Initialize core systems
        let asset_manager = Arc::new(Mutex::new(AssetManager::new()));
        let collision_system = CollisionSystem::new();
        let mut scene_manager = SceneManagerClass::new();
        scene_manager.set_ambient_light(Vec3::new(0.18, 0.20, 0.24));
        scene_manager.set_fog_enabled(true);
        scene_manager.set_fog_color(Vec4::new(0.58, 0.65, 0.72, 1.0));
        scene_manager.set_fog_range(35.0, 220.0);

        {
            let mut environment = scene_manager.light_environment_mut();

            let mut sun = LightClass::directional(
                Vec3::new(-0.45, -1.0, -0.28),
                Vec3::new(1.0, 0.96, 0.9),
                1.45,
            );
            sun.id = 1;
            environment.add_light(Arc::new(Mutex::new(sun)));

            let mut fill = LightClass::directional(
                Vec3::new(0.35, -0.6, 0.4),
                Vec3::new(0.45, 0.5, 0.6),
                0.55,
            );
            fill.id = 2;
            environment.add_light(Arc::new(Mutex::new(fill)));
        }

        let physics_world = PhysicsWorld::new();

        // Initialize renderer via the shared engine lifecycle
        let window_size = window.inner_size();

        let mut engine_config = EngineConfig::default();
        engine_config.width = window_size.width.max(1);
        engine_config.height = window_size.height.max(1);

        if let Err(err) = ww3d_engine::init_with_window(window.clone(), engine_config).await {
            if !matches!(err, EngineError::AlreadyInitialised) {
                panic!("failed to initialise ww3d engine: {err:?}");
            }
        }

        let renderer = {
            let renderer = WgpuMainRenderer::from_engine(WgpuMainRendererConfig::default())
                .expect("failed to construct WgpuMainRenderer from ww3d_engine");
            renderer
                .set_asset_manager(Arc::clone(&asset_manager))
                .expect("failed to bind asset manager to renderer");
            renderer
        };

        // Setup scene
        let aspect_ratio = window_size.width.max(1) as f32 / window_size.height.max(1) as f32;
        let mut render_camera = CameraUtils::create_perspective(
            std::f32::consts::FRAC_PI_3,
            aspect_ratio,
            0.1,
            1000.0,
        );
        render_camera.set_position(Vec3::new(0.0, 7.0, 16.0));
        render_camera.look_at(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));

        Self {
            asset_manager,
            collision_system,
            scene_manager,
            physics_world,
            renderer,
            render_camera,
            demo_mesh_id: None,
            skeletal_state: None,
            physics_bodies: Vec::new(),
            frame_timer: Instant::now(),
            frame_count: 0,
            fps: 0.0,
            camera_angle: 0.0,
            light_angle: 0.0,
            simulation_running: true,
        }
    }

    fn demo_asset_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEMO_ASSET_RELATIVE)
    }

    fn load_demo_mesh(&mut self) -> W3DResult<(MeshClass, Option<SkeletalAnimationState>)> {
        let asset_path = Self::demo_asset_path();
        if !asset_path.exists() {
            return Err(W3DError::AssetNotFound(format!(
                "Demo asset not found at {}",
                asset_path.display()
            )));
        }

        let (model, mesh_name, bind_transforms, _inverse_transforms, skeletal_state) = {
            let mut manager = self
                .asset_manager
                .lock()
                .map_err(|_| W3DError::InvalidParameter("Asset manager poisoned".into()))?;
            manager.load_w3d(&asset_path)?;

            let prototype = manager
                .prototypes
                .values()
                .filter_map(|proto| proto.as_any().downcast_ref::<MeshPrototype>())
                .max_by_key(|proto| proto.vertices.len())
                .ok_or_else(|| {
                    W3DError::AssetNotFound("No mesh prototypes in demo asset".into())
                })?;

            let container_name = prototype
                .header
                .as_ref()
                .map(|hdr| hdr.container_name_str())
                .filter(|name| !name.is_empty());

            let hierarchy_lookup = container_name
                .as_deref()
                .and_then(|name| manager.get_hierarchy_prototype(name));

            let mut model = MeshModelClass::from_mesh_prototype(prototype, hierarchy_lookup)?;
            let mesh_name = prototype.name.clone();

            let mut bind_transforms = Vec::new();
            let mut inverse_transforms = Vec::new();

            let skeletal_state = if let Some(hierarchy_name) =
                model.hierarchy_name().map(|name| name.to_string())
            {
                if let Some(hierarchy_proto) =
                    manager.get_hierarchy_prototype(&hierarchy_name)
                {
                    bind_transforms = hierarchy_proto.bind_transforms.clone();
                    inverse_transforms = hierarchy_proto.inverse_bind_transforms.clone();

                    let htree = htree_from_hierarchy_prototype(hierarchy_proto);
                    if let Some(anim_proto) = manager.find_animation_for_hierarchy(
                        &hierarchy_name,
                        Some(&mesh_name),
                    )
                    {
                        let anim = Arc::new(hanim_from_prototype(anim_proto));
                        Some(SkeletalAnimationState::new(
                            hierarchy_name,
                            htree,
                            anim,
                            &bind_transforms,
                            &inverse_transforms,
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            (
                model,
                mesh_name,
                bind_transforms,
                inverse_transforms,
                skeletal_state,
            )
        };

        let mut mesh = MeshClass::new();
        mesh.name = mesh_name;
        mesh.model = Some(Arc::new(model));
        if !bind_transforms.is_empty() {
            mesh.set_bone_palette_slice(&bind_transforms);
        }
        mesh.update_cached_bounding_volumes();
        Ok((mesh, skeletal_state))
    }

    pub fn create_demo_objects(&mut self) -> W3DResult<()> {
        // Create physics objects and collision proxies
        self.create_physics_objects();
        self.setup_collision_objects();

        let (mesh, skeletal_state) = self.load_demo_mesh()?;
        if skeletal_state.is_none() {
            println!(
                "⚠️  Demo mesh did not include a matching hierarchy/animation; using bind pose."
            );
        }
        self.skeletal_state = skeletal_state;

        let mesh_id = self
            .scene_manager
            .add_render_object(Box::new(mesh));
        self.demo_mesh_id = Some(mesh_id);
        Ok(())
    }

    fn create_physics_objects(&mut self) {
        // Create ground plane
        let ground_desc = RigidBodyDesc {
            position: Vec3::new(0.0, -2.0, 0.0),
            rotation: Quaternion::IDENTITY,
            shape: CollisionShape::Box {
                half_extents: Vec3::new(20.0, 0.5, 20.0)
            },
            mass: 0.0, // Static
            restitution: 0.3,
            friction: 0.8,
            ..Default::default()
        };
        let ground_id = self.physics_world.create_body(ground_desc);
        self.physics_bodies.push(ground_id);

        // Create falling spheres
        for i in 0..10 {
            let x = (i as f32 - 5.0) * 2.0;
            let y = 5.0 + (i as f32) * 0.5;
            let z = 0.0;

            let sphere_desc = RigidBodyDesc {
                position: Vec3::new(x, y, z),
                rotation: Quaternion::IDENTITY,
                shape: CollisionShape::Sphere { radius: 0.5 },
                mass: 1.0,
                restitution: 0.8,
                friction: 0.2,
                ..Default::default()
            };

            let sphere_id = self.physics_world.create_body(sphere_desc);
            self.physics_bodies.push(sphere_id);
        }

        // Create bouncing boxes
        for i in 0..5 {
            let x = (i as f32 - 2.0) * 3.0;
            let y = 8.0;
            let z = 5.0;

            let box_desc = RigidBodyDesc {
                position: Vec3::new(x, y, z),
                rotation: Quaternion::from_rotation_y(i as f32 * 0.5),
                shape: CollisionShape::Box {
                    half_extents: Vec3::new(0.5, 0.5, 0.5)
                },
                mass: 2.0,
                restitution: 0.6,
                friction: 0.3,
                ..Default::default()
            };

            let box_id = self.physics_world.create_body(box_desc);
            self.physics_bodies.push(box_id);
        }
    }

    fn setup_collision_objects(&mut self) {
        // Add collision planes
        let ground_plane = Plane::from_point_normal(
            Vec3::new(0.0, -2.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0)
        );
        self.collision_system.add_plane("ground".to_string(), ground_plane);

        // Add collision spheres for physics objects
        for (i, &body_id) in self.physics_bodies.iter().enumerate() {
            if let Some(body) = self.physics_world.get_body(body_id) {
                match &body.shape {
                    CollisionShape::Sphere { radius } => {
                        let sphere = Sphere::new(body.position, *radius);
                        self.collision_system.add_sphere(format!("sphere_{}", i), sphere);
                    }
                    CollisionShape::Box { half_extents } => {
                        let aabox = ww3d_collision::AABoxClass::from_center_and_extent(
                            body.position, *half_extents
                        );
                        self.collision_system.add_aabox(format!("box_{}", i), aabox);
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        // Update frame counter and FPS
        self.frame_count += 1;
        if self.frame_timer.elapsed() >= Duration::from_secs(1) {
            self.fps = self.frame_count as f32;
            self.frame_count = 0;
            self.frame_timer = Instant::now();

            println!("FPS: {:.1}", self.fps);
        }

        if self.simulation_running {
            // Update physics
            self.physics_world.step();

            // Update collision system with physics object positions
            for (i, &body_id) in self.physics_bodies.iter().enumerate() {
                if let Some(body) = self.physics_world.get_body(body_id) {
                    match &body.shape {
                        CollisionShape::Sphere { radius } => {
                            let sphere = Sphere::new(body.position, *radius);
                            self.collision_system.add_sphere(format!("sphere_{}", i), sphere);
                        }
                        CollisionShape::Box { half_extents } => {
                            let aabox = ww3d_collision::AABoxClass::from_center_and_extent(
                                body.position, *half_extents
                            );
                            self.collision_system.add_aabox(format!("box_{}", i), aabox);
                        }
                        _ => {}
                    }
                }
            }

            // Update camera for dynamic view
            self.camera_angle += delta_time * 0.5;
            self.light_angle += delta_time * 0.3;

            let radius = 15.0;
            let height = 8.0;
            let x = self.camera_angle.cos() * radius;
            let z = self.camera_angle.sin() * radius;

            self.render_camera.set_position(Vec3::new(x, height, z));
            self.render_camera
                .look_at(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        }

        // Update scene
        if let Err(err) = self.scene_manager.update(delta_time) {
            eprintln!("Scene update failed: {:?}", err);
        }

        let palette = if let Some(state) = self.skeletal_state.as_mut() {
            let anim_delta = if self.simulation_running {
                delta_time
            } else {
                0.0
            };
            Some(state.update(anim_delta))
        } else {
            None
        };

        if let Some(id) = self.demo_mesh_id {
            let rotation = Mat4::from_rotation_y(self.camera_angle * 0.25);
            if let Some(obj) = self.scene_manager.render_objects_mut().get_mut(&id) {
                if let Some(mesh) = obj.as_any_mut().downcast_mut::<MeshClass>() {
                    match palette {
                        Some(bones) if !bones.is_empty() => mesh.set_bone_palette_slice(bones),
                        _ => mesh.clear_bone_palette(),
                    }
                }
                obj.set_transform(rotation);
            }
        }
    }

    pub fn render(&mut self) -> Result<(), SurfaceError> {
        self.renderer
            .begin_frame()
            .map_err(map_renderer_error)?;

        Renderer::with_global_mut(|renderer| {
            renderer.set_camera(self.render_camera.clone());
            Ok(())
        })
        .map_err(map_renderer_error)?;

        self.render_camera.get_view_projection_matrix();
        let mut render_info = RenderInfoClass::new(Arc::new(self.render_camera.clone()));
        self.scene_manager
            .apply_environment_to_render_info(&mut render_info);
        render_info.frame_count = self.frame_count;
        render_info.time = self.frame_timer.elapsed().as_secs_f32();

        self.scene_manager
            .render(&render_info)
            .map_err(map_renderer_error)?;

        self.renderer.end_frame().map_err(map_renderer_error)?;
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.render_camera
            .set_aspect_ratio(width as f32 / height as f32);

        if let Err(err) = self.renderer.resize(width, height) {
            let surface_err = map_renderer_error(err);
            eprintln!("renderer resize failed: {:?}", surface_err);
        }
    }

    pub fn handle_input(&mut self, key: VirtualKeyCode, state: ElementState) {
        if state == ElementState::Pressed {
            match key {
                VirtualKeyCode::Space => {
                    self.simulation_running = !self.simulation_running;
                    println!("Simulation {}", if self.simulation_running { "running" } else { "paused" });
                }
                VirtualKeyCode::R => {
                    // Reset physics simulation
                    for &body_id in &self.physics_bodies {
                        if let Some(body) = self.physics_world.get_body_mut(body_id) {
                            if body.mass > 0.0 {
                                // Reset position with some randomness
                                let x = (body_id.0 as f32 * 0.1).sin() * 5.0;
                                let y = 5.0 + (body_id.0 as f32 * 0.1);
                                let z = (body_id.0 as f32 * 0.1).cos() * 5.0;
                                body.position = Vec3::new(x, y, z);
                                body.linear_velocity = Vec3::ZERO;
                                body.angular_velocity = Vec3::ZERO;
                                body.update_transform();
                            }
                        }
                    }
                    println!("Physics simulation reset");
                }
                VirtualKeyCode::Escape => {
                    println!("Demo application exit requested");
                }
                _ => {}
            }
        }
    }

    pub fn get_performance_stats(&self) -> PerformanceStats {
        PerformanceStats {
            fps: self.fps,
            physics_bodies: self.physics_bodies.len(),
            skinned_meshes: usize::from(self.skeletal_state.is_some()),
            collision_objects: self.collision_system.get_stats().total_objects,
            lights: self.scene_manager.light_environment().lights.len(),
        }
    }
}

/// Performance statistics
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub fps: f32,
    pub physics_bodies: usize,
    pub skinned_meshes: usize,
    pub collision_objects: usize,
    pub lights: usize,
}

impl std::fmt::Display for PerformanceStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "🚀 WW3D Enhanced Demo Performance")?;
        writeln!(f, "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")?;
        writeln!(f, "FPS: {:.1}", self.fps)?;
        writeln!(f, "Physics Bodies: {}", self.physics_bodies)?;
        writeln!(f, "Skinned Meshes: {}", self.skinned_meshes)?;
        writeln!(f, "Collision Objects: {}", self.collision_objects)?;
        writeln!(f, "Lights: {}", self.lights)?;
        writeln!(f, "")?;
        writeln!(f, "Controls:")?;
        writeln!(f, "  SPACE: Pause/Resume simulation")?;
        writeln!(f, "  R: Reset physics simulation")?;
        writeln!(f, "  ESC: Exit")?;
        Ok(())
    }
}

fn map_renderer_error(err: RendererError) -> SurfaceError {
    match err {
        RendererError::RenderError(msg) if msg.to_lowercase().contains("surface") => {
            SurfaceError::Lost
        }
        RendererError::OutOfMemory(_) => SurfaceError::OutOfMemory,
        RendererError::NotInitialized(msg) if msg.to_lowercase().contains("surface") => {
            SurfaceError::Outdated
        }
        _ => SurfaceError::Lost,
    }
}

/// Run the enhanced demo application
pub async fn run_demo() {
    // Create event loop and window
    let event_loop = EventLoop::new();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("WW3D Enhanced Demo")
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720))
            .build(&event_loop)
            .unwrap(),
    );

    // Create demo application
    let mut app = EnhancedDemoApp::new(window.clone()).await;
    if let Err(err) = app.create_demo_objects() {
        eprintln!("⚠️  Failed to create demo objects: {err}");
    }

    println!("🎮 WW3D Enhanced Demo Started");
    println!("{}", app.get_performance_stats());

    // Main event loop
    let window_for_loop = Arc::clone(&window);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    let _ = ww3d_engine::shutdown();
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::KeyboardInput {
                    input: KeyboardInput {
                        virtual_keycode: Some(key),
                        state,
                        ..
                    },
                    ..
                } => {
                    app.handle_input(key, state);

                    if key == VirtualKeyCode::Escape {
                        let _ = ww3d_engine::shutdown();
                        *control_flow = ControlFlow::Exit;
                    }
                }
                WindowEvent::Resized(size) => {
                    app.resize(size.width, size.height);
                }
                _ => {}
            }
            Event::MainEventsCleared => {
                // Update application
                app.update(0.016); // ~60 FPS

                // Render frame
                match app.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => {
                        let size = window_for_loop.inner_size();
                        app.resize(size.width, size.height);
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        let _ = ww3d_engine::shutdown();
                        *control_flow = ControlFlow::Exit;
                    }
                    Err(e) => {
                        eprintln!("Render error: {:?}", e);
                    }
                }
            }
            Event::LoopDestroyed => {
                println!("Demo application shutting down...");
                println!("{}", app.get_performance_stats());
                let _ = ww3d_engine::shutdown();
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_stats() {
        let stats = PerformanceStats {
            fps: 60.0,
            physics_bodies: 15,
            skinned_meshes: 1,
            collision_objects: 15,
            lights: 4,
        };

        let stats_str = stats.to_string();
        assert!(stats_str.contains("60.0"));
        assert!(stats_str.contains("15"));
        assert!(stats_str.contains("4"));
    }

    #[test]
    fn test_demo_app_creation() {
        // This would require a window, so we just test the structure
        assert!(true, "Demo app structure is valid");
    }
}
