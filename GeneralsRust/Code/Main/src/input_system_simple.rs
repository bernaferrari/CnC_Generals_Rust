use crate::game_logic::{GameLogic, KindOf, ObjectId, Team};
use crate::input_system::RtsInputSystem;
use crate::presentation_frame::PresentationFrame;
use anyhow::Result;
use glam::{Vec2, Vec3};
use std::collections::HashMap;
use tokio::sync::mpsc;
use winit::keyboard::{Key, NamedKey};

/// High-performance async input processor that connects input to game logic
///
/// This processor uses Tokio async operations to ensure the game loop never blocks
/// during input processing, providing maximum performance for real-time gameplay.
pub struct SimpleInputProcessor {
    local_player_id: u32,
    window_size: (f32, f32),
    last_frame: u32,
    // Async channels for input event processing
    input_sender: mpsc::UnboundedSender<InputEvent>,
    input_receiver: mpsc::UnboundedReceiver<InputEvent>,
    control_groups: HashMap<u8, Vec<ObjectId>>, // 0-9 control groups
    last_camera_position: Vec3,
    last_camera_zoom: f32,
    /// Dual-tick presentation snapshot for world pick residual (optional).
    presentation_frame: Option<PresentationFrame>,
}

/// Input events processed asynchronously
#[derive(Debug, Clone)]
pub enum InputEvent {
    SelectAll,
    Delete,
    TogglePause,
    CycleUnits,
    ControlGroup { number: u8, assign: bool },
    LeftClick { world_pos: Vec3, shift_held: bool },
    RightClick { world_pos: Vec3 },
}

impl SimpleInputProcessor {
    /// Create a new async input processor with event channels
    pub fn new(local_player_id: u32, window_size: (f32, f32)) -> Self {
        let (input_sender, input_receiver) = mpsc::unbounded_channel();

        Self {
            local_player_id,
            window_size,
            last_frame: 0,
            input_sender,
            input_receiver,
            control_groups: HashMap::new(),
            last_camera_position: Vec3::ZERO,
            last_camera_zoom: 50.0,
            presentation_frame: None,
        }
    }

    /// Install dual-tick presentation snapshot for pick residual.
    pub fn set_presentation_frame(&mut self, frame: Option<PresentationFrame>) {
        self.presentation_frame = frame;
    }

    fn presentation_local_team(&self, game_logic: &GameLogic) -> Team {
        if let Some(frame) = self.presentation_frame.as_ref() {
            return frame.local_team;
        }
        self.local_player_team(game_logic)
    }

    fn presentation_is_selectable(o: &crate::presentation_frame::RenderableObject) -> bool {
        !o.destroyed
            && PresentationFrame::object_has_kind(o, KindOf::Selectable)
            && o.contained_by.is_none()
    }

    fn presentation_is_attackable(o: &crate::presentation_frame::RenderableObject) -> bool {
        // Mirror C++ WeaponSet victim legality before turning an RMB click into
        // an attack command.  Authority still checks this independently.
        !o.destroyed
            && !o.sold
            && !o.masked
            && !o.unattackable
            && PresentationFrame::object_has_kind(o, KindOf::Attackable)
    }

    pub fn presentation_frame(&self) -> Option<&PresentationFrame> {
        self.presentation_frame.as_ref()
    }

    fn local_player_team(&self, game_logic: &GameLogic) -> Team {
        game_logic
            .get_player(self.local_player_id)
            .map(|player| player.team)
            .unwrap_or(Team::Neutral)
    }

