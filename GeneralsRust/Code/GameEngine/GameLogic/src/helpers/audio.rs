// Audio globals, resolvers, and TheAudio facade
//
// Split from `helpers.rs` for module-size parity.
// Observable behavior is unchanged.

use game_engine::common::audio::AudioSubmissionLocality;

/// Minimal misc-audio descriptor providing stable handles for crate events.
#[derive(Clone)]
pub struct MiscAudioEvents {
    pub crate_heal: AudioEventRts,
    pub crate_shroud: AudioEventRts,
    pub crate_salvage: AudioEventRts,
    pub crate_free_unit: AudioEventRts,
    pub crate_money: AudioEventRts,
    pub battle_cry_sound: AudioEventRts,
    pub all_cheer_sound: AudioEventRts,
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
            all_cheer_sound: AudioEventRts::new("UI_AllCheerSound"),
            money_deposit: AudioEventRts::new("money_deposit"),
            money_withdraw: AudioEventRts::new("money_withdraw"),
            sabotage_shut_down_building: AudioEventRts::new("sabotage_shut_down_building"),
            sabotage_reset_timer_building: AudioEventRts::new("sabotage_reset_timer_building"),
            unit_promoted: AudioEventRts::new("UnitPromoted"),
        }
    }
}

/// C++ `TheAudio` of one GameEngine: that engine's AudioManager (its
/// CommonServices) and the world whose Objects and Players resolve the
/// events it plays.
#[derive(Clone)]
pub struct TheAudio {
    common: Arc<game_engine::common::common_services::CommonServices>,
    world: Option<std::sync::Weak<crate::system::engine_stores::WorldServices>>,
}

impl std::fmt::Debug for TheAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TheAudio")
            .field("common", &Arc::as_ptr(&self.common))
            .field("bound_to_world", &self.world.is_some())
            .finish()
    }
}

#[cfg(test)]
static AUDIO_EVENTS_ENABLED_FOR_TESTS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

#[cfg(test)]
pub fn set_audio_events_enabled_for_tests(enabled: bool) -> bool {
    AUDIO_EVENTS_ENABLED_FOR_TESTS.swap(enabled, std::sync::atomic::Ordering::SeqCst)
}

/// Registered per CommonServices by the world that owns it; host objects are
/// that world's (C++ the engine's TheGameLogic), never the published one.
struct GameLogicAudioResolver {
    world: std::sync::Weak<crate::system::engine_stores::WorldServices>,
}

impl AudioEventOwnerResolver for GameLogicAudioResolver {
    fn resolve_object_position(&self, object_id: ObjectID) -> Option<EngineCoord3D> {
        let world = self.world.upgrade()?;
        if let Some(object) = world.object_registry().get_object(object_id) {
            if let Ok(guard) = object.read() {
                let position = *guard.get_position();
                return Some(EngineCoord3D {
                    x: position.x,
                    y: position.y,
                    z: position.z,
                });
            }
        }
        // Live host objects are not in leftover TheGameLogic. Host snapshot is
        // Y-up (x, y_height, z_ground); C++ AudioEventRTS is Z-up (x, y_ground, z_height).
        crate::scripting::HostScriptQuery::of(&world)
            .host_script_query_object_by_id(object_id)
            .map(|obj| EngineCoord3D {
                x: obj.x,
                y: obj.z,
                z: obj.y,
            })
    }

    fn resolve_drawable_position(&self, drawable_id: u32) -> Option<EngineCoord3D> {
        let world = self.world.upgrade()?;
        let position = world
            .client_visuals()
            .drawables
            .lock()
            .ok()?
            .get(&drawable_id)?
            .position;
        Some(EngineCoord3D {
            x: position.x,
            y: position.y,
            z: position.z,
        })
    }

    fn resolve_object_player_index(&self, object_id: ObjectID) -> Option<Int> {
        let world = self.world.upgrade()?;
        let object = world.object_registry().get_object(object_id)?;
        let controller = object.read().ok()?.controlling_player_id_from_pin().ok()??;
        let index = Int::try_from(controller).ok()?;
        world.players().read().ok()?.get_player(index)?;
        Some(index)
    }

    fn resolve_drawable_player_index(&self, drawable_id: u32) -> Option<Int> {
        let world = self.world.upgrade()?;
        let (shroud_status_object_id, drawable) = {
            let drawables = world.client_visuals().drawables.lock().ok()?;
            let state = drawables.get(&drawable_id)?;
            (state.shroud_status_object_id, state.drawable.clone())
        };

        if shroud_status_object_id != INVALID_ID {
            return self.resolve_object_player_index(shroud_status_object_id);
        }

        let drawable = drawable?;
        let object_id = drawable.read().ok()?.get_object_id();
        if object_id == INVALID_ID {
            return None;
        }

        self.resolve_object_player_index(object_id)
    }
}

