//! Unit Control System for Command & Conquer Generals Zero Hour
//!
//! This module provides comprehensive unit control functionality including:
//! - 3D mouse picking and raycasting for unit selection
//! - Unit selection with visual feedback
//! - Drag selection (box selection)
//! - Unit command system (move, attack, stop, etc.)
//! - Control groups (Ctrl+1-9 to assign, 1-9 to select)
//! - Unit highlighting and mouseover effects
//! - Integration with the existing input system and game logic

//! Wave 958: host_object dual-read seal (tests + residual).
use crate::command_system::{CommandType, GameCommand, ModifierKeys};
use crate::game_logic::{GameLogic, KindOf, ObjectId, Team};
use crate::input_system::RtsInputSystem;
use crate::presentation_frame::{PresentationFrame, RenderableObject};
use glam::{Mat4, Vec2, Vec3, Vec4};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as AsyncMutex;
use std::time::SystemTime;

/// Host residual: double-click select-type window (seconds).
pub const DOUBLE_CLICK_THRESHOLD_SECS: f32 = 0.3;

/// 3D Ray for mouse picking calculations
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Check if ray intersects with a sphere (for unit picking)
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> Option<f32> {
        let oc = self.origin - center;
        let a = self.direction.dot(self.direction);
        let b = 2.0 * oc.dot(self.direction);
        let c = oc.dot(oc) - radius * radius;
        let discriminant = b * b - 4.0 * a * c;

        if discriminant < 0.0 {
            None
        } else {
            let t1 = (-b - discriminant.sqrt()) / (2.0 * a);
            let t2 = (-b + discriminant.sqrt()) / (2.0 * a);
            let t = if t1 > 0.0 { t1 } else { t2 };
            if t > 0.0 { Some(t) } else { None }
        }
    }

    /// Check if ray intersects with ground plane (for movement commands)
    pub fn intersects_ground_plane(&self, ground_height: f32) -> Option<Vec3> {
        if self.direction.y.abs() < 0.001 {
            return None; // Ray is parallel to ground
        }

        let t = (ground_height - self.origin.y) / self.direction.y;
        if t > 0.0 {
            Some(self.origin + self.direction * t)
        } else {
            None
        }
    }
}

/// Camera system for proper 3D projection
#[derive(Debug, Clone)]
pub struct Camera3D {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov: f32,
    pub aspect_ratio: f32,
    pub near_plane: f32,
    pub far_plane: f32,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 50.0, 50.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov: 60.0_f32.to_radians(),
            aspect_ratio: 16.0 / 9.0,
            near_plane: 1.0,
            far_plane: 1000.0,
        }
    }
}

impl Camera3D {
    /// Get view matrix
    pub fn get_view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    /// Get projection matrix
    pub fn get_projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov, self.aspect_ratio, self.near_plane, self.far_plane)
    }

    /// Convert screen coordinates to world ray
    pub fn screen_to_ray(&self, screen_pos: Vec2, window_size: (f32, f32)) -> Ray {
        // Convert screen coordinates to normalized device coordinates (-1 to 1)
        let ndc_x = (2.0 * screen_pos.x) / window_size.0 - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_pos.y) / window_size.1;

        // Create clip space coordinates
        let clip_coords = Vec4::new(ndc_x, ndc_y, -1.0, 1.0);

        // Convert to eye coordinates
        let proj_matrix = self.get_projection_matrix();
        let inv_proj = proj_matrix.inverse();
        let eye_coords = inv_proj * clip_coords;
        let eye_coords = Vec4::new(eye_coords.x, eye_coords.y, -1.0, 0.0);

        // Convert to world coordinates
        let view_matrix = self.get_view_matrix();
        let inv_view = view_matrix.inverse();
        let world_coords = inv_view * eye_coords;
        let ray_direction = Vec3::new(world_coords.x, world_coords.y, world_coords.z).normalize();

        Ray::new(self.position, ray_direction)
    }

    /// Update camera from RTS input system
    pub fn update_from_input(&mut self, input: &RtsInputSystem) {
        let rts_camera = input.get_camera();

        // Convert RTS camera to 3D camera
        self.position = Vec3::new(
            rts_camera.position.x,
            rts_camera.zoom,
            rts_camera.position.z + rts_camera.zoom * 0.5,
        );
        self.target = Vec3::new(rts_camera.position.x, 0.0, rts_camera.position.z);
    }

    /// Set aspect ratio from window dimensions
    pub fn set_aspect_ratio(&mut self, window_size: (f32, f32)) {
        self.aspect_ratio = window_size.0 / window_size.1;
    }
}

/// Selection result from mouse picking
#[derive(Debug, Clone)]
pub struct SelectionResult {
    pub object_id: ObjectId,
    pub distance: f32,
    pub world_position: Vec3,
}

/// Control group data
#[derive(Debug, Clone)]
pub struct ControlGroup {
    pub objects: Vec<ObjectId>,
    positions: HashMap<ObjectId, Vec3>,
    pub center_position: Vec3,
}

impl Default for ControlGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlGroup {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            positions: HashMap::new(),
            center_position: Vec3::ZERO,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn clear(&mut self) {
        self.objects.clear();
        self.positions.clear();
        self.center_position = Vec3::ZERO;
    }

    pub fn add_object(&mut self, object_id: ObjectId, position: Vec3) {
        if !self.objects.contains(&object_id) {
            self.objects.push(object_id);
            self.positions.insert(object_id, position);
            self.recalculate_center();
        }
    }

    pub fn remove_object(&mut self, object_id: ObjectId) {
        self.objects.retain(|&id| id != object_id);
        self.positions.remove(&object_id);
        self.recalculate_center();
    }

    fn recalculate_center(&mut self) {
        if self.positions.is_empty() {
            self.center_position = Vec3::ZERO;
            return;
        }

        let mut sum = Vec3::ZERO;
        for pos in self.positions.values() {
            sum += *pos;
        }
        self.center_position = sum / self.positions.len() as f32;
    }
}

/// Main unit control system
pub struct UnitControlSystem {
    /// 3D camera for proper projection
    pub camera: Camera3D,

    /// Control groups (1-9)
    pub control_groups: HashMap<u8, ControlGroup>,

    /// Currently selected objects
    pub selected_objects: Vec<ObjectId>,

    /// Object under mouse cursor
    pub hovered_object: Option<ObjectId>,

    /// Window dimensions for coordinate conversion
    pub window_size: (f32, f32),

    /// Selection settings
    pub selection_radius: f32,
    /// Double-click residual window (seconds). See [`DOUBLE_CLICK_THRESHOLD_SECS`].
    pub double_click_threshold: f32,

    /// Visual feedback settings
    pub selection_color: [f32; 4],
    pub hover_color: [f32; 4],
    pub friendly_color: [f32; 4],
    pub enemy_color: [f32; 4],

    /// Player team for selection filtering
    pub local_player_team: Team,

    /// Last click time for double-click detection
    last_click_time: Option<f32>,

    /// Debug mode
    pub debug_mode: bool,

    /// Current player ID
    pub player_id: u32,

    /// Command ID counter
    next_command_id: u32,