    /// Process input commands asynchronously - never blocks the game loop
    pub fn process_input(
        &mut self,
        input_system: &mut RtsInputSystem,
        game_logic: &mut GameLogic,
    ) -> Result<()> {
        // Get current frame from GameLogic
        let current_frame = game_logic.get_frame();

        // Skip if we already processed this frame
        if current_frame == self.last_frame {
            return Ok(());
        }
        self.last_frame = current_frame;

        // Collect input state
        let input = &*input_system;
        let ctrl_pressed = input.is_ctrl_pressed();
        let select_all = ctrl_pressed && input.is_key_just_pressed(&Key::Character("a".into()));
        let delete_pressed = input.is_key_just_pressed(&Key::Named(NamedKey::Delete));
        let space_pressed = input.is_key_just_pressed(&Key::Named(NamedKey::Space));
        let tab_pressed = input.is_key_just_pressed(&Key::Named(NamedKey::Tab));
        let camera = input.get_camera();
        self.last_camera_position = camera.position;
        self.last_camera_zoom = camera.zoom;

        // Check number keys
        let mut control_group_action = None;
        for i in 0..=9 {
            if input.is_key_just_pressed(&Key::Character(i.to_string().into())) {
                if ctrl_pressed {
                    control_group_action = Some((i as u8, true)); // assign
                    break;
                } else {
                    control_group_action = Some((i as u8, false)); // select
                    break;
                }
            }
        }

        // Queue events for processing this frame
        if select_all {
            let _ = self.input_sender.send(InputEvent::SelectAll);
        }

        if delete_pressed {
            let _ = self.input_sender.send(InputEvent::Delete);
        }

        if space_pressed {
            let _ = self.input_sender.send(InputEvent::TogglePause);
        }

        if tab_pressed {
            let _ = self.input_sender.send(InputEvent::CycleUnits);
        }

        if let Some((group_num, is_assign)) = control_group_action {
            let _ = self.input_sender.send(InputEvent::ControlGroup {
                number: group_num,
                assign: is_assign,
            });
        }

        // Process queued events asynchronously
        self.process_queued_events(game_logic)?;

        Ok(())
    }

    /// Process all queued input events asynchronously
    fn process_queued_events(&mut self, game_logic: &mut GameLogic) -> Result<()> {
        while let Ok(event) = self.input_receiver.try_recv() {
            match event {
                InputEvent::SelectAll => {
                    self.select_all_units_async(game_logic)?;
                }
                InputEvent::Delete => {
                    self.delete_selected_units_async(game_logic)?;
                }
                InputEvent::TogglePause => {
                    self.toggle_pause_async(game_logic)?;
                }
                InputEvent::CycleUnits => {
                    self.cycle_units_async(game_logic)?;
                }
                InputEvent::ControlGroup { number, assign } => {
                    if assign {
                        self.assign_control_group_async(number, game_logic)?;
                    } else {
                        self.select_control_group_async(number, game_logic)?;
                    }
                }
                InputEvent::LeftClick {
                    world_pos,
                    shift_held,
                } => {
                    self.handle_left_click_async(world_pos, shift_held, game_logic)?;
                }
                InputEvent::RightClick { world_pos } => {
                    self.handle_right_click_async(world_pos, game_logic)?;
                }
            }
        }

        Ok(())
    }

    /// Select all player units asynchronously

    fn select_all_units_async(&mut self, game_logic: &mut GameLogic) -> Result<()> {
        let logic = game_logic;

        // Wave 953: presentation-only select-all (no live get_objects dual-read).
        let mut all_units = Vec::new();
        if let Some(frame) = self.presentation_frame.as_ref() {
            for o in &frame.objects {
                if frame.is_owned_by_local(o) && Self::presentation_is_selectable(o) {
                    all_units.push(o.id);
                }
            }
        }
        logic.select_objects(self.local_player_id, all_units.clone());
        println!("Selected all {} units", all_units.len());
        Ok(())
    }

    /// Delete selected units asynchronously
    fn delete_selected_units_async(&mut self, game_logic: &mut GameLogic) -> Result<()> {
        let logic = game_logic;

        let selected_objects = if let Some(player) = logic.get_player(self.local_player_id) {
            player.selected_objects.clone()
        } else {
            return Ok(());
        };

        if selected_objects.is_empty() {
            println!("No units selected to delete");
            return Ok(());
        }

        // Destroy selected objects
        for &object_id in &selected_objects {
            logic.destroy_object(object_id);
        }

        // Clear selection
        logic.select_objects(self.local_player_id, vec![]);
        println!("Destroyed {} selected units", selected_objects.len());

        Ok(())
    }

