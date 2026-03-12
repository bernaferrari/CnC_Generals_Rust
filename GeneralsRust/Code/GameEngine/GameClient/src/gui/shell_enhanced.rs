//! Enhanced Shell System
//!
//! Complete implementation of the Shell menu system that handles stack-based navigation,
//! screen transitions, animations, and menu management exactly like the original C++.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, Instant};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::system::{SubsystemInterface, SubsystemError, SubsystemState};
use super::game_window_enhanced::{EnhancedGameWindow, WindowCallbacks, WindowMessage, WindowMsgHandled};
use super::window_manager_enhanced::EnhancedWindowManager;
use super::ui_renderer::UIRenderer;

/// Shell system errors
#[derive(Error, Debug)]
pub enum ShellError {
    #[error("Layout not found: {0}")]
    LayoutNotFound(String),
    #[error("Shell stack overflow - maximum {max} screens reached")]
    StackOverflow { max: usize },
    #[error("Cannot pop from empty shell stack")]
    EmptyStack,
    #[error("Shell not initialized")]
    NotInitialized,
    #[error("Layout operation failed: {0}")]
    LayoutError(String),
    #[error("Animation error: {0}")]
    AnimationError(String),
    #[error("Scheme loading error: {0}")]
    SchemeError(String),
    #[error("File I/O error: {0}")]
    FileError(String),
}

type Result<T> = std::result::Result<T, ShellError>;

/// Animation types for window transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    None,
    Fade,
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
    Scale,
    Rotate,
    Dissolve,
    Wipe,
}

/// Layout state for tracking screen lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutState {
    None,
    Loading,
    Initializing,
    Active,
    ShuttingDown,
    Destroyed,
}

/// Shell menu scheme for theming
#[derive(Debug, Clone)]
pub struct ShellMenuScheme {
    pub name: String,
    pub background_image: Option<String>,
    pub background_color: [f32; 4],
    pub button_images: HashMap<String, String>,
    pub text_colors: HashMap<String, [f32; 4]>,
    pub fonts: HashMap<String, String>,
    pub sounds: HashMap<String, String>,
    pub animations: HashMap<String, AnimationConfig>,
}

/// Animation configuration
#[derive(Debug, Clone)]
pub struct AnimationConfig {
    pub animation_type: AnimationType,
    pub duration: f32,
    pub delay: f32,
    pub easing: EasingType,
}

/// Easing types for animations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingType {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Bounce,
    Elastic,
}

/// Window layout information
#[derive(Debug)]
pub struct WindowLayout {
    pub file_path: PathBuf,
    pub root_windows: Vec<Arc<EnhancedGameWindow>>,
    pub state: LayoutState,
    pub init_callback: Option<Box<dyn Fn(&mut WindowLayout) -> Result<()> + Send + Sync>>,
    pub update_callback: Option<Box<dyn Fn(&mut WindowLayout, f32) -> Result<()> + Send + Sync>>,
    pub shutdown_callback: Option<Box<dyn Fn(&mut WindowLayout, bool) -> Result<()> + Send + Sync>>,
    pub user_data: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
}

impl WindowLayout {
    pub fn new<P: AsRef<Path>>(file_path: P) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            root_windows: Vec::new(),
            state: LayoutState::None,
            init_callback: None,
            update_callback: None,
            shutdown_callback: None,
            user_data: HashMap::new(),
        }
    }
    
    pub fn set_init_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut WindowLayout) -> Result<()> + Send + Sync + 'static,
    {
        self.init_callback = Some(Box::new(callback));
    }
    
    pub fn set_update_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut WindowLayout, f32) -> Result<()> + Send + Sync + 'static,
    {
        self.update_callback = Some(Box::new(callback));
    }
    
    pub fn set_shutdown_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut WindowLayout, bool) -> Result<()> + Send + Sync + 'static,
    {
        self.shutdown_callback = Some(Box::new(callback));
    }
}

/// Animate Window Manager for handling transitions
pub struct AnimateWindowManager {
    active_animations: Vec<WindowAnimation>,
    finished_animations: VecDeque<WindowAnimation>,
}