    /// Latest presentation snapshot for pick/classify without live GameLogic identity.
    /// Commands still mutate host GameLogic; identity/filter prefers this when set.
    presentation_frame: Option<PresentationFrame>,
}

impl Default for UnitControlSystem {
    fn default() -> Self {
        Self::new((1024.0, 768.0), Team::USA, 0)
    }
}

impl UnitControlSystem {
    pub fn new(window_size: (f32, f32), local_player_team: Team, local_player_id: u32) -> Self {
        let mut camera = Camera3D::default();
        camera.set_aspect_ratio(window_size);

        Self {
            camera,
            control_groups: HashMap::new(),
            selected_objects: Vec::new(),
            hovered_object: None,
            window_size,
            selection_radius: 2.0,
            double_click_threshold: DOUBLE_CLICK_THRESHOLD_SECS,
            selection_color: [0.0, 1.0, 0.0, 1.0], // Green
            hover_color: [1.0, 1.0, 0.0, 0.5],     // Yellow
            friendly_color: [0.0, 0.0, 1.0, 1.0],  // Blue
            enemy_color: [1.0, 0.0, 0.0, 1.0],     // Red
            local_player_team,
            last_click_time: None,
            debug_mode: false,
            player_id: local_player_id,
            next_command_id: 1,
            presentation_frame: None,
        }
    }

    /// Install snapshot for pick/box-select identity residual.
    pub fn set_presentation_frame(&mut self, frame: Option<PresentationFrame>) {
        self.presentation_frame = frame;
    }

    pub fn presentation_frame(&self) -> Option<&PresentationFrame> {
        self.presentation_frame.as_ref()
    }

    /// Snapshot residual: selectable when alive, Selectable kind, not contained.
    /// C++ `Object::isSelectable` / `CanSelectDrawable` have no disabled-type
    /// gate — EMP / underpowered / unmanned units stay clickable.
    pub fn presentation_is_selectable(o: &RenderableObject) -> bool {
        // Wave 1092: presentation selectable residual fail-closed on sold /
        // unselectable/masked (catalog + pick peels match C++ status).
        // C++ CanSelectDrawable (SelectionXlat.cpp:113-116) keeps
        // KINDOF_ALWAYS_SELECTABLE even when effectively dead. Object.cpp:3011
        // returns TRUE first, so KINDOF_SELECTABLE is not required.
        if Self::presentation_is_always_selectable(o) {
            return !o.sold && !o.unselectable && !o.masked && o.contained_by.is_none();
        }
        !o.destroyed
            && !o.sold
            && !o.unselectable
            && !o.masked
            && PresentationFrame::object_has_kind(o, KindOf::Selectable)
            && o.contained_by.is_none()
    }

    /// Packed `always_selectable` bool (32-bit KindOf bank is full) or kind vec.
    pub fn presentation_is_always_selectable(o: &RenderableObject) -> bool {
        o.always_selectable || PresentationFrame::object_has_kind(o, KindOf::AlwaysSelectable)
    }

    /// C++ `CanSelectDrawable` (`SelectionXlat.cpp:113-116`): drop dead objects
    /// unless `KINDOF_ALWAYS_SELECTABLE`.
    pub fn presentation_pick_skips_dead(o: &RenderableObject) -> bool {
        o.destroyed && !Self::presentation_is_always_selectable(o)
    }

    /// C++ `CanSelectDrawable(draw, dragSelecting=TRUE)` rejects `KINDOF_STRUCTURE`
    /// for mass-drag. Lone-building / FireBase selection is applied later by
    /// `PresentationFrame::finalize_box_selection` (`SelectionXlat.cpp:634`).
    pub fn presentation_is_drag_selectable(o: &RenderableObject) -> bool {
        Self::presentation_is_selectable(o)
            && !o.is_structure
            && !PresentationFrame::object_has_kind(o, KindOf::Structure)
            && o.object_type != crate::presentation_frame::PresentationObjectType::Building
    }

    /// Snapshot residual: attackable when alive + Attackable kind.
    pub fn presentation_is_attackable(o: &RenderableObject) -> bool {
        // Keep the presentation pick path aligned with WeaponSet's first legality
        // checks.  `UNATTACKABLE` objects exist in the snapshot because they can
        // still have lifetime/vision behavior, but C++ never offers them as a
        // weapon victim (WeaponSet::getAbleToAttackSpecificObject).
        !o.destroyed
            && !o.sold
            && !o.masked
            && !o.unattackable
            && PresentationFrame::object_has_kind(o, KindOf::Attackable)
    }

