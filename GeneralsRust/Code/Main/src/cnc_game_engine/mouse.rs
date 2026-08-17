#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::input::MouseInputOrigin;
use super::*;

/// Physical input metadata held only across the synchronous context-command
/// execution boundary. `issued_at` is part of the command's own immutable
/// fingerprint, so an unrelated queued/AI Gather event cannot be mistaken for
/// this physical input edge.
#[derive(Debug, Clone)]
struct PhysicalGatherAttempt {
    command_id: u32,
    issued_at: SystemTime,
    player_id: u32,
    target_id: ObjectId,
}

impl PhysicalGatherAttempt {
    fn from_context_click_command(
        command: &crate::command_system::GameCommand,
        origin: MouseInputOrigin,
        physical_context_gesture: bool,
    ) -> Option<Self> {
        if !matches!(origin, MouseInputOrigin::Physical) || !physical_context_gesture {
            return None;
        }
        let crate::command_system::CommandType::Gather { target_id } = &command.command_type else {
            return None;
        };
        Some(Self {
            command_id: command.command_id,
            issued_at: command.timestamp.clone(),
            player_id: command.player_id,
            target_id: *target_id,
        })
    }

    fn matches(&self, event: &crate::game_logic::AcceptedGatherCommand) -> bool {
        self.command_id == event.command_id
            && self.issued_at == event.issued_at
            && self.player_id == event.player_id
            && self.target_id == event.target_id
    }
}

/// C++ `SelectionInfo::contextCommandForNewSelection` only lets the
/// prefer-selection modifier bypass a classic LMB context route for a locally
/// selectable target. Terrain and non-local targets still reach CommandXlat.
#[inline]
fn classic_left_context_action_allowed(
    has_selection: bool,
    shift_down: bool,
    target_is_locally_selectable: bool,
) -> bool {
    has_selection && (!shift_down || !target_is_locally_selectable)
}

// C++ `W3DView::deviceToWorld` equivalent for the active Main camera.  Input
// must be projected through the same view/projection pair used by WGPU rather
// than treating the window as a linear minimap.
const PICK_RAY_EPSILON: f32 = 1.0e-5;
const PICK_TERRAIN_STEPS: usize = 96;
const PICK_TERRAIN_BISECTION_STEPS: usize = 12;

fn unproject_mouse_ray(
    view_matrix: Mat4,
    projection_matrix: Mat4,
    mouse_position: (f32, f32),
    viewport_width: f32,
    viewport_height: f32,
) -> Option<(Vec3, Vec3)> {
    let width = viewport_width.max(1.0);
    let height = viewport_height.max(1.0);
    let ndc_x = (mouse_position.0 / width).clamp(0.0, 1.0) * 2.0 - 1.0;
    let ndc_y = 1.0 - (mouse_position.1 / height).clamp(0.0, 1.0) * 2.0;
    let inverse = (projection_matrix * view_matrix).inverse();
    if !inverse.is_finite() {
        return None;
    }

    // Main's WGPU projection uses depth [0, 1], matching the selection-overlay
    // unprojection path.  Retain the signed perspective divide; using abs(w)
    // can mirror a point behind the camera into the playable world.
    let near_homogeneous = inverse * glam::Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far_homogeneous = inverse * glam::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    if near_homogeneous.w.abs() <= PICK_RAY_EPSILON || far_homogeneous.w.abs() <= PICK_RAY_EPSILON {
        return None;
    }
    let near = near_homogeneous.truncate() / near_homogeneous.w;
    let far = far_homogeneous.truncate() / far_homogeneous.w;
    (near.is_finite() && far.is_finite() && (far - near).length_squared() > PICK_RAY_EPSILON)
        .then_some((near, far))
}

/// Parameter interval in which a finite ray segment lies over the playable XZ
/// rectangle.  This prevents a terrain edge from being sampled for arbitrary
/// off-map screen rays.
fn ray_interval_in_world_xz(
    near: Vec3,
    far: Vec3,
    world_min: Vec3,
    world_max: Vec3,
) -> Option<(f32, f32)> {
    let direction = far - near;
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;
    for (origin, delta, min, max) in [
        (near.x, direction.x, world_min.x, world_max.x),
        (near.z, direction.z, world_min.z, world_max.z),
    ] {
        if delta.abs() <= PICK_RAY_EPSILON {
            if origin < min || origin > max {
                return None;
            }
            continue;
        }
        let a = (min - origin) / delta;
        let b = (max - origin) / delta;
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
        if enter > exit {
            return None;
        }
    }
    (exit >= 0.0 && enter <= 1.0).then_some((enter.clamp(0.0, 1.0), exit.clamp(0.0, 1.0)))
}

fn frozen_terrain_height(
    world_env: Option<&crate::presentation_frame::PresentationWorldEnv>,
    point: Vec3,
) -> f32 {
    world_env
        .and_then(|env| {
            env.sample_gameplay_terrain_height(point.x, point.z)
                .or_else(|| env.sample_height(point.x, point.z))
        })
        .unwrap_or(0.0)
}

