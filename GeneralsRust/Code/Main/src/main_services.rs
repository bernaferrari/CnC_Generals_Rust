//! Per-instance Main runtime bag (HWND, assets, command system, …).
//!
//! Parallel game instances must not share process OnceLock/Atomic HWND slots.
//! Publish via [`with_main_services`] / [`install_main_services`].

use crate::assets::manager::AssetManager;
use crate::command_system::CommandSystem;
use crate::localization::LocalizationManager;
use crate::network::NetworkInterface;
use crate::save_load::SaveLoadManager;
use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32};
use parking_lot::RwLock as ParkingRwLock;
use std::sync::{Arc, Mutex, RwLock};

/// One game-instance owned Main runtime bag.
pub struct MainServices {
    pub application_instance: AtomicPtr<c_void>,
    pub application_window: AtomicPtr<c_void>,
    pub win32_mouse: AtomicPtr<c_void>,
    pub application_is_windowed: AtomicBool,
    pub message_time: AtomicU32,
    pub launcher_shutdown_requested: AtomicBool,
    pub asset_manager: Mutex<Option<Arc<Mutex<AssetManager>>>>,
    pub command_system: Mutex<Option<Arc<Mutex<CommandSystem>>>>,
    pub localization: Mutex<Option<Arc<ParkingRwLock<LocalizationManager>>>>,
    pub network: Mutex<Option<Arc<RwLock<NetworkInterface>>>>,
    pub save_load_manager: Mutex<Option<Arc<Mutex<SaveLoadManager>>>>,
    pub probe_frame_counter: AtomicU32,
    pub material_probe_frame_counter: AtomicU32,
    pub logged_fallback_lighting: AtomicBool,
    pub logged_force_two_sided: AtomicBool,
    pub retail_game_data_probe: AtomicBool,
    pub show_object_health_gate_logged: AtomicBool,
}

impl MainServices {
    pub fn new() -> Self {
        Self {
            application_instance: AtomicPtr::new(std::ptr::null_mut()),
            application_window: AtomicPtr::new(std::ptr::null_mut()),
            win32_mouse: AtomicPtr::new(std::ptr::null_mut()),
            application_is_windowed: AtomicBool::new(false),
            message_time: AtomicU32::new(0),
            launcher_shutdown_requested: AtomicBool::new(false),
            asset_manager: Mutex::new(None),
            command_system: Mutex::new(None),
            localization: Mutex::new(None),
            network: Mutex::new(None),
            save_load_manager: Mutex::new(None),
            probe_frame_counter: AtomicU32::new(0),
            material_probe_frame_counter: AtomicU32::new(0),
            logged_fallback_lighting: AtomicBool::new(false),
            logged_force_two_sided: AtomicBool::new(false),
            retail_game_data_probe: AtomicBool::new(false),
            show_object_health_gate_logged: AtomicBool::new(false),
        }
    }
}

impl Default for MainServices {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    static ACTIVE_STACK: RefCell<Vec<Arc<MainServices>>> = const { RefCell::new(Vec::new()) };
}

pub fn main_services() -> Arc<MainServices> {
    ACTIVE_STACK.with(|stack| {
        stack
            .borrow()
            .last()
            .cloned()
            .expect("MainServices requires with_main_services / install_main_services")
    })
}

pub fn try_main_services() -> Option<Arc<MainServices>> {
    ACTIVE_STACK.with(|stack| stack.borrow().last().cloned())
}

pub fn install_main_services(services: Arc<MainServices>) -> Option<Arc<MainServices>> {
    ACTIVE_STACK.with(|stack| {
        let mut active = stack.borrow_mut();
        let prior = active.last().cloned();
        active.push(services);
        prior
    })
}

pub fn uninstall_main_services_if_current(services: &Arc<MainServices>) -> bool {
    ACTIVE_STACK.with(|stack| {
        let mut active = stack.borrow_mut();
        if active.last().is_some_and(|head| Arc::ptr_eq(head, services)) {
            active.pop();
            true
        } else {
            false
        }
    })
}

pub fn with_main_services<R>(services: &Arc<MainServices>, f: impl FnOnce() -> R) -> R {
    install_main_services(Arc::clone(services));
    struct Guard(Arc<MainServices>);
    impl Drop for Guard {
        fn drop(&mut self) {
            uninstall_main_services_if_current(&self.0);
        }
    }
    let _g = Guard(Arc::clone(services));
    f()
}

pub struct MainServicesScope {
    services: Arc<MainServices>,
}

impl MainServicesScope {
    pub fn new() -> Self {
        let services = Arc::new(MainServices::new());
        install_main_services(Arc::clone(&services));
        Self { services }
    }

    pub fn services(&self) -> &Arc<MainServices> {
        &self.services
    }
}

impl Default for MainServicesScope {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MainServicesScope {
    fn drop(&mut self) {
        uninstall_main_services_if_current(&self.services);
    }
}
