//! Enhanced Window Manager Implementation
//!
//! Complete window management system that handles creation, destruction, hierarchy,
//! event routing, focus management, and rendering coordination.

use std::collections::{HashMap, VecDeque};
use std::sync::{atomic::AtomicI32, Arc, Mutex, RwLock};
use std::time::Instant;
use thiserror::Error;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode as WinitKeyCode, PhysicalKey};

use super::game_window::{
    color_to_rgba, WindowDrawData as LegacyDrawData, WindowStatus as LegacyWindowStatus,
    WindowWidget, GWS_ALL_SLIDER, GWS_CHECK_BOX, GWS_COMBO_BOX, GWS_ENTRY_FIELD, GWS_HORZ_SLIDER,
    GWS_MOUSE_TRACK, GWS_PROGRESS_BAR, GWS_PUSH_BUTTON, GWS_RADIO_BUTTON, GWS_SCROLL_LISTBOX,
    GWS_STATIC_TEXT, GWS_TAB_CONTROL, GWS_TAB_PANE, GWS_TAB_STOP, GWS_USER_WINDOW, GWS_VERT_SLIDER,
};
use super::game_window_enhanced::{
    ComboBoxLinks, EnhancedGameWindow, ListBoxLinks, WindowId, WindowMessage, WindowMsgHandled,
    WindowStatus, WINDOW_ID_INVALID,
};
use super::ui_renderer::UIRenderer;
use super::window_script::{parse_window_script, WindowDefinition, WindowLayoutDefinition};
use crate::core::subsystems::RadarPingKind;
use crate::game_text::GameText;
use crate::gui::gadgets::KeyCode as GuiKeyCode;
use crate::gui::gadgets::{
    CheckBox, ComboBox, HorizontalSlider, ListBox, ProgressBar, PushButton, RadioButton,
    RadioButtonGroup, SelectionMode, StaticText, TabControl, TabControlData, TextAlignment,
    TextEntry, ValidationMode, VerticalAlignment, VerticalSlider,
};
use crate::gui::header_template::get_header_template_manager;
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::path::{Path, PathBuf};

/// Window Manager errors
#[derive(Error, Debug)]
pub enum WindowManagerError {
    #[error("Window not found: {0}")]
    WindowNotFound(WindowId),
    #[error("Invalid window operation: {0}")]
    InvalidOperation(String),
    #[error("Window creation failed: {0}")]
    CreationFailed(String),
    #[error("Event handling error: {0}")]
    EventError(String),
    #[error("Too many windows created (max: {max})")]
    TooManyWindows { max: usize },
}

impl From<super::game_window_enhanced::GameWindowError> for WindowManagerError {
    fn from(err: super::game_window_enhanced::GameWindowError) -> Self {
        WindowManagerError::InvalidOperation(err.to_string())
    }
}

impl From<super::ui_renderer::UIRendererError> for WindowManagerError {
    fn from(err: super::ui_renderer::UIRendererError) -> Self {
        WindowManagerError::EventError(err.to_string())
    }
}

type Result<T> = std::result::Result<T, WindowManagerError>;

/// Tab direction for keyboard navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabDirection {
    Forward,
    Backward,
}

/// Capture flags for mouse and keyboard input
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CaptureFlags: u32 {
        const NONE = 0x00;
        const MOUSE = 0x01;
        const KEYBOARD = 0x02;
        const ALL = Self::MOUSE.bits | Self::KEYBOARD.bits;
    }
}

/// Modal window information
#[derive(Debug, Clone)]
pub struct ModalWindow {
    window_id: WindowId,
    background_color: [f32; 4],
    close_on_click_outside: bool,
}

/// Window layout information for loading from files
#[derive(Debug, Clone)]
pub struct WindowLayoutInfo {
    pub file_path: String,
    pub last_modified: std::time::SystemTime,
    pub is_loaded: bool,
}

/// Event queue entry
#[derive(Debug, Clone)]
struct QueuedEvent {
    window_id: WindowId,
    message: WindowMessage,
    wparam: u32,
    lparam: u32,
    timestamp: Instant,
}

/// Focus change information
#[derive(Debug, Clone)]
struct FocusChange {
    old_focus: Option<WindowId>,
    new_focus: Option<WindowId>,
    timestamp: Instant,
}

/// Enhanced Window Manager
pub struct EnhancedWindowManager {
    // Window storage and ID management
    windows: RwLock<HashMap<WindowId, Arc<EnhancedGameWindow>>>,
    next_window_id: AtomicI32,
    root_windows: RwLock<Vec<Arc<EnhancedGameWindow>>>,

    // Focus and input management
    focused_window: RwLock<Option<WindowId>>,
    mouse_capture_window: RwLock<Option<WindowId>>,
    keyboard_capture_window: RwLock<Option<WindowId>>,
    mouse_position: RwLock<(f32, f32)>,
    tab_list: RwLock<Vec<WindowId>>,
    radio_groups: RwLock<HashMap<u32, RadioButtonGroup>>,

    // Modal window support
    modal_stack: RwLock<Vec<ModalWindow>>,

    // Event handling
    event_queue: Mutex<VecDeque<QueuedEvent>>,
    focus_history: RwLock<Vec<FocusChange>>,

    // Layout management
    loaded_layouts: RwLock<HashMap<String, WindowLayoutInfo>>,

    // Rendering
    renderer: RwLock<Option<Arc<RwLock<UIRenderer>>>>,
    radar_overlay: RwLock<Vec<(f32, f32, f32, RadarPingKind)>>, // normalized x,z, age, kind

    // Configuration
    max_windows: usize,
    enable_tooltips: bool,
    tooltip_delay: u32,

    // Performance tracking
    frame_count: AtomicI32,
    last_update_time: RwLock<Instant>,
}

impl EnhancedWindowManager {
    /// Create a new enhanced window manager
    pub fn new() -> Self {
        Self {
            windows: RwLock::new(HashMap::new()),
            next_window_id: AtomicI32::new(1),
            root_windows: RwLock::new(Vec::new()),
            focused_window: RwLock::new(None),
            mouse_capture_window: RwLock::new(None),
            keyboard_capture_window: RwLock::new(None),
            mouse_position: RwLock::new((0.0, 0.0)),
            tab_list: RwLock::new(Vec::new()),
            radio_groups: RwLock::new(HashMap::new()),
            modal_stack: RwLock::new(Vec::new()),
            event_queue: Mutex::new(VecDeque::new()),
            focus_history: RwLock::new(Vec::new()),
            loaded_layouts: RwLock::new(HashMap::new()),
            renderer: RwLock::new(None),
            radar_overlay: RwLock::new(Vec::new()),
            max_windows: 576, // Match C++ MAX_WINDOWS
            enable_tooltips: true,
            tooltip_delay: 1000,
            frame_count: AtomicI32::new(0),
            last_update_time: RwLock::new(Instant::now()),
        }
    }

    /// Initialize the window manager with a UI renderer
    pub fn initialize(&self, renderer: Arc<RwLock<UIRenderer>>) -> Result<()> {
        *self.renderer.write().unwrap_or_else(|e| e.into_inner()) = Some(renderer);
        Ok(())
    }

    /// Update the overlay radar pings (normalized coordinates).
    pub fn set_radar_overlay(&self, dots: Vec<(f32, f32, f32, RadarPingKind)>) {
        *self
            .radar_overlay
            .write()
            .unwrap_or_else(|e| e.into_inner()) = dots;
    }

