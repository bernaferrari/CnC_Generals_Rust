//! # Integrated UI System
//!
//! Complete integration of all in-game UI components with wgpu rendering.
//! This module brings together selection boxes, command panels, minimap,
//! resource display, and building placement into a cohesive system.
//!
//! This is the main entry point for the in-game UI subsystem.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use glam::Vec2;
use thiserror::Error;
use wgpu::{
    CommandEncoderDescriptor, Device, LoadOp, Operations, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, StoreOp, TextureFormat, TextureView,
};

use super::{
    CommandButton, CommandButtonType, CommandPanel, CommandPanelContext, InGameUI,
    PlacementPreview, UIRenderer, UIRendererError,
};
use crate::input::keyboard::KeyboardState;
use crate::input::mouse::MouseState;

/// Integrated UI system errors
#[derive(Error, Debug)]
pub enum IntegratedUIError {
    #[error("Renderer error: {0}")]
    RendererError(#[from] UIRendererError),
    #[error("InGameUI error: {0}")]
    InGameUIError(#[from] super::ingame_ui::InGameUIError),
    #[error("CommandPanel error: {0}")]
    CommandPanelError(#[from] super::command_panel::CommandPanelError),
    #[error("System error: {0}")]
    SystemError(String),
}

type Result<T> = std::result::Result<T, IntegratedUIError>;

/// UI command event from user interaction
#[derive(Debug, Clone)]
pub enum UICommand {
    /// Build a unit or structure
    Build(String),
    /// Execute a unit command
    UnitCommand(String),
    /// Trigger a special power
    SpecialPower(String),
    /// Purchase an upgrade
    Upgrade(String),
    /// Cancel current action
    Cancel,
    /// Set camera position (from minimap click)
    SetCameraPosition(Vec2),
    /// Create selection group
    CreateGroup(usize),
    /// Recall selection group
    RecallGroup(usize),
}

/// Complete integrated UI system
pub struct IntegratedUISystem {
    /// UI renderer (shared)
    renderer: Arc<RwLock<UIRenderer>>,

    /// In-game UI (selection, minimap, resources)
    ingame_ui: InGameUI,

    /// Command panel
    command_panel: CommandPanel,

    /// Screen dimensions
    screen_size: (u32, u32),

    /// Whether system is initialized
    initialized: bool,

    /// UI command queue
    command_queue: Vec<UICommand>,

    /// Current player ID
    player_id: u32,
}

impl IntegratedUISystem {
    /// Create a new integrated UI system
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        format: TextureFormat,
        screen_width: u32,
        screen_height: u32,
    ) -> Result<Self> {
        // Create shared renderer
        let renderer = Arc::new(RwLock::new(UIRenderer::new(device, queue, format)?));
        crate::gui::set_ui_renderer(renderer.clone());

        // Create subsystems
        let ingame_ui = InGameUI::new(renderer.clone(), screen_width as f32, screen_height as f32);

        let command_panel =
            CommandPanel::new(renderer.clone(), screen_width as f32, screen_height as f32);

        Ok(Self {
            renderer,
            ingame_ui,
            command_panel,
            screen_size: (screen_width, screen_height),
            initialized: true,
            command_queue: Vec::new(),
            player_id: 0,
        })
    }

    /// Initialize the UI system
    pub fn init(&mut self) -> Result<()> {
        log::info!("Initializing Integrated UI System");

        // Set up initial command panel buttons (example)
        self.setup_default_commands();

        self.initialized = true;
        log::info!("Integrated UI System initialized successfully");

        Ok(())
    }

    /// Set up default command panel buttons
    fn setup_default_commands(&mut self) {
        let default_buttons = vec![
            CommandButton::new(
                "dozer".into(),
                "Dozer".into(),
                "dozer_icon".into(),
                CommandButtonType::Build,
            )
            .with_hotkey('D')
            .with_cost(500)
            .with_description("Build structures".into()),
            CommandButton::new(
                "supply_center".into(),
                "Supply Center".into(),
                "supply_icon".into(),
                CommandButtonType::Build,
            )
            .with_hotkey('S')
            .with_cost(2000)
            .with_description("Generates resources".into()),
            CommandButton::new(
                "barracks".into(),
                "Barracks".into(),
                "barracks_icon".into(),
                CommandButtonType::Build,
            )
            .with_hotkey('B')
            .with_cost(500)
            .with_description("Trains infantry".into()),
            CommandButton::new(
                "war_factory".into(),
                "War Factory".into(),
                "factory_icon".into(),
                CommandButtonType::Build,
            )
            .with_hotkey('W')
            .with_cost(2000)
            .with_description("Builds vehicles".into()),
        ];

        self.command_panel.set_buttons(default_buttons);
    }

    /// Update UI based on current selection
    pub fn update_for_selection(&mut self, selected_objects: Vec<u32>) -> Result<()> {
        // Update in-game UI selection state
        self.ingame_ui.clear_selection();
        for id in &selected_objects {
            self.ingame_ui.select_object(*id, true);
        }

        // Update command panel context
        let context = if selected_objects.is_empty() {
            CommandPanelContext::Default
        } else if selected_objects.len() > 1 {
            CommandPanelContext::MultiSelect
        } else {
            let object_id = selected_objects[0] as gamelogic::common::ObjectID;
            let mut context = CommandPanelContext::Unit;
            if let Some(obj) = gamelogic::object::registry::OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(guard) = obj.read() {
                    if guard.is_kind_of(gamelogic::common::KindOf::Structure)
                        || guard.is_kind_of(gamelogic::common::KindOf::Building)
                    {
                        context = CommandPanelContext::Structure;
                    }
                }
            }
            context
        };

        self.command_panel.set_context(context);

        Ok(())
    }

    /// Update resources display
    pub fn update_resources(&mut self, credits: i32, power_available: i32, power_used: i32) {
        self.ingame_ui
            .update_resources(credits, power_available, power_used);
    }

    /// Update minimap unit icons
    pub fn update_minimap_unit(&mut self, id: u32, world_x: f32, world_z: f32, color: [f32; 4]) {
        self.ingame_ui
            .update_minimap_unit(id, Vec2::new(world_x, world_z), color);
    }

    /// Remove unit from minimap
    pub fn remove_minimap_unit(&mut self, id: u32) {
        self.ingame_ui.remove_minimap_unit(id);
    }

    /// Set minimap world bounds
    pub fn set_minimap_bounds(&mut self, min_x: f32, min_z: f32, max_x: f32, max_z: f32) {
        self.ingame_ui
            .set_minimap_world_bounds(Vec2::new(min_x, min_z), Vec2::new(max_x, max_z));
    }

    /// Update camera position for minimap
    pub fn update_camera(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        self.ingame_ui.update_camera(
            glam::Vec3::new(x, y, z),
            Vec2::new(viewport_width, viewport_height),
        );
    }

    /// Start building placement mode
    pub fn start_building_placement(
        &mut self,
        template_name: String,
        footprint_x: f32,
        footprint_z: f32,
    ) {
        self.ingame_ui
            .start_building_placement(template_name, Vec2::new(footprint_x, footprint_z));
    }

    /// Cancel building placement
    pub fn cancel_building_placement(&mut self) {
        self.ingame_ui.cancel_building_placement();
    }

    /// Handle input events
    pub fn handle_input(&mut self, mouse: &MouseState, keyboard: &KeyboardState) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        // Handle command panel mouse input
        if let Some(command_id) = self.command_panel.handle_mouse_input(mouse) {
            log::info!("Command panel button clicked: {}", command_id);
            self.command_queue.push(UICommand::Build(command_id));
        }

        // Handle hotkey input from keyboard state
        // Check for pressed keys and match them to command panel hotkeys
        if let Some(last_key) = keyboard.get_last_key_pressed() {
            // Convert KeyCode to char for hotkey matching
            if let Some(key_char) = self.keycode_to_char(last_key) {
                if let Some(command_id) = self.command_panel.handle_hotkey(key_char) {
                    log::info!(
                        "Command panel hotkey activated: {} ({})",
                        command_id,
                        key_char
                    );
                    self.command_queue.push(UICommand::Build(command_id));
                }
            }
        }

        // Handle selection groups (Ctrl+1-9 to save, 1-9 to recall)
        let ctrl_pressed = keyboard.is_ctrl_pressed();
        for group_num in 0..10 {
            let key = match group_num {
                0 => crate::input::keyboard::KeyCode::Num0,
                1 => crate::input::keyboard::KeyCode::Num1,
                2 => crate::input::keyboard::KeyCode::Num2,
                3 => crate::input::keyboard::KeyCode::Num3,
                4 => crate::input::keyboard::KeyCode::Num4,
                5 => crate::input::keyboard::KeyCode::Num5,
                6 => crate::input::keyboard::KeyCode::Num6,
                7 => crate::input::keyboard::KeyCode::Num7,
                8 => crate::input::keyboard::KeyCode::Num8,
                9 => crate::input::keyboard::KeyCode::Num9,
                _ => continue,
            };

            if keyboard.is_key_just_pressed(key) {
                if ctrl_pressed {
                    // Ctrl+Number: Save group
                    self.set_selection_group(group_num);
                } else {
                    // Number: Recall group
                    self.recall_selection_group(group_num);
                }
            }
        }

        // Handle selection and minimap input
        self.ingame_ui.handle_mouse_input(mouse, keyboard)?;

        Ok(())
    }