/// Intersect the active camera ray with the frozen gameplay terrain.  The
/// height surface is sampled only from the presentation frame, preserving the
/// one-frame input/render boundary and avoiding a second mutable GameLogic read.
fn raycast_frozen_terrain(
    near: Vec3,
    far: Vec3,
    world_min: Vec3,
    world_max: Vec3,
    world_env: Option<&crate::presentation_frame::PresentationWorldEnv>,
) -> Option<Vec3> {
    let (start_t, end_t) = ray_interval_in_world_xz(near, far, world_min, world_max)?;
    let mut previous_t = start_t;
    let direction = far - near;
    let surface_delta = |t: f32| {
        let point = near + direction * t;
        point.y - frozen_terrain_height(world_env, point)
    };
    let mut previous_delta = surface_delta(previous_t);
    if !previous_delta.is_finite() {
        return None;
    }

    // A normal RTS camera starts above terrain.  March the bounded ray to find
    // the first downward surface crossing, then refine it.  This works for the
    // full frozen heightmap and degrades safely to its coarse snapshot/plane.
    for step in 1..=PICK_TERRAIN_STEPS {
        let t = start_t + (end_t - start_t) * step as f32 / PICK_TERRAIN_STEPS as f32;
        let delta = surface_delta(t);
        if !delta.is_finite() {
            return None;
        }
        if previous_delta >= 0.0 && delta <= 0.0 {
            let mut low = previous_t;
            let mut high = t;
            for _ in 0..PICK_TERRAIN_BISECTION_STEPS {
                let middle = (low + high) * 0.5;
                if surface_delta(middle) >= 0.0 {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            let point = near + direction * ((low + high) * 0.5);
            let ground = frozen_terrain_height(world_env, point);
            return ground
                .is_finite()
                .then_some(Vec3::new(point.x, ground, point.z));
        }
        previous_t = t;
        previous_delta = delta;
    }
    None
}

/// Camera-ray fallback when no frozen terrain snapshot exists yet (boot/load),
/// or an unusually shallow ray leaves the map before it reaches the terrain.
/// It is still camera-relative, never the old whole-map screen interpolation.
fn raycast_ground_plane_clamped(
    near: Vec3,
    far: Vec3,
    world_min: Vec3,
    world_max: Vec3,
    world_env: Option<&crate::presentation_frame::PresentationWorldEnv>,
) -> Option<Vec3> {
    let direction = far - near;
    if direction.y.abs() <= PICK_RAY_EPSILON {
        return None;
    }
    let t = -near.y / direction.y;
    if !t.is_finite() || t < 0.0 {
        return None;
    }
    let point = near + direction * t;
    let x = point.x.clamp(world_min.x, world_max.x);
    let z = point.z.clamp(world_min.z, world_max.z);
    let ground = frozen_terrain_height(world_env, Vec3::new(x, 0.0, z));
    ground.is_finite().then_some(Vec3::new(x, ground, z))
}

impl CnCGameEngine {
    pub(super) fn handle_left_click(&mut self) {
        self.is_dragging = true;
        self.selection_start = Some(self.mouse_world_position);
        self.selection_start_screen = Some(self.mouse_position);
        self.left_click_release_behavior = LeftMouseReleaseBehavior::Selection;

        let mouse_pos = self.mouse_world_position;
        let clicked_object = self.find_object_at_position(mouse_pos, false);

        // C++ GameClient.cpp:276-280 attach order (lower number first):
        // PlaceEventTranslator 30, GUICommandTranslator 40, SelectionTranslator
        // 50, CommandTranslator 70.  Both Place and GUI own LMB in every
        // mouse layout and must outrank a stale double-click selection.
        if let Some(template) = self.pending_structure_placement.clone() {
            // Wall/fence residual: defer commit to left-release so drag can form a line.
            if !Self::is_wall_structure_template(&template) {
                // C++ structure placement residual: empty-ground click commits DozerConstruct.
                self.place_structure_from_ui(&template, mouse_pos);
                self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
            }
            return;
        }
        if self.pending_map_command.is_some() {
            self.commit_pending_map_command(mouse_pos, clicked_object);
            self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
            return;
        }

        // Check for double-click
        let now = Instant::now();
        let is_double_click = if let (Some(last_time), Some(last_pos)) =
            (self.last_click_time, self.last_click_position)
        {
            let time_delta = now.duration_since(last_time).as_millis();
            let pos_delta = (mouse_pos - last_pos).length();
            time_delta < 500 && pos_delta < 10.0
        } else {
            false
        };

        self.last_click_time = Some(now);
        self.last_click_position = Some(mouse_pos);

        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
        let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));

        if is_double_click && clicked_object.is_some() && !ctrl_down {
            // Double-click: select all similar units
            if let Some(object_id) = clicked_object {
                self.select_similar_units(object_id);
            }
            self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
            return;
        }

        let had_selection = !self.ui_selected_ids(self.current_player_id).is_empty();
        if !self.use_alternate_mouse && ctrl_down && had_selection {
            // Classic layout: CommandXlat consumes Ctrl+LMB as force attack.
            // Alternate layout leaves force attack on its RMB context route.
            self.issue_force_attack_from_left_click(mouse_pos, clicked_object);
            self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
            return;
        }

        let target_is_locally_selectable = clicked_object
            .is_some_and(|object_id| self.is_locally_selectable_click_target(object_id));
        if !self.use_alternate_mouse
            && classic_left_context_action_allowed(
                had_selection,
                shift_down,
                target_is_locally_selectable,
            )
        {
            // In C++'s classic layout, LMB first gives CommandXlat an
            // opportunity to issue a context command.  Defer until release so
            // a box drag remains SelectionXlat-owned. Shift is only a
            // selection override for a locally selectable target: C++
            // SelectionInfo still lets Shift+LMB issue a context action on
            // terrain, enemy, allied, civilian, or crate targets. If the
            // context probe yields no command, `handle_left_release` falls
            // through to ordinary selection of the clicked drawable.
            self.left_click_release_behavior = LeftMouseReleaseBehavior::ContextCommand;
            return;
        }

        if let Some(object_id) = clicked_object {
            self.select_left_click_target(object_id, shift_down);
        }
        // Empty-ground selection clear remains deferred until left-release so
        // a potential box drag is not destroyed on its press edge.
    }

    /// Apply the selection half of C++ `SelectionXlat` for a point click that
    /// was not consumed by a command/placement path.
    fn select_left_click_target(&mut self, object_id: ObjectId, shift_down: bool) {
        if shift_down && self.is_locally_selectable_click_target(object_id) {
            // C++ Shift+select residual: toggle only locally owned units.
            // Enemy/civilian/allied point clicks always replace (SelectionXlat.cpp:679-693).
            self.toggle_select_object(object_id);
            return;
        }

        if !self.is_point_selectable_click_target(object_id) {
            return;
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, vec![object_id]);
        self.play_sound_effect(SoundType::Select);
    }

    /// Whether the frozen object is a locally owned target that SelectionXlat
    /// may select. This mirrors the point-selection predicate used both for a
    /// normal LMB selection and for classic Shift+LMB's prefer-selection
    /// override.
    fn is_locally_selectable_click_target(&self, object_id: ObjectId) -> bool {
        // Wave 1104: belt-and-suspenders local selectable check (pick peels FOW first).
        self.last_presentation_frame.as_ref().is_some_and(|frame| {
            frame.objects.iter().any(|o| {
                o.id == object_id
                    && frame.is_owned_by_local(o)
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
            })
        })
    }

    /// C++ `CanSelectDrawable` + lone enemy/civilian/allied point select
    /// (`SelectionXlat.cpp:181-189`, `679-693`). Drag-select stays local-only.
    fn is_point_selectable_click_target(&self, object_id: ObjectId) -> bool {
        self.last_presentation_frame.as_ref().is_some_and(|frame| {
            frame.objects.iter().any(|o| {
                o.id == object_id
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
                    && (frame.is_owned_by_local(o) || o.fow_visibility.visibility_alpha >= 0.95)
            })
        })
    }

    /// Shift+click residual: add friendly unit or remove if already selected.
    pub(super) fn toggle_select_object(&mut self, object_id: ObjectId) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        // Only toggle friendly selectable units (enemy click under Shift still replaces? retail
        // keeps multi-select among friendlies; enemy under Shift is ignored for add).
        let is_friendly_selectable = frame
            .objects
            .iter()
            .find(|o| o.id == object_id)
            .map(|o| {
                frame.is_owned_by_local(o)
                    && !o.destroyed
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
            })
            .unwrap_or(false);
        if !is_friendly_selectable {
            return;
        }

        let mut selection = self.selected_objects.clone();
        if let Some(idx) = selection.iter().position(|id| *id == object_id) {
            selection.remove(idx);
        } else {
            selection.push(object_id);
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, selection);
        self.play_sound_effect(SoundType::Select);
    }

    /// Ctrl+LMB ForceAttack residual (object or ground).
    /// Wave 612: via `host_issue_force_attack_from_left_click`.
    pub(super) fn issue_force_attack_from_left_click(
        &mut self,
        location: Vec3,
        target_object: Option<ObjectId>,
    ) {
        // Wave 612: thin wrapper — residual via host helper.
        self.host_issue_force_attack_from_left_click(location, target_object)
    }

    /// Ctrl+LMB ForceAttack residual (object or ground).
    pub(super) fn host_issue_force_attack_from_left_click(
        &mut self,
        location: Vec3,
        target_object: Option<ObjectId>,
    ) {
        // Wave 612: host residual helper.
        // Wave 234: selection prefers engine/presentation freeze.
        let mut selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            return;
        }

        let command_type = if let Some(tid) = target_object {
            crate::command_system::CommandType::ForceAttackObject { target_id: tid }
        } else {
            crate::command_system::CommandType::ForceAttackGround { location }
        };
        self.host_queue_command(crate::command_system::GameCommand {
            command_type,
            player_id: self.current_player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: selected,
            modifier_keys: crate::command_system::ModifierKeys {
                ctrl: true,
                shift: false,
                alt: false,
            },
        });
        self.host_process_commands_with_command_sound();
    }

    pub(super) fn select_similar_units(&mut self, clicked_object_id: ObjectId) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let player_team = frame.local_team();
        let similar_units = frame.similar_unit_ids(clicked_object_id, player_team);
        let template_label = frame
            .objects
            .iter()
            .find(|o| o.id == clicked_object_id)
            .map(|o| o.template_name.clone())
            .unwrap_or_default();

        if !similar_units.is_empty() {
            // Wave 583: selection residual via host_set_selection.
            self.host_set_selection(self.current_player_id, similar_units);
            self.play_sound_effect(SoundType::Select);
            info!(
                "Selected {} similar units ({})",
                self.selected_objects.len(),
                template_label
            );
        }
    }

    pub(super) fn handle_left_release(
        &mut self,
        origin: MouseInputOrigin,
        physical_lmb_gesture: bool,
    ) {
        self.is_dragging = false;
        let release_behavior = std::mem::replace(
            &mut self.left_click_release_behavior,
            LeftMouseReleaseBehavior::Selection,
        );
        let selection_start_screen = self.selection_start_screen.take();

        let Some(start) = self.selection_start.take() else {
            return;
        };

        let end = self.mouse_world_position;
        let selection_end_screen = glam::Vec2::new(self.mouse_position.0, self.mouse_position.1);
        let drag_distance_screen = selection_start_screen
            .map(|start_screen| {
                glam::Vec2::new(
                    selection_end_screen.x - start_screen.0,
                    selection_end_screen.y - start_screen.1,
                )
                .length()
            })
            .unwrap_or_default();

        // A map-target, structure placement, force attack, or double-click
        // selection already consumed the press edge.  C++'s higher-priority
        // translators suppress the corresponding release so it cannot also
        // clear selection or issue a second world action.
        if release_behavior == LeftMouseReleaseBehavior::Suppress {
            return;
        }

        if release_behavior == LeftMouseReleaseBehavior::ContextCommand
            && drag_distance_screen <= 2.0
        {
            let had_selection = !self.ui_selected_ids(self.current_player_id).is_empty();
            let issued = self.handle_left_context_click(origin, physical_lmb_gesture);
            self.interactive_playability.note_gameplay_order(
                matches!(origin, MouseInputOrigin::Physical) && !self.runtime_host_headless,
                had_selection && issued,
            );

            if !issued {
                // Classic C++ `SelectionInfo::contextCommandForNewSelection`
                // returns false for a drawable with no actionable context
                // command.  That exact fallthrough is what lets LMB replace
                // selection instead of moving selected units into a friendly
                // object under the cursor.
                if let Some(object_id) = self.find_object_at_position(end, false) {
                    let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
                    self.select_left_click_target(object_id, shift_down);
                }
            }
            return;
        }

        // C++ starts an area selection from a pixel delta and dispatches an
        // IRegion2D on release.  Terrain-ray distance changes with camera
        // pitch and must not decide whether a mouse drag was a click.
        if drag_distance_screen <= 2.0 {
            // Wall residual: short click places a single segment.
            if let Some(template) = self.pending_structure_placement.clone() {
                if Self::is_wall_structure_template(&template) {
                    self.place_structure_from_ui(&template, end);
                    return;
                }
            }
            // Click on empty ground (no pending command/placement handled on press): clear selection.
            if self.pending_map_command.is_none()
                && self.pending_structure_placement.is_none()
                && self.find_object_at_position(end, false).is_none()
            {
                let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
                if !shift_down {
                    self.selected_objects.clear();
                    // Wave 583: clear selection residual via host_set_selection.
                    self.host_set_selection(self.current_player_id, Vec::new());
                }
            }
            return;
        }

        // Wall/fence drag residual: DozerConstructLine along the drag segment.
        if let Some(template) = self.pending_structure_placement.clone() {
            if Self::is_wall_structure_template(&template) {
                self.place_wall_line_from_ui(&template, start, end);
                return;
            }
        }

        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));

        let mut selection: Vec<ObjectId> = if shift_down {
            self.selected_objects.clone()
        } else {
            Vec::new()
        };

        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let player_team = frame.local_team();
        let window_size = self.window.inner_size();
        let boxed: Vec<ObjectId> = selection_start_screen
            .map(|start_screen| {
                frame.box_select_unit_ids_in_screen_rect(
                    player_team,
                    self.view_matrix,
                    self.projection_matrix,
                    glam::Vec2::new(start_screen.0, start_screen.1),
                    selection_end_screen,
                    glam::Vec2::new(window_size.width as f32, window_size.height as f32),
                )
            })
            .unwrap_or_default();
        for id in boxed {
            if !selection.contains(&id) {
                selection.push(id);
            }
        }

        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, selection);
        self.play_sound_effect(SoundType::Select);
    }

    /// Issue a context-sensitive command through Alternate Mouse's RMB route.
    ///
    /// The actual command implementation is shared with the classic-layout
    /// LMB route below; only the physical button policy differs.
    pub(super) fn handle_right_click(
        &mut self,
        origin: MouseInputOrigin,
        physical_rmb_gesture: bool,
    ) -> bool {
        self.handle_context_click(origin, physical_rmb_gesture)
    }

    /// Issue a context-sensitive command through the classic-layout LMB
    /// route.  C++ `CommandXlat` treats this as the same logical context
    /// command as Alternate Mouse's RMB path.
    pub(super) fn handle_left_context_click(
        &mut self,
        origin: MouseInputOrigin,
        physical_lmb_gesture: bool,
    ) -> bool {
        self.handle_context_click(origin, physical_lmb_gesture)
    }

    /// Issue one logical C++ `evaluateContextCommand` action.  Physical gather
    /// evidence is threaded through this exact command path, but only becomes
    /// tracked after GameLogic reports the Gather command actually accepted its
    /// carrier IDs.
    fn handle_context_click(
        &mut self,
        origin: MouseInputOrigin,
        physical_context_gesture: bool,
    ) -> bool {
        let mouse_pos = self.mouse_world_position;

        // Prefer live player selection; fall back to engine selection residual.
        // Wave 234: selection prefers engine/presentation freeze.
        let selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            return false;
        }

        // C++ context-sensitive click residual via CommandSystem:
        // attack / gather / repair / enter / get-repaired / get-healed / move / attack-move.
        let target_object = self.find_object_at_position(mouse_pos, true);
        let ctrl = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Control)
            )
        });
        let shift = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Shift)
            )
        });
        let alt = self.sticky_waypoint_mode
            || self.keys_pressed.iter().any(|k| {
                matches!(
                    k,
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Alt)
                )
            });

        let context = crate::command_system::MouseCommandContext {
            world_position: mouse_pos,
            target_object,
            target_presentation: target_object.and_then(|id| self.presentation_target_hint(id)),
            selected_presentation: self.presentation_selected_unit_hints(&selected),
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
            viewport_size: None,
            world_min: None,
            world_max: None,
            // CommandSystem's `Right` arm represents its logical context
            // command, not the literal OS button.  C++ chooses the physical
            // button in CommandXlat before evaluating this shared action.
            mouse_button: crate::command_system::MouseButton::Right,
            modifier_keys: crate::command_system::ModifierKeys { ctrl, shift, alt },
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let mut cmd_sys = crate::command_system::CommandSystem::new();
        // Wave 236: presentation-only mouse path when frame installed.
        let command = cmd_sys.process_mouse_input(
            &context,
            &selected,
            self.current_player_id,
            self.presentation_mouse_game_logic(),
        );

        if let Some(mut command) = command {
            let crate_pickup = target_object
                .and_then(|id| self.presentation_target_hint(id))
                .is_some_and(|hint| hint.is_crate && !hint.is_salvage_crate);
            let target_had_no_context_action = target_object.is_some()
                && matches!(
                    &command.command_type,
                    crate::command_system::CommandType::MoveTo { .. }
                )
                && !crate_pickup
                && !matches!(
                    &command.command_type,
                    crate::command_system::CommandType::DoSalvage { .. }
                );
            if target_had_no_context_action {
                // C++ `evaluateContextCommand` returns MSG_INVALID rather
                // than a move when a drawable was under the cursor but had no
                // actionable relationship.  Classic LMB then falls back to
                // selection; Alternate RMB simply does nothing.
                // Crate clicks are the exception: MSG_DO_SALVAGE / move-to-crate.
                return false;
            }
            if self.sticky_auto_attack {
                if let crate::command_system::CommandType::MoveTo { destination, .. } =
                    command.command_type
                {
                    command.command_type = crate::command_system::CommandType::AttackMoveTo {
                        destination,
                        max_shots: -1,
                    };
                }
            }
            self.host_queue_and_process_context_click_command(
                command,
                origin,
                physical_context_gesture,
            );
            return true;
        }

        // A drawable with no accepted context action is deliberately not
        // converted to a move.  The only C++ fallback move is terrain (no
        // drawable), and that distinction is what lets classic LMB select a
        // friendly object after a failed context probe.
        if target_object.is_some() {
            return false;
        }

        // Fail-closed terrain fallback residual: move if the context path did
        // not synthesize a command.
        if self.sticky_auto_attack {
            self.host_command_attack_move(self.current_player_id, mouse_pos);
        } else {
            self.host_command_move(self.current_player_id, mouse_pos);
        }
        self.play_sound_effect(SoundType::Command);
        true
    }

    /// C++ SelectionXlat.cpp:1007-1023 sees a right-button click before
    /// CommandXlat.  An armed GUI command is cancelled without deselect;
    /// a pending place still deselects the builder (place source != 0)
    /// in both mouse layouts.  That click must never become a context
    /// command merely because Main owns direct OS input.
    pub(super) fn cancel_world_mouse_targeting(&mut self) -> bool {
        if self.pending_map_command.take().is_some() {
            self.clear_radius_cursor_overlays();
            let msg = "Cancelled pending command";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return true;
        }
        if self.pending_structure_placement.is_some() {
            self.deselect_world_selection_from_right_click();
            self.cancel_structure_placement_from_ui();
            return true;
        }
        false
    }

    /// C++ SelectionXlat deselects on a short classic-layout RMB click when
    /// no GUI/build target mode owns that click.  RMB drag remains LookAtXlat
    /// scrolling and is filtered by the caller before this method runs.
    pub(super) fn deselect_world_selection_from_right_click(&mut self) {
        if !self.ui_selected_ids(self.current_player_id).is_empty() {
            self.host_set_selection(self.current_player_id, Vec::new());
        }
    }

    /// Run the normal synchronous command authority path, then bind a physical
    /// Gather attempt to its executor-confirmed carrier subset. Injected,
    /// runtime-host, and AI commands have no physical attempt and are consumed
    /// without ever entering `physical_gather_carrier_ids`.
    fn host_queue_and_process_context_click_command(
        &mut self,
        command: crate::command_system::GameCommand,
        origin: MouseInputOrigin,
        physical_context_gesture: bool,
    ) {
        let physical_attempt = PhysicalGatherAttempt::from_context_click_command(
            &command,
            origin,
            physical_context_gesture,
        );
        self.host_queue_and_process_command(command);

        // `host_queue_and_process_command` is synchronous. Always consume the
        // transient events, even for nonphysical clicks, so background/AI
        // Gather traffic cannot accumulate or be matched by a later input.
        let accepted_gathers = self.host_game_logic_mut().take_accepted_gather_commands();
        let Some(attempt) = physical_attempt else {
            return;
        };
        if !self.host_physical_gather_evidence_eligible()
            || attempt.player_id != self.local_player_id_for_ui()
        {
            return;
        }

        for event in accepted_gathers {
            if attempt.matches(&event) {
                // `execute_gather` emitted only workers selected by this
                // command whose local-team Gather path was accepted.
                self.physical_gather_carrier_ids.extend(event.carrier_ids);
            }
        }
    }

    /// Consume economy events only from the real ReturningResources deposit
    /// branch. A resource-total delta, passive income, scripted income, or an
    /// untracked/remote carrier is deliberately insufficient.
    pub(super) fn host_drain_physical_gather_dropoffs(&mut self) {
        // Clear any non-input Gather acceptances that arose during simulation;
        // a physical context-click path consumes its own event synchronously above.
        let _ = self.host_game_logic_mut().take_accepted_gather_commands();
        let dropoffs = self.host_game_logic_mut().take_supply_dropoff_events();
        if dropoffs.is_empty() || !self.host_physical_gather_evidence_eligible() {
            return;
        }

        let local_player_id = self.local_player_id_for_ui();
        for dropoff in dropoffs {
            let is_tracked_local_deposit = dropoff.carried_amount > 0
                && dropoff.player_id == local_player_id
                && self
                    .physical_gather_carrier_ids
                    .contains(&dropoff.carrier_id);
            self.interactive_playability
                .note_physical_gather_resources(is_tracked_local_deposit);
        }
    }

    /// A physical Gather proof is valid only in a visible, non-headless,
    /// offline match. This intentionally does not infer input provenance from
    /// `CommandSourceType::FromUser` or a runtime-host command name.
    fn host_physical_gather_evidence_eligible(&self) -> bool {
        !self.runtime_host_headless
            && self.runtime_host_window_visible()
            && matches!(self.current_state, GameState::InGame)
            && matches!(
                self.host_match_game_mode,
                Some(
                    crate::game_logic::GameMode::SinglePlayer
                        | crate::game_logic::GameMode::Skirmish
                )
            )
    }

    pub(super) fn handle_mouse_wheel(&mut self, delta: &winit::event::MouseScrollDelta) {
        use winit::event::MouseScrollDelta;

        let delta_y = match delta {
            MouseScrollDelta::LineDelta(_, y) => *y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
        };

        #[cfg(feature = "game_client")]
        self.inject_game_client_mouse_scroll(delta_y);

        // C++ place-building rotate residual: wheel turns ghost while placement armed.
        if self.pending_structure_placement.is_some() {
            let step = delta_y * std::f32::consts::FRAC_PI_4; // 45 deg per notch
            self.game_hud
                .construction_panel
                .rotate_structure_placement(-step);
            self.ui_manager
                .game_hud_mut()
                .construction_panel
                .rotate_structure_placement(-step);
            return;
        }

        // Zoom camera with mouse wheel
        let zoom_speed = 0.1;
        let new_zoom = (self.camera_zoom - delta_y * zoom_speed).clamp(0.1, 5.0);

        if (new_zoom - self.camera_zoom).abs() > 0.001 {
            self.camera_zoom = new_zoom;
            // `W3DView::setZoom` immediately rebuilds the camera transform.
            self.apply_camera_orbit_transform();
            if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                self.update_mouse_world_position();
                self.sync_context_mouse_cursor();
            }
            debug!("Camera zoom changed to {:.2}", self.camera_zoom);
        }
    }

    pub(super) fn update_camera(&mut self, dt: f32) {
        let initial_zoom = self.camera_zoom;
        let initial_pitch = self.camera_pitch_radians;
        let initial_yaw = self.camera_yaw_radians;
        // Retail KP4/KP6 rotate and KP8/KP2 zoom hold residual.
        const ROTATE_RAD_PER_SEC: f32 = 1.2;
        const ZOOM_PER_SEC: f32 = 0.85;
        if self.camera_rotate_left_held {
            self.camera_yaw_radians -= ROTATE_RAD_PER_SEC * dt;
        }
        if self.camera_rotate_right_held {
            self.camera_yaw_radians += ROTATE_RAD_PER_SEC * dt;
        }
        if self.camera_zoom_in_held {
            self.camera_zoom = (self.camera_zoom - ZOOM_PER_SEC * dt).clamp(0.1, 5.0);
        }
        if self.camera_zoom_out_held {
            self.camera_zoom = (self.camera_zoom + ZOOM_PER_SEC * dt).clamp(0.1, 5.0);
        }

        self.update_camera_tracking_drawable();

        let mut movement = Vec3::ZERO;
        if self.camera_slave_mode.is_none() {
            let logic_frames_per_second =
                game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32;
            let (
                horizontal_scroll_speed_factor,
                vertical_scroll_speed_factor,
                keyboard_scroll_factor,
            ) = {
                let global_data = game_engine::common::global_data::read();
                (
                    global_data.horizontal_scroll_speed_factor,
                    global_data.vertical_scroll_speed_factor,
                    global_data.keyboard_scroll_factor,
                )
            };

            // C++ parity (LookAtXlat.cpp): key scrolling uses SCROLL_AMT=100 in screen-space and
            // applies horizontal/vertical/keyboard factors once per logic frame.
            const SCROLL_AMT: f32 = 100.0;
            let scroll_step =
                SCROLL_AMT * keyboard_scroll_factor * dt.max(0.0) * logic_frames_per_second;
            let mut screen_scroll = Vec2::ZERO;
            // C++ LookAt keyboard scroll uses arrows (not WASD).
            // WASD are unit hotkeys: A attack-move, S stop, D deploy, etc.
            let mods_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control))
                || self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                || self.keys_pressed.contains(&Key::Named(NamedKey::Alt));
            let ui_modal = self.chat_panel.is_open() || self.diplomacy_panel.is_active();
            if !mods_down && !ui_modal {
                if self.keys_pressed.contains(&Key::Named(NamedKey::ArrowUp)) {
                    screen_scroll.y -= vertical_scroll_speed_factor * scroll_step;
                }
                if self.keys_pressed.contains(&Key::Named(NamedKey::ArrowDown)) {
                    screen_scroll.y += vertical_scroll_speed_factor * scroll_step;
                }
                if self.keys_pressed.contains(&Key::Named(NamedKey::ArrowLeft)) {
                    screen_scroll.x -= horizontal_scroll_speed_factor * scroll_step;
                }
                if self
                    .keys_pressed
                    .contains(&Key::Named(NamedKey::ArrowRight))
                {
                    screen_scroll.x += horizontal_scroll_speed_factor * scroll_step;
                }
            }

            // Edge scrolling (C++ LookAt.cpp: near screen edge).
            // Enable for windowed + fullscreen so map-panning works without arrows.
            // Headless runtime-host residual: mouse stays at (0,0) without OS cursor
            // events, which would permanently edge-scroll the camera off the map.
            if matches!(self.current_state, GameState::InGame | GameState::Paused)
                && !self.runtime_host_headless
                && !self.chat_panel.is_open()
                && !self.diplomacy_panel.is_active()
            {
                const EDGE_SCROLL_SIZE: f32 = 5.0;
                let (mx, my) = self.mouse_position;
                let size = self.window.inner_size();
                let win_w = size.width as f32;
                let win_h = size.height as f32;

                let mut edge_dx = 0.0f32;
                let mut edge_dy = 0.0f32;

                if mx < EDGE_SCROLL_SIZE {
                    edge_dx = -1.0;
                } else if mx >= win_w - EDGE_SCROLL_SIZE {
                    edge_dx = 1.0;
                }
                if my < EDGE_SCROLL_SIZE {
                    edge_dy = -1.0;
                } else if my >= win_h - EDGE_SCROLL_SIZE {
                    edge_dy = 1.0;
                }

                if edge_dx != 0.0 || edge_dy != 0.0 {
                    let edge_step =
                        SCROLL_AMT * keyboard_scroll_factor * dt.max(0.0) * logic_frames_per_second;
                    screen_scroll.x += edge_dx * horizontal_scroll_speed_factor * edge_step;
                    screen_scroll.y += edge_dy * vertical_scroll_speed_factor * edge_step;
                }
            }

            // Right-mouse-button drag scrolling (C++ LookAtXlat.cpp:378-406)
            if self.is_rmb_scrolling {
                if let Some(mut anchor) = self.rmb_scroll_anchor {
                    let size = self.window.inner_size();
                    crate::cnc_game_engine::options_bridge::clamp_move_rmb_scroll_anchor(
                        &mut anchor,
                        self.mouse_position,
                        (size.width as f32, size.height as f32),
                        self.move_rmb_scroll_anchor,
                    );
                    self.rmb_scroll_anchor = Some(anchor);
                    let dx = self.mouse_position.0 - anchor.0;
                    let dy = self.mouse_position.1 - anchor.1;
                    let mut offset = Vec2::new(
                        horizontal_scroll_speed_factor * dx,
                        vertical_scroll_speed_factor * dy,
                    );

                    if offset.length_squared() > f32::EPSILON {
                        let direction = offset.normalize();
                        offset.x += horizontal_scroll_speed_factor
                            * direction.x
                            * keyboard_scroll_factor.powi(2);
                        offset.y += vertical_scroll_speed_factor
                            * direction.y
                            * keyboard_scroll_factor.powi(2);
                        screen_scroll += offset * dt.max(0.0) * logic_frames_per_second;
                    }
                }
            }

            // Middle-mouse-button camera yaw rotation (C++ LookAtXlat.cpp)
            if self.is_mmb_rotating {
                if let Some(anchor) = self.mmb_anchor {
                    let dx = self.mouse_position.0 - anchor.0;
                    self.camera_yaw_radians += dx * 0.005;
                }
                self.mmb_anchor = Some(self.mouse_position);
            }

            movement = self.camera_scroll_world_delta(screen_scroll);
        }

        let mut camera_changed = false;

        if movement.length() > 0.0 {
            self.camera_target += movement;
            camera_changed = true;
        }

        if let Some(mode) = self.camera_slave_mode.as_ref() {
            // Prefer dual-tick presentation pose so camera follow does not re-read live transforms.
            let target = if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame.first_alive_position_for_template(&mode.thing_template_name)
            } else {
                // Presentation required (no live get_objects dual-read).
                None
            };
            if let Some(target) = target {
                let clamped = self.clamp_to_world_bounds(target);
                if (self.camera_target.x - clamped.x).abs() > 0.001
                    || (self.camera_target.z - clamped.z).abs() > 0.001
                {
                    self.camera_target.x = clamped.x;
                    self.camera_target.z = clamped.z;
                    camera_changed = true;
                }
            }
        }

        if let Some(target) = self.camera_zoom_target {
            if self.camera_zoom_duration <= 0.0 {
                self.camera_zoom = target;
                self.camera_zoom_target = None;
            } else {
                self.camera_zoom_elapsed += dt;
                let t = (self.camera_zoom_elapsed / self.camera_zoom_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_zoom_ease_in / self.camera_zoom_duration,
                    self.camera_zoom_ease_out / self.camera_zoom_duration,
                );
                self.camera_zoom =
                    self.camera_zoom_start + (target - self.camera_zoom_start) * eased;
                if t >= 1.0 {
                    self.camera_zoom_target = None;
                }
            }
        }

        if let Some(target) = self.camera_pitch_target {
            if self.camera_pitch_duration <= 0.0 {
                self.camera_pitch_radians = target;
                self.camera_pitch_target = None;
                camera_changed = true;
            } else {
                self.camera_pitch_elapsed += dt;
                let t = (self.camera_pitch_elapsed / self.camera_pitch_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_pitch_ease_in / self.camera_pitch_duration,
                    self.camera_pitch_ease_out / self.camera_pitch_duration,
                );
                self.camera_pitch_radians =
                    self.camera_pitch_start + (target - self.camera_pitch_start) * eased;
                camera_changed = true;
                if t >= 1.0 {
                    self.camera_pitch_target = None;
                }
            }
        }

        if let Some(target) = self.camera_yaw_target {
            if self.camera_yaw_duration <= 0.0 {
                self.camera_yaw_radians = target;
                self.camera_yaw_target = None;
                camera_changed = true;
            } else {
                self.camera_yaw_elapsed += dt;
                let t = (self.camera_yaw_elapsed / self.camera_yaw_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_yaw_ease_in / self.camera_yaw_duration,
                    self.camera_yaw_ease_out / self.camera_yaw_duration,
                );
                self.camera_yaw_radians =
                    self.camera_yaw_start + (target - self.camera_yaw_start) * eased;
                camera_changed = true;
                if t >= 1.0 {
                    self.camera_yaw_target = None;
                }
            }
        }

        // Wave 250: prefer presentation freeze residual when a frame is installed.
        let shake_dt = if self.presentation_or_boot_time_frozen() {
            0.0
        } else {
            dt
        };
        if self.update_script_camera_shake(shake_dt) {
            camera_changed = true;
        }

        // Numpad/Middle rotation and scripted/wheel zoom all modify the same
        // W3D camera transform.  Previously only pan/shake paths set this
        // flag, leaving a visually stale view (and consequently stale picks).
        camera_changed |= (self.camera_zoom - initial_zoom).abs() > f32::EPSILON
            || (self.camera_pitch_radians - initial_pitch).abs() > f32::EPSILON
            || (self.camera_yaw_radians - initial_yaw).abs() > f32::EPSILON;

        // Several C++ camera entry points (minimap, selection hotkeys, and
        // scripted camera requests) update the target or zoom outside this
        // input routine.  Rebuild their W3D pose on the next frame as well;
        // otherwise the simulation state and the view/ray used for orders
        // disagree until the player happens to pan.
        camera_changed |= self.camera_transform_needs_rebuild();

        if camera_changed {
            self.apply_camera_orbit_transform();
            if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                self.update_mouse_world_position();
                self.sync_context_mouse_cursor();
            }
        }
    }

    pub(super) fn is_character_key_pressed(&self, expected: &str) -> bool {
        self.keys_pressed.iter().any(|key| match key {
            Key::Character(ch) => ch.eq_ignore_ascii_case(expected),
            _ => false,
        })
    }

    pub(super) fn camera_scroll_world_delta(&self, screen_scroll: Vec2) -> Vec3 {
        if screen_scroll.length_squared() <= f32::EPSILON {
            return Vec3::ZERO;
        }

        // Match C++ key-scroll semantics: "up/down/left/right" are screen-space intents.
        // Convert that intent to world-plane motion relative to current camera facing.
        let mut forward = self.camera_target - self.camera_position;
        forward.y = 0.0;
        if forward.length_squared() <= f32::EPSILON {
            return Vec3::ZERO;
        }
        let forward = forward.normalize();
        let right = Vec3::new(forward.z, 0.0, -forward.x);

        // C++ uses y- for UP and y+ for DOWN, so negate Y when mapping to forward motion.
        (right * screen_scroll.x) + (forward * -screen_scroll.y)
    }

    /// C++ InGameUI context cursor residual mapped onto winit CursorIcon.
    ///
    /// Fail-closed vs full Mouse.cpp ANI/CUR assets — uses platform icons with
    /// residual names from `MOUSE_CURSOR_INI_NAME_LIST`.
    pub(super) fn sync_context_mouse_cursor(&mut self) {
        use winit::window::CursorIcon;
        let (name, icon) = self.resolve_context_cursor_icon();
        if self.last_context_cursor == Some(name) {
            return;
        }
        self.last_context_cursor = Some(name);
        self.window.set_cursor(icon);
    }

    /// Wave 612: via `host_resolve_context_cursor_icon`.
    pub(super) fn resolve_context_cursor_icon(&self) -> (&'static str, winit::window::CursorIcon) {
        // Wave 612: thin wrapper — residual via host helper.
        self.host_resolve_context_cursor_icon()
    }

    pub(super) fn host_resolve_context_cursor_icon(
        &self,
    ) -> (&'static str, winit::window::CursorIcon) {
        // Wave 612: host residual helper.
        use winit::window::CursorIcon;

        // Placement mode residual.
        if self.pending_structure_placement.is_some() {
            let legal = self
                .game_hud
                .construction_panel
                .placement_preview()
                .is_legal;
            return if legal {
                ("Build", CursorIcon::Cell)
            } else {
                ("InvalidBuild", CursorIcon::NotAllowed)
            };
        }

        // Pending map command residual.
        if let Some(kind) = self.pending_map_command.as_ref() {
            return match kind {
                PendingMapCommand::AttackMove => ("AttackMove", CursorIcon::Crosshair),
                PendingMapCommand::Guard(_) => ("Move", CursorIcon::AllScroll),
                PendingMapCommand::SetRallyPoint => ("SetRallyPoint", CursorIcon::Cell),
                PendingMapCommand::CombatDrop(_) => ("CombatDrop", CursorIcon::Move),
                PendingMapCommand::PlaceBeacon => ("PlaceBeacon", CursorIcon::Cell),
                PendingMapCommand::SpecialPower(_) => ("Target", CursorIcon::Crosshair),
                PendingMapCommand::Weapon(_) => ("Target", CursorIcon::Crosshair),
                PendingMapCommand::UnitAbility(_) => ("Target", CursorIcon::Crosshair),
            };
        }

        // Wave 234: selection presence prefers engine/presentation freeze.
        let has_selection = !self.ui_selected_ids(self.current_player_id).is_empty();

        let hover = self.find_object_at_position(self.mouse_world_position, true);
        let ctrl = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        let alt = self.keys_pressed.contains(&Key::Named(NamedKey::Alt));

        if (alt || self.sticky_waypoint_mode) && has_selection {
            return ("Waypoint", CursorIcon::Cell);
        }

        if ctrl && has_selection {
            return if hover.is_some() {
                ("ForceAttackObj", CursorIcon::Crosshair)
            } else {
                ("ForceAttackGround", CursorIcon::Crosshair)
            };
        }

        if !has_selection {
            // Hover friendly selectable → Select residual.
            if let Some(id) = hover {
                let friendly = if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame
                        .objects
                        .iter()
                        .find(|o| o.id == id)
                        .map(|o| {
                            // Wave 1098: cursor Select residual uses full presentation
                            // selectable legality (sold/unselectable/masked/disabled).
                            frame.is_owned_by_local(o)
                                && crate::unit_control::UnitControlSystem::presentation_is_selectable(
                                    o,
                                )
                        })
                        .unwrap_or(false)
                } else {
                    // Wave 905: fail-closed without presentation freeze (no find_object dual-read).
                    false
                };
                if friendly {
                    return ("Select", CursorIcon::Pointer);
                }
            }
            return ("Normal", CursorIcon::Default);
        }

        // Has selection: context from CommandSystem residual.
        // Wave 229: selection via presentation-first ui_selected_ids.
        let selected = self.ui_selected_ids(self.current_player_id);
        let context = crate::command_system::MouseCommandContext {
            world_position: self.mouse_world_position,
            target_object: hover,
            target_presentation: hover.and_then(|id| self.presentation_target_hint(id)),
            selected_presentation: self.presentation_selected_unit_hints(&selected),
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: crate::command_system::MouseButton::Right,
            modifier_keys: crate::command_system::ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut cmd_sys = crate::command_system::CommandSystem::new();
        // Wave 236: presentation-only mouse path when frame installed.
        let cmd = cmd_sys.process_mouse_input(
            &context,
            &selected,
            self.current_player_id,
            self.presentation_mouse_game_logic(),
        );
        match cmd.map(|c| c.command_type) {
            Some(crate::command_system::CommandType::AttackObject { .. }) => {
                ("AttackObj", CursorIcon::Crosshair)
            }
            Some(crate::command_system::CommandType::ForceAttackObject { .. }) => {
                ("ForceAttackObj", CursorIcon::Crosshair)
            }
            Some(crate::command_system::CommandType::ForceAttackGround { .. }) => {
                ("ForceAttackGround", CursorIcon::Crosshair)
            }
            Some(crate::command_system::CommandType::Enter { .. }) => {
                ("EnterFriendly", CursorIcon::Copy)
            }
            Some(crate::command_system::CommandType::GetRepaired { .. })
            | Some(crate::command_system::CommandType::Repair { .. }) => {
                ("GetRepaired", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::ResumeConstruction { .. }) => {
                ("ResumeConstruction", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::CaptureBuilding { .. }) => {
                ("CaptureBuilding", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::MoveTo { .. })
            | Some(crate::command_system::CommandType::DoSalvage { .. })
            | Some(crate::command_system::CommandType::AttackMoveTo { .. }) => {
                ("Move", CursorIcon::AllScroll)
            }
            Some(crate::command_system::CommandType::AddWaypoint { .. }) => {
                ("Waypoint", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::Guard { .. }) => {
                ("Move", CursorIcon::AllScroll)
            }
            _ => {
                if hover.is_some() {
                    ("Select", CursorIcon::Pointer)
                } else {
                    ("Move", CursorIcon::AllScroll)
                }
            }
        }
    }

    /// Feed Main-owned OS keyboard state into GameClient Keyboard device residual.
    /// Main still owns command translation / hotkeys.
    /// Wave 606: via `host_inject_game_client_key`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_key(
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) {
        // Wave 606: thin wrapper — OS key inject via host helper.
        self.host_inject_game_client_key(physical_key, pressed);
    }

    /// Wave 606: host OS→GameClient key inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_key(
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) {
        // Wave 606: host OS key inject residual.
        if let Some(code) = Self::to_game_client_key_code(physical_key) {
            game_client::input::keyboard::with_keyboard(|kb| {
                let _ = kb.handle_key_simple(code, pressed);
            });
        }
    }

    /// Map winit physical keys to GameClient KeyCode without sharing winit types across crates.
    #[cfg(feature = "game_client")]
    pub(super) fn to_game_client_key_code(
        physical_key: &winit::keyboard::PhysicalKey,
    ) -> Option<game_client::input::KeyCode> {
        use game_client::input::KeyCode as Gk;
        use winit::keyboard::{KeyCode as Wk, PhysicalKey};
        let PhysicalKey::Code(code) = physical_key else {
            return None;
        };
        Some(match code {
            Wk::Escape => Gk::Escape,
            Wk::Enter | Wk::NumpadEnter => Gk::Enter,
            Wk::Space => Gk::Space,
            Wk::Tab => Gk::Tab,
            Wk::Backspace => Gk::Backspace,
            Wk::Delete => Gk::Delete,
            Wk::Home => Gk::Home,
            Wk::End => Gk::End,
            Wk::PageUp => Gk::PageUp,
            Wk::PageDown => Gk::PageDown,
            Wk::ArrowLeft => Gk::Left,
            Wk::ArrowRight => Gk::Right,
            Wk::ArrowUp => Gk::Up,
            Wk::ArrowDown => Gk::Down,
            Wk::ShiftLeft => Gk::LeftShift,
            Wk::ShiftRight => Gk::RightShift,
            Wk::ControlLeft => Gk::LeftCtrl,
            Wk::ControlRight => Gk::RightCtrl,
            Wk::AltLeft => Gk::LeftAlt,
            Wk::AltRight => Gk::RightAlt,
            Wk::KeyA => Gk::A,
            Wk::KeyB => Gk::B,
            Wk::KeyC => Gk::C,
            Wk::KeyD => Gk::D,
            Wk::KeyE => Gk::E,
            Wk::KeyF => Gk::F,
            Wk::KeyG => Gk::G,
            Wk::KeyH => Gk::H,
            Wk::KeyI => Gk::I,
            Wk::KeyJ => Gk::J,
            Wk::KeyK => Gk::K,
            Wk::KeyL => Gk::L,
            Wk::KeyM => Gk::M,
            Wk::KeyN => Gk::N,
            Wk::KeyO => Gk::O,
            Wk::KeyP => Gk::P,
            Wk::KeyQ => Gk::Q,
            Wk::KeyR => Gk::R,
            Wk::KeyS => Gk::S,
            Wk::KeyT => Gk::T,
            Wk::KeyU => Gk::U,
            Wk::KeyV => Gk::V,
            Wk::KeyW => Gk::W,
            Wk::KeyX => Gk::X,
            Wk::KeyY => Gk::Y,
            Wk::KeyZ => Gk::Z,
            Wk::Digit0 => Gk::Num0,
            Wk::Digit1 => Gk::Num1,
            Wk::Digit2 => Gk::Num2,
            Wk::Digit3 => Gk::Num3,
            Wk::Digit4 => Gk::Num4,
            Wk::Digit5 => Gk::Num5,
            Wk::Digit6 => Gk::Num6,
            Wk::Digit7 => Gk::Num7,
            Wk::Digit8 => Gk::Num8,
            Wk::Digit9 => Gk::Num9,
            Wk::F1 => Gk::F1,
            Wk::F2 => Gk::F2,
            Wk::F3 => Gk::F3,
            Wk::F4 => Gk::F4,
            Wk::F5 => Gk::F5,
            Wk::F6 => Gk::F6,
            Wk::F7 => Gk::F7,
            Wk::F8 => Gk::F8,
            Wk::F9 => Gk::F9,
            Wk::F10 => Gk::F10,
            Wk::F11 => Gk::F11,
            Wk::F12 => Gk::F12,
            _ => return None,
        })
    }

    /// Feed Main-owned OS mouse state into GameClient Mouse device residual.
    /// Main still owns command translation; this keeps client device state honest
    /// for presentation-shell UI without dual OS event ownership.
    /// Wave 606: via `host_inject_game_client_mouse_move`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_mouse_move(&self, x: f32, y: f32) {
        // Wave 606: thin wrapper — OS mouse move inject via host helper.
        self.host_inject_game_client_mouse_move(x, y);
    }

    /// Wave 606: host OS→GameClient mouse-move inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_mouse_move(&self, x: f32, y: f32) {
        // Wave 606: host OS mouse move inject residual.
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_mouse_move(x, y);
        });
    }

    /// Wave 606: via `host_inject_game_client_mouse_button`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_mouse_button(&self, button: MouseButton, pressed: bool) {
        // Wave 606: thin wrapper — OS mouse button inject via host helper.
        self.host_inject_game_client_mouse_button(button, pressed);
    }

    /// C++ WindowXlat: OS button → `TheWindowManager` gadget hit-test.
    /// Returns true when the WND consumed the click (shell active or gadget Used).
    /// C++ WindowXlat RAW_KEY → focused gadget `GWM_CHAR`.
    pub(super) fn dispatch_os_key_to_window_manager(
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::WindowInputReturnCode;
            let Some(vk) = Self::winit_physical_key_to_wnd_vk(physical_key) else {
                return false;
            };
            // C++ KEY_STATE_DOWN=0x02, KEY_STATE_UP=0x01
            let state = if pressed { 0x02 } else { 0x01 };
            game_client::gui::dispatch_os_key_to_window_manager(vk, state)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (physical_key, pressed);
            false
        }
    }

    pub(super) fn winit_physical_key_to_wnd_vk(
        physical_key: &winit::keyboard::PhysicalKey,
    ) -> Option<u8> {
        use winit::keyboard::{KeyCode as Wk, PhysicalKey};
        let PhysicalKey::Code(code) = physical_key else {
            return None;
        };
        Some(match code {
            Wk::Escape => 0x1B,
            Wk::Enter | Wk::NumpadEnter => 13,
            Wk::Space => 32,
            Wk::Tab => 9,
            Wk::Backspace => 8,
            Wk::Delete => 0x2E,
            Wk::Home => 36,
            Wk::End => 35,
            Wk::PageUp => 33,
            Wk::PageDown => 34,
            Wk::ArrowLeft => 37,
            Wk::ArrowUp => 38,
            Wk::ArrowRight => 39,
            Wk::ArrowDown => 40,
            Wk::KeyA => b'A',
            Wk::KeyB => b'B',
            Wk::KeyC => b'C',
            Wk::KeyD => b'D',
            Wk::KeyE => b'E',
            Wk::KeyF => b'F',
            Wk::KeyG => b'G',
            Wk::KeyH => b'H',
            Wk::KeyI => b'I',
            Wk::KeyJ => b'J',
            Wk::KeyK => b'K',
            Wk::KeyL => b'L',
            Wk::KeyM => b'M',
            Wk::KeyN => b'N',
            Wk::KeyO => b'O',
            Wk::KeyP => b'P',
            Wk::KeyQ => b'Q',
            Wk::KeyR => b'R',
            Wk::KeyS => b'S',
            Wk::KeyT => b'T',
            Wk::KeyU => b'U',
            Wk::KeyV => b'V',
            Wk::KeyW => b'W',
            Wk::KeyX => b'X',
            Wk::KeyY => b'Y',
            Wk::KeyZ => b'Z',
            Wk::Digit0 => b'0',
            Wk::Digit1 => b'1',
            Wk::Digit2 => b'2',
            Wk::Digit3 => b'3',
            Wk::Digit4 => b'4',
            Wk::Digit5 => b'5',
            Wk::Digit6 => b'6',
            Wk::Digit7 => b'7',
            Wk::Digit8 => b'8',
            Wk::Digit9 => b'9',
            _ => return None,
        })
    }

    pub(super) fn dispatch_os_mouse_to_window_manager(
        &self,
        button: MouseButton,
        pressed: bool,
        x: i32,
        y: i32,
        origin: MouseInputOrigin,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::{
                with_host_control_bar_input_provenance, HostControlBarInputProvenance,
            };
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            let msg = match (button, pressed) {
                (MouseButton::Left, true) => WindowMessage::LeftDown,
                (MouseButton::Left, false) => WindowMessage::LeftUp,
                (MouseButton::Right, true) => WindowMessage::RightDown,
                (MouseButton::Right, false) => WindowMessage::RightUp,
                (MouseButton::Middle, true) => WindowMessage::MiddleDown,
                (MouseButton::Middle, false) => WindowMessage::MiddleUp,
                _ => return false,
            };
            // `CommandSourceType::FromUser` alone cannot distinguish a real
            // winit event from `inject_winit_equivalent_*`: both deliberately
            // follow the same WND callback path. Scope the actual origin over
            // this synchronous dispatch so publication captures it exactly.
            let provenance = match origin {
                MouseInputOrigin::Physical => {
                    HostControlBarInputProvenance::PhysicalWindowMouseInput
                }
                MouseInputOrigin::Injected => HostControlBarInputProvenance::InjectedOrUnknown,
            };
            with_host_control_bar_input_provenance(provenance, || {
                game_client::gui::dispatch_os_mouse_to_window_manager(msg, x, y)
                    == WindowInputReturnCode::Used
            })
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (button, pressed, x, y, origin);
            false
        }
    }

    pub(super) fn dispatch_os_mouse_move(&self, x: i32, y: i32) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            game_client::gui::dispatch_os_mouse_to_window_manager(WindowMessage::MousePos, x, y)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (x, y);
            false
        }
    }

    pub(super) fn dispatch_os_mouse_wheel(
        &self,
        delta: &winit::event::MouseScrollDelta,
        x: i32,
        y: i32,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            let lines = match delta {
                winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                winit::event::MouseScrollDelta::PixelDelta(p) => (p.y as f32) / 16.0,
            };
            if lines.abs() < f32::EPSILON {
                return false;
            }
            let msg = if lines > 0.0 {
                WindowMessage::WheelUp
            } else {
                WindowMessage::WheelDown
            };
            game_client::gui::dispatch_os_mouse_to_window_manager(msg, x, y)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (delta, x, y);
            false
        }
    }

    /// Wave 606: host OS→GameClient mouse-button inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_mouse_button(&self, button: MouseButton, pressed: bool) {
        // Wave 606: host OS mouse button inject residual.
        use game_client::input::mouse::MouseButton as GcMouseButton;
        use std::time::Instant;
        let gc_btn = match button {
            MouseButton::Left => GcMouseButton::Left,
            MouseButton::Right => GcMouseButton::Right,
            MouseButton::Middle => GcMouseButton::Middle,
            MouseButton::Back => GcMouseButton::Other(3),
            MouseButton::Forward => GcMouseButton::Other(4),
            MouseButton::Other(n) => GcMouseButton::Other(n as u16),
        };
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_mouse_button(gc_btn, pressed, Instant::now());
        });
    }

    /// Wave 606: via `host_inject_game_client_mouse_scroll`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_mouse_scroll(&self, delta_y: f32) {
        // Wave 606: thin wrapper — OS mouse scroll inject via host helper.
        self.host_inject_game_client_mouse_scroll(delta_y);
    }

    /// Wave 606: host OS→GameClient mouse-scroll inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_mouse_scroll(&self, delta_y: f32) {
        // Wave 606: host OS mouse scroll inject residual.
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_scroll_lines(delta_y);
        });
    }

    pub(super) fn update_mouse_world_position(&mut self) {
        // C++ maps device coordinates through the active W3D camera.  The former
        // whole-map linear interpolation only happened to work while the camera
        // was centered and made ordinary selection/orders drift after panning,
        // rotation, or zoom.
        // A script/minimap/hotkey can change the orbit between normal camera
        // ticks and an OS mouse event.  Pick from the pose that will actually
        // be rendered, rather than the previous frame's matrix.
        if self.camera_transform_needs_rebuild() {
            self.apply_camera_orbit_transform();
        }
        let size = self.window.inner_size();
        let (world_min, world_max) = self.presentation_world_bounds();
        let picked = {
            let world_env = self
                .render_pipeline
                .presentation_frame()
                .or(self.last_presentation_frame.as_ref())
                .map(|frame| &frame.world_env);
            unproject_mouse_ray(
                self.view_matrix,
                self.projection_matrix,
                self.mouse_position,
                size.width.max(1) as f32,
                size.height.max(1) as f32,
            )
            .and_then(|(near, far)| {
                raycast_frozen_terrain(near, far, world_min, world_max, world_env).or_else(|| {
                    raycast_ground_plane_clamped(near, far, world_min, world_max, world_env)
                })
            })
        };
        if let Some(position) = picked {
            self.mouse_world_position = position;
        }
    }

    /// Presentation-only world pick. Returns `None` when no snapshot is installed
    /// (no live GameLogic dual-read residual). InGame always seeds
    /// `last_presentation_frame` before input.

    /// Wave 228: build presentation target hint for RMB classification.

    /// Wave 229: presentation-frozen selected-unit capabilities for RMB classification.
    pub(super) fn presentation_selected_unit_hints(
        &self,
        ids: &[crate::game_logic::ObjectId],
    ) -> Vec<crate::command_system::PresentationSelectedUnitHint> {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(ids.len());
        for &id in ids {
            let Some(o) = frame.objects.iter().find(|x| x.id == id) else {
                continue;
            };
            // Wave 1097: selected-hint residual fail-closed on unusable sources.
            if o.destroyed
                || o.health_current <= 0.0
                || o.sold
                || o.under_construction
                || o.masked
                || o.disabled
                || o.unselectable
            {
                continue;
            }
            // C++ construction/structure repair authority is `KINDOF_DOZER`.
            // Preserve it in the frozen input instead of classifying a unit
            // by its UI name (a harvester/worker is not necessarily a dozer).
            let is_worker = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Dozer,
            );
            // Gather authorization is an authored capability, not a template
            // naming convention.  C++ marks Chinooks, Supply Trucks, and GLA
            // Workers with KINDOF_HARVESTER.
            let is_resource_collector =
                crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::Harvester,
                );
            let can_attack = o.has_weapon;
            let can_move = o.is_mobile;
            let capture_power = o.capture_power;
            let capture_power_ready = o.capture_power_ready;
            let can_capture =
                capture_power != crate::game_logic::CapturePowerKind::None && capture_power_ready;
            let can_repair = is_worker;
            let is_damaged = o.health_max > 0.0 && o.health_current + 0.01 < o.health_max;
            let is_vehicle = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Vehicle,
            );
            let is_aircraft = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Aircraft,
            );
            let is_infantry = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Infantry,
            );
            let is_above_terrain = o.airborne_target
                || (o.ground_height_from_terrain && o.position.y > o.ground_height + 0.01);
            out.push(crate::command_system::PresentationSelectedUnitHint {
                id,
                is_alive: true,
                is_resource_collector,
                is_worker,
                can_attack,
                can_move,
                can_request_service: o.contained_by.is_none(),
                can_capture,
                template_name: o.template_name.clone(),
                can_repair,
                is_damaged,
                is_vehicle,
                is_aircraft,
                is_above_terrain,
                is_infantry,
                transport_slot_count: o.transport_slot_count,
                stored_supplies: o.stored_supplies,
                is_controlled_by_local: frame.is_owned_by_local(o),
                capture_power,
                capture_power_ready,
                is_salvager: crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::Salvager,
                ),
            });
        }
        out
    }

    pub(super) fn presentation_target_hint(
        &self,
        id: crate::game_logic::ObjectId,
    ) -> Option<crate::command_system::PresentationTargetHint> {
        let frame = self.last_presentation_frame.as_ref()?;
        // Wave 1097: target-hint residual fail-closed on sold/masked and non-local
        // FOW unless Clear (matches pick peels 1093–1096).
        let o = frame.objects.iter().find(|x| {
            x.id == id
                && !x.destroyed
                && !x.sold
                && !x.masked
                && (frame.is_owned_by_local(x) || x.fow_visibility.visibility_alpha >= 0.95)
        })?;
        let is_neutral = o.team == crate::game_logic::Team::Neutral;
        let is_enemy = frame.is_enemy_of_local(o);
        let is_structure = o.object_type
            == crate::presentation_frame::PresentationObjectType::Building
            || crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Structure,
            );
        let is_resource = crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::Harvestable,
        ) || crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::Resource,
        );
        let enter_available_capacity = frame
            .normal_enter_available_capacity_for_local(o)
            .unwrap_or(0);
        let can_be_entered = enter_available_capacity > 0;
        let is_damaged = o.health_max > 0.0 && o.health_current + 0.01 < o.health_max;
        let is_friendly = !is_neutral && frame.is_allied_with_local(o);
        // Freeze exact Object INI KindOf service tags.  The executor repeats
        // these pairings against live authority when consuming the command.
        let provides_heal = crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::HealPad,
        );
        let provides_aircraft_repair =
            crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::FSAirfield,
            );
        let provides_vehicle_repair = crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::RepairPad,
        );
        // C++ treats a non-stealthed occupant as a GarrisonContain gate, but
        // checks friendly contained occupants separately for every target.
        // Freeze both, including stale references which must fail closed.
        let (capture_nonstealthed_garrison_count, capture_friendly_garrison_count) = o
            .garrisoned_units
            .iter()
            .fold((0u16, 0u16), |counts, occupant_id| {
                let Some(occupant) = frame
                    .objects
                    .iter()
                    .find(|candidate| candidate.id == *occupant_id)
                else {
                    return (
                        counts.0.saturating_add(o.capture_garrisonable as u16),
                        counts.1.saturating_add(1),
                    );
                };
                (
                    counts
                        .0
                        .saturating_add((o.capture_garrisonable && !occupant.stealthed) as u16),
                    counts
                        .1
                        .saturating_add(frame.is_allied_with_local(occupant) as u16),
                )
            });
        Some(crate::command_system::PresentationTargetHint {
            id,
            // Wave 1098: is_alive residual excludes sold/masked.
            is_alive: !o.destroyed && o.health_current > 0.0 && !o.sold && !o.masked,
            is_structure,
            is_resource,
            under_construction: o.under_construction,
            sold: o.sold,
            team: o.team,
            is_enemy_of_local: is_enemy,
            is_neutral,
            template_name: o.template_name.clone(),
            can_be_entered,
            enter_available_capacity,
            enter_uses_transport_slots: o.normal_enter_uses_transport_slots(),
            enter_requires_infantry: o.normal_enter_requires_infantry(),
            enter_forbids_aircraft: o.normal_enter_forbids_aircraft(),
            enter_disabled_subdued: o.disabled_subdued,
            enter_is_rider_change: o.contain_module_kind
                == crate::game_logic::ContainModuleKind::RiderChange,
            rider_change_allowed_templates: o.rider_change_allowed_templates.clone(),
            is_damaged,
            is_friendly_of_local: is_friendly,
            // ActionManager keys service on KindOf, not ObjectType::Building.
            provides_vehicle_repair,
            provides_aircraft_repair,
            provides_heal,
            can_provide_service: o.contained_by.is_none(),
            dock_kind: o.dock_kind,
            dock_controller_is_local: frame.is_owned_by_local(o),
            stored_supplies: o.stored_supplies,
            capturable: o.capturable,
            immune_to_capture: o.immune_to_capture,
            capture_garrisonable: o.capture_garrisonable,
            capture_nonstealthed_garrison_count,
            capture_friendly_garrison_count,
            capture_target_effectively_stealthed: o.effectively_stealthed,
            is_crate: o.is_crate,
            is_salvage_crate: o.is_salvage_crate,
        })
    }

    pub(super) fn find_object_at_position(
        &self,
        position: Vec3,
        command_context: bool,
    ) -> Option<ObjectId> {
        const BASE_SELECTION_RADIUS: f32 = 20.0;
        // Wave 222: presentation-only pick (no GameLogic dual-read residual).

        let frame = self.last_presentation_frame.as_ref()?;
        let player_team = Some(frame.local_team());
        let has_selected_units = !self.selected_objects.is_empty();
        let prioritize_enemy_targets = command_context && has_selected_units;

        crate::unit_control::UnitControlSystem::pick_object_id_at_world_from_presentation(
            frame,
            position,
            player_team,
            prioritize_enemy_targets,
            BASE_SELECTION_RADIUS,
        )
    }

    /// Path following is authoritative in `GameLogic::update_movement`.
    /// Retained as a no-op compatibility hook for older call sites.

    /// Legacy render stub -- NOT called from the active render path.
    /// Actual rendering is handled by RenderPipeline::execute() -> ForwardPass::render()
    /// which queues MeshClass instances into the WW3D Renderer and issues real draw calls.
    #[allow(dead_code)] // Legacy stub: superseded by RenderPipeline, retained for reference
    pub(super) fn render_game_objects<'a>(&'a self, _render_pass: &mut wgpu::RenderPass<'a>) {
        // Presentation-only stub: RenderPipeline is the sole draw path.
        let n = self
            .last_presentation_frame
            .as_ref()
            .map(|f| f.objects.len())
            .unwrap_or(0);
        log::trace!(
            "Legacy stub: presentation has {} objects (RenderPipeline is sole draw path)",
            n
        );
    }

    /// Legacy per-object render stub -- logs model status but does NOT submit draw calls.
    /// The active render path is RenderPipeline::collect_render_items() which builds
    /// RenderItem list and ForwardPass::prepare_mesh_instance() which creates actual
    /// MeshClass instances submitted to the WW3D Renderer.
    #[allow(dead_code)] // Legacy stub: superseded by RenderPipeline, retained for reference
    pub(super) fn render_object<'a>(
        &'a self,
        obj: &Object,
        _render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        let model_name = obj.get_template().get_model_name();

        log::trace!(
            "Render object {} template '{}' model '{}' (cached={})",
            obj.id,
            obj.template_name,
            model_name,
            self.graphics_system.get_model(model_name).is_some()
        );

        let w3d_model = self
            .graphics_system
            .get_model(model_name)
            .or_else(|| self.graphics_system.get_model(&obj.template_name));

        if let Some(w3d_model) = w3d_model {
            let total_vertices: usize = w3d_model
                .meshes
                .iter()
                .map(|mesh| mesh.vertices.len())
                .sum();
            let total_indices: usize = w3d_model.meshes.iter().map(|mesh| mesh.indices.len()).sum();

            log::trace!("Rendering W3D model: {} (template: {}) with {} vertices, {} indices across {} meshes",
                model_name, obj.template_name, total_vertices, total_indices, w3d_model.meshes.len());
            log::trace!("Resolved W3D model '{}' for object {}", model_name, obj.id);
        } else {
            log::debug!(
                "No W3D model resolved for object {} template '{}' (model '{}') -- fallback cube will be used by RenderPipeline",
                obj.id,
                obj.template_name,
                model_name
            );
        }
    }

    #[allow(dead_code)] // Legacy stub: selection_renderer + PresentationFrame own production path
    pub(super) fn render_selection_indicators(&self, _render_pass: &mut wgpu::RenderPass) {
        // Prefer presentation selected residual when installed (no live find_object dual-read).
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            let n = frame
                .objects
                .iter()
                .filter(|o| o.selected && !o.destroyed)
                .count();
            log::trace!(
                "Legacy stub: presentation selected count={n} (selection_renderer is sole path)"
            );
            return;
        }
        // Boot residual only.
        for &object_id in &self.selected_objects {
            let _ = object_id;
        }
    }

    pub(super) fn render_projectiles(&self, _render_pass: &mut wgpu::RenderPass) {
        // Projectiles render from PresentationFrame (host CombatSystem freeze).
    }

    pub(super) fn render_ui(&self, _render_pass: &mut wgpu::RenderPass) {
        if let Err(err) = self.ui_manager.render() {
            log::warn!("UI manager render failed: {}", err);
        }
        log::trace!(
            "UI overlay rendered for {} selected units",
            self.selected_objects.len()
        );
    }
}

