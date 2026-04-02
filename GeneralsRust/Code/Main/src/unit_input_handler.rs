//! Unit Input Handler - Integration between Input System and Unit Control
//!
//! This module provides the bridge between the RTS input system and the
//! unit control system, handling mouse clicks, keyboard commands, and
//! translating them into unit actions.

use crate::game_logic::{GameLogic, Team};
use crate::input_system::{ButtonState, RtsCommand, RtsInputSystem};
use crate::unit_control::UnitControlSystem;
use glam::Vec2;
use std::sync::Arc;
use std::sync::Mutex as AsyncMutex;
use winit::keyboard::{Key, NamedKey};

/// Integration handler for unit control input
pub struct UnitInputHandler {
    /// Unit control system
    unit_control: UnitControlSystem,

    /// Local player ID
    local_player_id: u32,

    /// Last frame we processed input
    _last_frame: u32,

    /// Input state tracking
    left_click_processed: bool,
    right_click_processed: bool,
    drag_in_progress: bool,
    drag_start_pos: Option<Vec2>,
    current_mouse_pos: Vec2,
}

impl UnitInputHandler {
    /// Create new unit input handler
    pub fn new(window_size: (f32, f32), local_player_team: Team, local_player_id: u32) -> Self {
        Self {
            unit_control: UnitControlSystem::new(window_size, local_player_team, local_player_id),
            local_player_id,
            _last_frame: 0,
            left_click_processed: false,
            right_click_processed: false,
            drag_in_progress: false,
            drag_start_pos: None,
            current_mouse_pos: Vec2::ZERO,
        }
    }

    /// Update window size
    pub fn set_window_size(&mut self, width: f32, height: f32) {
        self.unit_control.set_window_size(width, height);
    }

    /// Process input from the RTS input system
    pub async fn process_input(
        &mut self,
        input_system: &mut RtsInputSystem,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
    ) {
        // Update camera
        self.unit_control.update_camera(input_system);

        // Update hover state
        let mouse_pos = input_system.get_mouse_position();
        self.current_mouse_pos = mouse_pos;
        self.unit_control.update_hover(mouse_pos, game_logic).await;

        // Process mouse input
        self.process_mouse_input(input_system, game_logic).await;

        // Process keyboard input
        self.process_keyboard_input(input_system, game_logic).await;

        // Update input state for next frame
        self.left_click_processed = false;
        self.right_click_processed = false;
    }

    /// Process mouse input events
    async fn process_mouse_input(
        &mut self,
        input_system: &mut RtsInputSystem,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
    ) {
        let mouse_pos = input_system.get_mouse_position();
        let shift_pressed = input_system.is_shift_pressed();
        let ctrl_pressed = input_system.is_ctrl_pressed();

        // Handle left mouse button
        match input_system.get_left_button_state() {
            ButtonState::Pressed if !self.left_click_processed => {
                self.left_click_processed = true;

                // Check if this is the start of a drag operation
                if !self.drag_in_progress {
                    self.drag_start_pos = Some(mouse_pos);
                }
            }
            ButtonState::Released if self.left_click_processed => {
                // Handle click or drag completion
                if let Some(start_pos) = self.drag_start_pos {
                    let drag_distance = (mouse_pos - start_pos).length();

                    if drag_distance > 5.0 {
                        // This was a drag operation - handle box selection
                        self.unit_control
                            .handle_box_selection(start_pos, mouse_pos, shift_pressed, game_logic)
                            .await;
                        self.drag_in_progress = false;
                    } else {
                        // This was a click - handle unit selection
                        self.unit_control
                            .handle_left_click(mouse_pos, shift_pressed, ctrl_pressed, game_logic)
                            .await;
                    }
                }
                self.drag_start_pos = None;
            }
            _ => {}
        }

        // Handle right mouse button
        match input_system.get_right_button_state() {
            ButtonState::Pressed if !self.right_click_processed => {
                self.right_click_processed = true;
                self.unit_control
                    .handle_right_click(mouse_pos, game_logic)
                    .await;
            }
            _ => {}
        }

        // Update drag selection visuals
        if let Some(start_pos) = self.drag_start_pos {
            let drag_distance = (mouse_pos - start_pos).length();
            if drag_distance > 5.0 {
                self.drag_in_progress = true;
                // Update selection box in input system for rendering
                input_system.get_selection_mut().selection_box_start = Some(start_pos);
                input_system.get_selection_mut().selection_box_current = mouse_pos;
                input_system.get_selection_mut().is_box_selecting = true;
            }
        } else {
            // Clear selection box
            self.drag_in_progress = false;
            input_system.get_selection_mut().is_box_selecting = false;
            input_system.get_selection_mut().selection_box_start = None;
            input_system.get_selection_mut().selection_box_current = self.current_mouse_pos;
        }
    }