impl AudioShroudResolver for GameLogicAudioResolver {
    fn is_position_visible_to_local_player(&self, position: &EngineCoord3D) -> Bool {
        let Some(world) = self.world.upgrade() else {
            return true;
        };
        let Some(local_player_index) = self.get_local_player_index() else {
            return true;
        };
        if local_player_index < 0 {
            return true;
        }

        let Ok(shroud) = world.shroud().lock() else {
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

impl GameLogicAudioResolver {
    fn players<R>(&self, f: impl FnOnce(&crate::player::PlayerList) -> R) -> Option<R> {
        let world = self.world.upgrade()?;
        let players = world.players().read().ok()?;
        Some(f(&players))
    }
}

impl AudioLocalityResolver for GameLogicAudioResolver {
    fn get_local_player_index(&self) -> Option<Int> {
        self.players(|list| {
            list.get_local_player()?;
            Some(list.get_local_player_index())
        })?
    }

    fn is_player_active(&self, player_index: Int) -> Bool {
        if player_index < 0 {
            return false;
        }
        self.players(|list| {
            list.get_player(player_index)
                .and_then(|player| player.read().ok())
                .map(|guard| guard.is_player_active())
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }

    fn player_exists(&self, player_index: Int) -> Bool {
        if player_index < 0 {
            return false;
        }
        self.players(|list| list.get_player(player_index).is_some())
            .unwrap_or(false)
    }

    fn has_default_team(&self, player_index: Int) -> Bool {
        if player_index < 0 {
            return false;
        }
        self.players(|list| {
            list.get_player(player_index)
                .and_then(|player| player.read().ok())
                .and_then(|guard| guard.get_default_team())
                .is_some()
        })
        .unwrap_or(false)
    }

    fn get_observer_look_at_player_index(&self) -> Option<Int> {
        let world = self.world.upgrade()?;
        let hooks = world.observer_audio_locality_hooks().lock().ok()?.clone()?;
        hooks.get_observer_look_at_player_index()
    }

    fn get_relationship_to_local_team(
        &self,
        source_player_index: Int,
        local_player_index: Int,
    ) -> AudioLocalityRelationship {
        if source_player_index < 0 || local_player_index < 0 {
            return AudioLocalityRelationship::Neutral;
        }

        let Some((source_player, local_player)) = self
            .players(|list| {
                Some((
                    list.get_player(source_player_index)?.clone(),
                    list.get_player(local_player_index)?.clone(),
                ))
            })
            .flatten()
        else {
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

/// The source Player is already borrowed by its driving operation. Read that
/// same value directly while resolving the other players from the supplied list.
struct BorrowedPlayerAudioLocalityResolver<'a> {
    player: &'a crate::player::Player,
    list: &'a crate::player::PlayerList,
}

impl BorrowedPlayerAudioLocalityResolver<'_> {
    fn with_player<R>(&self, index: Int, f: impl FnOnce(&crate::player::Player) -> R) -> Option<R> {
        let admitted = self.list.get_player(index)?;
        if index == self.player.get_player_index() {
            return Some(f(self.player));
        }
        let player = admitted.read().ok()?;
        Some(f(&player))
    }
}

impl AudioLocalityResolver for BorrowedPlayerAudioLocalityResolver<'_> {
    fn get_local_player_index(&self) -> Option<Int> {
        self.with_player(self.list.get_local_player_index(), |player| {
            player.get_player_index()
        })
    }

    fn is_player_active(&self, index: Int) -> Bool {
        self.with_player(index, |player| player.is_player_active())
            .unwrap_or(false)
    }

    fn player_exists(&self, index: Int) -> Bool {
        self.list.get_player(index).is_some()
    }

    fn has_default_team(&self, index: Int) -> Bool {
        self.with_player(index, |player| player.get_default_team())
            .flatten()
            .is_some()
    }

    fn get_observer_look_at_player_index(&self) -> Option<Int> {
        observer_audio_locality_hooks().and_then(|hooks| hooks.get_observer_look_at_player_index())
    }

    fn get_relationship_to_local_team(
        &self,
        source_index: Int,
        local_index: Int,
    ) -> AudioLocalityRelationship {
        let Some(local_team) = self
            .with_player(local_index, |player| player.get_default_team())
            .flatten()
        else {
            return AudioLocalityRelationship::Neutral;
        };
        let Ok(local_team) = local_team.read() else {
            return AudioLocalityRelationship::Neutral;
        };
        match self.with_player(source_index, |player| {
            player.get_relationship_with_team(&local_team)
        }) {
            Some(Relationship::Allies) => AudioLocalityRelationship::Allies,
            Some(Relationship::Enemies) => AudioLocalityRelationship::Enemies,
            _ => AudioLocalityRelationship::Neutral,
        }
    }
}

pub(crate) fn capture_player_audio_locality(
    player: &crate::player::Player,
    list: &crate::player::PlayerList,
) -> AudioSubmissionLocality {
    AudioSubmissionLocality::from_resolver(
        player.get_player_index(),
        &BorrowedPlayerAudioLocalityResolver { player, list },
    )
}

enum LogicAudioSubmission<'a> {
    Ambient(Option<Int>),
    Explicit(&'a AudioSubmissionLocality),
}

/// Resolver that provides camera/terrain view information for 3D audio positioning.
///
/// C++ equivalent: TheTacticalView and TheTerrainLogic access in AudioManager::update().
/// Uses the real terrain logic for ground height and provides tactical view data
/// from the game client when available.
impl GameLogicAudioResolver {
    fn observer_hooks(&self) -> Option<Arc<dyn crate::helpers::ObserverAudioViewHooks>> {
        let world = self.world.upgrade()?;
        world.observer_audio_view_hooks().lock().ok()?.clone()
    }
}

impl AudioViewResolver for GameLogicAudioResolver {
    fn get_tactical_view_position(&self) -> EngineCoord3D {
        if let Some((x, y, z)) = self
            .observer_hooks()
            .and_then(|hooks| hooks.get_tactical_view_position())
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
        if let Some(angle) = self
            .observer_hooks()
            .and_then(|hooks| hooks.get_tactical_view_angle())
        {
            return angle;
        }

        // C++ reads TheTacticalView->getAngle(). Fallback remains deterministic.
        0.0
    }

    fn get_3d_camera_position(&self) -> EngineCoord3D {
        if let Some((x, y, z)) = self
            .observer_hooks()
            .and_then(|hooks| hooks.get_3d_camera_position())
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
        self.world
            .upgrade()
            .and_then(|world| {
                world
                    .terrain()
                    .read()
                    .ok()
                    .map(|terrain| terrain.get_ground_height(x, y, None))
            })
            .unwrap_or(0.0)
    }
}

/// Register `audio`'s world as the owner/shroud/locality/view resolver of its
/// engine's AudioManager (once per CommonServices). Runs bound to that engine.
fn ensure_audio_event_resolvers_registered(audio: &TheAudio) {
    // A handle that names only the engine (no world) leaves registration to
    // the world that owns that engine.
    let Some(world) = audio.world.as_ref() else {
        return;
    };
    audio
        .common
        .system
        .game_logic_audio_resolvers_installed
        .get_or_init(|| {
            let resolver = Arc::new(GameLogicAudioResolver { world: world.clone() });
            let _ = register_audio_event_owner_resolver(resolver.clone());
            let _ = register_audio_shroud_resolver(resolver.clone());
            let _ = register_audio_locality_resolver(resolver.clone());
            let _ = register_audio_view_resolver(resolver);
            let _ = game_engine::common::system::radar::register_radar_event_feedback(Arc::new(
                WorldRadarAudioFeedback {
                    world: audio.world.clone(),
                },
            ));
        });
}

/// Ownerless radar sounds (event chirps, infiltration) play in the engine
/// whose CommonServices carries this feedback (C++ `TheAudio` of that engine).
struct WorldRadarAudioFeedback {
    world: Option<std::sync::Weak<crate::system::engine_stores::WorldServices>>,
}

impl game_engine::common::system::radar::RadarEventFeedback for WorldRadarAudioFeedback {
    fn play_radar_audio(&self, event_name: &str, player_index: i32) {
        let Some(world) = self.world.as_ref().and_then(std::sync::Weak::upgrade) else {
            return;
        };
        TheAudio::of(&world).add_audio_event(&crate::helpers::radar_notify_audio_event(
            event_name,
            player_index,
        ));
    }
}

/// C++ `INI::parseAudioEventRTS` stores the MiscAudio.ini token as the event name.
fn leftover_misc_event_name(
    event: &game_engine::common::ini::ini_misc_audio::AudioEventRTS,
) -> &str {
    event.playable_event_name()
}

/// C++ `AudioEventRTS` exclusive owner + caller volume, then `TheAudio->addAudioEvent`.
pub(crate) fn leftover_event_to_engine(event: &AudioEventRts) -> EngineAudioEventRts {
    let mut engine_event = match event.owner_type {
        LeftoverAudioOwner::Positional | LeftoverAudioOwner::Dead => {
            if let Some((x, y, z)) = event.position {
                EngineAudioEventRts::with_position(
                    event.get_event_name(),
                    &EngineCoord3D { x, y, z },
                )
            } else {
                EngineAudioEventRts::with_event_name(event.get_event_name())
            }
        }
        _ => EngineAudioEventRts::with_event_name(event.get_event_name()),
    };

    match event.owner_type {
        LeftoverAudioOwner::Object if event.object_id != 0 => {
            engine_event.set_object_id(event.object_id);
        }
        LeftoverAudioOwner::Drawable => {
            if let Some(drawable_id) = event.drawable_id {
                engine_event.set_drawable_id_override(drawable_id);
            }
        }
        LeftoverAudioOwner::Invalid => {
            if let Some(drawable_id) = event.drawable_id {
                engine_event.set_drawable_id_override(drawable_id);
            } else if event.object_id != 0 {
                engine_event.set_object_id(event.object_id);
            }
        }
        _ => {}
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
    if event.has_caller_volume() {
        engine_event.set_volume(event.volume);
    }
    engine_event
}

impl TheAudio {
    /// C++ `TheAudio` of the GameEngine that owns `world`.
    pub fn of(world: &Arc<crate::system::engine_stores::WorldServices>) -> Self {
        Self {
            common: Arc::clone(world.common()),
            world: Some(Arc::downgrade(world)),
        }
    }

    /// C++ `TheAudio` of the GameEngine whose services are `world`, borrowed
    /// without its Arc (its owner resolvers are registered by the world).
    pub fn of_services(world: &crate::system::engine_stores::WorldServices) -> Self {
        Self {
            common: Arc::clone(world.common()),
            world: None,
        }
    }

    /// TheAudio of the engine that owns `object`; an Object of no world
    /// plays nowhere.
    pub fn of_object(object: &Object) -> Option<Self> {
        object.owning_world_services().map(|world| Self::of(&world))
    }

    /// Register this world as its engine's audio owner/locality/view
    /// resolver and create its AudioManager (C++ `AudioManager::init`).
    pub fn install_resolvers(&self) {
        self.with_manager(|_| ());
    }

    /// Run `f` against this engine's AudioManager. The Common audio internals
    /// (resolvers, playback hook, INI parse) read the engine published for the
    /// call, so the call is bound to this one.
    fn with_manager<R>(
        &self,
        f: impl FnOnce(&Arc<std::sync::Mutex<game_engine::common::audio::game_audio::AudioManager>>) -> R,
    ) -> R {
        game_engine::common::common_services::with_common_services(&self.common, || {
            ensure_audio_event_resolvers_registered(self);
            let manager =
                get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
            f(&manager)
        })
    }

    /// C++ `TheAudio->getMiscAudio()`: this engine's MiscAudio as filled by
    /// its own MiscAudio.ini parse.
    /// This engine's authored MiscAudio.ini (C++ `TheAudio->getMiscAudio()`).
    pub fn misc_audio_ini(
        &self,
    ) -> parking_lot::RwLockReadGuard<'_, game_engine::common::ini::ini_misc_audio::MiscAudio> {
        self.common.ini.misc_audio.read()
    }

    pub fn misc_audio(&self) -> MiscAudioEvents {
        let misc_audio = self.common.ini.misc_audio.read();
        MiscAudioEvents {
            crate_heal: AudioEventRts::new(leftover_misc_event_name(&misc_audio.crate_heal)),
            crate_shroud: AudioEventRts::new(leftover_misc_event_name(&misc_audio.crate_shroud)),
            crate_salvage: AudioEventRts::new(leftover_misc_event_name(&misc_audio.crate_salvage)),
            crate_free_unit: AudioEventRts::new(leftover_misc_event_name(
                &misc_audio.crate_free_unit,
            )),
            crate_money: AudioEventRts::new(leftover_misc_event_name(&misc_audio.crate_money)),
            battle_cry_sound: AudioEventRts::new(leftover_misc_event_name(
                &misc_audio.battle_cry_sound,
            )),
            all_cheer_sound: AudioEventRts::new(leftover_misc_event_name(
                &misc_audio.all_cheer_sound,
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
        self.add_audio_event_with_submission(event, LogicAudioSubmission::Ambient(None))
    }

    /// The caller already owns this Object's borrow. Resolve submission
    /// locality from it instead of asking audio to reacquire the same object.
    pub(crate) fn add_audio_event_for_owner(&self, event: &AudioEventRts, owner: &Object) -> u32 {
        let player_index = owner
            .with_controlling_player(|player| player.get_player_index())
            .or_else(|| event.player_index.map(|index| index as Int))
            .unwrap_or(-1);
        self.add_audio_event_with_submission(
            event,
            LogicAudioSubmission::Ambient(Some(player_index)),
        )
    }

    pub(crate) fn add_audio_event_with_locality(
        &self,
        event: &AudioEventRts,
        locality: &AudioSubmissionLocality,
    ) -> u32 {
        self.add_audio_event_with_submission(event, LogicAudioSubmission::Explicit(locality))
    }

    fn add_audio_event_with_submission(
        &self,
        event: &AudioEventRts,
        submission: LogicAudioSubmission<'_>,
    ) -> u32 {
        #[cfg(test)]
        if !AUDIO_EVENTS_ENABLED_FOR_TESTS.load(std::sync::atomic::Ordering::SeqCst) {
            return 0;
        }

        let mut engine_event = leftover_event_to_engine(event);

        self.with_manager(|manager| {
            let mut manager = match manager.lock() {
                Ok(guard) => guard,
                Err(_) => return 0,
            };
            if let Some(info) = manager.find_audio_event_info(engine_event.get_event_name()) {
                engine_event.set_audio_event_info(info);
            }
            // C++ AudioManager::addAudioEvent (GameAudio.cpp:391-396) returns AHSV_Error
            // when the name is missing. Do not invent a blank SoundEffect via newAudioEventInfo.
            // generatePlayInfo does not overwrite a caller setVolume (default -1 uses INI).

            match submission {
                LogicAudioSubmission::Ambient(Some(index)) => {
                    manager.add_audio_event_for_player(&engine_event, index)
                }
                LogicAudioSubmission::Ambient(None) => manager.add_audio_event(&engine_event),
                LogicAudioSubmission::Explicit(locality) => {
                    manager.add_audio_event_with_locality(&engine_event, locality)
                }
            }
        })
    }

    pub fn add_misc_audio_event(&self, event: &AudioEventRts) -> u32 {
        self.add_audio_event(event)
    }

    pub fn remove_audio_event(&self, handle: u32) {
        if handle == 0 {
            return;
        }

        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            if let Ok(mut manager) = manager_lock {
                manager.remove_audio_event(handle);
            }
        })
    }

    pub fn is_currently_playing(&self, handle: u32) -> Bool {
        if handle == 0 {
            return false;
        }

        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            if let Ok(manager) = manager_lock {
                manager.is_currently_playing(handle)
            } else {
                false
            }
        })
    }

    pub fn get_audio_length_ms(&self, event: &AudioEventRts) -> Real {
        let mut engine_event = leftover_event_to_engine(event);

        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            let Ok(mut manager) = manager_lock else {
                return 0.0;
            };

            if let Some(info) = manager.find_audio_event_info(engine_event.get_event_name()) {
                engine_event.set_audio_event_info(info);
            } else {
                return 0.0;
            }

            manager.get_audio_length_ms(&engine_event)
        })
    }

    pub fn set_volume(&self, volume: Real, affect: EngineAudioAffect) {
        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            if let Ok(mut guard) = manager_lock {
                guard.set_volume(volume, affect);
            }
        })
    }