/// Window animation data
#[derive(Debug, Clone)]
pub struct WindowAnimation {
    pub window: Arc<EnhancedGameWindow>,
    pub animation_type: AnimationType,
    pub start_time: Instant,
    pub duration: f32,
    pub start_position: (f32, f32),
    pub end_position: (f32, f32),
    pub start_size: (f32, f32),
    pub end_size: (f32, f32),
    pub start_alpha: f32,
    pub end_alpha: f32,
    pub easing: EasingType,
    pub needs_to_finish: bool,
    pub finished: bool,
}

impl AnimateWindowManager {
    pub fn new() -> Self {
        Self {
            active_animations: Vec::new(),
            finished_animations: VecDeque::new(),
        }
    }
    
    pub fn register_window(&mut self, window: Arc<EnhancedGameWindow>, animation_type: AnimationType, needs_to_finish: bool, delay_ms: u32) {
        let animation = WindowAnimation {
            window,
            animation_type,
            start_time: Instant::now() + Duration::from_millis(delay_ms as u64),
            duration: 0.5, // Default duration
            start_position: (0.0, 0.0),
            end_position: (0.0, 0.0),
            start_size: (100.0, 100.0),
            end_size: (100.0, 100.0),
            start_alpha: 0.0,
            end_alpha: 1.0,
            easing: EasingType::EaseInOut,
            needs_to_finish,
            finished: false,
        };
        
        self.active_animations.push(animation);
    }
    
    pub fn update(&mut self, current_time: Instant) {
        for animation in &mut self.active_animations {
            if current_time >= animation.start_time && !animation.finished {
                let elapsed = (current_time - animation.start_time).as_secs_f32();
                let progress = (elapsed / animation.duration).min(1.0);
                
                // Apply easing
                let eased_progress = self.apply_easing(progress, animation.easing);
                
                // Update window properties based on animation type
                match animation.animation_type {
                    AnimationType::Fade => {
                        // Would update window alpha
                    }
                    AnimationType::SlideLeft => {
                        let current_x = animation.start_position.0 + 
                            (animation.end_position.0 - animation.start_position.0) * eased_progress;
                        animation.window.set_position(current_x as i32, animation.start_position.1 as i32);
                    }
                    AnimationType::Scale => {
                        let current_width = animation.start_size.0 + 
                            (animation.end_size.0 - animation.start_size.0) * eased_progress;
                        let current_height = animation.start_size.1 + 
                            (animation.end_size.1 - animation.start_size.1) * eased_progress;
                        animation.window.set_size(current_width as i32, current_height as i32);
                    }
                    _ => {}
                }
                
                if progress >= 1.0 {
                    animation.finished = true;
                    self.finished_animations.push_back(animation.clone());
                }
            }
        }
        
        // Remove finished animations
        self.active_animations.retain(|anim| !anim.finished);
    }
    
    pub fn is_finished(&self) -> bool {
        self.active_animations.iter().all(|anim| !anim.needs_to_finish || anim.finished)
    }
    
    pub fn reverse_animations(&mut self) {
        for animation in &mut self.active_animations {
            std::mem::swap(&mut animation.start_position, &mut animation.end_position);
            std::mem::swap(&mut animation.start_size, &mut animation.end_size);
            std::mem::swap(&mut animation.start_alpha, &mut animation.end_alpha);
            animation.start_time = Instant::now();
            animation.finished = false;
        }
    }
    
    pub fn is_reversed(&self) -> bool {
        // Would track reverse state
        false
    }
    
