use crate::game_logic::{GameLogic, KindOf, ObjectId};
use crate::input_system::RtsInputSystem;
use crate::presentation_frame::PresentationFrame;
use anyhow::Result;
use glam::{Vec2, Vec3};
use std::collections::{HashMap, VecDeque};
use winit::keyboard::{Key, NamedKey};

/// Deterministic command-intake input processor connecting input to game logic.
///
/// Input handlers enqueue events into an owned FIFO queue; the frame loop
/// drains the queue in order with `&mut GameLogic` (single owner, no shared
/// handle — matches the OWNERSHIP_AND_AUTHORITY "deterministic command
/// intake" target). The `async` fns below contain no await points: their
/// futures are ready on first poll (pinned Wave 953 honesty markers require
/// the `async fn ..._async` shapes).
pub struct SimpleInputProcessor {
    local_player_id: u32,
    window_size: (f32, f32),
    last_frame: u32,
    // Owned FIFO intake queue: producers append, the frame loop drains.
    input_queue: VecDeque<InputEvent>,
    control_groups: HashMap<u8, Vec<ObjectId>>, // 0-9 control groups
    last_camera_position: Vec3,
    last_camera_zoom: f32,
    /// Dual-tick presentation snapshot for world pick residual (optional).
    presentation_frame: Option<PresentationFrame>,
}

