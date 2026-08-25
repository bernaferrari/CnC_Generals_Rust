use crate::command_system::{
    CommandMode, CommandType, GuardTarget, ModifierKeys, MouseButton, MouseCommandContext,
    PresentationSelectedUnitHint, PresentationTargetHint, get_command_system, init_command_system,
};
use crate::game_logic::{GameLogic, ObjectId, ObjectType, Team};
use crate::presentation_frame::{PresentationFrame, PresentationObjectType};
use crate::ui::KeyCode as VirtualKeyCode;
use crate::ui::{InputEvent, KeyEvent, MouseEvent};
use glam::{Vec2, Vec3};
use std::collections::HashMap;
use winit::event::MouseButton as WinitMouseButton;

/// Input command processor that bridges raw input and command system
pub struct InputCommandProcessor {
    /// Current mouse position in screen coordinates
    mouse_screen_pos: Vec2,

    /// Current mouse position in world coordinates
    mouse_world_pos: Vec3,

    /// Current viewport size used for screen-to-world conversion.
    viewport_size: Vec2,

    /// Current modifier key states
    modifier_keys: ModifierKeys,

    /// Mouse button states
    left_button_down: bool,
    right_button_down: bool,
    middle_button_down: bool,

    /// Mouse drag tracking
    drag_start_pos: Option<Vec2>,
    drag_start_world: Option<Vec3>,
    is_dragging: bool,
    drag_threshold: f32,

    /// Current player ID
    current_player_id: u32,

    selection_cycles: HashMap<u32, SelectionCycleState>,

    /// Wave 531: optional presentation freeze for mouse classification / pick / box-select.
    /// When installed, process_mouse_input is presentation-only (no GameLogic dual-read).
    presentation_frame: Option<PresentationFrame>,
}

#[derive(Default)]
struct SelectionCycleState {
    last_worker: Option<ObjectId>,
    last_unit: Option<ObjectId>,
}

impl Default for InputCommandProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl InputCommandProcessor {
    /// Create new input command processor
    pub fn new() -> Self {
        Self {
            mouse_screen_pos: Vec2::ZERO,
            mouse_world_pos: Vec3::ZERO,
            viewport_size: Vec2::new(800.0, 600.0),
            modifier_keys: ModifierKeys::default(),
            left_button_down: false,
            right_button_down: false,
            middle_button_down: false,
            drag_start_pos: None,
            drag_start_world: None,
            is_dragging: false,
            drag_threshold: 5.0,  // pixels
            current_player_id: 0, // Default to player 0
            selection_cycles: HashMap::new(),
            presentation_frame: None,
        }
    }

    /// Wave 531: install/clear presentation freeze used by mouse command classification.
    pub fn install_presentation_frame(&mut self, frame: Option<PresentationFrame>) {
        self.presentation_frame = frame;
    }

    /// Wave 531: presentation freeze currently installed (if any).
    pub fn presentation_frame(&self) -> Option<&PresentationFrame> {
        self.presentation_frame.as_ref()
    }

    fn select_worker_cycle(&mut self, game_logic: &GameLogic, reverse: bool) -> bool {
        // Wave 245: team + worker list via probes (no get_player/get_objects dual-read).
        let mut workers: Vec<ObjectId> = game_logic
            .unit_ids_for_player_where(self.current_player_id, |id| game_logic.unit_is_worker(id));

        if workers.is_empty() {
            return false;
        }

        workers.sort_by_key(|id| id.0);
        let target_id = {
            let state = self
                .selection_cycles
                .entry(self.current_player_id)
                .or_default();

            if state.last_worker.is_none() {
                if let Some(selected_id) = game_logic
                    .player_selected_objects(self.current_player_id)
                    .into_iter()
                    .find(|&id| game_logic.unit_is_worker(id))
                {
                    state.last_worker = Some(selected_id);
                }
            }

            let start_index = state
                .last_worker
                .and_then(|last| workers.iter().position(|id| *id == last));

            let target_index = if reverse {
                if let Some(idx) = start_index {
                    if idx == 0 { workers.len() - 1 } else { idx - 1 }
                } else {
                    workers.len() - 1
                }
            } else if let Some(idx) = start_index {
                (idx + 1) % workers.len()
            } else {
                0
            };

            workers[target_index]
        };
        if self.select_units_matching(game_logic, ModifierKeys::default(), |id| id == target_id) {
            if let Some(state) = self.selection_cycles.get_mut(&self.current_player_id) {
                state.last_worker = Some(target_id);
            }
            true
        } else {
            false
        }
    }

    fn select_unit_cycle(&mut self, game_logic: &GameLogic, reverse: bool) -> bool {
        // Wave 245: team + selectable list via probes (no get_player/get_objects dual-read).
        let mut units: Vec<ObjectId> =
            game_logic.selectable_unit_ids_for_player_where(self.current_player_id, |_| true);

        if units.is_empty() {
            return false;
        }

        units.sort_by_key(|id| id.0);
        let target_id = {
            let state = self
                .selection_cycles
                .entry(self.current_player_id)
                .or_default();

            if state.last_unit.is_none() {
                if let Some(selected_id) = game_logic
                    .player_selected_objects(self.current_player_id)
                    .first()
                    .copied()
                {
                    state.last_unit = Some(selected_id);
                }
            }

            let start_index = state
                .last_unit
                .and_then(|last| units.iter().position(|id| *id == last));

            let target_index = if reverse {
                if let Some(idx) = start_index {
                    if idx == 0 { units.len() - 1 } else { idx - 1 }
                } else {
                    units.len() - 1
                }
            } else if let Some(idx) = start_index {
                (idx + 1) % units.len()
            } else {
                0
            };

            units[target_index]
        };
        if self.select_units_matching(game_logic, ModifierKeys::default(), |id| id == target_id) {
            if let Some(state) = self.selection_cycles.get_mut(&self.current_player_id) {
                state.last_unit = Some(target_id);
            }
            true
        } else {
            false
        }
    }

    fn select_matching_selected_unit(&mut self, game_logic: &GameLogic) -> bool {
        // Wave 245: selection seed via player_selected_objects probe.
        if !game_logic.player_exists(self.current_player_id) {
            return false;
        }
        let target_id = match game_logic
            .player_selected_objects(self.current_player_id)
            .first()
        {
            Some(id) => *id,
            None => return false,
        };

        // Wave 245: similar selection via unit probes (no get_object dual-read).
        let Some(team) = game_logic.unit_team(target_id) else {
            return false;
        };
        let Some(template) = game_logic.unit_template_name(target_id) else {
            return false;
        };
        self.select_units_matching(game_logic, self.selection_modifier_state(), |id| {
            game_logic.unit_team(id) == Some(team)
                && game_logic.unit_template_name(id).as_deref() == Some(template.as_str())
        })
    }

    fn select_hero_unit(&mut self, game_logic: &GameLogic) -> bool {
        // Wave 245: hero selection via unit_is_hero probe.
        self.select_units_matching(game_logic, self.selection_modifier_state(), |id| {
            game_logic.unit_is_hero(id)
        })
    }

