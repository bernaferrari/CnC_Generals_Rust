//! # Command Panel System
//!
//! Command button rendering and interaction system ported from C++ ControlBar.
//! Handles the bottom command panel with context-sensitive buttons for building,
//! unit commands, special powers, and upgrades.
//!
//! Original C++ file: GameClient/GUI/ControlBar/ControlBarCommand.cpp
//! Original Author: Colin Day, March 2002

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use glam::{Vec2, Vec4};
use thiserror::Error;

use super::ui_renderer::{UIRect, UIRenderer, UIRendererError};
use crate::display::image::get_mapped_image_collection;
use crate::input::mouse::{ButtonState, MouseButton, MouseState};

/// Command panel errors
#[derive(Error, Debug)]
pub enum CommandPanelError {
    #[error("Renderer error: {0}")]
    RendererError(#[from] UIRendererError),
    #[error("Invalid command: {0}")]
    InvalidCommand(String),
    #[error("System error: {0}")]
    SystemError(String),
}

type Result<T> = std::result::Result<T, CommandPanelError>;

/// Maximum number of command buttons visible at once
const MAX_VISIBLE_BUTTONS: usize = 18;

/// Command button grid layout (3 rows x 6 columns)
const BUTTON_ROWS: usize = 3;
const BUTTON_COLS: usize = 6;

/// Command button size
const BUTTON_SIZE: f32 = 60.0;
const BUTTON_SPACING: f32 = 4.0;

/// Hotkey codes
const HOTKEY_CODES: [char; 18] = [
    'Q', 'W', 'E', 'R', 'T', 'Y', 'A', 'S', 'D', 'F', 'G', 'H', 'Z', 'X', 'C', 'V', 'B', 'N',
];

/// Command button state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandButtonState {
    /// Button is disabled/unavailable
    Disabled,
    /// Button is available but not selected
    Available,
    /// Button is currently selected/active
    Selected,
    /// Button is being hovered over
    Hovered,
    /// Button is being pressed
    Pressed,
}

/// Command button type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandButtonType {
    /// Build a unit or structure
    Build,
    /// Execute a unit command
    UnitCommand,
    /// Trigger a special power
    SpecialPower,
    /// Purchase an upgrade
    Upgrade,
    /// Cancel current action
    Cancel,
    /// Generic action
    Action,
}

/// Command button definition
#[derive(Debug, Clone)]
pub struct CommandButton {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Description text
    pub description: String,
    /// Icon texture name
    pub icon: String,
    /// Button type
    pub button_type: CommandButtonType,
    /// Hotkey character
    pub hotkey: Option<char>,
    /// Cost in resources
    pub cost: i32,
    /// Build time in seconds
    pub build_time: f32,
    /// Prerequisites met
    pub prerequisites_met: bool,
    /// Currently available
    pub available: bool,
    /// Progress (0.0-1.0) for building/research
    pub progress: f32,
    /// Cooldown remaining (seconds)
    pub cooldown: f32,
    /// Maximum cooldown (seconds)
    pub max_cooldown: f32,
}

impl CommandButton {
    pub fn new(id: String, name: String, icon: String, button_type: CommandButtonType) -> Self {
        Self {
            id,
            name,
            description: String::new(),
            icon,
            button_type,
            hotkey: None,
            cost: 0,
            build_time: 0.0,
            prerequisites_met: true,
            available: true,
            progress: 0.0,
            cooldown: 0.0,
            max_cooldown: 0.0,
        }
    }

    pub fn with_hotkey(mut self, hotkey: char) -> Self {
        self.hotkey = Some(hotkey);
        self
    }

    pub fn with_cost(mut self, cost: i32) -> Self {
        self.cost = cost;
        self
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    pub fn is_on_cooldown(&self) -> bool {
        self.cooldown > 0.0
    }

    pub fn get_cooldown_percentage(&self) -> f32 {
        if self.max_cooldown > 0.0 {
            1.0 - (self.cooldown / self.max_cooldown)
        } else {
            1.0
        }
    }

    pub fn get_state(&self) -> CommandButtonState {
        if !self.available || !self.prerequisites_met {
            CommandButtonState::Disabled
        } else {
            CommandButtonState::Available
        }
    }
}

/// Command button slot in the panel
#[derive(Debug, Clone)]
struct ButtonSlot {
    /// Position in grid (row, col)
    grid_pos: (usize, usize),
    /// Screen rect
    rect: UIRect,
    /// Current button (if any)
    button: Option<CommandButton>,
    /// Visual state
    state: CommandButtonState,
    /// Hover time for tooltip
    hover_time: Option<Instant>,
}

impl ButtonSlot {
    fn new(row: usize, col: usize, rect: UIRect) -> Self {
        Self {
            grid_pos: (row, col),
            rect,
            button: None,
            state: CommandButtonState::Disabled,
            hover_time: None,
        }
    }