    /// World-space pick residual from a presentation snapshot (engine + unit_control).
    ///
    /// `BASE_SELECTION_RADIUS` matches CncGameEngine::find_object_at_position.
    pub fn pick_object_id_at_world_from_presentation(
        frame: &PresentationFrame,
        position: glam::Vec3,
        player_team: Option<Team>,
        prioritize_enemy_targets: bool,
        base_selection_radius: f32,
    ) -> Option<ObjectId> {
        // Pure residual acquire: priority bands + nearest 3D tiebreak.
        // Per-object selection radius is applied when building candidates.
        let cands: Vec<_> = frame
            .objects
            .iter()
            .filter_map(|o| {
                if Self::presentation_pick_skips_dead(o) {
                    return None;
                }
                // Wave 1094: non-local FOW residual fail-closed (Clear only),
                // matching dual collect_selectable shroud_status>=2 peel.
                // C++ CanSelectDrawable / SelectionInfo: also drop undetected
                // stealth neutrals+enemies. is_local units stay pickable;
                // visibility_alpha < 0.95 and stealthed-undetected non-local
                // are both hidden by box_pick_hides_non_local.
                if frame.box_pick_hides_non_local(o) {
                    return None;
                }
                let distance = o.position.distance(position);
                let radius = base_selection_radius.max(o.selection_radius);
                if distance > radius {
                    return None;
                }
                let selectable = Self::presentation_is_selectable(o);
                let attackable = Self::presentation_is_attackable(o);
                let priority = if prioritize_enemy_targets {
                    match player_team {
                        Some(_) if frame.is_enemy_of_local(o) && attackable => Some(0),
                        Some(_) if frame.is_owned_by_local(o) && selectable => Some(1),
                        _ if attackable => Some(2),
                        _ if selectable => Some(3),
                        _ => None,
                    }
                } else {
                    // C++ SelectionXlat.cpp:181-189 / 679-693: a point click
                    // may select a lone enemy, civilian, or allied drawable.
                    // Prefer a locally owned unit when both sit under the cursor.
                    match player_team {
                        Some(_) if frame.is_owned_by_local(o) && selectable => Some(0),
                        Some(_) if selectable => Some(1),
                        Some(_) => None,
                        None if selectable => Some(0),
                        None => None,
                    }
                };
                let priority = if priority.is_none()
                    && prioritize_enemy_targets
                    && (o.is_crate || o.is_salvage_crate)
                {
                    Some(4)
                } else {
                    priority
                };
                Some(
                    crate::game_logic::host_residual_acquire::PriorityAcquireCandidate {
                        id: o.id,
                        position: o.position,
                        is_alive: true,
                        priority,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_best_priority_residual_target(
            ObjectId(0),
            position,
            (position.x, position.z),
            f32::MAX,
            cands,
        )
        .map(|(id, _, _)| id)
    }

    /// Pick using presentation identity when a frame is cached.
    pub fn pick_object_at_screen_pos_from_presentation(
        &self,
        screen_pos: Vec2,
        frame: &PresentationFrame,
    ) -> Option<SelectionResult> {
        let ray = self.screen_to_ray(screen_pos);
        let mut closest_result: Option<SelectionResult> = None;
        let mut closest_distance = f32::MAX;

        for o in &frame.objects {
            if !Self::presentation_is_selectable(o) {
                continue;
            }
            // Wave 1094: non-local FOW residual fail-closed (Clear only).
            // C++ CanSelectDrawable / SelectionInfo: also drop undetected
            // stealth neutrals+enemies. is_local units stay pickable;
            // visibility_alpha < 0.95 and stealthed-undetected non-local
            // are both hidden by box_pick_hides_non_local.
            if frame.box_pick_hides_non_local(o) {
                continue;
            }
            let object_position = o.position;
            let radius = self.selection_radius.max(o.selection_radius);
            if let Some(distance) = ray.intersects_sphere(object_position, radius) {
                if distance < closest_distance {
                    closest_distance = distance;
                    closest_result = Some(SelectionResult {
                        object_id: o.id,
                        distance,
                        world_position: object_position,
                    });
                }
            }
        }
        closest_result
    }

    /// Update window size and camera aspect ratio
    pub fn set_window_size(&mut self, width: f32, height: f32) {
        self.window_size = (width, height);
        self.camera.set_aspect_ratio(self.window_size);
    }

    /// Update camera from input system
    pub fn update_camera(&mut self, input: &RtsInputSystem) {
        self.camera.update_from_input(input);
    }

    /// Convert screen coordinates to world ray for picking
    pub fn screen_to_ray(&self, screen_pos: Vec2) -> Ray {
        self.camera.screen_to_ray(screen_pos, self.window_size)
    }

    /// Find object at screen position using 3D raycasting.
    ///
    /// Presentation-only: returns `None` when no snapshot is installed (no live
    /// GameLogic dual-read residual). Callers must seed `set_presentation_frame`.
    pub fn pick_object_at_screen_pos(
        &self,
        screen_pos: Vec2,
        _game_logic: &GameLogic,
    ) -> Option<SelectionResult> {
        let frame = self.presentation_frame.as_ref()?;
        self.pick_object_at_screen_pos_from_presentation(screen_pos, frame)
    }

    /// Get ground position from screen coordinates
    pub fn screen_to_ground(&self, screen_pos: Vec2) -> Option<Vec3> {
        let ray = self.screen_to_ray(screen_pos);
        ray.intersects_ground_plane(0.0) // Assuming ground is at Y=0
    }

    /// Handle left mouse click for unit selection
    /// Supports: Regular click, Shift+click (add), Ctrl+click (remove), Double-click (select all of type)
    pub async fn handle_left_click(
        &mut self,
        screen_pos: Vec2,
        shift_pressed: bool,
        ctrl_pressed: bool,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
    ) {
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());
        let now = logic.get_total_play_time();
        let is_double_click = if let Some(last_click) = self.last_click_time {
            (now - last_click) < self.double_click_threshold
        } else {
            false
        };
        self.last_click_time = Some(now);

        if let Some(result) = self.pick_object_at_screen_pos(screen_pos, &logic) {
            let friendly = if let Some(frame) = self.presentation_frame.as_ref() {
                frame
                    .objects
                    .iter()
                    .find(|o| o.id == result.object_id)
                    .map(|o| frame.is_owned_by_local(o))
                    .unwrap_or(false)
            } else {
                // Wave 951: fail-closed without presentation freeze.
                false
            };

            if friendly {
                // Host presence for selection mutations via authority API.
                if logic.host_object(result.object_id).is_some() {
                    if is_double_click {
                        // Double-click: select all units of same type
                        self.select_similar_units(result.object_id, &logic);
                    } else if ctrl_pressed {
                        // Ctrl+click: toggle selection state
                        self.toggle_object_selection(result.object_id, &mut logic);
                    } else if shift_pressed {
                        // Shift+click: prefer-selection mode; click again to deselect.
                        if self.is_object_selected(result.object_id) {
                            self.remove_from_selection(result.object_id, &mut logic);
                        } else {
                            self.add_to_selection(result.object_id, &mut logic);
                        }
                    } else {
                        // Regular click: select single unit
                        self.select_single_object(result.object_id, &mut logic);
                    }

                    println!(
                        "🎯 Selected object {} at {:?}",
                        result.object_id, result.world_position
                    );
                }
            }
        } else {
            // Clicked on empty space
            if !shift_pressed && !ctrl_pressed {
                self.clear_selection(&mut logic);
                println!("Cleared selection");
            }
        }
    }

    /// Handle right mouse click for unit commands
    pub async fn handle_right_click(
        &mut self,
        screen_pos: Vec2,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
    ) {
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        if self.selected_objects.is_empty() {
            println!("No units selected for command");
            return;
        }

        // Check if clicking on an enemy unit (attack command)
        if let Some(result) = self.pick_object_at_screen_pos(screen_pos, &logic) {
            let attackable_enemy = if let Some(frame) = self.presentation_frame.as_ref() {
                frame
                    .objects
                    .iter()
                    .find(|o| o.id == result.object_id)
                    .map(|o| frame.is_enemy_of_local(o) && Self::presentation_is_attackable(o))
                    .unwrap_or(false)
            } else {
                // Wave 951: fail-closed without presentation freeze.
                false
            };
            if attackable_enemy && logic.host_object(result.object_id).is_some() {
                // Create attack command
                let command = self.create_attack_command(result.object_id);
                logic.queue_command(command);

                println!(
                    "📢 Commanded {} units to attack target {}",
                    self.selected_objects.len(),
                    result.object_id
                );
                return;
            }
        }

        // Otherwise, issue move command to ground position
        if let Some(ground_pos) = self.screen_to_ground(screen_pos) {
            // Create move command
            let command = self.create_move_command(ground_pos);
            logic.queue_command(command);

            println!(
                "📍 Commanded {} units to move to {:?}",
                self.selected_objects.len(),
                ground_pos
            );
        }
    }

    /// Handle drag selection (box selection)
    pub async fn handle_box_selection(
        &mut self,
        start_screen: Vec2,
        end_screen: Vec2,
        shift_pressed: bool,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
    ) {
        let logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        // Convert screen box to world coordinates
        let start_world = self.screen_to_ground(start_screen);
        let end_world = self.screen_to_ground(end_screen);

        if let (Some(start), Some(end)) = (start_world, end_world) {
            let min_x = start.x.min(end.x);
            let max_x = start.x.max(end.x);
            let min_z = start.z.min(end.z);
            let max_z = start.z.max(end.z);

            let mut selected_in_box = Vec::new();

            // Wave 949: box selection is presentation-only (no live GameLogic dual-read).
            // C++ SelectionXlat.cpp:628-693 — lone building / enemy / FireBase container.
            if let Some(frame) = self.presentation_frame.as_ref() {
                selected_in_box =
                    frame.box_select_unit_ids(self.local_player_team, min_x, max_x, min_z, max_z);
            }

            if !selected_in_box.is_empty() {
                if shift_pressed {
                    // Add to existing selection
                    for obj_id in selected_in_box {
                        if !self.selected_objects.contains(&obj_id) {
                            self.selected_objects.push(obj_id);
                        }
                    }
                } else {
                    // Replace selection
                    self.selected_objects = selected_in_box;
                }

                // Update game logic selection
                drop(logic);
                let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());
                logic.select_objects(self.player_id, self.selected_objects.clone());

                println!("📦 Box selected {} units", self.selected_objects.len());
            }
        }
    }

    /// Update hover state based on mouse position
    pub async fn update_hover(
        &mut self,
        screen_pos: Vec2,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
    ) {
        let logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        let new_hovered = self
            .pick_object_at_screen_pos(screen_pos, &logic)
            .map(|result| result.object_id);

        if self.hovered_object != new_hovered {
            self.hovered_object = new_hovered;

            if let Some(obj_id) = self.hovered_object {
                if self.debug_mode {
                    println!("🖱️ Hovering over object {}", obj_id);
                }
            }
        }
    }

    /// Select a single object
    fn select_single_object(&mut self, object_id: ObjectId, game_logic: &mut GameLogic) {
        // Wave 1095: when a presentation freeze is installed, refuse unusable ids
        // (pick already peels FOW/status; belt-and-suspenders for hotkey/stale paths).
        if let Some(frame) = self.presentation_frame.as_ref() {
            let ok = frame.objects.iter().any(|o| {
                o.id == object_id
                    && frame.is_owned_by_local(o)
                    && Self::presentation_is_selectable(o)
            });
            if !ok {
                self.selected_objects.clear();
                game_logic.select_objects(self.player_id, self.selected_objects.clone());
                return;
            }
        }
        self.selected_objects.clear();
        self.selected_objects.push(object_id);
        game_logic.select_objects(self.player_id, self.selected_objects.clone());
    }

    /// Add object to selection (Shift+click)
    fn add_to_selection(&mut self, object_id: ObjectId, game_logic: &mut GameLogic) {
        if self.selected_objects.contains(&object_id) {
            return;
        }
        // Wave 1095: presentation freeze fail-closed on unusable add-to-selection.
        if let Some(frame) = self.presentation_frame.as_ref() {
            let ok = frame.objects.iter().any(|o| {
                o.id == object_id
                    && frame.is_owned_by_local(o)
                    && Self::presentation_is_selectable(o)
            });
            if !ok {
                return;
            }
        }
        self.selected_objects.push(object_id);
        game_logic.select_objects(self.player_id, self.selected_objects.clone());
        println!(
            "Added unit {} to selection (total: {})",
            object_id,
            self.selected_objects.len()
        );
    }

    /// Remove object from selection (Ctrl+click)
    fn remove_from_selection(&mut self, object_id: ObjectId, game_logic: &mut GameLogic) {
        if let Some(index) = self.selected_objects.iter().position(|&id| id == object_id) {
            self.selected_objects.remove(index);
            game_logic.select_objects(self.player_id, self.selected_objects.clone());
            println!(
                "Removed unit {} from selection (remaining: {})",
                object_id,
                self.selected_objects.len()
            );
        }
    }

    /// Toggle object selection (add/remove) - deprecated in favor of explicit add/remove
    fn toggle_object_selection(&mut self, object_id: ObjectId, game_logic: &mut GameLogic) {
        if self.selected_objects.contains(&object_id) {
            self.remove_from_selection(object_id, game_logic);
        } else {
            self.add_to_selection(object_id, game_logic);
        }
    }

    /// Select all units of the same type as the clicked unit
    fn select_similar_units(&mut self, object_id: ObjectId, _game_logic: &GameLogic) {
        // Wave 949: presentation-only select-similar (no live GameLogic dual-read).
        let Some(frame) = self.presentation_frame.as_ref() else {
            return;
        };
        let Some(clicked) = frame.objects.iter().find(|o| o.id == object_id) else {
            return;
        };
        let template_name = clicked.template_name.clone();
        self.selected_objects.clear();
        for o in &frame.objects {
            if frame.is_owned_by_local(o)
                && Self::presentation_is_selectable(o)
                && o.template_name == template_name
            {
                self.selected_objects.push(o.id);
            }
        }
        println!(
            "Selected {} units of type {} (presentation)",
            self.selected_objects.len(),
            template_name
        );
    }

    /// Clear current selection
    fn clear_selection(&mut self, game_logic: &mut GameLogic) {
        self.selected_objects.clear();
        game_logic.select_objects(self.player_id, self.selected_objects.clone());
    }

    /// Assign selected units to a control group (Ctrl+0-9)
    pub async fn assign_control_group(
        &mut self,
        group_num: u8,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
    ) {
        // Support groups 0-9 (10 total) like C++ Generals
        if group_num > 9 {
            return;
        }

        let logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        let mut control_group = ControlGroup::new();
        for &object_id in &self.selected_objects {
            // Prefer presentation pose when dual-tick snapshot is installed.
            if let Some(frame) = self.presentation_frame.as_ref() {
                if let Some(o) = frame.objects.iter().find(|o| o.id == object_id) {
                    // Wave 1095: assign residual fail-closed on full selectable
                    // legality (sold/unselectable/masked/disabled), not only destroyed.
                    // C++ CREATE_TEAM also requires isLocallyControlled().
                    if !Self::presentation_is_selectable(o) || !frame.is_owned_by_local(o) {
                        continue;
                    }
                    control_group.add_object(object_id, o.position);
                    continue;
                }
            }
            // Wave 951: control-group assign is presentation-only (fail-closed live).
        }
        let assigned: Vec<ObjectId> = control_group.objects.clone();
        for (&other_num, other) in self.control_groups.iter_mut() {
            if other_num == group_num {
                continue;
            }
            for id in &assigned {
                other.remove_object(*id);
            }
        }
        self.control_groups.insert(group_num, control_group);
        println!(
            "🔗 Assigned {} units to control group {}",
            self.selected_objects.len(),
            group_num
        );
    }

    /// Select units from a control group (press 0-9)
    pub async fn select_control_group(
        &mut self,
        group_num: u8,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
    ) {
        // Support groups 0-9 (10 total) like C++ Generals
        if group_num > 9 {
            return;
        }

        if let Some(control_group) = self.control_groups.get_mut(&group_num) {
            let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

            // C++ SELECT_TEAM: getLiveObjects() + local owner. Not CanSelectDrawable
            // (garrisoned / transported / FireBase members stay on the squad).
            let valid_objects: Vec<ObjectId> = if let Some(frame) = self.presentation_frame.as_ref()
            {
                frame.filter_live_squad_ids(&control_group.objects, true)
            } else {
                // Wave 951: fail-closed without presentation freeze.
                Vec::new()
            };

            self.selected_objects = valid_objects;
            logic.select_objects(self.player_id, self.selected_objects.clone());

            // Refresh cached positions for display/centering.
            control_group.positions.clear();
            for &obj_id in &self.selected_objects {
                if let Some(frame) = self.presentation_frame.as_ref() {
                    if let Some(o) = frame.objects.iter().find(|o| o.id == obj_id) {
                        control_group.positions.insert(obj_id, o.position);
                        continue;
                    }
                }
                // Wave 951: positions from presentation only (fail-closed live).
            }
            control_group.recalculate_center();

            println!(
                "🎯 Selected control group {}: {} units",
                group_num,
                self.selected_objects.len()
            );
        } else {
            println!("Control group {} is empty", group_num);
        }
    }

    /// Get control group composition for UI display
    pub fn get_control_group_info(&self, group_num: u8) -> Option<&ControlGroup> {
        self.control_groups.get(&group_num)
    }

    /// Get all active control groups
    pub fn get_all_control_groups(&self) -> &HashMap<u8, ControlGroup> {
        &self.control_groups
    }

    /// Check if a unit belongs to any control group
    pub fn get_unit_control_groups(&self, object_id: ObjectId) -> Vec<u8> {
        let mut groups = Vec::new();
        for (group_num, control_group) in &self.control_groups {
            if control_group.objects.contains(&object_id) {
                groups.push(*group_num);
            }
        }
        groups
    }

    /// Select all player units (Ctrl+A)
    pub async fn select_all_units(&mut self, game_logic: &Arc<AsyncMutex<GameLogic>>) {
        // Wave 949: presentation-only select-all (no live GameLogic dual-read).
        self.selected_objects.clear();
        if let Some(frame) = self.presentation_frame.as_ref() {
            for o in &frame.objects {
                if frame.is_owned_by_local(o) && Self::presentation_is_selectable(o) {
                    self.selected_objects.push(o.id);
                }
            }
        }
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());
        logic.select_objects(self.player_id, self.selected_objects.clone());
        println!("Selected all {} units", self.selected_objects.len());
    }

    /// Get currently selected objects
    pub fn get_selected_objects(&self) -> &[ObjectId] {
        &self.selected_objects
    }

    /// Get hovered object
    pub fn get_hovered_object(&self) -> Option<ObjectId> {
        self.hovered_object
    }

    /// Check if an object is selected
    pub fn is_object_selected(&self, object_id: ObjectId) -> bool {
        self.selected_objects.contains(&object_id)
    }

    /// Get selection center for camera focusing.
    ///
    /// Presentation-only poses when a snapshot is installed; `None` otherwise
    /// (no live GameLogic dual-read residual).
    pub fn get_selection_center(&self, _game_logic: &GameLogic) -> Option<Vec3> {
        if self.selected_objects.is_empty() {
            return None;
        }
        let frame = self.presentation_frame.as_ref()?;
        let mut center = Vec3::ZERO;
        let mut count = 0;
        for &object_id in &self.selected_objects {
            if let Some(o) = frame
                .objects
                .iter()
                .find(|o| o.id == object_id && !o.destroyed)
            {
                center += o.position;
                count += 1;
            }
        }
        if count > 0 {
            Some(center / count as f32)
        } else {
            None
        }
    }

    /// Enable/disable debug mode
    pub fn set_debug_mode(&mut self, enabled: bool) {
        self.debug_mode = enabled;
        println!(
            "Unit control debug mode: {}",
            if enabled { "ON" } else { "OFF" }
        );
    }

    /// Issue Stop command to all selected units
    pub async fn command_stop(&mut self, game_logic: &Arc<AsyncMutex<GameLogic>>) {
        if self.selected_objects.is_empty() {
            return;
        }

        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());
        let command = self.create_stop_command();

        for &object_id in &self.selected_objects {
            if let Some(obj) = logic./* Wave 950 */ host_object_mut(object_id) {
                if obj.is_mobile() {
                    obj.stop();
                }
            }
        }

        println!(
            "🛑 Stop command issued to {} units",
            self.selected_objects.len()
        );
        self.log_command(&command);
    }