    pub fn update(&self) {
        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            if let Ok(mut guard) = manager_lock {
                guard.update();
            }
        })
    }

    pub fn update_with_view(
        &self,
        observation: game_engine::common::audio::game_audio::AudioViewObservation,
        visible: &mut dyn FnMut(&game_engine::common::audio::audio_event_rts::Coord3D) -> bool,
    ) {
        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            if let Ok(mut guard) = manager_lock {
                guard.update_with_view(observation, visible);
            }
        })
    }

    pub fn pause_audio(&self, affect: EngineAudioAffect) {
        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            if let Ok(mut guard) = manager_lock {
                guard.pause_audio(affect);
            }
        })
    }

    pub fn resume_audio(&self, affect: EngineAudioAffect) {
        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            if let Ok(mut guard) = manager_lock {
                guard.resume_audio(affect);
            }
        })
    }

    /// C++ `TheAudio->loseFocus()` (`GameAudio.cpp:1071-1082`).
    pub fn lose_focus(&self) {
        self.with_manager(|manager| {
            if let Ok(mut guard) = manager.lock() {
                guard.lose_focus();
            }
        })
    }

    /// C++ `TheAudio->regainFocus()` (`GameAudio.cpp:1086-1100`).
    pub fn regain_focus(&self) {
        self.with_manager(|manager| {
            if let Ok(mut guard) = manager.lock() {
                guard.regain_focus();
            }
        })
    }

    pub fn set_audio_event_enabled(&self, event_name: &str, enabled: Bool) {
        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            if let Ok(mut guard) = manager_lock {
                guard.set_audio_event_enabled(event_name.to_string(), enabled);
            }
        })
    }

    pub fn set_audio_event_volume_override(&self, event_name: &str, volume: Real) {
        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            if let Ok(mut guard) = manager_lock {
                guard.set_audio_event_volume_override(event_name.to_string(), volume);
            }
        })
    }

    pub fn remove_audio_event_by_name(&self, event_name: &str) {
        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            if let Ok(mut guard) = manager_lock {
                guard.remove_playing_audio(event_name);
            }
        })
    }

    pub fn remove_disabled_events(&self) {
        self.with_manager(|manager| {
            let manager_lock = manager.lock();
            if let Ok(mut guard) = manager_lock {
                guard.remove_all_disabled_audio();
            }
        })
    }

    /// C++ `TheAudio->hasMusicTrackCompleted(track, times)`.
    pub fn has_music_track_completed(&self, track_name: &str, number_of_times: i32) -> bool {
        self.with_manager(|manager| {
            let Ok(guard) = manager.lock() else {
                return false;
            };
            guard.has_music_track_completed(track_name, number_of_times)
        })
    }
}

