//! RTS Input System for Command & Conquer Generals Zero Hour
//!
//! This module provides a unified input management system that handles:
//! - Mouse input for RTS gameplay (selection, movement, attack)
//! - Keyboard input for commands and camera control
//! - Integration with GameLogic for unit management
//! - Camera control system for battlefield view

use glam::{Vec2, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;
use winit::{
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    window::Window,
};
use ww3d_engine::FrameTiming;

/// Key mappings for RTS controls
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RtsCommand {
    // Camera Controls
    CameraMoveUp,
    CameraMoveDown,
    CameraMoveLeft,
    CameraMoveRight,
    CameraZoomIn,
    CameraZoomOut,
    CameraPan,

    // Unit Commands
    SelectAll,
    DeleteSelected,
    CycleUnits,
    Attack,
    Move,
    Stop,
    Hold,
    Guard,

    // Control Groups (1-9)
    ControlGroup1,
    ControlGroup2,
    ControlGroup3,
    ControlGroup4,
    ControlGroup5,
    ControlGroup6,
    ControlGroup7,
    ControlGroup8,
    ControlGroup9,

    // Game Commands
    PauseGame,
    ToggleUI,
    QuickSave,
    QuickLoad,
    OpenMenu,
    Chat,

    // Debug Commands
    ToggleDebug,
    ToggleMusic,

    // Mouse Commands
    LeftClick,
    RightClick,
    MiddleClick,
    DragSelect,
    WheelUp,
    WheelDown,
}

/// High-level RTS command events emitted by `RtsInputSystem`.
///
/// The input system itself does not own game state; consumers are expected to drain these events
/// and apply them to the active game logic / selection system.
#[derive(Debug, Clone)]
pub enum RtsCommandEvent {
    LeftClick {
        screen_pos: Vec2,
    },
    RightClick {
        screen_pos: Vec2,
    },
    MiddleClick {
        screen_pos: Vec2,
    },
    DragSelect {
        start_screen: Vec2,
        end_screen: Vec2,
    },
    DoubleClickSelectSimilar {
        screen_pos: Vec2,
    },

    SelectAll,
    DeleteSelected {
        object_ids: Vec<u32>,
    },
    CycleUnits,
    TogglePause,
    AssignControlGroup {
        group: u8,
        object_ids: Vec<u32>,
    },
    RecallControlGroup {
        group: u8,
    },
    ToggleDebug,
    ToggleMusic,
}

/// Mouse input state for RTS controls
#[derive(Debug, Clone)]
pub struct MouseState {
    pub position: Vec2,
    pub delta: Vec2,
    pub left_button: ButtonState,
    pub right_button: ButtonState,
    pub middle_button: ButtonState,
    pub wheel_delta: f32,
    pub drag_start: Option<Vec2>,
    pub is_dragging: bool,
    pub drag_threshold: f32,
}

/// Button state tracking
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonState {
    Up,
    Down,
    Pressed,  // Just pressed this frame
    Released, // Just released this frame
}

/// Camera control state
#[derive(Debug, Clone)]
pub struct CameraController {
    pub position: Vec3,
    pub rotation: Vec3,
    pub zoom: f32,
    pub movement_speed: f32,
    pub zoom_speed: f32,
    pub rotation_speed: f32,
    pub pan_speed: f32,
    pub smooth_movement: bool,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 50.0, 0.0),
            rotation: Vec3::new(-30.0f32.to_radians(), 0.0, 0.0),
            zoom: 50.0,
            movement_speed: 25.0,
            zoom_speed: 10.0,
            rotation_speed: 1.0,
            pan_speed: 20.0,
            smooth_movement: true,
            bounds_min: Vec3::new(-256.0, 10.0, -256.0),
            bounds_max: Vec3::new(256.0, 100.0, 256.0),
        }
    }
}

