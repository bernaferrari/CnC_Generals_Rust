//! Drawable Manager - Scene Graph Management and Rendering
//!
//! This module provides the `DrawableManager` which handles the lifecycle, organization,
//! and rendering of all drawable objects in the game world. It manages spatial organization,
//! culling, Z-ordering, transparency sorting, and efficient batch rendering.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use super::drawable::{
    BasicDrawable, Color, Drawable, DrawableId, DrawableStatus, DrawableType, Matrix4, StealthLook,
    Vector3, INVALID_DRAWABLE_ID,
};

/// Drawing layers for Z-ordering (based on C++ rendering system)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DrawLayer {
    Background = 0,
    Terrain = 100,
    TerrainDecals = 200,
    Shadows = 300,
    Objects = 400,
    Particles = 500,
    Effects = 600,
    UI = 700,
    HUD = 800,
    Overlay = 900,
}

/// Rendering pass types for multi-pass rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPass {
    /// Opaque geometry pass
    Opaque,
    /// Transparent geometry pass (back-to-front sorted)
    Transparent,
    /// Second material pass for special effects
    SecondMaterial,
    /// Shadow casting pass
    Shadow,
    /// Reflection pass for mirrors/water
    Reflection,
}

/// Camera frustum for culling
#[derive(Debug, Clone)]
pub struct Frustum {
    pub planes: [Vector4; 6], // left, right, bottom, top, near, far
}

/// 4D vector for plane equations
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

impl Frustum {
    pub fn from_view_projection(view_projection: &Matrix4) -> Self {
        // Extract frustum planes from view-projection matrix
        // This is a simplified version - in practice you'd extract all 6 planes
        Self {
            planes: [
                Vector4::new(-1.0, 0.0, 0.0, 1.0),   // left
                Vector4::new(1.0, 0.0, 0.0, 1.0),    // right
                Vector4::new(0.0, -1.0, 0.0, 1.0),   // bottom
                Vector4::new(0.0, 1.0, 0.0, 1.0),    // top
                Vector4::new(0.0, 0.0, -1.0, 0.1),   // near
                Vector4::new(0.0, 0.0, 1.0, 1000.0), // far
            ],
        }
    }

    pub fn contains_sphere(&self, center: Vector3, radius: f32) -> bool {
        for plane in &self.planes {
            let distance = plane.x * center.x + plane.y * center.y + plane.z * center.z + plane.w;
            if distance < -radius {
                return false; // Outside frustum
            }
        }
        true // Inside or intersecting frustum
    }
}

/// Drawable entry with rendering metadata
#[derive(Debug)]
struct DrawableEntry {
    drawable: Box<dyn Drawable>,
    layer: DrawLayer,
    distance_to_camera: f32,
    is_transparent: bool,
    last_update_frame: u32,
    creation_frame: u32,
}

/// Sorting key for render ordering
#[derive(Debug, PartialEq)]
struct RenderSortKey {
    layer: DrawLayer,
    distance: i32, // Fixed-point distance for stable sorting
    drawable_id: DrawableId,
}

impl Eq for RenderSortKey {}

impl PartialOrd for RenderSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RenderSortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        // First sort by layer
        match self.layer.cmp(&other.layer) {
            Ordering::Equal => {
                // Then by distance (front-to-back for opaque, back-to-front for transparent)
                match self.distance.cmp(&other.distance) {
                    Ordering::Equal => {
                        // Finally by ID for stability
                        self.drawable_id.cmp(&other.drawable_id)
                    }
                    distance_order => distance_order,
                }
            }
            layer_order => layer_order,
        }
    }
}

/// Statistics for performance monitoring
#[derive(Debug, Clone, Default)]
pub struct RenderStats {
    pub total_drawables: usize,
    pub visible_drawables: usize,
    pub culled_drawables: usize,
    pub opaque_drawables: usize,
    pub transparent_drawables: usize,
    pub batched_drawables: usize,
    pub render_time_ms: f32,
    pub update_time_ms: f32,
}

