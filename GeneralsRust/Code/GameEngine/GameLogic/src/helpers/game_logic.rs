// TheGameLogic singleton bridge
//
// Split from `helpers.rs` for module-size parity.
// Observable behavior is unchanged.

/// TheGameLogic singleton bridge - global game state access (matching C++ TheGameLogic)
pub struct TheGameLogic;

static GAME_PAUSED: AtomicBool = AtomicBool::new(false);
static GAME_PAUSE_MUSIC: AtomicBool = AtomicBool::new(false);
static INTRO_MOVIE_PLAYING: AtomicBool = AtomicBool::new(false);
static START_NEW_GAME_REQUESTED: AtomicBool = AtomicBool::new(false);
static GAME_START_RANK_POINTS: AtomicI32 = AtomicI32::new(0);
static GLOBAL_DIFFICULTY: AtomicI32 = AtomicI32::new(0);
static LOCAL_ALLIED_VICTORY: AtomicBool = AtomicBool::new(false);
static HULK_MAX_LIFETIME_OVERRIDE: AtomicI32 = AtomicI32::new(-1);
static INPUT_ENABLED: AtomicBool = AtomicBool::new(true);

impl TheGameLogic {
    /// Destroy an object (matches C++ TheGameLogic::destroyObject)
    pub fn destroy_object(
        object: &crate::object::Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_id = object.get_id();
        if object_id == INVALID_ID {
            return Ok(());
        }

        let mut logic = crate::system::game_logic::get_game_logic()
            .lock()
            .map_err(|_| "Failed to lock game logic")?;
        logic.destroy_object(object_id);
        Ok(())
    }

    /// Destroy object by id (mirrors C++ overload used by behavior modules)
    pub fn destroy_object_by_id(
        object_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if object_id == INVALID_ID {
            return Ok(());
        }

        let mut logic = crate::system::game_logic::get_game_logic()
            .lock()
            .map_err(|_| "Failed to lock game logic")?;
        logic.destroy_object(object_id);
        Ok(())
    }