#[cfg(test)]
mod leftover_the_audio_tests {
    use super::*;

    struct TestObserver(i32);

    impl crate::helpers::ObserverAudioLocalityHooks for TestObserver {
        fn get_observer_look_at_player_index(&self) -> Option<Int> {
            Some(self.0)
        }
    }

    impl crate::helpers::ObserverAudioViewHooks for TestObserver {
        fn get_tactical_view_position(&self) -> Option<(Real, Real, Real)> {
            Some((self.0 as Real, 2.0, 3.0))
        }
        fn get_tactical_view_angle(&self) -> Option<Real> {
            Some(self.0 as Real)
        }
        fn get_3d_camera_position(&self) -> Option<(Real, Real, Real)> {
            Some((4.0, self.0 as Real, 6.0))
        }
    }

    fn setup_observed_audio_world(index: i32) {
        let world = crate::system::engine_stores::active_services();
        crate::scripting::HostScriptQuery::of(&world).set_host_script_query_snapshot(
            crate::scripting::HostScriptQuerySnapshot {
                objects: vec![crate::scripting::HostScriptQueryObject {
                    id: 77,
                    x: index as f32,
                    y: 42.0,
                    z: 20.0,
                    ..Default::default()
                }],
                ..Default::default()
            },
        );
        {
            let mut players = world.players().write().unwrap();
            players.add_player(Arc::new(std::sync::RwLock::new(
                crate::player::Player::new(index),
            )));
            players.set_local_player_index(index);
        }
        let mut shroud = world.shroud().lock().unwrap();
        shroud.init_shroud_grid(100.0, 100.0);
        if index == 3 {
            shroud
                .reveal_map_for_player_permanently(index as u32)
                .unwrap();
        }
        drop(shroud);
        *world.observer_audio_locality_hooks().lock().unwrap() =
            Some(Arc::new(TestObserver(index)));
        *world.observer_audio_view_hooks().lock().unwrap() = Some(Arc::new(TestObserver(index)));
        TheAudio::of(&world).install_resolvers();
    }

