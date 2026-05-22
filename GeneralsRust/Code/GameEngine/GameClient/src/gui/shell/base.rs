//! # Shell Menu System
//!
//! This module provides the shell menu system for managing UI screens and menu navigation.
//! It implements a stack-based approach for screen transitions with proper initialization,
//! shutdown, and animation support.
//!
//! ## Features
//! - Stack-based screen management with push/pop operations
//! - Proper initialization and shutdown lifecycle for screens
//! - Animation system integration for smooth transitions
//! - Menu scheme support for theming
//! - Background management for different game states
//! - Support for various menu types (main menu, options, save/load, etc.)
//!
//! ## Architecture
//!
//! The shell system operates on a stack model where:
//! 1. Screens are pushed onto a stack when navigating forward
//! 2. Screens are popped from the stack when going back
//! 3. Each screen has proper init/shutdown lifecycle management
//! 4. Animations can be applied during transitions
//!
//! ## Usage
//! ```rust
//! use crate::gui::shell::{Shell, WindowLayout};
//!
//! let mut shell = Shell::new();
//! shell.init()?;
//!
//! // Push a new screen
//! shell.push("Menus/MainMenu.wnd", false)?;
//!
//! // Update the shell (call this every frame)
//! shell.update()?;
//!
//! // Pop current screen
//! shell.pop()?;
//! ```

use super::super::game_window::WIN_COLOR_UNDEFINED;
use super::super::game_window::{GameWindow, WindowStatus};
use super::super::ime_manager::get_ime_manager;
use super::super::window_manager::{
    with_window_manager, with_window_manager_ref, WindowLayout as ManagerWindowLayout,
};
use crate::message_stream::{get_message_stream, GameMessageType};
use crate::system::SubsystemInterface;
use game_engine::common::ini::get_global_data;
use game_engine::common::random_value::init_random_with_seed;
use gamelogic::helpers::TheGameLogic;
use gamelogic::system::game_logic::{GAME_NONE, GAME_SHELL};
use std::cell::RefCell;
use std::cmp::min;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use thiserror::Error;

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
}

#[cfg(feature = "network")]
fn close_shell_gamespy_overlays() {
    crate::gamespy_overlay::close_all_overlays();
}

#[cfg(not(feature = "network"))]
fn close_shell_gamespy_overlays() {}

/// Animation types for window transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    /// No animation
    None,
    /// Slide from left
    SlideLeft,
    /// Slide from right
    SlideRight,
    /// Slide from top
    SlideTop,
    /// Slide from bottom
    SlideBottom,
    /// Slide from right (fast)
    SlideRightFast,
    /// Slide from top (fast)
    SlideTopFast,
    /// Slide from bottom (timed)
    SlideBottomTimed,
    /// Spiral animation
    Spiral,
}

/// Window position and size
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl WindowRect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn zero() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

/// Window layout state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutState {
    /// Layout is being initialized
    Initializing,
    /// Layout is active and visible
    Active,
    /// Layout is shutting down
    ShuttingDown,
    /// Layout is hidden but still on stack
    Hidden,
    /// Layout is being destroyed
    Destroying,
}

/// Represents a window layout/screen in the shell system
pub trait WindowLayout {
    /// Get the filename this layout was loaded from
    fn get_filename(&self) -> &str;

    /// Initialize the layout - called when pushed or when becoming top of stack
    fn run_init(&mut self, data: Option<&dyn std::any::Any>) -> Result<(), ShellError>;

    /// Update the layout - called every frame for all layouts on stack
    fn run_update(&mut self, data: Option<&dyn std::any::Any>) -> Result<(), ShellError>;

    /// Shutdown the layout - called when being popped or when new layout pushed on top
    /// The immediate_pop parameter indicates if the layout should shutdown immediately
    fn run_shutdown(&mut self, immediate_pop: &mut bool) -> Result<(), ShellError>;

    /// Show/hide the layout
    fn hide(&mut self, hide: bool);

    /// Check if the layout is hidden
    fn is_hidden(&self) -> bool;

    /// Bring the layout to the front
    fn bring_forward(&mut self);

    /// Destroy all windows in this layout
    fn destroy_windows(&mut self);

    /// Mark the first window as an image-backed shell background when applicable.
    fn set_first_window_image(&mut self) {}

    /// Get the current state of the layout
    fn get_state(&self) -> LayoutState;

    /// Set the layout state
    fn set_state(&mut self, state: LayoutState);
}

/// Default implementation of WindowLayout for basic functionality
#[derive(Debug)]
pub struct BasicWindowLayout {
    filename: String,
    state: LayoutState,
    hidden: bool,
    bounds: WindowRect,
    created_at: Instant,
    layout: Option<Rc<RefCell<ManagerWindowLayout>>>,
}

impl BasicWindowLayout {
    pub fn new(filename: String) -> Self {
        Self {
            filename,
            state: LayoutState::Initializing,
            hidden: true,
            bounds: WindowRect::zero(),
            created_at: Instant::now(),
            layout: None,
        }
    }

    fn ensure_layout(&mut self) -> Result<Rc<RefCell<ManagerWindowLayout>>, ShellError> {
        if let Some(layout) = &self.layout {
            return Ok(layout.clone());
        }

        let (layout, _info) =
            with_window_manager(|manager| manager.create_layout_with_windows(&self.filename))
                .map_err(|err| ShellError::LayoutError(format!("{}: {:?}", self.filename, err)))?;

        self.layout = Some(layout.clone());
        Ok(layout)
    }
}

impl WindowLayout for BasicWindowLayout {
    fn get_filename(&self) -> &str {
        &self.filename
    }

    fn run_init(&mut self, data: Option<&dyn std::any::Any>) -> Result<(), ShellError> {
        log::debug!("Initializing layout: {}", self.filename);

        let layout = self.ensure_layout()?;
        {
            let layout_ref = layout.borrow();
            layout_ref.run_init(data);
        }

        self.state = LayoutState::Active;
        self.hidden = false;
        Ok(())
    }

    fn run_update(&mut self, data: Option<&dyn std::any::Any>) -> Result<(), ShellError> {
        if let Some(layout) = &self.layout {
            layout.borrow().run_update(data);
        }
        Ok(())
    }

    fn run_shutdown(&mut self, immediate_pop: &mut bool) -> Result<(), ShellError> {
        log::debug!(
            "Shutting down layout: {} (immediate: {})",
            self.filename,
            *immediate_pop
        );

        if let Some(layout) = &self.layout {
            let layout_ref = layout.borrow();
            layout_ref.run_shutdown(Some(immediate_pop as &mut dyn std::any::Any));
        }

        if *immediate_pop {
            self.state = LayoutState::Destroying;
        } else {
            self.state = LayoutState::ShuttingDown;
        }

        Ok(())
    }

    fn hide(&mut self, hide: bool) {
        if let Some(layout) = &self.layout {
            layout.borrow().hide(hide);
        }
        self.hidden = hide;
    }

    fn is_hidden(&self) -> bool {
        self.layout
            .as_ref()
            .map(|layout| layout.borrow().is_hidden())
            .unwrap_or(self.hidden)
    }

    fn bring_forward(&mut self) {
        log::debug!("Bringing layout to front: {}", self.filename);
        if let Some(layout) = &self.layout {
            layout.borrow_mut().bring_forward();
        }
    }

    fn set_first_window_image(&mut self) {
        if let Some(layout) = &self.layout {
            if let Some(first_window) = layout.borrow().get_first_window() {
                first_window.borrow_mut().set_status(WindowStatus::IMAGE);
            }
        }
    }

    fn destroy_windows(&mut self) {
        log::debug!("Destroying windows for layout: {}", self.filename);
        if let Some(layout) = self.layout.take() {
            with_window_manager(|manager| manager.destroy_layout(&layout));
        }
        self.state = LayoutState::Destroying;
        self.hidden = true;
    }

    fn get_state(&self) -> LayoutState {
        self.state
    }

    fn set_state(&mut self, state: LayoutState) {
        self.state = state;
    }
}

/// 2D coordinate structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coord2D {
    pub x: i32,
    pub y: i32,
}

impl Coord2D {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self::new(0, 0)
    }
}

/// 2D float coordinate for animation velocities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2DF {
    pub x: f32,
    pub y: f32,
}

impl Coord2DF {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Color representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn black() -> Self {
        Self::new(0, 0, 0, 255)
    }

    pub fn white() -> Self {
        Self::new(255, 255, 255, 255)
    }

    pub fn transparent() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

/// Shell menu scheme line for decorative elements
#[derive(Debug, Clone)]
pub struct ShellMenuSchemeLine {
    pub start_pos: Coord2D,
    pub end_pos: Coord2D,
    pub width: i32,
    pub color: Color,
}

impl ShellMenuSchemeLine {
    pub fn new(start: Coord2D, end: Coord2D, width: i32, color: Color) -> Self {
        Self {
            start_pos: start,
            end_pos: end,
            width,
            color,
        }
    }
}

/// Shell menu scheme image for decorative elements
#[derive(Debug, Clone)]
pub struct ShellMenuSchemeImage {
    pub name: String,
    pub position: Coord2D,
    pub size: Coord2D,
    // In a real implementation, this would hold an image handle
    pub image_data: Option<Vec<u8>>,
}

impl ShellMenuSchemeImage {
    pub fn new(name: String, position: Coord2D, size: Coord2D) -> Self {
        Self {
            name,
            position,
            size,
            image_data: None,
        }
    }
}

/// Shell menu scheme for theming and decoration
#[derive(Debug)]
pub struct ShellMenuScheme {
    pub name: String,
    pub images: Vec<ShellMenuSchemeImage>,
    pub lines: Vec<ShellMenuSchemeLine>,
}

impl ShellMenuScheme {
    pub fn new(name: String) -> Self {
        Self {
            name,
            images: Vec::new(),
            lines: Vec::new(),
        }
    }

    pub fn add_image(&mut self, image: ShellMenuSchemeImage) {
        self.images.push(image);
    }

    pub fn add_line(&mut self, line: ShellMenuSchemeLine) {
        self.lines.push(line);
    }

    pub fn draw(&self) {
        with_window_manager_ref(|manager| {
            for image in &self.images {
                if let Some(mapped) = manager.win_find_image(&image.name) {
                    manager.win_draw_image(
                        &mapped,
                        image.position.x,
                        image.position.y,
                        image.position.x + image.size.x,
                        image.position.y + image.size.y,
                        WIN_COLOR_UNDEFINED,
                    );
                }
            }

            for line in &self.lines {
                let color = ((line.color.a as u32) << 24)
                    | ((line.color.r as u32) << 16)
                    | ((line.color.g as u32) << 8)
                    | line.color.b as u32;
                manager.win_draw_line(
                    color,
                    line.width as f32,
                    line.start_pos.x,
                    line.start_pos.y,
                    line.end_pos.x,
                    line.end_pos.y,
                );
            }
        });
    }
}

/// Manager for shell menu schemes
#[derive(Debug)]
pub struct ShellMenuSchemeManager {
    schemes: HashMap<String, ShellMenuScheme>,
    scheme_order: Vec<String>,
    current_scheme: Option<String>,
}

impl ShellMenuSchemeManager {
    pub fn new() -> Self {
        Self {
            schemes: HashMap::new(),
            scheme_order: Vec::new(),
            current_scheme: None,
        }
    }