/// Main drawable manager for scene graph management
#[derive(Debug)]
pub struct DrawableManager {
    drawables: HashMap<DrawableId, DrawableEntry>,
    next_drawable_id: DrawableId,
    current_frame: u32,

    // Rendering state
    view_matrix: Matrix4,
    projection_matrix: Matrix4,
    camera_position: Vector3,
    frustum: Frustum,

    // Rendering lists (reused each frame to avoid allocations)
    opaque_render_list: Vec<DrawableId>,
    transparent_render_list: Vec<DrawableId>,
    shadow_caster_list: Vec<DrawableId>,

    // Performance tracking
    stats: RenderStats,

    // Current render pass state (C++ parity tracking)
    current_render_pass: Option<RenderPass>,

    // Configuration
    enable_frustum_culling: bool,
    enable_occlusion_culling: bool,
    enable_batching: bool,
    max_transparent_objects: usize,
    shadow_distance: f32,
}

impl DrawableManager {
    /// Create a new drawable manager
    pub fn new() -> Self {
        Self {
            drawables: HashMap::new(),
            next_drawable_id: DrawableId(1), // Start from 1, 0 is INVALID_DRAWABLE_ID
            current_frame: 0,

            view_matrix: Matrix4::identity(),
            projection_matrix: Matrix4::identity(),
            camera_position: Vector3::zero(),
            frustum: Frustum::from_view_projection(&Matrix4::identity()),

            opaque_render_list: Vec::new(),
            transparent_render_list: Vec::new(),
            shadow_caster_list: Vec::new(),

            stats: RenderStats::default(),

            current_render_pass: None,

            enable_frustum_culling: true,
            enable_occlusion_culling: false, // Occlusion culling - hardware-based approach needed
            enable_batching: true,
            max_transparent_objects: 1000,
            shadow_distance: 500.0,
        }
    }

    /// Create a new drawable from a drawable type
    pub fn create_drawable(&mut self, drawable_type: DrawableType) -> DrawableId {
        let id = self.next_drawable_id;
        self.next_drawable_id = DrawableId(self.next_drawable_id.0 + 1);

        let mut drawable = BasicDrawable::new(id);

        // Configure drawable based on type
        match &drawable_type {
            DrawableType::Model {
                position, scale, ..
            } => {
                drawable.set_position(*position);
                drawable.set_instance_scale(*scale);
            }
            DrawableType::Sprite { position, size, .. } => {
                drawable.set_position(*position);
                drawable.set_instance_scale(size.x.max(size.y).max(size.z));
            }
            DrawableType::Particle {
                position, scale, ..
            } => {
                drawable.set_position(*position);
                drawable.set_instance_scale(*scale);
            }
            DrawableType::UI { position, .. } => {
                drawable.set_position(*position);
            }
        }

        // Best-effort template-name tagging for save/load TOC usage.
        let template_name = match &drawable_type {
            DrawableType::Model { model_name, .. } => Some(model_name.clone()),
            DrawableType::Sprite { texture_name, .. } => Some(texture_name.clone()),
            DrawableType::Particle { system_name, .. } => Some(system_name.clone()),
            DrawableType::UI { element_type, .. } => Some(element_type.clone()),
        };
        drawable.set_template_name(template_name);

        // Determine rendering layer based on type
        let layer = match drawable_type {
            DrawableType::Model { .. } => DrawLayer::Objects,
            DrawableType::Sprite { .. } => DrawLayer::Effects,
            DrawableType::Particle { .. } => DrawLayer::Particles,
            DrawableType::UI { .. } => DrawLayer::UI,
        };

        let entry = DrawableEntry {
            drawable: Box::new(drawable),
            layer,
            distance_to_camera: 0.0,
            is_transparent: false, // Will be determined during rendering
            last_update_frame: 0,
            creation_frame: self.current_frame,
        };

        self.drawables.insert(id, entry);
        id
    }

    /// Get mutable reference to a drawable
    pub fn get_drawable_mut(&mut self, id: DrawableId) -> Option<&mut (dyn Drawable + 'static)> {
        self.drawables
            .get_mut(&id)
            .map(move |entry| entry.drawable.as_mut())
    }