    fn assert_observed_audio_world(index: i32) {
        let world = crate::system::engine_stores::active_services();
        let services = world.common();
        let owner = services
            .system
            .audio_event_owner_resolver
            .lock()
            .unwrap()
            .clone()
            .unwrap();
        let position = owner.resolve_object_position(77).unwrap();
        assert_eq!(
            (position.x, position.y, position.z),
            (index as f32, 20.0, 42.0)
        );
        let locality = services
            .system
            .audio_locality_resolver
            .lock()
            .unwrap()
            .clone()
            .unwrap();
        assert_eq!(locality.get_observer_look_at_player_index(), Some(index));
        assert_eq!(locality.get_local_player_index(), Some(index));
        let view = services
            .system
            .audio_view_resolver
            .lock()
            .unwrap()
            .clone()
            .unwrap();
        assert_eq!(view.get_tactical_view_position().x, index as f32);
        assert_eq!(view.get_tactical_view_angle(), index as f32);
        assert_eq!(view.get_3d_camera_position().y, index as f32);
        let shroud = services
            .system
            .audio_shroud_resolver
            .lock()
            .unwrap()
            .clone()
            .unwrap();
        assert_eq!(
            shroud.is_position_visible_to_local_player(&position),
            index == 3
        );
    }

    #[test]
    fn audio_resolvers_install_for_each_live_world_and_survive_peer_drop() {
        let first = crate::system::engine_stores::new_for_world();
        let second = crate::system::engine_stores::new_for_world();
        crate::system::engine_stores::with_active_stores(&first, || {
            setup_observed_audio_world(3);
            crate::system::engine_stores::with_active_stores(&second, || {
                setup_observed_audio_world(8);
                assert_observed_audio_world(8);
            });
            assert_observed_audio_world(3);
            drop(second);
            assert_observed_audio_world(3);
        });
    }