    /// Process keyboard input events
    async fn process_keyboard_input(
        &mut self,
        input_system: &mut RtsInputSystem,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
    ) {
        // Control groups (0-9) - 10 groups total like C++ Generals
        for i in 0..=9 {
            let key = Key::Character(i.to_string().into());
            if input_system.is_key_just_pressed(&key) {
                if input_system.is_ctrl_pressed() {
                    // Ctrl+Number: Assign control group
                    self.unit_control
                        .assign_control_group(i as u8, game_logic)
                        .await;
                } else {
                    // Number: Select control group
                    self.unit_control
                        .select_control_group(i as u8, game_logic)
                        .await;
                }
            }
        }

        // Ctrl+A: Select all units
        if input_system.is_ctrl_pressed()
            && input_system.is_key_just_pressed(&Key::Character("a".into()))
        {
            self.unit_control.select_all_units(game_logic).await;
        }

        // Delete: Destroy selected units (debug feature)
        if input_system.is_key_just_pressed(&Key::Named(NamedKey::Delete)) {
            self.delete_selected_units(game_logic).await;
        }

        // Tab: Cycle through units
        if input_system.is_key_just_pressed(&Key::Named(NamedKey::Tab)) {
            self.cycle_selected_units(game_logic).await;
        }

        // F1: Toggle debug mode
        if input_system.is_key_just_pressed(&Key::Named(NamedKey::F1)) {
            self.unit_control
                .set_debug_mode(!self.unit_control.debug_mode);
        }

        // S: Stop command
        if input_system.is_key_just_pressed(&Key::Character("s".into()))
            && !input_system.is_ctrl_pressed()
        {
            self.unit_control.command_stop(game_logic).await;
        }

        // H: Hold position command
        if input_system.is_key_just_pressed(&Key::Character("h".into())) {
            self.unit_control.command_hold_position(game_logic).await;
        }

        // G: Guard command
        if input_system.is_key_just_pressed(&Key::Character("g".into())) {
            self.unit_control.command_guard(game_logic).await;
        }
    }

    /// Delete selected units (debug feature)
    async fn delete_selected_units(&mut self, game_logic: &Arc<AsyncMutex<GameLogic>>) {
        let selected_objects = self.unit_control.get_selected_objects().to_vec();

        if selected_objects.is_empty() {
            log::debug!("No units selected to delete");
            return;
        }

        let Ok(mut logic) = game_logic.lock() else {
            log::warn!("Skipping delete_selected_units: game logic lock poisoned");
            return;
        };

        for &object_id in &selected_objects {
            logic.destroy_object(object_id);
        }

        log::debug!("Destroyed {} selected units", selected_objects.len());

        // Clear selection since units are destroyed
        self.unit_control.selected_objects.clear();
        logic.select_objects(self.local_player_id, vec![]);
    }

    /// Cycle through selected units
    async fn cycle_selected_units(&mut self, game_logic: &Arc<AsyncMutex<GameLogic>>) {
        let Ok(logic) = game_logic.lock() else {
            log::warn!("Skipping cycle_selected_units read: game logic lock poisoned");
            return;
        };

        // Get all selectable units for the local player
        let mut all_units: Vec<crate::game_logic::ObjectId> = logic
            .get_objects()
            .iter()
            .filter(|(_, obj)| {
                obj.team == self.unit_control.local_player_team && obj.is_selectable()
            })
            .map(|(&id, _)| id)
            .collect();

        if all_units.is_empty() {
            log::debug!("No units to cycle through");
            return;
        }

        all_units.sort_by_key(|id| id.0); // Sort by ID for consistent order

        // Find currently selected unit
        let current_selection = self.unit_control.get_selected_objects();
        let next_unit = if let Some(&current_id) = current_selection.first() {
            // Find next unit in sequence
            if let Some(current_index) = all_units.iter().position(|&id| id == current_id) {
                let next_index = (current_index + 1) % all_units.len();
                all_units[next_index]
            } else {
                all_units[0]
            }
        } else {
            all_units[0]
        };

        // Select the next unit
        self.unit_control.selected_objects.clear();
        self.unit_control.selected_objects.push(next_unit);

        drop(logic);
        let Ok(mut logic) = game_logic.lock() else {
            log::warn!("Skipping cycle_selected_units write: game logic lock poisoned");
            return;
        };
        logic.select_objects(self.local_player_id, vec![next_unit]);

        log::debug!("Cycled to unit {}", next_unit);
    }