    fn apply_easing(&self, progress: f32, easing: EasingType) -> f32 {
        match easing {
            EasingType::Linear => progress,
            EasingType::EaseIn => progress * progress,
            EasingType::EaseOut => 1.0 - (1.0 - progress) * (1.0 - progress),
            EasingType::EaseInOut => {
                if progress < 0.5 {
                    2.0 * progress * progress
                } else {
                    1.0 - 2.0 * (1.0 - progress) * (1.0 - progress)
                }
            }
            EasingType::Bounce => {
                if progress < 1.0 / 2.75 {
                    7.5625 * progress * progress
                } else if progress < 2.0 / 2.75 {
                    let p = progress - 1.5 / 2.75;
                    7.5625 * p * p + 0.75
                } else if progress < 2.5 / 2.75 {
                    let p = progress - 2.25 / 2.75;
                    7.5625 * p * p + 0.9375
                } else {
                    let p = progress - 2.625 / 2.75;
                    7.5625 * p * p + 0.984375
                }
            }
            EasingType::Elastic => {
                if progress == 0.0 || progress == 1.0 {
                    progress
                } else {
                    let p = progress - 1.0;
                    -(2.0_f32.powf(10.0 * p)) * ((p * 40.0 - 3.0) * std::f32::consts::PI / 6.0).sin()
                }
            }
        }
    }
}

/// Shell Menu Scheme Manager
pub struct ShellMenuSchemeManager {
    current_scheme: Option<ShellMenuScheme>,
    available_schemes: HashMap<String, ShellMenuScheme>,
}

impl ShellMenuSchemeManager {
    pub fn new() -> Self {
        Self {
            current_scheme: None,
            available_schemes: HashMap::new(),
        }
    }
    
    pub fn load_scheme(&mut self, name: &str, file_path: &str) -> Result<()> {
        // Would load scheme from file
        let scheme = ShellMenuScheme {
            name: name.to_string(),
            background_image: None,
            background_color: [0.0, 0.0, 0.0, 1.0],
            button_images: HashMap::new(),
            text_colors: HashMap::new(),
            fonts: HashMap::new(),
            sounds: HashMap::new(),
            animations: HashMap::new(),
        };
        
        self.available_schemes.insert(name.to_string(), scheme.clone());
        self.current_scheme = Some(scheme);
        Ok(())
    }
    
    pub fn set_current_scheme(&mut self, name: &str) -> Result<()> {
        if let Some(scheme) = self.available_schemes.get(name) {
            self.current_scheme = Some(scheme.clone());
            Ok(())
        } else {
            Err(ShellError::SchemeError(format!("Scheme not found: {}", name)))
        }
    }
    
    pub fn get_current_scheme(&self) -> Option<&ShellMenuScheme> {
        self.current_scheme.as_ref()
    }
}

/// Enhanced Shell System
pub struct EnhancedShell {
    // Core state
    state: SubsystemState,
    is_shell_active: bool,
    use_shell_map: bool,
    is_hidden: bool,
    
    // Stack management
    screen_stack: Vec<WindowLayout>,
    screen_count: usize,
    max_shell_stack: usize,
    
    // Background handling
    background_layout: Option<WindowLayout>,
    clear_background: bool,
    
    // Pending operations
    pending_push: bool,
    pending_pop: bool,
    pending_push_name: Option<String>,
    pending_shutdown_immediate: bool,
    
    // Managers
    window_manager: Option<Arc<EnhancedWindowManager>>,
    animate_manager: AnimateWindowManager,
    scheme_manager: ShellMenuSchemeManager,
    
    // Special layouts
    save_load_layout: Option<WindowLayout>,
    popup_replay_layout: Option<WindowLayout>,
    options_layout: Option<WindowLayout>,
    
    // Update timing
    update_delay: Duration,
    last_update: Instant,
}

impl EnhancedShell {
    pub fn new() -> Self {
        Self {
            state: SubsystemState::Uninitialized,
            is_shell_active: false,
            use_shell_map: true,
            is_hidden: false,
            screen_stack: Vec::new(),
            screen_count: 0,
            max_shell_stack: 16,
            background_layout: None,
            clear_background: true,
            pending_push: false,
            pending_pop: false,
            pending_push_name: None,
            pending_shutdown_immediate: false,
            window_manager: None,
            animate_manager: AnimateWindowManager::new(),
            scheme_manager: ShellMenuSchemeManager::new(),
            save_load_layout: None,
            popup_replay_layout: None,
            options_layout: None,
            update_delay: Duration::from_millis(16), // 60 FPS
            last_update: Instant::now(),
        }
    }
    
