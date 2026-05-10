//! # Input Bridge Module
//!
//! Converts raw input events from the `input` subsystem into game commands that the
//! `GameLogic` command system can process. This sits between the low-level device input
//! and the high-level game logic, translating mouse/keyboard events into RTS commands
//! (move, attack, select, guard, etc.) matching the C++ GameClient input flow.
//!
//! ## C++ Reference
//!
//! The C++ game client translates raw input through `Mouse::createStreamMessages()` and
//! `Keyboard::createStreamMessages()` which emit `GameMessage` types (MSG_RAW_MOUSE_*,
//! MSG_RAW_KEY_*). The `InGameUI` class then interprets those messages, taking into account
//! the current selection state, cursor mode, and active context to produce network-ready
//! commands (MSG_DO_MOVETO, MSG_DO_ATTACK_OBJECT, etc.).
//!
//! ## Architecture
//!
//! ```text
//!  winit events  ->  input::{Keyboard, Mouse}  ->  InputEvent (from events.rs)
//!                                                    |
//!                                          GameInputHandler::process_input_event()
//!                                                    |
//!                                          SelectionSystem (box-select, click, group)
//!                                                    |
//!                                         Vec<GameCommand>  (CommandType + args)
//!                                                    |
//!                                        GameLogic command queue / dispatch
//! ```
//!
//! ## Example
//!
//! ```ignore
//! use game_client_rust::input_bridge::{GameInputHandler, GameCommand};
//!
//! let mut handler = GameInputHandler::new(0); // player 0
//! let commands = handler.process_input_event(&my_input_event);
//! for cmd in commands {
//!     // submit cmd to the dispatch system
//! }
//! ```

use std::collections::HashMap;
use std::time::Instant;

use crate::input::{InputEvent, KeyCode, KeyModifiers, MouseButton};
use gamelogic::commands::CommandType;

// ---------------------------------------------------------------------------
// GameCommand -- what the bridge produces for the game logic layer
// ---------------------------------------------------------------------------

/// A command produced by the input bridge, ready to be submitted to the
/// game logic dispatch / command queue.
#[derive(Debug, Clone)]
pub struct GameCommand {
    /// The high-level command type (mirrors C++ GameMessage::Type).
    pub command_type: CommandType,
    /// Ordered arguments attached to the command.
    pub arguments: Vec<GameCommandArg>,
    /// The player that issued the command.
    pub player_index: i32,
}

/// Argument values that can be attached to a `GameCommand`.
#[derive(Debug, Clone, Copy)]
pub enum GameCommandArg {
    Integer(i32),
    Real(f32),
    ObjectID(u32),
    Location(f32, f32, f32),
    Pixel(i32, i32),
    PixelRegion {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    },
}

impl GameCommand {
    /// Convenience: create a move-to-position command.
    pub fn move_to(objects: Vec<u32>, x: f32, y: f32, z: f32, player: i32) -> Self {
        let mut args = vec![GameCommandArg::Location(x, y, z)];
        for id in objects {
            args.push(GameCommandArg::ObjectID(id));
        }
        Self {
            command_type: CommandType::DoMoveTo,
            arguments: args,
            player_index: player,
        }
    }

    /// Convenience: create an attack-object command.
    pub fn attack_object(attackers: Vec<u32>, target: u32, player: i32) -> Self {
        let mut args = vec![GameCommandArg::ObjectID(target)];
        for id in attackers {
            args.push(GameCommandArg::ObjectID(id));
        }
        Self {
            command_type: CommandType::DoAttackObject,
            arguments: args,
            player_index: player,
        }
    }

    /// Convenience: create a stop command.
    pub fn stop(objects: Vec<u32>, player: i32) -> Self {
        let args: Vec<GameCommandArg> = objects
            .iter()
            .map(|&id| GameCommandArg::ObjectID(id))
            .collect();
        Self {
            command_type: CommandType::DoStop,
            arguments: args,
            player_index: player,
        }
    }

    /// Convenience: create a scatter command.
    pub fn scatter(objects: Vec<u32>, player: i32) -> Self {
        let args: Vec<GameCommandArg> = objects
            .iter()
            .map(|&id| GameCommandArg::ObjectID(id))
            .collect();
        Self {
            command_type: CommandType::DoScatter,
            arguments: args,
            player_index: player,
        }
    }