#[cfg(test)]
mod camera_pick_tests {
    use super::*;

    #[test]
    fn classic_shift_lmb_only_prefers_selection_for_a_local_target() {
        assert!(classic_left_context_action_allowed(true, false, true));
        assert!(!classic_left_context_action_allowed(true, true, true));
        assert!(classic_left_context_action_allowed(true, true, false));
        assert!(!classic_left_context_action_allowed(false, false, false));
    }

    #[test]
    fn center_screen_pick_follows_the_render_camera_not_map_extents() {
        let camera = Vec3::new(0.0, 120.0, 120.0);
        let view = Mat4::look_at_rh(camera, Vec3::ZERO, Vec3::Y);
        let projection = Mat4::perspective_rh(60.0_f32.to_radians(), 1.0, 1.0, 2_000.0);
        let (near, far) = unproject_mouse_ray(view, projection, (500.0, 500.0), 1_000.0, 1_000.0)
            .expect("a finite WGPU camera ray");

        let hit = raycast_ground_plane_clamped(
            near,
            far,
            Vec3::new(-1_000.0, 0.0, -1_000.0),
            Vec3::new(1_000.0, 0.0, 1_000.0),
            None,
        )
        .expect("center ray intersects the ground plane");

        assert!(
            hit.length() < 0.02,
            "center-screen pick must land at the camera target, got {hit:?}"
        );
    }

    #[test]
    fn ray_interval_rejects_a_parallel_ray_outside_the_map() {
        assert!(ray_interval_in_world_xz(
            Vec3::new(20.0, 5.0, 0.0),
            Vec3::new(20.0, -5.0, 0.0),
            Vec3::new(-10.0, 0.0, -10.0),
            Vec3::new(10.0, 0.0, 10.0),
        )
        .is_none());
    }

    #[test]
    fn point_select_predicate_accepts_non_local_selectable() {
        // C++ SelectionXlat.cpp:181-189 — point clicks select anything selectable.
        // Local-only remains the Shift-add / drag-select gate.
        assert!(classic_left_context_action_allowed(true, true, false));
        assert!(!classic_left_context_action_allowed(true, true, true));
    }
}
