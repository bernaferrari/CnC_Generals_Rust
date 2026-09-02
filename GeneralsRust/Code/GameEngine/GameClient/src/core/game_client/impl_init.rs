// GameClient construction, init, reset, and subsystem/translator bootstrap.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

impl GameClient {
    /// Publish the logic-owned W3D ghost source and the Mesh/HLOD final
    /// capture bridge at the same lifecycle boundary as the live client.
    /// Other classes and incomplete HLOD animation facts remain fail-closed.
    fn register_w3d_ghost_snapshot_bridges() {
        let _ =
            gamelogic::object::w3d_ghost_object::register_w3d_ghost_snapshot_capture_source_hook(
                Some(Arc::new(|object_id| {
                    TheGameClient::get()
                        .and_then(|client| client.object_w3d_ghost_snapshot_source(object_id))
                })),
            );
        gamelogic::object::w3d_ghost_object::register_w3d_ghost_snapshot_capture_hook(Some(
            Arc::new(|object_id| {
                let source =
                    gamelogic::object::w3d_ghost_object::capture_w3d_ghost_snapshot_source(
                        object_id,
                    )?;
                let mut bridge_guard = crate::render_bridge::get_render_bridge().lock().ok()?;
                let bridge = bridge_guard.as_mut()?;
                let generation = bridge.capture_window_generation_for_source(&source)?;
                bridge.materialize_exact_mesh_w3d_ghost_capture_at(&source, generation)
            }),
        ));
    }

    /// Creates a new GameClient instance
    pub fn new() -> GameClientResult<Self> {
        let mut client = Self {
            frame: 0,
            last_visual_time_frame: u32::MAX,
            next_drawable_id: DrawableId(1),
            local_player_id: 0,
            last_applied_military_caption: None,
            last_applied_military_caption_remaining_ms: None,
            last_applied_cinematic_text: None,
            last_applied_cinematic_remaining_ms: None,
            cinematic_overlay_font: None,
            cinematic_overlay_frames: 0,
            letterbox_overlay_enabled: false,
            letterbox_overlay_fade_start: None,
            last_live_ingame_hud_draw: LiveInGameHudDrawCounts::default(),

            drawable_map: std::collections::HashMap::with_capacity(super::DRAWABLE_HASH_SIZE),
            drawable_object_map: std::collections::HashMap::new(),
            presentation_direct_drawable_bindings: std::collections::HashMap::new(),
            next_presentation_direct_binding_generation: 1,
            drawable_toc: Vec::new(),
            text_bearing_drawables: Vec::new(),
            loaded_map: None,
            translators: [TRANSLATOR_ID_INVALID; super::MAX_CLIENT_TRANSLATORS],
            num_translators: 0,
            command_translator: None,
            message_dispatcher: Arc::new(GameClientMessageDispatcher::new()),
            network_bridge: None,
            subsystem_manager: SubsystemManager::new(),
            audio_event_queue: Some(AudioEventQueue::new(256)),
            music_system: Some(MusicSystem::new()),
            speech_system: Some(SpeechSystem::new()),
            audio_engine: AudioEngine::new().ok(),
            shadow_map: std::collections::HashMap::new(),
            shadows_enabled: true,
            rendered_object_count: 0,
            last_update_time: Instant::now(),
            target_frame_duration: Duration::from_millis(33),
            startup_sizzle_pending: false,
            initialized: false,
        };

        client.set_local_player_id(0);

        Ok(client)
    }

    /// Initializes all subsystems and resources
    pub fn init(&mut self) -> GameClientResult<()> {
        if self.initialized {
            return Err(GameClientError::InvalidOperation(
                "GameClient already initialized".to_string(),
            ));
        }

        register_live_game_client(self);
        Self::register_w3d_ghost_snapshot_bridges();

        reset_script_action_runtime_state();
        init_video_player();

        // Set expected frame rate
        self.set_frame_rate(Duration::from_millis(33))?; // ~30 FPS

        // Initialize subsystems in dependency order
        self.init_core_subsystems()?;
        self.init_asset_systems()?;
        self.init_input_subsystems()?;
        self.init_display_subsystems()?;
        self.init_audio_subsystems()?;
        self.init_game_subsystems()?;
        self.post_process_display_strings()?;
        self.init_message_translators()?;
        self.init_network_bridge();
        self.init_recorder_bridge();
        self.init_savegame_counter_bridge();
        // C++ GameClient.cpp initializes TheCampaignManager during client init.
        get_campaign_manager().init();

        self.initialized = true;

        log::info!("GameClient initialized successfully");
        Ok(())
    }