    /// Convenience: create a guard-position command.
    pub fn guard_position(objects: Vec<u32>, x: f32, y: f32, z: f32, player: i32) -> Self {
        let mut args = vec![GameCommandArg::Location(x, y, z)];
        for id in objects {
            args.push(GameCommandArg::ObjectID(id));
        }
        Self {
            command_type: CommandType::DoGuardPosition,
            arguments: args,
            player_index: player,
        }
    }

    /// Convenience: create an attack-move command.
    pub fn attack_move_to(objects: Vec<u32>, x: f32, y: f32, z: f32, player: i32) -> Self {
        let mut args = vec![GameCommandArg::Location(x, y, z)];
        for id in objects {
            args.push(GameCommandArg::ObjectID(id));
        }
        Self {
            command_type: CommandType::DoAttackMoveTo,
            arguments: args,
            player_index: player,
        }
    }

    /// Convenience: create a force-move command.
    pub fn force_move_to(objects: Vec<u32>, x: f32, y: f32, z: f32, player: i32) -> Self {
        let mut args = vec![GameCommandArg::Location(x, y, z)];
        for id in objects {
            args.push(GameCommandArg::ObjectID(id));
        }
        Self {
            command_type: CommandType::DoForceMoveTo,
            arguments: args,
            player_index: player,
        }
    }

    /// Convenience: create an area-selection command.
    pub fn area_selection(left: i32, top: i32, right: i32, bottom: i32, player: i32) -> Self {
        Self {
            command_type: CommandType::AreaSelection,
            arguments: vec![GameCommandArg::PixelRegion {
                left,
                top,
                right,
                bottom,
            }],
            player_index: player,
        }
    }

    /// Convenience: create a create-selected-group command (Ctrl+N).
    pub fn create_selected_group(group_index: u8, player: i32) -> Self {
        Self {
            command_type: CommandType::CreateSelectedGroup,
            arguments: vec![GameCommandArg::Integer(group_index as i32)],
            player_index: player,
        }
    }

    /// Convenience: create a selected-group-command (pressing N with no modifiers).
    pub fn select_group(group_index: u8, player: i32) -> Self {
        Self {
            command_type: CommandType::SelectedGroupCommand,
            arguments: vec![GameCommandArg::Integer(group_index as i32)],
            player_index: player,
        }
    }

    /// Convenience: create a waypoint add command.
    pub fn add_waypoint(objects: Vec<u32>, x: f32, y: f32, z: f32, player: i32) -> Self {
        let mut args = vec![GameCommandArg::Location(x, y, z)];
        for id in objects {
            args.push(GameCommandArg::ObjectID(id));
        }
        Self {
            command_type: CommandType::AddWaypoint,
            arguments: args,
            player_index: player,
        }
    }
}

// ---------------------------------------------------------------------------
// CursorMode -- what the cursor represents (from the C++ InGameUI)
// ---------------------------------------------------------------------------

/// Mirrors the C++ InGameUI cursor mode. Determines how right-clicks are
/// interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMode {
    /// Default -- left-click selects, right-click issues context action.
    Normal,
    /// Left-click issues the special power at the target location.
    SpecialPowerAtLocation,
    /// Left-click targets a specific object with the special power.
    SpecialPowerAtObject,
    /// Force-attack mode (Ctrl key held or special ability).
    ForceAttack,
    /// Force-move mode (Alt key held).
    ForceMove,
    /// Waypoint placement mode.
    Waypoint,
    /// Attack-move mode (A key toggled).
    AttackMove,
    /// Set rally point for a production structure.
    SetRallyPoint,
    /// Place beacon mode.
    PlaceBeacon,
}

impl Default for CursorMode {
    fn default() -> Self {
        CursorMode::Normal
    }
}

// ---------------------------------------------------------------------------
// GameInputHandler -- the main bridge
// ---------------------------------------------------------------------------

/// Trait used by `GameInputHandler` to resolve screen positions to world
/// positions and to query which objects live under the cursor.  The real
/// implementation lives in the game client; the bridge uses it as an opaque
/// callback.
pub trait InputContext {
    /// Convert a screen-space pixel position to a world-space 3-D position.
    fn screen_to_world(&self, screen_x: f32, screen_y: f32) -> (f32, f32, f32);

    /// Return the ID of the drawable under the given screen pixel, or `None`.
    fn pick_object_at(&self, screen_x: f32, screen_y: f32) -> Option<u32>;

    /// Return the ID of the drawable under the cursor (for the current frame).
    fn hovered_object_id(&self) -> Option<u32>;
}

