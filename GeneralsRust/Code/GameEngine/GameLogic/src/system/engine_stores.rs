//! C++ engine services separated from the standalone Core AI runtime.
//!
//! Main owns `WorldServices`: content upgrades, shroud and client visuals.
//! Constructing it never constructs Core AI or selects a world. Core's
//! reference runtime owns `EngineStores`, which adds mandatory AI and AIData.
//! The existing publication stack remains a bounded migration aid for legacy
//! presentation callbacks. It carries services independently of Core runtime;
//! asking for Core AI inside a Main service scope is an explicit error.
//! Unscoped `active` / `active_services` panic: there is no process-wide
//! fallback bag. Prefer passing `Arc<WorldServices>` / `EngineStores`.
use crate::ai::AI;
use crate::helpers::{ClientVisualState, EvaState, InGameUIState};
use crate::special_power_module::area_damage::DamageOverTimeField;
use crate::system::game_logic::RadarUpdate;
use crate::system::shroud_manager::ShroudManager;
use crate::system::world_logic_managers::WorldLogicManagers;
use crate::upgrade::center::UpgradeCenter;
use game_engine::common::common_services::{self, CommonServices};
use game_engine::common::ini::ini_ai_data::{self, AIDataStore};
use game_engine::common::system::upgrade as common_upgrade;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicUsize};
use std::sync::{Arc, LazyLock, Mutex, RwLock, OnceLock};

/// Per-world session flags formerly process atomics on `TheGameLogic` bridge.
pub struct SessionLogicFlags {
    pub game_paused: AtomicBool,
    pub game_pause_music: AtomicBool,
    pub intro_movie_playing: AtomicBool,
    pub start_new_game_requested: AtomicBool,
    pub game_start_rank_points: AtomicI32,
    pub global_difficulty: AtomicI32,
    pub local_allied_victory: AtomicBool,
    pub local_allied_defeat: AtomicBool,
    pub single_alliance_remaining: AtomicBool,
    pub victory_flags_from_live: AtomicBool,
    pub local_player_defeated: AtomicBool,
    pub hulk_max_lifetime_override: AtomicI32,
    pub input_enabled: AtomicBool,
    pub radar_attack_glow: AtomicBool,
    pub script_temp_group_id: AtomicU32,
    /// C++ ScriptGroup default-name counter (was process SCRIPT_GROUP_ID).
    pub script_group_name_id: AtomicU32,
    /// 0 = not inside GameLogic::update; else frame.saturating_add(1).
    pub in_update_frame_plus_one: AtomicU32,
    /// Pending SetRankLevelLimit while update holds &mut self; i32::MIN = none.
    pub pending_rank_limit: AtomicI32,
    /// Published rank level cap (was process RANK_LEVEL_LIMIT_ATOMIC).
    pub rank_level_limit: AtomicI32,
    pub rogue_eva_disable_warned: AtomicBool,
    pub world_epoch: AtomicU32,
    pub next_formation_id: AtomicU32,
    pub next_command_id: AtomicU32,
    pub next_script_engine_instance_id: AtomicU64,
    pub object_manager_reset_count: AtomicUsize,
    pub next_drawable_id: AtomicU32,
}

impl Default for SessionLogicFlags {
    fn default() -> Self {
        Self {
            game_paused: AtomicBool::new(false),
            game_pause_music: AtomicBool::new(false),
            intro_movie_playing: AtomicBool::new(false),
            start_new_game_requested: AtomicBool::new(false),
            game_start_rank_points: AtomicI32::new(0),
            global_difficulty: AtomicI32::new(0),
            local_allied_victory: AtomicBool::new(false),
            local_allied_defeat: AtomicBool::new(false),
            single_alliance_remaining: AtomicBool::new(false),
            victory_flags_from_live: AtomicBool::new(false),
            local_player_defeated: AtomicBool::new(false),
            hulk_max_lifetime_override: AtomicI32::new(-1),
            input_enabled: AtomicBool::new(true),
            radar_attack_glow: AtomicBool::new(false),
            script_temp_group_id: AtomicU32::new(1),
            script_group_name_id: AtomicU32::new(0),
            in_update_frame_plus_one: AtomicU32::new(0),
            pending_rank_limit: AtomicI32::new(i32::MIN),
            rank_level_limit: AtomicI32::new(1000),
            rogue_eva_disable_warned: AtomicBool::new(false),
            world_epoch: AtomicU32::new(0),
            next_formation_id: AtomicU32::new(1),
            next_command_id: AtomicU32::new(1),
            next_script_engine_instance_id: AtomicU64::new(1),
            object_manager_reset_count: AtomicUsize::new(0),
            next_drawable_id: AtomicU32::new(1),
        }
    }
}

/// Service values shared by Main and the standalone Core runtime. No Core AI.
pub struct WorldServices {
    shroud: Arc<Mutex<ShroudManager>>,
    client_visuals: ClientVisualState,
    terrain: Arc<RwLock<crate::terrain::TerrainLogic>>,
    /// Pending radar updates for this world (C++ logic→client fan-out).
    radar_updates: Mutex<VecDeque<RadarUpdate>>,
    /// Host-script request drains for this world (formerly process TLS).
    host_script_requests: crate::scripting::executor::HostScriptRequestQueues,
    session_flags: SessionLogicFlags,
    /// Per-world Common TheXxx bag (RNG, ThingFactory, CRC flags).
    common: Arc<CommonServices>,
    logic_managers: WorldLogicManagers,
    players: Arc<RwLock<crate::player::PlayerList>>,
    objects: Arc<RwLock<crate::object_manager::ObjectManager>>,
    in_game_ui: RwLock<InGameUIState>,
    eva: Arc<Mutex<EvaState>>,
    control_bar_ui_hooks: Mutex<Option<std::sync::Arc<dyn crate::control_bar::ControlBarUiHooks>>>,
    game_pause_hooks: Mutex<Option<std::sync::Arc<dyn crate::helpers::GamePauseHooks>>>,
    observer_audio_locality_hooks: Mutex<Option<std::sync::Arc<dyn crate::helpers::ObserverAudioLocalityHooks>>>,
    observer_audio_view_hooks: Mutex<Option<std::sync::Arc<dyn crate::helpers::ObserverAudioViewHooks>>>,
    scene_submission: Mutex<Option<std::sync::Arc<dyn game_engine::common::system::scene_submission::SceneSubmission>>>,
    fx_list_manager: Mutex<Option<std::sync::Arc<dyn crate::common::types::FXListManagerInterface>>>,
    particle_system_manager: Mutex<Option<std::sync::Arc<dyn crate::common::types::ParticleSystemManagerInterface>>>,
    scorch_hook: Mutex<Option<crate::helpers::ScorchHook>>,
    terrain_unit_moved_hook: Mutex<Option<crate::helpers::TerrainUnitMovedHook>>,
    terrain_tree_hook: Mutex<Option<crate::helpers::TerrainTreeHook>>,
    animation_metadata_hook: Mutex<Option<crate::helpers::AnimationMetadataHook>>,
    terrain_decal_client: Mutex<Option<std::sync::Arc<dyn crate::object::draw::TerrainDecalClient>>>,
    terrain_track_client: Mutex<Option<std::sync::Arc<dyn crate::object::draw::TerrainTrackClient>>>,
    object_registry: std::sync::Arc<crate::object::registry::ObjectRegistry>,
    unit_registry: std::sync::Arc<std::sync::RwLock<crate::object::unit::UnitRegistry>>,
    legacy_object_registry: std::sync::Arc<crate::ai::object_registry::LegacyObjectRegistryFacade>,
    locomotor_store: std::sync::Arc<crate::locomotor::LocomotorStore>,
    /// Main world-services GameLogic slot (was process PROCESS_LIFETIME Core).
    main_game_logic: OnceLock<&'static Mutex<crate::system::game_logic::GameLogic>>,
    /// Headless GameClient ImageCollection when no live client is registered
    /// (type-erased `Arc<RwLock<ImageCollection>>` owned by the GameClient crate).
    client_image_collection: Mutex<Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>>,
    control_bar_bridge: RwLock<Option<std::sync::Arc<crate::control_bar::ControlBarBridge>>>,
    damage_over_time_fields: Mutex<Vec<DamageOverTimeField>>,
    damage_over_time_next_id: AtomicU32,
    host_script_query: Mutex<crate::scripting::conditions::HostScriptQuerySnapshot>,
    cur_update_module: Mutex<Option<crate::modules::UpdateModulePtr>>,
    selection: Arc<RwLock<crate::commands::selection::SelectionManager>>,
    physics: Arc<RwLock<crate::physics::PhysicsEngine>>,
    sides: Arc<RwLock<crate::sides_list::SidesList>>,
    rts_actions: Arc<RwLock<game_engine::common::rts::ActionManager>>,
    ghost_objects: Arc<RwLock<crate::object::ghost_object::GhostObjectManager>>,
    w3d_ghost_objects: Arc<RwLock<crate::object::w3d_ghost_object::W3DGhostObjectManager>>,
    taxi_to_takeoff: Mutex<Vec<crate::common::ObjectID>>,
    transport_statuses: Mutex<Vec<(u32, u32, i32)>>,
    pending_team_script_events: Mutex<Vec<crate::team::PendingTeamScriptEvent>>,
    named_evac_disposition: Mutex<std::collections::HashMap<String, u32>>,
    player_money: Arc<RwLock<crate::special_power_module::player_money::PlayerMoneyManager>>,
    player_science: Arc<RwLock<crate::special_power_module::player_science::PlayerScienceManager>>,
    special_power_integration: Arc<RwLock<crate::special_power_module::integration::SpecialPowerIntegrationContext>>,
    special_power_registry: Arc<RwLock<crate::special_power_module::registry::SpecialPowerRegistry>>,
    map_string_overlay: RwLock<std::collections::HashMap<String, String>>,
    armor_store: Arc<RwLock<crate::object::armor::ArmorStore>>,
    special_power_store: Arc<RwLock<crate::object::special_power_template::SpecialPowerStore>>,
    tech_tree: Arc<RwLock<crate::upgrade::prerequisites::TechTree>>,
    rank_info_store: Arc<RwLock<crate::system::rank_info::RankInfoStore>>,
    weapon_store: Arc<RwLock<Option<crate::weapon::WeaponStore>>>,
    ocl_store: Arc<RwLock<Option<crate::object_creation_list::store::ObjectCreationListStore>>>,
    hlod_live_child_capture_hook: RwLock<Option<crate::object::draw::w3d_model_draw::HlodLiveChildCaptureHook>>,
    committed_live_states: Mutex<std::collections::HashMap<crate::common::ObjectID, Vec<crate::object::draw::w3d_model_draw::HlodLiveChildState>>>,
    model_obj_bounds_hook: RwLock<Option<crate::object::draw::w3d_model_draw::ModelBoundsHook>>,
    pristine_bone_lookup_hook: RwLock<Option<crate::object::draw::w3d_model_draw::PristineBoneLookupHook>>,
    sub_object_name_hook: RwLock<Option<crate::object::draw::w3d_model_draw::SubObjectNameHook>>,
    w3d_ghost_snapshot_capture_hook: RwLock<Option<crate::object::w3d_ghost_object::W3DGhostSnapshotCaptureHook>>,
    w3d_ghost_snapshot_capture_source_hook: RwLock<Option<crate::object::w3d_ghost_object::W3DGhostSnapshotCaptureSourceHook>>,



