// Audio globals, resolvers, and TheAudio facade
//
// Split from `helpers.rs` for module-size parity.
// Observable behavior is unchanged.

/// Minimal misc-audio descriptor providing stable handles for crate events.
#[derive(Clone)]
pub struct MiscAudioEvents {
    pub crate_heal: AudioEventRts,
    pub crate_shroud: AudioEventRts,
    pub crate_salvage: AudioEventRts,
    pub crate_free_unit: AudioEventRts,
    pub crate_money: AudioEventRts,
    pub battle_cry_sound: AudioEventRts,
    pub money_deposit: AudioEventRts,
    pub money_withdraw: AudioEventRts,
    pub sabotage_shut_down_building: AudioEventRts,
    pub sabotage_reset_timer_building: AudioEventRts,
    pub unit_promoted: AudioEventRts,
}

impl Default for MiscAudioEvents {
    fn default() -> Self {
        Self {
            crate_heal: AudioEventRts::new("crate_heal"),
            crate_shroud: AudioEventRts::new("crate_shroud"),
            crate_salvage: AudioEventRts::new("crate_salvage"),
            crate_free_unit: AudioEventRts::new("crate_free_unit"),
            crate_money: AudioEventRts::new("crate_money"),
            battle_cry_sound: AudioEventRts::new("battle_cry_sound"),
            money_deposit: AudioEventRts::new("money_deposit"),
            money_withdraw: AudioEventRts::new("money_withdraw"),
            sabotage_shut_down_building: AudioEventRts::new("sabotage_shut_down_building"),
            sabotage_reset_timer_building: AudioEventRts::new("sabotage_reset_timer_building"),
            unit_promoted: AudioEventRts::new("UnitPromoted"),
        }
    }
}

pub struct TheAudio;

#[cfg(test)]
static AUDIO_EVENTS_ENABLED_FOR_TESTS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

#[cfg(test)]
pub fn set_audio_events_enabled_for_tests(enabled: bool) -> bool {
    AUDIO_EVENTS_ENABLED_FOR_TESTS.swap(enabled, std::sync::atomic::Ordering::SeqCst)
}

struct GameLogicAudioEventOwnerResolver;

impl AudioEventOwnerResolver for GameLogicAudioEventOwnerResolver {
    fn resolve_object_position(&self, object_id: ObjectID) -> Option<EngineCoord3D> {
        let object = TheGameLogic::find_object_by_id(object_id)?;
        let guard = object.read().ok()?;
        let position = *guard.get_position();
        Some(EngineCoord3D {
            x: position.x,
            y: position.y,
            z: position.z,
        })
    }

    fn resolve_drawable_position(&self, drawable_id: u32) -> Option<EngineCoord3D> {
        let client = TheGameClient::get()?;
        let state = client.find_drawable_by_id(drawable_id)?;
        Some(EngineCoord3D {
            x: state.position.x,
            y: state.position.y,
            z: state.position.z,
        })
    }

    fn resolve_object_player_index(&self, object_id: ObjectID) -> Option<Int> {
        let object = TheGameLogic::find_object_by_id(object_id)?;
        let guard = object.read().ok()?;
        let player = guard.get_controlling_player()?;
        let player_guard = player.read().ok()?;
        Some(player_guard.get_player_index())
    }

    fn resolve_drawable_player_index(&self, drawable_id: u32) -> Option<Int> {
        let client = TheGameClient::get()?;
        let state = client.find_drawable_by_id(drawable_id)?;

        if state.shroud_status_object_id != INVALID_ID {
            return self.resolve_object_player_index(state.shroud_status_object_id);
        }

        let drawable = state.drawable?;
        let object_id = drawable.read().ok()?.get_object_id();
        if object_id == INVALID_ID {
            return None;
        }

        self.resolve_object_player_index(object_id)
    }
}

struct GameLogicAudioShroudResolver;

impl AudioShroudResolver for GameLogicAudioShroudResolver {
    fn is_position_visible_to_local_player(&self, position: &EngineCoord3D) -> Bool {
        let local_player_index = crate::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|player| player.read().ok().map(|guard| guard.get_player_index()));

        let Some(local_player_index) = local_player_index else {
            return true;
        };
        if local_player_index < 0 {
            return true;
        }

        let Ok(shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return true;
        };

        let world = Coord3D {
            x: position.x,
            y: position.y,
            z: position.z,
        };

        matches!(
            shroud.get_shroud_state(local_player_index as u32, &world),
            crate::system::shroud_manager::ShroudState::Visible
        )
    }
}

struct GameLogicAudioLocalityResolver;