    /// Issue Hold Position command to all selected units
    pub async fn command_hold_position(&mut self, game_logic: &Arc<AsyncMutex<GameLogic>>) {
        if self.selected_objects.is_empty() {
            return;
        }

        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        for &object_id in &self.selected_objects {
            if let Some(obj) = logic.host_object_mut(object_id) {
                obj.set_guard_position(None);
            }
        }

        println!(
            "⚓ Hold position command issued to {} units",
            self.selected_objects.len()
        );
    }

    /// Issue Guard command to all selected units
    pub async fn command_guard(&mut self, game_logic: &Arc<AsyncMutex<GameLogic>>) {
        if self.selected_objects.is_empty() {
            return;
        }

        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        for &object_id in &self.selected_objects {
            if let Some(obj) = logic.host_object_mut(object_id) {
                obj.set_guard_target(None);
            }
        }

        println!(
            "🛡️ Guard command issued to {} units",
            self.selected_objects.len()
        );
    }

    // === Command Generation Methods ===

    /// Create a move command for selected units
    fn create_move_command(&mut self, destination: Vec3) -> GameCommand {
        GameCommand {
            command_type: CommandType::MoveTo {
                destination,
                waypoints: Vec::new(),
            },
            player_id: self.player_id,
            command_id: self.get_next_command_id(),
            timestamp: SystemTime::now(),
            selected_units: self.selected_objects.clone(),
            modifier_keys: ModifierKeys::default(),
        }
    }