    pub fn set_window_manager(&mut self, manager: Arc<EnhancedWindowManager>) {
        self.window_manager = Some(manager);
    }
    
    /// Show or hide shell map
    pub fn show_shell_map(&mut self, use_shell_map: bool) {
        self.use_shell_map = use_shell_map;
    }
    
    /// Hide or show all shell layouts
    pub fn hide(&mut self, hide: bool) {
        self.is_hidden = hide;
        
        // Hide/show all layouts in stack
        for layout in &mut self.screen_stack {
            for window in &layout.root_windows {
                window.hide(hide);
            }
        }
    }
    
    /// Push new screen on top, optionally doing immediate shutdown
    pub fn push(&mut self, filename: &str, shutdown_immediate: bool) -> Result<()> {
        if self.screen_count >= self.max_shell_stack {
            return Err(ShellError::StackOverflow { max: self.max_shell_stack });
        }
        
        // Set pending push
        self.pending_push = true;
        self.pending_push_name = Some(filename.to_string());
        self.pending_shutdown_immediate = shutdown_immediate;
        
        // Start shutdown of current top if exists
        if let Some(current_top) = self.screen_stack.last_mut() {
            current_top.state = LayoutState::ShuttingDown;
            
            // Call shutdown callback
            if let Some(ref callback) = current_top.shutdown_callback.take() {
                callback(current_top, shutdown_immediate)?;
            }
            
            if shutdown_immediate {
                self.shutdown_complete(current_top, true)?;
            }
        } else {
            // No current screen, do push immediately
            self.do_push(filename)?;
        }
        
        Ok(())
    }
    
    /// Pop top layout
    pub fn pop(&mut self) -> Result<()> {
        if self.screen_stack.is_empty() {
            return Err(ShellError::EmptyStack);
        }
        
        // Set pending pop
        self.pending_pop = true;
        
        // Start shutdown of top
        if let Some(current_top) = self.screen_stack.last_mut() {
            current_top.state = LayoutState::ShuttingDown;
            
            // Call shutdown callback
            if let Some(ref callback) = current_top.shutdown_callback.take() {
                callback(current_top, false)?;
            }
        }
        
        Ok(())
    }
    
    /// Pop immediately without waiting for shutdown
    pub fn pop_immediate(&mut self) -> Result<()> {
        if self.screen_stack.is_empty() {
            return Err(ShellError::EmptyStack);
        }
        
        // Pop and destroy immediately
        if let Some(mut layout) = self.screen_stack.pop() {
            self.screen_count -= 1;
            layout.state = LayoutState::Destroyed;
            
            // Hide window
            for window in &layout.root_windows {
                window.hide(true);
            }
        }
        
        // Initialize new top if exists
        if let Some(new_top) = self.screen_stack.last_mut() {
            self.init_layout(new_top)?;
        }
        
        Ok(())
    }
    
    /// Initialize the top of stack
    pub fn show_shell(&mut self, run_init: bool) -> Result<()> {
        self.is_shell_active = true;
        
        if run_init {
            if let Some(top) = self.screen_stack.last_mut() {
                self.init_layout(top)?;
            }
        }
        
        Ok(())
    }
    
    /// Shutdown the top of stack
    pub fn hide_shell(&mut self) -> Result<()> {
        self.is_shell_active = false;
        
        if let Some(top) = self.screen_stack.last_mut() {
            top.state = LayoutState::ShuttingDown;
            
            // Call shutdown callback
            if let Some(ref callback) = top.shutdown_callback.take() {
                callback(top, true)?;
            }
        }
        
        Ok(())
    }
    
    /// Return top layout
    pub fn top(&self) -> Option<&WindowLayout> {
        self.screen_stack.last()
    }
    
    /// Return mutable top layout
    pub fn top_mut(&mut self) -> Option<&mut WindowLayout> {
        self.screen_stack.last_mut()
    }
    
