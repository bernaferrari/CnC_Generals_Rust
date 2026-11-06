//! Control bar resizer state and policy.
//!
//! Ported from `ControlBarResizer.cpp`.

use std::cell::RefCell;

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
// THREAD: main thread only — window list is instance state on ControlBarResizerLatch
// (no process TLS); RefCell is local to one resizer instance.
#[derive(Debug)]
pub struct IniControlBarResizer {
    windows: RefCell<Vec<ResizerWindow>>,
    base_resolution: (u32, u32),
}

impl Default for IniControlBarResizer {
    fn default() -> Self {
        Self {
            windows: RefCell::new(Vec::new()),
            base_resolution: (800, 600),
        }
    }
}

impl IniControlBarResizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_window(&self, window: ResizerWindow) {
        self.windows.borrow_mut().push(window);
    }

    pub fn clear(&self) {
        self.windows.borrow_mut().clear();
    }

    pub fn window_count(&self) -> usize {
        self.windows.borrow().len()
    }

    pub fn set_base_resolution(&mut self, width: u32, height: u32) {
        self.base_resolution = (width.max(1), height.max(1));
    }
}

impl IniControlBarResizer {
    fn resize(&self, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
        let (base_w, base_h) = self.base_resolution;
        let scale_x = width as f32 / base_w as f32;
        let scale_y = height as f32 / base_h as f32;

        let windows = self.windows.borrow();
        log::trace!(
            "ControlBarResizer resize {} windows to {}x{} (scale {:.3}, {:.3})",
            windows.len(),
            width,
            height,
            scale_x,
            scale_y
        );

        Ok(())
    }

    fn get_optimal_size(&self) -> (u32, u32) {
        self.base_resolution
    }
}

/// Residual: last ControlBar resizer action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ResidualControlBarResizerAction {
    #[default]
    None = 0,
    AddWindow = 1,
    Clear = 2,
    SetBaseResolution = 3,
    Resize = 4,
    GetOptimal = 5,
    Prepare = 6,
}

/// Per-ControlBar resizer latch (was TLS + process atomics).
///
/// C++ ControlBarResizer is instance state on the bar; Rust owns the resizer
/// and residual peels per client so multiple instances never share TLS/statics.
#[derive(Debug, Default)]
pub struct ControlBarResizerLatch {
    pub last_action: ResidualControlBarResizerAction,
    pub resizer: IniControlBarResizer,
}

/// Residual: last ControlBar resizer residual action.
pub fn residual_control_bar_resizer_last_action(
    latch: &ControlBarResizerLatch,
) -> ResidualControlBarResizerAction {
    latch.last_action
}

/// Residual: residual resizer window count latch.
pub fn residual_control_bar_resizer_window_count(latch: &ControlBarResizerLatch) -> usize {
    latch.resizer.window_count()
}

/// Residual: residual base resolution latch.
pub fn residual_control_bar_resizer_base_resolution(latch: &ControlBarResizerLatch) -> (u32, u32) {
    latch.resizer.get_optimal_size()
}

/// Residual: add a named resizer window without applying layout.
pub fn simulate_control_bar_resizer_add_window(
    latch: &mut ControlBarResizerLatch,
    name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    latch.resizer.add_window(ResizerWindow::new(name));
    latch.last_action = ResidualControlBarResizerAction::AddWindow;
    residual_control_bar_resizer_window_count(latch) > 0
}

/// Residual: clear residual resizer windows.
pub fn simulate_control_bar_resizer_clear(latch: &mut ControlBarResizerLatch) -> bool {
    latch.resizer.clear();
    latch.last_action = ResidualControlBarResizerAction::Clear;
    residual_control_bar_resizer_window_count(latch) == 0
}

/// Residual: set base resolution residual (default 800x600 retail).
pub fn simulate_control_bar_resizer_set_base_resolution(
    latch: &mut ControlBarResizerLatch,
    width: u32,
    height: u32,
) -> bool {
    latch.resizer.set_base_resolution(width, height);
    latch.last_action = ResidualControlBarResizerAction::SetBaseResolution;
    residual_control_bar_resizer_base_resolution(latch) == (width.max(1), height.max(1))
}

/// Residual: resize residual without GameWindow apply.
pub fn simulate_control_bar_resizer_resize(
    latch: &mut ControlBarResizerLatch,
    width: u32,
    height: u32,
) -> bool {
    match latch.resizer.resize(width, height) {
        Ok(()) => {
            latch.last_action = ResidualControlBarResizerAction::Resize;
            true
        }
        Err(_) => false,
    }
}

/// Residual: get optimal size residual.
pub fn simulate_control_bar_resizer_get_optimal_size(
    latch: &mut ControlBarResizerLatch,
) -> (u32, u32) {
    let size = latch.resizer.get_optimal_size();
    latch.last_action = ResidualControlBarResizerAction::GetOptimal;
    size
}

/// Residual: clear + base 800x600 + add ControlBarParent + resize composite.
pub fn simulate_control_bar_resizer_prepare_default(latch: &mut ControlBarResizerLatch) -> bool {
    if !simulate_control_bar_resizer_clear(latch) {
        return false;
    }
    if !simulate_control_bar_resizer_set_base_resolution(latch, 800, 600) {
        return false;
    }
    if !simulate_control_bar_resizer_add_window(latch, "ControlBar.wnd:ControlBarParent") {
        return false;
    }
    if !simulate_control_bar_resizer_resize(latch, 1024, 768) {
        return false;
    }
    latch.last_action = ResidualControlBarResizerAction::Prepare;
    residual_control_bar_resizer_window_count(latch) == 1
        && residual_control_bar_resizer_base_resolution(latch) == (800, 600)
}
