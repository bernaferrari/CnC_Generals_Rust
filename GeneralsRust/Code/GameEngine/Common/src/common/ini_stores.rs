//! Per-instance INI definition stores (C++ TheXxx registries on TheGameEngine).
//!
//! Owned by [`super::common_services::CommonServices`]. Free-function bridges
//! read the published instance via `common_services().ini`.

use std::sync::{Arc, Mutex, RwLock};
use parking_lot::RwLock as ParkingRwLock;

use crate::common::ini::ini_animation::{Anim2DCollection};
use crate::common::ini::ini_audio_settings::{AudioSettings};
use crate::common::ini::ini_campaign::{CampaignStore};
use crate::common::ini::ini_command_button::{ControlBar};
use crate::common::ini::ini_command_map::{MetaMap};
use crate::common::ini::ini_command_set::{CommandSetManager};
use crate::common::ini::ini_control_bar_scheme::{ControlBarSchemeManager};
use crate::common::ini::ini_crate::{ParsedCrateSystem};
use crate::common::ini::ini_damage_fx::{DamageFXStore};
use crate::common::ini::ini_draw_group_info::{DrawGroupInfo};
use crate::common::ini::ini_eva_event::{EvaEventStore};
use crate::common::ini::ini_fx_list::{FXListStore};
use crate::common::ini::ini_game_lod::{GameLODManager};
use crate::common::ini::ini_header_template::{HeaderTemplateManager};
use crate::common::ini::ini_in_game_ui::{InGameUISettings};
use crate::common::ini::ini_language::{GlobalLanguage};
use crate::common::ini::ini_locomotor::{LocomotorStore};
use crate::common::ini::ini_map_cache::{MapCache};
use crate::common::ini::ini_map_data::{MapData};
use crate::common::ini::ini_mapped_image::{ImageCollection};
use crate::common::ini::ini_misc_audio::{MiscAudio};
use crate::common::ini::ini_model::{ModelManager};
use crate::common::ini::ini_mouse::{MouseSettings};
use crate::common::ini::ini_multiplayer::{MultiplayerSettings};
use crate::common::ini::ini_particle_sys::{ParticleSystemManager};
use crate::common::ini::ini_rank::{RankInfoStore};
use crate::common::ini::ini_road::{TerrainRoadCollection};
use crate::common::ini::ini_science::{ScienceStore};
use crate::common::ini::ini_special_power::{SpecialPowerStore};
use crate::common::ini::ini_terrain::{TerrainTypes};
use crate::common::ini::ini_terrain_bridge::{TerrainRoads};
use crate::common::ini::ini_video::{VideoPlayer};
use crate::common::ini::ini_weapon::{WeaponStore};
use crate::common::ini::ini_weather::{WeatherSetting};
use crate::common::ini::ini_window_transition::{WindowTransitionStore};
use crate::common::rts::player_template::PlayerTemplateStore;
use crate::common::rts::science::ScienceStore as RtsScienceStore;
use crate::common::rts::special_power::SpecialPowerStore as RtsSpecialPowerStore;
use crate::common::ini::ini_mouse::CursorInfo;
use crate::common::ini::ini_object::ObjectDefinition;
use std::collections::HashMap;
use crate::common::ini::ini::{BlockParse, ParticleSystemLiveOverlay};
use crate::common::ini::ini_challenge_generals::{ChallengeGenerals, ChallengeGeneralsLoadError};
use std::sync::OnceLock;

/// INI / definition registries owned by one game instance.
pub struct IniStores {
    pub terrain_types: Arc<ParkingRwLock<TerrainTypes>>,
    pub audio_settings: Arc<ParkingRwLock<AudioSettings>>,
    pub mapped_image: Arc<ParkingRwLock<ImageCollection>>,
    pub crate_system: Arc<ParkingRwLock<ParsedCrateSystem>>,
    pub particle_system: Arc<ParkingRwLock<ParticleSystemManager>>,
    pub model_manager: Arc<ParkingRwLock<ModelManager>>,
    pub anim2d: Arc<ParkingRwLock<Anim2DCollection>>,
    pub draw_group_info: Arc<ParkingRwLock<DrawGroupInfo>>,
    pub global_language: Arc<ParkingRwLock<GlobalLanguage>>,
    pub control_bar_scheme: Arc<ParkingRwLock<ControlBarSchemeManager>>,
    pub terrain_roads: Arc<ParkingRwLock<TerrainRoads>>,
    pub misc_audio: Arc<ParkingRwLock<MiscAudio>>,
    pub video_player: Arc<Mutex<VideoPlayer>>,
    pub damage_fx: Arc<RwLock<DamageFXStore>>,
    pub weapon: Arc<RwLock<WeaponStore>>,
    pub science: Arc<RwLock<ScienceStore>>,
    pub special_power: Arc<RwLock<SpecialPowerStore>>,
    pub locomotor: Arc<RwLock<LocomotorStore>>,
    pub fx_list: Arc<RwLock<FXListStore>>,
    pub campaign: Arc<RwLock<CampaignStore>>,
    pub control_bar: Arc<RwLock<ControlBar>>,
    pub command_set: Arc<RwLock<CommandSetManager>>,
    pub rank: Arc<RwLock<RankInfoStore>>,
    pub eva: Arc<RwLock<EvaEventStore>>,
    pub multiplayer: Arc<RwLock<MultiplayerSettings>>,
    pub mouse: Arc<ParkingRwLock<MouseSettings>>,
    pub meta_map: Arc<RwLock<MetaMap>>,
    pub road: Arc<RwLock<TerrainRoadCollection>>,
    pub window_transition: Arc<RwLock<WindowTransitionStore>>,
    pub header_template: Arc<RwLock<HeaderTemplateManager>>,
    pub in_game_ui: Arc<RwLock<InGameUISettings>>,
    pub game_lod: Arc<RwLock<GameLODManager>>,
    pub map_cache: Arc<RwLock<MapCache>>,
    pub map_data: Arc<RwLock<MapData>>,
    pub weather: Arc<RwLock<Option<WeatherSetting>>>,
    pub player_template: Arc<RwLock<PlayerTemplateStore>>,
    pub rts_science: Arc<RwLock<RtsScienceStore>>,
    pub rts_special_power: Arc<RwLock<RtsSpecialPowerStore>>,
    pub cursor_info: Arc<ParkingRwLock<HashMap<String, CursorInfo>>>,
    pub object_definitions: Arc<RwLock<HashMap<String, ObjectDefinition>>>,
    /// Runtime-registered INI block parsers (client overlays).
    pub extra_block_parsers: RwLock<Vec<BlockParse>>,
    /// Live GameClient ParticleSystemManager overlay for map.ini blocks.
    pub particle_system_live_overlay: RwLock<Option<ParticleSystemLiveOverlay>>,
    /// Map-cache display-name text provider (GameTextManager bridge).
    pub game_text_provider: Mutex<Option<Arc<dyn Fn(&str) -> Option<String> + Send + Sync>>>,
    /// Authored ChallengeMode.ini personas (C++ TheChallengeGenerals table).
    pub challenge_generals: OnceLock<Result<(String, Arc<ChallengeGenerals>), ChallengeGeneralsLoadError>>,
}