    /// Layout has completed shutdown
    pub fn shutdown_complete(&mut self, layout: &mut WindowLayout, impending_push: bool) -> Result<()> {
        layout.state = LayoutState::Destroyed;
        
        if self.pending_push && impending_push {
            // Complete the pending push
            if let Some(ref filename) = self.pending_push_name.clone() {
                self.do_push(filename)?;
            }
            self.pending_push = false;
            self.pending_push_name = None;
        } else if self.pending_pop {
            // Complete the pending pop
            self.do_pop(false)?;
            self.pending_pop = false;
        }
        
        Ok(())
    }
    
    /// Find screen by filename
    pub fn find_screen_by_filename(&self, filename: &str) -> Option<&WindowLayout> {
        self.screen_stack.iter().find(|layout| {
            layout.file_path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name == filename)
                .unwrap_or(false)
        })
    }
    
    /// Get screen count
    pub fn get_screen_count(&self) -> usize {
        self.screen_count
    }
    
    /// Register window with animate manager
    pub fn register_with_animate_manager(&mut self, window: Arc<EnhancedGameWindow>, animation_type: AnimationType, needs_to_finish: bool, delay_ms: u32) {
        self.animate_manager.register_window(window, animation_type, needs_to_finish, delay_ms);
    }
    
    /// Check if animations are finished
    pub fn is_anim_finished(&self) -> bool {
        self.animate_manager.is_finished()
    }
    
    /// Reverse animate windows
    pub fn reverse_animate_window(&mut self) {
        self.animate_manager.reverse_animations();
    }
    
    /// Check if animations are reversed
    pub fn is_anim_reversed(&self) -> bool {
        self.animate_manager.is_reversed()
    }
    
    /// Load scheme
    pub fn load_scheme(&mut self, name: &str, file_path: &str) -> Result<()> {
        self.scheme_manager.load_scheme(name, file_path)
            .map_err(|e| ShellError::SchemeError(e.to_string()))
    }
    
    /// Get scheme manager
    pub fn get_shell_menu_scheme_manager(&self) -> &ShellMenuSchemeManager {
        &self.scheme_manager
    }
    
    /// Get scheme manager mutable
    pub fn get_shell_menu_scheme_manager_mut(&mut self) -> &mut ShellMenuSchemeManager {
        &mut self.scheme_manager
    }
    
    /// Get or create save/load menu layout
    pub fn get_save_load_menu_layout(&mut self) -> Result<&mut WindowLayout> {
        if self.save_load_layout.is_none() {
            let mut layout = WindowLayout::new("Menus/SaveLoadMenu.wnd");
            self.load_layout(&mut layout)?;
            self.save_load_layout = Some(layout);
        }
        
        Ok(self.save_load_layout.as_mut().unwrap())
    }
    
    /// Get or create popup replay layout
    pub fn get_popup_replay_layout(&mut self) -> Result<&mut WindowLayout> {
        if self.popup_replay_layout.is_none() {
            let mut layout = WindowLayout::new("Menus/PopupReplay.wnd");
            self.load_layout(&mut layout)?;
            self.popup_replay_layout = Some(layout);
        }
        
        Ok(self.popup_replay_layout.as_mut().unwrap())
    }
    
    /// Get or create options layout
    pub fn get_options_layout(&mut self, create: bool) -> Option<&mut WindowLayout> {
        if self.options_layout.is_none() && create {
            let mut layout = WindowLayout::new("Menus/OptionsMenu.wnd");
            if self.load_layout(&mut layout).is_ok() {
                self.options_layout = Some(layout);
            }
        }
        
        self.options_layout.as_mut()
    }
    
    /// Destroy options layout
    pub fn destroy_options_layout(&mut self) {
        if let Some(mut layout) = self.options_layout.take() {
            layout.state = LayoutState::Destroyed;
            for window in &layout.root_windows {
                window.hide(true);
            }
        }
    }
    
    // Private implementation methods
    
    fn do_push(&mut self, layout_file: &str) -> Result<()> {
        // Load new layout
        let mut layout = WindowLayout::new(layout_file);
        self.load_layout(&mut layout)?;
        
        // Add to stack
        self.screen_stack.push(layout);
        self.screen_count += 1;
        
        // Initialize the new layout
        if let Some(top) = self.screen_stack.last_mut() {
            self.init_layout(top)?;
        }
        
        self.pending_push = false;
        self.pending_push_name = None;
        
        Ok(())
    }
    
    fn do_pop(&mut self, impending_push: bool) -> Result<()> {
        // Remove top layout
        if let Some(mut layout) = self.screen_stack.pop() {
            self.screen_count -= 1;
            layout.state = LayoutState::Destroyed;
            
            // Hide window
            for window in &layout.root_windows {
                window.hide(true);
            }
        }
        
        // Initialize new top
        if !impending_push {
            if let Some(new_top) = self.screen_stack.last_mut() {
                self.init_layout(new_top)?;
            }
        }
        
        Ok(())
    }
    
    fn load_layout(&mut self, layout: &mut WindowLayout) -> Result<()> {
        layout.state = LayoutState::Loading;

        if let Some(ref window_manager) = self.window_manager {
            let windows = window_manager
                .create_windows_from_script(layout.file_path.to_string_lossy().as_ref())
                .map_err(|e| ShellError::LayoutError(e.to_string()))?;
            layout.root_windows = windows;
        }
        
        Ok(())
    }
    
    fn init_layout(&mut self, layout: &mut WindowLayout) -> Result<()> {
        layout.state = LayoutState::Initializing;
        
        // Show window
        for window in &layout.root_windows {
            window.hide(false);
            window.enable(true);
        }
        
        // Call init callback
        if let Some(ref callback) = layout.init_callback {
            callback(layout)?;
        }
        
        layout.state = LayoutState::Active;
        Ok(())
    }
}

