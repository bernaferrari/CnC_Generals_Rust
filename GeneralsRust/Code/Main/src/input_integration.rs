#![allow(dead_code, unused_variables)]

//! Input Integration Layer
//!
//! This module connects the RTS input system to the GameLogic singleton,
//! translating input commands into game actions like unit selection,
//! movement, attack commands, and camera control.

use crate::game_logic::{GameLogic, ObjectId, Team};
use crate::input_system::{RtsCommandEvent, RtsInputSystem};
use crate::presentation_frame::PresentationFrame;
use glam::{Vec2, Vec3};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Input processor that bridges input system and game logic
pub struct InputProcessor {
    /// Reference to the input system
    input_system: Arc<Mutex<RtsInputSystem>>,

    /// Current player ID for command execution
    local_player_id: u32,

    /// Window dimensions for coordinate conversion
    window_size: (f32, f32),

    /// Last processed frame to avoid duplicate commands
    last_frame: u32,

    /// Debug mode flag
    debug_mode: bool,

    /// Music enabled flag
    music_enabled: bool,

    /// Control groups (0-9)
    control_groups: HashMap<u8, Vec<ObjectId>>,

    /// Dual-tick presentation snapshot for world pick residual (optional).
    presentation_frame: Option<PresentationFrame>,
}

impl InputProcessor {
    /// Create new input processor
    pub fn new(
        input_system: Arc<Mutex<RtsInputSystem>>,
        local_player_id: u32,
        window_size: (f32, f32),
    ) -> Self {
        Self {
            input_system,
            local_player_id,
            window_size,
            last_frame: 0,
            debug_mode: false,
            music_enabled: true,
            control_groups: HashMap::new(),
            presentation_frame: None,
        }
    }

    /// Install dual-tick presentation snapshot for pick residual.
    pub fn set_presentation_frame(&mut self, frame: Option<PresentationFrame>) {
        self.presentation_frame = frame;
    }

    fn local_player_team(&self, game_logic: &GameLogic) -> Team {
        game_logic
            .get_player(self.local_player_id)
            .map(|player| player.team)
            .unwrap_or(Team::Neutral)
    }

    /// Process input and execute game commands
    pub async fn process_input(&mut self, game_logic: &Arc<std::sync::Mutex<GameLogic>>) {
        // Get current frame from GameLogic
        let current_frame = {
            let logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());
            logic.get_frame()
        };

        // Skip if we already processed this frame
        if current_frame == self.last_frame {
            return;
        }
        self.last_frame = current_frame;

        enum ResolvedEvent {
            LeftClick {
                world_pos: Vec3,
                shift: bool,
                ctrl: bool,
            },
            RightClick {
                world_pos: Vec3,
            },
            DragSelect {
                start_screen: Vec2,
                end_screen: Vec2,
                start_world: Vec3,
                end_world: Vec3,
                shift: bool,
            },
            DoubleClickSelectSimilar {
                world_pos: Vec3,
            },
            SelectAll,
            DeleteSelected,
            CycleUnits,
            TogglePause,
            AssignControlGroup {
                group: u8,
            },
            RecallControlGroup {
                group: u8,
            },
            ToggleDebug,
            ToggleMusic,
        }

        let mut resolved = Vec::new();