    /// Nested world-stage depth for the published services (was process TLS).
    staging_depth: AtomicU32,
    staging_deferred_team_effects:
        Mutex<Vec<crate::team::TeamFactoryDeferredEffects>>,
    /// Serializes host staging that still touches process-wide wrappers.
    runtime_transaction_lock: Arc<parking_lot::Mutex<()>>,
}
#[cfg(any(test, feature = "test-ambient-services"))]
thread_local! {
    static TEST_ADOPT_COMMON: std::cell::RefCell<Option<Arc<CommonServices>>> =
        const { std::cell::RefCell::new(None) };
}
#[cfg(any(test, feature = "test-ambient-services"))]
fn fresh_common_for_world() -> Arc<CommonServices> {
    TEST_ADOPT_COMMON
        .try_with(|slot| slot.borrow().clone())
        .ok()
        .flatten()
        .unwrap_or_else(|| Arc::new(CommonServices::new()))
}
#[cfg(not(any(test, feature = "test-ambient-services")))]
fn fresh_common_for_world() -> Arc<CommonServices> {
    Arc::new(CommonServices::new())
}
impl WorldServices {
    fn engine_defaults() -> Self {
        Self {
            shroud: Arc::new(Mutex::new(ShroudManager::new())),
            client_visuals: ClientVisualState::default(),
            terrain: Arc::clone(&crate::terrain::THE_TERRAIN_LOGIC),
            radar_updates: Mutex::new(VecDeque::new()),
            host_script_requests: crate::scripting::executor::HostScriptRequestQueues::default(),
            session_flags: SessionLogicFlags::default(),
            common: Arc::new(CommonServices::new()),
            logic_managers: WorldLogicManagers::default(),
            players: Arc::new(RwLock::new(crate::player::PlayerList::new())),
            objects: Arc::new(RwLock::new(crate::object_manager::ObjectManager::new())),
            in_game_ui: RwLock::new(InGameUIState::default()),
            eva: Arc::new(Mutex::new(EvaState::new_enabled())),
            control_bar_ui_hooks: Mutex::new(None),
            game_pause_hooks: Mutex::new(None),
            observer_audio_locality_hooks: Mutex::new(None),
            observer_audio_view_hooks: Mutex::new(None),
            scene_submission: Mutex::new(None),
            fx_list_manager: Mutex::new(None),
            particle_system_manager: Mutex::new(None),
            scorch_hook: Mutex::new(None),
            terrain_unit_moved_hook: Mutex::new(None),
            terrain_tree_hook: Mutex::new(None),
            animation_metadata_hook: Mutex::new(None),
            terrain_decal_client: Mutex::new(None),
            terrain_track_client: Mutex::new(None),
            object_registry: std::sync::Arc::new(crate::object::registry::ObjectRegistry::default()),
            unit_registry: crate::object::unit::new_unit_registry(),
            legacy_object_registry: crate::ai::object_registry::new_legacy_object_registry(),
            locomotor_store: crate::locomotor::new_locomotor_store(),
            main_game_logic: OnceLock::new(),
            client_image_collection: Mutex::new(None),
            control_bar_bridge: RwLock::new(None),
            damage_over_time_fields: Mutex::new(Vec::new()),
            damage_over_time_next_id: AtomicU32::new(50000),
            host_script_query: Mutex::new(Default::default()),
            cur_update_module: Mutex::new(None),
            selection: Arc::new(RwLock::new(crate::commands::selection::SelectionManager::new())),
            physics: Arc::new(RwLock::new(crate::physics::PhysicsEngine::new())),
            sides: Arc::new(RwLock::new(crate::sides_list::SidesList::new())),
            rts_actions: {
                let mut manager = game_engine::common::rts::ActionManager::new();
                crate::action_manager::GameLogicActionExecutor::install(&mut manager);
                Arc::new(RwLock::new(manager))
            },
            ghost_objects: Arc::new(RwLock::new(crate::object::ghost_object::GhostObjectManager::new())),
            w3d_ghost_objects: Arc::new(RwLock::new(crate::object::w3d_ghost_object::W3DGhostObjectManager::new())),
            taxi_to_takeoff: Mutex::new(Vec::new()),
            transport_statuses: Mutex::new(Vec::new()),
            pending_team_script_events: Mutex::new(Vec::new()),
            named_evac_disposition: Mutex::new(std::collections::HashMap::new()),
            player_money: Arc::new(RwLock::new(crate::special_power_module::player_money::PlayerMoneyManager::new())),
            player_science: Arc::new(RwLock::new(crate::special_power_module::player_science::PlayerScienceManager::new())),
            special_power_integration: Arc::new(RwLock::new(crate::special_power_module::integration::SpecialPowerIntegrationContext::new())),
            special_power_registry: Arc::new(RwLock::new(crate::special_power_module::registry::SpecialPowerRegistry::new())),
            map_string_overlay: RwLock::new(std::collections::HashMap::new()),
            armor_store: Arc::new(RwLock::new(crate::object::armor::ArmorStore::new())),
            special_power_store: Arc::new(RwLock::new(crate::object::special_power_template::SpecialPowerStore::new())),
            tech_tree: Arc::new(RwLock::new(crate::upgrade::prerequisites::TechTree::new())),
            rank_info_store: Arc::new(RwLock::new(crate::system::rank_info::RankInfoStore::default())),
            weapon_store: Arc::new(RwLock::new(None)),
            ocl_store: Arc::new(RwLock::new(None)),
            hlod_live_child_capture_hook: RwLock::new(None),
            committed_live_states: Mutex::new(std::collections::HashMap::new()),
            model_obj_bounds_hook: RwLock::new(None),
            pristine_bone_lookup_hook: RwLock::new(None),
            sub_object_name_hook: RwLock::new(None),
            w3d_ghost_snapshot_capture_hook: RwLock::new(None),
            w3d_ghost_snapshot_capture_source_hook: RwLock::new(None),
            staging_depth: AtomicU32::new(0),
            staging_deferred_team_effects: Mutex::new(Vec::new()),
            runtime_transaction_lock: Arc::new(parking_lot::Mutex::new(())),
        }
    }
    /// Snapshot engine content; fresh mutable per-world services. Inert.
    pub fn new_for_world() -> Self {
        Self {
            // Only definitions are inherited. Visibility, cells and queued
            // reveals belong to this match (CPP PartitionManager.cpp:2515).
            shroud: Arc::new(Mutex::new(ShroudManager::new())),
            client_visuals: ClientVisualState::default(),
            terrain: Arc::new(RwLock::new(crate::terrain::TerrainLogic::new())),
            radar_updates: Mutex::new(VecDeque::new()),
            host_script_requests: crate::scripting::executor::HostScriptRequestQueues::default(),
            session_flags: SessionLogicFlags::default(),
            common: fresh_common_for_world(),
            logic_managers: WorldLogicManagers::default(),
            players: Arc::new(RwLock::new(crate::player::PlayerList::new())),
            objects: Arc::new(RwLock::new(crate::object_manager::ObjectManager::new())),
            in_game_ui: RwLock::new(InGameUIState::default()),
            eva: Arc::new(Mutex::new(EvaState::new_enabled())),
            control_bar_ui_hooks: Mutex::new(None),
            game_pause_hooks: Mutex::new(None),
            observer_audio_locality_hooks: Mutex::new(None),
            observer_audio_view_hooks: Mutex::new(None),
            scene_submission: Mutex::new(None),
            fx_list_manager: Mutex::new(None),
            particle_system_manager: Mutex::new(None),
            scorch_hook: Mutex::new(None),
            terrain_unit_moved_hook: Mutex::new(None),
            terrain_tree_hook: Mutex::new(None),
            animation_metadata_hook: Mutex::new(None),
            terrain_decal_client: Mutex::new(None),
            terrain_track_client: Mutex::new(None),
            object_registry: std::sync::Arc::new(crate::object::registry::ObjectRegistry::default()),
            unit_registry: crate::object::unit::new_unit_registry(),
            legacy_object_registry: crate::ai::object_registry::new_legacy_object_registry(),
            locomotor_store: crate::locomotor::new_locomotor_store(),
            main_game_logic: OnceLock::new(),
            client_image_collection: Mutex::new(None),
            control_bar_bridge: RwLock::new(None),
            damage_over_time_fields: Mutex::new(Vec::new()),
            damage_over_time_next_id: AtomicU32::new(50000),
            host_script_query: Mutex::new(Default::default()),
            cur_update_module: Mutex::new(None),
            selection: Arc::new(RwLock::new(crate::commands::selection::SelectionManager::new())),
            physics: Arc::new(RwLock::new(crate::physics::PhysicsEngine::new())),
            sides: Arc::new(RwLock::new(crate::sides_list::SidesList::new())),
            rts_actions: {
                let mut manager = game_engine::common::rts::ActionManager::new();
                crate::action_manager::GameLogicActionExecutor::install(&mut manager);
                Arc::new(RwLock::new(manager))
            },
            ghost_objects: Arc::new(RwLock::new(crate::object::ghost_object::GhostObjectManager::new())),
            w3d_ghost_objects: Arc::new(RwLock::new(crate::object::w3d_ghost_object::W3DGhostObjectManager::new())),
            taxi_to_takeoff: Mutex::new(Vec::new()),
            transport_statuses: Mutex::new(Vec::new()),
            pending_team_script_events: Mutex::new(Vec::new()),
            named_evac_disposition: Mutex::new(std::collections::HashMap::new()),
            player_money: Arc::new(RwLock::new(crate::special_power_module::player_money::PlayerMoneyManager::new())),
            player_science: Arc::new(RwLock::new(crate::special_power_module::player_science::PlayerScienceManager::new())),
            special_power_integration: Arc::new(RwLock::new(crate::special_power_module::integration::SpecialPowerIntegrationContext::new())),
            special_power_registry: Arc::new(RwLock::new(crate::special_power_module::registry::SpecialPowerRegistry::new())),
            map_string_overlay: RwLock::new(std::collections::HashMap::new()),
            armor_store: Arc::new(RwLock::new(crate::object::armor::ArmorStore::new())),
            special_power_store: Arc::new(RwLock::new(crate::object::special_power_template::SpecialPowerStore::new())),
            tech_tree: Arc::new(RwLock::new(crate::upgrade::prerequisites::TechTree::new())),
            rank_info_store: Arc::new(RwLock::new(crate::system::rank_info::RankInfoStore::default())),
            weapon_store: Arc::new(RwLock::new(None)),
            ocl_store: Arc::new(RwLock::new(None)),
            hlod_live_child_capture_hook: RwLock::new(None),
            committed_live_states: Mutex::new(std::collections::HashMap::new()),
            model_obj_bounds_hook: RwLock::new(None),
            pristine_bone_lookup_hook: RwLock::new(None),
            sub_object_name_hook: RwLock::new(None),
            w3d_ghost_snapshot_capture_hook: RwLock::new(None),
            w3d_ghost_snapshot_capture_source_hook: RwLock::new(None),
            staging_depth: AtomicU32::new(0),
            staging_deferred_team_effects: Mutex::new(Vec::new()),
            runtime_transaction_lock: Arc::new(parking_lot::Mutex::new(())),
        }
    }
    pub fn terrain(&self) -> &Arc<RwLock<crate::terrain::TerrainLogic>> {
        &self.terrain
    }
    pub fn shroud(&self) -> &Arc<Mutex<ShroudManager>> {
        &self.shroud
    }
    pub(crate) fn client_visuals(&self) -> &ClientVisualState {
        &self.client_visuals
    }
    pub(crate) fn radar_updates(&self) -> &Mutex<VecDeque<RadarUpdate>> {
        &self.radar_updates
    }
    pub(crate) fn host_script_requests(&self) -> &crate::scripting::executor::HostScriptRequestQueues {
        &self.host_script_requests
    }
    pub fn session_flags(&self) -> &SessionLogicFlags {
        &self.session_flags
    }
    pub fn common(&self) -> &Arc<CommonServices> {
        &self.common
    }
    pub(crate) fn logic_managers(&self) -> &WorldLogicManagers {
        &self.logic_managers
    }
    pub fn players(&self) -> &Arc<RwLock<crate::player::PlayerList>> {
        &self.players
    }
    pub fn objects(&self) -> &Arc<RwLock<crate::object_manager::ObjectManager>> {
        &self.objects
    }
    pub(crate) fn in_game_ui(&self) -> &RwLock<InGameUIState> {
        &self.in_game_ui
    }
    pub(crate) fn eva(&self) -> &Arc<Mutex<EvaState>> {
        &self.eva
    }
    pub(crate) fn control_bar_ui_hooks(
        &self,
    ) -> &Mutex<Option<std::sync::Arc<dyn crate::control_bar::ControlBarUiHooks>>> {
        &self.control_bar_ui_hooks
    }
    pub(crate) fn game_pause_hooks(
        &self,
    ) -> &Mutex<Option<std::sync::Arc<dyn crate::helpers::GamePauseHooks>>> {
        &self.game_pause_hooks
    }
    pub(crate) fn observer_audio_locality_hooks(
        &self,
    ) -> &Mutex<Option<std::sync::Arc<dyn crate::helpers::ObserverAudioLocalityHooks>>> {
        &self.observer_audio_locality_hooks
    }
    pub(crate) fn observer_audio_view_hooks(
        &self,
    ) -> &Mutex<Option<std::sync::Arc<dyn crate::helpers::ObserverAudioViewHooks>>> {
        &self.observer_audio_view_hooks
    }
    pub(crate) fn scene_submission(
        &self,
    ) -> &Mutex<Option<std::sync::Arc<dyn game_engine::common::system::scene_submission::SceneSubmission>>> {
        &self.scene_submission
    }
    pub(crate) fn fx_list_manager(
        &self,
    ) -> &Mutex<Option<std::sync::Arc<dyn crate::common::types::FXListManagerInterface>>> {
        &self.fx_list_manager
    }
    pub(crate) fn particle_system_manager(
        &self,
    ) -> &Mutex<Option<std::sync::Arc<dyn crate::common::types::ParticleSystemManagerInterface>>> {
        &self.particle_system_manager
    }
    pub(crate) fn scorch_hook(&self) -> &Mutex<Option<crate::helpers::ScorchHook>> {
        &self.scorch_hook
    }
    pub(crate) fn terrain_unit_moved_hook(
        &self,
    ) -> &Mutex<Option<crate::helpers::TerrainUnitMovedHook>> {
        &self.terrain_unit_moved_hook
    }
    pub(crate) fn terrain_tree_hook(&self) -> &Mutex<Option<crate::helpers::TerrainTreeHook>> {
        &self.terrain_tree_hook
    }
    pub(crate) fn animation_metadata_hook(
        &self,
    ) -> &Mutex<Option<crate::helpers::AnimationMetadataHook>> {
        &self.animation_metadata_hook
    }
    pub(crate) fn terrain_decal_client(
        &self,
    ) -> &Mutex<Option<std::sync::Arc<dyn crate::object::draw::TerrainDecalClient>>> {
        &self.terrain_decal_client
    }
    pub(crate) fn terrain_track_client(
        &self,
    ) -> &Mutex<Option<std::sync::Arc<dyn crate::object::draw::TerrainTrackClient>>> {
        &self.terrain_track_client
    }
    pub(crate) fn object_registry(
        &self,
    ) -> &std::sync::Arc<crate::object::registry::ObjectRegistry> {
        &self.object_registry
    }
    pub(crate) fn unit_registry(
        &self,
    ) -> &std::sync::Arc<std::sync::RwLock<crate::object::unit::UnitRegistry>> {
        &self.unit_registry
    }
    pub(crate) fn legacy_object_registry(
        &self,
    ) -> &std::sync::Arc<crate::ai::object_registry::LegacyObjectRegistryFacade> {
        &self.legacy_object_registry
    }
    pub(crate) fn locomotor_store(
        &self,
    ) -> &std::sync::Arc<crate::locomotor::LocomotorStore> {
        &self.locomotor_store
    }
    pub(crate) fn game_logic_mutex(&self) -> &'static Mutex<crate::system::game_logic::GameLogic> {
        *self.main_game_logic.get_or_init(|| {
            Box::leak(Box::new(Mutex::new(
                crate::system::game_logic::GameLogic::new(),
            )))
        })
    }
    pub fn client_image_collection(
        &self,
    ) -> &Mutex<Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>> {
        &self.client_image_collection
    }
    pub(crate) fn control_bar_bridge(
        &self,
    ) -> &RwLock<Option<std::sync::Arc<crate::control_bar::ControlBarBridge>>> {
        &self.control_bar_bridge
    }
    pub(crate) fn damage_over_time_fields(&self) -> &Mutex<Vec<DamageOverTimeField>> {
        &self.damage_over_time_fields
    }
    pub(crate) fn damage_over_time_next_id(&self) -> &AtomicU32 {
        &self.damage_over_time_next_id
    }
    pub(crate) fn host_script_query(
        &self,
    ) -> &Mutex<crate::scripting::conditions::HostScriptQuerySnapshot> {
        &self.host_script_query
    }
    pub(crate) fn cur_update_module(
        &self,
    ) -> &Mutex<Option<crate::modules::UpdateModulePtr>> {
        &self.cur_update_module
    }
    pub(crate) fn staging_depth(&self) -> &AtomicU32 {
        &self.staging_depth
    }
    pub(crate) fn staging_deferred_team_effects(
        &self,
    ) -> &Mutex<Vec<crate::team::TeamFactoryDeferredEffects>> {
        &self.staging_deferred_team_effects
    }
    pub(crate) fn runtime_transaction_lock(&self) -> &Arc<parking_lot::Mutex<()>> {
        &self.runtime_transaction_lock
    }
    pub(crate) fn selection(&self) -> &Arc<RwLock<crate::commands::selection::SelectionManager>> {
        &self.selection
    }
    pub(crate) fn physics(&self) -> &Arc<RwLock<crate::physics::PhysicsEngine>> {
        &self.physics
    }
    pub fn sides(&self) -> &Arc<RwLock<crate::sides_list::SidesList>> {
        &self.sides
    }
    pub(crate) fn rts_actions(&self) -> &Arc<RwLock<game_engine::common::rts::ActionManager>> {
        &self.rts_actions
    }
    pub(crate) fn ghost_objects(&self) -> &Arc<RwLock<crate::object::ghost_object::GhostObjectManager>> {
        &self.ghost_objects
    }
    pub(crate) fn w3d_ghost_objects(
        &self,
    ) -> &Arc<RwLock<crate::object::w3d_ghost_object::W3DGhostObjectManager>> {
        &self.w3d_ghost_objects
    }
    pub(crate) fn taxi_to_takeoff(&self) -> &Mutex<Vec<crate::common::ObjectID>> {
        &self.taxi_to_takeoff
    }
    pub(crate) fn transport_statuses(&self) -> &Mutex<Vec<(u32, u32, i32)>> {
        &self.transport_statuses
    }
    pub(crate) fn pending_team_script_events(
        &self,
    ) -> &Mutex<Vec<crate::team::PendingTeamScriptEvent>> {
        &self.pending_team_script_events
    }
    pub(crate) fn named_evac_disposition(
        &self,
    ) -> &Mutex<std::collections::HashMap<String, u32>> {
        &self.named_evac_disposition
    }
    pub(crate) fn player_money(
        &self,
    ) -> &Arc<RwLock<crate::special_power_module::player_money::PlayerMoneyManager>> {
        &self.player_money
    }
    pub(crate) fn player_science(
        &self,
    ) -> &Arc<RwLock<crate::special_power_module::player_science::PlayerScienceManager>> {
        &self.player_science
    }
    pub(crate) fn special_power_integration(
        &self,
    ) -> &Arc<RwLock<crate::special_power_module::integration::SpecialPowerIntegrationContext>> {
        &self.special_power_integration
    }
    pub(crate) fn special_power_registry(
        &self,
    ) -> &Arc<RwLock<crate::special_power_module::registry::SpecialPowerRegistry>> {
        &self.special_power_registry
    }
    pub fn map_string_overlay(&self) -> &RwLock<std::collections::HashMap<String, String>> {
        &self.map_string_overlay
    }
    pub(crate) fn armor_store(&self) -> &Arc<RwLock<crate::object::armor::ArmorStore>> {
        &self.armor_store
    }
    pub(crate) fn special_power_store(
        &self,
    ) -> &Arc<RwLock<crate::object::special_power_template::SpecialPowerStore>> {
        &self.special_power_store
    }
    pub(crate) fn tech_tree(&self) -> &Arc<RwLock<crate::upgrade::prerequisites::TechTree>> {
        &self.tech_tree
    }
    pub(crate) fn rank_info_store(
        &self,
    ) -> &Arc<RwLock<crate::system::rank_info::RankInfoStore>> {
        &self.rank_info_store
    }
    pub(crate) fn weapon_store(
        &self,
    ) -> &Arc<RwLock<Option<crate::weapon::WeaponStore>>> {
        &self.weapon_store
    }
    pub(crate) fn ocl_store(
        &self,
    ) -> &Arc<RwLock<Option<crate::object_creation_list::store::ObjectCreationListStore>>> {
        &self.ocl_store
    }
    pub(crate) fn hlod_live_child_capture_hook(
        &self,
    ) -> &RwLock<Option<crate::object::draw::w3d_model_draw::HlodLiveChildCaptureHook>> {
        &self.hlod_live_child_capture_hook
    }
    pub(crate) fn committed_live_states(
        &self,
    ) -> &Mutex<
        std::collections::HashMap<
            crate::common::ObjectID,
            Vec<crate::object::draw::w3d_model_draw::HlodLiveChildState>,
        >,
    > {
        &self.committed_live_states
    }
    pub(crate) fn model_obj_bounds_hook(
        &self,
    ) -> &RwLock<Option<crate::object::draw::w3d_model_draw::ModelBoundsHook>> {
        &self.model_obj_bounds_hook
    }

    pub(crate) fn pristine_bone_lookup_hook(
        &self,
    ) -> &RwLock<Option<crate::object::draw::w3d_model_draw::PristineBoneLookupHook>> {
        &self.pristine_bone_lookup_hook
    }
    pub(crate) fn sub_object_name_hook(
        &self,
    ) -> &RwLock<Option<crate::object::draw::w3d_model_draw::SubObjectNameHook>> {
        &self.sub_object_name_hook
    }
    pub(crate) fn w3d_ghost_snapshot_capture_hook(
        &self,
    ) -> &RwLock<Option<crate::object::w3d_ghost_object::W3DGhostSnapshotCaptureHook>> {
        &self.w3d_ghost_snapshot_capture_hook
    }
    pub(crate) fn w3d_ghost_snapshot_capture_source_hook(
        &self,
    ) -> &RwLock<Option<crate::object::w3d_ghost_object::W3DGhostSnapshotCaptureSourceHook>> {
        &self.w3d_ghost_snapshot_capture_source_hook
    }
}
/// Standalone Core runtime; AI is mandatory, never optional or deferred.
pub struct EngineStores {
    upgrade_center: Arc<RwLock<UpgradeCenter>>,
    services: Arc<WorldServices>,
    ai: Arc<RwLock<AI>>,
    ai_data: Arc<RwLock<AIDataStore>>,
    /// Core `GameLogic` singleton for this runtime (was process OnceLock).
    core_game_logic: OnceLock<&'static Mutex<crate::system::game_logic::GameLogic>>,
}
impl EngineStores {
    fn engine_defaults() -> Self {
        Self {
            upgrade_center: common_upgrade::process_lifetime_upgrade_center(),
            services: Arc::new(WorldServices::engine_defaults()),
            ai: Arc::new(RwLock::new(AI::new())),
            ai_data: ini_ai_data::process_lifetime_ai_data_store(),
            core_game_logic: OnceLock::new(),
        }
    }
    pub fn new_for_world() -> Self {
        let snapshot = ini_ai_data::process_lifetime_ai_data_store()
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        Self {
            upgrade_center: Arc::new(RwLock::new(
                common_upgrade::process_lifetime_upgrade_center()
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone(),
            )),
            services: new_world_services(),
            ai: Arc::new(RwLock::new(AI::new())),
            ai_data: Arc::new(RwLock::new(snapshot)),
            core_game_logic: OnceLock::new(),
        }
    }
    pub fn services(&self) -> &Arc<WorldServices> {
        &self.services
    }
    pub fn upgrade_center(&self) -> &Arc<RwLock<UpgradeCenter>> {
        &self.upgrade_center
    }
    pub fn shroud(&self) -> &Arc<Mutex<ShroudManager>> {
        self.services.shroud()
    }
    pub fn ai(&self) -> &Arc<RwLock<AI>> {
        &self.ai
    }
    pub fn ai_data(&self) -> &Arc<RwLock<AIDataStore>> {
        &self.ai_data
    }
    pub(crate) fn game_logic_mutex(self: &Arc<Self>) -> &'static Mutex<crate::system::game_logic::GameLogic> {
        self.core_game_logic.get_or_init(|| {
            let logic = crate::system::game_logic::GameLogic::new_with_engine_stores(Arc::clone(self));
            // Process-lifetime Core slot: leak so accessors keep `&'static Mutex`.
            Box::leak(Box::new(Mutex::new(logic)))
        })
    }

    pub(crate) fn client_visuals(&self) -> &ClientVisualState {
        self.services.client_visuals()
    }
}
/// Published ambient owner (Core AI runtime or Main world services).
#[derive(Clone)]
pub enum PublishedOwner {
    Core(Arc<EngineStores>),
    Services(Arc<WorldServices>),
}
impl PublishedOwner {
    pub fn services(&self) -> &Arc<WorldServices> {
        match self {
            Self::Core(core) => core.services(),
            Self::Services(services) => services,
        }
    }
    pub fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Core(a), Self::Core(b)) => Arc::ptr_eq(a, b),
            (Self::Services(a), Self::Services(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    fn install_common(&self) {
        let common = Arc::clone(self.services().common());
        let _ = common_services::install_common_services(Arc::clone(&common));
        common.finish_bootstrap_after_publish();
        crate::locomotor::sync_locomotor_store_from_common(self.services().locomotor_store());
        if let Self::Core(core) = self {
            common_upgrade::install_upgrade_center(Arc::clone(core.upgrade_center()));
            ini_ai_data::install_ai_data_store(Arc::clone(core.ai_data()));
        }
    }
    fn uninstall_common(&self) {
        let _ = common_services::uninstall_common_services_if_current(self.services().common());
        if let Self::Core(core) = self {
            common_upgrade::uninstall_upgrade_center_if_current(core.upgrade_center());
            ini_ai_data::uninstall_ai_data_store_if_current(core.ai_data());
        }
    }
}

/// Built-in per-thread publication stack for transitional free-function bridges.
///
/// Long-term: delete this TLS and pass `&WorldServices` / `&mut GameLogic`
/// explicitly (C++ ownership-through-the-engine). Host/runtime paths in Main
/// already prefer owned `world_services`; nested GameLogic-crate free functions
/// still resolve here until those call sites are migrated.
thread_local! {
    static SCOPED_ACTIVE: std::cell::RefCell<Vec<PublishedOwner>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Current published owner, or `None` when the stack is unavailable.
///
/// Drop glue for owned objects can run while this thread-local is being torn
/// down or while the stack itself is mutably borrowed (popping the last owner
/// drops its world). Both cases must read as "nothing published" instead of
/// panicking inside a destructor.
fn stack_head() -> Option<PublishedOwner> {
    SCOPED_ACTIVE
        .try_with(|stack| stack.try_borrow().ok().and_then(|s| s.last().cloned()))
        .ok()
        .flatten()
}
fn stack_push(owner: PublishedOwner) {
    SCOPED_ACTIVE.with(|stack| stack.borrow_mut().push(owner));
}
fn stack_pop_if_head(owner: &PublishedOwner) -> bool {
    // Take the popped owner out of the borrow so its drop (which may tear down
    // a whole world) runs after the stack borrow is released.
    let popped = SCOPED_ACTIVE.with(|stack| {
        let mut active = stack.borrow_mut();
        if active.last().is_some_and(|head| head.same(owner)) {
            active.pop()
        } else {
            None
        }
    });
    popped.is_some()
}
fn stack_retain_not(owner: &PublishedOwner) {
    // Removed owners are dropped after the borrow ends (see `stack_pop_if_head`).
    let removed: Vec<PublishedOwner> = SCOPED_ACTIVE.with(|stack| {
        let mut active = stack.borrow_mut();
        let (removed, kept) = std::mem::take(&mut *active)
            .into_iter()
            .partition(|entry| entry.same(owner));
        *active = kept;
        removed
    });
    drop(removed);
}
fn stack_prior_core_ai_data() -> Option<Arc<RwLock<AIDataStore>>> {
    SCOPED_ACTIVE.with(|stack| {
        stack.borrow().iter().rev().find_map(|entry| match entry {
            PublishedOwner::Core(core) => Some(Arc::clone(core.ai_data())),
            PublishedOwner::Services(_) => None,
        })
    })
}
fn stack_depth() -> usize {
    SCOPED_ACTIVE.with(|stack| stack.borrow().len())
}

/// Deprecated no-op: ambient stack is built into GameLogic (hooks removed).
#[deprecated(note = "ambient hooks deleted; stack is built into engine_stores")]
pub struct AmbientStackHooks {
    pub head: fn() -> Option<PublishedOwner>,
    pub push: fn(PublishedOwner),
    pub pop_if_head: fn(&PublishedOwner) -> bool,
    pub retain_not: fn(&PublishedOwner),
    pub prior_core_ai_data: fn() -> Option<Arc<RwLock<AIDataStore>>>,
    pub depth: fn() -> usize,
}

/// Deprecated no-op kept so out-of-tree / GameClient test binders still link.
#[deprecated(note = "ambient hooks deleted; registration is a no-op")]
pub fn register_ambient_stack_hooks(_hooks: AmbientStackHooks) {}

/// Deprecated no-op kept so out-of-tree / GameClient test binders still link.
#[deprecated(note = "ambient hooks deleted; pre-ensure is a no-op")]
pub fn set_ambient_pre_ensure(_f: fn()) {}

fn published_owner() -> Option<PublishedOwner> {
    let head = stack_head();
    #[cfg(any(test, feature = "test-ambient-services"))]
    if head.is_none() {
        return test_thread_default_owner();
    }
    head
}

#[cfg(any(test, feature = "test-ambient-services"))]
thread_local! {
    // Unit-test convenience only: each libtest thread is its own isolated
    // "game instance" and lazily gets its own owned world when a test did not
    // publish one explicitly. Never compiled into production, never shared
    // between threads.
    static TEST_THREAD_DEFAULT: std::cell::RefCell<Option<PublishedOwner>> =
        const { std::cell::RefCell::new(None) };
    static TEST_AMBIENT_SUPPRESSED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(any(test, feature = "test-ambient-services"))]
fn test_thread_default_owner() -> Option<PublishedOwner> {
    if TEST_AMBIENT_SUPPRESSED.try_with(|flag| flag.get()).unwrap_or(true) {
        return None;
    }
    TEST_THREAD_DEFAULT
        .try_with(|slot| {
            if let Ok(existing) = slot.try_borrow() {
                if let Some(owner) = existing.as_ref() {
                    return Some(owner.clone());
                }
            } else {
                return None;
            }
            // The test world adopts the thread's ambient CommonServices so
            // Common-level and world-level state stay one consistent owner.
            let common = common_services::common_services();
            TEST_ADOPT_COMMON.with(|slot| *slot.borrow_mut() = Some(common));
            let core = Arc::new(EngineStores::new_for_world());
            TEST_ADOPT_COMMON.with(|slot| *slot.borrow_mut() = None);
            let owner = PublishedOwner::Core(core);
            owner.install_common();
            *slot.borrow_mut() = Some(owner.clone());
            Some(owner)
        })
        .ok()
        .flatten()
}
pub(crate) fn published_core_for_game_logic() -> Option<Arc<EngineStores>> {
    match published_owner() {
        Some(PublishedOwner::Core(core)) => Some(core),
        _ => None,
    }
}
pub fn active() -> Arc<EngineStores> {
    match published_owner() {
        Some(PublishedOwner::Core(core)) => core,
        Some(PublishedOwner::Services(_)) => {
            panic!("Core AI runtime requested from a Main world-services boundary")
        }
        None => panic!(
            "EngineStores::active requires with_active_stores / install_active (no process-lifetime fallback bag)"
        ),
    }
}
pub fn try_active_services() -> Option<Arc<WorldServices>> {
    published_owner().map(|owner| Arc::clone(owner.services()))
}
pub fn active_services() -> Arc<WorldServices> {
    try_active_services().expect(
        "WorldServices::active_services requires with_world_services / install_services (no process bag)",
    )
}
fn is_published(owner: &PublishedOwner) -> bool {
    stack_head().is_some_and(|head| head.same(owner))
}
pub fn is_active(core: &Arc<EngineStores>) -> bool {
    is_published(&PublishedOwner::Core(Arc::clone(core)))
}
pub fn is_services_active(services: &Arc<WorldServices>) -> bool {
    is_published(&PublishedOwner::Services(Arc::clone(services)))
}
fn install(owner: PublishedOwner) -> Option<PublishedOwner> {
    owner.install_common();
    let prior = stack_head();
    stack_push(owner);
    prior
}
pub fn install_active(core: Arc<EngineStores>) -> Option<Arc<EngineStores>> {
    install(PublishedOwner::Core(core)).and_then(|owner| match owner {
        PublishedOwner::Core(core) => Some(core),
        PublishedOwner::Services(_) => None,
    })
}
pub fn install_services(services: Arc<WorldServices>) {
    let _ = install(PublishedOwner::Services(services));
}
fn uninstall(owner: &PublishedOwner) -> bool {
    let was_head = stack_head().is_some_and(|head| head.same(owner));
    let restored = if was_head {
        stack_pop_if_head(owner);
        stack_head()
    } else {
        stack_retain_not(owner);
        None
    };
    if was_head {
        owner.uninstall_common();
        if let Some(restored) = restored {
            restored.install_common();
            // Main services do not own the Common AI parser slot. If a Core
            // owner was popped above them, restore the still-installed Core
            // parser below them without selecting/constructing a runtime.
            if matches!(owner, PublishedOwner::Core(_))
                && matches!(restored, PublishedOwner::Services(_))
            {
                if let Some(prior_data) = stack_prior_core_ai_data() {
                    ini_ai_data::install_ai_data_store(prior_data);
                }
            }
        }
    }
    was_head
}
pub fn uninstall_active_if_current(core: &Arc<EngineStores>) -> bool {
    uninstall(&PublishedOwner::Core(Arc::clone(core)))
}
pub fn uninstall_services(services: &Arc<WorldServices>) -> bool {
    uninstall(&PublishedOwner::Services(Arc::clone(services)))
}
pub(crate) fn restore_active(
    expected: &Arc<EngineStores>,
    previous: Option<Arc<EngineStores>>,
) -> bool {
    let owner = PublishedOwner::Core(Arc::clone(expected));
    if !is_published(&owner) {
        return false;
    }
    let removed = uninstall(&owner);
    if let Some(previous) = previous {
        if !is_active(&previous) {
            install_active(previous);
        }
    }
    removed
}
fn with_owner<R>(owner: PublishedOwner, f: impl FnOnce() -> R) -> R {
    struct RestoreScope {
        owner: PublishedOwner,
        prior_common: Option<Arc<CommonServices>>,
        prior_upgrade: Option<Arc<RwLock<UpgradeCenter>>>,
        prior_ai_data: Option<Arc<RwLock<AIDataStore>>>,
    }
    impl Drop for RestoreScope {
        fn drop(&mut self) {
            stack_pop_if_head(&self.owner);
            if common_services::uninstall_common_services_if_current(self.owner.services().common()) {
                if let Some(previous) = self.prior_common.take() {
                    common_services::install_common_services(previous);
                }
            }
            if let PublishedOwner::Core(core) = &self.owner {
                let upgrade = core.upgrade_center();
                if Arc::ptr_eq(&common_upgrade::get_upgrade_center(), upgrade) {
                    if let Some(previous) = self.prior_upgrade.take() {
                        common_upgrade::install_upgrade_center(previous);
                    } else {
                        common_upgrade::uninstall_upgrade_center_if_current(upgrade);
                    }
                }
            }
            if let PublishedOwner::Core(core) = &self.owner {
                if Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), core.ai_data()) {
                    if let Some(previous) = self.prior_ai_data.take() {
                        ini_ai_data::install_ai_data_store(previous);
                    } else {
                        ini_ai_data::uninstall_ai_data_store_if_current(core.ai_data());
                    }
                }
            }
        }
    }
    let prior_upgrade = match &owner {
        PublishedOwner::Core(core) => {
            common_upgrade::install_upgrade_center(Arc::clone(core.upgrade_center()))
        }
        PublishedOwner::Services(_) => None,
    };
    let prior_ai_data = match &owner {
        PublishedOwner::Core(core) => {
            ini_ai_data::install_ai_data_store(Arc::clone(core.ai_data()))
        }
        PublishedOwner::Services(_) => None,
    };
    let common = Arc::clone(owner.services().common());
    let prior_common = common_services::install_common_services(Arc::clone(&common));
    common.finish_bootstrap_after_publish();
    stack_push(owner.clone());
    let _restore = RestoreScope {
        owner,
        prior_common,
        prior_upgrade,
        prior_ai_data,
    };
    f()
}
pub fn with_active_stores<R>(core: &Arc<EngineStores>, f: impl FnOnce() -> R) -> R {
    with_owner(PublishedOwner::Core(Arc::clone(core)), f)
}
/// Temporary synchronous service-only publication for legacy presentation.
/// This never publishes an AI owner or writes the Common AIData parser slot.
pub fn with_world_services<R>(services: &Arc<WorldServices>, f: impl FnOnce() -> R) -> R {
    with_owner(PublishedOwner::Services(Arc::clone(services)), f)
}
pub fn new_for_world() -> Arc<EngineStores> {
    Arc::new(EngineStores::new_for_world())
}
pub fn new_world_services() -> Arc<WorldServices> {
    Arc::new(WorldServices::new_for_world())
}
pub fn upgrade_center() -> Arc<RwLock<UpgradeCenter>> {
    Arc::clone(active().upgrade_center())
}
pub fn shroud_manager() -> Arc<Mutex<ShroudManager>> {
    Arc::clone(active_services().shroud())
}