impl AudioLocalityResolver for GameLogicAudioLocalityResolver {
    fn get_local_player_index(&self) -> Option<Int> {
        let list = crate::player::player_list().read().ok()?;
        let player = list.get_local_player()?.clone();
        let guard = player.read().ok()?;
        Some(guard.get_player_index())
    }

    fn is_player_active(&self, player_index: Int) -> Bool {
        if player_index < 0 {
            return false;
        }
        let list = match crate::player::player_list().read() {
            Ok(list) => list,
            Err(_) => return false,
        };
        let Some(player) = list.get_player(player_index).cloned() else {
            return false;
        };
        player
            .read()
            .ok()
            .map(|guard| guard.is_player_active())
            .unwrap_or(false)
    }

    fn player_exists(&self, player_index: Int) -> Bool {
        if player_index < 0 {
            return false;
        }
        crate::player::player_list()
            .read()
            .ok()
            .map(|list| list.get_player(player_index).is_some())
            .unwrap_or(false)
    }

    fn has_default_team(&self, player_index: Int) -> Bool {
        if player_index < 0 {
            return false;
        }
        let list = match crate::player::player_list().read() {
            Ok(list) => list,
            Err(_) => return false,
        };
        let Some(player) = list.get_player(player_index).cloned() else {
            return false;
        };
        player
            .read()
            .ok()
            .and_then(|guard| guard.get_default_team())
            .is_some()
    }

    fn get_observer_look_at_player_index(&self) -> Option<Int> {
        observer_audio_locality_hooks().and_then(|hooks| hooks.get_observer_look_at_player_index())
    }

    fn get_relationship_to_local_team(
        &self,
        source_player_index: Int,
        local_player_index: Int,
    ) -> AudioLocalityRelationship {
        if source_player_index < 0 || local_player_index < 0 {
            return AudioLocalityRelationship::Neutral;
        }

        let list = match crate::player::player_list().read() {
            Ok(list) => list,
            Err(_) => return AudioLocalityRelationship::Neutral,
        };

        let Some(source_player) = list.get_player(source_player_index).cloned() else {
            return AudioLocalityRelationship::Neutral;
        };
        let Some(local_player) = list.get_player(local_player_index).cloned() else {
            return AudioLocalityRelationship::Neutral;
        };

        let local_team = local_player
            .read()
            .ok()
            .and_then(|guard| guard.get_default_team());
        let Some(local_team) = local_team else {
            return AudioLocalityRelationship::Neutral;
        };

        let Ok(local_team_guard) = local_team.read() else {
            return AudioLocalityRelationship::Neutral;
        };
        let Ok(source_guard) = source_player.read() else {
            return AudioLocalityRelationship::Neutral;
        };

        match source_guard.get_relationship_with_team(&local_team_guard) {
            Relationship::Allies => AudioLocalityRelationship::Allies,
            Relationship::Enemies => AudioLocalityRelationship::Enemies,
            Relationship::Neutral => AudioLocalityRelationship::Neutral,
        }
    }
}

/// Resolver that provides camera/terrain view information for 3D audio positioning.
///
/// C++ equivalent: TheTacticalView and TheTerrainLogic access in AudioManager::update().
/// Uses the real terrain logic for ground height and provides tactical view data
/// from the game client when available.
struct GameLogicAudioViewResolver;

impl AudioViewResolver for GameLogicAudioViewResolver {
    fn get_tactical_view_position(&self) -> EngineCoord3D {
        if let Some((x, y, z)) =
            observer_audio_view_hooks().and_then(|hooks| hooks.get_tactical_view_position())
        {
            return EngineCoord3D { x, y, z };
        }

        // C++ reads TheTacticalView->getPosition(). Fallback remains deterministic.
        EngineCoord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    fn get_tactical_view_angle(&self) -> f32 {
        if let Some(angle) =
            observer_audio_view_hooks().and_then(|hooks| hooks.get_tactical_view_angle())
        {
            return angle;
        }

        // C++ reads TheTacticalView->getAngle(). Fallback remains deterministic.
        0.0
    }

    fn get_3d_camera_position(&self) -> EngineCoord3D {
        if let Some((x, y, z)) =
            observer_audio_view_hooks().and_then(|hooks| hooks.get_3d_camera_position())
        {
            return EngineCoord3D { x, y, z };
        }

        // C++ reads TheTacticalView->get3DCameraPosition(). Fallback remains deterministic.
        EngineCoord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    fn get_ground_height(&self, x: f32, y: f32) -> f32 {
        // C++ reads TheTerrainLogic->getGroundHeight(x, y).
        // This uses the real terrain logic - the most important resolver method
        // for correct audio attenuation over terrain.
        crate::terrain::get_terrain_logic()
            .read()
            .map(|terrain| terrain.get_ground_height(x, y, None))
            .unwrap_or(0.0)
    }
}

fn ensure_audio_event_resolvers_registered() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let _ = register_audio_event_owner_resolver(Arc::new(GameLogicAudioEventOwnerResolver));
        let _ = register_audio_shroud_resolver(Arc::new(GameLogicAudioShroudResolver));
        let _ = register_audio_locality_resolver(Arc::new(GameLogicAudioLocalityResolver));
        let _ = register_audio_view_resolver(Arc::new(GameLogicAudioViewResolver));
    });
}