        if let Ok(mut input) = self.input_system.try_lock() {
            let shift = input.is_shift_pressed();
            let ctrl = input.is_ctrl_pressed();
            let window_size = self.window_size;

            for ev in input.drain_command_events() {
                match ev {
                    RtsCommandEvent::LeftClick { screen_pos } => {
                        resolved.push(ResolvedEvent::LeftClick {
                            world_pos: input.screen_to_world(screen_pos, window_size),
                            shift,
                            ctrl,
                        });
                    }
                    RtsCommandEvent::RightClick { screen_pos } => {
                        resolved.push(ResolvedEvent::RightClick {
                            world_pos: input.screen_to_world(screen_pos, window_size),
                        });
                    }
                    RtsCommandEvent::MiddleClick { .. } => {}
                    RtsCommandEvent::DragSelect {
                        start_screen,
                        end_screen,
                    } => {
                        resolved.push(ResolvedEvent::DragSelect {
                            start_screen,
                            end_screen,
                            start_world: input.screen_to_world(start_screen, window_size),
                            end_world: input.screen_to_world(end_screen, window_size),
                            shift,
                        });
                    }
                    RtsCommandEvent::DoubleClickSelectSimilar { screen_pos } => {
                        resolved.push(ResolvedEvent::DoubleClickSelectSimilar {
                            world_pos: input.screen_to_world(screen_pos, window_size),
                        });
                    }
                    RtsCommandEvent::SelectAll => resolved.push(ResolvedEvent::SelectAll),
                    RtsCommandEvent::DeleteSelected { .. } => {
                        resolved.push(ResolvedEvent::DeleteSelected)
                    }
                    RtsCommandEvent::CycleUnits => resolved.push(ResolvedEvent::CycleUnits),
                    RtsCommandEvent::TogglePause => resolved.push(ResolvedEvent::TogglePause),
                    RtsCommandEvent::AssignControlGroup { group, .. } => {
                        resolved.push(ResolvedEvent::AssignControlGroup { group });
                    }
                    RtsCommandEvent::RecallControlGroup { group } => {
                        resolved.push(ResolvedEvent::RecallControlGroup { group });
                    }
                    RtsCommandEvent::ToggleDebug => resolved.push(ResolvedEvent::ToggleDebug),
                    RtsCommandEvent::ToggleMusic => resolved.push(ResolvedEvent::ToggleMusic),
                }
            }
        }

