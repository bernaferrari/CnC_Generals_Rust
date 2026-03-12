use crate::command_system::{
    get_command_system, init_command_system, CommandMode, CommandType, GuardTarget, ModifierKeys,
    MouseButton, MouseCommandContext,
};
use crate::game_logic::{GameLogic, ObjectId, ObjectType, Team};
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
        }
    }

    fn select_worker_cycle(&mut self, game_logic: &GameLogic, reverse: bool) -> bool {
        let player = match game_logic.get_player(self.current_player_id) {
            Some(player) => player,
            None => return false,
        };

        let mut workers: Vec<ObjectId> = game_logic
            .get_objects()
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team == player.team && obj.is_worker() {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();

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
                if let Some(selected_id) = player.selected_objects.iter().find_map(|&id| {
                    game_logic
                        .get_object(id)
                        .filter(|obj| obj.is_worker())
                        .map(|_| id)
                }) {
                    state.last_worker = Some(selected_id);
                }
            }

            let start_index = state
                .last_worker
                .and_then(|last| workers.iter().position(|id| *id == last));

            let target_index = if reverse {
                if let Some(idx) = start_index {
                    if idx == 0 {
                        workers.len() - 1
                    } else {
                        idx - 1
                    }
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
        if self.select_units_matching(game_logic, ModifierKeys::default(), |obj| {
            obj.id == target_id
        }) {
            if let Some(state) = self.selection_cycles.get_mut(&self.current_player_id) {
                state.last_worker = Some(target_id);
            }
            true
        } else {
            false
        }
    }

    fn select_unit_cycle(&mut self, game_logic: &GameLogic, reverse: bool) -> bool {
        let player = match game_logic.get_player(self.current_player_id) {
            Some(player) => player,
            None => return false,
        };

        let mut units: Vec<ObjectId> = game_logic
            .get_objects()
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team == player.team && obj.is_selectable() {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();

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
                if let Some(selected_id) = player.selected_objects.first() {
                    state.last_unit = Some(*selected_id);
                }
            }

            let start_index = state
                .last_unit
                .and_then(|last| units.iter().position(|id| *id == last));

            let target_index = if reverse {
                if let Some(idx) = start_index {
                    if idx == 0 {
                        units.len() - 1
                    } else {
                        idx - 1
                    }
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
        if self.select_units_matching(game_logic, ModifierKeys::default(), |obj| {
            obj.id == target_id
        }) {
            if let Some(state) = self.selection_cycles.get_mut(&self.current_player_id) {
                state.last_unit = Some(target_id);
            }
            true
        } else {
            false
        }
    }

    fn select_matching_selected_unit(&mut self, game_logic: &GameLogic) -> bool {
        let player = match game_logic.get_player(self.current_player_id) {
            Some(player) => player,
            None => return false,
        };

        let target_id = match player.selected_objects.first() {
            Some(id) => *id,
            None => return false,
        };

        let Some(reference) = game_logic.get_object(target_id) else {
            return false;
        };
        let template = reference.template_name.clone();
        self.select_units_matching(game_logic, self.selection_modifier_state(), |obj| {
            obj.team == reference.team && obj.template_name == template
        })
    }

    fn select_hero_unit(&mut self, game_logic: &GameLogic) -> bool {
        self.select_units_matching(game_logic, self.selection_modifier_state(), |obj| {
            obj.is_hero()
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

        // Create command context
        let context = MouseCommandContext {
            world_position: self.mouse_world_pos,
            target_object: self.find_object_at_position(game_logic),
            screen_position: self.mouse_screen_pos,
            viewport_size: Some(self.viewport_size),
            world_min: Some(game_logic.world_bounds().0),
            world_max: Some(game_logic.world_bounds().1),
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

        // Get selected units for current player
        let selected_units = self.get_selected_units(game_logic);

        // Process mouse input through command system
        let command_system = get_command_system();
        if let Ok(mut system) = command_system.lock() {
            if let Some(command) = system.process_mouse_input(
                &context,
                &selected_units,
                self.current_player_id,
                game_logic,
            ) {
                system.queue_command(command);
                println!("Command queued from mouse input");
            }
        } else {
            eprintln!("Failed to acquire command system lock for mouse input processing");
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
                        eprintln!("Failed to acquire command system lock for force attack begin");
                    }
                }
            }
            VirtualKeyCode::LAlt | VirtualKeyCode::RAlt => {
                self.modifier_keys.alt = true;
                if let Ok(mut system) = command_system.lock() {
                    system.set_waypoint_mode_for_player(self.current_player_id, true);
                } else {
                    eprintln!("Failed to acquire command system lock for waypoint begin");
                }
            }

            // Command mode keys
            VirtualKeyCode::A => {
                if self.modifier_keys.ctrl {
                    if let Ok(mut system) = command_system.lock() {
                        system.set_mode(CommandMode::ForceAttack);
                    } else {
                        eprintln!("Failed to acquire command system lock for mode change");
                    }
                }
            }
            VirtualKeyCode::G => {
                let guard_target = GuardTarget::Position(self.mouse_world_pos);
                if self.issue_immediate_command(
                    CommandType::Guard {
                        target: guard_target,
                    },
                    game_logic,
                ) {
                    println!("Queued Guard command");
                }
            }
            VirtualKeyCode::P => {
                // Patrol falls back to attack-move toward the current mouse world position.
                if self.issue_immediate_command(
                    CommandType::AttackMoveTo {
                        destination: self.mouse_world_pos,
                    },
                    game_logic,
                ) {
                    println!("Queued Patrol (AttackMove) command");
                }
            }
            VirtualKeyCode::Q => {
                if self
                    .select_units_matching(&*game_logic, self.selection_modifier_state(), |_| true)
                {
                    println!("Selected all controllable units");
                }
            }
            VirtualKeyCode::W => {
                if self.select_units_matching(
                    &*game_logic,
                    self.selection_modifier_state(),
                    |obj| obj.object_type == ObjectType::Aircraft,
                ) {
                    println!("Selected all aircraft");
                }
            }
            VirtualKeyCode::S => {
                if self.issue_immediate_command(CommandType::Stop, game_logic) {
                    println!("Queued Stop command");
                }
            }
            VirtualKeyCode::X => {
                if self.issue_immediate_command(CommandType::Scatter, game_logic) {
                    println!("Queued Scatter command");
                }
            }
            VirtualKeyCode::Space => {
                if self.issue_global_command(CommandType::ViewLastRadarEvent) {
                    println!("Queued ViewLastRadarEvent command");
                }
            }
            VirtualKeyCode::F => {
                if self.modifier_keys.ctrl
                    && self.issue_immediate_command(CommandType::CreateFormation, game_logic)
                {
                    println!("Queued CreateFormation command");
                }
            }
            VirtualKeyCode::F1 => {
                if self.select_matching_selected_unit(game_logic) {
                    println!("Selected matching units");
                } else {
                    println!("No reference unit for select matching command");
                }
            }
            VirtualKeyCode::H => {
                if self.modifier_keys.ctrl {
                    if self.select_hero_unit(game_logic) {
                        println!("Selected hero unit");
                    }
                } else if self.issue_global_command(CommandType::ViewCommandCenter) {
                    println!("Centered camera on command center");
                }
            }

            VirtualKeyCode::Escape => {
                // ESC = Cancel current mode
                if let Ok(mut system) = command_system.lock() {
                    system.set_mode(CommandMode::Normal);
                } else {
                    eprintln!("Failed to acquire command system lock for mode reset");
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
                    eprintln!("Failed to acquire command system lock for force attack end");
                }
            }
            VirtualKeyCode::LAlt | VirtualKeyCode::RAlt => {
                self.modifier_keys.alt = false;
                if let Ok(mut system) = command_system.lock() {
                    system.set_waypoint_mode_for_player(self.current_player_id, false);
                } else {
                    eprintln!("Failed to acquire command system lock for waypoint end");
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
                eprintln!("Failed to acquire command system lock for immediate command");
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
                eprintln!("Failed to acquire command system lock for global command");
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
        F: FnMut(&crate::game_logic::Object) -> bool,
    {
        let command_system = get_command_system();
        match command_system.lock() {
            Ok(mut system) => system.select_units_by_predicate(
                self.current_player_id,
                modifiers,
                game_logic,
                predicate,
            ),
            Err(_) => {
                eprintln!("Failed to acquire command system lock for selection command");
                false
            }
        }
    }

    /// Find object at current mouse position
    fn find_object_at_position(&self, game_logic: &GameLogic) -> Option<ObjectId> {
        const BASE_SELECTION_RADIUS: f32 = 50.0;

        let (player_team, has_selected_units) = match game_logic.get_player(self.current_player_id)
        {
            Some(player) => (Some(player.team), !player.selected_objects.is_empty()),
            None => (None, false),
        };

        // Priority-driven picking:
        // - command targeting (units selected): prefer enemy attackable targets, then friendly/selectable.
        // - pure selection (nothing selected): only allow own selectable objects.
        let mut best: Option<(ObjectId, u8, f32)> = None; // (id, priority, distance)

        for (&id, obj) in game_logic.objects.iter() {
            if !obj.is_alive() {
                continue;
            }

            let distance = (obj.get_position() - self.mouse_world_pos).length();
            let radius = BASE_SELECTION_RADIUS.max(obj.selection_radius);
            if distance > radius {
                continue;
            }

            let priority = if has_selected_units {
                match player_team {
                    Some(team) if obj.team != team && obj.is_attackable() => 0,
                    Some(team) if obj.team == team && obj.is_selectable() => 1,
                    _ if obj.is_attackable() => 2,
                    _ if obj.is_selectable() => 3,
                    _ => continue,
                }
            } else {
                match player_team {
                    Some(team) if obj.team == team && obj.is_selectable() => 0,
                    Some(_) => continue,
                    None if obj.is_selectable() => 0,
                    None => continue,
                }
            };

            match best {
                Some((_, best_priority, best_distance))
                    if priority > best_priority
                        || (priority == best_priority && distance >= best_distance) => {}
                _ => best = Some((id, priority, distance)),
            }
        }

        best.map(|(id, _, _)| id)
    }

    /// Get currently selected units for the current player
    fn get_selected_units(&self, game_logic: &GameLogic) -> Vec<ObjectId> {
        if let Some(player) = game_logic.get_player(self.current_player_id) {
            player.selected_objects.clone()
        } else {
            Vec::new()
        }
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
    println!("Command integration system initialized");
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
                    println!("Command failed: Invalid target");
                }
                crate::command_system::CommandResult::OutOfRange => {
                    println!("Command failed: Out of range");
                }
                crate::command_system::CommandResult::InsufficientResources => {
                    println!("Command failed: Insufficient resources");
                }
                _ => {
                    println!("Command failed: {:?}", result);
                }
            }
        }
    } else {
        eprintln!("Failed to acquire command system lock for command processing");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, Object, Player, ThingTemplate};

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
}