/// Selection state for RTS units
#[derive(Debug, Clone)]
pub struct SelectionState {
    pub selected_objects: Vec<u32>, // Object IDs
    pub selection_box_start: Option<Vec2>,
    pub selection_box_current: Vec2,
    pub is_box_selecting: bool,
    pub control_groups: HashMap<u8, Vec<u32>>, // 1-9 control groups
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            selected_objects: Vec::new(),
            selection_box_start: None,
            selection_box_current: Vec2::ZERO,
            is_box_selecting: false,
            control_groups: HashMap::new(),
        }
    }
}

/// Main RTS input system
pub struct RtsInputSystem {
    // Input state
    mouse_state: MouseState,
    keys_pressed: HashSet<Key>,
    keys_this_frame: HashSet<Key>,
    keys_released_this_frame: HashSet<Key>,

    // Key bindings
    key_bindings: HashMap<Key, RtsCommand>,

    // RTS-specific state
    camera_controller: CameraController,
    selection_state: SelectionState,
    command_events: VecDeque<RtsCommandEvent>,

    // Timing
    clock_seconds: f32,
    double_click_time: Duration,
    last_click_time: Option<f32>,

    // Settings
    mouse_sensitivity: f32,
    keyboard_repeat_delay: Duration,
    drag_threshold: f32,
    key_repeat_timestamps: HashMap<Key, f32>,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            delta: Vec2::ZERO,
            left_button: ButtonState::Up,
            right_button: ButtonState::Up,
            middle_button: ButtonState::Up,
            wheel_delta: 0.0,
            drag_start: None,
            is_dragging: false,
            drag_threshold: 5.0,
        }
    }
}

impl Default for RtsInputSystem {
    fn default() -> Self {
        Self::new()
    }
}

// Alias for compatibility with subsystem manager
pub type InputSystem = RtsInputSystem;

impl RtsInputSystem {
    /// Create new RTS input system
    pub fn new() -> Self {
        let mut system = Self {
            mouse_state: MouseState::default(),
            keys_pressed: HashSet::new(),
            keys_this_frame: HashSet::new(),
            keys_released_this_frame: HashSet::new(),
            key_repeat_timestamps: HashMap::new(),
            key_bindings: HashMap::new(),
            camera_controller: CameraController::default(),
            selection_state: SelectionState::default(),
            command_events: VecDeque::new(),
            clock_seconds: 0.0,
            double_click_time: Duration::from_millis(300),
            last_click_time: None,
            mouse_sensitivity: 1.0,
            keyboard_repeat_delay: Duration::from_millis(150),
            drag_threshold: 5.0,
        };

        system.setup_default_keybindings();
        system
    }

    /// Setup default RTS key bindings
    fn setup_default_keybindings(&mut self) {
        // Camera controls (WASD + Arrow keys)
        self.key_bindings
            .insert(Key::Character("w".into()), RtsCommand::CameraMoveUp);
        self.key_bindings
            .insert(Key::Character("a".into()), RtsCommand::CameraMoveLeft);
        self.key_bindings
            .insert(Key::Character("s".into()), RtsCommand::CameraMoveDown);
        self.key_bindings
            .insert(Key::Character("d".into()), RtsCommand::CameraMoveRight);
        self.key_bindings
            .insert(Key::Named(NamedKey::ArrowUp), RtsCommand::CameraMoveUp);
        self.key_bindings
            .insert(Key::Named(NamedKey::ArrowLeft), RtsCommand::CameraMoveLeft);
        self.key_bindings
            .insert(Key::Named(NamedKey::ArrowDown), RtsCommand::CameraMoveDown);
        self.key_bindings.insert(
            Key::Named(NamedKey::ArrowRight),
            RtsCommand::CameraMoveRight,
        );

        // Unit selection and commands
        self.key_bindings
            .insert(Key::Character("r".into()), RtsCommand::SelectAll); // Ctrl+A handled separately
        self.key_bindings
            .insert(Key::Named(NamedKey::Delete), RtsCommand::DeleteSelected);
        self.key_bindings
            .insert(Key::Named(NamedKey::Tab), RtsCommand::CycleUnits);

        // Control groups (1-9)
        self.key_bindings
            .insert(Key::Character("1".into()), RtsCommand::ControlGroup1);
        self.key_bindings
            .insert(Key::Character("2".into()), RtsCommand::ControlGroup2);
        self.key_bindings
            .insert(Key::Character("3".into()), RtsCommand::ControlGroup3);
        self.key_bindings
            .insert(Key::Character("4".into()), RtsCommand::ControlGroup4);
        self.key_bindings
            .insert(Key::Character("5".into()), RtsCommand::ControlGroup5);
        self.key_bindings
            .insert(Key::Character("6".into()), RtsCommand::ControlGroup6);
        self.key_bindings
            .insert(Key::Character("7".into()), RtsCommand::ControlGroup7);
        self.key_bindings
            .insert(Key::Character("8".into()), RtsCommand::ControlGroup8);
        self.key_bindings
            .insert(Key::Character("9".into()), RtsCommand::ControlGroup9);

        // Game controls
        self.key_bindings
            .insert(Key::Named(NamedKey::Space), RtsCommand::PauseGame);
        self.key_bindings
            .insert(Key::Named(NamedKey::Escape), RtsCommand::OpenMenu);
        self.key_bindings
            .insert(Key::Named(NamedKey::Enter), RtsCommand::Chat);

        // Debug commands
        self.key_bindings
            .insert(Key::Named(NamedKey::F1), RtsCommand::ToggleDebug);
        self.key_bindings
            .insert(Key::Character("m".into()), RtsCommand::ToggleMusic);
    }