    /// Process input event and potentially generate commands
    pub fn process_input(&mut self, event: &InputEvent, game_logic: &mut GameLogic) -> Option<()> {
        match event {
            InputEvent::Mouse(mouse_event) => self.process_mouse_event(mouse_event, game_logic),
            InputEvent::Keyboard(keyboard_event) => {
                self.process_keyboard_event(keyboard_event, game_logic)
            }
            InputEvent::WindowResized { width, height } => {
                self.update_viewport_size(*width as f32, *height as f32);
                self.refresh_mouse_world_position(game_logic);
                None
            }
            InputEvent::WindowFocusChanged { focused } => {
                // Handle focus changes if needed
                if !focused {
                    // Reset input state when losing focus
                    self.reset_input_state();
                }
                None
            }
            // Handle other input event variants
            _ => None,
        }
    }

    /// Process mouse events
    fn process_mouse_event(
        &mut self,
        event: &MouseEvent,
        game_logic: &mut GameLogic,
    ) -> Option<()> {
        match event {
            MouseEvent::Move { x, y } => {
                self.update_mouse_position(*x, *y, game_logic);
                self.check_drag_state();
                None
            }
            MouseEvent::ButtonDown { button, x, y } => {
                self.update_mouse_position(*x, *y, game_logic);
                self.handle_mouse_button_down(*button);
                None
            }
            MouseEvent::ButtonUp { button, x, y } => {
                self.update_mouse_position(*x, *y, game_logic);
                self.handle_mouse_button_up(*button, game_logic)
            }
            MouseEvent::Scroll { delta: _delta } => {
                // Handle mouse wheel scrolling if needed
                None
            }
        }
    }

    /// Process keyboard events  
    fn process_keyboard_event(
        &mut self,
        event: &KeyEvent,
        game_logic: &mut GameLogic,
    ) -> Option<()> {
        if event.pressed {
            self.handle_key_down(event.key, game_logic);
        } else {
            self.handle_key_up(event.key, game_logic);
        }
        None
    }

    /// Update mouse position and convert to world coordinates
    fn update_mouse_position(&mut self, x: f32, y: f32, game_logic: &GameLogic) {
        self.mouse_screen_pos = Vec2::new(x, y);
        self.refresh_mouse_world_position(game_logic);
    }

    fn update_viewport_size(&mut self, width: f32, height: f32) {
        if width > 0.0 && height > 0.0 {
            self.viewport_size = Vec2::new(width, height);
        }
    }

    fn refresh_mouse_world_position(&mut self, game_logic: &GameLogic) {
        let viewport_width = self.viewport_size.x.max(1.0);
        let viewport_height = self.viewport_size.y.max(1.0);
        let normalized_x = (self.mouse_screen_pos.x / viewport_width).clamp(0.0, 1.0);
        let normalized_y = (self.mouse_screen_pos.y / viewport_height).clamp(0.0, 1.0);

        let (world_min, world_max) = game_logic.world_bounds();
        let world_width = (world_max.x - world_min.x).max(1.0);
        let world_height = (world_max.z - world_min.z).max(1.0);
        let world_x = world_min.x + normalized_x * world_width;
        let world_z = world_min.z + normalized_y * world_height;
        self.mouse_world_pos = Vec3::new(world_x, 0.0, world_z);
    }

    /// Handle mouse button down
    fn handle_mouse_button_down(&mut self, button: WinitMouseButton) {
        match button {
            WinitMouseButton::Left => {
                self.left_button_down = true;
                self.drag_start_pos = Some(self.mouse_screen_pos);
                self.drag_start_world = Some(self.mouse_world_pos);
                self.is_dragging = false;
            }
            WinitMouseButton::Right => {
                self.right_button_down = true;
                self.drag_start_pos = Some(self.mouse_screen_pos);
                self.drag_start_world = Some(self.mouse_world_pos);
                self.is_dragging = false;
            }
            WinitMouseButton::Middle => {
                self.middle_button_down = true;
            }
            _ => {}
        }
    }