    pub fn mark_initialized(&mut self) {
        // Main performs the WGPU-safe subset of GameClient initialization
        // directly (it must not create the legacy PlatformContext).  Register
        // that fully prepared client at the same lifecycle boundary as
        // `GameClient::init`, so published input frame state is live.
        register_live_game_client(self);
        Self::register_w3d_ghost_snapshot_bridges();
        get_campaign_manager().init();
        self.initialized = true;
    }

    /// Republish this client's address to the live snapshot slot after the
    /// owning engine has been moved to its final home.
    ///
    /// C++ parity: `TheGameClient` is a stable singleton pointer for the
    /// process lifetime. The Rust engine owns the client BY VALUE inside
    /// `CnCGameEngine`, which is constructed inside the boxed boot future and
    /// then moved out (`run_loop.rs`: `engine = Some(new_engine)`). The raw
    /// address registered by `mark_initialized`/`init` points into the
    /// future's memory and dangles the moment that move happens; the next
    /// `with_live_game_client_mut` (e.g. particle
    /// `resolve_attached_parent` → `query_live_drawable_fx_pose` at InGame
    /// entry) then dereferences freed heap and faults (SIGSEGV,
    /// generals_snap-2026-09-02-{050658,051348}.ips). Must be called once the
    /// engine has reached its final address, before any frame runs.
    pub fn republish_live_slot_after_engine_move(&mut self) {
        register_live_game_client(self);
    }

    /// Discard presentation-owned Drawable state after a successful world
    /// replacement.
    ///
    /// This is deliberately narrower than `reset`: it touches no client
    /// subsystems or map assets.  Before the drawables are dropped, reset their
    /// non-Xfer direct-shroud history so any retained implementation observes
    /// the same clear-frame/full-obscured defaults as a newly created
    /// Drawable.  Objectless and unbound drawables are not assigned a direct
    /// association; after invalidation they are absent just like all other
    /// presentation drawables.
    pub fn invalidate_presentation_drawable_world(&mut self) {
        for drawable in self.drawable_map.values_mut() {
            drawable.reset_volatile_shroud_state();
        }
        self.drawable_map.clear();
        self.drawable_object_map.clear();
        self.presentation_direct_drawable_bindings.clear();
        self.text_bearing_drawables.clear();
        self.shadow_map.clear();
        self.rendered_object_count = 0;
    }

    /// Resets the game client for a new game
    pub fn reset(&mut self) -> GameClientResult<()> {
        reset_script_action_runtime_state();
        Self::reset_global_video_player_streams();
        self.startup_sizzle_pending = false;

        // C++ parity: show a blank transition window while subsystems reset.
        let reset_background = with_window_manager(|manager| {
            manager
                .create_layout_with_windows("Menus/BlankWindow.wnd")
                .ok()
                .map(|(layout, _)| {
                    layout.borrow_mut().hide(false);
                    layout.borrow_mut().bring_forward();
                    if let Some(window) = layout.borrow().get_first_window() {
                        window.borrow_mut().clear_status(WindowStatus::IMAGE);
                    }
                    layout
                })
        });

        self.invalidate_presentation_drawable_world();

        if let Some(loaded) = self.loaded_map.take() {
            if let Some(ref asset_manager) = self.subsystem_manager.asset_manager {
                asset_manager.release_asset(loaded.handle);
            }
        }

        // Reset subsystems
        self.subsystem_manager.reset_all()?;

        if let Some(layout) = reset_background {
            with_window_manager(|manager| manager.destroy_layout(&layout));
        }

        // C++ GameClient.cpp:451-452 TheSnowManager->reset() restores
        // m_isVisible=TRUE so SHOW_WEATHER=No does not stick across maps.
        if let Some(snow) = crate::snow::get_snow_manager() {
            if let Ok(mut guard) = snow.lock() {
                guard.reset();
            }
        }

        // Clear TOC
        self.drawable_toc.clear();

        log::info!("GameClient reset completed");
        Ok(())
    }

