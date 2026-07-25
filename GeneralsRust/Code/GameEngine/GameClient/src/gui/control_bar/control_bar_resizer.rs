//! Control bar resizer state and policy.
//!
//! Ported from `ControlBarResizer.cpp`.

use super::control_bar::ControlBarResizer;
use std::sync::RwLock;

/// Mirrors the C++ `ResizerWindow` data blob.
#[derive(Debug, Clone)]
pub struct ResizerWindow {
    pub name: String,
    pub default_pos: (i32, i32),
    pub default_size: (u32, u32),
    pub alt_pos: (i32, i32),
    pub alt_size: (u32, u32),
}

impl ResizerWindow {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default_pos: (0, 0),
            default_size: (0, 0),
            alt_pos: (0, 0),
            alt_size: (0, 0),
        }
    }
}

/// Runtime control-bar resizer.
///
/// The original C++ implementation applies positions directly to `GameWindow`s.
/// The Rust UI stack is transitioning to layout-driven sizing, so we persist the same authored
/// data here and provide deterministic scaling calculations.
#[derive(Debug)]
pub struct IniControlBarResizer {
    windows: RwLock<Vec<ResizerWindow>>,
    base_resolution: (u32, u32),
}

impl Default for IniControlBarResizer {
    fn default() -> Self {
        Self {
            windows: RwLock::new(Vec::new()),
            base_resolution: (800, 600),
        }
    }
}

impl IniControlBarResizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_window(&self, window: ResizerWindow) {
        if let Ok(mut windows) = self.windows.write() {
            windows.push(window);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut windows) = self.windows.write() {
            windows.clear();
        }
    }

    pub fn window_count(&self) -> usize {
        self.windows.read().map(|w| w.len()).unwrap_or(0)
    }

    pub fn set_base_resolution(&mut self, width: u32, height: u32) {
        self.base_resolution = (width.max(1), height.max(1));
    }
}

impl ControlBarResizer for IniControlBarResizer {
    fn resize(&self, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
        let (base_w, base_h) = self.base_resolution;
        let scale_x = width as f32 / base_w as f32;
        let scale_y = height as f32 / base_h as f32;

        if let Ok(windows) = self.windows.read() {
            log::trace!(
                "ControlBarResizer resize {} windows to {}x{} (scale {:.3}, {:.3})",
                windows.len(),
                width,
                height,
                scale_x,
                scale_y
            );
        }

        Ok(())
    }

    fn get_optimal_size(&self) -> (u32, u32) {
        self.base_resolution
    }
}

/// Residual: last ControlBar resizer action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualControlBarResizerAction {
    None = 0,
    AddWindow = 1,
    Clear = 2,
    SetBaseResolution = 3,
    Resize = 4,
    GetOptimal = 5,
    Prepare = 6,
}

static RESIDUAL_CB_RESIZER_ACTION: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_CB_RESIZER_WINDOW_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static RESIDUAL_CB_RESIZER_BASE_W: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(800);
static RESIDUAL_CB_RESIZER_BASE_H: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(600);

fn residual_cb_resizer_action_store(action: ResidualControlBarResizerAction) {
    RESIDUAL_CB_RESIZER_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

fn residual_cb_resizer() -> &'static std::sync::Mutex<IniControlBarResizer> {
    static RESIZER: std::sync::OnceLock<std::sync::Mutex<IniControlBarResizer>> =
        std::sync::OnceLock::new();
    RESIZER.get_or_init(|| std::sync::Mutex::new(IniControlBarResizer::new()))
}

/// Residual: last ControlBar resizer residual action.
pub fn residual_control_bar_resizer_last_action() -> ResidualControlBarResizerAction {
    match RESIDUAL_CB_RESIZER_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualControlBarResizerAction::AddWindow,
        2 => ResidualControlBarResizerAction::Clear,
        3 => ResidualControlBarResizerAction::SetBaseResolution,
        4 => ResidualControlBarResizerAction::Resize,
        5 => ResidualControlBarResizerAction::GetOptimal,
        6 => ResidualControlBarResizerAction::Prepare,
        _ => ResidualControlBarResizerAction::None,
    }
}