    /// Process window events from winit
    pub fn handle_window_event(&mut self, event: &WindowEvent, _window: &Window) -> bool {
        match event {
            WindowEvent::KeyboardInput { event, .. } => self.handle_keyboard_input(event),
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_input(*button, *state);
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = Vec2::new(position.x as f32, position.y as f32);
                let delta = (new_pos - self.mouse_state.position) * self.mouse_sensitivity;
                self.mouse_state.delta = delta;
                self.mouse_state.position = new_pos;

                // Handle drag selection
                self.update_drag_selection();
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        self.mouse_state.wheel_delta = *y;
                        self.handle_mouse_wheel(*y);
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        self.mouse_state.wheel_delta = pos.y as f32 / 120.0; // Convert to line delta
                        self.handle_mouse_wheel(self.mouse_state.wheel_delta);
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Handle keyboard input
    fn handle_keyboard_input(&mut self, event: &KeyEvent) -> bool {
        match event.state {
            ElementState::Pressed => {
                // Check for modifier combinations
                let key = event.logical_key.clone();

                // Handle Ctrl+A for select all
                if self.is_ctrl_pressed()
                    && matches!(key, Key::Character(ref s) if s.to_lowercase() == "a")
                {
                    self.execute_command(RtsCommand::SelectAll);
                    return true;
                }

                // Add to pressed keys
                let allow_repeat = if self.keys_pressed.contains(&key) {
                    let last_time = self
                        .key_repeat_timestamps
                        .get(&key)
                        .copied()
                        .unwrap_or(f32::NEG_INFINITY);
                    let elapsed = self.clock_seconds - last_time;
                    elapsed >= self.keyboard_repeat_delay.as_secs_f32()
                } else {
                    true
                };

                if allow_repeat {
                    if !self.keys_pressed.contains(&key) {
                        self.keys_this_frame.insert(key.clone());
                    }
                    self.keys_pressed.insert(key.clone());
                    self.key_repeat_timestamps
                        .insert(key.clone(), self.clock_seconds);
                }

                // Execute bound command if any
                if let Some(&command) = self.key_bindings.get(&key) {
                    self.execute_command(command);
                }

                true
            }
            ElementState::Released => {
                let key = event.logical_key.clone();
                self.keys_pressed.remove(&key);
                self.key_repeat_timestamps.remove(&key);
                self.keys_released_this_frame.insert(key);
                true
            }
        }
    }

    /// Handle mouse button input
    fn handle_mouse_input(&mut self, button: MouseButton, state: ElementState) {
        let button_state = match state {
            ElementState::Pressed => {
                // Check for double-click
                let now = self.clock_seconds;
                let is_double_click = if let Some(last_click) = self.last_click_time {
                    (now - last_click) < self.double_click_time.as_secs_f32()
                } else {
                    false
                };
                self.last_click_time = Some(now);

                if is_double_click {
                    // Handle double-click (could select all units of same type)
                    self.handle_double_click(button);
                } else {
                    // Start drag detection for left button
                    if button == MouseButton::Left {
                        self.mouse_state.drag_start = Some(self.mouse_state.position);
                        self.selection_state.selection_box_start = Some(self.mouse_state.position);
                    }
                }

                ButtonState::Pressed
            }
            ElementState::Released => {
                // Handle click completion
                self.handle_mouse_click(button);

                // End drag selection
                if button == MouseButton::Left {
                    self.complete_drag_selection();
                }

                ButtonState::Released
            }
        };

        // Update button states
        match button {
            MouseButton::Left => self.mouse_state.left_button = button_state,
            MouseButton::Right => self.mouse_state.right_button = button_state,
            MouseButton::Middle => self.mouse_state.middle_button = button_state,
            _ => {}
        }
    }

    /// Handle mouse wheel input
    fn handle_mouse_wheel(&mut self, delta: f32) {
        // Zoom camera
        self.camera_controller.zoom -= delta * self.camera_controller.zoom_speed;
        self.camera_controller.zoom = self.camera_controller.zoom.clamp(10.0, 100.0);
    }

    /// Handle mouse click completion
    fn handle_mouse_click(&mut self, button: MouseButton) {
        match button {
            MouseButton::Left => {
                if !self.mouse_state.is_dragging {
                    // Single click - select unit at cursor (simplified for stability)
                    log::debug!("Command: Left Click at {:?}", self.mouse_state.position);
                    self.execute_command(RtsCommand::LeftClick); // Re-enabled for gameplay
                }
            }
            MouseButton::Right => {
                // Right click - move/attack command (simplified for stability)
                log::debug!("Command: Right Click at {:?}", self.mouse_state.position);
                self.execute_command(RtsCommand::RightClick); // Re-enabled for gameplay
            }
            MouseButton::Middle => {
                // Middle click - could be used for camera panning (simplified for stability)
                log::debug!("Command: Middle Click at {:?}", self.mouse_state.position);
                // self.execute_command(RtsCommand::MiddleClick); // Temporarily disabled to prevent crashes
            }
            _ => {}
        }
    }

    /// Handle double-click events
    fn handle_double_click(&mut self, button: MouseButton) {
        if button == MouseButton::Left {
            let pos = self.mouse_state.position;
            log::debug!("Double-click detected - selecting similar units");
            self.command_events
                .push_back(RtsCommandEvent::DoubleClickSelectSimilar { screen_pos: pos });
        }
    }

    /// Update drag selection
    fn update_drag_selection(&mut self) {
        if let Some(drag_start) = self.mouse_state.drag_start {
            let distance = (self.mouse_state.position - drag_start).length();

            if distance > self.drag_threshold && !self.mouse_state.is_dragging {
                self.mouse_state.is_dragging = true;
                self.selection_state.is_box_selecting = true;
                log::debug!("Started drag selection");
            }

            if self.mouse_state.is_dragging {
                self.selection_state.selection_box_current = self.mouse_state.position;
            }
        }
    }

    /// Complete drag selection
    fn complete_drag_selection(&mut self) {
        if self.selection_state.is_box_selecting {
            log::debug!(
                "Completed drag selection from {:?} to {:?}",
                self.selection_state.selection_box_start,
                self.selection_state.selection_box_current
            );

            self.execute_command(RtsCommand::DragSelect);
        }

        // Reset drag state
        self.mouse_state.drag_start = None;
        self.mouse_state.is_dragging = false;
        self.selection_state.is_box_selecting = false;
        self.selection_state.selection_box_start = None;
    }

    /// Update input system (call once per frame) using legacy delta time
    pub fn update(&mut self, dt: f32) {
        let delta_time = dt.max(0.0);
        self.clock_seconds += delta_time;
        self.step(delta_time);
    }

    /// Update input system with WW3D frame timing
    pub fn update_with_timing(&mut self, timing: &FrameTiming) {
        self.clock_seconds = timing.total_seconds();
        self.step(timing.delta_seconds().max(0.0));
    }

    fn step(&mut self, delta_time: f32) {
        // Update camera based on held keys
        self.update_camera_movement(delta_time);

        // Update button states (pressed -> down, released -> up)
        self.update_button_states();

        // Clear frame-specific key states
        self.keys_this_frame.clear();
        self.keys_released_this_frame.clear();

        // Reset wheel delta
        self.mouse_state.wheel_delta = 0.0;
    }

    /// Update camera movement based on held keys
    fn update_camera_movement(&mut self, dt: f32) {
        let speed = self.camera_controller.movement_speed * dt;
        let mut movement = Vec3::ZERO;

        // Check movement keys
        if self.is_key_pressed(&Key::Character("w".into()))
            || self.is_key_pressed(&Key::Named(NamedKey::ArrowUp))
        {
            movement.z -= speed;
        }
        if self.is_key_pressed(&Key::Character("s".into()))
            || self.is_key_pressed(&Key::Named(NamedKey::ArrowDown))
        {
            movement.z += speed;
        }
        if self.is_key_pressed(&Key::Character("a".into()))
            || self.is_key_pressed(&Key::Named(NamedKey::ArrowLeft))
        {
            movement.x -= speed;
        }
        if self.is_key_pressed(&Key::Character("d".into()))
            || self.is_key_pressed(&Key::Named(NamedKey::ArrowRight))
        {
            movement.x += speed;
        }

        // Apply movement with bounds checking
        self.camera_controller.position += movement;
        self.camera_controller.position = self.camera_controller.position.clamp(
            self.camera_controller.bounds_min,
            self.camera_controller.bounds_max,
        );
    }

    /// Update button states for proper frame-based detection
    fn update_button_states(&mut self) {
        // Update mouse button states
        match self.mouse_state.left_button {
            ButtonState::Pressed => self.mouse_state.left_button = ButtonState::Down,
            ButtonState::Released => self.mouse_state.left_button = ButtonState::Up,
            _ => {}
        }
        match self.mouse_state.right_button {
            ButtonState::Pressed => self.mouse_state.right_button = ButtonState::Down,
            ButtonState::Released => self.mouse_state.right_button = ButtonState::Up,
            _ => {}
        }
        match self.mouse_state.middle_button {
            ButtonState::Pressed => self.mouse_state.middle_button = ButtonState::Down,
            ButtonState::Released => self.mouse_state.middle_button = ButtonState::Up,
            _ => {}
        }
    }

    /// Execute an RTS command
    pub fn execute_command(&mut self, command: RtsCommand) {
        match command {
            RtsCommand::SelectAll => {
                log::debug!("Command: Select All Units (Ctrl+A)");
                self.command_events.push_back(RtsCommandEvent::SelectAll);
            }
            RtsCommand::DeleteSelected => {
                log::debug!("Command: Delete Selected Units");
                self.command_events
                    .push_back(RtsCommandEvent::DeleteSelected {
                        object_ids: self.selection_state.selected_objects.clone(),
                    });
            }
            RtsCommand::CycleUnits => {
                log::debug!("Command: Cycle Through Units (Tab)");
                self.command_events.push_back(RtsCommandEvent::CycleUnits);
            }
            RtsCommand::LeftClick => {
                log::debug!("Command: Left Click at {:?}", self.mouse_state.position);
                self.command_events.push_back(RtsCommandEvent::LeftClick {
                    screen_pos: self.mouse_state.position,
                });
            }
            RtsCommand::RightClick => {
                log::debug!("Command: Right Click at {:?}", self.mouse_state.position);
                self.command_events.push_back(RtsCommandEvent::RightClick {
                    screen_pos: self.mouse_state.position,
                });
            }
            RtsCommand::DragSelect => {
                if let Some(start) = self.selection_state.selection_box_start {
                    let end = self.selection_state.selection_box_current;
                    log::debug!("Command: Drag Select from {:?} to {:?}", start, end);
                    self.command_events.push_back(RtsCommandEvent::DragSelect {
                        start_screen: start,
                        end_screen: end,
                    });
                }
            }
            RtsCommand::PauseGame => {
                log::debug!("Command: Toggle Pause");
                self.command_events.push_back(RtsCommandEvent::TogglePause);
            }
            RtsCommand::ControlGroup1
            | RtsCommand::ControlGroup2
            | RtsCommand::ControlGroup3
            | RtsCommand::ControlGroup4
            | RtsCommand::ControlGroup5
            | RtsCommand::ControlGroup6
            | RtsCommand::ControlGroup7
            | RtsCommand::ControlGroup8
            | RtsCommand::ControlGroup9 => {
                let group_num = match command {
                    RtsCommand::ControlGroup1 => 1,
                    RtsCommand::ControlGroup2 => 2,
                    RtsCommand::ControlGroup3 => 3,
                    RtsCommand::ControlGroup4 => 4,
                    RtsCommand::ControlGroup5 => 5,
                    RtsCommand::ControlGroup6 => 6,
                    RtsCommand::ControlGroup7 => 7,
                    RtsCommand::ControlGroup8 => 8,
                    RtsCommand::ControlGroup9 => 9,
                    _ => 1,
                };
                if self.is_ctrl_pressed() {
                    log::debug!("Command: Assign Control Group {}", group_num);
                    let ids = self.selection_state.selected_objects.clone();
                    self.selection_state
                        .control_groups
                        .insert(group_num as u8, ids.clone());
                    self.command_events
                        .push_back(RtsCommandEvent::AssignControlGroup {
                            group: group_num as u8,
                            object_ids: ids,
                        });
                } else {
                    log::debug!("Command: Select Control Group {}", group_num);
                    if let Some(ids) = self.selection_state.control_groups.get(&(group_num as u8)) {
                        self.selection_state.selected_objects = ids.clone();
                    }
                    self.command_events
                        .push_back(RtsCommandEvent::RecallControlGroup {
                            group: group_num as u8,
                        });
                }
            }
            RtsCommand::ToggleDebug => {
                log::debug!("Command: Toggle Debug Info");
                self.command_events.push_back(RtsCommandEvent::ToggleDebug);
            }
            RtsCommand::ToggleMusic => {
                log::debug!("Command: Toggle Background Music");
                self.command_events.push_back(RtsCommandEvent::ToggleMusic);
            }
            _ => {
                log::debug!("Command: {:?}", command);
            }
        }
    }

    /// Drain all queued command events for processing by higher-level systems.
    pub fn drain_command_events(&mut self) -> Vec<RtsCommandEvent> {
        self.command_events.drain(..).collect()
    }

    /// Check if a key is currently pressed
    pub fn is_key_pressed(&self, key: &Key) -> bool {
        self.keys_pressed.contains(key)
    }

    /// Check if a key was just pressed this frame
    pub fn is_key_just_pressed(&self, key: &Key) -> bool {
        self.keys_this_frame.contains(key)
    }

    /// Check if a key was just released this frame
    pub fn is_key_just_released(&self, key: &Key) -> bool {
        self.keys_released_this_frame.contains(key)
    }

    /// Check if Ctrl is pressed
    pub fn is_ctrl_pressed(&self) -> bool {
        self.keys_pressed.contains(&Key::Named(NamedKey::Control))
    }

    /// Check if Shift is pressed
    pub fn is_shift_pressed(&self) -> bool {
        self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
    }

    /// Check if Alt is pressed
    pub fn is_alt_pressed(&self) -> bool {
        self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
    }

    /// Get current mouse position
    pub fn get_mouse_position(&self) -> Vec2 {
        self.mouse_state.position
    }

    /// Get mouse delta this frame
    pub fn get_mouse_delta(&self) -> Vec2 {
        self.mouse_state.delta
    }

    /// Get current camera state
    pub fn get_camera(&self) -> &CameraController {
        &self.camera_controller
    }

    /// Get mutable camera reference
    pub fn get_camera_mut(&mut self) -> &mut CameraController {
        &mut self.camera_controller
    }

    /// Get current selection state
    pub fn get_selection(&self) -> &SelectionState {
        &self.selection_state
    }

    /// Get mutable selection reference
    pub fn get_selection_mut(&mut self) -> &mut SelectionState {
        &mut self.selection_state
    }

    /// Get left mouse button state
    pub fn get_left_button_state(&self) -> ButtonState {
        self.mouse_state.left_button
    }

    /// Get right mouse button state
    pub fn get_right_button_state(&self) -> ButtonState {
        self.mouse_state.right_button
    }

    /// Get middle mouse button state
    pub fn get_middle_button_state(&self) -> ButtonState {
        self.mouse_state.middle_button
    }

    /// Convert screen coordinates to world coordinates
    pub fn screen_to_world(&self, screen_pos: Vec2, window_size: (f32, f32)) -> Vec3 {
        // Simple orthographic projection.
        //
        // This input pipeline treats the camera as a top-down RTS view and maps screen space into
        // a ground-plane world position directly. Higher-fidelity 3D picking is handled by the
        // WW3D view code in the full GameClient pipeline.
        let ndc_x = (screen_pos.x / window_size.0) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / window_size.1) * 2.0;

        Vec3::new(
            ndc_x * self.camera_controller.zoom + self.camera_controller.position.x,
            0.0,
            ndc_y * self.camera_controller.zoom + self.camera_controller.position.z,
        )
    }

    /// Set key binding
    pub fn set_key_binding(&mut self, key: Key, command: RtsCommand) {
        self.key_bindings.insert(key, command);
    }

    /// Remove key binding
    pub fn remove_key_binding(&mut self, key: &Key) {
        self.key_bindings.remove(key);
    }

    /// Get selection box for rendering
    pub fn get_selection_box(&self) -> Option<(Vec2, Vec2)> {
        if self.selection_state.is_box_selecting {
            self.selection_state
                .selection_box_start
                .map(|start| (start, self.selection_state.selection_box_current))
        } else {
            None
        }
    }

    /// Reset input system (for new games/maps)
    pub fn reset(&mut self) {
        self.keys_pressed.clear();
        self.keys_this_frame.clear();
        self.keys_released_this_frame.clear();
        self.selection_state = SelectionState::default();
        self.mouse_state = MouseState::default();
        self.camera_controller = CameraController::default();
        self.last_click_time = None;
    }
}

/// Utility functions for input conversion
impl RtsInputSystem {
    /// Convert winit mouse button to our internal representation
    pub fn convert_mouse_button(button: MouseButton) -> Option<RtsCommand> {
        match button {
            MouseButton::Left => Some(RtsCommand::LeftClick),
            MouseButton::Right => Some(RtsCommand::RightClick),
            MouseButton::Middle => Some(RtsCommand::MiddleClick),
            _ => None,
        }
    }

    /// Convert physical key code to logical key for compatibility
    pub fn physical_to_logical(physical: PhysicalKey) -> Option<Key> {
        match physical {
            PhysicalKey::Code(KeyCode::KeyW) => Some(Key::Character("w".into())),
            PhysicalKey::Code(KeyCode::KeyA) => Some(Key::Character("a".into())),
            PhysicalKey::Code(KeyCode::KeyS) => Some(Key::Character("s".into())),
            PhysicalKey::Code(KeyCode::KeyD) => Some(Key::Character("d".into())),
            PhysicalKey::Code(KeyCode::Space) => Some(Key::Named(NamedKey::Space)),
            PhysicalKey::Code(KeyCode::Escape) => Some(Key::Named(NamedKey::Escape)),
            PhysicalKey::Code(KeyCode::Tab) => Some(Key::Named(NamedKey::Tab)),
            PhysicalKey::Code(KeyCode::Delete) => Some(Key::Named(NamedKey::Delete)),
            PhysicalKey::Code(KeyCode::Enter) => Some(Key::Named(NamedKey::Enter)),
            PhysicalKey::Code(KeyCode::F1) => Some(Key::Named(NamedKey::F1)),
            _ => None,
        }
    }
}