    pub fn init_savegame_counter_bridge(&self) {
        register_drawable_id_counter_hooks(
            Some(Arc::new(Self::global_drawable_id_counter)),
            Some(Arc::new(Self::set_global_drawable_id_counter)),
        );
        register_save_load_mission_hooks(
            Some(Arc::new(|| {
                // C++ GameState.cpp:699-700 — clearGameData(FALSE) only if isInGame.
                let _ = TheGameLogic::clear_game_data();
            })),
            Some(Arc::new(|| {
                let campaign_manager = get_campaign_manager();
                (
                    campaign_manager.get_game_difficulty() as i32,
                    campaign_manager.get_rank_points(),
                )
            })),
        );
        register_save_load_campaign_hooks(Some(Arc::new(|| {
            let campaign_manager = get_campaign_manager();
            let campaign = campaign_manager.get_current_campaign()?;
            Some((
                campaign.name.clone(),
                campaign_manager
                    .get_current_mission_number()
                    .unwrap_or(-1),
                campaign_manager.get_current_map().unwrap_or_default(),
            ))
        })));
        register_campaign_manager_runtime_hooks(
            Some(Arc::new(|| get_campaign_manager().capture_logic_chunk_state())),
            Some(Arc::new(|state| {
                get_campaign_manager().apply_logic_chunk_state(state);
            })),
        );

        register_save_load_skirmish_hooks(
            Some(Arc::new(|| {
                // C++ GameStateMap.cpp:406 xferSnapshot(TheSkirmishGameInfo) v4.
                let bytes = crate::gui::skirmish_setup::snapshot_skirmish_lobby().encode_xfer_bytes();
                if bytes.is_empty() {
                    None
                } else {
                    Some(bytes)
                }
            })),
            Some(Arc::new(|payload| {
                crate::gui::skirmish_setup::restore_skirmish_lobby(payload);
            })),
        );
    }

    // Subsystem initialization methods

    pub fn init_core_subsystems(&mut self) -> GameClientResult<()> {
        log::debug!("Initializing core subsystems");
        self.init_draw_group_info()?;

        let bridge = crate::helpers::TacticalViewBridge::new();
        gamelogic::helpers::register_camera_view_bridge(std::sync::Arc::new(bridge));

        Ok(())
    }

    fn init_draw_group_info(&mut self) -> GameClientResult<()> {
        // C++ parity: DrawGroupInfo is loaded before the display string manager.
        let mut ini = INI::new();
        ini.load("Data/INI/DrawGroupInfo.ini", INILoadType::Overwrite)
            .map_err(|err| {
                GameClientError::SubsystemError(format!("DrawGroupInfo init failed: {err}"))
            })?;

        // C++ parity: localized DrawGroupInfo font overrides INI values.
        if let (Some(language), Some(draw_group_info)) = (
            get_global_language_read(),
            game_engine::common::ini::ini_draw_group_info::get_draw_group_info(),
        ) {
            let font = &language.draw_group_info_font;
            if !font.name.is_empty() {
                let mut draw_group_info = draw_group_info.write();
                draw_group_info.font.name = font.name.clone();
                draw_group_info.font.size = font.size;
                draw_group_info.font.is_bold = font.bold;
            }
        }

        if let Some(draw_group_info) =
            game_engine::common::ini::ini_draw_group_info::get_draw_group_info()
        {
            let draw_group_info = draw_group_info.read();
            crate::draw_group_info::sync_from_common_draw_group_info(&draw_group_info);
        }

        Ok(())
    }

