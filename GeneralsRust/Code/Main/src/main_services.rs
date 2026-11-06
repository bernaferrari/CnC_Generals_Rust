//! Per-instance Main runtime bag (HWND, assets, command system, …).
//!
//! Parallel game instances must not share process OnceLock/Atomic HWND slots.
//! Publish via [`with_main_services`] / [`install_main_services`].

use crate::assets::archive::{ArchiveFileSystem, BigArchiveFileReader};
use crate::assets::manager::AssetManager;
use crate::command_line::CommandLineArgs;
use crate::command_system::CommandSystem;
use crate::copy_protection::CopyProtection;
use crate::debug_system::DebugSystem;
use crate::game_logic::host_destroy_log::HostDestroyEvent;
use crate::game_logic::host_fire_intent_log::HostFireIntentEvent;
use crate::game_logic::host_production_progress_log::HostProductionProgressEvent;
use crate::localization::LocalizationManager;
use crate::network::NetworkInterface;
use crate::resource::AssetManager as ResourceAssetManager;
use crate::save_load::SaveLoadManager;
use parking_lot::RwLock as ParkingRwLock;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU32, AtomicU8, AtomicUsize};
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
    pub probe_frame_counter: AtomicUsize,
    pub material_probe_frame_counter: AtomicUsize,
    pub logged_fallback_lighting: AtomicBool,
    pub logged_force_two_sided: AtomicBool,
    /// 0 unresolved, 1 present, 2 absent.
    pub retail_game_data_probe: AtomicU8,
    pub show_object_health_gate_logged: AtomicBool,
    pub archive_dir_index: Mutex<HashMap<String, HashMap<String, PathBuf>>>,
    pub resolver_dir_index: Mutex<HashMap<String, HashMap<String, PathBuf>>>,
    pub archive_system: Mutex<Option<Arc<Mutex<ArchiveFileSystem>>>>,
    pub big_archive_reader: Mutex<Option<Arc<BigArchiveFileReader>>>,
    pub low_vertex_color_warnings: AtomicUsize,
    pub explicit_vertex_color_debugs: AtomicUsize,
    pub disabled_color_mask_warnings: AtomicUsize,
    pub startup_model_trace_count: AtomicUsize,
    pub drive_frame_count: AtomicU32,
    pub command_line_args: Mutex<Option<CommandLineArgs>>,
    pub bridges_installed: AtomicBool,
    pub development_mode: AtomicBool,
    pub copy_protection_enabled: AtomicBool,
    pub copy_protection: Mutex<Option<Arc<Mutex<CopyProtection>>>>,
    pub debug_system: Mutex<Option<Arc<DebugSystem>>>,
    pub fow_test_lock: Arc<parking_lot::Mutex<()>>,
    pub resource_asset_manager: Mutex<Option<Arc<ResourceAssetManager>>>,
    pub host_destroy_log: Mutex<Vec<HostDestroyEvent>>,
    pub host_fire_intent_log: Mutex<Vec<HostFireIntentEvent>>,
    pub host_production_progress_log: Mutex<Vec<HostProductionProgressEvent>>,
    pub hulk_max_lifetime_override: AtomicI32,
    pub next_live_popup_generation: AtomicUsize,
    pub debug_camera_move_logs: AtomicUsize,
    pub debug_camera_move_to_logs: AtomicUsize,
    pub host_state_residual_test_env_lock: Arc<parking_lot::Mutex<()>>,
    pub hud_feedback_chain_lock: Arc<parking_lot::Mutex<()>>,
    pub ocl_special_power_peels: Mutex<Option<Arc<Vec<crate::game_logic::host_ocl_special_power::OclSpecialPowerPeel>>>>,
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
            probe_frame_counter: AtomicUsize::new(0),
            material_probe_frame_counter: AtomicUsize::new(0),
            logged_fallback_lighting: AtomicBool::new(false),
            logged_force_two_sided: AtomicBool::new(false),
            retail_game_data_probe: AtomicU8::new(0),
            show_object_health_gate_logged: AtomicBool::new(false),
            archive_dir_index: Mutex::new(HashMap::new()),
            resolver_dir_index: Mutex::new(HashMap::new()),
            archive_system: Mutex::new(None),
            big_archive_reader: Mutex::new(None),
            low_vertex_color_warnings: AtomicUsize::new(0),
            explicit_vertex_color_debugs: AtomicUsize::new(0),
            disabled_color_mask_warnings: AtomicUsize::new(0),
            startup_model_trace_count: AtomicUsize::new(0),
            drive_frame_count: AtomicU32::new(0),
            command_line_args: Mutex::new(None),
            bridges_installed: AtomicBool::new(false),
            development_mode: AtomicBool::new(true),
            copy_protection_enabled: AtomicBool::new(false),
            copy_protection: Mutex::new(None),
            debug_system: Mutex::new(None),
            fow_test_lock: Arc::new(parking_lot::Mutex::new(())),
            resource_asset_manager: Mutex::new(None),  // Option<Arc<ResourceAssetManager>>
            host_destroy_log: Mutex::new(Vec::new()),
            host_fire_intent_log: Mutex::new(Vec::new()),
            host_production_progress_log: Mutex::new(Vec::new()),
            hulk_max_lifetime_override: AtomicI32::new(-1),
            next_live_popup_generation: AtomicUsize::new(1),
            debug_camera_move_logs: AtomicUsize::new(0),
            debug_camera_move_to_logs: AtomicUsize::new(0),
            host_state_residual_test_env_lock: Arc::new(parking_lot::Mutex::new(())),
            hud_feedback_chain_lock: Arc::new(parking_lot::Mutex::new(())),
            ocl_special_power_peels: Mutex::new(None),
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
        let head = stack.borrow().last().cloned();
        #[cfg(test)]
        if head.is_none() {
            // Unit-test convenience: each libtest thread is its own isolated
            // instance and gets an owned MainServices published on first use.
            let services = Arc::new(MainServices::new());
            stack.borrow_mut().push(Arc::clone(&services));
            return services;
        }
        head.expect("MainServices requires with_main_services / install_main_services")
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