/// A no-op context used for testing or when no world is loaded.
#[derive(Debug, Clone, Copy)]
pub struct NullInputContext;

impl InputContext for NullInputContext {
    fn screen_to_world(&self, x: f32, y: f32) -> (f32, f32, f32) {
        (x, 0.0, y)
    }

    fn pick_object_at(&self, _x: f32, _y: f32) -> Option<u32> {
        None
    }

    fn hovered_object_id(&self) -> Option<u32> {
        None
    }
}

/// Minimum drag distance (in pixels) to distinguish a drag from a click.
/// Matches C++ `Mouse::m_dragTolerance`.
const DRAG_TOLERANCE_PX: i32 = 5;

/// Central input bridge that converts `InputEvent` values into `GameCommand`
/// values, respecting selection state, modifier keys, and cursor mode.
pub struct GameInputHandler {
    /// Player index that owns this handler.
    pub player_index: i32,

    /// Currently selected object IDs.
    pub selected_objects: Vec<u32>,

    /// Current cursor mode (determines right-click semantics).
    pub cursor_mode: CursorMode,

    /// Mouse position at the start of a left-button drag.
    drag_anchor: Option<(f32, f32)>,

    /// Whether we are currently in a drag (left button held + past threshold).
    dragging: bool,

    /// Timestamp of the last left-click (for double-click detection).
    last_left_click_time: Option<Instant>,

    /// Position of the last left-click.
    last_left_click_pos: Option<(f32, f32)>,

    /// Control groups 0-9, each a list of object IDs.
    control_groups: [Vec<u32>; 10],

    /// Context trait for screen-to-world lookups.
    context: Box<dyn InputContext>,

    /// Double-click time threshold in milliseconds.
    pub double_click_time_ms: u64,
}

impl std::fmt::Debug for GameInputHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameInputHandler")
            .field("player_index", &self.player_index)
            .field("selected_objects", &self.selected_objects)
            .field("cursor_mode", &self.cursor_mode)
            .field("dragging", &self.dragging)
            .finish()
    }
}

impl GameInputHandler {
    /// Create a new handler for the given player.
    pub fn new(player_index: i32) -> Self {
        Self::with_context(player_index, Box::new(NullInputContext))
    }

    /// Create a new handler with an explicit `InputContext`.
    pub fn with_context(player_index: i32, context: Box<dyn InputContext>) -> Self {
        Self {
            player_index,
            selected_objects: Vec::new(),
            cursor_mode: CursorMode::Normal,
            drag_anchor: None,
            dragging: false,
            last_left_click_time: None,
            last_left_click_pos: None,
            control_groups: Default::default(),
            context,
            double_click_time_ms: 500,
        }
    }

