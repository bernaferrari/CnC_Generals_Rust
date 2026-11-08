//! Per-instance system managers (C++ TheXxx on TheGameEngine).
//!
//! Dense Common system bags hang off [`crate::common::common_services::CommonServices`]
//! instead of process LazyLock/OnceCell slots so parallel game instances do not share
//! AI data, upgrade masks, file systems, or team factories.

use crate::common::ini::ini_ai_data::AIDataStore;
use crate::common::language::LanguageId;
use std::collections::HashMap;
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
use crate::common::system::radar::{
    RadarDataSink, RadarEventFeedback, RadarMapSource, RadarObjectProvider, RadarSystem,
    RadarTerrainPaintSource, RadarWindowSource,
};
use crate::common::system::registry::Registry;
use crate::common::system::upgrade::center::UpgradeCenter;
use crate::common::system::subsystem_interface::SubsystemResult;
use crate::common::game_engine::{GameClientInterface, GameEngine};
use crate::common::global_data::GlobalData as RuntimeGlobalData;
use crate::common::recorder::Recorder;
use crate::common::version::Version;
use crate::common::audio::audio_cache::AudioFileCache;
use crate::common::audio::game_audio::{
    AudioLocalityResolver, AudioManager as GameAudioManager, AudioViewResolver,
    SoundPlaybackHook,
};
use crate::common::audio::gameplay_audio_dispatch::GameplayAudioDispatch;
use crate::common::audio::audio_event_rts::AudioEventOwnerResolver;
use crate::common::audio::game_sounds::AudioShroudResolver;
use parking_lot::Mutex as ParkingMutex;
use ww3d_animation::AnimatedSoundMgr;
use crate::System::SaveGame::{
    GameState, RuntimeIdCounterHooks, SaveLoadLifecycleHooks,
};
use crate::System::SaveGame::game_state::PostLoadHooks;
use crate::common::thing::thing::ThingTerrainProviders;
use crate::common::thing::thing_factory::ThingBridgeSlots;
use parking_lot::RwLock as ParkingRwLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
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
    pub build_assistant: Arc<ParkingMutex<BuildAssistant>>,
    pub build_assistant_ready: AtomicBool,
    /// C++ `TheArchiveFileSystem`.
    pub archive_file_system: Arc<ParkingMutex<ArchiveFileSystem>>,
    pub archive_file_system_ready: AtomicBool,
    /// C++ `TheLocalFileSystem`.
    pub local_file_system: Arc<ParkingMutex<LocalFileSystem>>,
    /// C++ `TheCDManager`.
    pub cd_manager: Mutex<Option<Arc<Mutex<Box<dyn CDManagerInterface + Send>>>>>,
    /// ZH registry (JSON-backed config).
    pub zh_registry: Mutex<Registry>,
    /// C++ memory pool factory.
    pub memory_pool_factory: ParkingMutex<MemoryPoolFactory>,
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
    pub game_state: Arc<ParkingMutex<GameState>>,
    /// C++ `TheRadar`.
    pub radar_system: Mutex<Option<Arc<RwLock<RadarSystem>>>>,
    /// Language runtime (was process LANGUAGE_RUNTIME).
    pub language_current: Mutex<LanguageId>,
    pub language_strings: Mutex<HashMap<String, String>>,
    /// Radar bridge hooks (was process OnceLocks).
    pub radar_feedback: Mutex<Option<Arc<dyn RadarEventFeedback>>>,
    pub radar_object_provider: Mutex<Option<Arc<dyn RadarObjectProvider>>>,
    pub radar_data_sink: Mutex<Option<Arc<dyn RadarDataSink>>>,
    pub radar_map_source: Mutex<Option<Arc<dyn RadarMapSource>>>,
    pub radar_window_source: Mutex<Option<Arc<dyn RadarWindowSource>>>,
    pub radar_paint_source: Mutex<Option<Arc<dyn RadarTerrainPaintSource>>>,
    /// SaveGame runtime ID counter hooks.
    pub save_id_counter_hooks: Arc<Mutex<RuntimeIdCounterHooks>>,
    /// SaveGame load lifecycle hooks.
    pub save_load_lifecycle_hooks: Arc<Mutex<SaveLoadLifecycleHooks>>,
    /// GameLOD client rebuild / adjust hooks (was process OnceLocks).
    pub rebuild_shadows: Mutex<Option<fn()>>,
    pub rebuild_shoreline: Mutex<Option<fn()>>,
    pub rebuild_tank_tracks: Mutex<Option<fn()>>,
    pub adjust_client_lod: Mutex<Option<fn(i32)>>,
    pub game_lod_ini_loaded: Mutex<bool>,
    /// GameEngine client factory / blank-window / instance slots.
    pub game_client_factory: Arc<
        ParkingMutex<
            Option<
                Arc<
                    dyn Fn() -> SubsystemResult<Box<dyn GameClientInterface>>
                        + Send
                        + Sync
                        + 'static,
                >,
            >,
        >,
    >,
    pub command_list_init: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    pub show_blank_window: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    pub hide_blank_window: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    pub game_engine_instance: Arc<ParkingMutex<Option<Arc<ParkingMutex<GameEngine>>>>>,
    /// Legacy `common::global_data` registry (distinct from INI GlobalData).
    pub runtime_global_data: Arc<ParkingRwLock<RuntimeGlobalData>>,
    pub global_data_test_lock: Arc<ParkingMutex<()>>,
    pub recorder: Mutex<Option<Arc<Mutex<Recorder>>>>,
    pub version: Mutex<Option<Arc<ParkingRwLock<Version>>>>,
    /// C++ TheAudio — owned per CommonServices instance.
    pub the_audio: Mutex<Option<Arc<Mutex<GameAudioManager>>>>,
    /// WW3D animated-sound definition bag (per CommonServices).
    pub animated_sound_mgr: Arc<AnimatedSoundMgr>,
    pub sound_playback_hook: Mutex<Option<Arc<dyn SoundPlaybackHook>>>,
    /// GameLogic's default audio resolvers are installed once per receiving service bag.
    pub game_logic_audio_resolvers_installed: std::sync::OnceLock<()>,
    pub audio_locality_resolver: Mutex<Option<Arc<dyn AudioLocalityResolver>>>,
    pub audio_view_resolver: Mutex<Option<Arc<dyn AudioViewResolver>>>,
    pub play_failure_reports: Mutex<std::collections::HashMap<String, String>>,
    pub live_audio_file_cache: Arc<AudioFileCache>,
    /// Rodio default output stream slot (non-wasm). Stored as opaque mutex payload
    /// so system_stores does not depend on rodio types at the type level.
    pub rodio_stream_slot: Mutex<Option<Box<dyn std::any::Any + Send>>>,
    pub gameplay_audio_dispatch: Mutex<Option<Arc<dyn GameplayAudioDispatch>>>,
    pub audio_event_owner_resolver: Mutex<Option<Arc<dyn AudioEventOwnerResolver>>>,
    pub audio_shroud_resolver: Mutex<Option<Arc<dyn AudioShroudResolver>>>,
    pub speech_registry: Arc<ParkingRwLock<std::collections::HashMap<String, crate::common::audio::game_speech::Speech>>>,
    pub effects_next_handle: std::sync::atomic::AtomicU32,
    /// Type-erased published terrain visual of the live GameClient (C++
    /// `TheTerrainVisual`); GameClient downcasts.
    pub terrain_visual_any: Mutex<Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>>,
    /// SaveGame post-load partition / world-dict hooks.
    pub post_load_hooks: Arc<Mutex<PostLoadHooks>>,
    /// C++ ThePlayerList academy peer counters (advice #31/#32).
    pub academy_local_player_mines: AtomicU32,
    pub academy_neutral_vehicles_sniped: AtomicU32,
    /// Thing terrain provider bridges (GameLogic registration).
    pub thing_terrain: Arc<ThingTerrainProviders>,
    /// ThingFactory object/drawable create bridges + ObjectCreateOverride leftovers.
    pub thing_bridge: Arc<ThingBridgeSlots>,
}