    pub fn init(&mut self) -> Result<(), ShellError> {
        log::info!("Initializing shell menu scheme manager");
        self.load_default_scheme_files();
        Ok(())
    }

    pub fn update(&mut self) -> Result<(), ShellError> {
        // Schemes don't need regular updates
        Ok(())
    }

    pub fn set_shell_menu_scheme(&mut self, name: &str) {
        if name.is_empty() {
            self.current_scheme = None;
            return;
        }
        let key = name.to_ascii_lowercase();
        if self.schemes.contains_key(&key) {
            self.current_scheme = Some(key);
            log::debug!("Set shell menu scheme to: {}", name);
        } else {
            // The C++ shell path does not require every menu to have a separate decorative
            // scheme object. Missing placeholder schemes should not spam startup warnings.
            log::debug!("Shell menu scheme not found: {}", name);
        }
    }

    pub fn draw(&self) {
        if let Some(scheme_name) = &self.current_scheme {
            if let Some(scheme) = self.schemes.get(scheme_name) {
                scheme.draw();
            }
        }
    }

    pub fn new_shell_menu_scheme(&mut self, name: String) -> &mut ShellMenuScheme {
        let key = name.trim().to_ascii_lowercase();
        self.schemes.remove(&key);
        self.scheme_order.retain(|existing| existing != &key);
        self.schemes
            .insert(key.clone(), ShellMenuScheme::new(key.clone()));
        self.scheme_order.push(key.clone());
        self.schemes.get_mut(&key).unwrap()
    }

    fn get_shell_menu_scheme_mut(&mut self, name: &str) -> Option<&mut ShellMenuScheme> {
        self.schemes.get_mut(&name.trim().to_ascii_lowercase())
    }

    fn load_default_scheme_files(&mut self) {
        let files = discover_shell_menu_scheme_ini_files();
        for path in files {
            if let Ok(contents) = fs::read_to_string(&path) {
                self.parse_shell_menu_schemes(&contents);
            }
        }
    }

    fn parse_shell_menu_schemes(&mut self, contents: &str) {
        let mut current_scheme: Option<String> = None;
        let mut current_image: Option<ShellMenuSchemeImage> = None;
        let mut current_line: Option<ShellMenuSchemeLine> = None;

        let flush_image = |manager: &mut ShellMenuSchemeManager,
                           scheme_name: &Option<String>,
                           image: &mut Option<ShellMenuSchemeImage>| {
            if let (Some(name), Some(image)) = (scheme_name.as_ref(), image.take()) {
                if let Some(scheme) = manager.get_shell_menu_scheme_mut(name) {
                    scheme.add_image(image);
                }
            }
        };
        let flush_line = |manager: &mut ShellMenuSchemeManager,
                          scheme_name: &Option<String>,
                          line: &mut Option<ShellMenuSchemeLine>| {
            if let (Some(name), Some(line)) = (scheme_name.as_ref(), line.take()) {
                if let Some(scheme) = manager.get_shell_menu_scheme_mut(name) {
                    scheme.add_line(line);
                }
            }
        };

        for raw_line in contents.lines() {
            let line = raw_line
                .split_once(';')
                .map(|(head, _)| head)
                .unwrap_or(raw_line)
                .trim();
            if line.is_empty() {
                continue;
            }
            if line.eq_ignore_ascii_case("End") {
                flush_image(self, &current_scheme, &mut current_image);
                flush_line(self, &current_scheme, &mut current_line);
                current_scheme = None;
                continue;
            }
            if line.eq_ignore_ascii_case("EndImagePart") {
                flush_image(self, &current_scheme, &mut current_image);
                continue;
            }
            if line.eq_ignore_ascii_case("EndLinePart") {
                flush_line(self, &current_scheme, &mut current_line);
                continue;
            }
            if let Some(name) = line.strip_prefix("ShellMenuScheme ") {
                flush_image(self, &current_scheme, &mut current_image);
                flush_line(self, &current_scheme, &mut current_line);
                let name = name.trim().to_string();
                self.new_shell_menu_scheme(name.clone());
                current_scheme = Some(name);
                continue;
            }
            if line.eq_ignore_ascii_case("ImagePart") {
                flush_image(self, &current_scheme, &mut current_image);
                current_image = Some(ShellMenuSchemeImage::new(
                    String::new(),
                    Coord2D::zero(),
                    Coord2D::zero(),
                ));
                continue;
            }
            if line.eq_ignore_ascii_case("LinePart") {
                flush_line(self, &current_scheme, &mut current_line);
                current_line = Some(ShellMenuSchemeLine::new(
                    Coord2D::zero(),
                    Coord2D::zero(),
                    1,
                    Color::transparent(),
                ));
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();

            if let Some(image) = current_image.as_mut() {
                match key {
                    "Position" => image.position = parse_coord2d(value),
                    "Size" => image.size = parse_coord2d(value),
                    "ImageName" => image.name = value.to_string(),
                    _ => {}
                }
                continue;
            }

            if let Some(line_part) = current_line.as_mut() {
                match key {
                    "StartPosition" => line_part.start_pos = parse_coord2d(value),
                    "EndPosition" => line_part.end_pos = parse_coord2d(value),
                    "Color" => line_part.color = parse_color_int(value),
                    "Width" => line_part.width = value.parse().unwrap_or(1),
                    _ => {}
                }
            }
        }

        flush_image(self, &current_scheme, &mut current_image);
        flush_line(self, &current_scheme, &mut current_line);
    }
}

fn parse_coord2d(value: &str) -> Coord2D {
    let mut parts = value.split_whitespace();
    let x = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let y = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    Coord2D::new(x, y)
}

fn parse_color_int(value: &str) -> Color {
    let parsed = value.parse::<u32>().unwrap_or(WIN_COLOR_UNDEFINED);
    Color::new(
        ((parsed >> 16) & 0xFF) as u8,
        ((parsed >> 8) & 0xFF) as u8,
        (parsed & 0xFF) as u8,
        ((parsed >> 24) & 0xFF) as u8,
    )
}

fn push_shell_menu_scheme_ini_file(
    files: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    path: PathBuf,
) {
    if !path.exists() {
        return;
    }
    if let Ok(canonical) = fs::canonicalize(&path) {
        if seen.insert(canonical.clone()) {
            files.push(canonical);
        }
    } else if seen.insert(path.clone()) {
        files.push(path);
    }
}

fn push_unique_root(roots: &mut Vec<PathBuf>, root: PathBuf) {
    if !roots.iter().any(|existing| existing == &root) {
        roots.push(root);
    }
}

fn ordered_shell_menu_scheme_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let mut ancestors: Vec<PathBuf> = parent.ancestors().map(Path::to_path_buf).collect();
            ancestors.reverse();
            for ancestor in ancestors {
                push_unique_root(&mut roots, ancestor);
            }
        }
    }

    if let Ok(current) = std::env::current_dir() {
        let mut ancestors: Vec<PathBuf> = current.ancestors().map(Path::to_path_buf).collect();
        ancestors.reverse();
        for ancestor in ancestors {
            push_unique_root(&mut roots, ancestor);
        }
    }

    if let Some(global) = get_global_data() {
        let mod_dir = global.read().mod_dir.clone();
        if !mod_dir.trim().is_empty() {
            push_unique_root(&mut roots, PathBuf::from(mod_dir.trim()));
        }
    }

    roots
}

fn discover_shell_menu_scheme_ini_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    for root in ordered_shell_menu_scheme_roots() {
        push_shell_menu_scheme_ini_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/Default/ShellMenuScheme.ini"),
        );
        push_shell_menu_scheme_ini_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/ShellMenuScheme.ini"),
        );
        for extracted in [
            root.join("windows_game/extracted_big_files/INIZH"),
            root.join("windows_game/extracted_big_files_v2/INIZH"),
        ] {
            push_shell_menu_scheme_ini_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/Default/ShellMenuScheme.ini"),
            );
            push_shell_menu_scheme_ini_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/ShellMenuScheme.ini"),
            );
        }
    }
    files
}

impl Default for ShellMenuSchemeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct AnimateWindow {
    window: Rc<RefCell<GameWindow>>,
    anim_type: AnimationType,
    delay_ms: u64,
    start_pos: Coord2D,
    end_pos: Coord2D,
    cur_pos: Coord2D,
    rest_pos: Coord2D,
    vel: Coord2DF,
    needs_to_finish: bool,
    finished: bool,
    start_time: Instant,
    end_time: Option<Instant>,
}

impl AnimateWindow {
    pub fn new(
        window: Rc<RefCell<GameWindow>>,
        anim_type: AnimationType,
        needs_to_finish: bool,
    ) -> Self {
        Self {
            window,
            anim_type,
            delay_ms: 0,
            start_pos: Coord2D::zero(),
            end_pos: Coord2D::zero(),
            cur_pos: Coord2D::zero(),
            rest_pos: Coord2D::zero(),
            vel: Coord2DF::new(0.0, 0.0),
            needs_to_finish,
            finished: false,
            start_time: Instant::now(),
            end_time: None,
        }
    }

    pub fn set_anim_data(
        &mut self,
        start_pos: Coord2D,
        end_pos: Coord2D,
        cur_pos: Coord2D,
        rest_pos: Coord2D,
        vel: Coord2DF,
        start_time: Instant,
        end_time: Option<Instant>,
    ) {
        self.start_pos = start_pos;
        self.end_pos = end_pos;
        self.cur_pos = cur_pos;
        self.rest_pos = rest_pos;
        self.vel = vel;
        self.start_time = start_time;
        self.end_time = end_time;
    }

    pub fn set_delay(&mut self, delay_ms: u64) {
        self.delay_ms = delay_ms;
    }

    pub fn get_delay(&self) -> u64 {
        self.delay_ms
    }
}

trait ProcessAnimateWindow {
    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32));
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        max_delay_ms: u64,
        screen_size: (i32, i32),
    );
    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        screen_size: (i32, i32),
    ) -> bool;
    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        screen_size: (i32, i32),
    ) -> bool;
    fn set_max_duration(&mut self, _duration_ms: u64) {}
}

struct ProcessAnimateWindowNoOp;