    /// Convert KeyCode to character for hotkey matching
    fn keycode_to_char(&self, key: crate::input::keyboard::KeyCode) -> Option<char> {
        use crate::input::keyboard::KeyCode;
        match key {
            KeyCode::A => Some('A'),
            KeyCode::B => Some('B'),
            KeyCode::C => Some('C'),
            KeyCode::D => Some('D'),
            KeyCode::E => Some('E'),
            KeyCode::F => Some('F'),
            KeyCode::G => Some('G'),
            KeyCode::H => Some('H'),
            KeyCode::I => Some('I'),
            KeyCode::J => Some('J'),
            KeyCode::K => Some('K'),
            KeyCode::L => Some('L'),
            KeyCode::M => Some('M'),
            KeyCode::N => Some('N'),
            KeyCode::O => Some('O'),
            KeyCode::P => Some('P'),
            KeyCode::Q => Some('Q'),
            KeyCode::R => Some('R'),
            KeyCode::S => Some('S'),
            KeyCode::T => Some('T'),
            KeyCode::U => Some('U'),
            KeyCode::V => Some('V'),
            KeyCode::W => Some('W'),
            KeyCode::X => Some('X'),
            KeyCode::Y => Some('Y'),
            KeyCode::Z => Some('Z'),
            _ => None,
        }
    }