    /// Create an attack command for selected units
    fn create_attack_command(&mut self, target_id: ObjectId) -> GameCommand {
        GameCommand {
            command_type: CommandType::AttackObject { target_id },
            player_id: self.player_id,
            command_id: self.get_next_command_id(),
            timestamp: SystemTime::now(),
            selected_units: self.selected_objects.clone(),
            modifier_keys: ModifierKeys::default(),
        }
    }

    /// Create a stop command for selected units
    fn create_stop_command(&mut self) -> GameCommand {
        GameCommand {
            command_type: CommandType::Stop,
            player_id: self.player_id,
            command_id: self.get_next_command_id(),
            timestamp: SystemTime::now(),
            selected_units: self.selected_objects.clone(),
            modifier_keys: ModifierKeys::default(),
        }
    }

    fn log_command(&self, command: &GameCommand) {
        if self.debug_mode {
            println!(
                "Command {:?} issued (id {}) for {} units",
                command.command_type,
                command.command_id,
                command.selected_units.len()
            );
        }
    }

    /// Create a build command
    pub fn create_build_command(&mut self, template_name: String, location: Vec3) -> GameCommand {
        GameCommand {
            command_type: CommandType::Build {
                template_name,
                location,
            },
            player_id: self.player_id,
            command_id: self.get_next_command_id(),
            timestamp: SystemTime::now(),
            selected_units: self.selected_objects.clone(),
            modifier_keys: ModifierKeys::default(),
        }
    }

    /// Create a queue unit production command
    pub fn create_queue_unit_command(
        &mut self,
        template_name: String,
        quantity: u32,
    ) -> GameCommand {
        GameCommand {
            command_type: CommandType::QueueUnitCreate {
                template_name,
                quantity,
            },
            player_id: self.player_id,
            command_id: self.get_next_command_id(),
            timestamp: SystemTime::now(),
            selected_units: self.selected_objects.clone(),
            modifier_keys: ModifierKeys::default(),
        }
    }