    #[test]
    fn installed_audio_resolvers_keep_their_world_when_ids_collide() {
        let first = crate::system::engine_stores::new_for_world();
        let second = crate::system::engine_stores::new_for_world();
        for (world, players, local, observer) in
            [(&first, &[7][..], 7, 71), (&second, &[7, 8][..], 8, 82)]
        {
            {
                let mut roster = world.services().players().write().unwrap();
                for index in players {
                    roster.add_player(Arc::new(std::sync::RwLock::new(
                        crate::player::Player::new(*index),
                    )));
                }
                roster.set_local_player_index(local);
            }
            *world
                .services()
                .observer_audio_locality_hooks()
                .lock()
                .unwrap() = Some(Arc::new(TestObserver(observer)));
            *world.services().observer_audio_view_hooks().lock().unwrap() =
                Some(Arc::new(TestObserver(observer)));
            crate::scripting::HostScriptQuery::of(world.services()).set_host_script_query_snapshot(
                crate::scripting::HostScriptQuerySnapshot {
                    objects: vec![crate::scripting::HostScriptQueryObject {
                        id: 77,
                        x: observer as f32,
                        y: 42.0,
                        z: 20.0,
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            );
            crate::system::engine_stores::with_active_stores(world, || {
                let mut object = crate::object::Object::new_test(88, 100.0);
                object.set_position(&Coord3D::new(observer as f32, 25.0, 3.0)).unwrap();
                world.services().object_registry().register_object(88, &Arc::new(RwLock::new(object)));
            });
            let mut shroud = world.services().shroud().lock().unwrap();
            shroud.init_shroud_grid(100.0, 100.0);
            if observer == 71 {
                shroud.reveal_map_for_player_permanently(7).unwrap();
            }
        }

        crate::system::engine_stores::with_active_stores(&first, || {
            TheAudio::of(first.services()).install_resolvers();
        });

        crate::system::engine_stores::with_active_stores(&second, || {
            let resolver = first
                .services()
                .common()
                .system
                .audio_locality_resolver
                .lock()
                .unwrap()
                .clone()
                .unwrap();
            let owner = first
                .services()
                .common()
                .system
                .audio_event_owner_resolver
                .lock()
                .unwrap()
                .clone()
                .unwrap();
            let position = owner.resolve_object_position(77).unwrap();
            assert_eq!((position.x, position.y, position.z), (71.0, 20.0, 42.0));
            let position = owner.resolve_object_position(88).unwrap();
            assert_eq!((position.x, position.y, position.z), (71.0, 25.0, 3.0));
            assert_eq!(resolver.get_local_player_index(), Some(7));
            assert!(resolver.player_exists(7));
            assert!(!resolver.player_exists(8));
            assert_eq!(resolver.get_observer_look_at_player_index(), Some(71));
            let view = first
                .services()
                .common()
                .system
                .audio_view_resolver
                .lock()
                .unwrap()
                .clone()
                .unwrap();
            assert_eq!(view.get_tactical_view_position().x, 71.0);
            assert_eq!(view.get_3d_camera_position().y, 71.0);
            let shroud = first
                .services()
                .common()
                .system
                .audio_shroud_resolver
                .lock()
                .unwrap()
                .clone()
                .unwrap();
            assert!(shroud.is_position_visible_to_local_player(&position));
        });
    }

    #[test]
    fn audio_resolvers_install_in_concurrent_worlds() {
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            for index in [3, 8] {
                let barrier = &barrier;
                scope.spawn(move || {
                    let world = crate::system::engine_stores::new_for_world();
                    crate::system::engine_stores::with_active_stores(&world, || {
                        barrier.wait();
                        setup_observed_audio_world(index);
                        assert_observed_audio_world(index);
                    });
                });
            }
        });
    }

    #[test]
    fn repeated_audio_get_preserves_later_resolver_registration() {
        let world = crate::system::engine_stores::new_for_world();
        crate::system::engine_stores::with_active_stores(&world, || {
            TheAudio::of(world.services()).install_resolvers();
            let replacement: Arc<dyn AudioViewResolver> = Arc::new(GameLogicAudioResolver {
                world: Arc::downgrade(world.services()),
            });
            assert!(!register_audio_view_resolver(Arc::clone(&replacement)));
            TheAudio::of(world.services()).install_resolvers();
            let services = game_engine::common::common_services::common_services();
            let installed = services.system.audio_view_resolver.lock().unwrap().clone().unwrap();
            assert!(Arc::ptr_eq(&replacement, &installed));
        });
    }

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
        let world = crate::system::engine_stores::new_for_world();
        world.services().common().ini.misc_audio.write().crate_heal =
            game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_event_name(
                "CrateHeal".to_string(),
            );
        let events = TheAudio::of(world.services()).misc_audio();
        assert_eq!(events.crate_heal.get_event_name(), "CrateHeal");
    }