    pub fn queue_objects_changed_trigger_areas(object_id: ObjectID) {
        if object_id == INVALID_ID {
            return;
        }

        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.queue_objects_changed_trigger_areas(object_id);
        }
    }

    /// C++ `TheGameLogic::getFrameObjectsChangedTriggerAreas()`.
    pub fn get_frame_objects_changed_trigger_areas() -> UnsignedInt {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_frame_objects_changed_trigger_areas())
            .unwrap_or(0)
    }


    /// Get current frame number (mirrors C++ TheGameLogic::Get_Frame)
    pub fn get_frame() -> UnsignedInt {
        crate::system::game_logic::current_frame()
    }

    /// Get number of sleepy update modules queued in GameLogic.
    pub fn get_number_sleepy_updates() -> usize {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_number_sleepy_updates())
            .unwrap_or(0)
    }

    /// Get the next object-id counter value from GameLogic.
    pub fn get_object_id_counter() -> ObjectID {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_object_id_counter())
            .unwrap_or(1)
    }

    /// Get whether draw icon UI indicators are enabled.
    pub fn get_draw_icon_ui() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_draw_icon_ui())
            .unwrap_or(true)
    }

    /// Get whether behind-building markers are enabled.
    pub fn get_show_behind_building_markers() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_show_behind_building_markers())
            .unwrap_or(true)
    }

    /// Set whether behind-building markers are enabled.
    pub fn set_show_behind_building_markers(enabled: Bool) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_show_behind_building_markers(enabled);
        }
    }

    /// Set whether draw icon UI indicators are enabled.
    pub fn set_draw_icon_ui(enabled: Bool) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_draw_icon_ui(enabled);
        }
    }

    /// Get whether dynamic LOD is enabled.
    pub fn get_show_dynamic_lod() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_show_dynamic_lod())
            .unwrap_or(true)
    }

    /// Set whether dynamic LOD is enabled.
    pub fn set_show_dynamic_lod(enabled: Bool) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_show_dynamic_lod(enabled);
        }
    }

    /// Get whether scoring is enabled.
    pub fn is_scoring_enabled() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.is_scoring_enabled())
            .unwrap_or(true)
    }

    /// Enable/disable scoring.
    pub fn set_scoring_enabled(enabled: Bool) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_scoring_enabled(enabled);
        }
    }

    /// Get the global map/script rank cap.
    pub fn get_rank_level_limit() -> Int {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_rank_level_limit())
            .unwrap_or(1000)
    }

    /// Set the global map/script rank cap.
    pub fn set_rank_level_limit(level: Int) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_rank_level_limit(level);
        }
    }

    /// Set a runtime buildability override for a template.
    pub fn set_buildable_status_override(template_name: &str, status: Int) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_buildable_status_override(template_name, status);
        }
    }

    /// Find a runtime buildability override for a template.
    pub fn find_buildable_status_override(template_name: &str) -> Option<Int> {
        crate::system::game_logic::get_game_logic()
            .lock()
            .ok()
            .and_then(|logic| logic.find_buildable_status_override(template_name))
    }

    /// Set the paused state of the game (matches C++ TheGameLogic::setGamePaused).
    pub fn set_game_paused(paused: Bool, pause_music: Bool) {
        let current = GAME_PAUSED.load(Ordering::Relaxed);
        if current == paused {
            return;
        }

        GAME_PAUSED.store(paused, Ordering::Relaxed);
        GAME_PAUSE_MUSIC.store(paused && pause_music, Ordering::Relaxed);

        if let Some(hooks) = game_pause_hooks() {
            hooks.on_game_pause_state_changed(paused);
        }

        if let Some(audio) = TheAudio::get() {
            if paused {
                if pause_music {
                    audio.pause_audio(EngineAudioAffect::All);
                } else {
                    audio.pause_audio(EngineAudioAffect::Sound);
                    audio.pause_audio(EngineAudioAffect::Sound3D);
                    audio.pause_audio(EngineAudioAffect::Speech);
                }
            } else if pause_music {
                audio.resume_audio(EngineAudioAffect::All);
            } else {
                audio.resume_audio(EngineAudioAffect::Sound);
                audio.resume_audio(EngineAudioAffect::Sound3D);
                audio.resume_audio(EngineAudioAffect::Speech);
            }
        }
    }

    /// Return the paused state of the game.
    pub fn is_game_paused() -> Bool {
        GAME_PAUSED.load(Ordering::Relaxed)
    }

    /// Return whether pause music is active.
    pub fn is_pause_music_active() -> Bool {
        GAME_PAUSE_MUSIC.load(Ordering::Relaxed)
    }

    /// Set whether the intro movie is playing.
    pub fn set_intro_movie_playing(playing: Bool) {
        INTRO_MOVIE_PLAYING.store(playing, Ordering::Relaxed);
    }

    /// Return whether the intro movie is playing.
    pub fn is_intro_movie_playing() -> Bool {
        INTRO_MOVIE_PLAYING.load(Ordering::Relaxed)
    }

    /// Set whether player input is enabled (ScriptActions::doDisableInput / doEnableInput).
    pub fn set_input_enabled(enabled: Bool) {
        INPUT_ENABLED.store(enabled, Ordering::Relaxed);
    }

    /// Return whether player input is enabled.
    pub fn is_input_enabled() -> Bool {
        INPUT_ENABLED.load(Ordering::Relaxed)
    }

    /// Return whether the game is currently loading a map.
    pub fn is_loading_map() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.is_loading_map())
            .unwrap_or(false)
    }

    /// Return whether the game is currently in multiplayer mode.
    pub fn is_in_multiplayer_game() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.is_in_multiplayer_game())
            .unwrap_or(false)
    }

    /// Return whether the game is currently in skirmish mode.
    pub fn is_in_skirmish_game() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.is_in_skirmish_game())
            .unwrap_or(false)
    }

    /// Return whether the game is currently replaying.
    pub fn is_in_replay_game() -> Bool {
        Self::get_game_mode() == crate::system::game_logic::GAME_REPLAY
    }

    /// Return whether the game has entered any mode (matches C++ GameLogic::isInGame()).
    pub fn is_in_game() -> Bool {
        let mode = Self::get_game_mode();
        mode != crate::system::game_logic::GAME_NONE
    }

    /// Get the current game mode.
    pub fn get_game_mode() -> Int {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_game_mode())
            .unwrap_or(crate::system::game_logic::GAME_NONE)
    }

    pub fn begin_load_screen(game_mode: Int, loading_save_game: Bool) -> Bool {
        with_load_screen_hooks(|hooks| hooks.begin_load_screen(game_mode, loading_save_game))
    }

    pub fn update_load_progress(progress: Int) -> Bool {
        with_load_screen_hooks(|hooks| hooks.update_load_screen(progress))
    }

    pub fn run_load_screen_completion_transition(loading_save_game: Bool) -> Bool {
        with_load_screen_hooks(|hooks| {
            hooks.run_load_screen_completion_transition(loading_save_game)
        })
    }

    pub fn end_load_screen() -> Bool {
        with_load_screen_hooks(|hooks| hooks.end_load_screen())
    }

    /// Prepare for a new game (matches C++ GameLogic::prepareNewGame).
    pub fn prepare_new_game(game_mode: Int, difficulty: Int, rank_points: Int) {
        TheScriptEngine::set_global_difficulty(difficulty);
        Self::clear_start_new_game_request();
        if let Some(hooks) = prepare_new_game_hooks() {
            hooks.ensure_background_window();
        }
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_game_mode(game_mode);
        }
        if let Some(data) = get_engine_global_data() {
            let mut data = data.write();
            if !data.pending_file.is_empty() {
                data.map_name = data.pending_file.clone();
                data.pending_file.clear();
            }
        }
        Self::set_rank_points_to_add_at_game_start(rank_points);
        if game_mode != crate::system::game_logic::GAME_SHELL {
            if let Some(hooks) = prepare_new_game_hooks() {
                hooks.hide_shell();
            }
        }
    }

    /// Start a prepared game (C++ parity: GameLogic::startNewGame(FALSE)).
    pub fn start_new_game(is_load_game: Bool) -> Result<(), String> {
        if !is_load_game {
            Self::request_start_new_game();
            return Ok(());
        }

        Self::clear_start_new_game_request();

        let map_path = get_engine_global_data()
            .map(|data| data.read().map_name.clone())
            .unwrap_or_default();

        if map_path.is_empty() {
            return Err("Cannot start game: global map_name is empty".to_string());
        }

        // C++ parity: GameLogic::startNewGame(FALSE) records pristine map name before
        // map INI/sidecar resolution so save-directory maps can remap to original path.
        if !is_load_game {
            let mut state = game_engine::System::get_game_state();
            state.set_pristine_map_name(map_path.clone());
            if state.is_in_save_directory(std::path::Path::new(&map_path)) {
                log::error!(
                    "Pristine map name points to save directory map '{}'; sidecar lookup may diverge from C++ expected source-map semantics",
                    map_path
                );
            }
        }

        let params = crate::system::game_initialization::GameInitParams {
            map_path,
            game_mode: Self::to_init_game_mode(Self::get_game_mode()),
            difficulty: Self::to_init_difficulty(TheScriptEngine::get_global_difficulty()),
            num_players: Self::detect_player_count_for_init(),
            player_templates: Vec::new(),
            victory_type: crate::system::victory_conditions::VictoryType::Annihilation,
            score_limit: None,
            time_limit: None,
            fog_of_war_enabled: true,
            starting_resources: 0,
            ai_script: "DefaultAI".to_string(),
        };

        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_loading_map(true);
        }
        Self::begin_load_screen(Self::get_game_mode(), is_load_game);
        Self::update_load_progress(crate::system::game_initialization::LOAD_PROGRESS_START);

        let init_result =
            crate::system::game_initialization::GameInitializer::initialize_game(params)
                .map(|_| ())
                .map_err(|err| format!("Game initialization failed: {}", err));
        if init_result.is_ok() {
            Self::update_load_progress(crate::system::game_initialization::LOAD_PROGRESS_END);
            Self::run_load_screen_completion_transition(is_load_game);
        }
        Self::end_load_screen();

        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_loading_map(false);
        }

        init_result
    }

    /// Request that the next game-logic update complete a staged new-game start.
    pub fn request_start_new_game() {
        START_NEW_GAME_REQUESTED.store(true, Ordering::Relaxed);
    }

    /// Check whether a staged new-game start is waiting to be completed.
    pub fn is_start_new_game_requested() -> Bool {
        START_NEW_GAME_REQUESTED.load(Ordering::Relaxed)
    }

    /// Clear any staged new-game start request.
    pub fn clear_start_new_game_request() {
        START_NEW_GAME_REQUESTED.store(false, Ordering::Relaxed);
    }

    fn to_init_game_mode(mode: Int) -> crate::system::game_initialization::GameMode {
        match mode {
            crate::system::game_logic::GAME_SHELL => {
                crate::system::game_initialization::GameMode::ShellMap
            }
            crate::system::game_logic::GAME_SKIRMISH => {
                crate::system::game_initialization::GameMode::Skirmish
            }
            crate::system::game_logic::GAME_LAN | crate::system::game_logic::GAME_INTERNET => {
                crate::system::game_initialization::GameMode::Multiplayer
            }
            crate::system::game_logic::GAME_REPLAY => {
                crate::system::game_initialization::GameMode::Replay
            }
            _ => crate::system::game_initialization::GameMode::SinglePlayer,
        }
    }

    fn to_init_difficulty(difficulty: Int) -> crate::system::game_initialization::GameDifficulty {
        match difficulty {
            0 => crate::system::game_initialization::GameDifficulty::Easy,
            2 => crate::system::game_initialization::GameDifficulty::Hard,
            3 => crate::system::game_initialization::GameDifficulty::Brutal,
            _ => crate::system::game_initialization::GameDifficulty::Normal,
        }
    }

    fn detect_player_count_for_init() -> usize {
        if let Ok(sides_guard) = crate::sides_list::get_sides_list().read() {
            let count = sides_guard.get_num_sides().max(1) as usize;
            return count.min(crate::system::player_init::MAX_PLAYER_COUNT);
        }

        if let Ok(player_list) = crate::player::ThePlayerList().read() {
            let count = player_list.iter().count();
            if count > 0 {
                return count.min(crate::system::player_init::MAX_PLAYER_COUNT);
            }
        }

        2
    }

    /// Reset game logic state (matches C++ TheGameLogic::clearGameData).
    pub fn clear_game_data() -> Result<(), String> {
        if !Self::is_in_game() {
            return Err("clear_game_data called while not in game".to_string());
        }

        // C++ parity: GameLogic::clearGameData() performs an engine reset, then forces
        // GAME_NONE and conditionally marks the engine quitting for initial-file startup.
        if let Some(engine) = get_game_engine() {
            let mut guard = engine.lock();
            let _ = futures::executor::block_on(guard.reset());
        }

        crate::system::game_logic::reset_game_logic()?;

        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_game_mode(crate::system::game_logic::GAME_NONE);
        }

        let has_initial_file = get_engine_global_data()
            .map(|data| !data.read().initial_file.is_empty())
            .unwrap_or(false);
        if has_initial_file {
            if let Some(engine) = get_game_engine() {
                engine.lock().set_quitting(true);
            }
        }

        Ok(())
    }

    /// Set rank points used at game start.
    pub fn set_rank_points_to_add_at_game_start(points: Int) {
        GAME_START_RANK_POINTS.store(points, Ordering::Relaxed);
    }

    /// Get rank points used at game start.
    pub fn get_rank_points_to_add_at_game_start() -> Int {
        GAME_START_RANK_POINTS.load(Ordering::Relaxed)
    }

    /// Get the global weapon bonus set used by all weapons.
    pub fn get_global_weapon_bonus_set() -> Option<WeaponBonusSet> {
        crate::system::game_logic::get_game_logic()
            .lock()
            .ok()
            .map(|logic| logic.get_global_weapon_bonus_set().clone())
    }

    /// Try to get current frame but propagate locking errors.
    pub fn try_get_frame() -> Result<UnsignedInt, String> {
        crate::system::game_logic::try_current_frame()
    }

    /// Find object by ID using the global registry (mirrors C++ TheGameLogic::findObjectByID).
    pub fn find_object_by_id(
        id: ObjectID,
    ) -> Option<std::sync::Arc<std::sync::RwLock<crate::object::Object>>> {
        // Wave 281: dual_world_registry_unavailable no longer forces None;
        // try OBJECT_REGISTRY then GameLogic.objects via try_lock.
        if let Some(obj) = OBJECT_REGISTRY.get_object(id) {
            return Some(obj);
        }
        // Host path: objects live on GameLogic.objects. try_lock avoids deadlock
        // if we are already inside GameLogic::update.
        crate::system::game_logic::get_game_logic()
            .try_lock()
            .ok()
            .and_then(|logic| logic.find_object_by_id(id))
    }

    /// Register a newly created object handle with the global registry.
    pub fn register_object(
        object: std::sync::Arc<std::sync::RwLock<crate::object::Object>>,
    ) -> Result<(), GameError> {
        let id = { object.read().map_err(|_| GameError::LockError)?.get_id() };
        OBJECT_REGISTRY.register_object(id, &object);
        register_legacy_object(&object);

        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            let _ = logic.track_object_in_update_list(object);
        }

        Ok(())
    }

    /// Remove an object handle from the registry.
    pub fn remove_object(object_id: ObjectID) {
        OBJECT_REGISTRY.unregister_object(object_id);
        unregister_legacy_object(object_id);
    }

    /// Deselect object (mirroring GameLogic::deselectObject selection flow).
    pub fn deselect_object(
        object: &crate::object::Object,
        mask: PlayerMaskType,
        affect_client: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use crate::commands::{get_selection_manager, SelectionType};

        let object_id = object.get_id();
        let selection_manager = get_selection_manager();
        let mut manager = selection_manager
            .write()
            .map_err(|_| "Failed to lock selection manager")?;

        if let Ok(list) = crate::player::player_list().read() {
            for (player_index, player_arc) in list.iter().enumerate() {
                let bit = PlayerMaskType::from_bits_truncate(1u32 << (player_index as u32));
                if !mask.contains(bit) {
                    continue;
                }

                let legacy_obj = crate::ai::object_registry::get_legacy_object(object_id);
                let mut actually_removed = false;

                if let Ok(mut player) = player_arc.write() {
                    if let Some(legacy_obj) = legacy_obj.as_ref() {
                        let mut group = crate::ai::AIGroup::new(0);
                        player.get_current_selection_as_ai_group(&mut group);
                        if let Ok(deleted) = group.remove(legacy_obj) {
                            actually_removed = true;
                            if deleted {
                                player.set_currently_selected_ai_group(None);
                            } else {
                                player.set_currently_selected_ai_group(Some(&group));
                            }
                        }
                    } else if player.remove_object_from_current_selection(object_id) {
                        actually_removed = true;
                    }

                    if actually_removed && affect_client {
                        if let Some(drawable) = object.get_drawable() {
                            TheInGameUI::deselect_drawable(&drawable);
                        }
                    }
                }

                if actually_removed {
                    if let Some(selection) = manager.get_player_selection(player_index as i32) {
                        selection.select_objects(vec![object_id], SelectionType::Remove);
                    }
                }
            }
        }
        Ok(())
    }

    /// Select object (mirroring GameLogic::selectObject selection flow).
    pub fn select_object(
        object: &crate::object::Object,
        create_new_selection: bool,
        mask: PlayerMaskType,
        affect_client: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use crate::commands::{get_selection_manager, SelectionType};

        if !object.is_mass_selectable() && !create_new_selection {
            return Ok(());
        }

        let object_id = object.get_id();
        let can_add_to_group = object.get_ai_update_interface().is_some()
            || object.is_any_kind_of(&[KindOf::Structure, KindOf::AlwaysSelectable]);
        let selection_manager = get_selection_manager();
        let mut manager = selection_manager
            .write()
            .map_err(|_| "Failed to lock selection manager")?;

        let selection_type = if create_new_selection {
            SelectionType::Replace
        } else {
            SelectionType::Add
        };

        if let Ok(list) = crate::player::player_list().read() {
            for (player_index, player_arc) in list.iter().enumerate() {
                let bit = PlayerMaskType::from_bits_truncate(1u32 << (player_index as u32));
                if !mask.contains(bit) {
                    continue;
                }

                let legacy_obj = crate::ai::object_registry::get_legacy_object(object_id);
                let mut added_to_group = false;

                if let Ok(mut player) = player_arc.write() {
                    if let Some(legacy_obj) = legacy_obj.as_ref() {
                        let mut group = crate::ai::AIGroup::new(0);
                        let _ = group.add(legacy_obj.clone());
                        added_to_group = group.get_count() > 0;
                        if create_new_selection {
                            player.set_currently_selected_ai_group(Some(&group));
                        } else {
                            player.add_ai_group_to_current_selection(&group);
                        }
                    } else if create_new_selection {
                        if can_add_to_group {
                            player.set_current_selection_to_object(object_id);
                            added_to_group = true;
                        } else {
                            player.set_currently_selected_ai_group(None);
                        }
                    } else if can_add_to_group {
                        player.add_object_to_current_selection(object_id);
                        added_to_group = true;
                    }
                }

                if added_to_group || (legacy_obj.is_none() && can_add_to_group) {
                    if let Some(selection) = manager.get_player_selection(player_index as i32) {
                        selection.select_objects(vec![object_id], selection_type);
                    }
                }

                if affect_client {
                    if let Some(drawable) = object.get_drawable() {
                        TheInGameUI::select_drawable(&drawable);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get hulk max lifetime override.
    pub fn get_hulk_max_lifetime_override() -> Int {
        HULK_MAX_LIFETIME_OVERRIDE.load(Ordering::Relaxed)
    }

    /// Set hulk max lifetime override (used by scripting)
    pub fn set_hulk_max_lifetime_override(lifetime: Int) {
        HULK_MAX_LIFETIME_OVERRIDE.store(lifetime, Ordering::Relaxed);
    }

    /// Register an update module with the global scheduler.
    pub fn register_update_module(
        object_id: ObjectID,
        module: UpdateModulePtr,
        wake_frame: UnsignedInt,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mutex = crate::system::game_logic::get_game_logic();
        let mut logic = mutex
            .lock()
            .map_err(|err| format!("Failed to lock GameLogic: {}", err))?;

        logic.unregister_update_module(object_id, module.clone());

        if wake_frame == 0 {
            logic.register_normal_update_module(object_id, module);
        } else {
            logic.register_sleepy_update_module(object_id, module, wake_frame);
        }

        Ok(())
    }

    /// Remove an update module from the global scheduler.
    pub fn unregister_update_module(
        object_id: ObjectID,
        module: UpdateModulePtr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mutex = crate::system::game_logic::get_game_logic();
        let mut logic = mutex
            .lock()
            .map_err(|err| format!("Failed to lock GameLogic: {}", err))?;

        logic.unregister_update_module(object_id, module);
        Ok(())
    }

    /// Set wake frame for an object's update modules (matches C++ setWakeFrame)
    pub fn set_wake_frame(object_id: ObjectID, sleep_time: crate::modules::UpdateSleepTime) {
        use crate::object_manager::get_object_manager;

        let current_frame = Self::get_frame();
        let manager_arc = get_object_manager();
        let manager_lock = &*manager_arc;
        let Ok(manager) = manager_lock.read() else {
            return;
        };
        let Some(instance_arc) = manager.get_object(object_id) else {
            if let Some(object_arc) = Self::find_object_by_id(object_id) {
                if let Ok(mut object) = object_arc.write() {
                    object.wake_update_modules_after(current_frame, sleep_time);
                }
            }
            return;
        };
        let instance_lock = &*instance_arc;
        if let Ok(mut instance) = instance_lock.write() {
            instance.wake_all_update_modules_after(current_frame, sleep_time);
        };

        if let Some(object_arc) = Self::find_object_by_id(object_id) {
            if let Ok(mut object) = object_arc.write() {
                object.wake_update_modules_after(current_frame, sleep_time);
            }
        }
    }
}

#[cfg(test)]
mod game_logic_tests {
    use super::TheGameLogic;
    use crate::system::game_logic::{get_game_logic, GAME_NONE, GAME_SHELL};

    #[test]
    fn is_in_game_matches_cpp_for_shell_mode() {
        let mut logic = get_game_logic().lock().unwrap();
        let previous_mode = logic.get_game_mode();

        logic.set_game_mode(GAME_NONE);
        drop(logic);
        assert!(!TheGameLogic::is_in_game());

        let mut logic = get_game_logic().lock().unwrap();
        logic.set_game_mode(GAME_SHELL);
        drop(logic);
        assert!(TheGameLogic::is_in_game());

        let mut logic = get_game_logic().lock().unwrap();
        logic.set_game_mode(previous_mode);
    }
}