    /// Replace the input context (e.g. when a new world is loaded).
    pub fn set_context(&mut self, ctx: Box<dyn InputContext>) {
        self.context = ctx;
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Process a single input event and return any resulting game commands.
    pub fn process_input_event(
        &mut self,
        event: &InputEvent,
        modifiers: KeyModifiers,
    ) -> Vec<GameCommand> {
        let mut commands = Vec::new();

        match event {
            // --- Mouse button press / release ------------------------------------
            InputEvent::MouseButtonPressed {
                button,
                x,
                y,
                click_count,
                timestamp,
            } => match button {
                MouseButton::Left => {
                    self.handle_left_button_down(
                        *x,
                        *y,
                        *click_count,
                        *timestamp,
                        modifiers,
                        &mut commands,
                    );
                }
                MouseButton::Right => {
                    self.handle_right_button_down(*x, *y, modifiers, &mut commands);
                }
                _ => {}
            },

            InputEvent::MouseButtonReleased { button, x, y, .. } => match button {
                MouseButton::Left => {
                    self.handle_left_button_up(*x, *y, modifiers, &mut commands);
                }
                _ => {}
            },

            // --- Mouse drag (continuous) ----------------------------------------
            InputEvent::MouseMoved { x, y, .. } => {
                // Check if the drag threshold has been crossed.
                if let Some((ax, ay)) = self.drag_anchor {
                    let dx = (x - ax).abs() as i32;
                    let dy = (y - ay).abs() as i32;
                    if dx > DRAG_TOLERANCE_PX || dy > DRAG_TOLERANCE_PX {
                        self.dragging = true;
                    }
                }
            }

            // --- Keyboard press -------------------------------------------------
            InputEvent::KeyPressed {
                key,
                modifiers: mods,
                ..
            } => {
                self.handle_key_press(*key, *mods, &mut commands);
            }

            // --- Scroll wheel ---------------------------------------------------
            InputEvent::MouseWheel { delta_y, .. } => {
                // Scroll can be used for camera zoom; that is handled by the
                // camera system directly.  The bridge does not generate commands
                // for scroll.
                let _ = delta_y;
            }

            _ => {}
        }

        commands
    }

    /// Convenience: process a batch of events.
    pub fn process_input_events(
        &mut self,
        events: &[InputEvent],
        modifiers: KeyModifiers,
    ) -> Vec<GameCommand> {
        let mut all_commands = Vec::new();
        for event in events {
            all_commands.extend(self.process_input_event(event, modifiers));
        }
        all_commands
    }

    /// Retrieve a control group (0-9).
    pub fn get_control_group(&self, index: usize) -> &[u32] {
        if index < 10 {
            &self.control_groups[index]
        } else {
            &[]
        }
    }

    // -----------------------------------------------------------------------
    // Left mouse button handling
    // -----------------------------------------------------------------------

    fn handle_left_button_down(
        &mut self,
        x: f32,
        y: f32,
        click_count: u32,
        timestamp: Instant,
        modifiers: KeyModifiers,
        commands: &mut Vec<GameCommand>,
    ) {
        if self.is_special_power_mode() {
            // In special-power mode, left-click targets the ability.
            let world = self.context.screen_to_world(x, y);
            let target_id = self.context.pick_object_at(x, y);

            if let Some(obj_id) = target_id {
                commands.push(GameCommand {
                    command_type: CommandType::DoSpecialPowerAtObject,
                    arguments: vec![
                        GameCommandArg::ObjectID(obj_id),
                        GameCommandArg::Location(world.0, world.1, world.2),
                    ],
                    player_index: self.player_index,
                });
            } else {
                commands.push(GameCommand {
                    command_type: CommandType::DoSpecialPowerAtLocation,
                    arguments: vec![GameCommandArg::Location(world.0, world.1, world.2)],
                    player_index: self.player_index,
                });
            }
            self.cursor_mode = CursorMode::Normal;
            return;
        }

        if self.cursor_mode == CursorMode::SetRallyPoint {
            let world = self.context.screen_to_world(x, y);
            commands.push(GameCommand {
                command_type: CommandType::SetRallyPoint,
                arguments: vec![
                    GameCommandArg::Location(world.0, world.1, world.2),
                    // first selected object is the production structure
                    GameCommandArg::ObjectID(self.selected_objects.first().copied().unwrap_or(0)),
                ],
                player_index: self.player_index,
            });
            self.cursor_mode = CursorMode::Normal;
            return;
        }

        // C++ SelectionXlat ignores double-click selection while force-attack is active.
        let force_attack_active =
            modifiers.contains(KeyModifiers::CTRL) || self.cursor_mode == CursorMode::ForceAttack;

        // Double-click: select all units of the same type on screen.
        if click_count >= 2 && !force_attack_active {
            if let Some(clicked_id) = self.context.pick_object_at(x, y) {
                commands.push(GameCommand {
                    command_type: CommandType::MetaSelectMatchingUnits,
                    arguments: vec![GameCommandArg::ObjectID(clicked_id)],
                    player_index: self.player_index,
                });
            }
            // Reset single-click tracking after double-click is handled.
            self.last_left_click_time = None;
            self.last_left_click_pos = None;
            return;
        }

        // Record the anchor for potential drag.
        self.drag_anchor = Some((x, y));
        self.dragging = false;

        // If Ctrl is held, add/remove the clicked object from the selection
        // (multi-select).  Otherwise the selection will be resolved on release.
        if modifiers.contains(KeyModifiers::CTRL) {
            if let Some(clicked_id) = self.context.pick_object_at(x, y) {
                if self.selected_objects.contains(&clicked_id) {
                    self.selected_objects.retain(|&id| id != clicked_id);
                } else {
                    self.selected_objects.push(clicked_id);
                }
            }
        }
    }

    fn handle_left_button_up(
        &mut self,
        x: f32,
        y: f32,
        _modifiers: KeyModifiers,
        commands: &mut Vec<GameCommand>,
    ) {
        if let Some((ax, ay)) = self.drag_anchor.take() {
            if self.dragging {
                // Box selection -- emit an AreaSelection command.
                let left = ax.min(x) as i32;
                let top = ay.min(y) as i32;
                let right = ax.max(x) as i32;
                let bottom = ay.max(y) as i32;

                commands.push(GameCommand::area_selection(
                    left,
                    top,
                    right,
                    bottom,
                    self.player_index,
                ));
            } else {
                // Single click (not a drag).
                if let Some(clicked_id) = self.context.pick_object_at(x, y) {
                    // Clicked on an object -- select it (replace selection unless
                    // Ctrl was held, which was already handled in button-down).
                    if !_modifiers.contains(KeyModifiers::CTRL) {
                        self.selected_objects.clear();
                        self.selected_objects.push(clicked_id);
                    }
                } else {
                    // Clicked on empty ground -- deselect unless Shift is held.
                    if !_modifiers.contains(KeyModifiers::SHIFT) {
                        self.selected_objects.clear();
                    }
                }
            }

            self.dragging = false;
        }
    }

    // -----------------------------------------------------------------------
    // Right mouse button handling -- context-sensitive action
    // -----------------------------------------------------------------------

    fn handle_right_button_down(
        &mut self,
        x: f32,
        y: f32,
        modifiers: KeyModifiers,
        commands: &mut Vec<GameCommand>,
    ) {
        if self.selected_objects.is_empty() {
            return;
        }

        let world = self.context.screen_to_world(x, y);
        let target_id = self.context.pick_object_at(x, y);

        // Ctrl held = force-attack
        // Alt held = force-move
        if modifiers.contains(KeyModifiers::CTRL) {
            // Force-attack: attack target or ground.
            if let Some(tid) = target_id {
                commands.push(GameCommand::attack_object(
                    self.selected_objects.clone(),
                    tid,
                    self.player_index,
                ));
            } else {
                commands.push(GameCommand {
                    command_type: CommandType::DoForceAttackGround,
                    arguments: vec![
                        GameCommandArg::Location(world.0, world.1, world.2),
                        GameCommandArg::ObjectID(self.selected_objects[0]),
                    ],
                    player_index: self.player_index,
                });
            }
            return;
        }

        if modifiers.contains(KeyModifiers::ALT) {
            // Force-move (ignore enemies, move through).
            commands.push(GameCommand::force_move_to(
                self.selected_objects.clone(),
                world.0,
                world.1,
                world.2,
                self.player_index,
            ));
            return;
        }

        // Shift held = queue waypoint.
        if modifiers.contains(KeyModifiers::SHIFT) {
            commands.push(GameCommand::add_waypoint(
                self.selected_objects.clone(),
                world.0,
                world.1,
                world.2,
                self.player_index,
            ));
            return;
        }

        // Default right-click: context-sensitive.
        if let Some(tid) = target_id {
            // Check for special context actions against the target.
            // In the full game this depends on object kind / relationship
            // (friendly building = enter/garrison, damaged friendly = repair,
            // hostile = attack, etc.).  For now we issue an attack command
            // against non-owned objects and a move-towards for friendly ones.
            //
            // The real determination would call into the game logic to check
            // object ownership and capabilities.  We emit a hint-style
            // command that the logic layer can re-interpret.
            commands.push(GameCommand::attack_object(
                self.selected_objects.clone(),
                tid,
                self.player_index,
            ));
        } else {
            // Right-click on empty ground = move.
            if self.cursor_mode == CursorMode::AttackMove {
                commands.push(GameCommand::attack_move_to(
                    self.selected_objects.clone(),
                    world.0,
                    world.1,
                    world.2,
                    self.player_index,
                ));
            } else if self.cursor_mode == CursorMode::Waypoint {
                commands.push(GameCommand::add_waypoint(
                    self.selected_objects.clone(),
                    world.0,
                    world.1,
                    world.2,
                    self.player_index,
                ));
            } else {
                commands.push(GameCommand::move_to(
                    self.selected_objects.clone(),
                    world.0,
                    world.1,
                    world.2,
                    self.player_index,
                ));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Keyboard handling
    // -----------------------------------------------------------------------

    fn handle_key_press(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
        commands: &mut Vec<GameCommand>,
    ) {
        // Ctrl+1..0 = create control group
        // Shift+1..0 = add to control group
        // 1..0 (no modifiers) = select control group
        if let Some(group_idx) = digit_to_group_index(key) {
            if modifiers.contains(KeyModifiers::CTRL) {
                self.control_groups[group_idx] = self.selected_objects.clone();
                commands.push(GameCommand::create_selected_group(
                    group_idx as u8,
                    self.player_index,
                ));
                return;
            }
            if modifiers.contains(KeyModifiers::SHIFT) {
                // Add current selection to the group.
                let group = &mut self.control_groups[group_idx];
                for &id in &self.selected_objects {
                    if !group.contains(&id) {
                        group.push(id);
                    }
                }
                return;
            }
            // Plain number = recall the group.
            {
                let group = self.control_groups[group_idx].clone();
                if !group.is_empty() {
                    if modifiers.contains(KeyModifiers::ALT) {
                        // Alt+N = select group AND move camera to center of group.
                        self.selected_objects = group.clone();
                        commands.push(GameCommand::select_group(
                            group_idx as u8,
                            self.player_index,
                        ));
                    } else {
                        self.selected_objects = group.clone();
                        commands.push(GameCommand::select_group(
                            group_idx as u8,
                            self.player_index,
                        ));
                    }
                }
            }
            return;
        }

        // Game-command hotkeys (only when selection is non-empty).
        if !self.selected_objects.is_empty() {
            match key {
                KeyCode::S
                    if !modifiers.contains(KeyModifiers::CTRL)
                        && !modifiers.contains(KeyModifiers::ALT) =>
                {
                    // S = Stop
                    commands.push(GameCommand::stop(
                        self.selected_objects.clone(),
                        self.player_index,
                    ));
                }
                KeyCode::G => {
                    // G = Guard position (use current mouse position via context)
                    // We don't have the mouse position in key events, so the
                    // command is issued at the last known selected position.
                    commands.push(GameCommand {
                        command_type: CommandType::DoGuardPosition,
                        arguments: self
                            .selected_objects
                            .iter()
                            .map(|&id| GameCommandArg::ObjectID(id))
                            .collect(),
                        player_index: self.player_index,
                    });
                }
                KeyCode::X => {
                    // X = Scatter
                    commands.push(GameCommand::scatter(
                        self.selected_objects.clone(),
                        self.player_index,
                    ));
                }
                KeyCode::A if modifiers.contains(KeyModifiers::ALT) => {
                    // Alt+A = Attack-move mode toggle
                    self.cursor_mode = if self.cursor_mode == CursorMode::AttackMove {
                        CursorMode::Normal
                    } else {
                        CursorMode::AttackMove
                    };
                }
                KeyCode::A if modifiers.contains(KeyModifiers::CTRL) => {
                    // Ctrl+A = Select all player units
                    commands.push(GameCommand {
                        command_type: CommandType::MetaSelectAll,
                        arguments: Vec::new(),
                        player_index: self.player_index,
                    });
                }
                KeyCode::D if !modifiers.contains(KeyModifiers::CTRL) => {
                    // D = Deploy (for deployable units like Tunnel Networks)
                    commands.push(GameCommand {
                        command_type: CommandType::MetaDeploy,
                        arguments: self
                            .selected_objects
                            .iter()
                            .map(|&id| GameCommandArg::ObjectID(id))
                            .collect(),
                        player_index: self.player_index,
                    });
                }
                KeyCode::F if modifiers.contains(KeyModifiers::CTRL) => {
                    // Ctrl+F = Force-attack mode toggle
                    self.cursor_mode = if self.cursor_mode == CursorMode::ForceAttack {
                        CursorMode::Normal
                    } else {
                        CursorMode::ForceAttack
                    };
                }
                KeyCode::W if modifiers.contains(KeyModifiers::SHIFT) => {
                    // Shift+W = Add waypoint mode toggle
                    self.cursor_mode = if self.cursor_mode == CursorMode::Waypoint {
                        CursorMode::Normal
                    } else {
                        CursorMode::Waypoint
                    };
                }
                _ => {}
            }
        }

        // Meta keys that do not require selection.
        match key {
            KeyCode::Escape => {
                // ESC cancels the current mode.
                self.cursor_mode = CursorMode::Normal;
                self.selected_objects.clear();
            }
            KeyCode::Tab => {
                // Tab cycles through production buildings (MetaSelectNextWorker
                // / MetaSelectPrevWorker in the C++ code).
                if modifiers.contains(KeyModifiers::SHIFT) {
                    commands.push(GameCommand {
                        command_type: CommandType::MetaSelectPrevWorker,
                        arguments: Vec::new(),
                        player_index: self.player_index,
                    });
                } else {
                    commands.push(GameCommand {
                        command_type: CommandType::MetaSelectNextWorker,
                        arguments: Vec::new(),
                        player_index: self.player_index,
                    });
                }
            }
            KeyCode::H => {
                // H = View home (command center).
                commands.push(GameCommand {
                    command_type: CommandType::MetaViewCommandCenter,
                    arguments: Vec::new(),
                    player_index: self.player_index,
                });
            }
            KeyCode::C if modifiers.contains(KeyModifiers::CTRL) => {
                // Ctrl+C = View last radar event.
                commands.push(GameCommand {
                    command_type: CommandType::MetaViewLastRadarEvent,
                    arguments: Vec::new(),
                    player_index: self.player_index,
                });
            }
            // Camera rotation / zoom via keyboard.
            KeyCode::Q => {
                commands.push(GameCommand {
                    command_type: CommandType::MetaBeginCameraRotateLeft,
                    arguments: Vec::new(),
                    player_index: self.player_index,
                });
            }
            KeyCode::E => {
                commands.push(GameCommand {
                    command_type: CommandType::MetaBeginCameraRotateRight,
                    arguments: Vec::new(),
                    player_index: self.player_index,
                });
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn is_special_power_mode(&self) -> bool {
        matches!(
            self.cursor_mode,
            CursorMode::SpecialPowerAtLocation | CursorMode::SpecialPowerAtObject
        )
    }
}

/// Map a KeyCode for digits 1-9 and 0 to a control group index 0-9.
fn digit_to_group_index(key: KeyCode) -> Option<usize> {
    match key {
        KeyCode::Num1 => Some(0),
        KeyCode::Num2 => Some(1),
        KeyCode::Num3 => Some(2),
        KeyCode::Num4 => Some(3),
        KeyCode::Num5 => Some(4),
        KeyCode::Num6 => Some(5),
        KeyCode::Num7 => Some(6),
        KeyCode::Num8 => Some(7),
        KeyCode::Num9 => Some(8),
        KeyCode::Num0 => Some(9),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A test context that pretends the world origin is at (0,0,0) and the
    /// ground object with ID 42 lives at pixel (100, 200).
    #[derive(Debug, Clone)]
    struct TestContext {
        ground_object_id: Option<u32>,
    }

    impl TestContext {
        fn new() -> Self {
            Self {
                ground_object_id: Some(42),
            }
        }
        fn without_objects() -> Self {
            Self {
                ground_object_id: None,
            }
        }
    }

    impl InputContext for TestContext {
        fn screen_to_world(&self, x: f32, y: f32) -> (f32, f32, f32) {
            (x, 0.0, y)
        }
        fn pick_object_at(&self, x: f32, y: f32) -> Option<u32> {
            // Only detect our test object at a specific location.
            if (x - 100.0).abs() < 10.0 && (y - 200.0).abs() < 10.0 {
                self.ground_object_id
            } else {
                None
            }
        }
        fn hovered_object_id(&self) -> Option<u32> {
            self.ground_object_id
        }
    }

    #[test]
    fn test_right_click_move_to_empty_ground() {
        let mut handler =
            GameInputHandler::with_context(0, Box::new(TestContext::without_objects()));
        handler.selected_objects = vec![1, 2];

        let event = InputEvent::MouseButtonPressed {
            button: MouseButton::Right,
            x: 300.0,
            y: 400.0,
            click_count: 1,
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::empty());

        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command_type, CommandType::DoMoveTo);
    }

    #[test]
    fn test_right_click_attack_object() {
        let mut handler = GameInputHandler::with_context(0, Box::new(TestContext::new()));
        handler.selected_objects = vec![1];

        // Right-click on the test object at (100, 200).
        let event = InputEvent::MouseButtonPressed {
            button: MouseButton::Right,
            x: 100.0,
            y: 200.0,
            click_count: 1,
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::empty());

        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command_type, CommandType::DoAttackObject);
    }

    #[test]
    fn test_ctrl_left_double_click_does_not_select_matching_units() {
        let mut handler = GameInputHandler::with_context(0, Box::new(TestContext::new()));

        let event = InputEvent::MouseButtonPressed {
            button: MouseButton::Left,
            x: 100.0,
            y: 200.0,
            click_count: 2,
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::CTRL);

        assert!(cmds.is_empty());
    }

    #[test]
    fn test_force_attack_left_double_click_does_not_select_matching_units() {
        let mut handler = GameInputHandler::with_context(0, Box::new(TestContext::new()));
        handler.cursor_mode = CursorMode::ForceAttack;

        let event = InputEvent::MouseButtonPressed {
            button: MouseButton::Left,
            x: 100.0,
            y: 200.0,
            click_count: 2,
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::empty());

        assert!(cmds.is_empty());
    }

    #[test]
    fn test_ctrl_right_click_force_attack() {
        let mut handler =
            GameInputHandler::with_context(0, Box::new(TestContext::without_objects()));
        handler.selected_objects = vec![1];

        let event = InputEvent::MouseButtonPressed {
            button: MouseButton::Right,
            x: 300.0,
            y: 400.0,
            click_count: 1,
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::CTRL);

        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command_type, CommandType::DoForceAttackGround);
    }

    #[test]
    fn test_alt_right_click_force_move() {
        let mut handler =
            GameInputHandler::with_context(0, Box::new(TestContext::without_objects()));
        handler.selected_objects = vec![1];

        let event = InputEvent::MouseButtonPressed {
            button: MouseButton::Right,
            x: 300.0,
            y: 400.0,
            click_count: 1,
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::ALT);

        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command_type, CommandType::DoForceMoveTo);
    }

    #[test]
    fn test_key_s_stop() {
        let mut handler =
            GameInputHandler::with_context(0, Box::new(TestContext::without_objects()));
        handler.selected_objects = vec![5, 6];

        let event = InputEvent::KeyPressed {
            key: KeyCode::S,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::empty());

        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command_type, CommandType::DoStop);
    }

    #[test]
    fn test_key_x_scatter() {
        let mut handler =
            GameInputHandler::with_context(0, Box::new(TestContext::without_objects()));
        handler.selected_objects = vec![5];

        let event = InputEvent::KeyPressed {
            key: KeyCode::X,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::empty());

        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command_type, CommandType::DoScatter);
    }

    #[test]
    fn test_ctrl_1_creates_group() {
        let mut handler =
            GameInputHandler::with_context(0, Box::new(TestContext::without_objects()));
        handler.selected_objects = vec![10, 20, 30];

        let event = InputEvent::KeyPressed {
            key: KeyCode::Num1,
            modifiers: KeyModifiers::CTRL,
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::CTRL);

        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command_type, CommandType::CreateSelectedGroup);
        assert_eq!(handler.get_control_group(0), &[10, 20, 30]);
    }

    #[test]
    fn test_recall_group() {
        let mut handler =
            GameInputHandler::with_context(0, Box::new(TestContext::without_objects()));
        handler.control_groups[2] = vec![99];

        let event = InputEvent::KeyPressed {
            key: KeyCode::Num3, // group index 2
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::empty());

        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command_type, CommandType::SelectedGroupCommand);
        assert_eq!(handler.selected_objects, vec![99]);
    }