    pub fn init_asset_systems(&mut self) -> GameClientResult<()> {
        log::debug!("Initializing asset management systems");
        let mut asset_config = AssetConfig::default();

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let data_path = cwd.join("Data");
        asset_config.base_path = if data_path.exists() { data_path } else { cwd };

        if let Ok(entries) = std::fs::read_dir(&asset_config.base_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("big"))
                {
                    asset_config.archive_paths.push(path);
                }
            }
        }

        asset_config.cache_size_mb = 512;
        asset_config.enable_hot_reload = cfg!(debug_assertions);
        asset_config.enable_validation = cfg!(debug_assertions);

        if self.subsystem_manager.asset_manager.is_none() {
            let asset_manager = AssetManager::new(asset_config).map_err(|e| {
                GameClientError::SubsystemError(format!("Asset manager initialization failed: {e}"))
            })?;

            let asset_manager = Arc::new(asset_manager);
            asset_manager.register_hot_reload_callbacks();
            asset_manager.register_streaming_callbacks();
            self.subsystem_manager.asset_manager = Some(asset_manager);
        }

        log::info!("Asset management systems initialized");
        Ok(())
    }

    pub fn init_input_subsystems(&mut self) -> GameClientResult<()> {
        log::debug!("Initializing input subsystems");

        // Create keyboard
        let keyboard = create_keyboard();
        keyboard.lock().unwrap_or_else(|e| e.into_inner()).init()?;
        self.subsystem_manager.input_keyboard = Some(keyboard);

        // Create mouse
        let mouse = create_mouse();
        mouse.lock().unwrap_or_else(|e| e.into_inner()).init()?;
        register_mouse_backend(mouse.clone());
        self.subsystem_manager.input_mouse = Some(mouse);

        Ok(())
    }

    fn init_localized_ui_resources(&mut self) -> GameClientResult<()> {
        log::info!("init_localized_ui_resources: loading GameText strings");
        let loaded_strings = GameText::init_runtime_strings().map_err(|err| {
            GameClientError::SubsystemError(format!("GameText init failed: {err}"))
        })?;
        log::info!("Loaded {loaded_strings} localized GameText strings");

        log::info!("init_localized_ui_resources: loading mapped images");
        game_engine::common::ini::ini_mapped_image::ImageCollection::load_global(512);
        log::info!("init_localized_ui_resources: load_global done, syncing");
        let imported = sync_mapped_images_from_common();
        log::info!("Imported {imported} mapped images into client image collection");
        log_startup_shell_mapped_images();

        Ok(())
    }

    pub fn init_display_subsystems(&mut self) -> GameClientResult<()> {
        log::info!("init_display_subsystems: starting");

        self.init_localized_ui_resources()?;

        // C++ parity: TheDisplay is created here, but we only need PlatformContext
        // if we're NOT already running inside a winit event loop.  When the engine
        // is created inside the Main event loop (the normal path on macOS), creating
        // a second EventLoop via PlatformContext::new() deadlocks.  In that case the
        // GraphicsDisplay is provided later via set_external_graphics_display().
        if self.subsystem_manager.display.is_none()
            && self.subsystem_manager.platform_context.is_some()
        {
            // We have a PlatformContext (standalone/test mode) — create GraphicsDisplay normally.
            let graphics_context =
                if let Some(context) = self.subsystem_manager.platform_context.as_ref() {
                    let size = context.window.inner_size();
                    log::info!(
                        "Platform context initialised (window {}x{})",
                        size.width,
                        size.height
                    );
                    context.graphics.clone()
                } else {
                    return Err(GameClientError::InitializationFailed(
                        "Platform context missing during display initialisation".to_string(),
                    ));
                };

            let mut display = GraphicsDisplay::new(graphics_context);
            display.init()?;
            let display = Arc::new(Mutex::new(display));
            register_script_display_bridge(Some(Arc::clone(&display)));
            Self::install_load_screen_presentation_pump(Arc::clone(&display));
            self.subsystem_manager.display = Some(display);
        } else if let Some(display) = self.subsystem_manager.display.as_ref() {
            register_script_display_bridge(Some(Arc::clone(display)));
            Self::install_load_screen_presentation_pump(Arc::clone(display));
        } else {
            clear_load_screen_presentation_pump();
        }
        // If neither PlatformContext nor display exists, we skip GraphicsDisplay
        // creation — the engine's own wgpu pipeline handles rendering.  All logical
        // UI subsystems below still initialise normally.

        if self.subsystem_manager.font_library.is_none() {
            log::info!("init_display_subsystems: initializing FontLibrary");
            let mut font_library = FontLibrarySubsystem::new();
            font_library.init()?;
            self.subsystem_manager.font_library = Some(Arc::new(Mutex::new(font_library)));
        }

        if self.subsystem_manager.header_templates.is_none() {
            log::info!("init_display_subsystems: initializing HeaderTemplates");
            let mut header_templates = HeaderTemplateManagerSubsystem::new();
            header_templates.init()?;
            self.subsystem_manager.header_templates = Some(Arc::new(Mutex::new(header_templates)));
        }

        if self.subsystem_manager.window_manager.is_none() {
            log::info!("init_display_subsystems: initializing WindowManager");
            let mut window_manager = WindowManagerSubsystem::new();
            window_manager.init()?;
            self.subsystem_manager.window_manager = Some(Arc::new(Mutex::new(window_manager)));
        }

        {
            log::info!("init_display_subsystems: initializing IME manager");
            let ime_manager = get_ime_manager();
            let mut ime = ime_manager.lock().map_err(|_| {
                GameClientError::SubsystemError("IME manager lock poisoned during init".to_string())
            })?;
            ime.init();
        }

        {
            log::info!("init_display_subsystems: initializing Shell");
            let mut shell = get_shell();
            shell.init().map_err(|err| {
                GameClientError::SubsystemError(format!("Shell init failed: {err}"))
            })?;
        }

        log::info!("init_display_subsystems: continuing after Shell init");

        if let Some(context) = self.subsystem_manager.platform_context.as_ref() {
            let config = context.graphics.config();
            let renderer = UIRenderer::new(
                context.graphics.device_arc(),
                context.graphics.queue_arc(),
                config.format,
            )
            .map_err(|err| {
                GameClientError::SubsystemError(format!("UI renderer initialization failed: {err}"))
            })?;
            set_ui_renderer(Arc::new(RwLock::new(renderer)));
        }

        if self.subsystem_manager.display_strings.is_none() {
            let mut display_strings = DisplayStringManagerSubsystem::new();
            display_strings.init()?;
            self.subsystem_manager.display_strings = Some(Arc::new(Mutex::new(display_strings)));
        }

        if self.subsystem_manager.hot_key_manager.is_none() {
            let mut hot_keys = HotKeyManagerSubsystem::new();
            hot_keys.init()?;
            self.subsystem_manager.hot_key_manager = Some(Arc::new(Mutex::new(hot_keys)));
        }

        crate::render_bridge::init_render_bridge();
        let _ = gamelogic::helpers::register_scene_submission(Arc::new(
            crate::render_bridge::RenderBridge::new(),
        ));

        log::debug!("init_display_subsystems done");
        Ok(())
    }

    pub fn init_audio_subsystems(&mut self) -> GameClientResult<()> {
        if self.subsystem_manager.audio.is_none() {
            let mut audio = AudioSubsystem::new()
                .map_err(|e| GameClientError::SubsystemError(format!("Audio init failed: {e}")))?;
            audio.init()?;
            let audio_arc = Arc::new(Mutex::new(audio));
            let hook_audio = Arc::clone(&audio_arc);
            register_fx_audio(Box::new(move |event, position| {
                if let Ok(mut guard) = hook_audio.lock() {
                    let _ = guard.play_event(event, position);
                }
            }));
            let button_audio = Arc::clone(&audio_arc);
            register_button_audio_hook(Box::new(move |event| {
                if let Ok(mut guard) = button_audio.lock() {
                    let _ = guard.play_event(event, None);
                }
            }));
            self.subsystem_manager.audio = Some(audio_arc);
        }

        if let Some(asset_manager) = self.subsystem_manager.asset_manager.as_ref() {
            crate::assets::register_audio_playback_bridge(Arc::clone(asset_manager));
        }

        Ok(())
    }

    pub fn init_game_subsystems(&mut self) -> GameClientResult<()> {
        register_campaign_snapshot_block();
        register_game_client_snapshot_block();
        crate::snow::register_weather_definition_parser();
        crate::eva::initialize_eva_system()
            .map_err(|err| GameClientError::SubsystemError(format!("Eva init failed: {err}")))?;
        if self.subsystem_manager.terrain_visual.is_none() {
            let mut terrain_visual = TerrainVisualStub::default();
            terrain_visual.init()?;
            let terrain_visual = Arc::new(Mutex::new(terrain_visual));
            self.subsystem_manager.terrain_visual = Some(terrain_visual);
        }

        if let Some(terrain_visual) = self.subsystem_manager.terrain_visual.as_ref() {
            let terrain_visual = Arc::clone(terrain_visual);
            register_terrain_visual_snapshot_block(Arc::clone(&terrain_visual));
            let _ = register_terrain_tree_hook(Arc::new(move |event| {
                if let Ok(mut terrain) = terrain_visual.lock() {
                    match event {
                        TerrainTreeEvent::Add(tree) => terrain.add_tree_registration(tree),
                        TerrainTreeEvent::Remove(drawable_id) => {
                            terrain.remove_tree_registration(drawable_id)
                        }
                    }
                }
            }));
        }
        let _ = register_terrain_unit_moved_hook(Arc::new(|info: TerrainUnitMovedInfo| {
            crate::terrain::notify_terrain_unit_moved(
                crate::terrain::TreeCollisionUnit {
                    object_id: info.object_id,
                    position: glam::Vec3::new(info.x, info.y, info.z),
                    direction_2d: glam::Vec2::new(info.dir_x, info.dir_y),
                    major_radius: info.major_radius,
                    minor_radius: info.minor_radius,
                    geometry_type: if info.is_box {
                        crate::terrain::TreeGeometryType::Box
                    } else {
                        crate::terrain::TreeGeometryType::Cylinder
                    },
                    crusher_level: info.crusher_level,
                    immobile: info.immobile,
                },
                info.frame,
            );
        }));

        if let Some(asset_manager) = self.subsystem_manager.asset_manager.as_ref() {
            let resolver = Arc::new(AnimationDurationResolver::new(Arc::clone(asset_manager)));
            let _ = register_animation_metadata_hook(Arc::new(move |animation_name| {
                resolver.get_duration_ms(animation_name)
            }));
        }

        init_fx_list_store()
            .map_err(|e| GameClientError::SubsystemError(format!("FXList init failed: {e}")))?;
        crate::fx_list::register_fx_list_manager_bridge();
        if let Err(e) = crate::effects::particle_manager::initialize_particle_system_manager() {
            return Err(GameClientError::SubsystemError(format!(
                "Particle system manager init failed: {e}"
            )));
        }
        crate::effects::particle_manager::register_particle_system_manager_bridge();
        register_particle_system_snapshot_block();
        if let Err(e) = initialize_weather_system() {
            return Err(GameClientError::SubsystemError(format!(
                "Weather system init failed: {e}"
            )));
        }

        if self.subsystem_manager.decal_manager.is_none() {
            let decals = Arc::new(Mutex::new(DecalManager::default()));
            register_decal_manager(Arc::clone(&decals));
            let _ = register_scorch_hook(Arc::new(move |position, size, type_id| {
                // C++ W3DGameClient::addScorch → terrain scorch only, no decal.
                let _ = crate::terrain::scorch_mesh::add_terrain_scorch(
                    [position.x, position.y, position.z],
                    size,
                    type_id,
                );
            }));
            self.subsystem_manager.decal_manager = Some(decals);
        }

        let mut prefs = UserPreferences::new();
        let _ = prefs.load("Options.ini");
        if let Some(value) = prefs.get_string("DynamicGameLOD") {
            game_engine::common::game_lod::set_dynamic_lod_from_string(value);
        }
        if let Some(value) = prefs.get_string("StaticGameLOD") {
            game_engine::common::game_lod::set_static_lod_from_string(value);
        }
        if let Some(value) = prefs.get_string("IdealStaticGameLOD") {
            game_engine::common::game_lod::set_ideal_static_lod_from_string(value);
        }
        // C++ W3DDisplay::init: if getStaticLODLevel()==UNKNOWN, find+set.
        game_engine::common::game_lod::ensure_static_lod_applied();


        if self.subsystem_manager.in_game_ui.is_none() {
            let mut ui = InGameUISubsystem::default();
            ui.init()?;
            let ui_arc = Arc::new(Mutex::new(ui));
            register_in_game_ui_backend(Arc::new(InGameUiHandle::new(ui_arc.clone())));
            crate::core::subsystems::register_in_game_ui_snapshot_block(ui_arc.clone());
            register_radar_snapshot_block(ui_arc.clone());
            self.subsystem_manager.in_game_ui = Some(ui_arc);
        }

        // C++ GameClient.cpp:351-353 creates and initializes
        // `TheChallengeGenerals` immediately after TheInGameUI.  The Rust UI
        // object is now only a projection of Common's authoritative authored
        // persona table, which also serves headless Skirmish validation.
        crate::gui::challenge_generals::init_challenge_generals();

        crate::core::subsystems::register_tactical_view_snapshot_block();

        crate::helpers::register_prepare_new_game_hooks();
        crate::helpers::register_load_screen_hooks();
        crate::helpers::register_observer_audio_locality_hooks();
        crate::helpers::register_observer_audio_view_hooks();
        crate::helpers::register_live_control_bar_hooks();
        self.install_script_action_handler();

        let _ = crate::snow::initialize_snow_manager();

        if self.subsystem_manager.video_player.is_none() {
            let mut video_player = VideoPlayerSubsystem;
            video_player.init()?;
            self.subsystem_manager.video_player = Some(Arc::new(Mutex::new(video_player)));
        }

        Ok(())
    }

    pub fn post_process_display_strings(&mut self) -> GameClientResult<()> {
        if let Some(display_strings) = self.subsystem_manager.display_strings.as_ref() {
            display_strings
                .lock()
                .map_err(|_| {
                    GameClientError::SubsystemError(
                        "Display string manager lock poisoned during post-process load".to_string(),
                    )
                })?
                .post_process_load()?;
        }
        Ok(())
    }

    fn install_script_action_handler(&self) {
        if let Ok(mut engine_guard) = gamelogic::get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.set_action_handler(Some(Arc::new(GameClientScriptActionHandler::new())));
            }
        }
    }

    fn reset_global_video_player_streams() {
        if let Some(player) = get_video_player() {
            if let Ok(mut guard) = player.lock() {
                if let Some(player) = guard.as_mut() {
                    player.reset();
                }
            }
        }
    }

    pub fn init_message_translators(&mut self) -> GameClientResult<()> {
        let mut stream = THE_MESSAGE_STREAM
            .write()
            .map_err(|_| GameClientError::SubsystemError("Message stream lock poisoned".into()))?;

        self.num_translators = 0;

        let mut register_translator = |translator, priority| {
            let id = stream.attach_translator(translator, priority);
            if self.num_translators < self.translators.len() {
                self.translators[self.num_translators] = id;
                self.num_translators += 1;
            }
        };

        register_translator(TranslatorFactory::create_window_translator(), 10);
        register_translator(TranslatorFactory::create_meta_event_translator(), 20);
        register_translator(TranslatorFactory::create_hot_key_translator(), 25);
        register_translator(TranslatorFactory::create_place_event_translator(), 30);
        register_translator(TranslatorFactory::create_gui_command_translator(), 40);
        register_translator(TranslatorFactory::create_selection_translator(), 50);
        register_translator(TranslatorFactory::create_look_at_translator(), 60);

        let command_translator = TranslatorFactory::create_command_translator();
        self.command_translator = Some(command_translator.clone());
        register_translator(
            Arc::new(RwLock::new(CommandTranslatorMessageAdapter::new(
                command_translator,
            ))),
            70,
        );

        register_translator(TranslatorFactory::create_hint_spy(), 100);

        let dispatcher_translator = Arc::new(RwLock::new(DispatcherTranslator::new(Arc::clone(
            &self.message_dispatcher,
        ))));
        let dispatcher_id = stream.attach_translator(dispatcher_translator, 999_999_999);
        if self.num_translators < self.translators.len() {
            self.translators[self.num_translators] = dispatcher_id;
            self.num_translators += 1;
        }

        Ok(())
    }

    fn init_network_bridge(&mut self) {
        if self.network_bridge.is_some() {
            return;
        }

        match NetworkBridgeHandle::install() {
            Some(handle) => {
                log::info!("Network command bridge installed");
                self.network_bridge = Some(handle);
            }
            None => {
                log::debug!(
                    "Network interface unavailable; network command bridge not installed yet"
                );
            }
        }
    }

    // Update methods

    pub fn create_frame_tick_message(&self) -> GameClientResult<()> {
        let mut stream = THE_MESSAGE_STREAM
            .write()
            .map_err(|_| GameClientError::SubsystemError("Message stream lock poisoned".into()))?;

        let frame = self.frame;
        let message = stream.append_message(GameMessageType::FrameTick(frame));
        message.append_timestamp_argument(frame);
        Ok(())
    }

    pub fn pump_message_stream(&self) -> GameClientResult<()> {
        let completed_messages = {
            let mut stream = THE_MESSAGE_STREAM.write().map_err(|_| {
                GameClientError::SubsystemError("Message stream lock poisoned".into())
            })?;
            stream.propagate_messages().map_err(|e| {
                GameClientError::SubsystemError(format!("Message stream update failed: {e}"))
            })?
        };

        if !completed_messages.is_empty() {
            let command_list_arc = get_command_list();
            let mut command_list = command_list_arc.write().map_err(|_| {
                GameClientError::SubsystemError("Command list lock poisoned".into())
            })?;
            command_list.append_message_list(completed_messages);
        }

        with_recorder_mut(|recorder| {
            recorder.set_current_frame(self.frame);
            recorder.update();
        });

        self.flush_command_list_to_logic()
    }

    fn flush_command_list_to_logic(&self) -> GameClientResult<()> {
        let command_list_arc = get_command_list();
        let commands = {
            let mut command_list = command_list_arc.write().map_err(|_| {
                GameClientError::SubsystemError("Command list lock poisoned".into())
            })?;
            command_list.reset_frame_counter();
            command_list.get_all_commands()
        };

        if commands.is_empty() {
            return Ok(());
        }

        route_commands_to_gamelogic(commands, self.frame).map_err(|err| {
            GameClientError::SubsystemError(format!("Failed to route commands: {err}"))
        })?;

        Ok(())
    }

    pub fn init_recorder_bridge(&self) {
        init_recorder();

        let command_source: Arc<dyn Fn() -> Vec<GameMessage> + Send + Sync> = Arc::new(|| {
            let command_list_arc = get_command_list();
            let read_result = command_list_arc.read();
            match read_result {
                Ok(command_list) => command_list.snapshot_messages(),
                Err(_) => Vec::new(),
            }
        });

        let command_sink: Arc<dyn Fn(GameMessage) + Send + Sync> = Arc::new(|message| {
            let command_list_arc = get_command_list();
            let write_result = command_list_arc.write();
            if let Ok(mut command_list) = write_result {
                command_list.append_message(message);
            }
        });

        let command_cull: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {
            let command_list_arc = get_command_list();
            let write_result = command_list_arc.write();
            if let Ok(mut command_list) = write_result {
                command_list.retain_messages(|msg| {
                    let msg_type = msg.get_type().clone();
                    !(is_network_command_message(&msg_type)
                        && !matches!(msg_type, GameMessageType::LogicCRC(_)))
                });
            }
        });

        with_recorder_mut(|recorder| {
            recorder.set_command_source(Some(command_source));
            recorder.set_command_sink(Some(command_sink));
            recorder.set_command_cull(Some(command_cull));
            recorder.set_game_mode_provider(Some(Arc::new(TheGameLogic::get_game_mode)));
            recorder.set_replay_control_visibility_hook(Some(Arc::new(|hide| {
                crate::gui::callbacks::replay_controls::apply_replay_control_visibility(hide);
            })));
        });
    }
}