    fn contains_point(&self, pos: Vec2) -> bool {
        self.rect.contains(pos.x, pos.y)
    }
}

/// Command panel context - what to display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPanelContext {
    /// No selection - show general commands
    Default,
    /// Single unit selected
    Unit,
    /// Single structure selected
    Structure,
    /// Multiple units selected
    MultiSelect,
    /// Observer mode
    Observer,
}

/// Main command panel
pub struct CommandPanel {
    /// Panel position on screen
    position: Vec2,
    /// Panel size
    size: Vec2,
    /// Button slots
    slots: Vec<ButtonSlot>,
    /// Current context
    context: CommandPanelContext,
    /// Currently hovered button index
    hovered_button: Option<usize>,
    /// Currently pressed button index
    pressed_button: Option<usize>,
    /// Tooltip display delay
    tooltip_delay: Duration,
    /// Whether panel is visible
    visible: bool,
    /// UI renderer
    renderer: Arc<RwLock<UIRenderer>>,
}

impl CommandPanel {
    pub fn new(renderer: Arc<RwLock<UIRenderer>>, screen_width: f32, screen_height: f32) -> Self {
        let panel_width = (BUTTON_SIZE + BUTTON_SPACING) * BUTTON_COLS as f32;
        let panel_height = (BUTTON_SIZE + BUTTON_SPACING) * BUTTON_ROWS as f32;
        let panel_x = (screen_width - panel_width) / 2.0;
        let panel_y = screen_height - panel_height - 20.0;

        let position = Vec2::new(panel_x, panel_y);
        let size = Vec2::new(panel_width, panel_height);

        // Create button slots
        let mut slots = Vec::new();
        for row in 0..BUTTON_ROWS {
            for col in 0..BUTTON_COLS {
                let x = position.x + col as f32 * (BUTTON_SIZE + BUTTON_SPACING);
                let y = position.y + row as f32 * (BUTTON_SIZE + BUTTON_SPACING);
                let rect = UIRect::new(x, y, BUTTON_SIZE, BUTTON_SIZE);
                slots.push(ButtonSlot::new(row, col, rect));
            }
        }

        Self {
            position,
            size,
            slots,
            context: CommandPanelContext::Default,
            hovered_button: None,
            pressed_button: None,
            tooltip_delay: Duration::from_millis(500),
            visible: true,
            renderer,
        }
    }

    /// Set command buttons for current context
    pub fn set_buttons(&mut self, buttons: Vec<CommandButton>) {
        // Clear existing buttons
        for slot in &mut self.slots {
            slot.button = None;
            slot.state = CommandButtonState::Disabled;
        }

        // Assign new buttons to slots
        for (i, button) in buttons.into_iter().enumerate() {
            if i < self.slots.len() {
                self.slots[i].state = button.get_state();
                self.slots[i].button = Some(button);
            }
        }
    }

    /// Update button state
    pub fn update_button(&mut self, button_id: &str, update_fn: impl FnOnce(&mut CommandButton)) {
        for slot in &mut self.slots {
            if let Some(ref mut button) = slot.button {
                if button.id == button_id {
                    update_fn(button);
                    slot.state = button.get_state();
                    break;
                }
            }
        }
    }

    /// Set command panel context
    pub fn set_context(&mut self, context: CommandPanelContext) {
        self.context = context;
    }

    /// Handle mouse input
    pub fn handle_mouse_input(&mut self, mouse: &MouseState) -> Option<String> {
        if !self.visible {
            return None;
        }

        let mouse_pos = Vec2::new(mouse.position().0, mouse.position().1);
        let left_button = mouse.button_state(MouseButton::Left);

        // Update hover state
        let mut new_hovered = None;
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.contains_point(mouse_pos) && slot.button.is_some() {
                new_hovered = Some(i);
                if slot.hover_time.is_none() {
                    slot.hover_time = Some(Instant::now());
                }
            } else {
                slot.hover_time = None;
            }
        }