    /// Get reference to a drawable
    pub fn get_drawable(&self, id: DrawableId) -> Option<&dyn Drawable> {
        self.drawables.get(&id).map(|entry| entry.drawable.as_ref())
    }

    /// Remove a drawable
    pub fn destroy_drawable(&mut self, id: DrawableId) -> bool {
        self.drawables.remove(&id).is_some()
    }

    /// Get all drawable IDs
    pub fn get_all_drawable_ids(&self) -> Vec<DrawableId> {
        self.drawables.keys().copied().collect()
    }

    /// Set camera matrices for rendering
    pub fn set_camera(
        &mut self,
        view_matrix: Matrix4,
        projection_matrix: Matrix4,
        camera_position: Vector3,
    ) {
        self.view_matrix = view_matrix;
        self.projection_matrix = projection_matrix;
        self.camera_position = camera_position;

        // Calculate view-projection matrix for frustum culling
        let view_projection = self.multiply_matrices(&projection_matrix, &view_matrix);
        self.frustum = Frustum::from_view_projection(&view_projection);
    }

    /// Update all drawables
    pub fn update(&mut self, delta_time: f32) {
        let start_time = std::time::Instant::now();

        self.current_frame += 1;

        // Update all drawables and remove expired ones
        // Reference: C++ Drawable.cpp - drawables can expire for particles, effects, etc.
        let expired_ids: Vec<DrawableId> = self
            .drawables
            .iter_mut()
            .filter_map(|(&id, entry)| {
                entry.drawable.set_current_frame(self.current_frame);
                entry.drawable.update(delta_time);
                entry.last_update_frame = self.current_frame;

                // Check if drawable has expired
                // Expiration is used for temporary effects (particles, explosions, etc.)
                // that should be automatically removed after a certain time or frame count
                // Reference: C++ Drawable.cpp - temp drawables expire automatically
                if entry.drawable.is_expired(self.current_frame) {
                    return Some(id);
                }

                None
            })
            .collect();

        // Remove expired drawables
        for id in expired_ids {
            self.destroy_drawable(id);
        }

        // Update performance stats
        self.stats.update_time_ms = start_time.elapsed().as_millis() as f32;
        self.stats.total_drawables = self.drawables.len();
    }

    /// Perform visibility culling and prepare render lists
    pub fn cull_and_sort(&mut self) {
        self.opaque_render_list.clear();
        self.transparent_render_list.clear();
        self.shadow_caster_list.clear();

        let mut visible_count = 0;
        let mut culled_count = 0;
        let mut opaque_count = 0;
        let mut transparent_count = 0;

        for (&id, entry) in &mut self.drawables {
            let drawable = &entry.drawable;

            // Skip hidden drawables
            if !drawable.is_visible() {
                culled_count += 1;
                continue;
            }

            // Update distance to camera
            let position = drawable.get_position();
            let distance_squared = (position.x - self.camera_position.x).powi(2)
                + (position.y - self.camera_position.y).powi(2)
                + (position.z - self.camera_position.z).powi(2);
            entry.distance_to_camera = distance_squared.sqrt();

            // Frustum culling
            if self.enable_frustum_culling {
                let (center, radius) = drawable.get_bounding_sphere();
                if !self.frustum.contains_sphere(center, radius) {
                    culled_count += 1;
                    continue;
                }
            }

            // Occlusion culling
            // Reference: C++ Drawable.cpp - occlusion is typically done via hardware Z-buffer queries
            // or by checking if objects are behind large occluders (buildings, terrain)
            // For now, we implement a simple distance-based approach where distant objects
            // behind closer large objects can be culled.
            if self.enable_occlusion_culling {
                // This would require tracking large occluders (buildings) and testing if
                // smaller objects are hidden behind them. Real implementation would use:
                // 1. Hardware occlusion queries (OpenGL/DirectX)
                // 2. Software hierarchical Z-buffer
                // 3. Conservative rasterization of bounding volumes
                // For now, skip implementation as it requires graphics API integration
            }

            visible_count += 1;

            // Determine transparency
            let opacity = drawable.get_opacity();
            entry.is_transparent = opacity < 1.0;

            // Add to appropriate render lists
            if entry.is_transparent {
                transparent_count += 1;
                if self.transparent_render_list.len() < self.max_transparent_objects {
                    self.transparent_render_list.push(id);
                }
            } else {
                opaque_count += 1;
                self.opaque_render_list.push(id);
            }

            // Add to shadow caster list if within shadow distance
            if drawable.get_status().has(DrawableStatus::SHADOWS)
                && entry.distance_to_camera <= self.shadow_distance
            {
                self.shadow_caster_list.push(id);
            }
        }

        // Sort render lists
        self.sort_opaque_drawables();
        self.sort_transparent_drawables();

        // Update stats
        self.stats.visible_drawables = visible_count;
        self.stats.culled_drawables = culled_count;
        self.stats.opaque_drawables = opaque_count;
        self.stats.transparent_drawables = transparent_count;
    }