    /// Get current selection for UI display
    pub fn get_selected_objects(&self) -> &[crate::game_logic::ObjectId] {
        self.unit_control.get_selected_objects()
    }

    /// Get hovered object for UI display
    pub fn get_hovered_object(&self) -> Option<crate::game_logic::ObjectId> {
        self.unit_control.get_hovered_object()
    }

    /// Get selection center for camera focusing
    pub async fn get_selection_center(
        &self,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
    ) -> Option<glam::Vec3> {
        let Ok(logic) = game_logic.lock() else {
            log::warn!("Skipping get_selection_center: game logic lock poisoned");
            return None;
        };
        self.unit_control.get_selection_center(&logic)
    }

    /// Check if an object is selected
    pub fn is_object_selected(&self, object_id: crate::game_logic::ObjectId) -> bool {
        self.unit_control.is_object_selected(object_id)
    }

    /// Get unit control system for direct access
    pub fn get_unit_control(&self) -> &UnitControlSystem {
        &self.unit_control
    }

    /// Get mutable unit control system
    pub fn get_unit_control_mut(&mut self) -> &mut UnitControlSystem {
        &mut self.unit_control
    }

    /// Handle window events that affect input
    pub fn handle_window_event(&mut self, event: &winit::event::WindowEvent) {
        if let winit::event::WindowEvent::Resized(new_size) = event {
            self.set_window_size(new_size.width as f32, new_size.height as f32);
        }
    }

    /// Get current drag selection box for UI rendering
    pub fn get_selection_box(&self) -> Option<(Vec2, Vec2)> {
        if self.drag_in_progress {
            if let Some(start_pos) = self.drag_start_pos {
                Some((start_pos, self.current_mouse_pos))
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_selection_box_returns_drag_bounds() {
        let mut handler = UnitInputHandler::new((1280.0, 720.0), Team::USA, 0);
        handler.drag_in_progress = true;
        handler.drag_start_pos = Some(Vec2::new(100.0, 120.0));
        handler.current_mouse_pos = Vec2::new(300.0, 340.0);

        assert_eq!(
            handler.get_selection_box(),
            Some((Vec2::new(100.0, 120.0), Vec2::new(300.0, 340.0)))
        );
    }

    #[test]
    fn get_selection_box_none_when_not_dragging() {
        let handler = UnitInputHandler::new((1280.0, 720.0), Team::USA, 0);
        assert_eq!(handler.get_selection_box(), None);
    }
}

/// Helper functions for command processing
impl UnitInputHandler {
    /// Execute RTS command through unit control system
    pub async fn execute_rts_command(
        &mut self,
        command: RtsCommand,
        input_system: &RtsInputSystem,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
    ) {
        match command {
            RtsCommand::LeftClick => {
                let mouse_pos = input_system.get_mouse_position();
                let shift_pressed = input_system.is_shift_pressed();
                let ctrl_pressed = input_system.is_ctrl_pressed();
                self.unit_control
                    .handle_left_click(mouse_pos, shift_pressed, ctrl_pressed, game_logic)
                    .await;
            }
            RtsCommand::RightClick => {
                let mouse_pos = input_system.get_mouse_position();
                self.unit_control
                    .handle_right_click(mouse_pos, game_logic)
                    .await;
            }
            RtsCommand::SelectAll => {
                self.unit_control.select_all_units(game_logic).await;
            }
            RtsCommand::DeleteSelected => {
                self.delete_selected_units(game_logic).await;
            }
            RtsCommand::CycleUnits => {
                self.cycle_selected_units(game_logic).await;
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

                if input_system.is_ctrl_pressed() {
                    self.unit_control
                        .assign_control_group(group_num as u8, game_logic)
                        .await;
                } else {
                    self.unit_control
                        .select_control_group(group_num as u8, game_logic)
                        .await;
                }
            }
            // Note: Group 0 is handled via direct keyboard input (Ctrl+0 or just 0)
            // since RtsCommand enum doesn't include ControlGroup0
            _ => {
                // Other commands not handled by unit control
                log::debug!("Unhandled RTS command: {:?}", command);
            }
        }
    }
}