    /// Create a new window
    pub fn create_window(
        &self,
        parent: Option<&Arc<EnhancedGameWindow>>,
        name: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Arc<EnhancedGameWindow>> {
        let windows = self.windows.read().unwrap_or_else(|e| e.into_inner());
        if windows.len() >= self.max_windows {
            return Err(WindowManagerError::TooManyWindows {
                max: self.max_windows,
            });
        }
        drop(windows);

        // Generate unique window ID
        let window_id = self
            .next_window_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Create the window
        let window = EnhancedGameWindow::new(window_id, name);
        window.set_bounds(x, y, width, height);
        window.set_status(WindowStatus::ENABLED);

        // Add to parent if specified
        if let Some(parent_window) = parent {
            parent_window.add_child(window.clone())?;
        } else {
            // Add to root windows list
            self.root_windows
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .push(window.clone());
        }

        // Store in windows map
        self.windows
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(window_id, window.clone());

        // Send create message
        window.send_message(WindowMessage::Create, 0, 0);

        Ok(window)
    }

    pub fn create_window_with_id(
        &self,
        parent: Option<&Arc<EnhancedGameWindow>>,
        name: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        window_id: WindowId,
    ) -> Result<Arc<EnhancedGameWindow>> {
        let windows = self.windows.read().unwrap_or_else(|e| e.into_inner());
        if windows.len() >= self.max_windows {
            return Err(WindowManagerError::TooManyWindows {
                max: self.max_windows,
            });
        }
        if windows.contains_key(&window_id) {
            return Err(WindowManagerError::CreationFailed(format!(
                "Window ID {} already exists",
                window_id
            )));
        }
        drop(windows);

        let window = EnhancedGameWindow::new(window_id, name);
        window.set_bounds(x, y, width, height);
        window.set_status(WindowStatus::ENABLED);

        if let Some(parent_window) = parent {
            parent_window.add_child(window.clone())?;
        } else {
            self.root_windows
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .push(window.clone());
        }

        self.windows
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(window_id, window.clone());

        let current_next = self
            .next_window_id
            .load(std::sync::atomic::Ordering::SeqCst);
        if window_id >= current_next {
            self.next_window_id.store(
                window_id.saturating_add(1),
                std::sync::atomic::Ordering::SeqCst,
            );
        }

        window.send_message(WindowMessage::Create, 0, 0);

        Ok(window)
    }

    /// Load a legacy .wnd layout and create the window hierarchy.
    pub fn create_windows_from_script(
        &self,
        filename: &str,
    ) -> Result<Vec<Arc<EnhancedGameWindow>>> {
        let path = resolve_window_script_path(filename).map_err(|_| {
            WindowManagerError::InvalidOperation(format!("Layout not found: {}", filename))
        })?;
        let layout_def = parse_window_script(&path)
            .map_err(|err| WindowManagerError::InvalidOperation(err.to_string()))?;

        let screen_size = self.get_screen_size();
        let mut roots = Vec::new();
        for window_def in &layout_def.windows {
            let window =
                self.create_window_from_definition(window_def, None, &layout_def, screen_size)?;
            roots.push(window);
        }
        Ok(roots)
    }

    /// Destroy a window and all its children
    pub fn destroy_window(&self, window_id: WindowId) -> Result<()> {
        let window = {
            let windows = self.windows.read().unwrap_or_else(|e| e.into_inner());
            windows
                .get(&window_id)
                .cloned()
                .ok_or(WindowManagerError::WindowNotFound(window_id))?
        };

        // Send destroy message
        window.send_message(WindowMessage::Destroy, 0, 0);

        // Recursively destroy children
        let children = window.get_children();
        for child in children {
            self.destroy_window(child.get_id())?;
        }

        // Remove from parent
        if let Some(parent) = window.get_parent() {
            parent.remove_child(&window)?;
        } else {
            // Remove from root windows
            let mut root_windows = self.root_windows.write().unwrap_or_else(|e| e.into_inner());
            root_windows.retain(|w| w.get_id() != window_id);
        }

        // Clear focus if this window was focused
        let mut focused = self
            .focused_window
            .write()
            .unwrap_or_else(|e| e.into_inner());
        if *focused == Some(window_id) {
            *focused = None;
        }

        // Clear captures
        let mut mouse_capture = self
            .mouse_capture_window
            .write()
            .unwrap_or_else(|e| e.into_inner());
        if *mouse_capture == Some(window_id) {
            *mouse_capture = None;
        }

        let mut keyboard_capture = self
            .keyboard_capture_window
            .write()
            .unwrap_or_else(|e| e.into_inner());
        if *keyboard_capture == Some(window_id) {
            *keyboard_capture = None;
        }

        // Mark as destroyed
        window.set_status(window.get_status() | WindowStatus::DESTROYED);

        // Remove from windows map
        self.windows
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&window_id);
        self.tab_list
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|id| *id != window_id);