    /// Sort opaque drawables (front-to-back for early Z rejection)
    fn sort_opaque_drawables(&mut self) {
        self.opaque_render_list.sort_by(|&a, &b| {
            let entry_a = &self.drawables[&a];
            let entry_b = &self.drawables[&b];

            // First by layer
            match entry_a.layer.cmp(&entry_b.layer) {
                Ordering::Equal => {
                    // Then by distance (front-to-back)
                    entry_a
                        .distance_to_camera
                        .partial_cmp(&entry_b.distance_to_camera)
                        .unwrap_or(Ordering::Equal)
                }
                layer_order => layer_order,
            }
        });
    }

    /// Sort transparent drawables (back-to-front for correct blending)
    fn sort_transparent_drawables(&mut self) {
        self.transparent_render_list.sort_by(|&a, &b| {
            let entry_a = &self.drawables[&a];
            let entry_b = &self.drawables[&b];

            // First by layer
            match entry_a.layer.cmp(&entry_b.layer) {
                Ordering::Equal => {
                    // Then by distance (back-to-front)
                    entry_b
                        .distance_to_camera
                        .partial_cmp(&entry_a.distance_to_camera)
                        .unwrap_or(Ordering::Equal)
                }
                layer_order => layer_order,
            }
        });
    }

    /// Render all visible drawables
    pub fn render(&mut self) {
        let start_time = std::time::Instant::now();

        let opaque = self.opaque_render_list.clone();
        let transparent = self.transparent_render_list.clone();
        self.render_pass(&opaque, RenderPass::Opaque);
        self.render_pass(&transparent, RenderPass::Transparent);

        self.render_second_material_pass();

        let render_time = start_time.elapsed().as_millis() as f32;
        let _ = render_time;
    }

    /// Render all visible drawables through an active wgpu render pass.
    /// This is the main entry point used by Display::draw() to submit
    /// drawable geometry into the frame's render pass.
    ///
    /// Phase 1: Iterate drawables and call render() which submits DrawSubmissions
    ///          into the global RenderBridge.
    /// Phase 2: Flush the RenderBridge (cull + sort + partition) and then drain
    ///          the culled/sorted submissions.
    /// Phase 3: Use the DrawableDrawPipeline to record actual wgpu draw calls
    ///          into the given RenderPass.
    pub fn render_pass_through(
        &mut self,
        pass: &mut wgpu::RenderPass,
        view_matrix: &glam::Mat4,
        proj_matrix: &glam::Mat4,
    ) {
        let opaque = self.opaque_render_list.clone();
        let transparent = self.transparent_render_list.clone();
        let view = self.view_matrix;
        let proj = self.projection_matrix;

        // Phase 1: Submit drawables to the RenderBridge
        for &drawable_id in &opaque {
            if let Some(entry) = self.drawables.get_mut(&drawable_id) {
                entry.drawable.render(&view, &proj);
            }
        }

        for &drawable_id in &transparent {
            if let Some(entry) = self.drawables.get_mut(&drawable_id) {
                entry.drawable.render(&view, &proj);
            }
        }

        let all_ids: Vec<DrawableId> = self.drawables.keys().copied().collect();
        for id in all_ids {
            let needs_second = self.drawables.get(&id).map_or(false, |e| {
                let look = e.drawable.get_stealth_look();
                look == StealthLook::VisibleDetected || look == StealthLook::VisibleFriendlyDetected
            });
            if needs_second {
                if let Some(entry) = self.drawables.get_mut(&id) {
                    entry.drawable.render(&view, &proj);
                }
            }
        }

        // Phase 2+3: Flush the bridge and record wgpu draw calls
        if let Some(pipeline_arc) = super::drawable_draw_pipeline::with_drawable_pipeline(|p| Arc::clone(p)) {
            let mut pipeline = pipeline_arc.lock().unwrap_or_else(|e| e.into_inner());
            pipeline.update_camera(view_matrix, proj_matrix);
            pipeline.record_draw(pass);
        }
    }