pub fn the_ai() -> Arc<RwLock<AI>> {
    Arc::clone(active().ai())
}

/// Swap AI contents for the scoped-bundle restoration fixture.
#[cfg(test)]
pub(crate) fn take_ai_for_world_boundary(bundle: &Arc<EngineStores>) -> AI {
    let mut ai = bundle
        .ai()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::replace(&mut *ai, AI::new())
}

/// Restore the fixture's captured AI contents to its original bundle.
#[cfg(test)]
pub(crate) fn replace_ai_for_world_boundary(bundle: &Arc<EngineStores>, next: AI) -> AI {
    let mut ai = bundle
        .ai()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::replace(&mut *ai, next)
}

#[cfg(test)]
fn active_stack_depth() -> usize {
    stack_depth()
}

#[cfg(test)]
mod tests {
    /// Resolve with the per-thread test convenience world disabled, so the
    /// "nothing published" contract itself stays testable.
    fn strict<R>(f: impl FnOnce() -> R) -> R {
        struct Restore;
        impl Drop for Restore {
            fn drop(&mut self) {
                super::TEST_AMBIENT_SUPPRESSED.with(|flag| flag.set(false));
            }
        }
        super::TEST_AMBIENT_SUPPRESSED.with(|flag| flag.set(true));
        let _restore = Restore;
        f()
    }