    #[test]
    fn leftover_get_misc_audio_maps_all_cheer_sound() {
        // C++ MiscAudio.ini AllCheerSound = UI_AllCheerSound.
        let world = crate::system::engine_stores::new_for_world();
        world.services().common().ini.misc_audio.write().all_cheer_sound =
            game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_event_name(
                "UI_AllCheerSound".to_string(),
            );
        let events = TheAudio::of(world.services()).misc_audio();
        assert_eq!(events.all_cheer_sound.get_event_name(), "UI_AllCheerSound");
    }

    #[test]
    fn leftover_the_audio_object_owner_beats_stamped_position() {
        // C++ setObjectID then setPosition is a no-op; TheAudio follows the object.
        let mut leftover = AudioEventRts::new("PucBeam");
        leftover.set_object_id(77);
        leftover.set_position(&(1.0, 2.0, 3.0));
        let engine = leftover_event_to_engine(&leftover);
        assert_eq!(
            engine.get_owner_type(),
            game_engine::common::audio::OwnerType::Object
        );
        assert_eq!(engine.get_object_id(), 77);
        assert!(!engine.is_positional());
    }

    #[test]
    fn host_snapshot_resolver_uses_host_height() {
        use super::AudioEventOwnerResolver;
        let world = crate::system::engine_stores::new_world_services();
        crate::scripting::HostScriptQuery::of(&world).set_host_script_query_snapshot(
            crate::scripting::HostScriptQuerySnapshot {
                objects: vec![crate::scripting::HostScriptQueryObject {
                    id: 77,
                    x: 10.0,
                    y: 42.0,
                    z: 20.0,
                    ..Default::default()
                }],
                ..Default::default()
            },
        );
        let resolver = super::GameLogicAudioResolver {
            world: Arc::downgrade(&world),
        };
        let pos = resolver.resolve_object_position(77).expect("host object");
        assert_eq!(pos.x, 10.0);
        assert_eq!(pos.y, 20.0);
        assert_eq!(pos.z, 42.0);
    }