impl ProcessAnimateWindow for ProcessAnimateWindowNoOp {
    fn init_animate_window(&self, _anim_win: &mut AnimateWindow, _screen_size: (i32, i32)) {}
    fn init_reverse_animate_window(
        &self,
        _anim_win: &mut AnimateWindow,
        _max_delay_ms: u64,
        _screen_size: (i32, i32),
    ) {
    }
    fn update_animate_window(
        &self,
        _anim_win: &mut AnimateWindow,
        _now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        true
    }
    fn reverse_animate_window(
        &self,
        _anim_win: &mut AnimateWindow,
        _now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        true
    }
}

struct ProcessAnimateWindowSlideFromRight {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromRight {
    fn new() -> Self {
        let slow_down_ratio = 0.67;
        Self {
            max_vel: Coord2DF::new(-40.0, 0.0),
            slow_down_threshold: 80,
            slow_down_ratio,
            speed_up_ratio: 2.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromRight {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        max_delay_ms: u64,
        _screen_size: (i32, i32),
    ) {
        if anim_win.get_delay() > 0 {
            anim_win.start_time =
                Instant::now() + Duration::from_millis(max_delay_ms - anim_win.get_delay());
        }
        anim_win.vel.x *= -1.0;
        anim_win.vel.y *= -1.0;
        anim_win.finished = false;
        let pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        anim_win.cur_pos.y = pos.y;
        anim_win.end_pos.y = pos.y;
        anim_win.start_pos.y = pos.y;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let (rest_pos, size) = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            let (w, h) = win.get_size();
            (Coord2D::new(x, y), Coord2D::new(w, h))
        };
        let end_pos = rest_pos;
        let travel_distance = screen_width - rest_pos.x + size.x;
        let start_pos = Coord2D::new(rest_pos.x + travel_distance, rest_pos.y);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        let vel = self.max_vel;
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            vel,
            Instant::now() + Duration::from_millis(anim_win.get_delay()),
            None,
        );
        anim_win.finished = false;
    }

    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let end_pos = anim_win.end_pos;
        let mut vel = anim_win.vel;
        cur_pos.x += vel.x as i32;