    #[test]
    fn test_escape_clears_selection_and_mode() {
        let mut handler =
            GameInputHandler::with_context(0, Box::new(TestContext::without_objects()));
        handler.selected_objects = vec![1, 2, 3];
        handler.cursor_mode = CursorMode::AttackMove;

        let event = InputEvent::KeyPressed {
            key: KeyCode::Escape,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };
        handler.process_input_event(&event, KeyModifiers::empty());

        assert!(handler.selected_objects.is_empty());
        assert_eq!(handler.cursor_mode, CursorMode::Normal);
    }

    #[test]
    fn test_no_commands_when_selection_empty_and_right_click() {
        let mut handler =
            GameInputHandler::with_context(0, Box::new(TestContext::without_objects()));
        assert!(handler.selected_objects.is_empty());

        let event = InputEvent::MouseButtonPressed {
            button: MouseButton::Right,
            x: 100.0,
            y: 100.0,
            click_count: 1,
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::empty());
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_shift_right_click_queues_waypoint() {
        let mut handler =
            GameInputHandler::with_context(0, Box::new(TestContext::without_objects()));
        handler.selected_objects = vec![1];

        let event = InputEvent::MouseButtonPressed {
            button: MouseButton::Right,
            x: 300.0,
            y: 400.0,
            click_count: 1,
            timestamp: Instant::now(),
        };
        let cmds = handler.process_input_event(&event, KeyModifiers::SHIFT);

        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command_type, CommandType::AddWaypoint);
    }
}