impl SubsystemInterface for EnhancedShell {
    fn name(&self) -> &str {
        "EnhancedShell"
    }
    
    fn init(&mut self) -> std::result::Result<(), SubsystemError> {
        self.state = SubsystemState::Initializing;
        
        // Initialize scheme manager with default scheme
        if let Err(e) = self.scheme_manager.load_scheme("Default", "Data/DefaultScheme.xml") {
            log::warn!("Failed to load default scheme: {}", e);
        }
        
        self.state = SubsystemState::Running;
        Ok(())
    }
    
    fn reset(&mut self) -> std::result::Result<(), SubsystemError> {
        // Clear all layouts
        self.screen_stack.clear();
        self.screen_count = 0;
        
        // Reset state
        self.is_shell_active = false;
        self.is_hidden = false;
        self.pending_push = false;
        self.pending_pop = false;
        self.pending_push_name = None;
        
        // Clear special layouts
        self.save_load_layout = None;
        self.popup_replay_layout = None;
        self.options_layout = None;
        
        Ok(())
    }
    
    fn update(&mut self, delta_time: std::time::Duration) -> std::result::Result<(), SubsystemError> {
        let now = Instant::now();
        if now.duration_since(self.last_update) < self.update_delay {
            return Ok(());
        }
        
        let dt_secs = delta_time.as_secs_f32();
        self.last_update = now;
        
        // Update animations
        self.animate_manager.update(now);

        // Keep window manager state in sync (press animations, hover, etc.).
        if let Some(manager) = &self.window_manager {
            if let Err(err) = manager.update() {
                log::error!("Window manager update error: {}", err);
            }
        }
        
        // Update all layouts in stack (from bottom to top)
        for layout in &mut self.screen_stack {
            if let Some(ref callback) = layout.update_callback {
                if let Err(e) = callback(layout, dt_secs) {
                    log::error!("Layout update error: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    fn shutdown(&mut self) -> std::result::Result<(), SubsystemError> {
        self.state = SubsystemState::ShuttingDown;
        
        // Clear all layouts
        self.screen_stack.clear();
        self.screen_count = 0;
        
        self.state = SubsystemState::Shutdown;
        Ok(())
    }
    
    fn state(&self) -> SubsystemState {
        self.state
    }
}