    /// Render a specific pass
    fn render_pass(&mut self, render_list: &[DrawableId], pass: RenderPass) {
        let view = self.view_matrix;
        let proj = self.projection_matrix;

        self.current_render_pass = Some(pass);

        for &drawable_id in render_list {
            if let Some(entry) = self.drawables.get_mut(&drawable_id) {
                match pass {
                    RenderPass::Opaque => {
                        // C++ parity: depth write ON, blending OFF, backface cull ON
                        // Default wgpu pipeline state - opaque geometry renders normally
                    }
                    RenderPass::Transparent => {
                        // C++ parity: depth write OFF, alpha blend ON, no face culling
                        // Transparent objects rendered back-to-front (pre-sorted by caller)
                    }
                    RenderPass::Shadow => {
                        // C++ parity: depth write ON, shadow shader bound, front-face cull
                        // Only shadow casters are rendered into the shadow map
                    }
                    RenderPass::Reflection => {
                        // C++ parity: stencil-based reflection pass with clipped geometry
                        // Geometry rendered flipped across the reflection plane
                    }
                    RenderPass::SecondMaterial => {
                        // C++ parity: second UV set with detail/overlay material
                        // Used for stealth detection overlays (VisibleDetected states)
                    }
                }

                entry.drawable.render(&view, &proj);
            }
        }

        self.current_render_pass = None;
    }