        Ok(())
    }

    /// Find a window by ID
    pub fn find_window(&self, window_id: WindowId) -> Option<Arc<EnhancedGameWindow>> {
        self.windows
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&window_id)
            .cloned()
    }

    /// Find a window by name (searches recursively)
    pub fn find_window_by_name(&self, name: &str) -> Option<Arc<EnhancedGameWindow>> {
        let root_windows = self.root_windows.read().unwrap_or_else(|e| e.into_inner());
        for root_window in root_windows.iter() {
            if root_window.get_name() == name {
                return Some(root_window.clone());
            }
            if let Some(found) = root_window.find_child_by_name(name) {
                return Some(found);
            }
        }
        None
    }

    /// Get the currently focused window
    pub fn get_focused_window(&self) -> Option<Arc<EnhancedGameWindow>> {
        let focused_id = *self
            .focused_window
            .read()
            .unwrap_or_else(|e| e.into_inner());
        focused_id.and_then(|id| self.find_window(id))
    }

    /// Set window focus
    pub fn set_focus(&self, window_id: Option<WindowId>) -> Result<()> {
        let old_focus = *self
            .focused_window
            .read()
            .unwrap_or_else(|e| e.into_inner());

        // Validate new focus window exists
        if let Some(new_id) = window_id {
            if let Some(window) = self.find_window(new_id) {
                let status = window.get_status();
                if !status.contains(WindowStatus::ENABLED)
                    || status.contains(WindowStatus::HIDDEN)
                    || status.contains(WindowStatus::NO_FOCUS)
                {
                    return Ok(());
                }
            } else {
                return Err(WindowManagerError::WindowNotFound(new_id));
            }
        }

        // Update focus
        *self
            .focused_window
            .write()
            .unwrap_or_else(|e| e.into_inner()) = window_id;

        // Record focus change
        let focus_change = FocusChange {
            old_focus,
            new_focus: window_id,
            timestamp: Instant::now(),
        };
        self.focus_history
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .push(focus_change);

        // Send focus messages
        if let Some(old_id) = old_focus {
            if let Some(old_window) = self.find_window(old_id) {
                old_window.send_message(WindowMessage::InputFocus, 0, 0);
            }
        }

        if let Some(new_id) = window_id {
            if let Some(new_window) = self.find_window(new_id) {
                new_window.send_message(WindowMessage::InputFocus, 1, 0);
            }
        }

        Ok(())
    }

    /// Navigate focus using tab key
    pub fn tab_to_next_window(&self, direction: TabDirection) -> Result<()> {
        let current_focus = *self
            .focused_window
            .read()
            .unwrap_or_else(|e| e.into_inner());

        // Get all focusable windows in tab order
        let focusable_windows = self.get_focusable_windows();
        if focusable_windows.is_empty() {
            return Ok(());
        }

        let current_index = if let Some(current_id) = current_focus {
            focusable_windows
                .iter()
                .position(|w| w.get_id() == current_id)
        } else {
            None
        };

        let next_index = match (current_index, direction) {
            (Some(idx), TabDirection::Forward) => (idx + 1) % focusable_windows.len(),
            (Some(idx), TabDirection::Backward) => {
                if idx == 0 {
                    focusable_windows.len() - 1
                } else {
                    idx - 1
                }
            }
            (None, TabDirection::Forward) => 0,
            (None, TabDirection::Backward) => focusable_windows.len() - 1,
        };

        if let Some(next_window) = focusable_windows.get(next_index) {
            self.set_focus(Some(next_window.get_id()))?;
        }

        Ok(())
    }

    fn get_focusable_windows(&self) -> Vec<Arc<EnhancedGameWindow>> {
        let mut focusable = Vec::new();
        let tab_list = self.tab_list.read().unwrap_or_else(|e| e.into_inner());
        if !tab_list.is_empty() {
            let windows = self.windows.read().unwrap_or_else(|e| e.into_inner());
            for id in tab_list.iter() {
                if let Some(window) = windows.get(id) {
                    let status = window.get_status();
                    if status.contains(WindowStatus::ENABLED)
                        && !status.contains(WindowStatus::HIDDEN)
                        && status.contains(WindowStatus::TAB_STOP)
                        && !status.contains(WindowStatus::NO_FOCUS)
                    {
                        focusable.push(window.clone());
                    }
                }
            }
            return focusable;
        }

        let root_windows = self.root_windows.read().unwrap_or_else(|e| e.into_inner());
        for root_window in root_windows.iter() {
            self.collect_focusable_recursive(root_window, &mut focusable);
        }
        focusable
    }

    fn collect_focusable_recursive(
        &self,
        window: &Arc<EnhancedGameWindow>,
        focusable: &mut Vec<Arc<EnhancedGameWindow>>,
    ) {
        let status = window.get_status();
        if status.contains(WindowStatus::ENABLED)
            && !status.contains(WindowStatus::HIDDEN)
            && status.contains(WindowStatus::TAB_STOP)
            && !status.contains(WindowStatus::NO_FOCUS)
        {
            focusable.push(window.clone());
        }

        for child in window.get_children() {
            self.collect_focusable_recursive(&child, focusable);
        }
    }

    /// Capture mouse input to a specific window
    pub fn capture_mouse(&self, window_id: WindowId) -> Result<()> {
        if self.find_window(window_id).is_none() {
            return Err(WindowManagerError::WindowNotFound(window_id));
        }

        *self
            .mouse_capture_window
            .write()
            .unwrap_or_else(|e| e.into_inner()) = Some(window_id);
        Ok(())
    }

    /// Release mouse capture
    pub fn release_mouse_capture(&self) {
        *self
            .mouse_capture_window
            .write()
            .unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Capture keyboard input to a specific window
    pub fn capture_keyboard(&self, window_id: WindowId) -> Result<()> {
        if self.find_window(window_id).is_none() {
            return Err(WindowManagerError::WindowNotFound(window_id));
        }

        *self
            .keyboard_capture_window
            .write()
            .unwrap_or_else(|e| e.into_inner()) = Some(window_id);
        Ok(())
    }

    /// Release keyboard capture
    pub fn release_keyboard_capture(&self) {
        *self
            .keyboard_capture_window
            .write()
            .unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Handle winit window events
    pub fn handle_window_event(&self, event: &WindowEvent) -> Result<bool> {
        match event {
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_button(*state, *button)
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.handle_mouse_move(position.x as f32, position.y as f32)
            }
            WindowEvent::MouseWheel { delta, .. } => self.handle_mouse_wheel(*delta),
            WindowEvent::KeyboardInput { event, .. } => {
                // For winit 0.29 compatibility, we need to extract key code and state
                if let Some(gui_key) = map_winit_keycode(&event.physical_key) {
                    self.handle_keyboard_input(&gui_key, event.state)
                } else {
                    Ok(false)
                }
            }
            WindowEvent::ReceivedCharacter(ch) => {
                // Forward text input to focused/captured window
                if ch.is_control() {
                    Ok(false)
                } else {
                    self.handle_character(*ch)
                }
            }
            _ => Ok(false),
        }
    }

    fn handle_mouse_button(&self, state: ElementState, button: MouseButton) -> Result<bool> {
        let (mouse_x, mouse_y) = *self
            .mouse_position
            .read()
            .unwrap_or_else(|e| e.into_inner());

        let window_message = match (button, state) {
            (MouseButton::Left, ElementState::Pressed) => WindowMessage::LeftDown,
            (MouseButton::Left, ElementState::Released) => WindowMessage::LeftUp,
            (MouseButton::Right, ElementState::Pressed) => WindowMessage::RightDown,
            (MouseButton::Right, ElementState::Released) => WindowMessage::RightUp,
            (MouseButton::Middle, ElementState::Pressed) => WindowMessage::MiddleDown,
            (MouseButton::Middle, ElementState::Released) => WindowMessage::MiddleUp,
            _ => return Ok(false),
        };

        let is_press = window_message == WindowMessage::LeftDown;
        let is_release = window_message == WindowMessage::LeftUp;

        // Check if mouse is captured
        if let Some(capture_id) = *self
            .mouse_capture_window
            .read()
            .unwrap_or_else(|e| e.into_inner())
        {
            if let Some(capture_window) = self.find_window(capture_id) {
                let (rel_x, rel_y) =
                    Self::coords_relative_to_parent(&capture_window, mouse_x, mouse_y);
                let handled = capture_window.handle_mouse_event(window_message, rel_x, rel_y);
                if is_release {
                    self.release_mouse_capture();
                }
                return Ok(handled == WindowMsgHandled::Handled);
            }
        }

        // Check modal windows first
        if let Some(modal) = self
            .modal_stack
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .last()
        {
            if let Some(modal_window) = self.find_window(modal.window_id) {
                let (rel_x, rel_y) =
                    Self::coords_relative_to_parent(&modal_window, mouse_x, mouse_y);
                let handled = modal_window.handle_mouse_event(window_message, rel_x, rel_y);
                if is_press {
                    let _ = self.capture_mouse(modal.window_id);
                }
                if is_release {
                    self.release_mouse_capture();
                }
                if handled == WindowMsgHandled::Handled || modal.close_on_click_outside {
                    return Ok(true);
                }
            }
        }

        // Hit test for normal windows
        let root_windows = self.root_windows.read().unwrap_or_else(|e| e.into_inner());
        for window in root_windows.iter().rev() {
            // Reverse for proper z-order
            if let Some(hit_window) = window.hit_test(mouse_x, mouse_y) {
                let (rel_x, rel_y) = Self::coords_relative_to_parent(&hit_window, mouse_x, mouse_y);
                let handled = hit_window.handle_mouse_event(window_message, rel_x, rel_y);
                if handled == WindowMsgHandled::Handled {
                    // Set focus on click if appropriate
                    if matches!(
                        window_message,
                        WindowMessage::LeftDown | WindowMessage::RightDown
                    ) {
                        if !hit_window.get_status().contains(WindowStatus::NO_FOCUS) {
                            let _ = self.set_focus(Some(hit_window.get_id()));
                        }
                    }
                    if is_press {
                        let _ = self.capture_mouse(hit_window.get_id());
                    }
                    if is_release {
                        self.release_mouse_capture();
                    }
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn handle_mouse_move(&self, x: f32, y: f32) -> Result<bool> {
        // Update mouse position
        *self
            .mouse_position
            .write()
            .unwrap_or_else(|e| e.into_inner()) = (x, y);

        if let Some(capture_id) = *self
            .mouse_capture_window
            .read()
            .unwrap_or_else(|e| e.into_inner())
        {
            if let Some(capture_window) = self.find_window(capture_id) {
                let (rel_x, rel_y) = Self::coords_relative_to_parent(&capture_window, x, y);
                capture_window.handle_mouse_event(WindowMessage::MousePos, rel_x, rel_y);
                self.update_mouse_hover_recursive(&capture_window, rel_x as f32, rel_y as f32);
                return Ok(true);
            }
        }

        // Handle mouse enter/leave events
        let root_windows = self.root_windows.read().unwrap_or_else(|e| e.into_inner());
        for window in root_windows.iter() {
            self.update_mouse_hover_recursive(window, x, y);
        }

        // Send mouse position update to the window under the cursor
        for window in root_windows.iter().rev() {
            if let Some(hit_window) = window.hit_test(x, y) {
                let (rel_x, rel_y) = Self::coords_relative_to_parent(&hit_window, x, y);
                hit_window.handle_mouse_event(WindowMessage::MousePos, rel_x, rel_y);
                break;
            }
        }

        Ok(true)
    }

    fn handle_mouse_wheel(&self, delta: MouseScrollDelta) -> Result<bool> {
        let (_, scroll_y) = match delta {
            MouseScrollDelta::LineDelta(_, y) => (0.0, y),
            MouseScrollDelta::PixelDelta(pos) => (0.0, pos.y as f32),
        };

        if scroll_y.abs() < f32::EPSILON {
            return Ok(false);
        }

        let message = if scroll_y > 0.0 {
            WindowMessage::WheelUp
        } else {
            WindowMessage::WheelDown
        };

        let (mouse_x, mouse_y) = *self
            .mouse_position
            .read()
            .unwrap_or_else(|e| e.into_inner());

        if let Some(capture_id) = *self
            .mouse_capture_window
            .read()
            .unwrap_or_else(|e| e.into_inner())
        {
            if let Some(capture_window) = self.find_window(capture_id) {
                let (rel_x, rel_y) =
                    Self::coords_relative_to_parent(&capture_window, mouse_x, mouse_y);
                let handled = capture_window.handle_mouse_event(message, rel_x, rel_y);
                return Ok(handled == WindowMsgHandled::Handled);
            }
        }

        let root_windows = self.root_windows.read().unwrap_or_else(|e| e.into_inner());
        for window in root_windows.iter().rev() {
            if let Some(hit_window) = window.hit_test(mouse_x, mouse_y) {
                let (rel_x, rel_y) = Self::coords_relative_to_parent(&hit_window, mouse_x, mouse_y);
                let handled = hit_window.handle_mouse_event(message, rel_x, rel_y);
                return Ok(handled == WindowMsgHandled::Handled);
            }
        }

        Ok(false)
    }

    fn coords_relative_to_parent(
        window: &Arc<EnhancedGameWindow>,
        screen_x: f32,
        screen_y: f32,
    ) -> (i32, i32) {
        if let Some(parent) = window.get_parent() {
            let (px, py) = parent.get_screen_position();
            (screen_x as i32 - px, screen_y as i32 - py)
        } else {
            (screen_x as i32, screen_y as i32)
        }
    }

    fn update_mouse_hover_recursive(
        &self,
        window: &Arc<EnhancedGameWindow>,
        mouse_x: f32,
        mouse_y: f32,
    ) {
        if window.is_hidden() {
            return;
        }

        let bounds = window.get_bounds();
        let is_over = bounds.contains(mouse_x, mouse_y);
        if (window.get_style() & GWS_MOUSE_TRACK) != 0 {
            let was_over = window.is_mouse_over();
            if is_over && !was_over {
                let (rel_x, rel_y) = Self::coords_relative_to_parent(window, mouse_x, mouse_y);
                window.handle_mouse_event(WindowMessage::MouseEntering, rel_x, rel_y);
            } else if !is_over && was_over {
                let (rel_x, rel_y) = Self::coords_relative_to_parent(window, mouse_x, mouse_y);
                window.handle_mouse_event(WindowMessage::MouseLeaving, rel_x, rel_y);
            }
        }

        // Check children
        for child in window.get_children() {
            self.update_mouse_hover_recursive(&child, mouse_x - bounds.x, mouse_y - bounds.y);
        }
    }

    fn handle_keyboard_input(&self, key_code: &GuiKeyCode, state: ElementState) -> Result<bool> {
        // Handle tab navigation
        if let KeyCode::Tab = key_code {
            if state == ElementState::Pressed {
                let direction = TabDirection::Forward; // Simplified for now
                self.tab_to_next_window(direction)?;
                return Ok(true);
            }
        }

        // Send to focused or captured window
        let target_window = if let Some(capture_id) = *self
            .keyboard_capture_window
            .read()
            .unwrap_or_else(|e| e.into_inner())
        {
            self.find_window(capture_id)
        } else {
            self.get_focused_window()
        };

        if let Some(window) = target_window {
            let encoded = encode_keycode(key_code);
            let handled = window.send_message(WindowMessage::Char, encoded, 0);
            return Ok(handled == WindowMsgHandled::Handled);
        }

        Ok(false)
    }

    fn handle_character(&self, ch: char) -> Result<bool> {
        let target_window = if let Some(capture_id) = *self
            .keyboard_capture_window
            .read()
            .unwrap_or_else(|e| e.into_inner())
        {
            self.find_window(capture_id)
        } else {
            self.get_focused_window()
        };

        if let Some(window) = target_window {
            let handled = window.send_message(WindowMessage::Char, ch as u32, 0);
            return Ok(handled == WindowMsgHandled::Handled);
        }

        Ok(false)
    }

    /// Show a modal window
    pub fn show_modal(
        &self,
        window_id: WindowId,
        background_color: [f32; 4],
        close_on_click_outside: bool,
    ) -> Result<()> {
        if self.find_window(window_id).is_none() {
            return Err(WindowManagerError::WindowNotFound(window_id));
        }

        let modal = ModalWindow {
            window_id,
            background_color,
            close_on_click_outside,
        };

        self.modal_stack
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .push(modal);
        Ok(())
    }

    /// Close the current modal window
    pub fn close_modal(&self) -> Option<WindowId> {
        self.modal_stack
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .pop()
            .map(|modal| modal.window_id)
    }

    /// Update all windows (call once per frame)
    pub fn update(&self) -> Result<()> {
        let now = Instant::now();
        let last = *self
            .last_update_time
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let delta_time = now.duration_since(last).as_secs_f32();
        *self
            .last_update_time
            .write()
            .unwrap_or_else(|e| e.into_inner()) = now;
        self.frame_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Process event queue
        self.process_event_queue()?;

        // Update press animations for elastic feel
        self.update_press_animations(delta_time);

        // Update all windows
        let root_windows = self.root_windows.read().unwrap_or_else(|e| e.into_inner());
        for window in root_windows.iter() {
            // Windows would have their own update logic here
        }

        Ok(())
    }

    fn update_press_animations(&self, delta_time: f32) {
        let root_windows = self.root_windows.read().unwrap_or_else(|e| e.into_inner());
        for window in root_windows.iter() {
            Self::update_press_animation_recursive(window, delta_time);
        }
    }

    fn update_press_animation_recursive(window: &Arc<EnhancedGameWindow>, delta_time: f32) {
        window.update_press_animation(delta_time);
        for child in window.get_children() {
            Self::update_press_animation_recursive(&child, delta_time);
        }
    }

    fn process_event_queue(&self) -> Result<()> {
        let mut queue = self.event_queue.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(event) = queue.pop_front() {
            if let Some(window) = self.find_window(event.window_id) {
                window.send_message(event.message, event.wparam, event.lparam);
            }
        }
        Ok(())
    }

    /// Render all windows
    pub fn render(&self) -> Result<()> {
        let renderer_opt = self.renderer.read().unwrap_or_else(|e| e.into_inner());
        let renderer = renderer_opt
            .as_ref()
            .ok_or(WindowManagerError::InvalidOperation(
                "Renderer not initialized".to_string(),
            ))?;

        let mut renderer = renderer.write().unwrap_or_else(|e| e.into_inner());
        renderer.begin_frame();

        // Render modal background if needed
        if let Some(modal) = self
            .modal_stack
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .last()
        {
            // Render semi-transparent background
            let screen_size = (800.0, 600.0); // Would get from renderer
            let full_screen =
                super::ui_renderer::UIRect::new(0.0, 0.0, screen_size.0, screen_size.1);
            renderer.draw_rect(full_screen, modal.background_color, 0.5);
        }

        // Render all root windows
        let root_windows = self.root_windows.read().unwrap_or_else(|e| e.into_inner());
        for window in root_windows.iter() {
            window.render(&mut *renderer, (0.0, 0.0))?;
        }

        // Render simple radar overlay in the bottom-right corner using normalized pings.
        let dots = self
            .radar_overlay
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if !dots.is_empty() {
            let (w, h) = renderer.screen_size();
            let overlay_size = 180.0;
            let padding = 12.0;
            let origin_x = w as f32 - overlay_size - padding;
            let origin_y = h as f32 - overlay_size - padding;
            let frame_rect =
                super::ui_renderer::UIRect::new(origin_x, origin_y, overlay_size, overlay_size);
            renderer.draw_rect(frame_rect, [0.05, 0.08, 0.10, 0.6], 0.8);

            for (nx, nz, age, kind) in dots {
                let px = (origin_x + nx.clamp(0.0, 1.0) * overlay_size)
                    .clamp(origin_x, origin_x + overlay_size);
                let py = (origin_y + nz.clamp(0.0, 1.0) * overlay_size)
                    .clamp(origin_y, origin_y + overlay_size);
                let fade = (1.0 - (age / 6.0)).clamp(0.0, 1.0);
                let color = match kind {
                    RadarPingKind::Attack => [1.0, 0.3, 0.3, fade],
                    RadarPingKind::Ally => [0.3, 0.7, 1.0, fade],
                    RadarPingKind::Generic => [1.0, 1.0, 1.0, fade],
                };
                let dot_rect = super::ui_renderer::UIRect::new(px - 3.0, py - 3.0, 6.0, 6.0);
                renderer.draw_rect(dot_rect, color, 0.9);
            }
        }

        renderer.end_frame();
        Ok(())
    }

    /// Get window count
    pub fn get_window_count(&self) -> usize {
        self.windows.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Get root window count
    pub fn get_root_window_count(&self) -> usize {
        self.root_windows
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// Queue an event for processing
    pub fn queue_event(
        &self,
        window_id: WindowId,
        message: WindowMessage,
        wparam: u32,
        lparam: u32,
    ) {
        let event = QueuedEvent {
            window_id,
            message,
            wparam,
            lparam,
            timestamp: Instant::now(),
        };

        self.event_queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(event);
    }

    fn get_screen_size(&self) -> (i32, i32) {
        if let Some(renderer) = self
            .renderer
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            let (w, h) = renderer
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .screen_size();
            (w as i32, h as i32)
        } else {
            (800, 600)
        }
    }

    fn create_window_from_definition(
        &self,
        window_def: &WindowDefinition,
        parent: Option<&Arc<EnhancedGameWindow>>,
        layout: &WindowLayoutDefinition,
        screen_size: (i32, i32),
    ) -> Result<Arc<EnhancedGameWindow>> {
        let (x, y, width, height) = resolve_window_rect(window_def, parent, screen_size);
        let window = if window_def.name.is_empty() {
            self.create_window(parent, &window_def.name, x, y, width, height)?
        } else {
            let window_id = NameKeyGenerator::name_to_key(&window_def.name) as WindowId;
            self.create_window_with_id(parent, &window_def.name, x, y, width, height, window_id)?
        };

        let mut resolved_tooltip: Option<String> = None;

        if let Some(data) = window_def.listbox_data.as_ref() {
            window.set_user_data("listbox_data", data.clone());
        }
        if let Some(data) = window_def.text_entry_data.as_ref() {
            window.set_user_data("text_entry_data", data.clone());
        }
        if let Some(data) = window_def.combo_box_data.as_ref() {
            window.set_user_data("combo_box_data", data.clone());
        }
        if let Some(data) = window_def.tab_control_data.as_ref() {
            window.set_user_data("tab_control_data", data.clone());
        }
        if let Some(data) = window_def.slider_data.as_ref() {
            window.set_user_data("slider_data", data.clone());
        }
        if let Some(data) = window_def.radio_button_data.as_ref() {
            window.set_user_data("radio_button_data", data.clone());
        }
        if let Some(data) = window_def.static_text_data.as_ref() {
            window.set_user_data("static_text_data", data.clone());
        }

        let combined_style = window_def.style | style_for_window_type(&window_def.window_type);
        window.set_window_type(&window_def.window_type);
        window.set_style(combined_style);

        let mut status = WindowStatus::from_bits_retain(window_def.status.bits());
        if !status.contains(WindowStatus::ENABLED) {
            status.insert(WindowStatus::ENABLED);
        }
        if (combined_style & GWS_TAB_STOP) != 0 {
            status.insert(WindowStatus::TAB_STOP);
        }
        window.set_status(status);
        if status.contains(WindowStatus::TAB_STOP) {
            let mut tab_list = self.tab_list.write().unwrap_or_else(|e| e.into_inner());
            if !tab_list.contains(&window.get_id()) {
                tab_list.push(window.get_id());
            }
        }
        if let Some(data) = window_def.static_text_data.as_ref() {
            if data.centered {
                let mut updated = window.get_status();
                updated.insert(WindowStatus::WRAP_CENTERED);
                window.set_status(updated);
            }
        }

        if !window_def.draw_callback.is_empty() && window_def.draw_callback != "[None]" {
            let mut updated = window.get_status();
            if window_def.draw_callback.contains("ImageDraw") {
                updated.insert(WindowStatus::IMAGE);
            }
            window.set_status(updated);
        }

        if let Some(text) = resolve_window_text(window_def) {
            window.set_text(&text);
        }

        if let Some(font) = &window_def.font {
            window.set_font(&font.name, font.size);
        } else if let Some(default_font) = &layout.default_font {
            window.set_font(&default_font.name, default_font.size);
        }

        if !window_def.header_template.is_empty() {
            if let Some(font) =
                get_header_template_manager().get_font_from_template(&window_def.header_template)
            {
                window.set_font(&font.name, font.size);
            }
        }

        if !window_def.tooltip.is_empty() && window_def.tooltip_delay >= 0 {
            let tooltip = GameText::fetch(&window_def.tooltip);
            window.set_tooltip(&tooltip, window_def.tooltip_delay as u32);
            resolved_tooltip = Some(tooltip);
        }

        if let Some(widget) = create_widget_for_style(
            &mut self.radio_groups.write().unwrap_or_else(|e| e.into_inner()),
            window_def,
            window.get_id(),
            x,
            y,
            width,
            height,
        ) {
            window.set_widget(widget);
        }

        let enabled = pick_first_image(&window_def.enabled_draw_data);
        let disabled = pick_first_image(&window_def.disabled_draw_data);
        let hilited = pick_first_image(&window_def.hilite_draw_data);
        let pushed = hilited.clone().or(enabled.clone());

        let (enabled_color, enabled_border) = pick_first_colors(&window_def.enabled_draw_data);
        let (mut disabled_color, mut disabled_border) =
            pick_first_colors(&window_def.disabled_draw_data);
        let (mut hilited_color, mut hilited_border) =
            pick_first_colors(&window_def.hilite_draw_data);

        if disabled_color[3] == 0.0 && disabled_border[3] == 0.0 {
            disabled_color = enabled_color;
            disabled_border = enabled_border;
        }
        if hilited_color[3] == 0.0 && hilited_border[3] == 0.0 {
            hilited_color = enabled_color;
            hilited_border = enabled_border;
        }

        let (pushed_color, pushed_border) = if hilited_color[3] > 0.0 || hilited_border[3] > 0.0 {
            (hilited_color, hilited_border)
        } else {
            (enabled_color, enabled_border)
        };

        window.set_draw_data(
            enabled,
            disabled,
            hilited,
            pushed,
            enabled_color,
            enabled_border,
            disabled_color,
            disabled_border,
            hilited_color,
            hilited_border,
            pushed_color,
            pushed_border,
        );

        let mut enabled_color = window_def.enabled_text.color;
        let mut disabled_color = window_def.disabled_text.color;
        let mut hilited_color = window_def.hilite_text.color;
        let mut enabled_border = window_def.enabled_text.border_color;
        let mut disabled_border = window_def.disabled_text.border_color;
        let mut hilited_border = window_def.hilite_text.border_color;
        if enabled_color == 0 && disabled_color == 0 && hilited_color == 0 {
            if let Some(default_color) = layout.default_text_color {
                enabled_color = default_color;
                disabled_color = default_color;
                hilited_color = default_color;
                enabled_border = default_color;
                disabled_border = default_color;
                hilited_border = default_color;
            }
        }
        let enabled_color = color_to_rgba(enabled_color);
        let disabled_color = color_to_rgba(disabled_color);
        let hilited_color = color_to_rgba(hilited_color);
        let enabled_border = color_to_rgba(enabled_border);
        let disabled_border = color_to_rgba(disabled_border);
        let hilited_border = color_to_rgba(hilited_border);
        window.set_text_colors(
            enabled_color,
            disabled_color,
            hilited_color,
            hilited_color,
            enabled_border,
            disabled_border,
            hilited_border,
            hilited_border,
        );

        apply_window_status_to_widget(&window);
        apply_window_widget_data(&window, window_def);
        if let Some(tooltip) = resolved_tooltip.as_ref() {
            apply_window_tooltip(&window, tooltip);
        }

        window.send_message(WindowMessage::ScriptCreate, 0, 0);

        let has_tab_pane_child = window_def.children.iter().any(|child| {
            (child.style | style_for_window_type(&child.window_type)) & GWS_TAB_PANE != 0
        });

        for child in &window_def.children {
            let _ =
                self.create_window_from_definition(child, Some(&window), layout, screen_size)?;
        }

        if (window.get_style() & GWS_TAB_CONTROL) != 0 {
            if !has_tab_pane_child {
                let _ = self.create_default_tab_panes(&window)?;
            }
            self.resize_tab_panes(&window);
            let active_index = window
                .with_widget_mut(|widget| {
                    if let WindowWidget::TabControl(tab_control) = widget {
                        Some(tab_control.active_tab_index())
                    } else {
                        None
                    }
                })
                .flatten()
                .unwrap_or(0);
            window.show_tab_pane(active_index);
        }

        if (window.get_style() & GWS_ALL_SLIDER) != 0 {
            let _ = self.create_slider_thumb_child(&window, layout)?;
        }

        if (window.get_style() & GWS_COMBO_BOX) != 0 {
            let _ = self.create_combo_box_children(&window, layout, window_def)?;
        }

        if (window.get_style() & GWS_SCROLL_LISTBOX) != 0 {
            if let Some(listbox_data) = window_def.listbox_data.as_ref() {
                if listbox_data.scrollbar {
                    let _ = self.create_listbox_scrollbar_children(&window, layout)?;
                }
            }
        }

        Ok(window)
    }

    fn create_default_tab_panes(&self, window: &Arc<EnhancedGameWindow>) -> Result<()> {
        let (pane_x, pane_y, pane_width, pane_height) = self.compute_tab_pane_rect(window);

        for pane_index in 0..crate::gui::gadgets::tabcontrol::NUM_TAB_PANES {
            let pane =
                self.create_window(Some(window), "", pane_x, pane_y, pane_width, pane_height)?;
            let mut status = pane.get_status();
            status.insert(WindowStatus::ENABLED);
            pane.set_status(status);
            pane.set_style(pane.get_style() | GWS_TAB_PANE);
            pane.set_widget(WindowWidget::TabPane);
            pane.set_text(&format!("Pane {}", pane_index));
        }

        Ok(())
    }

    fn resize_tab_panes(&self, window: &Arc<EnhancedGameWindow>) {
        let (pane_x, pane_y, pane_width, pane_height) = self.compute_tab_pane_rect(window);
        let panes: Vec<Arc<EnhancedGameWindow>> = window
            .get_children()
            .into_iter()
            .filter(|child| (child.get_style() & GWS_TAB_PANE) != 0)
            .collect();

        for pane in panes {
            pane.set_size(pane_width, pane_height);
            pane.set_position(pane_x, pane_y);
        }
    }

    fn compute_tab_pane_rect(&self, window: &Arc<EnhancedGameWindow>) -> (i32, i32, i32, i32) {
        let (win_width, win_height) = window.get_size();
        let (win_width, win_height) = (win_width as i32, win_height as i32);
        let mut tab_edge = crate::gui::gadgets::tabcontrol::TP_TOP_SIDE;
        let mut tab_width = 0;
        let mut tab_height = 0;
        let mut pane_border = 0;

        let _ = window.with_widget_mut(|widget| {
            if let WindowWidget::TabControl(tab_control) = widget {
                tab_edge = tab_control.tab_edge();
                tab_width = tab_control.tab_width_px();
                tab_height = tab_control.tab_height_px();
                pane_border = tab_control.pane_border();
            }
        });

        let mut width = win_width - (2 * pane_border);
        let mut height = win_height - (2 * pane_border);

        if tab_edge == crate::gui::gadgets::tabcontrol::TP_TOP_SIDE
            || tab_edge == crate::gui::gadgets::tabcontrol::TP_BOTTOM_SIDE
        {
            height -= tab_height;
        }
        if tab_edge == crate::gui::gadgets::tabcontrol::TP_LEFT_SIDE
            || tab_edge == crate::gui::gadgets::tabcontrol::TP_RIGHT_SIDE
        {
            width -= tab_width;
        }

        let mut x = pane_border;
        let mut y = pane_border;
        if tab_edge == crate::gui::gadgets::tabcontrol::TP_LEFT_SIDE {
            x += tab_width;
        }
        if tab_edge == crate::gui::gadgets::tabcontrol::TP_TOP_SIDE {
            y += tab_height;
        }

        (x, y, width.max(0), height.max(0))
    }

    fn create_combo_box_children(
        &self,
        window: &Arc<EnhancedGameWindow>,
        layout: &WindowLayoutDefinition,
        window_def: &WindowDefinition,
    ) -> Result<()> {
        let (width, height) = window.get_size();
        let mut status = window.get_status();
        status.remove(WindowStatus::BORDER);
        status.remove(WindowStatus::HIDDEN);
        let is_editable = window_def
            .combo_box_data
            .as_ref()
            .map(|data| data.is_editable)
            .unwrap_or(false);

        let button_width = 21;
        let button_height = height as i32;

        let drop_down = self.create_window(
            Some(window),
            "",
            (width as i32 - button_width).max(0),
            0,
            button_width,
            button_height,
        )?;
        {
            drop_down.set_style(drop_down.get_style() | GWS_PUSH_BUTTON);
            drop_down.set_widget(WindowWidget::PushButton(PushButton::new(
                drop_down.get_id() as u32,
                0,
                0,
                button_width as u32,
                height as u32,
            )));
            drop_down.set_status(status | WindowStatus::ACTIVE | WindowStatus::ENABLED);
            if !window.get_font_name().is_empty() {
                drop_down.set_font(&window.get_font_name(), window.get_font_size());
            }
            let tooltip = window.get_tooltip();
            if !tooltip.is_empty() {
                drop_down.set_tooltip(&tooltip, window.get_tooltip_delay());
            }
            apply_draw_data_set(
                &drop_down,
                &layout.combo_dropdown_enabled_draw_data,
                &layout.combo_dropdown_disabled_draw_data,
                &layout.combo_dropdown_hilite_draw_data,
            );
        }

        let edit_width = (width as i32 - button_width).max(0);
        let edit = self.create_window(Some(window), "", 0, 0, edit_width, height as i32)?;
        {
            edit.set_style(edit.get_style() | GWS_ENTRY_FIELD);
            edit.set_widget(WindowWidget::TextEntry(TextEntry::new(
                edit.get_id() as u32,
                0,
                0,
                edit_width as u32,
                height as u32,
            )));
            let mut edit_status = status;
            if !is_editable {
                edit_status.insert(WindowStatus::NO_INPUT);
            }
            edit.set_status(edit_status);
            if !window.get_font_name().is_empty() {
                edit.set_font(&window.get_font_name(), window.get_font_size());
            }
            let tooltip = window.get_tooltip();
            if !tooltip.is_empty() {
                edit.set_tooltip(&tooltip, window.get_tooltip_delay());
            }
            if let Some(data) = window_def.combo_box_data.as_ref() {
                let _ = edit.with_widget_mut(|widget| {
                    if let WindowWidget::TextEntry(entry) = widget {
                        let validation = if data.ascii_only {
                            ValidationMode::AsciiOnly
                        } else if data.letters_and_numbers {
                            ValidationMode::AlphanumericOnly
                        } else {
                            ValidationMode::None
                        };
                        entry.set_validation(validation);
                        if data.max_chars > 0 {
                            entry.set_max_length(data.max_chars);
                        }
                    }
                });
            }
            apply_draw_data_set(
                &edit,
                &layout.combo_edit_enabled_draw_data,
                &layout.combo_edit_disabled_draw_data,
                &layout.combo_edit_hilite_draw_data,
            );
        }

        let list = self.create_window(
            Some(window),
            "",
            0,
            height as i32,
            width as i32,
            height as i32,
        )?;
        {
            list.set_style(list.get_style() | GWS_SCROLL_LISTBOX);
            list.set_widget(WindowWidget::ListBox(ListBox::new(
                list.get_id() as u32,
                0,
                height as i32,
                width as u32,
                height as u32,
            )));
            let mut list_status = status;
            list_status.remove(WindowStatus::IMAGE);
            list.set_status(list_status | WindowStatus::ABOVE | WindowStatus::ONE_LINE);
            list.hide(true);
            if !window.get_font_name().is_empty() {
                list.set_font(&window.get_font_name(), window.get_font_size());
            }
            let tooltip = window.get_tooltip();
            if !tooltip.is_empty() {
                list.set_tooltip(&tooltip, window.get_tooltip_delay());
            }
            let _ = list.with_widget_mut(|widget| {
                if let WindowWidget::ListBox(listbox) = widget {
                    listbox.set_max_length(10);
                    listbox.set_auto_purge(false);
                    listbox.set_auto_scroll(false);
                    listbox.set_scroll_if_at_end(false);
                    listbox.set_force_select(true);
                    listbox.set_selection_mode(SelectionMode::Single);
                    listbox.set_columns(1);
                    listbox.set_audio_feedback(true);
                }
            });
            apply_draw_data_set(
                &list,
                &layout.combo_list_enabled_draw_data,
                &layout.combo_list_disabled_draw_data,
                &layout.combo_list_hilite_draw_data,
            );
        }

        self.create_listbox_scrollbar_children(&list, layout)?;

        window.set_combobox_links(ComboBoxLinks {
            drop_down: drop_down.get_id(),
            edit_box: edit.get_id(),
            list_box: list.get_id(),
        });

        Ok(())
    }

    fn create_slider_thumb_child(
        &self,
        slider: &Arc<EnhancedGameWindow>,
        layout: &WindowLayoutDefinition,
    ) -> Result<()> {
        if layout.slider_thumb_enabled_draw_data.is_empty()
            && layout.slider_thumb_disabled_draw_data.is_empty()
            && layout.slider_thumb_hilite_draw_data.is_empty()
        {
            return Ok(());
        }

        let (width, _height) = slider.get_size();
        let is_horizontal = (slider.get_style() & GWS_HORZ_SLIDER) != 0;
        let (thumb_w, thumb_h) = if is_horizontal { (13, 16) } else { (width, 16) };
        let thumb_y = if is_horizontal { 10 } else { 0 };

        let mut status = slider.get_status();
        status.remove(WindowStatus::BORDER);
        status.remove(WindowStatus::HIDDEN);
        status.insert(WindowStatus::ACTIVE);
        status.insert(WindowStatus::ENABLED);
        status.insert(WindowStatus::NO_INPUT);

        let thumb = self.create_window(Some(slider), "", 0, thumb_y, thumb_w, thumb_h)?;
        {
            thumb.set_style(thumb.get_style() | GWS_PUSH_BUTTON);
            thumb.set_status(status);
            thumb.set_widget(WindowWidget::PushButton(PushButton::new(
                thumb.get_id() as u32,
                0,
                0,
                thumb_w as u32,
                thumb_h as u32,
            )));
            apply_draw_data_set(
                &thumb,
                &layout.slider_thumb_enabled_draw_data,
                &layout.slider_thumb_disabled_draw_data,
                &layout.slider_thumb_hilite_draw_data,
            );
        }

        slider.set_slider_thumb(thumb.get_id());
        slider.update_slider_thumb();

        Ok(())
    }

    fn create_listbox_scrollbar_children(
        &self,
        listbox: &Arc<EnhancedGameWindow>,
        layout: &WindowLayoutDefinition,
    ) -> Result<()> {
        let (width, height) = listbox.get_size();
        let button_width = 21;
        let button_height = 22;
        let has_title = !listbox.get_text().is_empty();
        let font_height = if has_title {
            listbox.get_font_size().max(12)
        } else {
            0
        };
        let top = if has_title { font_height + 1 } else { 0 };
        let bottom = if has_title {
            height - (font_height + 1)
        } else {
            height
        };

        let mut status = listbox.get_status();
        status.remove(WindowStatus::BORDER);
        status.remove(WindowStatus::HIDDEN);
        status.remove(WindowStatus::NO_INPUT);
        status.insert(WindowStatus::ACTIVE);
        status.insert(WindowStatus::ENABLED);

        let up_button = self.create_window(
            Some(listbox),
            "",
            width - button_width - 2,
            top + 2,
            button_width,
            button_height,
        )?;
        {
            up_button.set_style(up_button.get_style() | GWS_PUSH_BUTTON);
            up_button.set_status(status);
            let mut button = PushButton::new(
                up_button.get_id() as u32,
                0,
                0,
                button_width as u32,
                button_height as u32,
            );
            button.set_triggers_on_mouse_down(true);
            up_button.set_widget(WindowWidget::PushButton(button));
            apply_draw_data_set(
                &up_button,
                &layout.listbox_enabled_up_button_draw_data,
                &layout.listbox_disabled_up_button_draw_data,
                &layout.listbox_hilite_up_button_draw_data,
            );
        }

        let down_button = self.create_window(
            Some(listbox),
            "",
            width - button_width - 2,
            top + bottom - button_height - 2,
            button_width,
            button_height,
        )?;
        {
            down_button.set_style(down_button.get_style() | GWS_PUSH_BUTTON);
            down_button.set_status(status);
            let mut button = PushButton::new(
                down_button.get_id() as u32,
                0,
                0,
                button_width as u32,
                button_height as u32,
            );
            button.set_triggers_on_mouse_down(true);
            down_button.set_widget(WindowWidget::PushButton(button));
            apply_draw_data_set(
                &down_button,
                &layout.listbox_enabled_down_button_draw_data,
                &layout.listbox_disabled_down_button_draw_data,
                &layout.listbox_hilite_down_button_draw_data,
            );
        }

        let slider_height = (bottom - (2 * button_height) - 6).max(0);
        let slider = self.create_window(
            Some(listbox),
            "",
            width - button_width - 2,
            top + button_height + 3,
            button_width,
            slider_height,
        )?;
        {
            slider.set_style(slider.get_style() | GWS_VERT_SLIDER);
            slider.set_status(status);
            slider.set_widget(WindowWidget::VerticalSlider(VerticalSlider::new(
                slider.get_id() as u32,
                0,
                0,
                button_width as u32,
                slider_height as u32,
            )));
            apply_draw_data_set(
                &slider,
                &layout.listbox_enabled_slider_draw_data,
                &layout.listbox_disabled_slider_draw_data,
                &layout.listbox_hilite_slider_draw_data,
            );
        }

        let mut thumb_id = None;
        if !layout.slider_thumb_enabled_draw_data.is_empty()
            || !layout.slider_thumb_disabled_draw_data.is_empty()
            || !layout.slider_thumb_hilite_draw_data.is_empty()
        {
            let thumb = self.create_window(Some(&slider), "", 0, 0, button_width, 16)?;
            {
                thumb.set_style(thumb.get_style() | GWS_PUSH_BUTTON);
                thumb.set_status(status);
                thumb.set_widget(WindowWidget::PushButton(PushButton::new(
                    thumb.get_id() as u32,
                    0,
                    0,
                    button_width as u32,
                    16,
                )));
                apply_draw_data_set(
                    &thumb,
                    &layout.slider_thumb_enabled_draw_data,
                    &layout.slider_thumb_disabled_draw_data,
                    &layout.slider_thumb_hilite_draw_data,
                );
            }
            thumb_id = Some(thumb.get_id());
        }

        listbox.set_listbox_links(ListBoxLinks {
            up_button: up_button.get_id(),
            down_button: down_button.get_id(),
            slider: slider.get_id(),
            thumb: thumb_id,
        });
        listbox.update_listbox_scrollbar();

        Ok(())
    }
}

fn resolve_window_script_path(filename: &str) -> Result<PathBuf, ()> {
    let candidates = [
        Path::new("windows_game/extracted_big_files_v2/WindowZH/Window").join(filename),
        Path::new("windows_game/extracted_big_files_v2/WindowZH/Window/Menus").join(filename),
        Path::new("windows_game/extracted_big_files/WindowZH/Window").join(filename),
        Path::new("windows_game/extracted_big_files/WindowZH/Window/Menus").join(filename),
        Path::new(filename).to_path_buf(),
    ];
    for path in candidates {
        if path.exists() {
            return Ok(path);
        }
    }
    Err(())
}

fn resolve_window_rect(
    window_def: &WindowDefinition,
    parent: Option<&Arc<EnhancedGameWindow>>,
    screen_size: (i32, i32),
) -> (i32, i32, i32, i32) {
    if let Some((x1, y1, x2, y2)) = window_def.raw_screen_rect {
        let (screen_w, screen_h) = screen_size;
        let (create_w, create_h) = window_def
            .creation_resolution
            .unwrap_or((screen_w.max(1), screen_h.max(1)));
        let x_scale = screen_w as f32 / create_w.max(1) as f32;
        let y_scale = screen_h as f32 / create_h.max(1) as f32;
        let scaled_x1 = (x1 as f32 * x_scale).round() as i32;
        let scaled_y1 = (y1 as f32 * y_scale).round() as i32;
        let scaled_x2 = (x2 as f32 * x_scale).round() as i32;
        let scaled_y2 = (y2 as f32 * y_scale).round() as i32;
        let (mut rel_x, mut rel_y) = (scaled_x1, scaled_y1);
        if let Some(parent_window) = parent {
            let (parent_x, parent_y) = parent_window.get_screen_position();
            rel_x -= parent_x;
            rel_y -= parent_y;
        }
        let width = scaled_x2 - scaled_x1;
        let height = scaled_y2 - scaled_y1;
        return (rel_x, rel_y, width, height);
    }

    let (x, y) = window_def.position;
    let (width, height) = window_def.size;
    (x, y, width, height)
}

fn resolve_window_text(window_def: &WindowDefinition) -> Option<String> {
    if !window_def.text_label.is_empty() {
        return Some(GameText::fetch(&window_def.text_label));
    }
    if !window_def.text.is_empty() {
        if window_def.text.contains(':') && !window_def.text.contains(' ') {
            return Some(GameText::fetch(&window_def.text));
        }
        return Some(window_def.text.clone());
    }
    None
}

fn style_for_window_type(window_type: &str) -> u32 {
    match window_type.trim().to_ascii_uppercase().as_str() {
        "PUSHBUTTON" => GWS_PUSH_BUTTON,
        "RADIOBUTTON" => GWS_RADIO_BUTTON,
        "CHECKBOX" => GWS_CHECK_BOX,
        "VERTSLIDER" => GWS_VERT_SLIDER,
        "HORZSLIDER" => GWS_HORZ_SLIDER,
        "SCROLLLISTBOX" => GWS_SCROLL_LISTBOX,
        "ENTRYFIELD" => GWS_ENTRY_FIELD,
        "STATICTEXT" => GWS_STATIC_TEXT,
        "PROGRESSBAR" => GWS_PROGRESS_BAR,
        "USER" => GWS_USER_WINDOW,
        "TABCONTROL" => GWS_TAB_CONTROL,
        "TABPANE" => GWS_TAB_PANE,
        "COMBOBOX" => GWS_COMBO_BOX,
        _ => 0,
    }
}

fn apply_draw_data_set(
    window: &Arc<EnhancedGameWindow>,
    enabled: &[LegacyDrawData],
    disabled: &[LegacyDrawData],
    hilite: &[LegacyDrawData],
) {
    let enabled_img = pick_first_image(enabled);
    let disabled_img = pick_first_image(disabled);
    let hilited_img = pick_first_image(hilite);
    let pushed_img = hilited_img.clone().or_else(|| enabled_img.clone());

    let (enabled_color, enabled_border) = pick_first_colors(enabled);
    let (mut disabled_color, mut disabled_border) = pick_first_colors(disabled);
    let (mut hilited_color, mut hilited_border) = pick_first_colors(hilite);

    if disabled_color[3] == 0.0 && disabled_border[3] == 0.0 {
        disabled_color = enabled_color;
        disabled_border = enabled_border;
    }
    if hilited_color[3] == 0.0 && hilited_border[3] == 0.0 {
        hilited_color = enabled_color;
        hilited_border = enabled_border;
    }

    let (pushed_color, pushed_border) = if hilited_color[3] > 0.0 || hilited_border[3] > 0.0 {
        (hilited_color, hilited_border)
    } else {
        (enabled_color, enabled_border)
    };

    window.set_draw_data(
        enabled_img,
        disabled_img,
        hilited_img,
        pushed_img,
        enabled_color,
        enabled_border,
        disabled_color,
        disabled_border,
        hilited_color,
        hilited_border,
        pushed_color,
        pushed_border,
    );

    if enabled_img.is_some() || disabled_img.is_some() || hilited_img.is_some() {
        let mut status = window.get_status();
        status.insert(WindowStatus::IMAGE);
        window.set_status(status);
    }
}

fn create_widget_for_style(
    radio_groups: &mut HashMap<u32, RadioButtonGroup>,
    window_def: &WindowDefinition,
    window_id: WindowId,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Option<WindowWidget> {
    let gadget_id = if window_id > 0 { window_id as u32 } else { 0 };
    let width_u = width.max(0) as u32;
    let height_u = height.max(0) as u32;
    let size = width.min(height).max(0) as u32;
    let text = if !window_def.text.is_empty() {
        window_def.text.clone()
    } else {
        window_def.text_label.clone()
    };

    let style = window_def.style | style_for_window_type(&window_def.window_type);
    if (style & GWS_PUSH_BUTTON) != 0 {
        let mut button = PushButton::new(gadget_id, x, y, width_u, height_u);
        if !text.is_empty() {
            button.set_text(text);
        }
        return Some(WindowWidget::PushButton(button));
    }
    if (style & GWS_RADIO_BUTTON) != 0 {
        let group_id = window_def
            .radio_button_data
            .as_ref()
            .map(|data| data.group)
            .unwrap_or(gadget_id);
        let group = radio_groups
            .entry(group_id)
            .or_insert_with(|| RadioButtonGroup::new(group_id))
            .clone();
        let mut radio = RadioButton::new(gadget_id, x, y, size, group);
        if !text.is_empty() {
            radio.set_label(text);
        }
        return Some(WindowWidget::RadioButton(radio));
    }
    if (style & GWS_CHECK_BOX) != 0 {
        return Some(WindowWidget::CheckBox(CheckBox::new(gadget_id, x, y, size)));
    }
    if (style & GWS_VERT_SLIDER) != 0 {
        return Some(WindowWidget::VerticalSlider(VerticalSlider::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if (style & GWS_HORZ_SLIDER) != 0 {
        return Some(WindowWidget::HorizontalSlider(HorizontalSlider::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if (style & GWS_SCROLL_LISTBOX) != 0 {
        return Some(WindowWidget::ListBox(ListBox::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if (style & GWS_ENTRY_FIELD) != 0 {
        let mut entry = TextEntry::new(gadget_id, x, y, width_u, height_u);
        if !text.is_empty() {
            entry.set_text(text);
        }
        return Some(WindowWidget::TextEntry(entry));
    }
    if (style & GWS_STATIC_TEXT) != 0 {
        let mut label = StaticText::new(gadget_id, x, y, width_u, height_u);
        if !text.is_empty() {
            label.set_text(text);
        }
        return Some(WindowWidget::StaticText(label));
    }
    if (style & GWS_PROGRESS_BAR) != 0 {
        return Some(WindowWidget::ProgressBar(ProgressBar::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if (style & GWS_USER_WINDOW) != 0 {
        return Some(WindowWidget::User);
    }
    if (style & GWS_MOUSE_TRACK) != 0 {
        return Some(WindowWidget::MouseTrack);
    }
    if (style & GWS_TAB_CONTROL) != 0 {
        return Some(WindowWidget::TabControl(TabControl::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if (style & GWS_TAB_PANE) != 0 {
        return Some(WindowWidget::TabPane);
    }
    if (style & GWS_COMBO_BOX) != 0 {
        return Some(WindowWidget::ComboBox(ComboBox::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }

    None
}

fn apply_window_tooltip(window: &Arc<EnhancedGameWindow>, tooltip: &str) {
    let _ = window.with_widget_mut(|widget| {
        if let WindowWidget::ListBox(listbox) = widget {
            listbox.set_tooltip(tooltip);
        }
    });
}

fn apply_window_status_to_widget(window: &Arc<EnhancedGameWindow>) {
    let status = window.get_status();
    let _ = window.with_widget_mut(|widget| {
        if let WindowWidget::PushButton(button) = widget {
            if status.contains(WindowStatus::CHECK_LIKE) {
                button.set_checkbox(true, false);
            }
            if status.contains(WindowStatus::ON_MOUSE_DOWN) {
                button.set_triggers_on_mouse_down(true);
            }
        }
    });
}

fn apply_window_widget_data(window: &Arc<EnhancedGameWindow>, window_def: &WindowDefinition) {
    let _ = window.with_widget_mut(|widget| match widget {
        WindowWidget::ListBox(listbox) => {
            if let Some(data) = window_def.listbox_data.as_ref() {
                if data.length > 0 {
                    listbox.set_max_length(data.length);
                }
                listbox.set_auto_purge(data.autopurge);
                listbox.set_auto_scroll(data.autoscroll);
                listbox.set_scroll_if_at_end(data.scroll_if_at_end);
                listbox.set_force_select(data.force_select);
                listbox.set_columns(data.columns);
                if !data.column_widths.is_empty() {
                    listbox.set_column_width_percentages(data.column_widths.clone());
                }
                if data.multiselect {
                    listbox.set_selection_mode(SelectionMode::Multiple);
                }
            }
        }
        WindowWidget::TextEntry(entry) => {
            if let Some(data) = window_def.text_entry_data.as_ref() {
                if data.max_len > 0 {
                    entry.set_max_length(data.max_len);
                }
                entry.set_password(data.secret_text);
                let validation = if data.numerical_only {
                    ValidationMode::NumericOnly
                } else if data.alphanumerical_only {
                    ValidationMode::AlphanumericOnly
                } else if data.ascii_only {
                    ValidationMode::AsciiOnly
                } else {
                    ValidationMode::None
                };
                entry.set_validation(validation);
            }
        }
        WindowWidget::StaticText(label) => {
            if let Some(data) = window_def.static_text_data.as_ref() {
                if data.centered {
                    label.set_alignment(TextAlignment::Center, VerticalAlignment::Center);
                }
            }
        }
        WindowWidget::HorizontalSlider(slider) => {
            if let Some(data) = window_def.slider_data.as_ref() {
                slider.set_range(data.min_value, data.max_value);
            }
        }
        WindowWidget::VerticalSlider(slider) => {
            if let Some(data) = window_def.slider_data.as_ref() {
                slider.set_range(data.min_value, data.max_value);
            }
        }
        WindowWidget::ComboBox(combo) => {
            if let Some(data) = window_def.combo_box_data.as_ref() {
                combo.set_editable(data.is_editable);
                if data.max_chars > 0 {
                    combo.set_max_chars(data.max_chars);
                }
                combo.set_ascii_only(data.ascii_only);
                combo.set_letters_and_numbers(data.letters_and_numbers);
                if data.max_display > 0 {
                    combo.set_max_display(data.max_display);
                }
            }
        }
        WindowWidget::TabControl(tab_control) => {
            if let Some(data) = window_def.tab_control_data.as_ref() {
                tab_control.set_tab_data(TabControlData {
                    tab_orientation: data.tab_orientation,
                    tab_edge: data.tab_edge,
                    tab_width: data.tab_width,
                    tab_height: data.tab_height,
                    tab_count: data.tab_count,
                    pane_border: data.pane_border,
                    sub_pane_disabled: data.sub_pane_disabled,
                });
            }
        }
        _ => {}
    });
}

fn pick_first_image(draw_data: &[LegacyDrawData]) -> Option<String> {
    for entry in draw_data {
        if let Some(image) = &entry.image {
            if !image.name.is_empty() {
                if image.name != "NoImage" {
                    return Some(image.name.clone());
                }
            }
        }
    }
    None
}

fn pick_first_colors(draw_data: &[LegacyDrawData]) -> ([f32; 4], [f32; 4]) {
    for entry in draw_data {
        let color = color_to_rgba(entry.color);
        let border = color_to_rgba(entry.border_color);
        if color[3] > 0.0 || border[3] > 0.0 {
            return (color, border);
        }
    }
    ([0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0])
}

/// Map winit physical key codes to UI KeyCode
fn map_winit_keycode(physical: &PhysicalKey) -> Option<GuiKeyCode> {
    let keycode = match physical {
        PhysicalKey::Code(code) => code,
        // We ignore Unidentified/Other variants
        _ => return None,
    };

    Some(match keycode {
        WinitKeyCode::Tab => GuiKeyCode::Tab,
        WinitKeyCode::Enter | WinitKeyCode::NumpadEnter => GuiKeyCode::Enter,
        WinitKeyCode::Escape => GuiKeyCode::Escape,
        WinitKeyCode::Space => GuiKeyCode::Space,
        WinitKeyCode::Backspace => GuiKeyCode::Backspace,
        WinitKeyCode::Delete => GuiKeyCode::Delete,
        WinitKeyCode::ArrowLeft => GuiKeyCode::Left,
        WinitKeyCode::ArrowRight => GuiKeyCode::Right,
        WinitKeyCode::ArrowUp => GuiKeyCode::Up,
        WinitKeyCode::ArrowDown => GuiKeyCode::Down,
        WinitKeyCode::Home => GuiKeyCode::Home,
        WinitKeyCode::End => GuiKeyCode::End,
        WinitKeyCode::PageUp => GuiKeyCode::PageUp,
        WinitKeyCode::PageDown => GuiKeyCode::PageDown,
        WinitKeyCode::F1 => GuiKeyCode::F1,
        WinitKeyCode::F2 => GuiKeyCode::F2,
        WinitKeyCode::F3 => GuiKeyCode::F3,
        WinitKeyCode::F4 => GuiKeyCode::F4,
        WinitKeyCode::F5 => GuiKeyCode::F5,
        WinitKeyCode::F6 => GuiKeyCode::F6,
        WinitKeyCode::F7 => GuiKeyCode::F7,
        WinitKeyCode::F8 => GuiKeyCode::F8,
        WinitKeyCode::F9 => GuiKeyCode::F9,
        WinitKeyCode::F10 => GuiKeyCode::F10,
        WinitKeyCode::F11 => GuiKeyCode::F11,
        WinitKeyCode::F12 => GuiKeyCode::F12,
        WinitKeyCode::KeyA => GuiKeyCode::A,
        WinitKeyCode::KeyB => GuiKeyCode::B,
        WinitKeyCode::KeyC => GuiKeyCode::C,
        WinitKeyCode::KeyD => GuiKeyCode::D,
        WinitKeyCode::KeyE => GuiKeyCode::E,
        WinitKeyCode::KeyF => GuiKeyCode::F,
        WinitKeyCode::KeyG => GuiKeyCode::G,
        WinitKeyCode::KeyH => GuiKeyCode::H,
        WinitKeyCode::KeyI => GuiKeyCode::I,
        WinitKeyCode::KeyJ => GuiKeyCode::J,
        WinitKeyCode::KeyK => GuiKeyCode::K,
        WinitKeyCode::KeyL => GuiKeyCode::L,
        WinitKeyCode::KeyM => GuiKeyCode::M,
        WinitKeyCode::KeyN => GuiKeyCode::N,
        WinitKeyCode::KeyO => GuiKeyCode::O,
        WinitKeyCode::KeyP => GuiKeyCode::P,
        WinitKeyCode::KeyQ => GuiKeyCode::Q,
        WinitKeyCode::KeyR => GuiKeyCode::R,
        WinitKeyCode::KeyS => GuiKeyCode::S,
        WinitKeyCode::KeyT => GuiKeyCode::T,
        WinitKeyCode::KeyU => GuiKeyCode::U,
        WinitKeyCode::KeyV => GuiKeyCode::V,
        WinitKeyCode::KeyW => GuiKeyCode::W,
        WinitKeyCode::KeyX => GuiKeyCode::X,
        WinitKeyCode::KeyY => GuiKeyCode::Y,
        WinitKeyCode::KeyZ => GuiKeyCode::Z,
        WinitKeyCode::Digit0 => GuiKeyCode::Num0,
        WinitKeyCode::Digit1 => GuiKeyCode::Num1,
        WinitKeyCode::Digit2 => GuiKeyCode::Num2,
        WinitKeyCode::Digit3 => GuiKeyCode::Num3,
        WinitKeyCode::Digit4 => GuiKeyCode::Num4,
        WinitKeyCode::Digit5 => GuiKeyCode::Num5,
        WinitKeyCode::Digit6 => GuiKeyCode::Num6,
        WinitKeyCode::Digit7 => GuiKeyCode::Num7,
        WinitKeyCode::Digit8 => GuiKeyCode::Num8,
        WinitKeyCode::Digit9 => GuiKeyCode::Num9,
        // Numpad digits map to same numeric keys for UI shortcuts
        WinitKeyCode::Numpad0 => GuiKeyCode::Num0,
        WinitKeyCode::Numpad1 => GuiKeyCode::Num1,
        WinitKeyCode::Numpad2 => GuiKeyCode::Num2,
        WinitKeyCode::Numpad3 => GuiKeyCode::Num3,
        WinitKeyCode::Numpad4 => GuiKeyCode::Num4,
        WinitKeyCode::Numpad5 => GuiKeyCode::Num5,
        WinitKeyCode::Numpad6 => GuiKeyCode::Num6,
        WinitKeyCode::Numpad7 => GuiKeyCode::Num7,
        WinitKeyCode::Numpad8 => GuiKeyCode::Num8,
        WinitKeyCode::Numpad9 => GuiKeyCode::Num9,
        _ => return None,
    })
}

fn encode_keycode(key: &GuiKeyCode) -> u32 {
    match key {
        GuiKeyCode::Backspace => 8,
        GuiKeyCode::Tab => 9,
        GuiKeyCode::Enter => 13,
        GuiKeyCode::Escape => 27,
        GuiKeyCode::Space => 32,
        GuiKeyCode::Delete => 127,
        GuiKeyCode::Left => 0x1000,
        GuiKeyCode::Right => 0x1001,
        GuiKeyCode::Up => 0x1002,
        GuiKeyCode::Down => 0x1003,
        GuiKeyCode::Home => 0x1004,
        GuiKeyCode::End => 0x1005,
        GuiKeyCode::PageUp => 0x1006,
        GuiKeyCode::PageDown => 0x1007,
        GuiKeyCode::Num0 => b'0' as u32,
        GuiKeyCode::Num1 => b'1' as u32,
        GuiKeyCode::Num2 => b'2' as u32,
        GuiKeyCode::Num3 => b'3' as u32,
        GuiKeyCode::Num4 => b'4' as u32,
        GuiKeyCode::Num5 => b'5' as u32,
        GuiKeyCode::Num6 => b'6' as u32,
        GuiKeyCode::Num7 => b'7' as u32,
        GuiKeyCode::Num8 => b'8' as u32,
        GuiKeyCode::Num9 => b'9' as u32,
        GuiKeyCode::A => b'a' as u32,
        GuiKeyCode::B => b'b' as u32,
        GuiKeyCode::C => b'c' as u32,
        GuiKeyCode::D => b'd' as u32,
        GuiKeyCode::E => b'e' as u32,
        GuiKeyCode::F => b'f' as u32,
        GuiKeyCode::G => b'g' as u32,
        GuiKeyCode::H => b'h' as u32,
        GuiKeyCode::I => b'i' as u32,
        GuiKeyCode::J => b'j' as u32,
        GuiKeyCode::K => b'k' as u32,
        GuiKeyCode::L => b'l' as u32,
        GuiKeyCode::M => b'm' as u32,
        GuiKeyCode::N => b'n' as u32,
        GuiKeyCode::O => b'o' as u32,
        GuiKeyCode::P => b'p' as u32,
        GuiKeyCode::Q => b'q' as u32,
        GuiKeyCode::R => b'r' as u32,
        GuiKeyCode::S => b's' as u32,
        GuiKeyCode::T => b't' as u32,
        GuiKeyCode::U => b'u' as u32,
        GuiKeyCode::V => b'v' as u32,
        GuiKeyCode::W => b'w' as u32,
        GuiKeyCode::X => b'x' as u32,
        GuiKeyCode::Y => b'y' as u32,
        GuiKeyCode::Z => b'z' as u32,
        _ => 0,
    }
}