        for ev in resolved {
            match ev {
                ResolvedEvent::LeftClick {
                    world_pos,
                    shift,
                    ctrl,
                } => {
                    self.handle_left_click(world_pos, shift, ctrl, game_logic)
                        .await;
                }
                ResolvedEvent::RightClick { world_pos } => {
                    self.handle_right_click(world_pos, game_logic).await;
                }
                ResolvedEvent::DragSelect {
                    start_screen,
                    end_screen,
                    start_world,
                    end_world,
                    shift,
                } => {
                    self.handle_box_selection(
                        start_screen,
                        end_screen,
                        start_world,
                        end_world,
                        shift,
                        game_logic,
                    )
                    .await;
                }
                ResolvedEvent::DoubleClickSelectSimilar { world_pos } => {
                    self.select_similar_units(world_pos, game_logic).await;
                }
                ResolvedEvent::SelectAll => self.select_all_units(game_logic).await,
                ResolvedEvent::DeleteSelected => self.delete_selected_units(game_logic).await,
                ResolvedEvent::CycleUnits => self.cycle_units(game_logic).await,
                ResolvedEvent::TogglePause => self.toggle_pause(game_logic).await,
                ResolvedEvent::AssignControlGroup { group } => {
                    self.assign_control_group(group, game_logic).await;
                }
                ResolvedEvent::RecallControlGroup { group } => {
                    self.select_control_group(group, game_logic).await;
                }
                ResolvedEvent::ToggleDebug => self.toggle_debug_mode(),
                ResolvedEvent::ToggleMusic => self.toggle_music(game_logic),
            }
        }
    }

    /// Handle left mouse click for unit selection
    async fn handle_left_click(
        &mut self,
        world_pos: Vec3,
        shift_pressed: bool,
        ctrl_pressed: bool,
        game_logic: &Arc<std::sync::Mutex<GameLogic>>,
    ) {
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        // Find object at world position
        let clicked_object = self.find_object_at_position(world_pos, &logic);

        if let Some(object_id) = clicked_object {
            // Check if object belongs to local player
            if let Some(obj) = logic.find_object(object_id) {
                let player_team = self.local_player_team(&logic);
                if obj.team == player_team && obj.is_selectable() {
                    // Select the object
                    if shift_pressed {
                        // Add to selection
                        let mut current_selection = logic
                            .get_player(self.local_player_id)
                            .map(|p| p.selected_objects.clone())
                            .unwrap_or_default();

                        if !current_selection.contains(&object_id) {
                            current_selection.push(object_id);
                            logic.select_objects(self.local_player_id, current_selection);
                            println!("Added object {} to selection", object_id);
                        }
                    } else if ctrl_pressed {
                        // Ctrl+click toggles selection state
                        let mut current_selection = logic
                            .get_player(self.local_player_id)
                            .map(|p| p.selected_objects.clone())
                            .unwrap_or_default();
                        if let Some(index) =
                            current_selection.iter().position(|&id| id == object_id)
                        {
                            current_selection.swap_remove(index);
                            println!("Removed object {} from selection", object_id);
                        } else {
                            current_selection.push(object_id);
                            println!("Added object {} to selection", object_id);
                        }
                        logic.select_objects(self.local_player_id, current_selection);
                    } else {
                        // Replace selection
                        logic.select_objects(self.local_player_id, vec![object_id]);
                        println!("Selected object {} at {:?}", object_id, world_pos);
                    }
                }
            }
        } else {
            // Clicked on empty space - clear selection unless shift is held
            if !shift_pressed && !ctrl_pressed {
                logic.select_objects(self.local_player_id, vec![]);
                println!("Cleared selection");
            }
        }
    }

    /// Handle right mouse click for movement/attack commands
    async fn handle_right_click(
        &mut self,
        world_pos: Vec3,
        game_logic: &Arc<std::sync::Mutex<GameLogic>>,
    ) {
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        // Get currently selected units
        let selected_objects = if let Some(player) = logic.get_player(self.local_player_id) {
            player.selected_objects.clone()
        } else {
            return;
        };

        if selected_objects.is_empty() {
            println!("No units selected for command");
            return;
        }

        // Check if clicking on an enemy unit (attack command)
        let target_object = self.find_object_at_position(world_pos, &logic);

        if let Some(target_id) = target_object {
            if let Some(target) = logic.find_object(target_id) {
                let player_team = logic
                    .get_player(self.local_player_id)
                    .map(|player| player.team)
                    .unwrap_or(Team::Neutral);
                // Check if target is enemy
                if target.team != player_team && target.is_attackable() {
                    // Issue attack command
                    logic.command_attack(self.local_player_id, target_id);
                    println!(
                        "Commanded {} units to attack target {}",
                        selected_objects.len(),
                        target_id
                    );
                    return;
                }
            }
        }

        // Otherwise, issue move command
        logic.command_move(self.local_player_id, world_pos);
        println!(
            "Commanded {} units to move to {:?}",
            selected_objects.len(),
            world_pos
        );
    }

    /// Handle box selection
    async fn handle_box_selection(
        &mut self,
        start_screen: Vec2,
        end_screen: Vec2,
        start_world: Vec3,
        end_world: Vec3,
        shift_pressed: bool,
        game_logic: &Arc<std::sync::Mutex<GameLogic>>,
    ) {
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        // Create selection rectangle
        let min_x = start_world.x.min(end_world.x);
        let max_x = start_world.x.max(end_world.x);
        let min_z = start_world.z.min(end_world.z);
        let max_z = start_world.z.max(end_world.z);

        // Find all selectable objects within the rectangle
        let mut selected_objects = Vec::new();
        let player_team = self.local_player_team(&logic);

        for (object_id, object) in logic.get_objects().iter() {
            if object.team == player_team && object.is_selectable() {
                // Local player units only
                let pos = object.get_position();
                if pos.x >= min_x && pos.x <= max_x && pos.z >= min_z && pos.z <= max_z {
                    selected_objects.push(*object_id);
                }
            }
        }

        // Apply selection
        if !selected_objects.is_empty() {
            if shift_pressed {
                // Add to existing selection
                let mut current_selection = logic
                    .get_player(self.local_player_id)
                    .map(|p| p.selected_objects.clone())
                    .unwrap_or_default();

                for obj_id in &selected_objects {
                    if !current_selection.contains(obj_id) {
                        current_selection.push(*obj_id);
                    }
                }
                logic.select_objects(self.local_player_id, current_selection);
            } else {
                // Replace selection
                logic.select_objects(self.local_player_id, selected_objects.clone());
            }

            println!("Box selected {} units", selected_objects.len());
        }
    }

    /// Select all friendly units matching the clicked unit's template (double-click behavior).
    async fn select_similar_units(
        &mut self,
        world_pos: Vec3,
        game_logic: &Arc<std::sync::Mutex<GameLogic>>,
    ) {
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        let Some(clicked_object_id) = self.find_object_at_position(world_pos, &logic) else {
            return;
        };
        let Some(clicked_obj) = logic.find_object(clicked_object_id) else {
            return;
        };
        let player_team = self.local_player_team(&logic);
        if clicked_obj.team != player_team || !clicked_obj.is_selectable() {
            return;
        }

        let template = clicked_obj.template_name.clone();
        let mut matches = Vec::new();
        for (object_id, object) in logic.get_objects().iter() {
            if object.team == player_team
                && object.is_selectable()
                && object.template_name == template
            {
                matches.push(*object_id);
            }
        }

        if matches.is_empty() {
            return;
        }

        logic.select_objects(self.local_player_id, matches.clone());
        println!("Selected {} similar units ({})", matches.len(), template);
    }

    /// Select all player units
    async fn select_all_units(&self, game_logic: &Arc<std::sync::Mutex<GameLogic>>) {
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        // Find all selectable units belonging to the player
        let mut all_units = Vec::new();
        let player_team = self.local_player_team(&logic);

        for (object_id, object) in logic.get_objects().iter() {
            if object.team == player_team && object.is_selectable() {
                all_units.push(*object_id);
            }
        }

        logic.select_objects(self.local_player_id, all_units.clone());
        println!("Selected all {} units", all_units.len());
    }

    /// Delete selected units
    async fn delete_selected_units(&self, game_logic: &Arc<std::sync::Mutex<GameLogic>>) {
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        let selected_objects = if let Some(player) = logic.get_player(self.local_player_id) {
            player.selected_objects.clone()
        } else {
            return;
        };

        if selected_objects.is_empty() {
            println!("No units selected to delete");
            return;
        }

        // Destroy selected objects
        for &object_id in &selected_objects {
            logic.destroy_object(object_id);
        }

        // Clear selection
        logic.select_objects(self.local_player_id, vec![]);
        println!("Destroyed {} selected units", selected_objects.len());
    }

    /// Cycle through units
    async fn cycle_units(&self, game_logic: &Arc<std::sync::Mutex<GameLogic>>) {
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        // Get all selectable units
        let player_team = self.local_player_team(&logic);
        let mut all_units: Vec<ObjectId> = logic
            .get_objects()
            .iter()
            .filter(|(_, obj)| obj.team == player_team && obj.is_selectable())
            .map(|(&id, _)| id)
            .collect();

        if all_units.is_empty() {
            println!("No units to cycle through");
            return;
        }

        all_units.sort(); // Ensure consistent order

        // Find currently selected unit
        let current_selection = logic
            .get_player(self.local_player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();

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

        logic.select_objects(self.local_player_id, vec![next_unit]);
        println!("Cycled to unit {}", next_unit);
    }

    /// Toggle game pause
    async fn toggle_pause(&self, game_logic: &Arc<std::sync::Mutex<GameLogic>>) {
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());
        let is_paused = logic.is_paused();
        logic.set_paused(!is_paused);

        if !is_paused {
            println!("Game paused");
        } else {
            println!("Game resumed");
        }
    }

    /// Assign selected units to a control group
    async fn assign_control_group(
        &mut self,
        group_num: u8,
        game_logic: &Arc<std::sync::Mutex<GameLogic>>,
    ) {
        let logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        let selected_objects = if let Some(player) = logic.get_player(self.local_player_id) {
            player.selected_objects.clone()
        } else {
            return;
        };

        if selected_objects.is_empty() {
            println!("No units selected to assign to control group {}", group_num);
            return;
        }

        self.control_groups
            .insert(group_num, selected_objects.clone());
        println!(
            "Assigned {} units to control group {}",
            selected_objects.len(),
            group_num
        );
    }

    /// Select units in a control group
    async fn select_control_group(
        &mut self,
        group_num: u8,
        game_logic: &Arc<std::sync::Mutex<GameLogic>>,
    ) {
        let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());

        let Some(group) = self.control_groups.get(&group_num) else {
            println!("Control group {} is empty", group_num);
            return;
        };

        // Filter out dead / invalid objects.
        let mut selection = Vec::new();
        for &object_id in group {
            if let Some(obj) = logic.find_object(object_id) {
                let player_team = self.local_player_team(&logic);
                if obj.team == player_team && obj.is_selectable() {
                    selection.push(object_id);
                }
            }
        }

        if selection.is_empty() {
            println!("Control group {} has no valid units", group_num);
            return;
        }

        logic.select_objects(self.local_player_id, selection.clone());
        println!(
            "Selected control group {} ({} units)",
            group_num,
            selection.len()
        );
    }

    /// Toggle debug mode
    fn toggle_debug_mode(&mut self) {
        self.debug_mode = !self.debug_mode;
        println!("Debug mode: {}", if self.debug_mode { "ON" } else { "OFF" });
    }

    /// Toggle background music
    fn toggle_music(&mut self, game_logic: &Arc<std::sync::Mutex<GameLogic>>) {
        self.music_enabled = !self.music_enabled;
        println!(
            "Background music: {}",
            if self.music_enabled { "ON" } else { "OFF" }
        );
        if let Ok(mut logic) = game_logic.try_lock() {
            let event = if self.music_enabled {
                crate::game_logic::AudioEventRequest::new("MusicEnable")
                    .with_priority(255)
                    .looping()
            } else {
                crate::game_logic::AudioEventRequest::new("MusicDisable").with_priority(255)
            };
            logic.queue_audio_event(event);
        }
    }

    /// Find object at world position (simple distance-based selection)
    fn find_object_at_position(&self, world_pos: Vec3, game_logic: &GameLogic) -> Option<ObjectId> {
        const SELECTION_RADIUS: f32 = 5.0; // Units within this radius can be selected

        // Prefer presentation poses when a dual-tick snapshot is installed.
        if let Some(frame) = self.presentation_frame.as_ref() {
            let mut closest_object = None;
            let mut closest_distance = SELECTION_RADIUS;
            for o in &frame.objects {
                if o.destroyed {
                    continue;
                }
                let distance = (Vec2::new(o.position.x, o.position.z)
                    - Vec2::new(world_pos.x, world_pos.z))
                .length();
                let radius = o.selection_radius.max(SELECTION_RADIUS);
                if distance < closest_distance.min(radius) {
                    closest_distance = distance;
                    closest_object = Some(o.id);
                }
            }
            if closest_object.is_some() {
                return closest_object;
            }
        }

        // Boot residual: live GameLogic dual-read when presentation is absent/misses.
        let mut closest_object = None;
        let mut closest_distance = SELECTION_RADIUS;

        for (object_id, object) in game_logic.get_objects().iter() {
            let obj_pos = object.get_position();
            let distance =
                (Vec2::new(obj_pos.x, obj_pos.z) - Vec2::new(world_pos.x, world_pos.z)).length();

            if distance < closest_distance {
                closest_distance = distance;
                closest_object = Some(*object_id);
            }
        }

        closest_object
    }

    /// Update window size for coordinate conversion
    pub fn set_window_size(&mut self, width: f32, height: f32) {
        self.window_size = (width, height);
    }

    /// Get debug mode status
    pub fn is_debug_mode(&self) -> bool {
        self.debug_mode
    }

    /// Get music enabled status
    pub fn is_music_enabled(&self) -> bool {
        self.music_enabled
    }

    // Static helper methods for internal processing
    fn handle_left_click_internal(
        world_pos: Vec3,
        input: &mut RtsInputSystem,
        game_logic: &Arc<std::sync::Mutex<GameLogic>>,
    ) {
        println!("Left click at world position: {:?}", world_pos);
        // Implementation would handle unit selection logic
    }

    fn handle_right_click_internal(world_pos: Vec3, game_logic: &Arc<std::sync::Mutex<GameLogic>>) {
        println!("Right click at world position: {:?}", world_pos);
        // Implementation would handle unit movement/attack commands
    }

    fn handle_box_selection_internal(
        start_screen: Vec2,
        end_screen: Vec2,
        input: &mut RtsInputSystem,
        game_logic: &Arc<std::sync::Mutex<GameLogic>>,
        window_size: (f32, f32),
    ) {
        println!("Box selection from {:?} to {:?}", start_screen, end_screen);
        // Implementation would handle box selection of units
    }

    fn select_all_units_internal(game_logic: &Arc<std::sync::Mutex<GameLogic>>) {
        println!("Select all units command");
        // Implementation would select all player units
    }

    fn delete_selected_units_internal(game_logic: &Arc<std::sync::Mutex<GameLogic>>) {
        println!("Delete selected units command");
        // Implementation would destroy selected units
    }

    fn toggle_pause_internal(game_logic: &Arc<std::sync::Mutex<GameLogic>>) {
        if let Ok(mut logic) = game_logic.try_lock() {
            let is_paused = logic.is_paused();
            logic.set_paused(!is_paused);
            println!("Game pause toggled: {}", !is_paused);
        }
    }

    fn cycle_units_internal(game_logic: &Arc<std::sync::Mutex<GameLogic>>) {
        println!("Cycle units command");
        // Implementation would cycle through available units
    }
}