    /// Update UI state
    pub fn update(&mut self, delta_time: Duration) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        self.ingame_ui.update(delta_time);
        self.command_panel.update(delta_time);

        Ok(())
    }

    /// Render the complete UI
    pub fn render(&mut self, target: &TextureView) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        // Begin frame
        {
            let mut renderer = self
                .renderer
                .write()
                .map_err(|_| IntegratedUIError::SystemError("Failed to lock renderer".into()))?;
            renderer.set_screen_size(self.screen_size.0, self.screen_size.1);
            renderer.begin_frame();
        }

        // Render in-game UI (selection box, minimap, resources, placement preview)
        self.ingame_ui.render()?;
        self.command_panel.render()?;

        // Execute rendering with a real render pass
        {
            let mut renderer = self
                .renderer
                .write()
                .map_err(|_| IntegratedUIError::SystemError("Failed to lock renderer".into()))?;

            let mut encoder = renderer
                .device()
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("IntegratedUI Render Encoder"),
                });

            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("IntegratedUI Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                renderer.render(&mut render_pass)?;
            }

            renderer.queue().submit(Some(encoder.finish()));
            renderer.end_frame();
        }

        Ok(())
    }

    /// Resize UI elements
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
        self.ingame_ui.resize(width as f32, height as f32);
        self.command_panel.resize(width as f32, height as f32);
    }

    /// Get pending UI commands
    pub fn get_commands(&mut self) -> Vec<UICommand> {
        std::mem::take(&mut self.command_queue)
    }

    /// Set current player
    pub fn set_player(&mut self, player_id: u32) {
        self.player_id = player_id;
        self.ingame_ui.set_player_id(player_id);
    }

    /// Enable/disable UI
    pub fn set_enabled(&mut self, enabled: bool) {
        self.ingame_ui.set_enabled(enabled);
        self.command_panel.set_visible(enabled);
    }

    /// Get current selection
    pub fn get_selection(&self) -> Vec<u32> {
        self.ingame_ui.get_selection()
    }

    /// Set selection group
    pub fn set_selection_group(&mut self, group: usize) {
        if group < 10 {
            self.ingame_ui.set_selection_group(group);
            self.command_queue.push(UICommand::CreateGroup(group));
        }
    }

    /// Recall selection group
    pub fn recall_selection_group(&mut self, group: usize) {
        if group < 10 {
            self.ingame_ui.recall_selection_group(group);
            self.command_queue.push(UICommand::RecallGroup(group));
        }
    }
}