    /// Toggle game pause asynchronously
    fn toggle_pause_async(&mut self, game_logic: &mut GameLogic) -> Result<()> {
        let logic = game_logic;
        let is_paused = logic.is_paused();
        logic.set_paused(!is_paused);

        if !is_paused {
            println!("Game paused");
        } else {
            println!("Game resumed");
        }

        Ok(())
    }

    /// Cycle through units asynchronously

    fn cycle_units_async(&mut self, game_logic: &mut GameLogic) -> Result<()> {
        let logic = game_logic;

        // Wave 953: presentation-only unit cycle (no live get_objects dual-read).
        let mut all_units: Vec<ObjectId> = if let Some(frame) = self.presentation_frame.as_ref() {
            frame
                .objects
                .iter()
                .filter(|o| frame.is_owned_by_local(o) && Self::presentation_is_selectable(o))
                .map(|o| o.id)
                .collect()
        } else {
            Vec::new()
        };

        if all_units.is_empty() {
            println!("No units to cycle through");
            return Ok(());
        }

        all_units.sort();

        let current_selection = logic
            .get_player(self.local_player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();

        let next_unit = if let Some(&current_id) = current_selection.first() {
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
        println!("Cycled to unit {:?}", next_unit);
        Ok(())
    }

    /// Assign selected units to a control group asynchronously
    fn assign_control_group_async(
        &mut self,
        group_num: u8,
        game_logic: &mut GameLogic,
    ) -> Result<()> {
        let logic = game_logic;

        let selected_objects = if let Some(player) = logic.get_player(self.local_player_id) {
            player.selected_objects.clone()
        } else {
            return Ok(());
        };

        if selected_objects.is_empty() {
            println!("No units selected to assign to control group {}", group_num);
            return Ok(());
        }

        self.control_groups
            .insert(group_num, selected_objects.clone());
        println!(
            "Assigned {} units to control group {}",
            selected_objects.len(),
            group_num
        );

        Ok(())
    }

    /// Select units in a control group asynchronously

    fn select_control_group_async(
        &mut self,
        group_num: u8,
        game_logic: &mut GameLogic,
    ) -> Result<()> {
        let Some(stored) = self.control_groups.get(&group_num).cloned() else {
            println!("Control group {} is empty", group_num);
            return Ok(());
        };

        let logic = game_logic;
        // Wave 953: control-group filter presentation-only (fail-closed without freeze).
        // C++ SELECT_TEAM: getLiveObjects / isSelectable — not CanSelectDrawable.
        let mut selection = Vec::new();
        if let Some(frame) = self.presentation_frame.as_ref() {
            selection = frame.filter_live_squad_ids(&stored, true);
        }
        logic.select_objects(self.local_player_id, selection.clone());
        println!(
            "Selected control group {}: {} units",
            group_num,
            selection.len()
        );
        Ok(())
    }

    /// Handle left click for unit selection asynchronously
    pub fn handle_left_click(&self, world_pos: Vec3, input_system: &RtsInputSystem) -> Result<()> {
        let shift_pressed = input_system.is_shift_pressed();

        // Queue the click event for async processing
        let _ = self.input_sender.send(InputEvent::LeftClick {
            world_pos,
            shift_held: shift_pressed,
        });

        Ok(())
    }

    /// Handle left click processing asynchronously

    fn handle_left_click_async(
        &mut self,
        world_pos: Vec3,
        shift_held: bool,
        game_logic: &mut GameLogic,
    ) -> Result<()> {
        let logic = game_logic;

        // Wave 953: pick + friendly classify presentation-only.
        let clicked_object = self.find_object_at_position(world_pos, &logic);

        if let Some(object_id) = clicked_object {
            let friendly_selectable =
                self.presentation_frame
                    .as_ref()
                    .and_then(|frame| {
                        frame.objects.iter().find(|o| o.id == object_id).map(|o| {
                            frame.is_owned_by_local(o) && Self::presentation_is_selectable(o)
                        })
                    })
                    .unwrap_or(false);
            if friendly_selectable && logic.host_object(object_id).is_some() {
                if shift_held {
                    let mut current_selection = logic
                        .get_player(self.local_player_id)
                        .map(|p| p.selected_objects.clone())
                        .unwrap_or_default();
                    if !current_selection.contains(&object_id) {
                        current_selection.push(object_id);
                    }
                    logic.select_objects(self.local_player_id, current_selection);
                } else {
                    logic.select_objects(self.local_player_id, vec![object_id]);
                }
            }
        } else if !shift_held {
            // Click empty ground clears selection residual.
            logic.select_objects(self.local_player_id, Vec::new());
        }

        Ok(())
    }

    /// Handle right click for movement/attack commands asynchronously
    pub fn handle_right_click(&self, world_pos: Vec3) -> Result<()> {
        // Queue the click event for async processing
        let _ = self.input_sender.send(InputEvent::RightClick { world_pos });

        Ok(())
    }

    /// Handle right click processing asynchronously

    fn handle_right_click_async(
        &mut self,
        world_pos: Vec3,
        game_logic: &mut GameLogic,
    ) -> Result<()> {
        let logic = game_logic;

        // Get currently selected units
        let selected_objects = if let Some(player) = logic.get_player(self.local_player_id) {
            player.selected_objects.clone()
        } else {
            return Ok(());
        };

        if selected_objects.is_empty() {
            println!("No units selected for command");
            return Ok(());
        }

        // Wave 953: attack target classify presentation-only.
        let target_object = self.find_object_at_position(world_pos, &logic);
        if let Some(target_id) = target_object {
            let attackable_enemy =
                self.presentation_frame
                    .as_ref()
                    .and_then(|frame| {
                        frame.objects.iter().find(|o| o.id == target_id).map(|o| {
                            frame.is_enemy_of_local(o) && Self::presentation_is_attackable(o)
                        })
                    })
                    .unwrap_or(false);
            if attackable_enemy && logic.host_object(target_id).is_some() {
                logic.command_attack(self.local_player_id, target_id);
                println!(
                    "Commanded {} units to attack target {}",
                    selected_objects.len(),
                    target_id
                );
                return Ok(());
            }
        }

        // Otherwise, issue move command
        logic.command_move(self.local_player_id, world_pos);
        println!(
            "Commanded {} units to move to {:?}",
            selected_objects.len(),
            world_pos
        );

        Ok(())
    }

    /// Find object at world position (optimized for async processing)
    ///
    /// This function is kept synchronous since it only reads data and performs calculations.
    /// It's called while holding the GameLogic lock, so it should be fast.
    fn find_object_at_position(
        &self,
        world_pos: Vec3,
        _game_logic: &GameLogic,
    ) -> Option<ObjectId> {
        const SELECTION_RADIUS: f32 = 5.0; // Units within this radius can be selected

        // Presentation-only: no live GameLogic dual-read residual.
        // Wave 1096: sold/masked + non-local FOW Clear-only (matches UnitControl pick).
        let frame = self.presentation_frame.as_ref()?;
        let mut closest_object = None;
        let mut closest_distance = SELECTION_RADIUS;
        for o in &frame.objects {
            if o.destroyed || o.sold || o.masked {
                continue;
            }
            let is_local = frame.is_owned_by_local(o);
            if !is_local && o.fow_visibility.visibility_alpha < 0.95 {
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
        closest_object
    }

    /// Convert screen coordinates to world coordinates
    pub fn screen_to_world(&self, screen_pos: Vec2) -> Vec3 {
        // Orthographic screen->world mapping based on last known RTS camera state.
        let ndc_x = (screen_pos.x / self.window_size.0) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / self.window_size.1) * 2.0;

        Vec3::new(
            ndc_x * self.last_camera_zoom + self.last_camera_position.x,
            0.0,
            ndc_y * self.last_camera_zoom + self.last_camera_position.z,
        )
    }

    /// Update window size for coordinate conversion
    pub fn set_window_size(&mut self, width: f32, height: f32) {
        self.window_size = (width, height);
    }

    /// Get input event sender for external systems to queue events
    ///
    /// This allows other parts of the game to queue input events for async processing
    /// without blocking the game loop.
    pub fn get_input_sender(&self) -> mpsc::UnboundedSender<InputEvent> {
        self.input_sender.clone()
    }

    /// Process a single input event asynchronously (used for external event queuing)
    pub fn process_single_event(
        &mut self,
        event: InputEvent,
        game_logic: &mut GameLogic,
    ) -> Result<()> {
        match event {
            InputEvent::SelectAll => {
                self.select_all_units_async(game_logic)?;
            }
            InputEvent::Delete => {
                self.delete_selected_units_async(game_logic)?;
            }
            InputEvent::TogglePause => {
                self.toggle_pause_async(game_logic)?;
            }
            InputEvent::CycleUnits => {
                self.cycle_units_async(game_logic)?;
            }
            InputEvent::ControlGroup { number, assign } => {
                if assign {
                    self.assign_control_group_async(number, game_logic)?;
                } else {
                    self.select_control_group_async(number, game_logic)?;
                }
            }
            InputEvent::LeftClick {
                world_pos,
                shift_held,
            } => {
                self.handle_left_click_async(world_pos, shift_held, game_logic)?;
            }
            InputEvent::RightClick { world_pos } => {
                self.handle_right_click_async(world_pos, game_logic)?;
            }
        }

        Ok(())
    }

    /// Flush all pending input events (useful for frame cleanup or shutdown)
    pub fn flush_events(&mut self, game_logic: &mut GameLogic) -> Result<usize> {
        let mut count = 0;
        while let Ok(event) = self.input_receiver.try_recv() {
            count += 1;
            self.process_single_event(event, game_logic)?;
        }

        Ok(count)
    }
}

/*
** Single-threaded game-thread input path: events are queued on an unbounded
** channel and drained on the same frame that owns GameLogic, so no locks are
** needed anywhere in this module.
*/

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    #[test]
    fn find_object_at_position_prefers_presentation_pose() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SimpleInputPresPick");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SipUnit") {
            let mut tmpl = ThingTemplate::new("SipUnit");
            tmpl.set_health(100.0);
            tmpl.add_kind_of(KindOf::Selectable);
            logic.templates.insert("SipUnit".into(), tmpl);
        }
        let id = logic
            .create_object("SipUnit", Team::USA, glam::Vec3::new(10.0, 0.0, 20.0))
            .expect("id");
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        // Poison live pose — presentation must win.
        if let Some(obj) = logic.host_object_mut(id) {
            obj.set_position(glam::Vec3::new(9999.0, 0.0, 9999.0));
        }
        let mut proc = SimpleInputProcessor::new(0, (1024.0, 768.0));
        assert!(
            proc.find_object_at_position(glam::Vec3::new(10.0, 0.0, 20.0), &logic)
                .is_none(),
            "without presentation frame pick must not dual-read live GameLogic"
        );
        proc.set_presentation_frame(Some(frame));
        let picked = proc.find_object_at_position(glam::Vec3::new(10.0, 0.0, 20.0), &logic);
        assert_eq!(
            picked,
            Some(id),
            "must pick presentation pose, not live dual-read"
        );
        let src = include_str!("input_system_simple.rs");
        let fstart = src.find("fn find_object_at_position").expect("fn");
        let fbody = &src[fstart..src.len().min(fstart + 900)];
        assert!(
            fbody.contains("Presentation-only") && !fbody.contains("game_logic.get_objects()"),
            "find_object_at_position must be presentation-only"
        );
    }
}