/// Helper functions for coordinate conversion and object detection
impl InputProcessor {
    /// Convert normalized device coordinates to world space
    pub fn ndc_to_world(&self, ndc: Vec2, input: &RtsInputSystem) -> Vec3 {
        let camera = input.get_camera();
        Vec3::new(
            ndc.x * camera.zoom + camera.position.x,
            0.0,
            ndc.y * camera.zoom + camera.position.z,
        )
    }

    /// Convert world coordinates to screen space
    pub fn world_to_screen(&self, world_pos: Vec3, input: &RtsInputSystem) -> Vec2 {
        let camera = input.get_camera();
        let ndc_x = (world_pos.x - camera.position.x) / camera.zoom;
        let ndc_y = (world_pos.z - camera.position.z) / camera.zoom;

        Vec2::new(
            (ndc_x + 1.0) * self.window_size.0 / 2.0,
            (1.0 - ndc_y) * self.window_size.1 / 2.0,
        )
    }

    /// Check if a point is within a rectangle
    pub fn point_in_rect(&self, point: Vec2, rect_min: Vec2, rect_max: Vec2) -> bool {
        point.x >= rect_min.x
            && point.x <= rect_max.x
            && point.y >= rect_min.y
            && point.y <= rect_max.y
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use crate::input_system::RtsInputSystem;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use std::sync::{Arc, Mutex};

    #[test]
    fn input_processor_pick_prefers_presentation_pose() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("InputIntegPresPick");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("IipUnit") {
            let mut tmpl = ThingTemplate::new("IipUnit");
            tmpl.set_health(100.0);
            tmpl.add_kind_of(KindOf::Selectable);
            logic.templates.insert("IipUnit".into(), tmpl);
        }
        let id = logic
            .create_object("IipUnit", Team::USA, glam::Vec3::new(12.0, 0.0, 18.0))
            .expect("id");
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            obj.position = glam::Vec3::new(8888.0, 0.0, 8888.0);
        }
        let input = Arc::new(Mutex::new(RtsInputSystem::new()));
        let mut proc = InputProcessor::new(input, 0, (1024.0, 768.0));
        proc.set_presentation_frame(Some(frame));
        let picked = proc.find_object_at_position(glam::Vec3::new(12.0, 0.0, 18.0), &logic);
        assert_eq!(picked, Some(id));
        let src = include_str!("input_integration.rs");
        assert!(
            src.contains("Prefer presentation poses when a dual-tick snapshot is installed"),
            "input integration pick must prefer presentation residual"
        );
    }
}
