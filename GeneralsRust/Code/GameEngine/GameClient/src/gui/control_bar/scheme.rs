//! Control Bar Scheme System
//!
//! Rust conversion of ControlBarScheme.cpp - manages control bar visual themes and layouts

use super::{ControlBarAnimation, ControlBarLayout, ControlBarScheme, ControlBarSchemeManager};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Default control bar scheme manager implementation
#[derive(Default)]
pub struct DefaultControlBarSchemeManager {
    current_scheme: Arc<RwLock<Option<Arc<ControlBarScheme>>>>,
    loaded_schemes: Arc<RwLock<HashMap<String, Arc<ControlBarScheme>>>>,
}

impl DefaultControlBarSchemeManager {
    pub fn new() -> Self {
        Self::default()
    }
}

impl super::ControlBarSchemeManager for DefaultControlBarSchemeManager {
    fn load_scheme(&self, scheme_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Loading control bar scheme: {}", scheme_name);

        // Create default scheme for now
        let scheme = Arc::new(ControlBarScheme {
            name: scheme_name.to_string(),
            images: HashMap::new(),
            animations: HashMap::new(),
            layout: ControlBarLayout {
                command_buttons: Vec::new(),
                info_panels: Vec::new(),
                construction_queue: super::QueueLayout {
                    x: 0,
                    y: 0,
                    width: 200,
                    height: 100,
                    max_visible_items: 5,
                },
            },
        });

        self.loaded_schemes
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(scheme_name.to_string(), scheme.clone());
        *self
            .current_scheme
            .write()
            .unwrap_or_else(|e| e.into_inner()) = Some(scheme);

        Ok(())
    }

    fn get_scheme(&self) -> Option<Arc<ControlBarScheme>> {
        self.current_scheme.read().ok()?.clone()
    }

    fn set_scheme(&mut self, scheme: Arc<ControlBarScheme>) {
        *self
            .current_scheme
            .write()
            .unwrap_or_else(|e| e.into_inner()) = Some(scheme);
    }
}

/// Default control bar resizer implementation
#[derive(Default)]
pub struct DefaultControlBarResizer;

impl super::ControlBarResizer for DefaultControlBarResizer {
    fn resize(&self, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
        log::debug!("Resizing control bar to {}x{}", width, height);
        Ok(())
    }

    fn get_optimal_size(&self) -> (u32, u32) {
        (800, 150) // Default control bar size
    }
}

/// Residual: last ControlBar scheme action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualControlBarSchemeAction {
    None = 0,
    Load = 1,
    Get = 2,
    Clear = 3,
}

static RESIDUAL_CBS_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_CBS_LOADED_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static RESIDUAL_CBS_HAS_CURRENT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn residual_cbs_action_store(action: ResidualControlBarSchemeAction) {
    RESIDUAL_CBS_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

fn residual_cbs_manager() -> &'static std::sync::Mutex<DefaultControlBarSchemeManager> {
    static MGR: std::sync::OnceLock<std::sync::Mutex<DefaultControlBarSchemeManager>> =
        std::sync::OnceLock::new();
    MGR.get_or_init(|| std::sync::Mutex::new(DefaultControlBarSchemeManager::new()))
}

/// Residual: last ControlBar scheme residual action.
pub fn residual_control_bar_scheme_last_action() -> ResidualControlBarSchemeAction {
    match RESIDUAL_CBS_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualControlBarSchemeAction::Load,
        2 => ResidualControlBarSchemeAction::Get,
        3 => ResidualControlBarSchemeAction::Clear,
        _ => ResidualControlBarSchemeAction::None,
    }
}

/// Residual: loaded scheme count latch.
pub fn residual_control_bar_scheme_loaded_count() -> usize {
    RESIDUAL_CBS_LOADED_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: has current scheme latch.
pub fn residual_control_bar_scheme_has_current() -> bool {
    RESIDUAL_CBS_HAS_CURRENT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Retail ControlBarScheme.ini names residual (8x6 set).
pub const CONTROL_BAR_SCHEME_NAMES_8X6: &[&str] = &[
    "America8x6",
    "GLA8x6",
    "China8x6",
    "Observer8x6",
    "AmericaSuperWeaponGeneral8x6",
    "AmericaLaserGeneral8x6",
    "AmericaAirForceGeneral8x6",
    "ChinaTankGeneral8x6",
    "ChinaInfantryGeneral8x6",
    "ChinaNukeGeneral8x6",
    "GLAToxinGeneral8x6",
    "GLADemolitionGeneral8x6",
    "GLAStealthGeneral8x6",
    "ChinaBossGeneral8x6",
];

fn residual_cbs_sync(manager: &DefaultControlBarSchemeManager) {
    let n = manager.loaded_schemes.read().map(|m| m.len()).unwrap_or(0);
    RESIDUAL_CBS_LOADED_COUNT.store(n, std::sync::atomic::Ordering::Relaxed);
    let has = manager.get_scheme().is_some();
    RESIDUAL_CBS_HAS_CURRENT.store(has, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: load a named scheme without INI images.
pub fn simulate_control_bar_scheme_load(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let Ok(manager) = residual_cbs_manager().lock() else {
        return false;
    };
    if manager.load_scheme(name).is_err() {
        return false;
    }
    residual_cbs_sync(&manager);
    residual_cbs_action_store(ResidualControlBarSchemeAction::Load);
    residual_control_bar_scheme_has_current()
        && manager
            .get_scheme()
            .map(|s| s.name == name)
            .unwrap_or(false)
}

/// Residual: get current scheme residual.
pub fn simulate_control_bar_scheme_get_current() -> bool {
    let Ok(manager) = residual_cbs_manager().lock() else {
        return false;
    };
    let has = manager.get_scheme().is_some();
    residual_cbs_sync(&manager);
    residual_cbs_action_store(ResidualControlBarSchemeAction::Get);
    has
}

/// Residual: clear loaded schemes residual.
pub fn simulate_control_bar_scheme_clear() -> bool {
    let Ok(manager) = residual_cbs_manager().lock() else {
        return false;
    };
    if let Ok(mut loaded) = manager.loaded_schemes.write() {
        loaded.clear();
    }
    if let Ok(mut current) = manager.current_scheme.write() {
        *current = None;
    }
    residual_cbs_sync(&manager);
    residual_cbs_action_store(ResidualControlBarSchemeAction::Clear);
    !residual_control_bar_scheme_has_current() && residual_control_bar_scheme_loaded_count() == 0
}

/// Residual: load America8x6 + Observer8x6 composite.
pub fn simulate_control_bar_scheme_prepare_default() -> bool {
    if !simulate_control_bar_scheme_clear() {
        return false;
    }
    if !simulate_control_bar_scheme_load("America8x6") {
        return false;
    }
    if !simulate_control_bar_scheme_load("Observer8x6") {
        return false;
    }
    residual_control_bar_scheme_loaded_count() >= 2
        && residual_control_bar_scheme_has_current()
        && simulate_control_bar_scheme_get_current()
}