impl SystemStores {
    pub fn new() -> Self {
        // Defer UpgradeCenter::init until CommonServices is published —
        // veterancy templates call NameKeyGenerator::name_to_key, which needs
        // the active CommonServices bind (install_common_services). See
        // CommonServices::finish_bootstrap_after_publish.
        let upgrade = UpgradeCenter::new();
        Self {
            ai_data_lifetime: Arc::new(RwLock::new(AIDataStore::default())),
            ai_data_active: Mutex::new(None),
            upgrade_center_lifetime: Arc::new(RwLock::new(upgrade)),
            upgrade_center_active: Mutex::new(None),
            build_assistant: Arc::new(ParkingMutex::new(BuildAssistant::new())),
            build_assistant_ready: AtomicBool::new(false),
            archive_file_system: Arc::new(ParkingMutex::new(ArchiveFileSystem::new())),
            archive_file_system_ready: AtomicBool::new(false),
            local_file_system: Arc::new(ParkingMutex::new(LocalFileSystem::new())),
            cd_manager: Mutex::new(None),
            zh_registry: Mutex::new(Registry::new()),
            memory_pool_factory: ParkingMutex::new(MemoryPoolFactory::new()),
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
                Arc::new(ParkingMutex::new(state))
            },
            radar_system: Mutex::new(None),
            language_current: Mutex::new(LanguageId::Us),
            language_strings: Mutex::new(HashMap::new()),
            radar_feedback: Mutex::new(None),
            radar_object_provider: Mutex::new(None),
            radar_data_sink: Mutex::new(None),
            radar_map_source: Mutex::new(None),
            radar_window_source: Mutex::new(None),
            radar_paint_source: Mutex::new(None),
            save_id_counter_hooks: Arc::new(Mutex::new(RuntimeIdCounterHooks::default())),
            save_load_lifecycle_hooks: Arc::new(Mutex::new(SaveLoadLifecycleHooks::default())),
            rebuild_shadows: Mutex::new(None),
            rebuild_shoreline: Mutex::new(None),
            rebuild_tank_tracks: Mutex::new(None),
            adjust_client_lod: Mutex::new(None),
            game_lod_ini_loaded: Mutex::new(false),
            game_client_factory: Arc::new(ParkingMutex::new(None)),
            command_list_init: Mutex::new(None),
            show_blank_window: Mutex::new(None),
            hide_blank_window: Mutex::new(None),
            game_engine_instance: Arc::new(ParkingMutex::new(None)),
            runtime_global_data: Arc::new(ParkingRwLock::new(RuntimeGlobalData::default())),
            global_data_test_lock: Arc::new(ParkingMutex::new(())),
            recorder: Mutex::new(None),
            version: Mutex::new(None),
            the_audio: Mutex::new(None),
            animated_sound_mgr: Arc::new(AnimatedSoundMgr::new()),
            sound_playback_hook: Mutex::new(None),
            game_logic_audio_resolvers_installed: std::sync::OnceLock::new(),
            audio_locality_resolver: Mutex::new(None),
            audio_view_resolver: Mutex::new(None),
            play_failure_reports: Mutex::new(std::collections::HashMap::new()),
            live_audio_file_cache: Arc::new(AudioFileCache::new(16 * 1024 * 1024)),
            rodio_stream_slot: Mutex::new(None),
            gameplay_audio_dispatch: Mutex::new(None),
            audio_event_owner_resolver: Mutex::new(None),
            audio_shroud_resolver: Mutex::new(None),
            speech_registry: Arc::new(ParkingRwLock::new(std::collections::HashMap::new())),
            effects_next_handle: std::sync::atomic::AtomicU32::new(10000),
            terrain_visual_any: Mutex::new(None),
            post_load_hooks: Arc::new(Mutex::new(PostLoadHooks::default())),
            academy_local_player_mines: AtomicU32::new(0),
            academy_neutral_vehicles_sniped: AtomicU32::new(0),
            thing_terrain: Arc::new(ThingTerrainProviders::default()),
            thing_bridge: Arc::new(ThingBridgeSlots::default()),
        }
    }

    /// Finish UpgradeCenter lifetime init once CommonServices is on the TLS stack.
    /// Idempotent: UpgradeCenter::init skips names that already exist.
    pub fn init_lifetime_upgrades_after_common_published(&self) {
        let mut upgrade = self
            .upgrade_center_lifetime
            .write()
            .unwrap_or_else(|p| p.into_inner());
        upgrade.init();
    }

}

impl Default for SystemStores {
    fn default() -> Self {
        Self::new()
    }
}