/// Builder for IntegratedUISystem
pub struct IntegratedUISystemBuilder {
    device: Option<Arc<Device>>,
    queue: Option<Arc<Queue>>,
    format: Option<TextureFormat>,
    screen_width: u32,
    screen_height: u32,
}

impl IntegratedUISystemBuilder {
    pub fn new() -> Self {
        Self {
            device: None,
            queue: None,
            format: None,
            screen_width: 1024,
            screen_height: 768,
        }
    }

    pub fn with_device(mut self, device: Arc<Device>) -> Self {
        self.device = Some(device);
        self
    }

    pub fn with_queue(mut self, queue: Arc<Queue>) -> Self {
        self.queue = Some(queue);
        self
    }

    pub fn with_format(mut self, format: TextureFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub fn with_screen_size(mut self, width: u32, height: u32) -> Self {
        self.screen_width = width;
        self.screen_height = height;
        self
    }

    pub fn build(self) -> Result<IntegratedUISystem> {
        let device = self
            .device
            .ok_or_else(|| IntegratedUIError::SystemError("Device not set".into()))?;
        let queue = self
            .queue
            .ok_or_else(|| IntegratedUIError::SystemError("Queue not set".into()))?;
        let format = self
            .format
            .ok_or_else(|| IntegratedUIError::SystemError("Format not set".into()))?;

        IntegratedUISystem::new(device, queue, format, self.screen_width, self.screen_height)
    }
}

impl Default for IntegratedUISystemBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a wgpu device which is not available in unit tests
    // They are included for documentation and would run in integration tests

    #[test]
    #[ignore]
    fn test_ui_system_creation() {
        // This would require a real wgpu device
        // let device = ...;
        // let queue = ...;
        // let system = IntegratedUISystem::new(...);
        // assert!(system.is_ok());
    }

    #[test]
    fn test_ui_command_queue() {
        let mut queue = Vec::new();
        queue.push(UICommand::Build("barracks".into()));
        queue.push(UICommand::UnitCommand("attack".into()));

        assert_eq!(queue.len(), 2);

        let commands = std::mem::take(&mut queue);
        assert_eq!(commands.len(), 2);
        assert_eq!(queue.len(), 0);
    }
}