    /// Handle mouse button up and generate commands
    fn handle_mouse_button_up(
        &mut self,
        button: WinitMouseButton,
        game_logic: &mut GameLogic,
    ) -> Option<()> {
        let mouse_button = match button {
            WinitMouseButton::Left => {
                self.left_button_down = false;
                MouseButton::Left
            }
            WinitMouseButton::Right => {
                self.right_button_down = false;
                MouseButton::Right
            }
            WinitMouseButton::Middle => {
                self.middle_button_down = false;
                MouseButton::Middle
            }
            _ => return None,
        };

        // Wave 531: prefer presentation freeze for pick / selection / classification.
        let selected_units = self.get_selected_units(game_logic);
        let target_object = self.find_object_at_position(game_logic);
        let (target_presentation, selected_presentation, box_units, similar_units, world_bounds) =
            if let Some(frame) = self.presentation_frame.as_ref() {
                let target_presentation =
                    target_object.and_then(|id| self.presentation_target_hint(frame, id));
                let selected_presentation =
                    self.presentation_selected_unit_hints(frame, &selected_units);
                let box_units = if self.is_dragging {
                    if let (Some(start), Some(end)) =
                        (self.drag_start_world, Some(self.mouse_world_pos))
                    {
                        let min_x = start.x.min(end.x);
                        let max_x = start.x.max(end.x);
                        let min_z = start.z.min(end.z);
                        let max_z = start.z.max(end.z);
                        frame.box_select_unit_ids(frame.local_team(), min_x, max_x, min_z, max_z)
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                let similar_units = target_object
                    .map(|id| frame.similar_unit_ids(id, frame.local_team()))
                    .unwrap_or_default();
                (
                    target_presentation,
                    selected_presentation,
                    box_units,
                    similar_units,
                    frame.world_env.world_bounds_vec3(),
                )
            } else {
                (
                    None,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    game_logic.world_bounds(),
                )
            };

        // Create command context
        let context = MouseCommandContext {
            world_position: self.mouse_world_pos,
            target_object,
            target_presentation,
            selected_presentation,
            presentation_box_select_units: box_units,
            presentation_select_similar_units: similar_units,
            screen_position: self.mouse_screen_pos,
            viewport_size: Some(self.viewport_size),
            world_min: Some(world_bounds.0),
            world_max: Some(world_bounds.1),
            mouse_button,
            modifier_keys: self.modifier_keys,
            is_drag: self.is_dragging,
            drag_start: if self.is_dragging {
                self.drag_start_pos
            } else {
                None
            },
            drag_end: if self.is_dragging {
                Some(self.mouse_screen_pos)
            } else {
                None
            },
            drag_start_world: if self.is_dragging {
                self.drag_start_world
            } else {
                None
            },
            drag_end_world: if self.is_dragging {
                Some(self.mouse_world_pos)
            } else {
                None
            },
        };

        // Wave 531: presentation-only mouse path when frame installed (mirrors engine Wave 236).
        let gl_for_classify = if self.presentation_frame.is_some() {
            None
        } else {
            Some(&*game_logic)
        };

        // Process mouse input through command system
        let command_system = get_command_system();
        if let Ok(mut system) = command_system.lock() {
            if let Some(command) = system.process_mouse_input(
                &context,
                &selected_units,
                self.current_player_id,
                gl_for_classify,
            ) {
                system.queue_command(command);
                log::trace!("Command queued from mouse input");
            }
        } else {
            log::warn!("Failed to acquire command system lock for mouse input processing");
        }

        // Reset drag state
        self.drag_start_pos = None;
        self.drag_start_world = None;
        self.is_dragging = false;

        None
    }

    /// Check if mouse movement constitutes a drag
    fn check_drag_state(&mut self) {
        if let Some(start_pos) = self.drag_start_pos {
            let drag_distance = (self.mouse_screen_pos - start_pos).length();
            if drag_distance > self.drag_threshold {
                self.is_dragging = true;
            }
        }
    }

    /// Handle key down events
    fn handle_key_down(&mut self, key: VirtualKeyCode, game_logic: &mut GameLogic) {
        let command_system = get_command_system();

        match key {
            // Modifier keys
            VirtualKeyCode::LShift | VirtualKeyCode::RShift => {
                self.modifier_keys.shift = true;
            }
            VirtualKeyCode::LControl | VirtualKeyCode::RControl => {
                if !self.modifier_keys.ctrl {
                    self.modifier_keys.ctrl = true;
                    if let Ok(mut system) = command_system.lock() {
                        system.set_mode(CommandMode::ForceAttack);
                    } else {
                        log::warn!("Failed to acquire command system lock for force attack begin");
                    }
                }
            }
            VirtualKeyCode::LAlt | VirtualKeyCode::RAlt => {
                self.modifier_keys.alt = true;
                if let Ok(mut system) = command_system.lock() {
                    system.set_waypoint_mode_for_player(self.current_player_id, true);
                } else {
                    log::warn!("Failed to acquire command system lock for waypoint begin");
                }
            }

            // Command mode keys
            VirtualKeyCode::A => {
                if self.modifier_keys.ctrl {
                    if let Ok(mut system) = command_system.lock() {
                        system.set_mode(CommandMode::ForceAttack);
                    } else {
                        log::warn!("Failed to acquire command system lock for mode change");
                    }
                } else {
                    // Plain A = Attack Move (standard C&C Generals hotkey)
                    if self.issue_immediate_command(
                        CommandType::AttackMoveTo {
                            destination: self.mouse_world_pos,
                            max_shots: -1,
                        },
                        game_logic,
                    ) {
                        log::debug!("Queued AttackMove command (A key)");
                    }
                }
            }
            VirtualKeyCode::D => {
                // D = Deploy (C&C Generals standard)
                if self.issue_immediate_command(CommandType::Deploy, game_logic) {
                    log::debug!("Queued Deploy command (D key)");
                }
            }
            VirtualKeyCode::T => {
                // T = Force Attack Ground (C&C Generals standard)
                if self.issue_immediate_command(
                    CommandType::ForceAttackGround {
                        location: self.mouse_world_pos,
                    },
                    game_logic,
                ) {
                    log::debug!("Queued ForceAttackGround command (T key)");
                }
            }
            VirtualKeyCode::G => {
                let guard_target = GuardTarget::Position(self.mouse_world_pos);
                if self.issue_immediate_command(
                    CommandType::Guard {
                        target: guard_target,
                        mode: crate::game_logic::GuardMode::Normal,
                    },
                    game_logic,
                ) {
                    log::debug!("Queued Guard command");
                }
            }
            VirtualKeyCode::P => {
                // Patrol falls back to attack-move toward the current mouse world position.
                if self.issue_immediate_command(
                    CommandType::AttackMoveTo {
                        destination: self.mouse_world_pos,
                        max_shots: -1,
                    },
                    game_logic,
                ) {
                    log::debug!("Queued Patrol (AttackMove) command");
                }
            }
            VirtualKeyCode::Q => {
                if self
                    .select_units_matching(&*game_logic, self.selection_modifier_state(), |_| true)
                {
                    log::debug!("Selected all controllable units");
                }
            }
            VirtualKeyCode::W => {
                if self.select_units_matching(&*game_logic, self.selection_modifier_state(), |id| {
                    game_logic.unit_object_type(id) == Some(crate::game_logic::ObjectType::Aircraft)
                }) {
                    log::debug!("Selected all aircraft");
                }
            }
            VirtualKeyCode::S => {
                if self.issue_immediate_command(CommandType::Stop, game_logic) {
                    log::debug!("Queued Stop command");
                }
            }
            VirtualKeyCode::X => {
                if self.issue_immediate_command(CommandType::Scatter, game_logic) {
                    log::debug!("Queued Scatter command");
                }
            }
            VirtualKeyCode::Space => {
                if self.issue_global_command(CommandType::ViewLastRadarEvent) {
                    log::debug!("Queued ViewLastRadarEvent command");
                }
            }
            VirtualKeyCode::F => {
                if self.modifier_keys.ctrl
                    && self.issue_immediate_command(CommandType::CreateFormation, game_logic)
                {
                    log::debug!("Queued CreateFormation command");
                }
            }
            VirtualKeyCode::F1 => {
                if self.select_matching_selected_unit(game_logic) {
                    log::debug!("Selected matching units");
                } else {
                    log::debug!("No reference unit for select matching command");
                }
            }
            VirtualKeyCode::H => {
                if self.modifier_keys.ctrl {
                    if self.select_hero_unit(game_logic) {
                        log::debug!("Selected hero unit");
                    }
                } else if self.issue_global_command(CommandType::ViewCommandCenter) {
                    log::debug!("Centered camera on command center");
                }
            }

            VirtualKeyCode::Escape => {
                // ESC = Cancel current mode
                if let Ok(mut system) = command_system.lock() {
                    system.set_mode(CommandMode::Normal);
                } else {
                    log::warn!("Failed to acquire command system lock for mode reset");
                }
            }

            _ => {}
        }
    }

    /// Handle key up events
    fn handle_key_up(&mut self, key: VirtualKeyCode, _game_logic: &mut GameLogic) {
        let command_system = get_command_system();
        match key {
            // Modifier keys
            VirtualKeyCode::LShift | VirtualKeyCode::RShift => {
                self.modifier_keys.shift = false;
            }
            VirtualKeyCode::LControl | VirtualKeyCode::RControl => {
                self.modifier_keys.ctrl = false;
                if let Ok(mut system) = command_system.lock() {
                    if matches!(system.current_mode, CommandMode::ForceAttack) {
                        system.set_mode(CommandMode::Normal);
                    }
                } else {
                    log::warn!("Failed to acquire command system lock for force attack end");
                }
            }
            VirtualKeyCode::LAlt | VirtualKeyCode::RAlt => {
                self.modifier_keys.alt = false;
                if let Ok(mut system) = command_system.lock() {
                    system.set_waypoint_mode_for_player(self.current_player_id, false);
                } else {
                    log::warn!("Failed to acquire command system lock for waypoint end");
                }
            }
            _ => {}
        }
    }

    fn issue_immediate_command(
        &mut self,
        command_type: CommandType,
        game_logic: &mut GameLogic,
    ) -> bool {
        let selected_units = self.get_selected_units(game_logic);
        if selected_units.is_empty() {
            return false;
        }

        let command_system = get_command_system();
        match command_system.lock() {
            Ok(mut system) => {
                system.queue_immediate_command(
                    command_type,
                    &selected_units,
                    self.current_player_id,
                    self.modifier_keys,
                );
                true
            }
            Err(_) => {
                log::warn!("Failed to acquire command system lock for immediate command");
                false
            }
        }
    }

    fn issue_global_command(&mut self, command_type: CommandType) -> bool {
        let command_system = get_command_system();
        match command_system.lock() {
            Ok(mut system) => {
                system.queue_immediate_command(
                    command_type,
                    &[],
                    self.current_player_id,
                    ModifierKeys::default(),
                );
                true
            }
            Err(_) => {
                log::warn!("Failed to acquire command system lock for global command");
                false
            }
        }
    }

    fn selection_modifier_state(&self) -> ModifierKeys {
        ModifierKeys {
            shift: self.modifier_keys.shift,
            ..ModifierKeys::default()
        }
    }

    fn select_units_matching<F>(
        &mut self,
        game_logic: &GameLogic,
        modifiers: ModifierKeys,
        predicate: F,
    ) -> bool
    where
        F: FnMut(crate::game_logic::ObjectId) -> bool,
    {
        // Wave 245: ObjectId predicates (no &Object dual-read).
        let command_system = get_command_system();
        match command_system.lock() {
            Ok(mut system) => system.select_units_by_predicate(
                self.current_player_id,
                modifiers,
                game_logic,
                predicate,
            ),
            Err(_) => {
                log::warn!("Failed to acquire command system lock for selection command");
                false
            }
        }
    }

    /// Find object at current mouse position
    fn find_object_at_position(&self, game_logic: &GameLogic) -> Option<ObjectId> {
        const BASE_SELECTION_RADIUS: f32 = 50.0;

        // Wave 531: presentation-only pick when freeze installed.
        if let Some(frame) = self.presentation_frame.as_ref() {
            let player_team = Some(frame.local_team());
            let has_selected_units = !self.get_selected_units(game_logic).is_empty();
            return crate::unit_control::UnitControlSystem::pick_object_id_at_world_from_presentation(
                frame,
                self.mouse_world_pos,
                player_team,
                has_selected_units,
                BASE_SELECTION_RADIUS,
            );
        }

        // Wave 246: world pick via GameLogic probe (no caller-side objects dual-walk).
        let player_team = game_logic.player_team(self.current_player_id);
        let has_selected_units = !game_logic
            .player_selected_objects(self.current_player_id)
            .is_empty();
        game_logic.pick_object_id_at_world(
            self.mouse_world_pos,
            player_team,
            has_selected_units,
            BASE_SELECTION_RADIUS,
        )
    }

    /// Get currently selected units for the current player
    fn get_selected_units(&self, game_logic: &GameLogic) -> Vec<ObjectId> {
        // Wave 245: selection via player_selected_objects probe.
        // Wave 531: when presentation freeze is installed and host selection is empty,
        // fall through to host probe only (selection still lives on GameLogic).
        game_logic.player_selected_objects(self.current_player_id)
    }

    /// Wave 531: build target classification freeze from presentation.
    fn presentation_target_hint(
        &self,
        frame: &PresentationFrame,
        id: ObjectId,
    ) -> Option<PresentationTargetHint> {
        let o = frame.objects.iter().find(|x| x.id == id && !x.destroyed)?;
        let is_neutral = o.team == Team::Neutral;
        let is_enemy = frame.is_enemy_of_local(o);
        let is_structure = o.object_type == PresentationObjectType::Building
            || PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Structure);
        let is_resource =
            PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Harvestable)
                || PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Resource);
        let enter_available_capacity = frame
            .normal_enter_available_capacity_for_local(o)
            .unwrap_or(0);
        let can_be_entered = enter_available_capacity > 0;
        let is_damaged = o.health_max > 0.0 && o.health_current + 0.01 < o.health_max;
        let is_friendly = !is_neutral && frame.is_allied_with_local(o);
        // C++ ActionManager service eligibility is the source-authored
        // REPAIR_PAD / HEAL_PAD / FS_AIRFIELD KindOf matrix, frozen here for
        // physical input and revalidated at executor authority.
        let provides_heal =
            PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::HealPad);
        let provides_aircraft_repair =
            PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::FSAirfield);
        let provides_vehicle_repair =
            PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::RepairPad);
        // C++ capture has two independent containment gates: a
        // `GarrisonContain` target rejects any non-stealthed occupant, while
        // `appearsToContainFriendlies` rejects a friendly occupant on any
        // target. Keep both facts frozen instead of making the latter depend
        // on the former.
        let (capture_nonstealthed_garrison_count, capture_friendly_garrison_count) = o
            .garrisoned_units
            .iter()
            .fold((0u16, 0u16), |counts, occupant_id| {
                let Some(occupant) = frame
                    .objects
                    .iter()
                    .find(|candidate| candidate.id == *occupant_id)
                else {
                    // A stale/missing contained record must not make a
                    // defended target capturable in a frozen physical-input
                    // context.
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
        Some(PresentationTargetHint {
            id,
            is_alive: !o.destroyed && o.health_current > 0.0,
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
            is_vehicle: PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Vehicle),
            is_aircraft: PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Aircraft),
            is_drone: PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Drone),
            is_carbomb: o.is_carbomb,
            is_unmanned: o.disabled_unmanned,
            is_mine: o.has_mine
                || PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Mine)
                || PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::DemoTrap)
                || crate::game_logic::host_car_bomb::object_definition_has_kind(
                    &o.template_name,
                    "MINE",
                ),
        })
    }

    /// Wave 531: build selected-unit capability freeze from presentation.
    fn presentation_selected_unit_hints(
        &self,
        frame: &PresentationFrame,
        ids: &[ObjectId],
    ) -> Vec<PresentationSelectedUnitHint> {
        let mut out = Vec::with_capacity(ids.len());
        for &id in ids {
            let Some(o) = frame.objects.iter().find(|x| x.id == id) else {
                continue;
            };
            if o.destroyed || o.health_current <= 0.0 || o.under_construction {
                continue;
            }
            let is_worker = PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Dozer);
            let is_resource_collector =
                PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Harvester);
            let can_attack = o.has_weapon;
            let can_move = o.is_mobile;
            let capture_power = o.capture_power;
            let capture_power_ready = o.capture_power_ready;
            let can_capture =
                capture_power != crate::game_logic::CapturePowerKind::None && capture_power_ready;
            let can_repair = is_worker;
            let is_damaged = o.health_max > 0.0 && o.health_current + 0.01 < o.health_max;
            let is_vehicle =
                PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Vehicle);
            let is_aircraft =
                PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Aircraft);
            let is_infantry =
                PresentationFrame::object_has_kind(o, crate::game_logic::KindOf::Infantry);
            let is_above_terrain = o.airborne_target
                || (o.ground_height_from_terrain && o.position.y > o.ground_height + 0.01);
            out.push(PresentationSelectedUnitHint {
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
                is_salvager: PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::Salvager,
                ),
                can_override_special_power_destination: o
                    .special_power_override_destination
                    .is_some(),
            });
        }
        out
    }

    /// Reset all input state
    fn reset_input_state(&mut self) {
        self.left_button_down = false;
        self.right_button_down = false;
        self.middle_button_down = false;
        self.drag_start_pos = None;
        self.drag_start_world = None;
        self.is_dragging = false;
        self.modifier_keys = ModifierKeys::default();
    }

    /// Set current player ID
    pub fn set_current_player(&mut self, player_id: u32) {
        self.current_player_id = player_id;
        self.selection_cycles.entry(player_id).or_default();
    }

    /// Get current player ID
    pub fn get_current_player(&self) -> u32 {
        self.current_player_id
    }
}