impl Default for IniStores {
    fn default() -> Self {
        Self {
            terrain_types: Arc::new(ParkingRwLock::new(TerrainTypes::new())),
            audio_settings: Arc::new(ParkingRwLock::new(AudioSettings::new())),
            mapped_image: Arc::new(ParkingRwLock::new(ImageCollection::new())),
            crate_system: Arc::new(ParkingRwLock::new(ParsedCrateSystem::new())),
            particle_system: Arc::new(ParkingRwLock::new(ParticleSystemManager::new())),
            model_manager: Arc::new(ParkingRwLock::new(ModelManager::new())),
            anim2d: Arc::new(ParkingRwLock::new(Anim2DCollection::new())),
            draw_group_info: Arc::new(ParkingRwLock::new(DrawGroupInfo::new())),
            global_language: Arc::new(ParkingRwLock::new(GlobalLanguage::new())),
            control_bar_scheme: Arc::new(ParkingRwLock::new(ControlBarSchemeManager::new())),
            terrain_roads: Arc::new(ParkingRwLock::new(TerrainRoads::new())),
            misc_audio: Arc::new(ParkingRwLock::new(MiscAudio::default())),
            video_player: Arc::new(Mutex::new(VideoPlayer::new())),
            damage_fx: Arc::new(RwLock::new(DamageFXStore::new())),
            weapon: Arc::new(RwLock::new(WeaponStore::new())),
            science: Arc::new(RwLock::new(ScienceStore::new())),
            special_power: Arc::new(RwLock::new(SpecialPowerStore::new())),
            locomotor: Arc::new(RwLock::new(LocomotorStore::new())),
            fx_list: Arc::new(RwLock::new(FXListStore::new())),
            campaign: Arc::new(RwLock::new(CampaignStore::new())),
            control_bar: Arc::new(RwLock::new(ControlBar::new())),
            command_set: Arc::new(RwLock::new(CommandSetManager::new())),
            rank: Arc::new(RwLock::new(RankInfoStore::new())),
            eva: Arc::new(RwLock::new(EvaEventStore::new())),
            multiplayer: Arc::new(RwLock::new(MultiplayerSettings::new())),
            mouse: Arc::new(ParkingRwLock::new(MouseSettings::new())),
            meta_map: Arc::new(RwLock::new(MetaMap::new())),
            road: Arc::new(RwLock::new(TerrainRoadCollection::new())),
            window_transition: Arc::new(RwLock::new(WindowTransitionStore::new())),
            header_template: Arc::new(RwLock::new(HeaderTemplateManager::new())),
            in_game_ui: Arc::new(RwLock::new(InGameUISettings::default())),
            game_lod: Arc::new(RwLock::new(GameLODManager::new())),
            map_cache: Arc::new(RwLock::new(MapCache::new())),
            map_data: Arc::new(RwLock::new(MapData::new())),
            weather: Arc::new(RwLock::new(None)),
            player_template: Arc::new(RwLock::new(PlayerTemplateStore::new())),
            rts_science: Arc::new(RwLock::new(RtsScienceStore::new())),
            rts_special_power: Arc::new(RwLock::new(RtsSpecialPowerStore::new())),
            cursor_info: Arc::new(ParkingRwLock::new(HashMap::new())),
            object_definitions: Arc::new(RwLock::new(HashMap::new())),
            extra_block_parsers: RwLock::new(Vec::new()),
            particle_system_live_overlay: RwLock::new(None),
            game_text_provider: Mutex::new(None),
            challenge_generals: OnceLock::new(),
        }
    }
}

impl IniStores {
    pub fn new() -> Self { Self::default() }
}