/// Input events queued for same-frame application by the frame loop
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
    /// Create a new input processor with an owned event queue
    pub fn new(local_player_id: u32, window_size: (f32, f32)) -> Self {
        Self {
            local_player_id,
            window_size,
            last_frame: 0,
            input_queue: VecDeque::new(),
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


    /// Process input commands for the current frame.
    ///
    /// Frame-loop entry point: reads input state, enqueues the resulting
    /// commands, then drains the intake queue in order against the
    /// caller-owned `game_logic`.
    pub async fn process_input(
        &mut self,
        input_system: &RtsInputSystem,
        game_logic: &mut GameLogic,
    ) -> Result<()> {
        let current_frame = game_logic.get_frame();

        // Skip if we already processed this frame
        if current_frame == self.last_frame {
            return Ok(());
        }
        self.last_frame = current_frame;

        let ctrl_pressed = input_system.is_ctrl_pressed();
        let select_all =
            ctrl_pressed && input_system.is_key_just_pressed(&Key::Character("a".into()));
        let delete_pressed = input_system.is_key_just_pressed(&Key::Named(NamedKey::Delete));
        let space_pressed = input_system.is_key_just_pressed(&Key::Named(NamedKey::Space));
        let tab_pressed = input_system.is_key_just_pressed(&Key::Named(NamedKey::Tab));
        let camera = input_system.get_camera();
        self.last_camera_position = camera.position;
        self.last_camera_zoom = camera.zoom;

        // Check number keys
        let mut control_group_action = None;
        for i in 0..=9 {
            if input_system.is_key_just_pressed(&Key::Character(i.to_string().into())) {
                if ctrl_pressed {
                    control_group_action = Some((i as u8, true)); // assign
                    break;
                } else {
                    control_group_action = Some((i as u8, false)); // select
                    break;
                }
            }
        }

        // Queue events for same-frame application (deterministic order)
        if select_all {
            self.input_queue.push_back(InputEvent::SelectAll);
        }

        if delete_pressed {
            self.input_queue.push_back(InputEvent::Delete);
        }

        if space_pressed {
            self.input_queue.push_back(InputEvent::TogglePause);
        }

        if tab_pressed {
            self.input_queue.push_back(InputEvent::CycleUnits);
        }

        if let Some((group_num, is_assign)) = control_group_action {
            self.input_queue.push_back(InputEvent::ControlGroup {
                number: group_num,
                assign: is_assign,
            });
        }

        // Drain the queue in FIFO order (same order the channel preserved)
        self.process_queued_events(game_logic).await?;

        Ok(())
    }

    /// Process all queued input events in FIFO order
    async fn process_queued_events(&mut self, game_logic: &mut GameLogic) -> Result<()> {
        // Drain first, then apply: preserves event order without borrowing
        // the queue across the applier calls.
        let mut events = Vec::new();
        while let Some(event) = self.input_queue.pop_front() {
            events.push(event);
        }

        for event in events {
            self.process_single_event(event, game_logic).await?;
        }

        Ok(())
    }

    /// Select all player units
    async fn select_all_units_async(&self, game_logic: &mut GameLogic) -> Result<()> {
        // Wave 953: presentation-only select-all (no live get_objects dual-read).
        let mut all_units = Vec::new();
        if let Some(frame) = self.presentation_frame.as_ref() {
            for o in &frame.objects {
                if frame.is_owned_by_local(o) && Self::presentation_is_selectable(o) {
                    all_units.push(o.id);
                }
            }
        }
        game_logic.select_objects(self.local_player_id, all_units.clone());
        println!("Selected all {} units", all_units.len());
        Ok(())
    }

    /// Delete selected units
    async fn delete_selected_units_async(&self, game_logic: &mut GameLogic) -> Result<()> {
        let selected_objects = if let Some(player) = game_logic.get_player(self.local_player_id) {
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
            game_logic.destroy_object(object_id);
        }

        // Clear selection
        game_logic.select_objects(self.local_player_id, vec![]);
        println!("Destroyed {} selected units", selected_objects.len());

        Ok(())
    }

    /// Toggle game pause
    async fn toggle_pause_async(&self, game_logic: &mut GameLogic) -> Result<()> {
        let is_paused = game_logic.is_paused();
        game_logic.set_paused(!is_paused);

        if !is_paused {
            println!("Game paused");
        } else {
            println!("Game resumed");
        }

        Ok(())
    }

    /// Cycle through units
    async fn cycle_units_async(&self, game_logic: &mut GameLogic) -> Result<()> {
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

        let current_selection = game_logic
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

        game_logic.select_objects(self.local_player_id, vec![next_unit]);
        println!("Cycled to unit {:?}", next_unit);
        Ok(())
    }

    /// Assign selected units to a control group
    async fn assign_control_group_async(
        &mut self,
        group_num: u8,
        game_logic: &GameLogic,
    ) -> Result<()> {
        let selected_objects = if let Some(player) = game_logic.get_player(self.local_player_id) {
            player.selected_objects.clone()
        } else {
            return Ok(());
        };

        if selected_objects.is_empty() {
            println!("No units selected to assign to control group {}", group_num);
            return Ok(());
        }

        self.control_groups.insert(group_num, selected_objects.clone());
        println!(
            "Assigned {} units to control group {}",
            selected_objects.len(),
            group_num
        );

        Ok(())
    }

    /// Select units in a control group
    async fn select_control_group_async(
        &self,
        group_num: u8,
        game_logic: &mut GameLogic,
    ) -> Result<()> {
        let stored = self.control_groups.get(&group_num).cloned();
        let Some(stored) = stored else {
            println!("Control group {} is empty", group_num);
            return Ok(());
        };

        // Wave 953: control-group filter presentation-only (fail-closed without freeze).
        // C++ SELECT_TEAM: getLiveObjects / isSelectable — not CanSelectDrawable.
        let mut selection = Vec::new();
        if let Some(frame) = self.presentation_frame.as_ref() {
            selection = frame.filter_live_squad_ids(&stored, true);
        }
        game_logic.select_objects(self.local_player_id, selection.clone());
        println!(
            "Selected control group {}: {} units",
            group_num,
            selection.len()
        );
        Ok(())
    }

    /// Handle left click for unit selection (queues an intake event)
    pub async fn handle_left_click(
        &self,
        world_pos: Vec3,
        input_system: &RtsInputSystem,
    ) -> Result<()> {
        let shift_pressed = input_system.is_shift_pressed();

        // Queue the click event for same-frame application
        self.input_queue.push_back(InputEvent::LeftClick {
            world_pos,
            shift_held: shift_pressed,
        });

        Ok(())
    }

    /// Apply a queued left click
    async fn handle_left_click_async(
        &self,
        world_pos: Vec3,
        shift_held: bool,
        game_logic: &mut GameLogic,
    ) -> Result<()> {
        // Wave 953: pick + friendly classify presentation-only.
        let clicked_object = self.find_object_at_position(world_pos, game_logic);

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
            if friendly_selectable && game_logic.host_object(object_id).is_some() {
                if shift_held {
                    let mut current_selection = game_logic
                        .get_player(self.local_player_id)
                        .map(|p| p.selected_objects.clone())
                        .unwrap_or_default();
                    if !current_selection.contains(&object_id) {
                        current_selection.push(object_id);
                    }
                    game_logic.select_objects(self.local_player_id, current_selection);
                } else {
                    game_logic.select_objects(self.local_player_id, vec![object_id]);
                }
            }
        } else if !shift_held {
            // Click empty ground clears selection residual.
            game_logic.select_objects(self.local_player_id, Vec::new());
        }

        Ok(())
    }

    /// Handle right click for movement/attack commands (queues an intake event)
    pub async fn handle_right_click(&self, world_pos: Vec3) -> Result<()> {
        // Queue the click event for same-frame application
        self.input_queue.push_back(InputEvent::RightClick { world_pos });

        Ok(())
    }

    /// Apply a queued right click
    async fn handle_right_click_async(
        &self,
        world_pos: Vec3,
        game_logic: &mut GameLogic,
    ) -> Result<()> {
        // Get currently selected units
        let selected_objects = if let Some(player) = game_logic.get_player(self.local_player_id) {
            player.selected_objects.clone()
        } else {
            return Ok(());
        };

        if selected_objects.is_empty() {
            println!("No units selected for command");
            return Ok(());
        }

        // Wave 953: attack target classify presentation-only.
        let target_object = self.find_object_at_position(world_pos, game_logic);
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
            if attackable_enemy && game_logic.host_object(target_id).is_some() {
                game_logic.command_attack(self.local_player_id, target_id);
                println!(
                    "Commanded {} units to attack target {}",
                    selected_objects.len(),
                    target_id
                );
                return Ok(());
            }
        }

        // Otherwise, issue move command
        game_logic.command_move(self.local_player_id, world_pos);
        println!(
            "Commanded {} units to move to {:?}",
            selected_objects.len(),
            world_pos
        );

        Ok(())
    }

    /// Find object at world position (presentation pick)
    ///
    /// This function is kept synchronous since it only reads data and performs
    /// calculations.
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


    /// Process a single input event (used by the queue drain)
    pub async fn process_single_event(
        &mut self,
        event: InputEvent,
        game_logic: &mut GameLogic,
    ) -> Result<()> {
        match event {
            InputEvent::SelectAll => {
                self.select_all_units_async(game_logic).await?;
            }
            InputEvent::Delete => {
                self.delete_selected_units_async(game_logic).await?;
            }
            InputEvent::TogglePause => {
                self.toggle_pause_async(game_logic).await?;
            }
            InputEvent::CycleUnits => {
                self.cycle_units_async(game_logic).await?;
            }
            InputEvent::ControlGroup { number, assign } => {
                if assign {
                    self.assign_control_group_async(number, game_logic).await?;
                } else {
                    self.select_control_group_async(number, game_logic).await?;
                }
            }
            InputEvent::LeftClick {
                world_pos,
                shift_held,
            } => {
                self.handle_left_click_async(world_pos, shift_held, game_logic)
                    .await?;
            }
            InputEvent::RightClick { world_pos } => {
                self.handle_right_click_async(world_pos, game_logic).await?;
            }
        }

        Ok(())
    }

    /// Flush all pending input events (useful for frame cleanup or shutdown)
    pub async fn flush_events(&mut self, game_logic: &mut GameLogic) -> Result<usize> {
        let mut count = 0;
        // Drain first, then apply: preserves event order without borrowing
        // the queue across the applier calls.
        let mut events = Vec::new();
        while let Some(event) = self.input_queue.pop_front() {
            count += 1;
            events.push(event);
        }

        for event in events {
            self.process_single_event(event, game_logic).await?;
        }

        Ok(count)
    }
}

/*
** DESIGN NOTES
**
** Command-intake input processing (single owner, no shared handle):
**
** - Input handlers append InputEvents to an owned FIFO VecDeque.
** - The frame loop drains the queue in order and applies each event to the
**   caller-owned `&mut GameLogic`, so input event ordering and pause toggling
**   keep their original per-frame timing.
** - The `async` fns contain no await points: their futures are ready on the
**   first poll (pollster::block_on callers are unchanged).
**
** USAGE PATTERN (once per frame):
**
** ```rust
** // In main game loop
** if let Err(e) = input_processor.process_input(&input_system, &mut game_logic).await {
**     eprintln!("Input processing error: {}", e);
** }
** ```
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
            obj.position = glam::Vec3::new(9999.0, 0.0, 9999.0);
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