/// Residual: residual resizer window count latch.
pub fn residual_control_bar_resizer_window_count() -> usize {
    RESIDUAL_CB_RESIZER_WINDOW_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: residual base resolution latch.
pub fn residual_control_bar_resizer_base_resolution() -> (u32, u32) {
    (
        RESIDUAL_CB_RESIZER_BASE_W.load(std::sync::atomic::Ordering::Relaxed),
        RESIDUAL_CB_RESIZER_BASE_H.load(std::sync::atomic::Ordering::Relaxed),
    )
}

fn residual_cb_resizer_sync(resizer: &IniControlBarResizer) {
    RESIDUAL_CB_RESIZER_WINDOW_COUNT
        .store(resizer.window_count(), std::sync::atomic::Ordering::Relaxed);
    let (w, h) = resizer.get_optimal_size();
    RESIDUAL_CB_RESIZER_BASE_W.store(w, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_CB_RESIZER_BASE_H.store(h, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: add a named resizer window without applying layout.
pub fn simulate_control_bar_resizer_add_window(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let Ok(resizer) = residual_cb_resizer().lock() else {
        return false;
    };
    resizer.add_window(ResizerWindow::new(name));
    residual_cb_resizer_sync(&resizer);
    residual_cb_resizer_action_store(ResidualControlBarResizerAction::AddWindow);
    residual_control_bar_resizer_window_count() > 0
}

/// Residual: clear residual resizer windows.
pub fn simulate_control_bar_resizer_clear() -> bool {
    let Ok(resizer) = residual_cb_resizer().lock() else {
        return false;
    };
    resizer.clear();
    residual_cb_resizer_sync(&resizer);
    residual_cb_resizer_action_store(ResidualControlBarResizerAction::Clear);
    residual_control_bar_resizer_window_count() == 0
}

/// Residual: set base resolution residual (default 800x600 retail).
pub fn simulate_control_bar_resizer_set_base_resolution(width: u32, height: u32) -> bool {
    let Ok(mut resizer) = residual_cb_resizer().lock() else {
        return false;
    };
    resizer.set_base_resolution(width, height);
    residual_cb_resizer_sync(&resizer);
    residual_cb_resizer_action_store(ResidualControlBarResizerAction::SetBaseResolution);
    residual_control_bar_resizer_base_resolution() == (width.max(1), height.max(1))
}

/// Residual: resize residual without GameWindow apply.
pub fn simulate_control_bar_resizer_resize(width: u32, height: u32) -> bool {
    let Ok(resizer) = residual_cb_resizer().lock() else {
        return false;
    };
    match resizer.resize(width, height) {
        Ok(()) => {
            residual_cb_resizer_action_store(ResidualControlBarResizerAction::Resize);
            true
        }
        Err(_) => false,
    }
}

/// Residual: get optimal size residual.
pub fn simulate_control_bar_resizer_get_optimal_size() -> (u32, u32) {
    let Ok(resizer) = residual_cb_resizer().lock() else {
        return (0, 0);
    };
    let size = resizer.get_optimal_size();
    residual_cb_resizer_action_store(ResidualControlBarResizerAction::GetOptimal);
    size
}

/// Residual: clear + base 800x600 + add ControlBarParent + resize composite.
pub fn simulate_control_bar_resizer_prepare_default() -> bool {
    if !simulate_control_bar_resizer_clear() {
        return false;
    }
    if !simulate_control_bar_resizer_set_base_resolution(800, 600) {
        return false;
    }
    if !simulate_control_bar_resizer_add_window("ControlBar.wnd:ControlBarParent") {
        return false;
    }
    if !simulate_control_bar_resizer_resize(1024, 768) {
        return false;
    }
    residual_cb_resizer_action_store(ResidualControlBarResizerAction::Prepare);
    residual_control_bar_resizer_window_count() == 1
        && residual_control_bar_resizer_base_resolution() == (800, 600)
}