/// C++ `INI::parseAudioEventRTS` stores the MiscAudio.ini token as the event name.
fn leftover_misc_event_name(
    event: &game_engine::common::ini::ini_misc_audio::AudioEventRTS,
) -> &str {
    event.playable_event_name()
}


impl TheAudio {
    pub fn get() -> Option<&'static Self> {
        static AUDIO: OnceLock<TheAudio> = OnceLock::new();
        let audio = AUDIO.get_or_init(|| TheAudio);
        ensure_audio_event_resolvers_registered();
        Some(audio)
    }

    pub fn get_misc_audio() -> MiscAudioEvents {
        // C++ `TheAudio->getMiscAudio()` returns the live MiscAudio filled by INI parse.
        // Do not OnceLock empty leftover defaults before MiscAudio.ini is loaded.
        let Some(misc_audio) = game_engine::common::ini::ini_misc_audio::get_misc_audio() else {
            return MiscAudioEvents::default();
        };

        let misc_audio = misc_audio.read();
        MiscAudioEvents {
            crate_heal: AudioEventRts::new(leftover_misc_event_name(&misc_audio.crate_heal)),
            crate_shroud: AudioEventRts::new(leftover_misc_event_name(&misc_audio.crate_shroud)),
            crate_salvage: AudioEventRts::new(leftover_misc_event_name(
                &misc_audio.crate_salvage,
            )),
            crate_free_unit: AudioEventRts::new(leftover_misc_event_name(
                &misc_audio.crate_free_unit,
            )),
            crate_money: AudioEventRts::new(leftover_misc_event_name(&misc_audio.crate_money)),
            battle_cry_sound: AudioEventRts::new(leftover_misc_event_name(
                &misc_audio.battle_cry_sound,
            )),
            money_deposit: AudioEventRts::new(leftover_misc_event_name(
                &misc_audio.money_deposit_sound,
            )),
            money_withdraw: AudioEventRts::new(leftover_misc_event_name(
                &misc_audio.money_withdraw_sound,
            )),
            sabotage_shut_down_building: AudioEventRts::new(leftover_misc_event_name(
                &misc_audio.sabotage_shut_down_building,
            )),
            sabotage_reset_timer_building: AudioEventRts::new(leftover_misc_event_name(
                &misc_audio.sabotage_reset_timer_building,
            )),
            unit_promoted: AudioEventRts::new(leftover_misc_event_name(&misc_audio.unit_promoted)),
        }
    }

    pub fn add_audio_event(&self, event: &AudioEventRts) -> u32 {
        #[cfg(test)]
        if !AUDIO_EVENTS_ENABLED_FOR_TESTS.load(std::sync::atomic::Ordering::SeqCst) {
            return 0;
        }

        let mut engine_event = if let Some((x, y, z)) = event.position {
            let pos = EngineCoord3D { x, y, z };
            EngineAudioEventRts::with_position(event.get_event_name(), &pos)
        } else {
            EngineAudioEventRts::with_event_name(event.get_event_name())
        };

        if let Some(drawable_id) = event.drawable_id {
            engine_event.set_drawable_id_override(drawable_id);
        } else if event.position.is_none() && event.object_id != 0 {
            engine_event.set_object_id(event.object_id);
        }

        if let Some(time_of_day) = event.time_of_day {
            engine_event.set_time_of_day(map_audio_time_of_day(time_of_day));
        }

        if let Some(player_index) = event.player_index {
            engine_event.set_player_index(player_index as i32);
        }
        engine_event.set_should_fade(event.should_fade());
        engine_event.set_is_logical_audio(event.is_logical_audio());
        engine_event.set_uninterruptable(event.is_uninterruptable());

        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let mut manager = match manager.lock() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };
        if let Some(info) = manager.find_audio_event_info(engine_event.get_event_name()) {
            engine_event.set_audio_event_info(info.clone());
            engine_event.set_volume(info.volume);
        }
        // C++ AudioManager::addAudioEvent (GameAudio.cpp:391-396) returns AHSV_Error
        // when the name is missing. Do not invent a blank SoundEffect via newAudioEventInfo.

        manager.add_audio_event(&engine_event)
    }

    pub fn add_misc_audio_event(&self, event: &AudioEventRts) -> u32 {
        self.add_audio_event(event)
    }

    pub fn remove_audio_event(&self, handle: u32) {
        if handle == 0 {
            return;
        }

        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut manager) = manager_lock {
            manager.remove_audio_event(handle);
        }
    }

    pub fn is_currently_playing(&self, handle: u32) -> Bool {
        if handle == 0 {
            return false;
        }

        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(manager) = manager_lock {
            manager.is_currently_playing(handle)
        } else {
            false
        }
    }

    pub fn get_audio_length_ms(&self, event: &AudioEventRts) -> Real {
        let mut engine_event = if let Some((x, y, z)) = event.position {
            let pos = EngineCoord3D { x, y, z };
            EngineAudioEventRts::with_position(event.get_event_name(), &pos)
        } else {
            EngineAudioEventRts::with_event_name(event.get_event_name())
        };

        if let Some(drawable_id) = event.drawable_id {
            engine_event.set_drawable_id_override(drawable_id);
        } else if event.position.is_none() && event.object_id != 0 {
            engine_event.set_object_id(event.object_id);
        }

        if let Some(time_of_day) = event.time_of_day {
            engine_event.set_time_of_day(map_audio_time_of_day(time_of_day));
        }
        if let Some(player_index) = event.player_index {
            engine_event.set_player_index(player_index as i32);
        }
        engine_event.set_should_fade(event.should_fade());
        engine_event.set_is_logical_audio(event.is_logical_audio());
        engine_event.set_uninterruptable(event.is_uninterruptable());

        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        let Ok(mut manager) = manager_lock else {
            return 0.0;
        };

        if let Some(info) = manager.find_audio_event_info(engine_event.get_event_name()) {
            engine_event.set_audio_event_info(info.clone());
            engine_event.set_volume(info.volume);
        } else {
            return 0.0;
        }

        manager.get_audio_length_ms(&engine_event)
    }

    pub fn set_volume(&self, volume: Real, affect: EngineAudioAffect) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.set_volume(volume, affect);
        }
    }

    pub fn update(&self) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.update();
        }
    }

    pub fn pause_audio(&self, affect: EngineAudioAffect) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.pause_audio(affect);
        }
    }

    pub fn resume_audio(&self, affect: EngineAudioAffect) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.resume_audio(affect);
        }
    }

    pub fn set_audio_event_enabled(&self, event_name: &str, enabled: Bool) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.set_audio_event_enabled(event_name.to_string(), enabled);
        }
    }

    pub fn set_audio_event_volume_override(&self, event_name: &str, volume: Real) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.set_audio_event_volume_override(event_name.to_string(), volume);
        }
    }

    pub fn remove_audio_event_by_name(&self, event_name: &str) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.remove_playing_audio(event_name);
        }
    }

    pub fn remove_disabled_events(&self) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.remove_all_disabled_audio();
        }
    }

    /// C++ `TheAudio->hasMusicTrackCompleted(track, times)`.
    pub fn has_music_track_completed(&self, track_name: &str, number_of_times: i32) -> bool {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let Ok(guard) = manager.lock() else {
            return false;
        };
        guard.has_music_track_completed(track_name, number_of_times)
    }
}

#[cfg(test)]
mod leftover_the_audio_tests {
    use super::*;

    #[test]
    fn leftover_misc_prefers_event_name_over_sound_file() {
        // C++ INI::parseAudioEventRTS (INI.cpp:1146+) writes the token as the event name.
        let mut src = game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_event_name(
            "CrateHeal".to_string(),
        );
        src.sound_file.clear();
        assert_eq!(leftover_misc_event_name(&src), "CrateHeal");

        let file_only = game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_sound_file(
            "legacy.wav".to_string(),
        );
        assert_eq!(leftover_misc_event_name(&file_only), "legacy.wav");
    }

    #[test]
    fn leftover_get_misc_audio_reads_live_ini_event_name() {
        // C++ TheAudio->getMiscAudio() returns the live MiscAudio after INI parse.
        let handle = game_engine::common::ini::ini_misc_audio::ensure_misc_audio();
        {
            let mut misc = handle.write();
            misc.crate_heal =
                game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_event_name(
                    "CrateHeal".to_string(),
                );
        }
        let events = TheAudio::get_misc_audio();
        assert_eq!(events.crate_heal.get_event_name(), "CrateHeal");
    }
}