        self.hovered_button = new_hovered;

        // Handle button press
        let mut clicked_button = None;

        match left_button {
            ButtonState::JustPressed => {
                if let Some(hovered) = self.hovered_button {
                    if let Some(ref button) = self.slots[hovered].button {
                        if button.get_state() != CommandButtonState::Disabled {
                            self.pressed_button = Some(hovered);
                            self.slots[hovered].state = CommandButtonState::Pressed;
                        }
                    }
                }
            }
            ButtonState::JustReleased => {
                if let Some(pressed) = self.pressed_button {
                    if Some(pressed) == self.hovered_button {
                        // Button was clicked
                        if let Some(ref button) = self.slots[pressed].button {
                            clicked_button = Some(button.id.clone());
                        }
                    }
                    self.pressed_button = None;
                    // Restore state
                    if let Some(ref button) = self.slots[pressed].button {
                        self.slots[pressed].state = button.get_state();
                    }
                }
            }
            _ => {}
        }

        // Update visual states
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.button.is_some() && Some(i) != self.pressed_button {
                slot.state = if Some(i) == self.hovered_button {
                    CommandButtonState::Hovered
                } else if let Some(ref button) = slot.button {
                    button.get_state()
                } else {
                    CommandButtonState::Disabled
                };
            }
        }

        clicked_button
    }

    /// Handle keyboard input for hotkeys
    pub fn handle_hotkey(&mut self, key: char) -> Option<String> {
        let key_upper = key.to_ascii_uppercase();

        for slot in &self.slots {
            if let Some(ref button) = slot.button {
                if let Some(hotkey) = button.hotkey {
                    if hotkey == key_upper && button.get_state() != CommandButtonState::Disabled {
                        return Some(button.id.clone());
                    }
                }
            }
        }

        None
    }

    /// Update command panel state
    pub fn update(&mut self, delta_time: Duration) {
        // Update cooldowns
        let delta_secs = delta_time.as_secs_f32();

        for slot in &mut self.slots {
            if let Some(ref mut button) = slot.button {
                if button.cooldown > 0.0 {
                    button.cooldown = (button.cooldown - delta_secs).max(0.0);
                }
            }
        }
    }

    /// Render command panel
    pub fn render(&self) -> Result<()> {
        if !self.visible {
            return Ok(());
        }

        let mut renderer = self
            .renderer
            .write()
            .map_err(|_| CommandPanelError::SystemError("Failed to lock renderer".into()))?;

        // Draw panel background
        let panel_rect = UIRect::new(self.position.x, self.position.y, self.size.x, self.size.y);
        renderer.draw_rect_with_scissor(panel_rect, [0.0, 0.0, 0.0, 0.8], None)?;

        // Draw panel border
        renderer.draw_rect_outline_with_scissor(panel_rect, 2.0, [0.5, 0.5, 0.5, 1.0], None)?;

        // Draw buttons
        for slot in &self.slots {
            self.render_button_slot(&mut renderer, slot)?;
        }

        // Draw tooltip if hovering
        if let Some(hovered) = self.hovered_button {
            if let Some(ref hover_time) = self.slots[hovered].hover_time {
                if hover_time.elapsed() >= self.tooltip_delay {
                    if let Some(ref button) = self.slots[hovered].button {
                        self.render_tooltip(&mut renderer, button, self.slots[hovered].rect)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Render a single button slot
    fn render_button_slot(&self, renderer: &mut UIRenderer, slot: &ButtonSlot) -> Result<()> {
        // Determine colors based on state
        let (bg_color, border_color) = match slot.state {
            CommandButtonState::Disabled => ([0.2, 0.2, 0.2, 0.5], [0.4, 0.4, 0.4, 0.8]),
            CommandButtonState::Available => ([0.3, 0.3, 0.3, 0.9], [0.6, 0.6, 0.6, 1.0]),
            CommandButtonState::Selected => ([0.2, 0.4, 0.6, 0.9], [0.4, 0.8, 1.0, 1.0]),
            CommandButtonState::Hovered => ([0.4, 0.4, 0.4, 0.9], [0.8, 0.8, 0.8, 1.0]),
            CommandButtonState::Pressed => ([0.5, 0.5, 0.5, 0.9], [1.0, 1.0, 1.0, 1.0]),
        };

        // Draw button background
        renderer.draw_rect_with_scissor(slot.rect, bg_color, None)?;

        // Draw button border
        renderer.draw_rect_outline_with_scissor(slot.rect, 2.0, border_color, None)?;

        if let Some(ref button) = slot.button {
            // Draw icon
            let icon_rect = UIRect::new(
                slot.rect.x + 5.0,
                slot.rect.y + 5.0,
                slot.rect.width - 10.0,
                slot.rect.height - 10.0,
            );
            let mut drew_icon = false;
            if !button.icon.is_empty() {
                let collection = get_mapped_image_collection();
                if let Some(mut mapped) = collection.try_write() {
                    if let Some(image) = mapped.find_image_by_name_mut(&button.icon) {
                        if image.get_gpu_texture().is_none() {
                            let _ = image.create_gpu_texture(renderer.device(), renderer.queue());
                        }
                        if let Some(gpu) = image.get_gpu_texture() {
                            let uv = image.get_uv();
                            let tex_rect = UIRect::new(
                                uv.min.x,
                                uv.min.y,
                                uv.max.x - uv.min.x,
                                uv.max.y - uv.min.y,
                            );
                            renderer.draw_textured_rect(
                                icon_rect,
                                Arc::new(gpu.view().clone()),
                                [1.0, 1.0, 1.0, 1.0],
                                Some(tex_rect),
                                0.0,
                            );
                            drew_icon = true;
                        }
                    }
                };
            }
            if !drew_icon {
                renderer.draw_rect_with_scissor(icon_rect, [0.5, 0.5, 0.5, 1.0], None)?;
            }

            // Draw hotkey
            if let Some(hotkey) = button.hotkey {
                let hotkey_text = hotkey.to_string();
                renderer.draw_text_simple(
                    &hotkey_text,
                    Vec2::new(slot.rect.x + 5.0, slot.rect.y + slot.rect.height - 18.0),
                    12.0,
                    [1.0, 1.0, 1.0, 1.0],
                )?;
            }

            // Draw cost
            if button.cost > 0 {
                let cost_text = format!("${}", button.cost);
                let text_width = cost_text.len() as f32 * 6.0; // Approximate
                renderer.draw_text_simple(
                    &cost_text,
                    Vec2::new(
                        slot.rect.x + slot.rect.width - text_width - 5.0,
                        slot.rect.y + slot.rect.height - 18.0,
                    ),
                    10.0,
                    [1.0, 1.0, 0.0, 1.0],
                )?;
            }

            // Draw progress bar
            if button.progress > 0.0 && button.progress < 1.0 {
                let progress_height = 4.0;
                let progress_rect = UIRect::new(
                    slot.rect.x + 2.0,
                    slot.rect.y + slot.rect.height - progress_height - 2.0,
                    (slot.rect.width - 4.0) * button.progress,
                    progress_height,
                );
                renderer.draw_rect_with_scissor(progress_rect, [0.0, 1.0, 0.0, 1.0], None)?;
            }

            // Draw cooldown overlay
            if button.is_on_cooldown() {
                let cooldown_pct = 1.0 - button.get_cooldown_percentage();
                let overlay_height = slot.rect.height * cooldown_pct;
                let cooldown_rect =
                    UIRect::new(slot.rect.x, slot.rect.y, slot.rect.width, overlay_height);
                renderer.draw_rect_with_scissor(cooldown_rect, [0.0, 0.0, 0.0, 0.6], None)?;

                // Draw cooldown text
                let cooldown_text = format!("{:.1}s", button.cooldown);
                renderer.draw_text_simple(
                    &cooldown_text,
                    Vec2::new(
                        slot.rect.x + slot.rect.width / 2.0 - 15.0,
                        slot.rect.y + slot.rect.height / 2.0 - 8.0,
                    ),
                    14.0,
                    [1.0, 1.0, 1.0, 1.0],
                )?;
            }
        }

        Ok(())
    }

    /// Render button tooltip
    fn render_tooltip(
        &self,
        renderer: &mut UIRenderer,
        button: &CommandButton,
        button_rect: UIRect,
    ) -> Result<()> {
        let tooltip_width = 250.0;
        let tooltip_height = 80.0;
        let tooltip_x = button_rect.x + button_rect.width + 10.0;
        let tooltip_y = button_rect.y;

        let tooltip_rect = UIRect::new(tooltip_x, tooltip_y, tooltip_width, tooltip_height);

        // Draw tooltip background
        renderer.draw_rect_with_scissor(tooltip_rect, [0.1, 0.1, 0.1, 0.95], None)?;

        // Draw tooltip border
        renderer.draw_rect_outline_with_scissor(tooltip_rect, 1.0, [0.7, 0.7, 0.7, 1.0], None)?;

        // Draw title
        renderer.draw_text_simple(
            &button.name,
            Vec2::new(tooltip_x + 10.0, tooltip_y + 10.0),
            14.0,
            [1.0, 1.0, 0.0, 1.0],
        )?;

        // Draw description
        renderer.draw_text_simple(
            &button.description,
            Vec2::new(tooltip_x + 10.0, tooltip_y + 30.0),
            11.0,
            [0.9, 0.9, 0.9, 1.0],
        )?;

        // Draw cost and build time
        let info_text = if button.build_time > 0.0 {
            format!("Cost: ${} | Time: {:.1}s", button.cost, button.build_time)
        } else {
            format!("Cost: ${}", button.cost)
        };

        renderer.draw_text_simple(
            &info_text,
            Vec2::new(tooltip_x + 10.0, tooltip_y + 55.0),
            10.0,
            [0.7, 0.7, 0.7, 1.0],
        )?;

        Ok(())
    }

    /// Resize panel
    pub fn resize(&mut self, screen_width: f32, screen_height: f32) {
        let panel_width = (BUTTON_SIZE + BUTTON_SPACING) * BUTTON_COLS as f32;
        let panel_height = (BUTTON_SIZE + BUTTON_SPACING) * BUTTON_ROWS as f32;
        let panel_x = (screen_width - panel_width) / 2.0;
        let panel_y = screen_height - panel_height - 20.0;

        self.position = Vec2::new(panel_x, panel_y);

        // Update button slot positions
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let row = i / BUTTON_COLS;
            let col = i % BUTTON_COLS;
            let x = self.position.x + col as f32 * (BUTTON_SIZE + BUTTON_SPACING);
            let y = self.position.y + row as f32 * (BUTTON_SIZE + BUTTON_SPACING);
            slot.rect = UIRect::new(x, y, BUTTON_SIZE, BUTTON_SIZE);
        }
    }

    /// Set visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Get visibility
    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_button_creation() {
        let button = CommandButton::new(
            "build_barracks".into(),
            "Barracks".into(),
            "barracks_icon".into(),
            CommandButtonType::Build,
        )
        .with_hotkey('B')
        .with_cost(500)
        .with_description("Trains infantry units".into());

        assert_eq!(button.id, "build_barracks");
        assert_eq!(button.hotkey, Some('B'));
        assert_eq!(button.cost, 500);
        assert!(button.available);
    }

    #[test]
    fn test_button_cooldown() {
        let mut button = CommandButton::new(
            "nuke".into(),
            "Nuclear Missile".into(),
            "nuke_icon".into(),
            CommandButtonType::SpecialPower,
        );

        button.max_cooldown = 300.0;
        button.cooldown = 150.0;

        assert!(button.is_on_cooldown());
        assert!((button.get_cooldown_percentage() - 0.5).abs() < 0.01);

        button.cooldown = 0.0;
        assert!(!button.is_on_cooldown());
        assert_eq!(button.get_cooldown_percentage(), 1.0);
    }

    #[test]
    fn test_button_state() {
        let mut button = CommandButton::new(
            "test".into(),
            "Test".into(),
            "test_icon".into(),
            CommandButtonType::Action,
        );

        assert_eq!(button.get_state(), CommandButtonState::Available);

        button.available = false;
        assert_eq!(button.get_state(), CommandButtonState::Disabled);

        button.available = true;
        button.prerequisites_met = false;
        assert_eq!(button.get_state(), CommandButtonState::Disabled);
    }
}
