//! Per-instance system managers (C++ TheXxx on TheGameEngine).
//!
//! Dense Common system bags hang off [`crate::common::common_services::CommonServices`]
//! instead of process LazyLock/OnceCell slots so parallel game instances do not share
//! AI data, upgrade masks, file systems, or team factories.

use crate::common::ini::ini_ai_data::AIDataStore;
use crate::common::ini::ini_damage_fx::FxBonePosQuery;
use crate::common::ini::ini_fx_list::{DispatchedFxNugget, FxListObjRuntime};
use crate::common::ini::ini_game_data::GlobalData;
use crate::common::rts::team::TeamFactory;
use crate::common::system::archive_file_system::ArchiveFileSystem;
use crate::common::system::build_assistant::BuildAssistant;
use crate::common::system::cd_manager::CDManagerInterface;
use crate::common::system::debug::{DebugSystem, Profiler};
use crate::common::system::game_memory::MemoryPoolFactory;
use crate::common::system::local_file_system::LocalFileSystem;
use crate::common::system::radar::RadarSystem;
use crate::common::system::registry::Registry;
use crate::common::system::upgrade::center::UpgradeCenter;
use crate::System::SaveGame::GameState;
use parking_lot::RwLock as ParkingRwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// One game-instance owned system-manager bag.
pub struct SystemStores {
    /// Engine-lifetime AIData (C++ TheAI data while no world is active).
    pub ai_data_lifetime: Arc<RwLock<AIDataStore>>,
    /// Installed world AIData override.
    pub ai_data_active: Mutex<Option<Arc<RwLock<AIDataStore>>>>,
    /// Engine-lifetime UpgradeCenter.
    pub upgrade_center_lifetime: Arc<RwLock<UpgradeCenter>>,
    /// Installed world UpgradeCenter override.
    pub upgrade_center_active: Mutex<Option<Arc<RwLock<UpgradeCenter>>>>,
    /// C++ `TheBuildAssistant`.
    pub build_assistant: Mutex<BuildAssistant>,
    pub build_assistant_ready: AtomicBool,
    /// C++ `TheArchiveFileSystem`.
    pub archive_file_system: Mutex<ArchiveFileSystem>,
    pub archive_file_system_ready: AtomicBool,
    /// C++ `TheLocalFileSystem`.
    pub local_file_system: Mutex<LocalFileSystem>,
    /// C++ `TheCDManager`.
    pub cd_manager: Mutex<Option<Arc<Mutex<Box<dyn CDManagerInterface + Send>>>>>,
    /// ZH registry (JSON-backed config).
    pub zh_registry: Mutex<Registry>,
    /// C++ memory pool factory.
    pub memory_pool_factory: Mutex<MemoryPoolFactory>,
    /// C++ debug system + profiler.
    pub debug_system: Mutex<Option<Arc<Mutex<DebugSystem>>>>,
    pub profiler: Mutex<Option<Arc<Profiler>>>,
    /// C++ `TheTeamFactory`.
    pub team_factory: Arc<Mutex<TeamFactory>>,
    /// FXList::doFXObj live runner.
    pub fx_list_obj_runtime: RwLock<Option<Arc<dyn FxListObjRuntime>>>,
    /// Dispatched FX nugget log (tests / client bridge).
    pub dispatched_fx_nuggets: Mutex<Vec<DispatchedFxNugget>>,
    /// Drawable bone-position query for FX.
    pub fx_bone_pos_runtime: Mutex<Option<FxBonePosQuery>>,
    /// C++ `TheWritableGlobalData` / `TheGlobalData`.
    pub global_data: Mutex<Option<Arc<ParkingRwLock<GlobalData>>>>,
    /// C++ `TheGameState`.
    pub game_state: Mutex<GameState>,
    /// C++ `TheRadar`.
    pub radar_system: Mutex<Option<Arc<RwLock<RadarSystem>>>>,
}

impl SystemStores {
    pub fn new() -> Self {
        let mut upgrade = UpgradeCenter::new();
        upgrade.init();
        Self {
            ai_data_lifetime: Arc::new(RwLock::new(AIDataStore::default())),
            ai_data_active: Mutex::new(None),
            upgrade_center_lifetime: Arc::new(RwLock::new(upgrade)),
            upgrade_center_active: Mutex::new(None),
            build_assistant: Mutex::new(BuildAssistant::new()),
            build_assistant_ready: AtomicBool::new(false),
            archive_file_system: Mutex::new(ArchiveFileSystem::new()),
            archive_file_system_ready: AtomicBool::new(false),
            local_file_system: Mutex::new(LocalFileSystem::new()),
            cd_manager: Mutex::new(None),
            zh_registry: Mutex::new(Registry::new()),
            memory_pool_factory: Mutex::new(MemoryPoolFactory::new()),
            debug_system: Mutex::new(None),
            profiler: Mutex::new(None),
            team_factory: Arc::new(Mutex::new(TeamFactory::new())),
            fx_list_obj_runtime: RwLock::new(None),
            dispatched_fx_nuggets: Mutex::new(Vec::new()),
            fx_bone_pos_runtime: Mutex::new(None),
            global_data: Mutex::new(None),
            game_state: {
                // Avoid get_global_data during CommonServices construction.
                let mut state = GameState::new(std::path::PathBuf::from("UserData/Save"));
                state.init();
                Mutex::new(state)
            },
            radar_system: Mutex::new(None),
        }
    }
}

impl Default for SystemStores {
    fn default() -> Self {
        Self::new()
    }
}