    use super::*;

    #[test]
    fn client_visuals_follow_world_rollback_and_commit() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);
        let client = crate::helpers::TheGameClient::get().expect("logic client bridge");
        let drawable_id = 9_700_101;
        let object_id = 9_700_102;

        let live = new_for_world();
        // Constructing a candidate must not publish its empty visual maps.
        let candidate = new_for_world();
        assert_eq!(active_stack_depth(), 0);
        install_active(Arc::clone(&live));
        client.seed_drawable_pose_for_test(drawable_id, crate::common::Coord3D::ZERO, 0.25);
        client.begin_object_model_draw_frame(object_id);
        client.set_object_wheel_info(object_id, crate::helpers::DrawWheelInfo::default());
        crate::helpers::ClientVisualHandle::new(Arc::clone(live.services()))
            .note_weapon_recoil(object_id, 2.0, 0.5);
        let live_light = crate::helpers::create_scene_point_light();
        client.add_tree(
            drawable_id,
            &crate::common::Coord3D::ZERO,
            1.0,
            0.0,
            0.0,
            &crate::object::draw::w3d_tree_draw::W3DTreeDrawModuleData::new(),
        );

        // Map/save staging works on its own captured bundle. Reusing IDs in
        // the candidate cannot overwrite any live-world presentation data.
        with_active_stores(&candidate, || {
            assert!(client.find_drawable_by_id(drawable_id).is_none());
            assert!(client.get_object_wheel_info(object_id).is_none());
            assert!(client.get_registered_tree(drawable_id).is_none());
            assert!(
                crate::helpers::ClientVisualHandle::new(Arc::clone(candidate.services()))
                    .take_weapon_recoils()
                    .is_empty()
            );
            assert!(crate::helpers::scene_point_lights().is_empty());
            client.seed_drawable_pose_for_test(
                drawable_id,
                crate::common::Coord3D::new(3.0, 4.0, 5.0),
                0.75,
            );
            assert_eq!(crate::helpers::create_scene_point_light(), 1);
            crate::helpers::ClientVisualHandle::new(Arc::clone(candidate.services()))
                .note_weapon_recoil(object_id, 4.0, 0.75);
        });
        assert_eq!(live_light, 1);
        let candidate_visuals =
            crate::helpers::ClientVisualHandle::new(Arc::clone(candidate.services()));
        let captured = candidate_visuals.snapshot_objectless_drawables();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].1.orientation, 0.75);
        assert_eq!(
            candidate_visuals.take_weapon_recoils(),
            vec![(object_id, 4.0, 0.75)]
        );
        assert_eq!(
            client.find_drawable_by_id(drawable_id).unwrap().orientation,
            0.25
        );
        assert_eq!(
            crate::helpers::ClientVisualHandle::new(Arc::clone(live.services()))
                .take_weapon_recoils(),
            vec![(object_id, 2.0, 0.5)]
        );
        assert_eq!(
            live.client_visuals()
                .model_draw_frames
                .lock()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(live.client_visuals().terrain_trees.lock().unwrap().len(), 1);

        // Failed candidate: dropping it leaves the installed live world and
        // its visual state intact. Successful candidate: the candidate's
        // bundle becomes the head and the buried old world can be dropped.
        install_active(Arc::clone(&candidate));
        assert_eq!(
            client.find_drawable_by_id(drawable_id).unwrap().orientation,
            0.75
        );
        assert!(uninstall_active_if_current(&candidate));
        assert_eq!(
            client.find_drawable_by_id(drawable_id).unwrap().orientation,
            0.25
        );
        candidate_visuals.clear_visual_state_for_reset();
        with_active_stores(&candidate, || {
            assert!(client.find_drawable_by_id(drawable_id).is_none());
            assert!(crate::helpers::scene_point_lights().is_empty());
            client.seed_drawable_pose_for_test(
                drawable_id,
                crate::common::Coord3D::new(3.0, 4.0, 5.0),
                0.75,
            );
        });
        assert_eq!(
            client.find_drawable_by_id(drawable_id).unwrap().orientation,
            0.25
        );
        install_active(Arc::clone(&candidate));
        assert!(!uninstall_active_if_current(&live));
        assert_eq!(
            client.find_drawable_by_id(drawable_id).unwrap().orientation,
            0.75
        );
        assert!(uninstall_active_if_current(&candidate));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn drawable_recoil_uses_bound_world_even_when_another_is_active() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);
        let a = new_for_world();
        let b = new_for_world();
        let a_weak = Arc::downgrade(&a);
        let a_visuals = crate::helpers::ClientVisualHandle::new(Arc::clone(a.services()));
        let b_visuals = crate::helpers::ClientVisualHandle::new(Arc::clone(b.services()));
        let mut drawable = crate::object::drawable::Drawable::new(
            71,
            81,
            "RecoilOwnerTest".to_string(),
            crate::object::drawable::DrawableType::Animated,
        );
        drawable.bind_visual_owner(a_visuals.downgrade());

        install_active(Arc::clone(&b));
        drawable.apply_weapon_recoil(2.5, 0.75);
        assert!(b_visuals.take_weapon_recoils().is_empty());
        assert_eq!(a_visuals.take_weapon_recoils(), vec![(81, 2.5, 0.75)]);
        assert!(uninstall_active_if_current(&b));

        drop(a_visuals);
        drop(a);
        assert!(a_weak.upgrade().is_none(), "drawable must not retain world");
        drawable.apply_weapon_recoil(1.0, 0.25);
        assert!(b_visuals.take_weapon_recoils().is_empty());
    }

    #[test]
    fn client_created_drawable_keeps_its_world_after_active_switch() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);
        let a = new_for_world();
        let b = new_for_world();
        let a_weak = Arc::downgrade(&a);
        let a_visuals = crate::helpers::ClientVisualHandle::new(Arc::clone(a.services()));
        let b_visuals = crate::helpers::ClientVisualHandle::new(Arc::clone(b.services()));
        install_active(Arc::clone(&a));
        let template = crate::common::DefaultThingTemplate::new("VisualOwnerTest".to_string());
        let id = crate::helpers::TheGameClient::get()
            .expect("client bridge")
            .create_drawable(&template);
        let drawable = a
            .client_visuals()
            .drawables
            .lock()
            .unwrap()
            .get(&id)
            .and_then(|state| state.drawable.clone())
            .expect("created drawable");
        let mut saved = a
            .client_visuals()
            .drawables
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .expect("saved drawable");
        saved.position = crate::common::Coord3D::new(4.0, 5.0, 6.0);
        saved.orientation = 0.5;
        saved.beam_start = Some(crate::common::Coord3D::ZERO);
        saved.beam_end = Some(crate::common::Coord3D::new(1.0, 2.0, 3.0));
        install_active(Arc::clone(&b));
        let b_id = crate::helpers::TheGameClient::get()
            .expect("client bridge")
            .create_drawable(&template);
        drawable.write().unwrap().apply_weapon_recoil(1.5, 0.25);
        assert_eq!(
            a_visuals.take_weapon_recoils(),
            vec![(crate::common::INVALID_ID, 1.5, 0.25)]
        );
        assert!(b_visuals.take_weapon_recoils().is_empty());
        a_visuals.restore_objectless_drawable(id, &saved);
        let restored = a
            .client_visuals()
            .drawables
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .unwrap();
        assert_eq!(restored.position, saved.position);
        assert_eq!(restored.orientation, saved.orientation);
        assert_eq!(restored.beam_end, saved.beam_end);
        assert_eq!(
            b.client_visuals()
                .drawables
                .lock()
                .unwrap()
                .get(&b_id)
                .unwrap()
                .position,
            crate::common::Coord3D::ZERO
        );
        a_visuals.clear_objectless_drawables();
        assert!(a.client_visuals().drawables.lock().unwrap().is_empty());
        assert!(
            b.client_visuals()
                .drawables
                .lock()
                .unwrap()
                .contains_key(&b_id)
        );
        assert!(uninstall_active_if_current(&b));
        assert!(uninstall_active_if_current(&a));
        drop(a_visuals);
        drop(saved);
        drop(restored);
        drop(a);
        assert!(a_weak.upgrade().is_none(), "drawable must not retain world");
    }

    #[test]
    fn dropping_newer_world_restores_previous_as_active() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        // Construct A, construct B (both at their start boundaries), drop B:
        // the audit's headline ordering. Resolution must fail closed to A,
        // never to the engine-lifetime fallback.
        let a = new_for_world();
        install_active(Arc::clone(&a));
        let b = new_for_world();
        let displaced_by_b = install_active(Arc::clone(&b));
        assert!(displaced_by_b.as_ref().is_some_and(|p| Arc::ptr_eq(p, &a)));
        assert!(Arc::ptr_eq(&active(), &b));

        uninstall_active_if_current(&b); // GameLogic::drop for B
        assert!(Arc::ptr_eq(&active(), &a));
        assert!(Arc::ptr_eq(&the_ai(), a.ai()));
        assert!(Arc::ptr_eq(&upgrade_center(), a.upgrade_center()));
        assert!(Arc::ptr_eq(&shroud_manager(), a.shroud()));
        // The Common INI funnels mirror the reinstated head.
        assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), a.ai_data()));
        assert!(Arc::ptr_eq(
            &common_upgrade::get_upgrade_center(),
            a.upgrade_center()
        ));

        let services = new_world_services();
        install_services(Arc::clone(&services));
        assert!(Arc::ptr_eq(&active_services(), &services));
        assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), a.ai_data()));
        install_active(Arc::clone(&b));
        assert!(uninstall_active_if_current(&b));
        assert!(Arc::ptr_eq(&active_services(), &services));
        assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), a.ai_data()));
        assert!(uninstall_services(&services));
        assert!(Arc::ptr_eq(&active(), &a));
        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn staged_restore_rollback_never_touches_process_lifetime() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        // Live world A with content worth protecting.
        let a = new_for_world();
        install_active(Arc::clone(&a));
        let shroud_before = a
            .shroud()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot_state();
        // Capture the live bundle explicitly and take the live AI/shroud
        // contents out (fresh defaults stay behind as staging scratch).
        let live_bundle = active();
        assert!(Arc::ptr_eq(&live_bundle, &a));
        let live_ai = take_ai_for_world_boundary(&live_bundle);
        let live_shroud = with_active_stores(&live_bundle, || {
            let manager = shroud_manager();
            let mut guard = manager
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, ShroudManager::new())
        });

        // The candidate installs at its start_new_game boundary.
        let b = new_for_world();
        install_active(Arc::clone(&b));

        // Early Err: locals drop newest-declared-first, so staged GameLogic B
        // drops before rollback; B's drop pops it and reinstates A as head...
        uninstall_active_if_current(&b);
        assert!(Arc::ptr_eq(&active(), &a));

        // ...then rollback reinstalls the live contents through the CAPTURED
        // bundle handle (pinned via with_active_stores), never
        // through ambient/engine-lifetime resolution. This is exactly the
        // ordering that used to install A's AI/shroud into the
        // engine-lifetime bundle.
        let pinned_bundle = Arc::clone(&live_bundle);
        with_active_stores(&live_bundle, move || {
            let scratch_ai = take_ai_for_world_boundary(&pinned_bundle);
            drop(scratch_ai);
            let replaced = replace_ai_for_world_boundary(&pinned_bundle, live_ai);
            drop(replaced);
            // Shroud rides the same pinned ambient funnel.
            let manager = shroud_manager();
            let mut guard = manager
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let scratch_shroud = std::mem::replace(&mut *guard, live_shroud);
            drop(scratch_shroud);
        });

        // A serves its own bundle and contents again...
        assert!(Arc::ptr_eq(&the_ai(), a.ai()));
        assert!(Arc::ptr_eq(&shroud_manager(), a.shroud()));
        let shroud_after = a
            .shroud()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot_state();
        assert_eq!(shroud_before, shroud_after);
        // No process-lifetime bag remains to pollute; A alone is authoritative.

        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn unscoped_active_services_panics_without_process_bag() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| strict(active_services))).is_err(),
            "unscoped active_services must not mint a process bag"
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| strict(active))).is_err(),
            "unscoped active must not mint a process bag"
        );
    }

    #[test]
    fn world_construction_does_not_touch_the_active_slot() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        let _bundled = new_for_world();
        let _plain = EngineStores::new_for_world();

        assert_eq!(active_stack_depth(), 0);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| strict(active))).is_err(),
            "constructing a world must not publish a process bag"
        );
    }

    #[test]
    fn one_upgrade_center_per_bundle_shared_with_common() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);
        // A world snapshots Common's process center under a fresh lock.
        let world = new_for_world();
        assert!(!Arc::ptr_eq(
            world.upgrade_center(),
            &common_upgrade::process_lifetime_upgrade_center()
        ));
        let engine = common_upgrade::process_lifetime_upgrade_center()
            .read()
            .unwrap()
            .clone();
        for template in world.upgrade_center().read().unwrap().get_all_upgrades() {
            let engine_template = engine.find_upgrade(template.get_name().as_str()).unwrap();
            assert_eq!(engine_template.get_mask(), template.get_mask());
        }
        // Installed, GameLogic and Common resolve the same instance.
        install_active(Arc::clone(&world));
        assert!(Arc::ptr_eq(&upgrade_center(), world.upgrade_center()));
        assert!(Arc::ptr_eq(
            &common_upgrade::get_upgrade_center(),
            world.upgrade_center()
        ));
        assert!(uninstall_active_if_current(&world));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn with_active_stores_pins_and_restores_resolution() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        let a = new_for_world();
        install_active(Arc::clone(&a));
        let b = new_for_world();

        let scoped_saw_bundle = with_active_stores(&b, || {
            assert!(Arc::ptr_eq(&active(), &b));
            assert!(Arc::ptr_eq(&the_ai(), b.ai()));
            assert!(Arc::ptr_eq(&shroud_manager(), b.shroud()));
            assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), b.ai_data()));
            // Nested scopes: innermost wins, outer restored after.
            let nested = with_active_stores(&a, || Arc::ptr_eq(&active(), &a));
            assert!(nested);
            assert!(Arc::ptr_eq(&active(), &b));
            true
        });
        assert!(scoped_saw_bundle);
        assert!(Arc::ptr_eq(&active(), &a));
        assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), a.ai_data()));
        assert!(Arc::ptr_eq(
            &common_upgrade::get_upgrade_center(),
            a.upgrade_center()
        ));

        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn with_active_stores_restores_on_unwind() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        let b = new_for_world();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_active_stores(&b, || panic!("scoped active-stores unwind probe"));
        }))
        .is_err();
        assert!(panicked);

        // Scoped publication and the Common INI slots are restored even on
        // unwind: no published owner remains (no process-lifetime bag).
        assert_eq!(active_stack_depth(), 0);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| strict(active))).is_err(),
            "unscoped active() must not resolve a process bag"
        );
        assert!(Arc::ptr_eq(
            &ini_ai_data::get_ai_data_store(),
            &ini_ai_data::process_lifetime_ai_data_store()
        ));
    }

    #[test]
    fn restore_active_reinstates_previous_only_while_head() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        let a = new_for_world();
        let b = new_for_world();
        let c = new_for_world();
        install_active(Arc::clone(&a));

        let displaced_by_b = install_active(Arc::clone(&b));
        assert!(displaced_by_b.as_ref().is_some_and(|p| Arc::ptr_eq(p, &a)));
        assert!(restore_active(&b, displaced_by_b));
        assert!(Arc::ptr_eq(&active(), &a));

        // A newer install between install and restore must not be clobbered.
        let displaced_by_c = install_active(Arc::clone(&c));
        assert!(displaced_by_c.as_ref().is_some_and(|p| Arc::ptr_eq(p, &a)));
        assert!(!restore_active(&b, Some(Arc::clone(&a))));
        assert!(Arc::ptr_eq(&active(), &c));

        assert!(uninstall_active_if_current(&c));
        assert!(Arc::ptr_eq(&active(), &a));
        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn dropping_buried_world_cannot_be_reinstated_by_later_pop() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        // The staged-commit ordering: A live, B staged on top, A dies while
        // B is the newer world (host drops the old world after install).
        let a = new_for_world();
        let b = new_for_world();
        install_active(Arc::clone(&a));
        install_active(Arc::clone(&b));

        assert!(!uninstall_active_if_current(&a)); // buried: head unchanged
        assert!(Arc::ptr_eq(&active(), &b));
        assert_eq!(active_stack_depth(), 1);

        // The newer world dying later leaves no published owner and must
        // never reinstate the dead world A (no process-lifetime bag).
        assert!(uninstall_active_if_current(&b));
        assert_eq!(active_stack_depth(), 0);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| strict(active))).is_err(),
            "unscoped active() must not resolve a process bag"
        );
    }
}