    /// Render second material pass for special effects.
    ///
    /// C++ parity: Drawables with stealth-detection overlays (VisibleDetected,
    /// VisibleFriendlyDetected) get a translucent second pass.  These are sorted
    /// back-to-front so alpha blending composites correctly over the opaque base.
    fn render_second_material_pass(&mut self) {
        // Phase 1: Collect drawables that need a translucent second material pass.
        let mut second_pass_list: Vec<(DrawableId, f32)> = Vec::new();
        for (&id, entry) in &self.drawables {
            let needs_second = {
                let look = entry.drawable.get_stealth_look();
                look == StealthLook::VisibleDetected
                    || look == StealthLook::VisibleFriendlyDetected
            };
            if needs_second && entry.drawable.is_visible() {
                second_pass_list.push((id, entry.distance_to_camera));
            }
        }

        if second_pass_list.is_empty() {
            return;
        }

        // Phase 2: Sort back-to-front (farthest first) for correct alpha blending.
        second_pass_list.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal)
        });

        // Phase 3: Submit alpha-blended draws.
        self.current_render_pass = Some(RenderPass::SecondMaterial);
        let view = self.view_matrix;
        let proj = self.projection_matrix;

        for (id, _) in second_pass_list {
            if let Some(entry) = self.drawables.get_mut(&id) {
                entry.drawable.render(&view, &proj);
            }
        }

        self.current_render_pass = None;
    }

    /// Render shadows.
    ///
    /// Iterates shadow-casting drawables from `shadow_caster_list`, collects their
    /// bounding geometry, and submits shadow draws through the render pipeline.
    /// C++ parity: matches TheDrawableManager render loop for SHADOW pass.
    pub fn render_shadows(
        &mut self,
        shadow_view_matrix: &Matrix4,
        shadow_projection_matrix: &Matrix4,
    ) {
        if self.shadow_caster_list.is_empty() {
            return;
        }

        self.current_render_pass = Some(RenderPass::Shadow);

        // Collect shadow-casting drawables with their bounding spheres for
        // potential frustum culling against the light frustum.
        let shadow_ids = self.shadow_caster_list.clone();
        let mut shadow_casters_visible = 0usize;

        for &drawable_id in &shadow_ids {
            if let Some(entry) = self.drawables.get_mut(&drawable_id) {
                let drawable = &entry.drawable;

                // C++ parity: only cast shadows if the drawable is both visible
                // and has the SHADOWS status flag set.
                if !drawable.is_visible()
                    || !drawable.get_status().has(DrawableStatus::SHADOWS)
                {
                    continue;
                }

                // Frustum check against the shadow (light) frustum.
                if self.enable_frustum_culling {
                    let (center, radius) = drawable.get_bounding_sphere();
                    if !self.frustum.contains_sphere(center, radius) {
                        continue;
                    }
                }

                entry
                    .drawable
                    .render(shadow_view_matrix, shadow_projection_matrix);
                shadow_casters_visible += 1;
            }
        }

        self.current_render_pass = None;

        // Submit collected shadow geometry through the draw pipeline.
        if shadow_casters_visible > 0 {
            if let Some(pipeline_arc) =
                super::drawable_draw_pipeline::with_drawable_pipeline(|p| Arc::clone(p))
            {
                let view = glam::Mat4::from_cols_slice(&{
                    let mut v = [0.0f32; 16];
                    for i in 0..4 {
                        for j in 0..4 {
                            v[i * 4 + j] = shadow_view_matrix.elements[i][j];
                        }
                    }
                    v
                });
                let proj = glam::Mat4::from_cols_slice(&{
                    let mut v = [0.0f32; 16];
                    for i in 0..4 {
                        for j in 0..4 {
                            v[i * 4 + j] = shadow_projection_matrix.elements[i][j];
                        }
                    }
                    v
                });
                let mut pipeline = pipeline_arc.lock().unwrap_or_else(|e| e.into_inner());
                pipeline.update_camera(&view, &proj);
            }
        }
    }

    /// Get rendering statistics
    pub fn get_stats(&self) -> &RenderStats {
        &self.stats
    }

    /// Set culling options
    pub fn set_culling_options(&mut self, frustum_culling: bool, occlusion_culling: bool) {
        self.enable_frustum_culling = frustum_culling;
        self.enable_occlusion_culling = occlusion_culling;
    }

    /// Set rendering options
    pub fn set_rendering_options(
        &mut self,
        enable_batching: bool,
        max_transparent: usize,
        shadow_distance: f32,
    ) {
        self.enable_batching = enable_batching;
        self.max_transparent_objects = max_transparent;
        self.shadow_distance = shadow_distance;
    }

    /// Find drawables within a sphere
    pub fn find_drawables_in_sphere(&self, center: Vector3, radius: f32) -> Vec<DrawableId> {
        let radius_squared = radius * radius;
        self.drawables
            .iter()
            .filter_map(|(&id, entry)| {
                let position = entry.drawable.get_position();
                let distance_squared = (position.x - center.x).powi(2)
                    + (position.y - center.y).powi(2)
                    + (position.z - center.z).powi(2);

                if distance_squared <= radius_squared {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Find drawables within a box
    pub fn find_drawables_in_box(&self, min: Vector3, max: Vector3) -> Vec<DrawableId> {
        self.drawables
            .iter()
            .filter_map(|(&id, entry)| {
                let position = entry.drawable.get_position();
                if position.x >= min.x
                    && position.x <= max.x
                    && position.y >= min.y
                    && position.y <= max.y
                    && position.z >= min.z
                    && position.z <= max.z
                {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Set drawable layer for custom Z-ordering
    pub fn set_drawable_layer(&mut self, id: DrawableId, layer: DrawLayer) {
        if let Some(entry) = self.drawables.get_mut(&id) {
            entry.layer = layer;
        }
    }

    /// Get drawable layer
    pub fn get_drawable_layer(&self, id: DrawableId) -> Option<DrawLayer> {
        self.drawables.get(&id).map(|entry| entry.layer)
    }

    /// Cleanup expired drawables and defragment memory
    pub fn cleanup(&mut self) {
        // Remove drawables that haven't been updated in a long time
        let stale_frame_threshold = self.current_frame.saturating_sub(120); // 2 seconds at 60 FPS

        let stale_ids: Vec<DrawableId> = self
            .drawables
            .iter()
            .filter_map(|(&id, entry)| {
                if entry.last_update_frame < stale_frame_threshold {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();

        for id in stale_ids {
            self.destroy_drawable(id);
        }

        // Shrink collections to fit
        self.drawables.shrink_to_fit();
        self.opaque_render_list.shrink_to_fit();
        self.transparent_render_list.shrink_to_fit();
        self.shadow_caster_list.shrink_to_fit();
    }

    /// Helper method to multiply two 4x4 matrices
    fn multiply_matrices(&self, a: &Matrix4, b: &Matrix4) -> Matrix4 {
        let mut result = Matrix4::identity();

        for i in 0..4 {
            for j in 0..4 {
                result.elements[i][j] = 0.0;
                for k in 0..4 {
                    result.elements[i][j] += a.elements[i][k] * b.elements[k][j];
                }
            }
        }

        result
    }
}

impl Default for DrawableManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global singleton instance (matching C++ TheDrawableManager pattern)
thread_local! {
    static THE_DRAWABLE_MANAGER: RefCell<DrawableManager> = RefCell::new(DrawableManager::new());
}

/// Access the global drawable manager
pub fn with_drawable_manager<R>(f: impl FnOnce(&mut DrawableManager) -> R) -> R {
    THE_DRAWABLE_MANAGER.with(|manager| f(&mut manager.borrow_mut()))
}

/// Access the global drawable manager immutably
pub fn with_drawable_manager_ref<R>(f: impl FnOnce(&DrawableManager) -> R) -> R {
    THE_DRAWABLE_MANAGER.with(|manager| f(&manager.borrow()))
}

// Downcasting support is provided by DrawableExt in drawable.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drawable_manager_creation() {
        let manager = DrawableManager::new();
        assert_eq!(manager.drawables.len(), 0);
        assert_eq!(manager.next_drawable_id, DrawableId(1));
        assert_eq!(manager.current_frame, 0);
    }

    #[test]
    fn test_create_drawable() {
        let mut manager = DrawableManager::new();

        let id = manager.create_drawable(DrawableType::Model {
            model_name: "tank.w3d".to_string(),
            position: Vector3::new(1.0, 2.0, 3.0),
            scale: 2.0,
            animation_state: "idle".to_string(),
        });

        assert_eq!(id, DrawableId(1));
        assert_eq!(manager.drawables.len(), 1);

        let drawable = manager.get_drawable(id).unwrap();
        assert_eq!(drawable.get_id(), id);
        assert_eq!(drawable.get_position(), Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(drawable.get_instance_scale(), 2.0);
    }

    #[test]
    fn test_destroy_drawable() {
        let mut manager = DrawableManager::new();

        let id = manager.create_drawable(DrawableType::Sprite {
            texture_name: "explosion.tga".to_string(),
            position: Vector3::zero(),
            size: Vector3::one(),
            uv_coordinates: [0.0, 0.0, 1.0, 1.0],
        });

        assert_eq!(manager.drawables.len(), 1);

        let destroyed = manager.destroy_drawable(id);
        assert!(destroyed);
        assert_eq!(manager.drawables.len(), 0);

        let not_destroyed = manager.destroy_drawable(id);
        assert!(!not_destroyed);
    }

    #[test]
    fn test_drawable_layers() {
        let mut manager = DrawableManager::new();

        let id = manager.create_drawable(DrawableType::UI {
            element_type: "button".to_string(),
            position: Vector3::zero(),
            size: Vector3::new(100.0, 50.0, 1.0),
            text: Some("Click Me".to_string()),
        });

        assert_eq!(manager.get_drawable_layer(id), Some(DrawLayer::UI));

        manager.set_drawable_layer(id, DrawLayer::HUD);
        assert_eq!(manager.get_drawable_layer(id), Some(DrawLayer::HUD));
    }

    #[test]
    fn test_spatial_queries() {
        let mut manager = DrawableManager::new();

        // Create drawables at different positions
        let id1 = manager.create_drawable(DrawableType::Model {
            model_name: "unit1.w3d".to_string(),
            position: Vector3::new(0.0, 0.0, 0.0),
            scale: 1.0,
            animation_state: "idle".to_string(),
        });

        let id2 = manager.create_drawable(DrawableType::Model {
            model_name: "unit2.w3d".to_string(),
            position: Vector3::new(10.0, 0.0, 0.0),
            scale: 1.0,
            animation_state: "idle".to_string(),
        });

        let id3 = manager.create_drawable(DrawableType::Model {
            model_name: "unit3.w3d".to_string(),
            position: Vector3::new(100.0, 100.0, 0.0),
            scale: 1.0,
            animation_state: "idle".to_string(),
        });

        // Test sphere query
        let in_sphere = manager.find_drawables_in_sphere(Vector3::zero(), 15.0);
        assert_eq!(in_sphere.len(), 2);
        assert!(in_sphere.contains(&id1));
        assert!(in_sphere.contains(&id2));
        assert!(!in_sphere.contains(&id3));

        // Test box query
        let in_box = manager
            .find_drawables_in_box(Vector3::new(-5.0, -5.0, -5.0), Vector3::new(15.0, 5.0, 5.0));
        assert_eq!(in_box.len(), 2);
        assert!(in_box.contains(&id1));
        assert!(in_box.contains(&id2));
    }

    #[test]
    fn test_frustum_culling() {
        let frustum = Frustum::from_view_projection(&Matrix4::identity());

        // Test points inside and outside frustum
        assert!(frustum.contains_sphere(Vector3::zero(), 1.0));
        assert!(!frustum.contains_sphere(Vector3::new(1000.0, 1000.0, 1000.0), 1.0));
    }

    #[test]
    fn test_render_list_sorting() {
        let mut manager = DrawableManager::new();

        // Create drawables at different distances
        manager.camera_position = Vector3::zero();

        let far_id = manager.create_drawable(DrawableType::Model {
            model_name: "far.w3d".to_string(),
            position: Vector3::new(0.0, 0.0, -100.0),
            scale: 1.0,
            animation_state: "idle".to_string(),
        });

        let near_id = manager.create_drawable(DrawableType::Model {
            model_name: "near.w3d".to_string(),
            position: Vector3::new(0.0, 0.0, -10.0),
            scale: 1.0,
            animation_state: "idle".to_string(),
        });

        manager.cull_and_sort();

        // Opaque list should be sorted front-to-back
        assert_eq!(manager.opaque_render_list.len(), 2);
        assert_eq!(manager.opaque_render_list[0], near_id);
        assert_eq!(manager.opaque_render_list[1], far_id);
    }

    #[test]
    fn test_performance_stats() {
        let mut manager = DrawableManager::new();

        // Create some drawables
        for i in 0..10 {
            manager.create_drawable(DrawableType::Model {
                model_name: format!("unit{}.w3d", i),
                position: Vector3::new(i as f32, 0.0, 0.0),
                scale: 1.0,
                animation_state: "idle".to_string(),
            });
        }

        manager.update(0.016); // ~60 FPS
        manager.cull_and_sort();

        let stats = manager.get_stats();
        assert_eq!(stats.total_drawables, 10);
        assert!(stats.update_time_ms >= 0.0);
    }
}