    #[test]
    fn leftover_the_audio_preserves_caller_volume() {
        // C++ addAudioEvent copies caller setVolume; INI volume is not written over it.
        let mut leftover = AudioEventRts::new("TrainCrush");
        leftover.set_volume(0.3);
        let engine = leftover_event_to_engine(&leftover);
        assert!((engine.get_volume() - 0.3).abs() < 1e-5);
    }

    #[test]
    fn leftover_the_audio_lose_focus_mutes_and_regain_restores() {
        // C++ WinMain WM_ACTIVATE → TheAudio->loseFocus / regainFocus.
        let audio = TheAudio::of(&crate::system::engine_stores::active_services());
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        {
            let mut guard = manager.lock().unwrap();
            guard.set_volume(0.8, EngineAudioAffect::AllSystemSetting);
        }
        audio.lose_focus();
        {
            let guard = manager.lock().unwrap();
            assert_eq!(guard.get_volume(EngineAudioAffect::Music), 0.0);
        }
        audio.regain_focus();
        {
            let guard = manager.lock().unwrap();
            assert!((guard.get_volume(EngineAudioAffect::Music) - 0.8).abs() < 1e-5);
        }
    }

    #[test]
    fn two_worlds_on_one_thread_play_in_their_own_audio_manager() {
        use crate::system::engine_stores::{new_world_services, with_world_services};
        let manager_of = |world: &Arc<crate::system::engine_stores::WorldServices>| {
            world
                .common()
                .system
                .the_audio
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone()
        };
        let a = new_world_services();
        let b = new_world_services();
        assert!(manager_of(&a).is_none() && manager_of(&b).is_none());
        // b is published; a's TheAudio still acts on a's own AudioManager.
        with_world_services(&b, || {
            TheAudio::of(&a).set_volume(0.25, EngineAudioAffect::AllSystemSetting);
        });
        let manager_a = manager_of(&a).expect("a's TheAudio created a's manager");
        assert!(manager_of(&b).is_none(), "b's engine never saw a's audio");
        with_world_services(&a, || {
            TheAudio::of(&b).set_volume(0.75, EngineAudioAffect::AllSystemSetting);
        });
        let manager_b = manager_of(&b).expect("b's manager");
        assert!(!Arc::ptr_eq(&manager_a, &manager_b));
        let music = |m: &Arc<std::sync::Mutex<game_engine::common::audio::game_audio::AudioManager>>| {
            m.lock().unwrap().get_volume(EngineAudioAffect::Music)
        };
        assert!((music(&manager_a) - 0.25).abs() < 1e-5);
        assert!((music(&manager_b) - 0.75).abs() < 1e-5);
        // Each engine's misc audio is its own MiscAudio.ini.
        a.common().ini.misc_audio.write().unit_promoted =
            game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_event_name("PromotedInA".into());
        assert_eq!(TheAudio::of(&a).misc_audio().unit_promoted.get_event_name(), "PromotedInA");
        assert_ne!(TheAudio::of(&b).misc_audio().unit_promoted.get_event_name(), "PromotedInA");
    }
}