/// Initialize command integration system
pub fn init_command_integration() {
    init_command_system();
    log::info!("Command integration system initialized");
}

/// Process queued commands
pub fn process_command_queue(game_logic: &mut GameLogic) {
    let command_system = get_command_system();
    if let Ok(mut system) = command_system.lock() {
        let results = system.process_commands(game_logic);

        for result in results {
            match result {
                crate::command_system::CommandResult::Success => {
                    // Command executed successfully
                }
                crate::command_system::CommandResult::InvalidTarget => {
                    log::debug!("Command failed: Invalid target");
                }
                crate::command_system::CommandResult::OutOfRange => {
                    log::debug!("Command failed: Out of range");
                }
                crate::command_system::CommandResult::InsufficientResources => {
                    log::debug!("Command failed: Insufficient resources");
                }
                _ => {
                    log::debug!("Command failed: {:?}", result);
                }
            }
        }
    } else {
        log::warn!("Failed to acquire command system lock for command processing");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{
        ContainAdmission, ContainModuleKind, ContainModuleMetadata, GameLogic, KindOf, Object,
        Player, ThingTemplate, Weapon,
    };

    fn ensure_player(game_logic: &mut GameLogic, player_id: u32, team: Team) {
        if game_logic.get_player(player_id).is_some() {
            return;
        }
        let mut player = Player::new(player_id, team, "TestPlayer", true);
        player.resources.supplies = 100_000;
        game_logic.add_player(player);
    }

    fn add_selectable_object(
        game_logic: &mut GameLogic,
        id: ObjectId,
        team: Team,
        position: Vec3,
        attackable: bool,
    ) {
        let template_name = format!("TestObject_{}", id.0);
        let mut template = ThingTemplate::new(&template_name);
        template
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Vehicle);
        if attackable {
            template.add_kind_of(KindOf::Attackable);
        }

        let mut object = Object::new(template, id, team);
        object.set_position(position);
        game_logic.add_object(object);
    }

    #[test]
    fn test_input_processor_creation() {
        let processor = InputCommandProcessor::new();
        assert_eq!(processor.current_player_id, 0);
        assert!(!processor.is_dragging);
    }

    #[test]
    fn test_mouse_position_update() {
        let mut processor = InputCommandProcessor::new();
        let game_logic = GameLogic::new();
        processor.update_mouse_position(100.0, 200.0, &game_logic);

        assert_eq!(processor.mouse_screen_pos, Vec2::new(100.0, 200.0));
        assert!((processor.mouse_world_pos.x - (-192.0)).abs() < 0.001);
        assert!((processor.mouse_world_pos.z - (-85.333336)).abs() < 0.001);
    }

    #[test]
    fn test_mouse_position_respects_runtime_viewport_size() {
        let mut processor = InputCommandProcessor::new();
        let game_logic = GameLogic::new();

        processor.update_viewport_size(1600.0, 900.0);
        processor.update_mouse_position(800.0, 450.0, &game_logic);

        assert!((processor.mouse_world_pos.x - 0.0).abs() < 0.001);
        assert!((processor.mouse_world_pos.z - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_modifier_keys() {
        let mut processor = InputCommandProcessor::new();
        let mut game_logic = GameLogic::new();

        processor.handle_key_down(VirtualKeyCode::LShift, &mut game_logic);
        assert!(processor.modifier_keys.shift);

        processor.handle_key_up(VirtualKeyCode::LShift, &mut game_logic);
        assert!(!processor.modifier_keys.shift);
    }

    #[test]
    fn find_object_prefers_enemy_attackable_targets_when_units_selected() {
        let mut processor = InputCommandProcessor::new();
        let mut game_logic = GameLogic::new();
        ensure_player(&mut game_logic, 0, Team::USA);

        let selected_id = ObjectId(10);
        let enemy_id = ObjectId(11);
        let friendly_id = ObjectId(12);
        add_selectable_object(
            &mut game_logic,
            selected_id,
            Team::USA,
            Vec3::new(-20.0, 0.0, 0.0),
            true,
        );
        add_selectable_object(
            &mut game_logic,
            enemy_id,
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
            true,
        );
        add_selectable_object(
            &mut game_logic,
            friendly_id,
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            true,
        );

        game_logic
            .get_player_mut(0)
            .expect("player should exist")
            .selected_objects
            .push(selected_id);

        processor.set_current_player(0);
        processor.mouse_world_pos = Vec3::new(0.0, 0.0, 0.0);

        let target = processor.find_object_at_position(&game_logic);
        assert_eq!(target, Some(enemy_id));
    }

    #[test]
    fn find_object_ignores_enemy_when_no_selection_active() {
        let mut processor = InputCommandProcessor::new();
        let mut game_logic = GameLogic::new();
        ensure_player(&mut game_logic, 0, Team::USA);

        let enemy_id = ObjectId(20);
        let friendly_id = ObjectId(21);
        add_selectable_object(
            &mut game_logic,
            enemy_id,
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
            true,
        );
        add_selectable_object(
            &mut game_logic,
            friendly_id,
            Team::USA,
            Vec3::new(15.0, 0.0, 0.0),
            true,
        );

        processor.set_current_player(0);
        processor.mouse_world_pos = Vec3::new(0.0, 0.0, 0.0);

        let target = processor.find_object_at_position(&game_logic);
        assert_eq!(target, Some(friendly_id));
    }

    #[test]
    fn physical_rmb_enter_uses_frozen_transport_capability_not_template_spelling() {
        use crate::command_system::{CommandResult, CommandSystem};
        use crate::game_logic::AIState;

        let mut game_logic = GameLogic::new();
        ensure_player(&mut game_logic, 0, Team::USA);

        let infantry_id = ObjectId(801);
        let humvee_id = ObjectId(802);

        let mut infantry_template = ThingTemplate::new("TestRanger");
        infantry_template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        infantry_template.transport_slot_count = Some(1);
        let mut infantry = Object::new(infantry_template, infantry_id, Team::USA);
        infantry.owner_player_id = Some(0);
        infantry.set_position(Vec3::ZERO);
        game_logic.add_object(infantry);

        // `AmericaVehicleHumvee` does not contain any of the former RMB
        // spelling probes (`transport`, `chinook`, `bunker`, `garrison`,
        // `overlord`).  The only valid source here is the frozen
        // TransportContain role and Slots=5 installed by host authority.
        let mut humvee_template = ThingTemplate::new("AmericaVehicleHumvee");
        humvee_template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(300.0);
        let mut humvee = Object::new(humvee_template, humvee_id, Team::USA);
        humvee.owner_player_id = Some(0);
        humvee.install_humvee_transport();
        humvee.set_position(Vec3::new(40.0, 0.0, 0.0));
        game_logic.add_object(humvee);

        let frame = PresentationFrame::build_from_logic(&game_logic, 0);
        let processor = InputCommandProcessor::new();
        let target = processor
            .presentation_target_hint(&frame, humvee_id)
            .expect("the live carrier must reach the frozen RMB target snapshot");
        assert!(target.can_be_entered, "open Humvee seat must be enterable");
        assert!(
            target.enter_requires_infantry,
            "Humvee TransportContain retains AllowInsideKindOf=INFANTRY"
        );

        let selected = processor.presentation_selected_unit_hints(&frame, &[infantry_id]);
        assert_eq!(selected.len(), 1, "selected infantry must cross the freeze");

        let context = MouseCommandContext {
            world_position: Vec3::new(40.0, 0.0, 0.0),
            target_object: Some(humvee_id),
            target_presentation: Some(target),
            selected_presentation: selected,
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let mut commands = CommandSystem::new();
        // Deliberately provide no live GameLogic to classification: the
        // physical input branch must rely solely on frozen presentation data.
        let command = commands
            .process_mouse_input(&context, &[infantry_id], 0, None)
            .expect("right click must create a command");
        assert!(matches!(
            command.command_type,
            CommandType::Enter { target_id } if target_id == humvee_id
        ));

        assert_eq!(
            commands.execute_command(&command, &mut game_logic),
            CommandResult::Success,
            "the physical Enter command must reach live command authority"
        );
        let infantry_after = game_logic
            .host_object(infantry_id)
            .expect("infantry remains live while walking to carrier");
        assert_eq!(infantry_after.target, Some(humvee_id));
        assert_eq!(infantry_after.ai_state, AIState::Entering);
    }

    #[test]
    fn physical_rmb_parsed_rider_change_replaces_live_rider_without_name_admission() {
        use crate::assets::IniParser;
        use crate::command_system::{CommandResult, CommandSystem};
        use crate::game_logic::{AIState, VeterancyLevel};

        let mut parser = IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ParsedRetailRiderHost
  Type = Vehicle
  Model = UVCombatBike
  KindOf = SELECTABLE VEHICLE
  Locomotor = SET_NORMAL CombatBikeGroundLocomotor CombatBikeCliffLocomotor
  Locomotor = SET_SLUGGISH CombatBikeTerroristGroundLocomotor CombatBikeTerroristCliffLocomotor
  Behavior = RiderChangeContain ModuleTag_Rider
    Slots = 1
    AllowInsideKindOf = INFANTRY
    Rider1 = GLAInfantryWorker RIDER1 WEAPON_RIDER1 STATUS_RIDER1 GLAVehicleCombatBikeDefaultCommandSet SET_NORMAL
    Rider2 = GLAInfantryRebel RIDER2 WEAPON_RIDER2 STATUS_RIDER2 GLAVehicleCombatBikeDefaultCommandSet SET_NORMAL
    Rider3 = GLAInfantryTunnelDefender RIDER3 WEAPON_RIDER3 STATUS_RIDER3 GLAVehicleCombatBikeDefaultCommandSet SET_NORMAL
    Rider4 = GLAInfantryJarmenKell RIDER4 WEAPON_RIDER4 STATUS_RIDER4 GLAVehicleCombatBikeJarmenKellCommandSet SET_NORMAL
    Rider5 = GLAInfantryTerrorist RIDER5 WEAPON_RIDER5 STATUS_RIDER5 GLAVehicleCombatBikeDefaultCommandSet SET_SLUGGISH
    Rider6 = GLAInfantryHijacker RIDER6 WEAPON_RIDER6 STATUS_RIDER6 GLAVehicleCombatBikeDefaultCommandSet SET_NORMAL
    Rider7 = GLAInfantrySaboteur RIDER7 WEAPON_RIDER7 STATUS_RIDER7 GLAVehicleCombatBikeDefaultCommandSet SET_NORMAL
    ScuttleDelay = 1500
    ScuttleStatus = TOPPLED
  End
End
"#,
                "parsed_retail_rider_change.ini",
            )
            .expect("retail-shaped RiderChange fixture must parse");
        let bike_template = GameLogic::build_template_from_object_definition(
            "ParsedRetailRiderHost",
            parser
                .get_definition("ParsedRetailRiderHost")
                .expect("parsed RiderChange definition"),
            None,
        );
        let rider_change = &bike_template.contain_module;
        assert!(
            rider_change.has_supported_rider_change_roster(),
            "the complete parsed RiderChange roster, not a vehicle basename, enables it"
        );
        assert_eq!(rider_change.slots, Some(1));
        assert_eq!(rider_change.rider_change_scuttle_delay_frames, Some(45));
        assert_eq!(
            bike_template.locomotor_name.as_deref(),
            Some("CombatBikeGroundLocomotor"),
            "RiderChange uses the authored SET_NORMAL primary, not the later sluggish row"
        );
        assert_eq!(
            rider_change
                .rider_change_riders
                .iter()
                .find(|rider| rider.slot == 5)
                .expect("retail Terrorist row retained")
                .locomotor_set,
            "SET_SLUGGISH"
        );
        assert!(
            rider_change
                .rider_change_riders
                .iter()
                .find(|rider| rider.slot == 5)
                .expect("retail Terrorist row retained")
                .physical_enter_supported,
            "SET_SLUGGISH Terrorist row is now a live RiderChange enter"
        );

        fn rider_template(name: &str) -> ThingTemplate {
            let mut template = ThingTemplate::new(name);
            template
                .add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            template.transport_slot_count = Some(1);
            template
        }

        let mut game_logic = GameLogic::new();
        ensure_player(&mut game_logic, 0, Team::GLA);

        let bike_id = ObjectId(805);
        let old_rider_id = ObjectId(806);
        let incoming_rider_id = ObjectId(807);
        let slow_rider_id = ObjectId(808);
        // Start inside the authoritative enter radius so the next public
        // gameplay tick is the actual arrival/replace boundary, not merely
        // one movement-path tick.
        let bike_position = Vec3::new(2.0, 0.0, 0.0);

        // Deliberately use a host identity that does not match the old
        // `is_combat_cycle_template` spelling residual.  Only the parsed
        // RiderChange module may expose physical normal Enter here.
        let mut bike = Object::new(bike_template, bike_id, Team::GLA);
        bike.owner_player_id = Some(0);
        bike.set_position(bike_position);
        bike.experience.level = VeterancyLevel::Veteran;
        bike.occupants.push(old_rider_id);

        let mut old_rider =
            Object::new(rider_template("GLAInfantryWorker"), old_rider_id, Team::GLA);
        old_rider.owner_player_id = Some(0);
        old_rider.set_position(bike_position);
        old_rider.set_contained_by(Some(bike_id));
        old_rider.set_ai_state(AIState::Docked);

        let mut incoming_rider = Object::new(
            rider_template("GLAInfantryRebel"),
            incoming_rider_id,
            Team::GLA,
        );
        incoming_rider.owner_player_id = Some(0);
        incoming_rider.set_position(Vec3::ZERO);
        incoming_rider.experience.level = VeterancyLevel::Elite;
        incoming_rider.select();

        let mut slow_rider = Object::new(
            rider_template("GLAInfantryTerrorist"),
            slow_rider_id,
            Team::GLA,
        );
        slow_rider.owner_player_id = Some(0);
        slow_rider.set_position(Vec3::ZERO);

        game_logic.add_object(bike);
        game_logic.add_object(old_rider);
        game_logic.add_object(incoming_rider);
        game_logic.add_object(slow_rider);
        game_logic
            .get_player_mut(0)
            .expect("local player")
            .selected_objects
            .push(incoming_rider_id);

        // SET_SLUGGISH is representable: Terrorist can physically enter.
        assert!(
            game_logic.can_unit_enter_normal_target(slow_rider_id, bike_id),
            "SET_SLUGGISH Terrorist row must be physically enterable"
        );
        let frame = PresentationFrame::build_from_logic(&game_logic, 0);
        let processor = InputCommandProcessor::new();
        let target = processor
            .presentation_target_hint(&frame, bike_id)
            .expect("parsed RiderChange host crosses the frozen frame");
        assert!(target.enter_is_rider_change);
        assert!(target.can_be_entered);
        assert!(
            target
                .rider_change_allowed_templates
                .iter()
                .any(|template| template.eq_ignore_ascii_case("GLAInfantryRebel"))
        );
        assert!(
            target
                .rider_change_allowed_templates
                .iter()
                .any(|template| template.eq_ignore_ascii_case("GLAInfantryTerrorist")),
            "presentation exposes the modeled SET_SLUGGISH Terrorist row"
        );

        let slow_selected = processor.presentation_selected_unit_hints(&frame, &[slow_rider_id]);
        let context = MouseCommandContext {
            world_position: bike_position,
            target_object: Some(bike_id),
            target_presentation: Some(target.clone()),
            selected_presentation: slow_selected,
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut commands = CommandSystem::new();
        let slow_command = commands
            .process_mouse_input(&context, &[slow_rider_id], 0, None)
            .expect("SET_SLUGGISH Terrorist reaches frozen physical RMB Enter");
        assert!(
            matches!(slow_command.command_type, CommandType::Enter { target_id } if target_id == bike_id),
            "frozen physical RMB must admit the modeled SET_SLUGGISH row"
        );

        // Run a real RMB -> frozen command -> executor -> arrival sequence.
        // The target has an existing valid worker rider, so the final arrival
        // must be an atomic C++ RiderChange replacement rather than a generic
        // one-seat transport add.
        let selected = processor.presentation_selected_unit_hints(&frame, &[incoming_rider_id]);
        let mut context = context;
        context.selected_presentation = selected;
        let command = commands
            .process_mouse_input(&context, &[incoming_rider_id], 0, None)
            .expect("supported parsed rider reaches frozen physical RMB input");
        assert!(matches!(
            command.command_type,
            CommandType::Enter { target_id } if target_id == bike_id
        ));
        assert_eq!(
            commands.execute_command(&command, &mut game_logic),
            CommandResult::Success,
            "executor repeats parsed roster/controller authority"
        );
        assert_eq!(
            game_logic
                .host_object(incoming_rider_id)
                .expect("incoming rider remains live while approaching")
                .ai_state,
            AIState::Entering
        );

        // Public gameplay tick reaches the same support-state arrival
        // authority used by an offline match; keep this regression outside
        // the private GameLogic AI helper.
        game_logic.update();

        let bike_after = game_logic
            .host_object(bike_id)
            .expect("bike after replacement");
        assert_eq!(bike_after.contained_units(), vec![incoming_rider_id]);
        assert_eq!(bike_after.rider_change_active_slot, Some(2));
        assert_eq!(
            bike_after.rider_change_weapon_set.as_deref(),
            Some("WEAPON_RIDER2")
        );
        assert_eq!(
            bike_after.rider_change_locomotor_set.as_deref(),
            Some("SET_NORMAL")
        );
        assert_eq!(
            bike_after.rider_change_locomotor_name.as_deref(),
            Some("CombatBikeGroundLocomotor"),
            "arrival binds the exact parsed normal primary rather than a name fallback"
        );
        assert_eq!(bike_after.movement.max_speed, 120.0);
        assert_eq!(
            bike_after.command_set_override.as_deref(),
            Some("GLAVehicleCombatBikeDefaultCommandSet")
        );
        assert_eq!(bike_after.combat_cycle_rider, 2);
        assert!(
            bike_after.weapon.is_some(),
            "parsed Rider2 slot sets bike weapon"
        );
        assert_eq!(bike_after.experience.level, VeterancyLevel::Elite);
        assert!(
            bike_after.selected,
            "selected incoming rider transfers selection to bike"
        );
        assert_eq!(
            game_logic.player_selected_objects(0),
            vec![bike_id],
            "RiderChange selection transfer preserves the player's selection roster too"
        );

        let old_after = game_logic
            .host_object(old_rider_id)
            .expect("ejected rider remains live");
        assert_eq!(old_after.contained_by, None);
        assert_eq!(old_after.ai_state, AIState::Idle);
        assert_eq!(old_after.experience.level, VeterancyLevel::Veteran);
        let incoming_after = game_logic
            .host_object(incoming_rider_id)
            .expect("new rider remains contained");
        assert_eq!(incoming_after.contained_by, Some(bike_id));
        assert_eq!(incoming_after.ai_state, AIState::Docked);
        assert_eq!(incoming_after.experience.level, VeterancyLevel::Rookie);
        assert!(!incoming_after.selected);

        // Generic host refresh is intentionally unable to replace the parsed
        // selected RiderN with a rider-name heuristic after the transaction.
        game_logic.refresh_combat_cycle_rider_weapon(bike_id);
        let bike_after_refresh = game_logic.host_object(bike_id).expect("bike after refresh");
        assert_eq!(bike_after_refresh.rider_change_active_slot, Some(2));
        assert_eq!(bike_after_refresh.combat_cycle_rider, 2);
        assert_eq!(
            bike_after_refresh.rider_change_weapon_set.as_deref(),
            Some("WEAPON_RIDER2")
        );

        // C++ RiderChangeContain::onContaining chooseLocomotorSet(SET_SLUGGISH).
        if let Some(terrorist) = game_logic.host_object_mut(slow_rider_id) {
            terrorist.set_position(bike_position);
        }
        let frame = PresentationFrame::build_from_logic(&game_logic, 0);
        let target = processor
            .presentation_target_hint(&frame, bike_id)
            .expect("bike still a RiderChange target");
        context.target_presentation = Some(target);
        context.selected_presentation =
            processor.presentation_selected_unit_hints(&frame, &[slow_rider_id]);
        let terror_command = commands
            .process_mouse_input(&context, &[slow_rider_id], 0, None)
            .expect("Terrorist Enter after Rebel");

        assert!(matches!(
            terror_command.command_type,
            CommandType::Enter { target_id } if target_id == bike_id
        ));
        assert_eq!(
            commands.execute_command(&terror_command, &mut game_logic),
            CommandResult::Success
        );
        game_logic.update();
        let bike_sluggish = game_logic
            .host_object(bike_id)
            .expect("bike after SET_SLUGGISH enter");
        assert_eq!(bike_sluggish.contained_units(), vec![slow_rider_id]);
        assert_eq!(
            bike_sluggish.rider_change_locomotor_set.as_deref(),
            Some("SET_SLUGGISH")
        );
        assert_eq!(
            bike_sluggish.rider_change_locomotor_name.as_deref(),
            Some("CombatBikeTerroristGroundLocomotor")
        );
        assert!(
            (bike_sluggish.movement.max_speed - 40.0).abs() < 0.05,
            "SET_SLUGGISH must install the slow bike table, got {}",
            bike_sluggish.movement.max_speed
        );
    }

    #[test]
    fn physical_rmb_enters_empty_enemy_garrison_before_attack() {
        use crate::command_system::{CommandResult, CommandSystem};
        use crate::game_logic::AIState;

        let mut game_logic = GameLogic::new();
        ensure_player(&mut game_logic, 0, Team::USA);
        ensure_player(&mut game_logic, 1, Team::GLA);

        let infantry_id = ObjectId(811);
        let garrison_id = ObjectId(812);

        let mut infantry_template = ThingTemplate::new("TestRanger");
        infantry_template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0);
        infantry_template.transport_slot_count = Some(1);
        let mut infantry = Object::new(infantry_template, infantry_id, Team::USA);
        infantry.owner_player_id = Some(0);
        // Make the alternate branch genuinely available: prior to this fix,
        // physical RMB classified this exact input as AttackObject before it
        // could reach Enter.
        infantry.weapon = Some(Weapon::default());
        infantry.set_position(Vec3::ZERO);
        game_logic.add_object(infantry);

        let mut garrison_template = ThingTemplate::new("TestBunker");
        garrison_template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1_000.0);
        garrison_template.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Garrison,
            slots: Some(5),
            admission: ContainAdmission::InfantryOnly,
            allow_allies_inside: true,
            allow_enemies_inside: true,
            allow_neutral_inside: true,
            ..ContainModuleMetadata::default()
        };
        let mut garrison = Object::new(garrison_template, garrison_id, Team::GLA);
        garrison.owner_player_id = Some(1);
        garrison.set_position(Vec3::new(40.0, 0.0, 0.0));
        game_logic.add_object(garrison);

        let frame = PresentationFrame::build_from_logic(&game_logic, 0);
        let processor = InputCommandProcessor::new();
        let target = processor
            .presentation_target_hint(&frame, garrison_id)
            .expect("the empty non-faction garrison must reach physical RMB input");
        assert!(
            target.is_enemy_of_local,
            "the garrison is controlled by player 1"
        );
        assert!(
            target.can_be_entered,
            "C++ permits entry into an empty enemy non-faction garrison"
        );
        let selected = processor.presentation_selected_unit_hints(&frame, &[infantry_id]);
        assert!(
            selected[0].can_attack,
            "the regression must distinguish Enter precedence from a move fallback"
        );

        let context = MouseCommandContext {
            world_position: Vec3::new(40.0, 0.0, 0.0),
            target_object: Some(garrison_id),
            target_presentation: Some(target),
            selected_presentation: selected,
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let mut commands = CommandSystem::new();
        let command = commands
            .process_mouse_input(&context, &[infantry_id], 0, None)
            .expect("right click must create a command from frozen input");
        assert!(matches!(
            command.command_type,
            CommandType::Enter { target_id } if target_id == garrison_id
        ));
        assert_eq!(
            commands.execute_command(&command, &mut game_logic),
            CommandResult::Success,
            "the selected infantry must reach the same authority Enter path"
        );
        let infantry_after = game_logic
            .host_object(infantry_id)
            .expect("infantry remains live while approaching the garrison");
        assert_eq!(infantry_after.target, Some(garrison_id));
        assert_eq!(infantry_after.ai_state, AIState::Entering);
    }

    #[test]
    fn physical_rmb_rejects_same_faction_other_player_transport() {
        use crate::command_system::CommandSystem;

        let mut game_logic = GameLogic::new();
        ensure_player(&mut game_logic, 0, Team::USA);
        ensure_player(&mut game_logic, 1, Team::USA);

        let rider_id = ObjectId(821);
        let transport_id = ObjectId(822);

        let mut rider_template = ThingTemplate::new("ExactOwnerRider");
        rider_template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        rider_template.transport_slot_count = Some(1);
        let mut rider = Object::new(rider_template, rider_id, Team::USA);
        rider.owner_player_id = Some(0);
        rider.set_position(Vec3::ZERO);
        game_logic.add_object(rider);

        let mut transport_template = ThingTemplate::new("ExactOwnerCarrier");
        transport_template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(300.0);
        transport_template.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Transport,
            slots: Some(3),
            admission: ContainAdmission::InfantryOnly,
            allow_allies_inside: true,
            allow_enemies_inside: true,
            allow_neutral_inside: true,
            ..ContainModuleMetadata::default()
        };
        let mut transport = Object::new(transport_template, transport_id, Team::USA);
        transport.owner_player_id = Some(1);
        transport.set_position(Vec3::new(40.0, 0.0, 0.0));
        game_logic.add_object(transport);

        assert!(
            !game_logic.can_unit_enter_normal_target(rider_id, transport_id),
            "two same-faction player slots must not share an ordinary transport"
        );

        let frame = PresentationFrame::build_from_logic(&game_logic, 0);
        let processor = InputCommandProcessor::new();
        let target = processor
            .presentation_target_hint(&frame, transport_id)
            .expect("transport reaches frozen RMB snapshot");
        assert!(
            !target.can_be_entered,
            "frozen input must reject the other player's empty transport too"
        );
        let selected = processor.presentation_selected_unit_hints(&frame, &[rider_id]);
        let context = MouseCommandContext {
            world_position: Vec3::new(40.0, 0.0, 0.0),
            target_object: Some(transport_id),
            target_presentation: Some(target),
            selected_presentation: selected,
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut commands = CommandSystem::new();
        let command = commands
            .process_mouse_input(&context, &[rider_id], 0, None)
            .expect("RMB still yields an ordinary fallback command");
        assert!(
            !matches!(command.command_type, CommandType::Enter { .. }),
            "physical RMB must not emit Enter for another controller's transport"
        );
    }

    #[test]
    fn physical_rmb_rejects_rider_that_exceeds_remaining_transport_slots() {
        use crate::command_system::CommandSystem;

        let mut game_logic = GameLogic::new();
        ensure_player(&mut game_logic, 0, Team::USA);

        let existing_id = ObjectId(831);
        let heavy_rider_id = ObjectId(832);
        let transport_id = ObjectId(833);

        let mut heavy_template = ThingTemplate::new("WeightedRider");
        heavy_template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(250.0);
        heavy_template.transport_slot_count = Some(2);
        let mut existing = Object::new(heavy_template.clone(), existing_id, Team::USA);
        existing.owner_player_id = Some(0);
        game_logic.add_object(existing);
        let mut heavy_rider = Object::new(heavy_template, heavy_rider_id, Team::USA);
        heavy_rider.owner_player_id = Some(0);
        heavy_rider.set_position(Vec3::ZERO);
        game_logic.add_object(heavy_rider);

        let mut transport_template = ThingTemplate::new("WeightedCarrier");
        transport_template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(300.0);
        transport_template.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Transport,
            slots: Some(3),
            admission: ContainAdmission::AnyMobile,
            allow_allies_inside: true,
            allow_enemies_inside: true,
            allow_neutral_inside: true,
            ..ContainModuleMetadata::default()
        };
        let mut transport = Object::new(transport_template, transport_id, Team::USA);
        transport.owner_player_id = Some(0);
        transport.set_position(Vec3::new(40.0, 0.0, 0.0));
        assert!(transport.add_occupant(existing_id));
        game_logic.add_object(transport);

        assert!(
            !game_logic.can_unit_enter_normal_target(heavy_rider_id, transport_id),
            "two used slots plus a two-slot rider must not fit in Slots=3"
        );

        let frame = PresentationFrame::build_from_logic(&game_logic, 0);
        let processor = InputCommandProcessor::new();
        let target = processor
            .presentation_target_hint(&frame, transport_id)
            .expect("transport reaches frozen RMB snapshot");
        assert_eq!(target.enter_available_capacity, 1);
        assert!(target.enter_uses_transport_slots);
        let selected = processor.presentation_selected_unit_hints(&frame, &[heavy_rider_id]);
        let context = MouseCommandContext {
            world_position: Vec3::new(40.0, 0.0, 0.0),
            target_object: Some(transport_id),
            target_presentation: Some(target),
            selected_presentation: selected,
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut commands = CommandSystem::new();
        let command = commands
            .process_mouse_input(&context, &[heavy_rider_id], 0, None)
            .expect("RMB still yields an ordinary fallback command");
        assert!(
            !matches!(command.command_type, CommandType::Enter { .. }),
            "frozen physical RMB must compare rider slot cost before emitting Enter"
        );
    }
}