        if cur_pos.x < end_pos.x {
            cur_pos.x = end_pos.x;
            anim_win.finished = true;
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        if cur_pos.x - end_pos.x <= self.slow_down_threshold {
            vel.x *= self.slow_down_ratio;
        }
        if vel.x >= -1.0 {
            vel.x = -1.0;
        }
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let start_pos = anim_win.start_pos;
        let mut vel = anim_win.vel;
        cur_pos.x += vel.x as i32;

        if cur_pos.x > start_pos.x {
            cur_pos.x = start_pos.x;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        let end_pos = anim_win.end_pos;
        if cur_pos.x - end_pos.x <= self.slow_down_threshold {
            vel.x *= self.speed_up_ratio;
        } else {
            vel.x = -self.max_vel.x;
        }
        if vel.x > -self.max_vel.x {
            vel.x = -self.max_vel.x;
        }
        anim_win.vel = vel;
        false
    }
}

struct ProcessAnimateWindowSlideFromLeft {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromLeft {
    fn new() -> Self {
        let slow_down_ratio = 0.67;
        Self {
            max_vel: Coord2DF::new(40.0, 0.0),
            slow_down_threshold: 80,
            slow_down_ratio,
            speed_up_ratio: 2.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromLeft {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        max_delay_ms: u64,
        _screen_size: (i32, i32),
    ) {
        if anim_win.get_delay() > 0 {
            anim_win.start_time =
                Instant::now() + Duration::from_millis(max_delay_ms - anim_win.get_delay());
        }
        anim_win.vel.x *= -1.0;
        anim_win.vel.y *= -1.0;
        anim_win.finished = false;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let start_pos = Coord2D::new(rest_pos.x - screen_width, rest_pos.y);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        let vel = self.max_vel;
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            vel,
            Instant::now() + Duration::from_millis(anim_win.get_delay()),
            None,
        );
        anim_win.finished = false;
    }

    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let end_pos = anim_win.end_pos;
        let mut vel = anim_win.vel;
        cur_pos.x += vel.x as i32;
        if cur_pos.x > end_pos.x {
            cur_pos.x = end_pos.x;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            anim_win.finished = true;
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        if end_pos.x - cur_pos.x <= self.slow_down_threshold {
            vel.x *= self.slow_down_ratio;
        }
        if vel.x < 1.0 {
            vel.x = 1.0;
        }
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let start_pos = anim_win.start_pos;
        let mut vel = anim_win.vel;
        cur_pos.x += vel.x as i32;

        if cur_pos.x < start_pos.x {
            cur_pos.x = start_pos.x;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        let end_pos = anim_win.end_pos;
        if end_pos.x - cur_pos.x <= self.slow_down_threshold {
            vel.x *= self.speed_up_ratio;
        } else {
            vel.x = -self.max_vel.x;
        }
        if vel.x < -self.max_vel.x {
            vel.x = -self.max_vel.x;
        }
        anim_win.vel = vel;
        false
    }
}

struct ProcessAnimateWindowSlideFromTop {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromTop {
    fn new() -> Self {
        let slow_down_ratio = 0.67;
        Self {
            max_vel: Coord2DF::new(0.0, 40.0),
            slow_down_threshold: 80,
            slow_down_ratio,
            speed_up_ratio: 2.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromTop {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        max_delay_ms: u64,
        _screen_size: (i32, i32),
    ) {
        if anim_win.get_delay() > 0 {
            anim_win.start_time =
                Instant::now() + Duration::from_millis(max_delay_ms - anim_win.get_delay());
        }
        anim_win.vel.x *= -1.0;
        anim_win.vel.y *= -1.0;
        anim_win.finished = false;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let _ = screen_size;
        let (rest_pos, height) = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            let (_, h) = win.get_size();
            (Coord2D::new(x, y), h)
        };
        let end_pos = rest_pos;
        let start_pos = Coord2D::new(rest_pos.x, -height);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        let vel = self.max_vel;
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            vel,
            Instant::now() + Duration::from_millis(anim_win.get_delay()),
            None,
        );
        anim_win.finished = false;
    }

    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let end_pos = anim_win.end_pos;
        let mut vel = anim_win.vel;
        cur_pos.y += vel.y as i32;
        if cur_pos.y > end_pos.y {
            cur_pos.y = end_pos.y;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            anim_win.finished = true;
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        if end_pos.y - cur_pos.y <= self.slow_down_threshold {
            vel.y *= self.slow_down_ratio;
        }
        if vel.y <= 1.0 {
            vel.y = 1.0;
        }
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let start_pos = anim_win.start_pos;
        let mut vel = anim_win.vel;
        cur_pos.y += vel.y as i32;

        if cur_pos.y < start_pos.y {
            cur_pos.y = start_pos.y;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        let end_pos = anim_win.end_pos;
        if end_pos.y - cur_pos.y <= self.slow_down_threshold {
            vel.y *= self.speed_up_ratio;
        } else {
            vel.y = -self.max_vel.y;
        }
        if vel.y < -self.max_vel.y {
            vel.y = -self.max_vel.y;
        }
        anim_win.vel = vel;
        false
    }
}

struct ProcessAnimateWindowSlideFromTopFast {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromTopFast {
    fn new() -> Self {
        let slow_down_ratio = 0.67;
        Self {
            max_vel: Coord2DF::new(0.0, 60.0),
            slow_down_threshold: 40,
            slow_down_ratio,
            speed_up_ratio: 4.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromTopFast {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        max_delay_ms: u64,
        _screen_size: (i32, i32),
    ) {
        if anim_win.get_delay() > 0 {
            anim_win.start_time =
                Instant::now() + Duration::from_millis(max_delay_ms - anim_win.get_delay());
        }
        anim_win.vel.x *= -1.0;
        anim_win.vel.y *= -1.0;
        anim_win.finished = false;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let travel_distance = screen_width;
        let start_pos = Coord2D::new(rest_pos.x, rest_pos.y - travel_distance);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        let vel = self.max_vel;
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            vel,
            Instant::now() + Duration::from_millis(anim_win.get_delay()),
            None,
        );
        anim_win.finished = false;
    }

    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let end_pos = anim_win.end_pos;
        let mut vel = anim_win.vel;
        cur_pos.y += vel.y as i32;
        if cur_pos.y > end_pos.y {
            cur_pos.y = end_pos.y;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            anim_win.finished = true;
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        if end_pos.y - cur_pos.y <= self.slow_down_threshold {
            vel.y *= self.slow_down_ratio;
        }
        if vel.y <= 1.0 {
            vel.y = 1.0;
        }
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let start_pos = anim_win.start_pos;
        let mut vel = anim_win.vel;
        cur_pos.y += vel.y as i32;

        if cur_pos.y < start_pos.y {
            cur_pos.y = start_pos.y;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        let end_pos = anim_win.end_pos;
        if end_pos.y - cur_pos.y <= self.slow_down_threshold {
            vel.y *= self.speed_up_ratio;
        } else {
            vel.y = -self.max_vel.y;
        }
        if vel.y < -self.max_vel.y {
            vel.y = -self.max_vel.y;
        }
        anim_win.vel = vel;
        false
    }
}

struct ProcessAnimateWindowSlideFromBottom {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromBottom {
    fn new() -> Self {
        let slow_down_ratio = 0.67;
        Self {
            max_vel: Coord2DF::new(0.0, -40.0),
            slow_down_threshold: 80,
            slow_down_ratio,
            speed_up_ratio: 2.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromBottom {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        max_delay_ms: u64,
        _screen_size: (i32, i32),
    ) {
        if anim_win.get_delay() > 0 {
            anim_win.start_time =
                Instant::now() + Duration::from_millis(max_delay_ms - anim_win.get_delay());
        }
        anim_win.vel.x *= -1.0;
        anim_win.vel.y *= -1.0;
        anim_win.finished = false;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let travel_distance = screen_width;
        let start_pos = Coord2D::new(rest_pos.x, rest_pos.y + travel_distance);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        let vel = self.max_vel;
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            vel,
            Instant::now() + Duration::from_millis(anim_win.get_delay()),
            None,
        );
        anim_win.finished = false;
    }

    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let end_pos = anim_win.end_pos;
        let mut vel = anim_win.vel;
        cur_pos.y += vel.y as i32;
        if cur_pos.y < end_pos.y {
            cur_pos.y = end_pos.y;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        if cur_pos.y - end_pos.y <= self.slow_down_threshold {
            vel.y *= self.slow_down_ratio;
        }
        if vel.y >= -1.0 {
            vel.y = -1.0;
        }
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let start_pos = anim_win.start_pos;
        let mut vel = anim_win.vel;
        cur_pos.y += vel.y as i32;

        if cur_pos.y > start_pos.y {
            cur_pos.y = start_pos.y;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        let end_pos = anim_win.end_pos;
        if cur_pos.y - end_pos.y <= self.slow_down_threshold {
            vel.y *= self.speed_up_ratio;
        } else {
            vel.y = -self.max_vel.y;
        }
        if vel.y > -self.max_vel.y {
            vel.y = -self.max_vel.y;
        }
        anim_win.vel = vel;
        false
    }
}

struct ProcessAnimateWindowSlideFromBottomTimed {
    max_duration_ms: u64,
}

impl ProcessAnimateWindowSlideFromBottomTimed {
    fn new() -> Self {
        Self {
            max_duration_ms: 1000,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromBottomTimed {
    fn set_max_duration(&mut self, duration_ms: u64) {
        self.max_duration_ms = duration_ms;
    }

    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        _max_delay_ms: u64,
        screen_size: (i32, i32),
    ) {
        let (screen_width, _) = screen_size;
        let rest_pos = anim_win.rest_pos;
        let start_pos = rest_pos;
        let mut cur_pos = start_pos;
        let end_pos = Coord2D::new(rest_pos.x, rest_pos.y + screen_width);
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        let now = Instant::now();
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            Coord2DF::new(0.0, 0.0),
            now,
            Some(now + Duration::from_millis(self.max_duration_ms)),
        );
        anim_win.finished = false;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let start_pos = Coord2D::new(rest_pos.x, rest_pos.y + screen_width);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        let now = Instant::now();
        let delay = anim_win.get_delay();
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            Coord2DF::new(0.0, 0.0),
            now + Duration::from_millis(delay),
            Some(now + Duration::from_millis(self.max_duration_ms + delay)),
        );
        anim_win.finished = false;
    }

    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let end_time = match anim_win.end_time {
            Some(end_time) => end_time,
            None => return true,
        };
        let start_pos = anim_win.start_pos;
        let mut cur_pos = anim_win.cur_pos;
        let end_pos = anim_win.end_pos;
        if now >= end_time {
            cur_pos.y = end_pos.y;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        let elapsed_ms = now.duration_since(anim_win.start_time).as_millis() as f32;
        let percent_done = elapsed_ms / self.max_duration_ms as f32;
        cur_pos.y = start_pos.y + ((end_pos.y - start_pos.y) as f32 * percent_done) as i32;
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        screen_size: (i32, i32),
    ) -> bool {
        self.update_animate_window(anim_win, now, screen_size)
    }
}

struct ProcessAnimateWindowSpiral {
    max_r: f32,
    delta_theta: f32,
}

impl ProcessAnimateWindowSpiral {
    fn new(screen_size: (i32, i32)) -> Self {
        let max_r = (screen_size.0 / 2) as f32;
        Self {
            max_r,
            delta_theta: 0.33,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSpiral {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        max_delay_ms: u64,
        _screen_size: (i32, i32),
    ) {
        if anim_win.get_delay() > 0 {
            anim_win.start_time =
                Instant::now() + Duration::from_millis(max_delay_ms - anim_win.get_delay());
        }
        anim_win.vel.x = 0.0;
        anim_win.vel.y = 0.0;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, _screen_size: (i32, i32)) {
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let vel = Coord2DF::new(0.0, self.max_r);
        let start_pos = Coord2D::new(
            (vel.y * vel.x.cos()) as i32 + end_pos.x,
            (vel.y * vel.x.sin()) as i32 + end_pos.y,
        );
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            vel,
            Instant::now() + Duration::from_millis(anim_win.get_delay()),
            None,
        );
        anim_win.finished = false;
    }

    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let end_pos = anim_win.end_pos;
        let mut vel = anim_win.vel;
        cur_pos.x = (vel.y * vel.x.cos()) as i32 + end_pos.x;
        cur_pos.y = (vel.y * vel.x.sin()) as i32 + end_pos.y;
        vel.x += self.delta_theta;
        vel.y -= 5.0;
        let size = {
            let win = anim_win.window.borrow();
            win.get_size()
        };
        let max_size = min(size.0 / 2, size.1 / 2);
        if vel.y < max_size as f32 {
            let rest_pos = anim_win.rest_pos;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(rest_pos.x, rest_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let end_pos = anim_win.end_pos;
        let mut vel = anim_win.vel;
        cur_pos.x = (vel.y * vel.x.cos()) as i32 + end_pos.x;
        cur_pos.y = (vel.y * vel.x.sin()) as i32 + end_pos.y;
        vel.x -= self.delta_theta;
        vel.y += 5.0;
        if vel.y > self.max_r {
            anim_win.finished = true;
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        anim_win.vel = vel;
        false
    }
}

struct ProcessAnimateWindowSlideFromRightFast {
    max_vel: Coord2DF,
    slow_down_threshold: i32,
    slow_down_ratio: f32,
    speed_up_ratio: f32,
}

impl ProcessAnimateWindowSlideFromRightFast {
    fn new() -> Self {
        let slow_down_ratio = 0.77;
        Self {
            max_vel: Coord2DF::new(-80.0, 0.0),
            slow_down_threshold: 60,
            slow_down_ratio,
            speed_up_ratio: 3.0 - slow_down_ratio,
        }
    }
}

impl ProcessAnimateWindow for ProcessAnimateWindowSlideFromRightFast {
    fn init_reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        max_delay_ms: u64,
        _screen_size: (i32, i32),
    ) {
        if anim_win.get_delay() > 0 {
            anim_win.start_time =
                Instant::now() + Duration::from_millis(max_delay_ms - anim_win.get_delay());
        }
        anim_win.vel.x *= -1.0;
        anim_win.vel.y *= -1.0;
        anim_win.finished = false;
    }

    fn init_animate_window(&self, anim_win: &mut AnimateWindow, screen_size: (i32, i32)) {
        let (screen_width, _) = screen_size;
        let rest_pos = {
            let win = anim_win.window.borrow();
            let (x, y) = win.get_position();
            Coord2D::new(x, y)
        };
        let end_pos = rest_pos;
        let travel_distance = screen_width;
        let start_pos = Coord2D::new(rest_pos.x + travel_distance, rest_pos.y);
        let cur_pos = start_pos;
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(start_pos.x, start_pos.y);
        }
        let vel = self.max_vel;
        anim_win.set_anim_data(
            start_pos,
            end_pos,
            cur_pos,
            rest_pos,
            vel,
            Instant::now() + Duration::from_millis(anim_win.get_delay()),
            None,
        );
        anim_win.finished = false;
    }

    fn update_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let end_pos = anim_win.end_pos;
        let mut vel = anim_win.vel;
        cur_pos.x += vel.x as i32;
        if cur_pos.x < end_pos.x {
            cur_pos.x = end_pos.x;
            anim_win.finished = true;
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        if cur_pos.x - end_pos.x <= self.slow_down_threshold {
            vel.x *= self.slow_down_ratio;
        }
        if vel.x >= -1.0 {
            vel.x = -1.0;
        }
        anim_win.vel = vel;
        false
    }

    fn reverse_animate_window(
        &self,
        anim_win: &mut AnimateWindow,
        now: Instant,
        _screen_size: (i32, i32),
    ) -> bool {
        if anim_win.finished {
            return true;
        }
        if now < anim_win.start_time {
            return false;
        }
        let mut cur_pos = anim_win.cur_pos;
        let start_pos = anim_win.start_pos;
        let mut vel = anim_win.vel;
        cur_pos.x += vel.x as i32;

        if cur_pos.x > start_pos.x {
            cur_pos.x = start_pos.x;
            anim_win.finished = true;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
            return true;
        }
        {
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(cur_pos.x, cur_pos.y);
        }
        anim_win.cur_pos = cur_pos;
        let end_pos = anim_win.end_pos;
        if cur_pos.x - end_pos.x <= self.slow_down_threshold {
            vel.x *= self.speed_up_ratio;
        } else {
            vel.x = -self.max_vel.x;
        }
        if vel.x > -self.max_vel.x {
            vel.x = -self.max_vel.x;
        }
        anim_win.vel = vel;
        false
    }
}

/// Animation window manager for handling screen transitions (C++-accurate).
pub struct AnimateWindowManager {
    win_list: Vec<AnimateWindow>,
    win_must_finish_list: Vec<AnimateWindow>,
    needs_update: bool,
    reverse: bool,
    screen_size: (i32, i32),
    slide_from_right: ProcessAnimateWindowSlideFromRight,
    slide_from_right_fast: ProcessAnimateWindowSlideFromRightFast,
    slide_from_left: ProcessAnimateWindowSlideFromLeft,
    slide_from_top: ProcessAnimateWindowSlideFromTop,
    slide_from_top_fast: ProcessAnimateWindowSlideFromTopFast,
    slide_from_bottom: ProcessAnimateWindowSlideFromBottom,
    slide_from_bottom_timed: ProcessAnimateWindowSlideFromBottomTimed,
    spiral: ProcessAnimateWindowSpiral,
    no_op: ProcessAnimateWindowNoOp,
}

impl AnimateWindowManager {
    pub fn new() -> Self {
        let screen_size = (800, 600);
        Self {
            win_list: Vec::new(),
            win_must_finish_list: Vec::new(),
            needs_update: false,
            reverse: false,
            screen_size,
            slide_from_right: ProcessAnimateWindowSlideFromRight::new(),
            slide_from_right_fast: ProcessAnimateWindowSlideFromRightFast::new(),
            slide_from_left: ProcessAnimateWindowSlideFromLeft::new(),
            slide_from_top: ProcessAnimateWindowSlideFromTop::new(),
            slide_from_top_fast: ProcessAnimateWindowSlideFromTopFast::new(),
            slide_from_bottom: ProcessAnimateWindowSlideFromBottom::new(),
            slide_from_bottom_timed: ProcessAnimateWindowSlideFromBottomTimed::new(),
            spiral: ProcessAnimateWindowSpiral::new(screen_size),
            no_op: ProcessAnimateWindowNoOp,
        }
    }

    pub fn set_screen_size(&mut self, width: i32, height: i32) {
        self.screen_size = (width, height);
        self.spiral = ProcessAnimateWindowSpiral::new(self.screen_size);
    }

    pub fn init(&mut self) {
        self.win_list.clear();
        self.win_must_finish_list.clear();
        self.needs_update = false;
        self.reverse = false;
    }

    pub fn reset(&mut self) {
        self.reset_to_rest_position();
        self.win_list.clear();
        self.win_must_finish_list.clear();
        self.needs_update = false;
        self.reverse = false;
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        let reverse = self.reverse;
        let screen_size = self.screen_size;
        if self.needs_update {
            self.needs_update = false;
            let (
                slide_from_right,
                slide_from_right_fast,
                slide_from_left,
                slide_from_top,
                slide_from_top_fast,
                slide_from_bottom,
                slide_from_bottom_timed,
                spiral,
                no_op,
            ) = (
                &self.slide_from_right,
                &self.slide_from_right_fast,
                &self.slide_from_left,
                &self.slide_from_top,
                &self.slide_from_top_fast,
                &self.slide_from_bottom,
                &self.slide_from_bottom_timed,
                &self.spiral,
                &self.no_op,
            );
            for anim_win in &mut self.win_must_finish_list {
                let process: &dyn ProcessAnimateWindow = match anim_win.anim_type {
                    AnimationType::SlideRight => slide_from_right,
                    AnimationType::SlideRightFast => slide_from_right_fast,
                    AnimationType::SlideLeft => slide_from_left,
                    AnimationType::SlideTop => slide_from_top,
                    AnimationType::SlideTopFast => slide_from_top_fast,
                    AnimationType::SlideBottom => slide_from_bottom,
                    AnimationType::SlideBottomTimed => slide_from_bottom_timed,
                    AnimationType::Spiral => spiral,
                    AnimationType::None => no_op,
                };

                let finished = if reverse {
                    process.reverse_animate_window(anim_win, now, screen_size)
                } else {
                    process.update_animate_window(anim_win, now, screen_size)
                };
                if !finished {
                    self.needs_update = true;
                }
            }
        }

        let (
            slide_from_right,
            slide_from_right_fast,
            slide_from_left,
            slide_from_top,
            slide_from_top_fast,
            slide_from_bottom,
            slide_from_bottom_timed,
            spiral,
            no_op,
        ) = (
            &self.slide_from_right,
            &self.slide_from_right_fast,
            &self.slide_from_left,
            &self.slide_from_top,
            &self.slide_from_top_fast,
            &self.slide_from_bottom,
            &self.slide_from_bottom_timed,
            &self.spiral,
            &self.no_op,
        );
        for anim_win in &mut self.win_list {
            let process: &dyn ProcessAnimateWindow = match anim_win.anim_type {
                AnimationType::SlideRight => slide_from_right,
                AnimationType::SlideRightFast => slide_from_right_fast,
                AnimationType::SlideLeft => slide_from_left,
                AnimationType::SlideTop => slide_from_top,
                AnimationType::SlideTopFast => slide_from_top_fast,
                AnimationType::SlideBottom => slide_from_bottom,
                AnimationType::SlideBottomTimed => slide_from_bottom_timed,
                AnimationType::Spiral => spiral,
                AnimationType::None => no_op,
            };
            if reverse {
                process.reverse_animate_window(anim_win, now, screen_size);
            } else {
                process.update_animate_window(anim_win, now, screen_size);
            }
        }
    }

    pub fn register_window(
        &mut self,
        window: Rc<RefCell<GameWindow>>,
        anim_type: AnimationType,
        needs_to_finish: bool,
        duration_ms: u64,
        delay_ms: u64,
    ) {
        if anim_type == AnimationType::None {
            log::debug!("Ignoring AnimationType::None for animate window registration");
            return;
        }
        let mut anim_win = AnimateWindow::new(window, anim_type, needs_to_finish);
        anim_win.set_delay(delay_ms);
        let screen_size = self.screen_size;
        let process = self.process_for_mut(anim_type);
        process.set_max_duration(duration_ms);
        process.init_animate_window(&mut anim_win, screen_size);
        if needs_to_finish {
            self.win_must_finish_list.push(anim_win);
            self.needs_update = true;
        } else {
            self.win_list.push(anim_win);
        }
    }

    fn process_for(&self, anim_type: AnimationType) -> &dyn ProcessAnimateWindow {
        match anim_type {
            AnimationType::SlideRight => &self.slide_from_right,
            AnimationType::SlideRightFast => &self.slide_from_right_fast,
            AnimationType::SlideLeft => &self.slide_from_left,
            AnimationType::SlideTop => &self.slide_from_top,
            AnimationType::SlideTopFast => &self.slide_from_top_fast,
            AnimationType::SlideBottom => &self.slide_from_bottom,
            AnimationType::SlideBottomTimed => &self.slide_from_bottom_timed,
            AnimationType::Spiral => &self.spiral,
            AnimationType::None => &self.no_op,
        }
    }

    fn process_for_mut(&mut self, anim_type: AnimationType) -> &mut dyn ProcessAnimateWindow {
        match anim_type {
            AnimationType::SlideRight => &mut self.slide_from_right,
            AnimationType::SlideRightFast => &mut self.slide_from_right_fast,
            AnimationType::SlideLeft => &mut self.slide_from_left,
            AnimationType::SlideTop => &mut self.slide_from_top,
            AnimationType::SlideTopFast => &mut self.slide_from_top_fast,
            AnimationType::SlideBottom => &mut self.slide_from_bottom,
            AnimationType::SlideBottomTimed => &mut self.slide_from_bottom_timed,
            AnimationType::Spiral => &mut self.spiral,
            AnimationType::None => &mut self.no_op,
        }
    }

    pub fn reverse_animate_window(&mut self) {
        self.reverse = true;
        self.needs_update = true;
        let screen_size = self.screen_size;
        let mut max_delay = 0;
        for anim_win in &self.win_must_finish_list {
            if anim_win.get_delay() > max_delay {
                max_delay = anim_win.get_delay();
            }
        }

        let (
            slide_from_right,
            slide_from_right_fast,
            slide_from_left,
            slide_from_top,
            slide_from_top_fast,
            slide_from_bottom,
            slide_from_bottom_timed,
            spiral,
            no_op,
        ) = (
            &self.slide_from_right,
            &self.slide_from_right_fast,
            &self.slide_from_left,
            &self.slide_from_top,
            &self.slide_from_top_fast,
            &self.slide_from_bottom,
            &self.slide_from_bottom_timed,
            &self.spiral,
            &self.no_op,
        );
        for anim_win in &mut self.win_must_finish_list {
            let process: &dyn ProcessAnimateWindow = match anim_win.anim_type {
                AnimationType::SlideRight => slide_from_right,
                AnimationType::SlideRightFast => slide_from_right_fast,
                AnimationType::SlideLeft => slide_from_left,
                AnimationType::SlideTop => slide_from_top,
                AnimationType::SlideTopFast => slide_from_top_fast,
                AnimationType::SlideBottom => slide_from_bottom,
                AnimationType::SlideBottomTimed => slide_from_bottom_timed,
                AnimationType::Spiral => spiral,
                AnimationType::None => no_op,
            };
            process.init_reverse_animate_window(anim_win, max_delay, screen_size);
            anim_win.finished = false;
        }

        let (
            slide_from_right,
            slide_from_right_fast,
            slide_from_left,
            slide_from_top,
            slide_from_top_fast,
            slide_from_bottom,
            slide_from_bottom_timed,
            spiral,
            no_op,
        ) = (
            &self.slide_from_right,
            &self.slide_from_right_fast,
            &self.slide_from_left,
            &self.slide_from_top,
            &self.slide_from_top_fast,
            &self.slide_from_bottom,
            &self.slide_from_bottom_timed,
            &self.spiral,
            &self.no_op,
        );
        for anim_win in &mut self.win_list {
            let process: &dyn ProcessAnimateWindow = match anim_win.anim_type {
                AnimationType::SlideRight => slide_from_right,
                AnimationType::SlideRightFast => slide_from_right_fast,
                AnimationType::SlideLeft => slide_from_left,
                AnimationType::SlideTop => slide_from_top,
                AnimationType::SlideTopFast => slide_from_top_fast,
                AnimationType::SlideBottom => slide_from_bottom,
                AnimationType::SlideBottomTimed => slide_from_bottom_timed,
                AnimationType::Spiral => spiral,
                AnimationType::None => no_op,
            };
            process.init_reverse_animate_window(anim_win, 0, screen_size);
            anim_win.finished = false;
        }
    }

    pub fn reset_to_rest_position(&mut self) {
        for anim_win in &mut self.win_must_finish_list {
            let rest_pos = anim_win.rest_pos;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(rest_pos.x, rest_pos.y);
        }
        for anim_win in &mut self.win_list {
            let rest_pos = anim_win.rest_pos;
            let mut win = anim_win.window.borrow_mut();
            let _ = win.set_position(rest_pos.x, rest_pos.y);
        }
    }

    pub fn is_finished(&self) -> bool {
        !self.needs_update
    }

    pub fn is_reversed(&self) -> bool {
        self.reverse
    }

    pub fn is_empty(&self) -> bool {
        self.win_list.is_empty() && self.win_must_finish_list.is_empty()
    }
}

impl Default for AnimateWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

/// The main Shell system for managing menu screens
///
/// This provides a stack-based screen management system where screens can be
/// pushed and popped with proper initialization and shutdown handling.
pub struct Shell {
    /// Stack of screen layouts (top of stack is the active screen)
    screen_stack: Vec<Box<dyn WindowLayout>>,
    /// Maximum number of screens allowed on the stack
    max_stack_size: usize,
    /// Whether the shell is currently active
    is_shell_active: bool,
    /// Whether the shell map background is enabled
    shell_map_on: bool,
    /// Background layout for non-3D shell mode
    background: Option<Box<dyn WindowLayout>>,
    /// Whether to clear the background
    clear_background: bool,
    /// Pending push operation
    pending_push: bool,
    /// Pending pop operation
    pending_pop: bool,
    /// Name of layout to push when pending operation completes
    pending_push_name: String,
    /// Animation window manager
    animate_window_manager: AnimateWindowManager,
    /// Shell menu scheme manager
    scheme_manager: ShellMenuSchemeManager,
    /// Cached special layouts
    save_load_menu_layout: Option<Box<dyn WindowLayout>>,
    popup_replay_layout: Option<Box<dyn WindowLayout>>,
    options_layout: Option<Box<dyn WindowLayout>>,
    /// Whether the shell has been initialized
    initialized: bool,
    /// Shell update timing
    last_update: Instant,
    update_interval: Duration,
}

impl Shell {
    /// Create a new Shell system
    pub fn new() -> Self {
        Self {
            screen_stack: Vec::new(),
            max_stack_size: 16, // MAX_SHELL_STACK from original
            is_shell_active: true,
            shell_map_on: false,
            background: None,
            clear_background: false,
            pending_push: false,
            pending_pop: false,
            pending_push_name: String::new(),
            animate_window_manager: AnimateWindowManager::new(),
            scheme_manager: ShellMenuSchemeManager::new(),
            save_load_menu_layout: None,
            popup_replay_layout: None,
            options_layout: None,
            initialized: false,
            last_update: Instant::now(),
            update_interval: Duration::from_millis(33), // ~30 FPS like original
        }
    }

    /// Push a new screen layout onto the stack
    ///
    /// # Arguments
    /// * `filename` - Path to the layout file to load
    /// * `shutdown_immediate` - Whether to shutdown the current top immediately
    pub fn push(&mut self, filename: &str, shutdown_immediate: bool) -> Result<(), ShellError> {
        if !self.initialized {
            return Err(ShellError::NotInitialized);
        }

        if filename.is_empty() {
            return Err(ShellError::LayoutNotFound("Empty filename".to_string()));
        }

        if self.screen_stack.len() >= self.max_stack_size {
            return Err(ShellError::StackOverflow {
                max: self.max_stack_size,
            });
        }

        close_shell_gamespy_overlays();

        log::debug!(
            "Shell::push({}) - current stack size: {}",
            filename,
            self.screen_stack.len()
        );

        // Set pending push operation
        self.pending_push = true;
        self.pending_push_name = filename.to_string();

        // Get current top of stack
        if let Some(current_top) = self.screen_stack.last_mut() {
            if !current_top.is_hidden() {
                let mut immediate = shutdown_immediate;
                current_top.run_shutdown(&mut immediate)?;

                if immediate {
                    // Complete the shutdown immediately
                    self.shutdown_complete(None, true)?;
                }
            } else {
                // Match C++ Shell::push(): if the top is already hidden, complete the pending
                // push immediately instead of leaving the shell stuck with a latent push request.
                self.shutdown_complete(None, false)?;
            }
        } else {
            // No current top, do push immediately
            self.shutdown_complete(None, false)?;
        }

        Ok(())
    }

    /// Pop the top screen from the stack
    pub fn pop(&mut self) -> Result<(), ShellError> {
        if self.screen_stack.is_empty() {
            return Err(ShellError::EmptyStack);
        }

        close_shell_gamespy_overlays();

        log::debug!(
            "Shell::pop() - current stack size: {}",
            self.screen_stack.len()
        );

        // Set pending pop operation
        self.pending_pop = true;

        // Shutdown the top screen
        if let Some(top) = self.screen_stack.last_mut() {
            let mut immediate_pop = false;
            top.run_shutdown(&mut immediate_pop)?;

            if immediate_pop {
                self.shutdown_complete(None, false)?;
            }
        }

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        Ok(())
    }

    /// Immediately pop the top screen without waiting for shutdown completion
    pub fn pop_immediate(&mut self) -> Result<(), ShellError> {
        if self.screen_stack.is_empty() {
            return Err(ShellError::EmptyStack);
        }

        log::debug!(
            "Shell::pop_immediate() - current stack size: {}",
            self.screen_stack.len()
        );

        // Don't set pending pop - we're doing it immediately
        self.pending_pop = false;

        // Match C++ Shell::popImmediate(): run shutdown while the screen is still the active top,
        // then perform the actual pop through the normal workhorse.
        if let Some(top) = self.screen_stack.last_mut() {
            let mut immediate_pop = true;
            top.run_shutdown(&mut immediate_pop)?;
        }

        self.do_pop(false)?;

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        Ok(())
    }

    /// Get the top screen on the stack
    pub fn top(&mut self) -> Option<&mut (dyn WindowLayout + 'static)> {
        self.screen_stack
            .last_mut()
            .map(move |layout| layout.as_mut())
    }

    /// Get the current number of screens on the stack
    pub fn get_screen_count(&self) -> usize {
        self.screen_stack.len()
    }

    /// Check if the shell is currently active
    pub fn is_shell_active(&self) -> bool {
        self.is_shell_active
    }

    /// Show or hide all shell layouts
    pub fn hide(&mut self, hide: bool) {
        for layout in &mut self.screen_stack {
            layout.hide(hide);
        }

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }
    }

    /// Show the shell (initialize top screen)
    pub fn show_shell(&mut self, run_init: bool) -> Result<(), ShellError> {
        log::debug!("Shell::show_shell(run_init: {})", run_init);

        if get_global_data()
            .map(|data| !data.read().initial_file.is_empty())
            .unwrap_or(false)
        {
            return Ok(());
        }

        if run_init {
            if let Some(layout) = self.screen_stack.last_mut() {
                layout.run_init(None)?;
            }
        }

        let shell_map_enabled = get_global_data()
            .map(|data| data.read().shell_map_on)
            .unwrap_or(false);
        if !shell_map_enabled && self.screen_stack.is_empty() {
            self.push("Menus/MainMenu.wnd", false)?;
        }

        self.is_shell_active = true;
        Ok(())
    }

    /// Hide the shell (shutdown top screen without popping)
    pub fn hide_shell(&mut self) -> Result<(), ShellError> {
        log::debug!("Shell::hide_shell()");

        self.clear_background = true;

        if let Some(layout) = self.screen_stack.last_mut() {
            let mut immediate_pop = true;
            layout.run_shutdown(&mut immediate_pop)?;
        }

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        self.is_shell_active = false;
        Ok(())
    }

    /// Called when a layout has completed its shutdown process
    pub fn shutdown_complete(
        &mut self,
        _layout: Option<&dyn WindowLayout>,
        impending_push: bool,
    ) -> Result<(), ShellError> {
        // Reset animation manager
        self.animate_window_manager.reset();

        if self.pending_push {
            // Do the push
            self.do_push(&self.pending_push_name.clone())?;
            self.pending_push = false;
            self.pending_push_name.clear();
        } else if self.pending_pop {
            // Do the pop
            self.do_pop(impending_push)?;
            self.pending_pop = false;
        }

        if self.clear_background {
            if let Some(mut background) = self.background.take() {
                background.destroy_windows();
                self.clear_background = false;
            }
        }

        Ok(())
    }

    /// Find a screen by its filename
    pub fn find_screen_by_filename(&self, filename: &str) -> Option<&dyn WindowLayout> {
        self.screen_stack
            .iter()
            .find(|layout| layout.get_filename().eq_ignore_ascii_case(filename))
            .map(|layout| layout.as_ref())
    }

    /// Register a window with the animation manager
    pub fn register_with_animate_manager(
        &mut self,
        window: Rc<RefCell<GameWindow>>,
        anim_type: AnimationType,
        needs_to_finish: bool,
        delay_ms: u64,
    ) {
        self.animate_window_manager.register_window(
            window,
            anim_type,
            needs_to_finish,
            500, // Default 500ms duration
            delay_ms,
        );
    }

    /// Check if animations are finished
    pub fn is_anim_finished(&self) -> bool {
        if !with_window_manager_ref(|manager| manager.transitions_finished()) {
            return false;
        }

        let animate_windows = get_global_data()
            .map(|data| data.read().animate_windows)
            .unwrap_or(true);
        if animate_windows {
            self.animate_window_manager.is_finished()
        } else {
            true
        }
    }

    /// Reverse window animations
    pub fn reverse_animate_window(&mut self) {
        self.animate_window_manager.reverse_animate_window();
    }

    /// Check if animations are reversed
    pub fn is_anim_reversed(&self) -> bool {
        self.animate_window_manager.is_reversed()
    }

    /// Load a menu scheme
    pub fn load_scheme(&mut self, name: &str) {
        self.scheme_manager.set_shell_menu_scheme(name);
    }

    /// Get the shell menu scheme manager
    pub fn get_shell_menu_scheme_manager(&mut self) -> &mut ShellMenuSchemeManager {
        &mut self.scheme_manager
    }

    /// Get or create the save/load menu layout
    pub fn get_save_load_menu_layout(&mut self) -> Result<&mut dyn WindowLayout, ShellError> {
        if self.save_load_menu_layout.is_none() {
            let layout = Box::new(BasicWindowLayout::new(
                "Menus/PopupSaveLoad.wnd".to_string(),
            ));
            self.save_load_menu_layout = Some(layout);
        }

        Ok(self.save_load_menu_layout.as_mut().unwrap().as_mut())
    }

    /// Get or create the popup replay layout
    pub fn get_popup_replay_layout(&mut self) -> Result<&mut dyn WindowLayout, ShellError> {
        if self.popup_replay_layout.is_none() {
            let layout = Box::new(BasicWindowLayout::new("Menus/PopupReplay.wnd".to_string()));
            self.popup_replay_layout = Some(layout);
        }

        Ok(self.popup_replay_layout.as_mut().unwrap().as_mut())
    }

    /// Get or create the options layout
    pub fn get_options_layout(
        &mut self,
        create: bool,
    ) -> Option<&mut (dyn WindowLayout + 'static)> {
        if create && self.options_layout.is_none() {
            let layout = Box::new(BasicWindowLayout::new("Menus/OptionsMenu.wnd".to_string()));
            self.options_layout = Some(layout);
        }

        self.options_layout
            .as_mut()
            .map(move |layout| layout.as_mut())
    }

    /// Destroy the options layout
    pub fn destroy_options_layout(&mut self) {
        if let Some(mut layout) = self.options_layout.take() {
            layout.destroy_windows();
        }
    }

    /// Show or hide the shell map
    pub fn show_shell_map(&mut self, use_shell_map: bool) {
        let Some(global) = get_global_data() else {
            return;
        };
        let initial_file_not_empty = !global.read().initial_file.is_empty();
        if initial_file_not_empty {
            return;
        }

        let shell_map_enabled = global.read().shell_map_on;
        if use_shell_map && shell_map_enabled {
            if TheGameLogic::is_in_game() && TheGameLogic::get_game_mode() == GAME_SHELL {
                return;
            }

            if TheGameLogic::is_in_game() {
                let message_stream = get_message_stream();
                let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
                stream.append_message(GameMessageType::ClearGameData);
            }

            let shell_map_name = global.read().shell_map_name.clone();
            {
                let mut data = global.write();
                data.pending_file = shell_map_name;
            }
            init_random_with_seed(0);
            let message_stream = get_message_stream();
            let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
            let msg = stream.append_message(GameMessageType::NewGame);
            msg.append_integer_argument(GAME_SHELL);
            self.shell_map_on = true;
        } else {
            if TheGameLogic::is_in_game() && TheGameLogic::get_game_mode() == GAME_SHELL {
                let message_stream = get_message_stream();
                let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
                stream.append_message(GameMessageType::ClearGameData);
            }

            if !self.is_shell_active {
                return;
            }
            if self.background.is_none() {
                self.background = Some(Box::new(BasicWindowLayout::new(
                    "Menus/BlankWindow.wnd".to_string(),
                )));
            }
            if let Some(ref mut bg) = self.background {
                if let Err(err) = bg.run_init(None) {
                    log::warn!("Failed to initialize shell background layout: {}", err);
                }
                bg.set_first_window_image();
                bg.hide(false);
                if let Some(top) = self.screen_stack.last_mut() {
                    top.bring_forward();
                }
            }
            self.shell_map_on = false;
            self.clear_background = false;
        }

        log::debug!("Shell map enabled: {}", self.shell_map_on);
    }

    fn do_push(&mut self, layout_file: &str) -> Result<(), ShellError> {
        log::debug!("Shell::do_push({})", layout_file);

        // Create new layout - in a real implementation, this would load from file
        let mut new_screen = Box::new(BasicWindowLayout::new(layout_file.to_string()));
        if layout_file.eq_ignore_ascii_case("Menus/MainMenu.wnd") {
            self.load_scheme("MainMenu");
        }

        // Add to stack
        self.screen_stack.push(new_screen);

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        // Initialize the new screen
        if let Some(screen) = self.screen_stack.last_mut() {
            screen.run_init(None)?;
            screen.bring_forward();
        }

        Ok(())
    }

    fn do_pop(&mut self, impending_push: bool) -> Result<(), ShellError> {
        log::debug!("Shell::do_pop(impending_push: {})", impending_push);

        // Remove and destroy the top screen
        if let Some(mut current_top) = self.screen_stack.pop() {
            current_top.destroy_windows();
        }

        // Initialize the new top if present and not doing an impending push
        if !impending_push {
            if let Some(new_top) = self.screen_stack.last_mut() {
                new_top.run_init(None)?;
            }
        }

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        Ok(())
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for Shell {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing shell system");

        // Initialize the scheme manager
        self.scheme_manager.init()?;
        self.last_update = Instant::now();

        self.initialized = true;
        log::info!("Shell system initialized successfully");
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting shell system");

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        // Pop all screens. The local test layouts don't model the C++ callback chain,
        // so we use the immediate pop path to keep the stack teardown deterministic here.
        while !self.screen_stack.is_empty() {
            self.pop_immediate()?;
        }

        // Reset animation manager
        self.animate_window_manager.reset();

        log::info!("Shell system reset completed");
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        if now.duration_since(self.last_update) >= self.update_interval {
            if let Some(name) = PENDING_SHELL_SCHEME.with(|pending| pending.borrow_mut().take()) {
                self.load_scheme(&name);
            }

            // Update all layouts on the stack (from top to bottom)
            for i in (0..self.screen_stack.len()).rev() {
                self.screen_stack[i].run_update(None)?;
            }

            let global_shell_map_on = get_global_data()
                .map(|data| data.read().shell_map_on)
                .unwrap_or(false);
            if global_shell_map_on && self.shell_map_on && self.background.is_some() {
                if let Some(mut background) = self.background.take() {
                    background.destroy_windows();
                }
            }

            self.animate_window_manager.update();
            self.scheme_manager.update()?;

            self.last_update = now;
        }

        Ok(())
    }
}

thread_local! {
    static SHELL: RefCell<Shell> = RefCell::new(Shell::new());
    static PENDING_SHELL_SCHEME: RefCell<Option<String>> = RefCell::new(None);
}

/// Get the global shell instance.
///
/// Returns a `RefMut<'static, Shell>` which implements `Deref<Target = Shell>` and
/// `DerefMut<Target = Shell>`, so all existing call sites that used `ShellGuard`
/// continue to work unchanged.
pub fn get_shell() -> std::cell::RefMut<'static, Shell> {
    SHELL.with(|cell| {
        // SAFETY: The thread_local SHELL lives for the entire duration of the thread.
        // This mirrors the C++ TheShell global pointer pattern where the singleton
        // is alive for the whole process lifetime on the main thread.
        unsafe {
            std::mem::transmute::<std::cell::RefMut<'_, Shell>, std::cell::RefMut<'static, Shell>>(
                cell.borrow_mut(),
            )
        }
    })
}

pub fn request_shell_menu_scheme(name: &str) {
    SHELL.with(|cell| {
        cell.borrow_mut().load_scheme(name);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell as StdRefCell;
    use std::rc::Rc as StdRc;
    use std::sync::{Mutex, OnceLock};

    fn shell_global_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[derive(Clone)]
    struct TestLayout {
        filename: String,
        hidden: bool,
        state: LayoutState,
        events: StdRc<StdRefCell<Vec<String>>>,
    }

    impl TestLayout {
        fn new(filename: &str, hidden: bool, events: StdRc<StdRefCell<Vec<String>>>) -> Self {
            Self {
                filename: filename.to_string(),
                hidden,
                state: LayoutState::Initializing,
                events,
            }
        }
    }

    impl WindowLayout for TestLayout {
        fn get_filename(&self) -> &str {
            &self.filename
        }

        fn run_init(&mut self, _data: Option<&dyn std::any::Any>) -> Result<(), ShellError> {
            self.hidden = false;
            self.state = LayoutState::Active;
            self.events
                .borrow_mut()
                .push(format!("init:{}", self.filename));
            Ok(())
        }

        fn run_update(&mut self, _data: Option<&dyn std::any::Any>) -> Result<(), ShellError> {
            Ok(())
        }

        fn run_shutdown(&mut self, _immediate_pop: &mut bool) -> Result<(), ShellError> {
            self.hidden = true;
            self.state = LayoutState::ShuttingDown;
            self.events
                .borrow_mut()
                .push(format!("shutdown:{}", self.filename));
            Ok(())
        }

        fn hide(&mut self, hide: bool) {
            self.hidden = hide;
            self.events
                .borrow_mut()
                .push(format!("hide:{}:{}", self.filename, hide));
        }

        fn is_hidden(&self) -> bool {
            self.hidden
        }

        fn bring_forward(&mut self) {
            self.events
                .borrow_mut()
                .push(format!("bring_forward:{}", self.filename));
        }

        fn set_first_window_image(&mut self) {
            self.events
                .borrow_mut()
                .push(format!("set_first_window_image:{}", self.filename));
        }

        fn destroy_windows(&mut self) {
            self.state = LayoutState::Destroying;
            self.events
                .borrow_mut()
                .push(format!("destroy:{}", self.filename));
        }

        fn get_state(&self) -> LayoutState {
            self.state
        }

        fn set_state(&mut self, state: LayoutState) {
            self.state = state;
        }
    }

    #[test]
    fn test_shell_creation() {
        let shell = Shell::new();
        assert_eq!(shell.get_screen_count(), 0);
        assert!(shell.is_shell_active());
        assert!(!shell.shell_map_on);
    }

    #[test]
    fn test_shell_init() {
        let mut shell = Shell::new();
        assert!(shell.init().is_ok());
        assert!(shell.initialized);
    }

    #[test]
    fn test_push_before_init() {
        let mut shell = Shell::new();
        let result = shell.push("test.wnd", false);
        assert!(matches!(result, Err(ShellError::NotInitialized)));
    }

    #[test]
    fn test_push_empty_filename() {
        let mut shell = Shell::new();
        shell.init().unwrap();
        let result = shell.push("", false);
        assert!(matches!(result, Err(ShellError::LayoutNotFound(_))));
    }

    #[test]
    fn test_pop_empty_stack() {
        let mut shell = Shell::new();
        shell.init().unwrap();
        let result = shell.pop();
        assert!(matches!(result, Err(ShellError::EmptyStack)));
    }

    #[test]
    fn test_basic_window_layout() {
        let mut layout = BasicWindowLayout::new("test.wnd".to_string());

        assert_eq!(layout.get_filename(), "test.wnd");
        assert!(layout.is_hidden());
        assert_eq!(layout.get_state(), LayoutState::Initializing);

        // Missing .wnd files should fail to initialize instead of silently succeeding.
        assert!(matches!(
            layout.run_init(None),
            Err(ShellError::LayoutError(_))
        ));
        assert!(layout.is_hidden());
        assert_eq!(layout.get_state(), LayoutState::Initializing);

        let mut immediate = false;
        layout.run_shutdown(&mut immediate).unwrap();
        assert!(layout.is_hidden());
        assert_eq!(layout.get_state(), LayoutState::ShuttingDown);
    }

    #[test]
    fn test_coord2d() {
        let coord = Coord2D::new(10, 20);
        assert_eq!(coord.x, 10);
        assert_eq!(coord.y, 20);

        let zero = Coord2D::zero();
        assert_eq!(zero.x, 0);
        assert_eq!(zero.y, 0);
    }

    #[test]
    fn test_color() {
        let color = Color::new(255, 128, 64, 200);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 64);
        assert_eq!(color.a, 200);

        let black = Color::black();
        assert_eq!(black.r, 0);
        assert_eq!(black.a, 255);
    }

    #[test]
    fn test_shell_menu_scheme() {
        let mut scheme = ShellMenuScheme::new("test".to_string());
        assert_eq!(scheme.name, "test");
        assert_eq!(scheme.images.len(), 0);
        assert_eq!(scheme.lines.len(), 0);

        let image = ShellMenuSchemeImage::new(
            "test_image".to_string(),
            Coord2D::new(10, 10),
            Coord2D::new(100, 100),
        );
        scheme.add_image(image);
        assert_eq!(scheme.images.len(), 1);

        let line = ShellMenuSchemeLine::new(
            Coord2D::new(0, 0),
            Coord2D::new(100, 100),
            2,
            Color::white(),
        );
        scheme.add_line(line);
        assert_eq!(scheme.lines.len(), 1);
    }

    #[test]
    fn test_scheme_manager() {
        let mut manager = ShellMenuSchemeManager::new();
        manager.init().unwrap();

        let scheme = manager.new_shell_menu_scheme("test_scheme".to_string());
        assert_eq!(scheme.name, "test_scheme");

        manager.set_shell_menu_scheme("test_scheme");
        // Should not crash when drawing
        manager.draw();
    }

    #[test]
    fn test_scheme_manager_clears_current_scheme_on_empty_name() {
        let mut manager = ShellMenuSchemeManager::new();
        manager.new_shell_menu_scheme("test_scheme".to_string());
        manager.set_shell_menu_scheme("test_scheme");
        assert_eq!(manager.current_scheme.as_deref(), Some("test_scheme"));
        manager.set_shell_menu_scheme("");
        assert!(manager.current_scheme.is_none());
    }

    #[test]
    fn test_scheme_manager_replaces_duplicates_in_cpp_list_order() {
        let mut manager = ShellMenuSchemeManager::new();

        manager.new_shell_menu_scheme("first".to_string());
        manager.new_shell_menu_scheme("second".to_string());
        manager
            .new_shell_menu_scheme("FIRST".to_string())
            .add_line(ShellMenuSchemeLine::new(
                Coord2D::new(1, 2),
                Coord2D::new(3, 4),
                5,
                Color::white(),
            ));

        assert_eq!(manager.scheme_order, vec!["second", "first"]);
        assert_eq!(manager.schemes["first"].lines.len(), 1);
        assert!(manager.schemes["first"].images.is_empty());
    }

    #[test]
    fn test_parse_shell_menu_schemes_replaces_duplicate_blocks() {
        let mut manager = ShellMenuSchemeManager::new();

        manager.parse_shell_menu_schemes(
            r#"
ShellMenuScheme Alpha
  ImagePart
    Position = 1 2
    Size = 3 4
    ImageName = stale
  EndImagePart
End
ShellMenuScheme Beta
End
ShellMenuScheme Alpha
  LinePart
    StartPosition = 5 6
    EndPosition = 7 8
    Color = 4294967295
    Width = 9
  EndLinePart
End
"#,
        );

        assert_eq!(manager.scheme_order, vec!["beta", "alpha"]);
        let alpha = &manager.schemes["alpha"];
        assert!(alpha.images.is_empty());
        assert_eq!(alpha.lines.len(), 1);
        assert_eq!(alpha.lines[0].width, 9);
    }

    #[test]
    fn test_shell_menu_scheme_discovery_uses_deterministic_order() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let _guard = shell_global_test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        struct CwdGuard(PathBuf);
        impl Drop for CwdGuard {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.0);
            }
        }

        struct ModDirGuard(Option<String>);
        impl Drop for ModDirGuard {
            fn drop(&mut self) {
                if let Some(global) = get_global_data() {
                    global.write().mod_dir = self.0.take().unwrap_or_default();
                }
            }
        }

        let original_dir = std::env::current_dir().unwrap();
        let _cwd_guard = CwdGuard(original_dir);

        let temp_root = std::env::temp_dir().join(format!(
            "shell_menu_scheme_order_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp_root.join("Data/INI/Default")).unwrap();
        fs::create_dir_all(temp_root.join("Data/INI")).unwrap();
        fs::create_dir_all(
            temp_root.join("windows_game/extracted_big_files/INIZH/Data/INI/Default"),
        )
        .unwrap();
        fs::create_dir_all(temp_root.join("windows_game/extracted_big_files/INIZH/Data/INI"))
            .unwrap();
        fs::create_dir_all(
            temp_root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Default"),
        )
        .unwrap();
        fs::create_dir_all(temp_root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI"))
            .unwrap();

        for path in [
            temp_root.join("Data/INI/Default/ShellMenuScheme.ini"),
            temp_root.join("Data/INI/ShellMenuScheme.ini"),
            temp_root.join(
                "windows_game/extracted_big_files/INIZH/Data/INI/Default/ShellMenuScheme.ini",
            ),
            temp_root.join("windows_game/extracted_big_files/INIZH/Data/INI/ShellMenuScheme.ini"),
            temp_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Default/ShellMenuScheme.ini",
            ),
            temp_root
                .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/ShellMenuScheme.ini"),
        ] {
            fs::write(path, b"").unwrap();
        }

        std::env::set_current_dir(&temp_root).unwrap();

        let old_mod_dir = if let Some(global) = get_global_data() {
            let mut global = global.write();
            let old = global.mod_dir.clone();
            global.mod_dir.clear();
            Some(old)
        } else {
            None
        };
        let _mod_dir_guard = ModDirGuard(old_mod_dir);

        let files = discover_shell_menu_scheme_ini_files();
        let expected = vec![
            fs::canonicalize(temp_root.join("Data/INI/Default/ShellMenuScheme.ini")).unwrap(),
            fs::canonicalize(temp_root.join("Data/INI/ShellMenuScheme.ini")).unwrap(),
            fs::canonicalize(temp_root.join(
                "windows_game/extracted_big_files/INIZH/Data/INI/Default/ShellMenuScheme.ini",
            ))
            .unwrap(),
            fs::canonicalize(
                temp_root
                    .join("windows_game/extracted_big_files/INIZH/Data/INI/ShellMenuScheme.ini"),
            )
            .unwrap(),
            fs::canonicalize(temp_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Default/ShellMenuScheme.ini",
            ))
            .unwrap(),
            fs::canonicalize(
                temp_root
                    .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/ShellMenuScheme.ini"),
            )
            .unwrap(),
        ];

        assert_eq!(files, expected);
    }

    #[test]
    fn test_animation_manager() {
        let mut manager = AnimateWindowManager::new();
        assert!(manager.is_finished());

        let window = Rc::new(RefCell::new(GameWindow::new()));
        manager.register_window(window, AnimationType::SlideRight, true, 100, 0);
        assert!(!manager.is_finished());

        // Animation should not be finished immediately
        manager.update();
        // Note: In a real test, we'd need to wait for the duration to pass
    }

    #[test]
    fn test_animation_types() {
        let anim = AnimationType::SlideRight;
        assert_eq!(anim, AnimationType::SlideRight);
        assert_ne!(anim, AnimationType::Spiral);
    }

    #[test]
    fn test_window_rect() {
        let rect = WindowRect::new(10, 20, 100, 200);
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 100);
        assert_eq!(rect.height, 200);

        let zero = WindowRect::zero();
        assert_eq!(zero.x, 0);
        assert_eq!(zero.width, 0);
    }

    #[test]
    fn test_global_shell() {
        let mut shell = get_shell();
        assert!(shell.init().is_ok());
        assert!(shell.is_shell_active());
    }

    #[test]
    fn test_shell_special_layouts() {
        let mut shell = Shell::new();
        shell.init().unwrap();

        // Test save/load menu layout
        let _layout = shell.get_save_load_menu_layout().unwrap();

        // Test popup replay layout
        let _layout = shell.get_popup_replay_layout().unwrap();

        // Test options layout
        let layout = shell.get_options_layout(true);
        assert!(layout.is_some());

        shell.destroy_options_layout();
        let layout = shell.get_options_layout(false);
        assert!(layout.is_none());
    }

    #[test]
    fn test_find_screen_by_filename_is_case_insensitive() {
        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.screen_stack.push(Box::new(BasicWindowLayout::new(
            "Menus/MainMenu.wnd".to_string(),
        )));

        assert!(shell
            .find_screen_by_filename("menus/mainmenu.wnd")
            .is_some());
    }

    #[test]
    fn test_shell_show_hide() {
        let _guard = shell_global_test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        game_engine::common::ini::ini_game_data::init_global_data();
        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.initial_file.clear();
            global.shell_map_on = true;
        }
        if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }

        let mut shell = Shell::new();
        shell.init().unwrap();

        // Test hide/show functionality
        shell.hide(true);
        shell.hide(false);

        // Test shell map functionality
        shell.show_shell_map(true);
        assert!(shell.shell_map_on);

        shell.show_shell_map(false);
        assert!(!shell.shell_map_on);
    }

    #[test]
    fn test_show_shell_does_not_push_main_menu_when_shell_map_is_on() {
        let _guard = shell_global_test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        game_engine::common::ini::ini_game_data::init_global_data();
        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.initial_file.clear();
            global.shell_map_on = true;
        }
        if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }

        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.show_shell_map(true);
        shell.show_shell(false).unwrap();
        assert_eq!(shell.get_screen_count(), 0);
    }

    #[test]
    fn test_show_shell_map_reapplies_background_image_status() {
        let _guard = shell_global_test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        game_engine::common::ini::ini_game_data::init_global_data();
        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.initial_file.clear();
            global.shell_map_on = false;
        }

        let events = StdRc::new(StdRefCell::new(Vec::new()));
        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.background = Some(Box::new(TestLayout::new(
            "background.wnd",
            false,
            events.clone(),
        )));

        shell.show_shell_map(false);

        let event_log = events.borrow();
        let image_index = event_log
            .iter()
            .position(|event| event == "set_first_window_image:background.wnd")
            .expect("expected background image status to be reapplied");
        let hide_index = event_log
            .iter()
            .position(|event| event == "hide:background.wnd:false")
            .expect("expected background to be shown");
        assert!(image_index < hide_index);
    }

    #[test]
    fn test_shutdown_complete_keeps_clear_background_when_no_background_exists() {
        let mut shell = Shell::new();
        shell.clear_background = true;

        shell.shutdown_complete(None, false).unwrap();

        assert!(shell.clear_background);
    }

    #[test]
    fn test_reset_keeps_special_layouts_like_cpp() {
        let events = StdRc::new(StdRefCell::new(Vec::new()));
        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.screen_stack.push(Box::new(TestLayout::new(
            "stack.wnd",
            false,
            events.clone(),
        )));
        shell.save_load_menu_layout = Some(Box::new(TestLayout::new(
            "save_load.wnd",
            false,
            events.clone(),
        )));
        shell.popup_replay_layout = Some(Box::new(TestLayout::new(
            "popup_replay.wnd",
            false,
            events.clone(),
        )));
        shell.options_layout = Some(Box::new(TestLayout::new(
            "options.wnd",
            false,
            events.clone(),
        )));
        shell.background = Some(Box::new(TestLayout::new(
            "background.wnd",
            false,
            events.clone(),
        )));
        shell.clear_background = true;
        shell.pending_push = true;
        shell.pending_pop = true;
        shell.pending_push_name = "Menus/MainMenu.wnd".to_string();
        shell.last_update = Instant::now() - shell.update_interval;

        shell.reset().unwrap();

        assert_eq!(shell.get_screen_count(), 0);
        assert!(shell.save_load_menu_layout.is_some());
        assert!(shell.popup_replay_layout.is_some());
        assert!(shell.options_layout.is_some());
        assert!(shell.background.is_some());
        assert!(shell.clear_background);
        assert!(shell.pending_push);
        assert!(!shell.pending_pop);
        assert_eq!(shell.pending_push_name, "Menus/MainMenu.wnd");

        let event_log = events.borrow();
        assert!(event_log.iter().any(|event| event == "destroy:stack.wnd"));
        assert!(!event_log
            .iter()
            .any(|event| event == "destroy:save_load.wnd"));
        assert!(!event_log
            .iter()
            .any(|event| event == "destroy:popup_replay.wnd"));
        assert!(!event_log.iter().any(|event| event == "destroy:options.wnd"));
        assert!(!event_log
            .iter()
            .any(|event| event == "destroy:background.wnd"));
    }

    #[test]
    fn test_push_hidden_top_completes_immediately_like_cpp() {
        let events = StdRc::new(StdRefCell::new(Vec::new()));
        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.screen_stack.push(Box::new(TestLayout::new(
            "hidden_top.wnd",
            true,
            events.clone(),
        )));

        shell.push("Menus/MainMenu.wnd", false).unwrap();

        assert_eq!(shell.get_screen_count(), 2);
        assert!(shell.pending_push_name.is_empty());
        assert!(!shell.pending_push);
        let event_log = events.borrow();
        assert!(
            !event_log
                .iter()
                .any(|event| event == "shutdown:hidden_top.wnd"),
            "hidden top should not run shutdown before immediate push completion"
        );
        assert!(
            !event_log
                .iter()
                .any(|event| event == "hide:hidden_top.wnd:true"),
            "C++ Shell::shutdownComplete() does not re-hide the current top during a pending push"
        );
    }

    #[test]
    fn test_pop_immediate_runs_shutdown_before_destroy() {
        let events = StdRc::new(StdRefCell::new(Vec::new()));
        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.screen_stack.push(Box::new(TestLayout::new(
            "first.wnd",
            false,
            events.clone(),
        )));
        shell
            .screen_stack
            .push(Box::new(TestLayout::new("top.wnd", false, events.clone())));

        shell.pop_immediate().unwrap();

        let event_log = events.borrow();
        let shutdown_index = event_log
            .iter()
            .position(|event| event == "shutdown:top.wnd")
            .unwrap();
        let destroy_index = event_log
            .iter()
            .position(|event| event == "destroy:top.wnd")
            .unwrap();
        assert!(shutdown_index < destroy_index);
        assert_eq!(shell.get_screen_count(), 1);
    }
}