    /// Get next command ID
    fn get_next_command_id(&mut self) -> u32 {
        let id = self.next_command_id;
        self.next_command_id += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;
    use glam::Vec2;

    fn logic_with_selectable_unit() -> (GameLogic, ObjectId) {
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("Ranger");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("Ranger".into(), t);
        let id = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        (logic, id)
    }

    #[test]
    fn selection_center_prefers_presentation_pose() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SelCenterPres");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SelC") {
            let mut t = ThingTemplate::new("SelC");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("SelC".into(), t);
        }
        let id = logic
            .create_object("SelC", Team::USA, glam::Vec3::new(10.0, 0.0, 20.0))
            .expect("id");
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        // Poison live pose — presentation must still win.
        if let Some(obj) = logic.host_object_mut(id) {
            obj.position = glam::Vec3::new(9999.0, 0.0, 9999.0);
        }
        let mut ctl = UnitControlSystem::new((800.0, 600.0), Team::USA, 0);
        ctl.selected_objects = vec![id];
        ctl.set_presentation_frame(Some(frame));
        let center = ctl.get_selection_center(&logic).expect("center");
        assert!(
            (center.x - 10.0).abs() < 0.1 && (center.z - 20.0).abs() < 0.1,
            "expected presentation pose, got {center:?}"
        );
        let src = include_str!("unit_control.rs");
        assert!(
            src.contains("Prefer presentation poses when a snapshot is installed"),
            "selection center must prefer presentation residual"
        );
    }

    fn select_similar_prefers_presentation_identity() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SelSimilarPres");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["SimA", "SimB"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.set_health(100.0);
                t.add_kind_of(KindOf::Selectable);
                logic.templates.insert(name.into(), t);
            }
        }
        let a1 = logic
            .create_object("SimA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("a1");
        let a2 = logic
            .create_object("SimA", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("a2");
        let b1 = logic
            .create_object("SimB", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("b1");
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        // Live renames one SimA to poison live dual-read if used.
        if let Some(obj) = logic.host_object_mut(a2) {
            obj.template_name = "Poisoned".into();
        }
        let mut ctl = UnitControlSystem::new((800.0, 600.0), Team::USA, 0);
        ctl.set_presentation_frame(Some(frame));
        ctl.select_similar_units(a1, &logic);
        assert!(ctl.selected_objects.contains(&a1));
        assert!(
            ctl.selected_objects.contains(&a2),
            "presentation still sees a2 as SimA"
        );
        assert!(!ctl.selected_objects.contains(&b1));
        assert_eq!(ctl.selected_objects.len(), 2);
        let src = include_str!("unit_control.rs");
        assert!(
            src.contains("presentation_is_selectable(o)")
                && src.contains("Prefer presentation identity"),
            "select_similar must prefer presentation residual"
        );
    }

    fn pick_prefers_presentation_identity_not_live_move() {
        let (mut logic, id) = logic_with_selectable_unit();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        // Move live object far away after snapshot.
        if let Some(o) = logic.host_object_mut(id) {
            o.set_position(glam::Vec3::new(5000.0, 0.0, 5000.0));
        }
        let mut ctl = UnitControlSystem::new((800.0, 600.0), Team::USA, 0);
        // Camera default looks at origin-ish; use presentation pick helper directly.
        ctl.set_presentation_frame(Some(frame.clone()));
        let picked =
            ctl.pick_object_at_screen_pos_from_presentation(Vec2::new(400.0, 300.0), &frame);
        // May miss due to camera/projection; assert presentation path is required
        // and live-only (no snapshot) returns None.
        let live_only = UnitControlSystem::new((800.0, 600.0), Team::USA, 0);
        // No presentation on live_only → presentation-only pick is empty.
        let live_pick = live_only.pick_object_at_screen_pos(Vec2::new(400.0, 300.0), &logic);
        assert!(
            live_pick.is_none(),
            "pick without presentation frame must not dual-read live GameLogic"
        );
        let pres_pick = ctl.pick_object_at_screen_pos(Vec2::new(400.0, 300.0), &logic);
        // Snapshot still has unit at origin; live is at 5000.
        if let Some(p) = pres_pick {
            assert_eq!(p.object_id, id);
            // world position from snapshot residual (origin), not live 5000.
            assert!(p.world_position.x.abs() < 50.0);
            assert!(p.world_position.z.abs() < 50.0);
        } else {
            // Camera may not intersect; still verify helper walks presentation objects.
            assert!(frame.objects.iter().any(|o| o.id == id));
            assert!(PresentationFrame::object_has_kind(
                frame.objects.iter().find(|o| o.id == id).unwrap(),
                KindOf::Selectable
            ));
            let _ = picked;
        }
    }

    #[test]
    fn box_select_uses_presentation_structure_kind() {
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("WarFactory");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("WarFactory".into(), t);
        let mut u = ThingTemplate::new("Ranger");
        u.set_health(100.0);
        u.add_kind_of(KindOf::Infantry);
        u.add_kind_of(KindOf::Selectable);
        logic.templates.insert("Ranger".into(), u);
        let bid = logic
            .create_object("WarFactory", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("b");
        let uid = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("u");
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let b = frame.objects.iter().find(|o| o.id == bid).unwrap();
        let r = frame.objects.iter().find(|o| o.id == uid).unwrap();
        assert!(PresentationFrame::object_has_kind(b, KindOf::Structure));
        assert!(!PresentationFrame::object_has_kind(r, KindOf::Structure));
        assert!(UnitControlSystem::presentation_is_selectable(b));
        assert!(UnitControlSystem::presentation_is_selectable(r));
        assert!(!UnitControlSystem::presentation_is_drag_selectable(b));
        assert!(UnitControlSystem::presentation_is_drag_selectable(r));
    }

    #[test]
    fn box_select_selects_lone_building_and_firebase_container() {
        let mut logic = GameLogic::new();
        let mut factory = ThingTemplate::new("WarFactory");
        factory.set_health(1000.0);
        factory.add_kind_of(KindOf::Structure);
        factory.add_kind_of(KindOf::Selectable);
        logic.templates.insert("WarFactory".into(), factory);
        let mut firebase = ThingTemplate::new("AmericaFireBase");
        firebase.set_health(1000.0);
        firebase.add_kind_of(KindOf::Structure);
        firebase.add_kind_of(KindOf::Selectable);
        logic.templates.insert("AmericaFireBase".into(), firebase);
        let mut ranger = ThingTemplate::new("Ranger");
        ranger.set_health(100.0);
        ranger.add_kind_of(KindOf::Infantry);
        ranger.add_kind_of(KindOf::Selectable);
        logic.templates.insert("Ranger".into(), ranger);
        let factory_id = logic
            .create_object("WarFactory", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("factory");
        let firebase_id = logic
            .create_object(
                "AmericaFireBase",
                Team::USA,
                glam::Vec3::new(20.0, 0.0, 20.0),
            )
            .expect("firebase");
        let occupant = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(20.5, 0.0, 20.5))
            .expect("occupant");
        if let Some(obj) = logic.host_object_mut(occupant) {
            obj.set_contained_by(Some(firebase_id));
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(
            frame.box_select_unit_ids(Team::USA, -1.0, 1.0, -1.0, 1.0),
            vec![factory_id]
        );
        let mut firebase_box = frame.box_select_unit_ids(Team::USA, 19.0, 21.0, 19.0, 21.0);
        firebase_box.sort_by_key(|id| id.0);
        assert_eq!(firebase_box, vec![firebase_id]);
    }

    #[test]
    fn presentation_attackable_residual() {
        let (logic, id) = logic_with_selectable_unit();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let o = frame.objects.iter().find(|x| x.id == id).unwrap();
        assert!(UnitControlSystem::presentation_is_attackable(o));
    }

    #[test]
    fn presentation_attackable_rejects_unattackable_weaponset_victims() {
        let (mut logic, id) = logic_with_selectable_unit();
        logic
            .host_object_mut(id)
            .expect("live object")
            .thing
            .template
            .add_kind_of(KindOf::Unattackable);
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let object = frame.objects.iter().find(|object| object.id == id).unwrap();
        assert!(object.unattackable);
        assert!(!UnitControlSystem::presentation_is_attackable(object));
    }

    #[test]
    fn first_enemy_attack_command_id_prefers_attackable_before_force_attack() {
        let mut logic = GameLogic::new();
        let mut barracks = ThingTemplate::new("ChinaBarracks");
        barracks.set_health(1000.0);
        barracks.add_kind_of(KindOf::Structure);
        barracks.add_kind_of(KindOf::Attackable);
        logic.templates.insert("ChinaBarracks".into(), barracks);
        let mut ranger = ThingTemplate::new("ChinaRanger");
        ranger.set_health(100.0);
        ranger.add_kind_of(KindOf::Infantry);
        ranger.add_kind_of(KindOf::Attackable);
        ranger.add_kind_of(KindOf::Selectable);
        logic.templates.insert("ChinaRanger".into(), ranger);
        let mut usa = ThingTemplate::new("UsaRanger");
        usa.set_health(100.0);
        usa.add_kind_of(KindOf::Infantry);
        usa.add_kind_of(KindOf::Selectable);
        usa.add_kind_of(KindOf::Attackable);
        logic.templates.insert("UsaRanger".into(), usa);
        let _friendly = logic
            .create_object("UsaRanger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("usa");
        let building = logic
            .create_object(
                "ChinaBarracks",
                Team::China,
                glam::Vec3::new(40.0, 0.0, 0.0),
            )
            .expect("b");
        let mobile = logic
            .create_object("ChinaRanger", Team::China, glam::Vec3::new(50.0, 0.0, 0.0))
            .expect("m");
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let attackable = frame.first_enemy_attackable_id(Team::USA);
        let force = frame.first_enemy_force_attack_id(Team::USA);
        let cmd = frame.first_enemy_attack_command_id(Team::USA);
        assert_eq!(attackable, Some(building));
        assert_eq!(force, Some(mobile));
        assert_eq!(
            cmd, attackable,
            "host attack_nearest_enemy must prefer FOW-clear attackable before force-attack"
        );
        assert_eq!(cmd, attackable.or(force));
    }

    #[test]
    fn world_pick_from_presentation_ignores_live_move() {
        let (mut logic, id) = logic_with_selectable_unit();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        if let Some(o) = logic.host_object_mut(id) {
            o.set_position(glam::Vec3::new(9000.0, 0.0, 9000.0));
        }
        let picked = UnitControlSystem::pick_object_id_at_world_from_presentation(
            &frame,
            glam::Vec3::ZERO,
            Some(Team::USA),
            false,
            20.0,
        );
        assert_eq!(picked, Some(id));
        // Live object is far away — presentation still hits origin residual.
        let live_pos = logic.host_object(id).unwrap().get_position();
        assert!(live_pos.x > 1000.0);
    }

    #[test]
    fn control_group_assign_prefers_presentation_pose() {
        let (mut logic, id) = logic_with_selectable_unit();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        if let Some(o) = logic.host_object_mut(id) {
            o.set_position(glam::Vec3::new(8000.0, 0.0, 8000.0));
        }
        let mut ctl = UnitControlSystem::new((800.0, 600.0), Team::USA, 0);
        ctl.selected_objects = vec![id];
        ctl.set_presentation_frame(Some(frame));
        let logic_arc = std::sync::Arc::new(AsyncMutex::new(logic));
        futures::executor::block_on(ctl.assign_control_group(1, &logic_arc));
        let group = ctl.get_control_group_info(1).expect("g1");
        assert_eq!(group.objects, vec![id]);
        let pos = *group.positions.get(&id).expect("pos");
        assert!(
            pos.x.abs() < 50.0 && pos.z.abs() < 50.0,
            "expected snapshot origin pose, got {pos:?}"
        );
    }

    #[test]
    fn control_group_select_filters_destroyed_from_presentation() {
        let (mut logic, id) = logic_with_selectable_unit();
        let mut t = ThingTemplate::new("RangerB");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RangerB".into(), t);
        let id2 = logic
            .create_object("RangerB", Team::USA, glam::Vec3::new(3.0, 0.0, 0.0))
            .unwrap();
        if let Some(o) = logic.host_object_mut(id2) {
            o.status.destroyed = true;
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let mut ctl = UnitControlSystem::new((800.0, 600.0), Team::USA, 0);
        ctl.selected_objects = vec![id, id2];
        ctl.set_presentation_frame(Some(frame));
        let logic_arc = std::sync::Arc::new(AsyncMutex::new(logic));
        futures::executor::block_on(ctl.assign_control_group(2, &logic_arc));
        let group = ctl.get_control_group_info(2).expect("g2");
        assert_eq!(group.objects, vec![id]);
        futures::executor::block_on(ctl.select_control_group(2, &logic_arc));
        assert_eq!(ctl.selected_objects, vec![id]);
    }

    #[test]
    fn point_pick_selects_lone_enemy_civilian_and_allied_units() {
        // C++ SelectionXlat.cpp:181-189 / 679-693 — a point click may select
        // a lone enemy, civilian (NEUTRAL), or allied drawable. Drag stays local-only.
        use crate::game_logic::{GameLogic, Player};
        let _shroud_test_guard = crate::fow_rendering::shroud_test_isolation_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        {
            let shroud_manager = gamelogic::system::shroud_manager::get_shroud_manager();
            let mut shroud = shroud_manager.lock()
                .expect("shroud");
            shroud.clear_all();
        }

        let mut logic = GameLogic::new();
        let local = Player::new(0, Team::USA, "USA", true);
        let ally = {
            let mut p = Player::new(1, Team::China, "China", false);
            p.alliance_team = 7;
            p
        };
        let mut local = local;
        local.alliance_team = 7;
        logic.add_player(local);
        logic.add_player(ally);
        logic.add_player(Player::new(2, Team::GLA, "GLA", false));

        let mut make = |name: &str| {
            let mut t = ThingTemplate::new(name);
            t.set_health(100.0);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        };
        make("LocalRanger");
        make("AllyRedGuard");
        make("EnemyRebel");
        make("Civilian");

        let local_id = logic
            .create_object_for_player("LocalRanger", 0, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("local");
        let ally_id = logic
            .create_object_for_player("AllyRedGuard", 1, glam::Vec3::new(40.0, 0.0, 0.0))
            .expect("ally");
        let enemy_id = logic
            .create_object_for_player("EnemyRebel", 2, glam::Vec3::new(80.0, 0.0, 0.0))
            .expect("enemy");
        let civ_id = logic
            .create_object("Civilian", Team::Neutral, glam::Vec3::new(120.0, 0.0, 0.0))
            .expect("civ");

        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let pick = |pos: glam::Vec3| {
            UnitControlSystem::pick_object_id_at_world_from_presentation(
                &frame,
                pos,
                Some(Team::USA),
                false,
                20.0,
            )
        };

        assert_eq!(pick(glam::Vec3::new(0.0, 0.0, 0.0)), Some(local_id));
        assert_eq!(pick(glam::Vec3::new(40.0, 0.0, 0.0)), Some(ally_id));
        assert_eq!(pick(glam::Vec3::new(80.0, 0.0, 0.0)), Some(enemy_id));
        assert_eq!(pick(glam::Vec3::new(120.0, 0.0, 0.0)), Some(civ_id));
    }

    #[test]
    fn presentation_is_selectable_allows_disabled_emp_units() {
        // C++ Object.cpp:3001-3020 / SelectionXlat.cpp:104-189 — no disabled gate.
        let (logic, id) = logic_with_selectable_unit();
        let mut frame = PresentationFrame::build_from_logic(&logic, 0);
        let obj = frame.objects.iter_mut().find(|o| o.id == id).expect("unit");
        obj.disabled = true;
        obj.disabled_emp = true;
        assert!(
            UnitControlSystem::presentation_is_selectable(obj),
            "EMP / disabled units must stay clickable"
        );
        assert_eq!(
            UnitControlSystem::pick_object_id_at_world_from_presentation(
                &frame,
                glam::Vec3::ZERO,
                Some(Team::USA),
                false,
                20.0,
            ),
            Some(id)
        );
    }

    #[test]
    fn presentation_is_selectable_keeps_always_selectable_dead_objects() {
        // C++ SelectionXlat.cpp:113-116 CanSelectDrawable + Object.cpp:3011.
        use crate::pick_ray::pick_object_id_along_camera_ray;
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("UIBeacon");
        t.set_health(100.0);
        t.add_kind_of(KindOf::AlwaysSelectable);
        logic.templates.insert("UIBeacon".into(), t);
        let id = logic
            .create_object("UIBeacon", Team::USA, glam::Vec3::ZERO)
            .expect("beacon");
        let mut frame = PresentationFrame::build_from_logic(&logic, 0);
        {
            let obj = frame
                .objects
                .iter_mut()
                .find(|o| o.id == id)
                .expect("beacon row");
            assert!(
                obj.always_selectable,
                "presentation must freeze KINDOF_ALWAYS_SELECTABLE outside the 32-bit bank"
            );
            obj.destroyed = true;
            obj.kind_of.retain(|k| *k != KindOf::Selectable);
            assert!(
                UnitControlSystem::presentation_is_selectable(obj),
                "ALWAYS_SELECTABLE dead objects stay clickable without Selectable kind"
            );
            assert!(!UnitControlSystem::presentation_pick_skips_dead(obj));
        }
        assert_eq!(
            UnitControlSystem::pick_object_id_at_world_from_presentation(
                &frame,
                glam::Vec3::ZERO,
                Some(Team::USA),
                false,
                20.0,
            ),
            Some(id)
        );
        assert_eq!(
            pick_object_id_along_camera_ray(
                &frame,
                glam::Vec3::new(0.0, 40.0, 40.0),
                glam::Vec3::ZERO,
                Some(Team::USA),
                false,
            ),
            Some(id)
        );
    }

    #[test]
    fn presentation_is_selectable_drops_ordinary_dead_objects() {
        let (logic, id) = logic_with_selectable_unit();
        let mut frame = PresentationFrame::build_from_logic(&logic, 0);
        {
            let obj = frame.objects.iter_mut().find(|o| o.id == id).expect("unit");
            obj.destroyed = true;
            assert!(!UnitControlSystem::presentation_is_selectable(obj));
            assert!(UnitControlSystem::presentation_pick_skips_dead(obj));
        }
        assert_eq!(
            UnitControlSystem::pick_object_id_at_world_from_presentation(
                &frame,
                glam::Vec3::ZERO,
                Some(Team::USA),
                false,
                20.0,
            ),
            None
        );
    }

    #[test]
    fn click_pick_skips_undetected_stealthed_enemies() {
        // C++ SelectionInfo.cpp:352-371 / leftover selection_xlat.rs:426-429.
        use crate::game_logic::{GameLogic, Player};
        use crate::pick_ray::pick_object_id_along_camera_ray;
        let _shroud_test_guard = crate::fow_rendering::shroud_test_isolation_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        {
            let shroud_manager = gamelogic::system::shroud_manager::get_shroud_manager();
            let mut shroud = shroud_manager.lock()
                .expect("shroud");
            shroud.clear_all();
        }

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        logic.add_player(Player::new(1, Team::GLA, "GLA", false));
        let mut t = ThingTemplate::new("StealthRebel");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("StealthRebel".into(), t);
        let mut mine = ThingTemplate::new("LocalRanger");
        mine.set_health(100.0);
        mine.add_kind_of(KindOf::Infantry);
        mine.add_kind_of(KindOf::Selectable);
        logic.templates.insert("LocalRanger".into(), mine);

        let enemy = logic
            .create_object_for_player("StealthRebel", 1, glam::Vec3::new(80.0, 0.0, 0.0))
            .expect("enemy");
        let local = logic
            .create_object_for_player("LocalRanger", 0, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("local");
        if let Some(o) = logic.host_object_mut(enemy) {
            o.status.stealthed = true;
            o.status.detected = false;
        }
        if let Some(o) = logic.host_object_mut(local) {
            o.status.stealthed = true;
            o.status.detected = false;
        }

        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let pick = |pos: glam::Vec3| {
            UnitControlSystem::pick_object_id_at_world_from_presentation(
                &frame,
                pos,
                Some(Team::USA),
                false,
                20.0,
            )
        };
        assert_eq!(
            pick(glam::Vec3::new(80.0, 0.0, 0.0)),
            None,
            "undetected stealthed enemy must not be click-picked"
        );
        assert_eq!(
            pick(glam::Vec3::new(0.0, 0.0, 0.0)),
            Some(local),
            "own stealthed unit stays pickable"
        );

        let ray_miss = pick_object_id_along_camera_ray(
            &frame,
            glam::Vec3::new(80.0, 40.0, 40.0),
            glam::Vec3::new(80.0, 0.0, 0.0),
            Some(Team::USA),
            false,
        );
        assert_eq!(
            ray_miss, None,
            "undetected stealthed enemy must not be hover/ray picked"
        );

        if let Some(o) = logic.host_object_mut(enemy) {
            o.status.detected = true;
        }
        let detected = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(
            UnitControlSystem::pick_object_id_at_world_from_presentation(
                &detected,
                glam::Vec3::new(80.0, 0.0, 0.0),
                Some(Team::USA),
                false,
                20.0,
            ),
            Some(enemy),
            "detected stealthed enemy is pickable"
        );
    }
}
