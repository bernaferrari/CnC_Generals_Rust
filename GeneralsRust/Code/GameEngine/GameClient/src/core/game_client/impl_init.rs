// GameClient construction, init, reset, and subsystem/translator bootstrap.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

impl GameClient {
    pub fn popup_save_load_bridge_installed(&self) -> bool {
        self.menu_manager.save_load_host_installed()
    }
    pub fn publish_popup_save_load_entries(
        &mut self,
        entries: Vec<crate::gui::callbacks::popup_save_load::PopupSaveLoadEntry>,
    ) {
        self.menu_manager.publish_save_load_entries(entries);
    }
    pub fn take_popup_save_load_requests(
        &mut self,
    ) -> Vec<crate::gui::callbacks::popup_save_load::HostPopupSaveLoadPublishedRequest> {
        self.menu_manager.take_save_load_requests()
    }

    pub fn in_game_callbacks(&self) -> &crate::gui::callbacks::ingame_callbacks::InGameUISystem {
        self.menu_manager.ingame_ui()
    }
    pub fn set_in_game_chat_type_owned(
        &mut self,
        chat_type: crate::gui::callbacks::ingame_callbacks::InGameChatType,
        local_player_active: bool,
    ) {
        let label = self
            .menu_manager
            .ingame_ui_mut()
            .assign_chat_type(chat_type.clone());
        if let Some(label) = label {
            let text = crate::gui::callbacks::ingame_callbacks::chat_type_label(
                &chat_type,
                local_player_active,
            );
            let _ = label.borrow_mut().set_text(&text);
        }
    }

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
        Self::new_with_input_authority(ClientInputAuthority::StandaloneCore)
    }

    /// Construction chooses the runtime owner but installs no translators or
    /// global state. The role remains fixed for this client's lifetime.
    pub fn new_with_input_authority(
        input_authority: ClientInputAuthority,
    ) -> GameClientResult<Self> {
        let window_manager_owner = WindowManagerOwner::new();
        let client = Self {
            window_manager_owner,
            prepare_background_layout: None,
            campaign_manager: crate::gui::campaign_manager::CampaignManager::new(),
            main_menu: crate::gui::shell::main_menu::MainMenu::new(),
            menu_manager: crate::gui::callbacks::menu_callbacks::MenuManager::new(),
            menu_host: crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default(),
            ui_renderer: None,
            ime: crate::gui::ime_manager::ImeManager::new(),
            script_display: crate::core::script_action_handler::ScriptDisplayReceiver::default(),
            message_stream: crate::message_stream::message_stream::MessageStream::new(),
            input_authority,
            shell_game_first_load: true,
            window_videos: crate::gui::window_video_manager::ClientWindowVideos::new(),
            load_screen_runtime: crate::gui::load_screen::LoadScreenRuntime::default(),
            load_screen_messages: Vec::new(),
            frame: 0,
            session_time_of_day: 2,
            session_weather_is_snowy: false,
            session_force_models_to_follow_time_of_day: false,
            session_force_models_to_follow_weather: true,
            session_lighting: Default::default(),
            command_hints: Default::default(),
            owned_world_animations: OwnedWorldAnimations::default(),
            receiving_draw_icon_ui: true,
            snow: crate::snow::SnowManager::new(),
            snow_definition: crate::snow::WeatherSetting::default(),
            snow_base_definition: crate::snow::WeatherSetting::default(),
            last_visual_time_frame: u32::MAX,
            // C++ W3DDisplay initializes lastFrame to ~0; reset does not clear it.
            last_native_draw_frame: u32::MAX,
            native_particle_capability_gaps: Vec::new(),
            local_player_id: 0,
            visual_world: None,
            last_applied_military_caption: None,
            last_applied_military_caption_remaining_ms: None,
            last_applied_cinematic_text: None,
            last_applied_cinematic_remaining_ms: None,
            cinematic_overlay_font: None,
            cinematic_overlay_frames: 0,
            last_live_ingame_hud_draw: LiveInGameHudDrawCounts::default(),

            render_bridge: crate::render_bridge::RenderBridge::new(),
            drawables: drawable_world::DrawableWorld::new(),
            drawable_toc: Vec::new(),
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
            presentation_shell_owner: std::sync::Arc::new(()),
            presentation_shell_generation: 0,
            target_frame_duration: Duration::from_millis(33),
            startup_sizzle_pending: false,
            initialized: false,
        };

        Ok(client)
    }

    /// Initializes all subsystems and resources
    /// Borrow this client's live campaign progress.
    pub fn campaign_manager(&self) -> &crate::gui::campaign_manager::CampaignManager {
        &self.campaign_manager
    }

    pub fn challenge_generals(&self) -> &crate::gui::challenge_generals::ChallengeGenerals {
        self.menu_manager.challenge_generals()
    }
    pub fn admit_challenge_generals(
        &mut self,
        catalog: crate::gui::challenge_generals::ChallengeGenerals,
    ) {
        self.menu_manager.admit_challenge_generals(catalog);
    }

    /// Mutably borrow this client's live campaign progress.
    pub fn campaign_manager_mut(&mut self) -> &mut crate::gui::campaign_manager::CampaignManager {
        &mut self.campaign_manager
    }

    /// Borrow the two receiving client owners needed by synchronous menu callbacks.
    pub fn campaign_and_menu_mut(
        &mut self,
    ) -> (
        &mut crate::gui::campaign_manager::CampaignManager,
        &mut crate::gui::shell::main_menu::MainMenu,
    ) {
        (&mut self.campaign_manager, &mut self.main_menu)
    }

    /// Borrow the receiver identity and the two disjoint menu recipients together.
    pub fn window_owner_campaign_and_menu_mut(
        &mut self,
    ) -> (
        &WindowManagerOwner,
        &mut crate::gui::campaign_manager::CampaignManager,
        &mut crate::gui::shell::main_menu::MainMenu,
    ) {
        (
            &self.window_manager_owner,
            &mut self.campaign_manager,
            &mut self.main_menu,
        )
    }

    /// Initialize a load screen using this client's persistent first-load state.
    /// Run a synchronous GameLogic operation with this client's load-screen recipient.
    /// The callback borrows both the adapter and client for the operation duration.
    pub fn with_load_screen_hooks<R>(
        &mut self,
        menu_audio: &mut dyn crate::gui::menu_audio::MenuAudioEffects,
        operation: impl FnOnce(&mut dyn gamelogic::helpers::LoadScreenHooks) -> R,
    ) -> Result<R, String> {
        let mut hooks = crate::helpers::GameClientLoadScreenHooks::new(self, menu_audio)?;
        Ok(operation(&mut hooks))
    }

    /// Paired Core UI launch. The receiver remains borrowed through original
    /// preparation, map load and completion, never installed globally.
    pub fn prepare_and_start_core_game(
        &mut self,
        game_mode: i32,
        difficulty: i32,
        rank_points: i32,
        is_load: bool,
        menu_audio: &mut dyn crate::gui::menu_audio::MenuAudioEffects,
    ) -> Result<(), String> {
        self.with_load_screen_hooks(menu_audio, |hooks| {
            gamelogic::helpers::TheGameLogic::prepare_new_game_with_load_screen_hooks(
                game_mode,
                difficulty,
                rank_points,
                Some(&mut *hooks),
            )?;
            gamelogic::helpers::TheGameLogic::start_new_game_with_load_screen_hooks(
                is_load,
                Some(&mut *hooks),
            )
        })?
    }

    /// Admit effective settings to this client without publishing a recipient.
    pub fn configure_shell_map(
        &mut self,
        settings: crate::gui::campaign_launch_host_bridge::ShellMapSettings,
    ) {
        self.menu_host.configure_shell_map(settings);
    }

    pub fn init_owned_load_screen(
        &mut self,
        kind: crate::gui::load_screen::LoadScreenKind,
        context: &crate::gui::load_screen::LoadScreenInitContext,
        menu_audio: &mut dyn crate::gui::menu_audio::MenuAudioEffects,
    ) -> Result<bool, String> {
        if !menu_audio.is_bound() {
            return Err("load screen requires bound receiving menu audio".into());
        }
        let (mut operation, mut pump) = self.split_load_screen_services(menu_audio);
        operation.init(kind, context, &mut pump)
    }

    pub fn init(&mut self) -> GameClientResult<()> {
        self.init_with_window_owner()
    }

    fn init_with_window_owner(&mut self) -> GameClientResult<()> {
        if self.initialized {
            return Err(GameClientError::InvalidOperation(
                "GameClient already initialized".to_string(),
            ));
        }

        register_live_game_client(self);
        Self::register_w3d_ghost_snapshot_bridges();

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
        // Each receiving GameClient owns its own campaign runtime. Definitions
        // are initialized here; construction itself remains inert.
        self.campaign_manager.init();

        self.initialized = true;

        log::info!("GameClient initialized successfully");
        Ok(())
    }

    pub fn mark_initialized(&mut self) {
        // Native Main owns its client directly. Snapshot, frame, bone and FX
        // operations receive that owner and never publish its address.
        self.campaign_manager.init();
        self.initialized = true;
    }

    /// Bind the committed logic world's presentation queues to this client.
    /// A staged candidate does not call this until its world is installed.
    pub fn bind_visual_world(&mut self, visual_world: gamelogic::helpers::ClientVisualHandle) {
        self.visual_world = Some(visual_world);
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
        // Explicit void world boundary, never a Drop cleanup. A poisoned
        // required service cannot silently discard owner retirement.
        self.try_invalidate_presentation_drawable_world()
            .expect("native Drawable retirement must complete before invalidation");
    }

    fn try_invalidate_presentation_drawable_world(&mut self) -> GameClientResult<()> {
        self.retire_native_drawable_particles(None)?;
        self.drawables.finish_native_retirement();
        self.drawables.clear_live_terrain_decal_handles();
        self.invalidate_presentation_shell();
        for drawable in self.drawables.drawable_map.values_mut() {
            drawable.reset_volatile_shroud_state();
        }
        self.drawables.drawable_map.clear();
        self.drawables.native_drawable_order.clear();
        self.drawables.ray_effects.reset();
        self.drawables.light_scene = Default::default();
        self.drawables.owned_laser_order.clear();
        self.drawables.drawable_object_map.clear();
        self.drawables.presentation_direct_drawable_bindings.clear();
        self.drawables.text_bearing_drawables.clear();
        self.shadow_map.clear();
        self.rendered_object_count = 0;
        self.render_bridge.reset_world_after_drawable_retirement();
        // Particle/native retirement has succeeded; clear both playback and
        // transport now, before another world can be installed or rendered.
        self.clear_world_animation_runtime_after_retirement()?;
        Ok(())
    }

    fn clear_world_animation_runtime_after_retirement(&mut self) -> GameClientResult<()> {
        let ui = self.subsystem_manager.in_game_ui.as_mut();
        self.owned_world_animations.clear_runtime();
        if let Some(ui) = ui {
            ui.replace_world_anims_from_presentation(&[]);
        }
        Ok(())
    }

    /// Admit a detached semantic CPU world without reconstructing or renumbering
    /// its Drawables. Retire the old world's effects while its visual handle is
    /// still bound. The application must call this before replacing GameLogic.
    ///
    /// This retains the existing required retirement capability and can fail;
    /// callers must not treat failure as an empty candidate or continue commit.
    pub fn retire_native_drawable_world_for_replace(&mut self) -> GameClientResult<()> {
        self.try_invalidate_presentation_drawable_world()?;
        self.command_hints.clear();
        Ok(())
    }
    /// Only after retirement succeeded. No callbacks or failure after moving
    /// the candidate, so its IDs/order and continuation are installed unchanged.
    pub fn install_prepared_native_drawable_world(&mut self, world: NativeDrawableWorld) {
        self.drawables = world;
    }

    /// Resets the game client for a new game
    pub fn reset(&mut self) -> GameClientResult<()> {
        self.reset_receiving_core(None)
    }

    fn reset_receiving_core(
        &mut self,
        world: Option<&mut gamelogic::system::game_logic::GameLogic>,
    ) -> GameClientResult<()> {
        // A paused standalone popup owns a world pause. A client-only reset
        // cannot infer that owner, and must not discard its resume receipt.
        if world.is_none()
            && self.with_window_event_effects(None, |effects| {
                crate::gui::callbacks::in_game_popup_message::receiving_popup_message(effects)
                    .is_some_and(|data| data.pause)
            })
        {
            return Err(GameClientError::SubsystemError(
                "paused popup reset requires its receiving Core world or native host retirement"
                    .into(),
            ));
        }
        self.window_manager_owner.clear_replay_steps();
        self.window_videos.reset();
        self.reset_with_window_owner(world)
    }

    fn reset_with_window_owner(
        &mut self,
        world: Option<&mut gamelogic::system::game_logic::GameLogic>,
    ) -> GameClientResult<()> {
        self.command_hints.clear();
        self.receiving_draw_icon_ui = true;
        self.startup_sizzle_pending = false;

        // C++ parity: show a blank transition window while subsystems reset.
        let reset_background = self.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                manager
                    .create_layout_with_windows("Menus/BlankWindow.wnd", effects)
                    .ok()
                    .map(|(layout, _)| {
                        layout.borrow_mut().hide(manager, false);
                        layout.borrow_mut().bring_forward(manager);
                        if let Some(window) = layout.borrow().get_first_window() {
                            window.borrow_mut().clear_status(WindowStatus::IMAGE);
                        }
                        layout
                    })
            })
        });

        // GameClient.cpp:435 resets InGameUI before destroying Drawables.
        // Its popup clear destroys the WND before resuming the paired world
        // (InGameUI.cpp:5187). Keep both operations in this same reset phase.
        self.subsystem_manager.reset_in_game_ui()?;
        self.with_window_event_effects(None, |effects| {
            crate::gui::callbacks::quit_menu::reset_quit_menu(effects);
            crate::gui::callbacks::popup_save_load::reset_save_load_menu(effects);
            crate::gui::callbacks::difficulty_select::reset_difficulty_select(effects);
        });
        match world {
            Some(world) => self.clear_core_popup_message(world),
            None => {
                let _ = self.clear_owned_popup_message();
            }
        }

        self.try_invalidate_presentation_drawable_world()?;

        if let Some(loaded) = self.loaded_map.take() {
            if let Some(ref asset_manager) = self.subsystem_manager.asset_manager {
                asset_manager.release_asset(loaded.handle);
            }
        }

        // Reset subsystems
        self.subsystem_manager
            .reset_all(self.script_display.fx_mut(), &self.session_lighting)?;
        self.drawables.particles.reset_runtime();
        self.window_manager_owner
            .with_manager(|manager| self.ime.reset(manager));

        // C++ GameClient.cpp:438-451 clears Drawables, then resets the
        // Display/Terrain/RayEffects for the next match. Drop the Rust
        // transient tracer/ray/light streams at the same boundary.
        if owns_live_game_client_slot(self) {
            crate::effects::clear_live_effects_after_drawables();
        }

        if let Some(layout) = reset_background {
            self.with_window_event_effects(None, |effects| {
                effects.with_manager(|manager, effects| manager.destroy_layout(&layout, effects))
            });
        }

        // C++ GameClient.cpp:451-452 TheSnowManager->reset() restores
        // m_isVisible=TRUE so SHOW_WEATHER=No does not stick across maps.
        self.snow.reset();

        // Clear TOC
        self.drawable_toc.clear();

        log::info!("GameClient reset completed");
        Ok(())
    }

    pub fn init_savegame_counter_bridge(&self) {
        register_save_load_mission_hooks(
            Some(Arc::new(|| {
                // C++ GameState.cpp:699-700 — clearGameData(FALSE) only if isInGame.
                let _ = TheGameLogic::clear_game_data();
            })),
            None,
        );
        // Campaign state is borrowed from the receiving GameClient at each
        // save/load operation; do not publish an ambient campaign callback.
        register_save_load_skirmish_hooks(
            Some(Arc::new(|| {
                // C++ GameStateMap.cpp:406 xferSnapshot(TheSkirmishGameInfo) v4.
                let bytes =
                    crate::gui::skirmish_setup::snapshot_skirmish_lobby().encode_xfer_bytes();
                if bytes.is_empty() { None } else { Some(bytes) }
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
        let mut keyboard = create_keyboard();
        keyboard.init()?;
        self.subsystem_manager.input_keyboard = Some(keyboard);

        // Create mouse
        let mut mouse = create_mouse();
        mouse.init()?;
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
        self.init_display_subsystems_with_replay_menu(None)
    }

    /// Native immutable Recorder policy is admitted before Shell binds authored windows.
    pub fn init_display_subsystems_with_replay_menu(
        &mut self,
        replay_menu: Option<crate::gui::shell::replay_menu::ReplayMenuInputs>,
    ) -> GameClientResult<()> {
        self.init_display_subsystems_with_replay_menu_scoped(replay_menu)
    }

    fn init_display_subsystems_with_replay_menu_scoped(
        &mut self,
        replay_menu: Option<crate::gui::shell::replay_menu::ReplayMenuInputs>,
    ) -> GameClientResult<()> {
        log::info!("init_display_subsystems: starting");
        // Definitions have been loaded; construction itself publishes nothing.
        if !self.menu_host.shell_map_is_configured() {
            if let Some(global) = get_global_data() {
                let definitions = global.read();
                self.menu_host.configure_shell_map(
                    crate::gui::campaign_launch_host_bridge::ShellMapSettings {
                        initial_file: definitions.initial_file.clone(),
                        enabled: definitions.shell_map_on,
                        map: definitions.shell_map_name.clone(),
                    },
                );
            }
        }

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
            if let Some(data) = get_global_data() {
                self.script_display
                    .fx_mut()
                    .set_gamma_state(data.read().display_gamma, 0.0, 1.0);
            }
            let display = Arc::new(Mutex::new(display));

            self.subsystem_manager.display = Some(display);
        } else if let Some(display) = self.subsystem_manager.display.as_ref() {
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

        if !self.subsystem_manager.window_manager_initialized {
            log::info!("init_display_subsystems: initializing WindowManager");
            self.window_manager_owner
                .with_manager(|manager| manager.init());
            self.subsystem_manager.window_manager_initialized = true;
        }

        if let Some(inputs) = replay_menu {
            self.window_manager_owner
                .with_manager(|manager| manager.configure_replay_menu_owned(inputs));
        }

        {
            log::info!("init_display_subsystems: initializing IME manager");
            self.with_window_event_effects(None, |effects| {
                effects.with_manager(|manager, effects| {
                    crate::gui::ime_manager::initialize_receiving_ime(manager, effects)
                })
            });
        }

        {
            log::info!("init_display_subsystems: initializing Shell");
            let mut shell = self.window_manager_owner.shell_handle();
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
            self.set_ui_renderer(renderer);
        }

        if self.subsystem_manager.display_strings.is_none() {
            let mut display_strings = DisplayStringManagerSubsystem::new();
            display_strings.init()?;
            self.subsystem_manager.display_strings = Some(Arc::new(Mutex::new(display_strings)));
        }

        crate::render_bridge::init_render_bridge();
        let _ = gamelogic::helpers::register_scene_submission(Arc::new(
            crate::render_bridge::RenderBridge::new(),
        ));

        log::debug!("init_display_subsystems done");
        Ok(())
    }

    /// Clone the handle to this client's sole native WND tree.
    pub fn window_manager_owner(&self) -> WindowManagerOwner {
        self.window_manager_owner.clone()
    }

    /// Borrow this client's native WND identity without cloning its owner handle.
    pub fn window_manager_owner_ref(&self) -> &WindowManagerOwner {
        &self.window_manager_owner
    }

    /// Borrow this client's message pipeline for its synchronous producer/consumer operations.
    pub fn message_stream(&self) -> &crate::message_stream::message_stream::MessageStream {
        &self.message_stream
    }

    /// Borrow this client's message pipeline for an explicit producer operation.
    pub fn message_stream_mut(
        &mut self,
    ) -> &mut crate::message_stream::message_stream::MessageStream {
        &mut self.message_stream
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
            register_terrain_visual_snapshot_block();
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
            crate::core::subsystems::register_in_game_ui_snapshot_block();
            register_radar_snapshot_block();
            let mut ui = ui;
            if self.input_authority == ClientInputAuthority::NativeHosted {
                ui.admit_native_control_bar_transport();
            }
            self.subsystem_manager.in_game_ui = Some(ui);
            // InGameUI.cpp1157 creates this authored transport during UI init;
            // Recorder::initControls later only changes existing visibility.
            self.with_window_event_effects(None, |effects| {
                effects.with_manager(|manager, effects| {
                    crate::gui::callbacks::replay_controls::create_replay_control(manager, effects)
                })
            });
        }

        // C++ GameClient.cpp:351-353 creates and initializes
        // `TheChallengeGenerals` immediately after TheInGameUI.  The Rust UI
        // object is now only a projection of Common's authoritative authored
        // persona table, which also serves headless Skirmish validation.
        self.menu_manager.init_challenge_generals();

        crate::core::subsystems::register_tactical_view_snapshot_block();

        crate::helpers::register_observer_audio_locality_hooks();
        crate::helpers::register_observer_audio_view_hooks();
        if self.input_authority == ClientInputAuthority::StandaloneCore {
            crate::helpers::register_legacy_core_control_bar_ui_ingress();
        }
        self.install_core_nonwindow_script_services();

        self.snow_base_definition =
            game_engine::common::ini::ini_weather::get_base_weather_setting()
                .as_ref()
                .map(crate::snow::WeatherSetting::from_common)
                .unwrap_or_default();
        self.snow_definition = self.snow_base_definition.clone();
        self.snow
            .init_with_definition(&self.snow_definition, &mut |low, high| {
                crate::GameClientRandomValue!(low, high)
            });

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

    fn install_core_nonwindow_script_services(&self) {
        if let Ok(mut engine_guard) = gamelogic::get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.set_action_handler(Some(Arc::new(GameClientScriptActionHandler::new())));
            }
        }
    }

    pub fn init_message_translators(&mut self) -> GameClientResult<()> {
        self.message_stream.clear_messages();
        self.menu_host.clear_pending();
        let stream = &mut self.message_stream;

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
        // Main's physical/raw input and per-frame camera step already own
        // the native view. Only standalone Core installs its LookAt state.
        if self.input_authority.runs_core_look_at() {
            register_translator(TranslatorFactory::create_look_at_translator(), 60);
        }

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

    pub fn create_frame_tick_message(&mut self) -> GameClientResult<()> {
        let frame = self.frame;
        let message = self
            .message_stream
            .append_message(GameMessageType::FrameTick(frame));
        message.append_timestamp_argument(frame);
        Ok(())
    }

    /// Supply lighting facts from the GameLogic instance that drives this client.
    /// This keeps message callbacks from consulting process-wide weather state.
    pub fn set_session_model_environment(
        &mut self,
        lighting: &game_engine::common::ini::ini_game_data::LightingContentRules,
        weather_is_snowy: bool,
    ) {
        self.session_time_of_day = lighting.time_of_day as i32;
        self.session_weather_is_snowy = weather_is_snowy;
        self.session_force_models_to_follow_time_of_day =
            lighting.force_models_to_follow_time_of_day;
        self.session_force_models_to_follow_weather = lighting.force_models_to_follow_weather;
        self.session_lighting.clone_from(lighting);
    }

    /// Transfer a local demo TOD input to the GameLogic instance driving this client.
    pub fn take_requested_session_time_of_day(
        &mut self,
    ) -> Option<game_engine::common::ini::TimeOfDay> {
        self.subsystem_manager
            .in_game_ui
            .as_mut()
            .and_then(|ui| ui.requested_session_time_of_day.take())
    }

    pub fn window_owner_campaign_menu_and_host_mut(
        &mut self,
    ) -> (
        &WindowManagerOwner,
        &mut crate::gui::campaign_manager::CampaignManager,
        &mut crate::gui::shell::main_menu::MainMenu,
        &mut crate::gui::campaign_launch_host_bridge::MenuHostMailbox,
        &mut crate::core::script_action_handler::ScriptDisplayReceiver,
        &mut crate::gui::window_video_manager::ClientWindowVideos,
        &mut crate::gui::ime_manager::ImeManager,
        crate::gui::callbacks::menu_callbacks::MenuCallbackRecipients<'_>,
        Option<&mut crate::core::subsystems::InGameUISubsystem>,
        Option<&mut crate::input::Mouse>,
    ) {
        self.script_display
            .bind(self.subsystem_manager.display.as_ref());
        (
            &self.window_manager_owner,
            &mut self.campaign_manager,
            &mut self.main_menu,
            &mut self.menu_host,
            &mut self.script_display,
            &mut self.window_videos,
            &mut self.ime,
            self.menu_manager.receiving_menus(self.input_authority),
            self.subsystem_manager.in_game_ui.as_mut(),
            self.subsystem_manager.input_mouse.as_mut(),
        )
    }
    pub fn main_menu(&self) -> &crate::gui::shell::main_menu::MainMenu {
        &self.main_menu
    }
    pub fn campaign_menu_and_host_mut(
        &mut self,
    ) -> (
        &mut crate::gui::campaign_manager::CampaignManager,
        &mut crate::gui::shell::main_menu::MainMenu,
        &mut crate::gui::campaign_launch_host_bridge::MenuHostMailbox,
    ) {
        self.script_display
            .bind(self.subsystem_manager.display.as_ref());
        (
            &mut self.campaign_manager,
            &mut self.main_menu,
            &mut self.menu_host,
        )
    }
    pub fn take_host_options_requests(
        &mut self,
    ) -> Vec<crate::gui::options_host_bridge::HostOptionsRequest> {
        self.menu_host.options.take_requests()
    }
    pub fn take_menu_host_requests(
        &mut self,
    ) -> Vec<crate::gui::campaign_launch_host_bridge::MenuHostRequest> {
        self.menu_host.take_requests()
    }
    pub fn has_menu_host_requests(&self) -> bool {
        self.menu_host.has_requests()
    }
    pub fn clear_pending_client_messages(&mut self) {
        if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
            crate::gui::control_bar::clear_host_control_bar_requests(ui);
            crate::gui::control_bar::clear_host_production_pause_requests(ui);
        }
        self.message_stream.clear_messages();
        self.menu_host.clear_pending();
        self.window_manager_owner
            .discard_pending_event_operations_for_world_boundary();
    }
    pub fn finish_source_menu_campaign_start(&mut self) {
        self.main_menu.complete_source_campaign_start();
    }
    pub fn finish_source_menu_campaign_shutdown(&mut self) {
        self.main_menu.complete_source_campaign_shutdown();
    }
    pub fn set_receiving_menu_cursor_visibility(&mut self, visible: bool) {
        if let Some(mouse) = self.subsystem_manager.input_mouse.as_mut() {
            mouse.set_cursor_visible(visible);
        }
    }
    pub fn reset_receiving_menu_placement(&mut self) {
        if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
            ui.reset_menu_placement();
        }
    }

    pub fn script_display_receiver(
        &mut self,
    ) -> &mut crate::core::script_action_handler::ScriptDisplayReceiver {
        self.script_display
            .bind(self.subsystem_manager.display.as_ref());
        &mut self.script_display
    }

    pub fn take_pending_display_mode(
        &mut self,
    ) -> Option<crate::display::display_fx::PendingDeviceMode> {
        self.script_display.take_pending_device_mode()
    }

    pub fn ime(&self) -> &crate::gui::ime_manager::ImeManager {
        &self.ime
    }
    pub fn ime_mut(&mut self) -> &mut crate::gui::ime_manager::ImeManager {
        &mut self.ime
    }

    pub fn clear_owned_popup_message(&mut self) -> Option<(bool, bool)> {
        self.with_window_event_effects(None, |effects| {
            crate::gui::callbacks::in_game_popup_message::clear_receiving_popup_message(effects)
                .map(|data| (data.pause, data.pause_music))
        })
    }
    pub fn with_window_event_effects<R>(
        &mut self,
        menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
        operation: impl FnOnce(&mut crate::gui::game_window::WindowEventEffects<'_>) -> R,
    ) -> R {
        self.with_window_and_control_bar_effects(menu_world, |effects, _videos| operation(effects))
    }

    /// Split the bar's movie domain beside the same client's callback recipients.
    pub fn with_window_and_control_bar_effects<R>(
        &mut self,
        menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
        operation: impl FnOnce(
            &mut crate::gui::game_window::WindowEventEffects<'_>,
            &mut crate::gui::window_video_manager::WindowVideoManager,
        ) -> R,
    ) -> R {
        self.with_window_and_control_bar_effects_and_audio(menu_world, None, operation)
    }
    pub fn with_window_event_effects_and_menu_audio<R>(
        &mut self,
        menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
        audio: &mut dyn crate::gui::menu_audio::MenuAudioEffects,
        operation: impl FnOnce(&mut crate::gui::game_window::WindowEventEffects<'_>) -> R,
    ) -> R {
        self.with_window_and_control_bar_effects_and_audio(menu_world, Some(audio), |effects, _| {
            operation(effects)
        })
    }
    fn with_window_and_control_bar_effects_and_audio<R>(
        &mut self,
        menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
        mut audio: Option<&mut dyn crate::gui::menu_audio::MenuAudioEffects>,
        operation: impl FnOnce(
            &mut crate::gui::game_window::WindowEventEffects<'_>,
            &mut crate::gui::window_video_manager::WindowVideoManager,
        ) -> R,
    ) -> R {
        self.script_display
            .bind(self.subsystem_manager.display.as_ref());
        let owner = &self.window_manager_owner;
        let mut messages = Vec::new();
        let mut effects = crate::gui::game_window::WindowEventEffects {
            cursor_mouse: self.subsystem_manager.input_mouse.as_mut(),
            control_bar_observation: None,
            menu_audio: audio
                .as_mut()
                .map(|audio| &mut **audio as &mut dyn crate::gui::menu_audio::MenuAudioEffects),
            input_provenance:
                crate::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
            standalone_view: None,
            client_frame: Some(self.frame),
            logic_frame: menu_world.map(|facts| facts.logic_frame),
            ui: self.subsystem_manager.in_game_ui.as_mut(),
            payloads: Default::default(),
            pending_window_callbacks: Default::default(),
            diplomacy_world: None,
            menus: Some(self.menu_manager.receiving_menus(self.input_authority)),
            manager: None,
            ime: Some(&mut self.ime),
            window_manager_owner: Some(owner),
            selected_window: None,
            emitted_messages: &mut messages,
            pending_control_bar_clicks: None,
            campaign_manager: Some(&mut self.campaign_manager),
            main_menu: Some(&mut self.main_menu),
            menu_world,
            menu_host: Some(&mut self.menu_host),
            script_display: Some(&mut self.script_display),
            window_videos: Some(&mut self.window_videos.challenge),
            pending_main_menu_actions: Vec::new(),
        };
        let result = operation(&mut effects, &mut self.window_videos.control_bar);
        owner.complete_event_callbacks(&mut effects);
        for message in messages {
            self.message_stream.append_message_value(message);
        }
        result
    }

    pub fn with_receiving_shell<R>(
        &mut self,
        menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
        operation: impl FnOnce(
            &mut crate::gui::shell::base::Shell,
            &mut crate::gui::game_window::WindowEventEffects<'_>,
        ) -> R,
    ) -> Option<R> {
        self.with_receiving_shell_audio(menu_world, None, operation)
    }
    pub fn with_receiving_shell_and_menu_audio<R>(
        &mut self,
        menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
        audio: &mut dyn crate::gui::menu_audio::MenuAudioEffects,
        operation: impl FnOnce(
            &mut crate::gui::shell::base::Shell,
            &mut crate::gui::game_window::WindowEventEffects<'_>,
        ) -> R,
    ) -> Option<R> {
        self.with_receiving_shell_audio(menu_world, Some(audio), operation)
    }
    fn with_receiving_shell_audio<R>(
        &mut self,
        menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
        mut audio: Option<&mut dyn crate::gui::menu_audio::MenuAudioEffects>,
        operation: impl FnOnce(
            &mut crate::gui::shell::base::Shell,
            &mut crate::gui::game_window::WindowEventEffects<'_>,
        ) -> R,
    ) -> Option<R> {
        self.script_display
            .bind(self.subsystem_manager.display.as_ref());
        let owner = &self.window_manager_owner;
        let mut messages = Vec::new();
        let mut effects = crate::gui::game_window::WindowEventEffects {
            cursor_mouse: self.subsystem_manager.input_mouse.as_mut(),
            control_bar_observation: None,
            menu_audio: audio
                .as_mut()
                .map(|audio| &mut **audio as &mut dyn crate::gui::menu_audio::MenuAudioEffects),
            input_provenance:
                crate::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
            standalone_view: None,
            client_frame: Some(self.frame),
            logic_frame: menu_world.map(|facts| facts.logic_frame),
            ui: self.subsystem_manager.in_game_ui.as_mut(),
            payloads: Default::default(),
            pending_window_callbacks: Default::default(),
            diplomacy_world: None,
            menus: Some(self.menu_manager.receiving_menus(self.input_authority)),
            manager: None,
            ime: Some(&mut self.ime),
            window_manager_owner: Some(owner),
            selected_window: None,
            emitted_messages: &mut messages,
            pending_control_bar_clicks: None,
            campaign_manager: Some(&mut self.campaign_manager),
            main_menu: Some(&mut self.main_menu),
            menu_world,
            menu_host: Some(&mut self.menu_host),
            script_display: Some(&mut self.script_display),
            window_videos: Some(&mut self.window_videos.challenge),
            pending_main_menu_actions: Vec::new(),
        };
        let result = owner.with_shell_effects_mut(&mut effects, operation);
        owner.complete_event_callbacks(&mut effects);
        for message in messages {
            self.message_stream.append_message_value(message);
        }
        result
    }

    /// Finish the synchronous WND event before admitting its messages to this client's stream.
    pub fn complete_window_event_messages(
        &mut self,
        mut messages: Vec<GameMessage>,
        menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
    ) {
        self.script_display
            .bind(self.subsystem_manager.display.as_ref());
        let owner = &self.window_manager_owner;
        let mut effects = crate::gui::game_window::WindowEventEffects {
            cursor_mouse: self.subsystem_manager.input_mouse.as_mut(),
            control_bar_observation: None,
            menu_audio: None,
            input_provenance:
                crate::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
            standalone_view: None,
            client_frame: Some(self.frame),
            logic_frame: menu_world.map(|facts| facts.logic_frame),
            ui: self.subsystem_manager.in_game_ui.as_mut(),
            payloads: Default::default(),
            pending_window_callbacks: Default::default(),
            diplomacy_world: None,
            menus: Some(self.menu_manager.receiving_menus(self.input_authority)),
            manager: None,
            ime: Some(&mut self.ime),
            window_manager_owner: Some(owner),
            selected_window: None,
            emitted_messages: &mut messages,
            pending_control_bar_clicks: None,
            campaign_manager: Some(&mut self.campaign_manager),
            main_menu: Some(&mut self.main_menu),
            menu_world,
            menu_host: Some(&mut self.menu_host),
            script_display: Some(&mut self.script_display),
            window_videos: Some(&mut self.window_videos.challenge),
            pending_main_menu_actions: Vec::new(),
        };
        owner.complete_event_callbacks(&mut effects);
        for message in messages {
            self.message_stream.append_message_value(message);
        }
    }

    fn take_completed_client_messages(
        &mut self,
        playback: bool,
        logic_frame: u32,
        game_mode: i32,
        mut view: Option<&mut crate::display::view::View>,
    ) -> GameClientResult<Vec<GameMessage>> {
        let client_frame = self.frame;
        let mut standalone =
            |owner: &crate::gui::window_manager::WindowManagerOwner,
             campaign_manager: &mut crate::gui::campaign_manager::CampaignManager,
             main_menu: &mut crate::gui::shell::main_menu::MainMenu,
             menu_host: &mut crate::gui::campaign_launch_host_bridge::MenuHostMailbox,
             menus: &mut crate::gui::callbacks::menu_callbacks::MenuCallbackRecipients<'_>,
             display: &mut crate::core::script_action_handler::ScriptDisplayReceiver,
             videos: &mut crate::gui::window_video_manager::ClientWindowVideos,
             ime: &mut crate::gui::ime_manager::ImeManager,
             ui: &mut crate::core::subsystems::InGameUISubsystem,
             cursor_mouse: Option<&mut crate::input::Mouse>,
             request,
             _emitted_messages: &mut Vec<GameMessage>| {
                if let game_engine::common::message_stream::WindowInputDispatch::Camera(action) =
                    request
                {
                    return view.as_deref_mut().map(|view| {
                        game_engine::common::message_stream::WindowInputDispatchResult::Camera(
                            crate::gui::callbacks::control_bar_callbacks::apply_camera_input(
                                view, action,
                            ),
                        )
                    });
                }
                let mut pending_clicks = Vec::new();
                let mut effects = crate::gui::game_window::WindowEventEffects {
                    cursor_mouse,
                    control_bar_observation: None,
                    menu_audio: None,
                    input_provenance:
                        crate::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
                    standalone_view: view.as_deref_mut(),
                    client_frame: Some(client_frame),
                    logic_frame: Some(logic_frame),
                    ui: Some(ui),
                    payloads: Default::default(),
                    pending_window_callbacks: Default::default(),
                    diplomacy_world: None,
                    menus: Some(menus.reborrow()),
                    manager: None,
                    ime: Some(ime),
                    window_manager_owner: Some(owner),
                    selected_window: None,
                    emitted_messages: _emitted_messages,
                    pending_control_bar_clicks: Some(&mut pending_clicks),
                    campaign_manager: Some(campaign_manager),
                    main_menu: Some(main_menu),
                    menu_world: None,
                    menu_host: Some(menu_host),
                    script_display: Some(display),
                    window_videos: Some(&mut videos.challenge),
                    pending_main_menu_actions: Vec::new(),
                };
                if let game_engine::common::message_stream::WindowInputDispatch::Ui(action) =
                    request
                {
                    let result = owner.with_manager(|manager| {
                        crate::message_stream::meta_event::execute_ui_input_action(
                            action,
                            manager,
                            &mut effects,
                            false,
                        )
                    });
                    owner.complete_event_callbacks(&mut effects);
                    return Some(
                        game_engine::common::message_stream::WindowInputDispatchResult::Ui(result),
                    );
                }
                let result = owner.try_with_manager(|manager| match request {
                    game_engine::common::message_stream::WindowInputDispatch::Camera(_) | game_engine::common::message_stream::WindowInputDispatch::OpaqueWindowAtScreen{..} | game_engine::common::message_stream::WindowInputDispatch::Display(_) | game_engine::common::message_stream::WindowInputDispatch::Ui(_) => {
                        unreachable!("display dispatch is completed before the window manager loan")
                    }
                    game_engine::common::message_stream::WindowInputDispatch::Mouse {
                        message,
                        x,
                        y,
                        data,
                        delta,
                    } => manager.process_mouse_event_with_delta_and_message_sink(
                        crate::gui::game_window::WindowMessage::from(message),
                        x,
                        y,
                        data as crate::gui::WindowMsgData,
                        delta,
                        &mut effects,
                    ),
                    game_engine::common::message_stream::WindowInputDispatch::Key {
                        key,
                        state,
                    } => manager.process_key_event_with_message_sink(key, state, &mut effects),
                });
                owner.complete_event_callbacks(&mut effects);
                drop(effects);
                assert!(
                    pending_clicks.is_empty(),
                    "bound ControlBar commands require a receiving native input driver"
                );
                result.map(|code| {
                    if code == crate::gui::game_window::WindowInputReturnCode::Used {
                        game_engine::common::message_stream::WindowInputDispatchResult::Used
                    } else {
                        game_engine::common::message_stream::WindowInputDispatchResult::NotUsed
                    }
                })
            };
        self.take_message_stream_with_native_input(
            playback,
            logic_frame,
            game_mode,
            &mut standalone,
        )
    }

    pub fn take_message_stream_with_native_input(
        &mut self,
        playback: bool,
        logic_frame: u32,
        game_mode: i32,
        driver: &mut NativeWindowInputDriver<'_>,
    ) -> GameClientResult<Vec<GameMessage>> {
        match self.take_message_stream_until_menu_host(
            playback,
            logic_frame,
            game_mode,
            driver,
            None,
        )? {
            game_engine::common::message_stream::PropagationProgress::Complete(messages) => {
                Ok(messages)
            }
            game_engine::common::message_stream::PropagationProgress::HostBoundary(_) => {
                Err(GameClientError::SubsystemError(
                    "menu lifecycle requires receiving host continuation".into(),
                ))
            }
        }
    }

    pub fn take_message_stream_until_menu_host(
        &mut self,
        playback: bool,
        logic_frame: u32,
        game_mode: i32,
        driver: &mut NativeWindowInputDriver<'_>,
        cursor: Option<game_engine::common::message_stream::PropagationCursor>,
    ) -> GameClientResult<game_engine::common::message_stream::PropagationProgress> {
        self.script_display
            .bind(self.subsystem_manager.display.as_ref());
        use game_engine::common::message_stream::PropagationProgress;
        let yielded = std::cell::Cell::new(false);
        let frame = self.frame;
        let time_of_day = self.session_time_of_day;
        let weather_is_snowy = self.session_weather_is_snowy;
        let force_models_to_follow_time_of_day = self.session_force_models_to_follow_time_of_day;
        let force_models_to_follow_weather = self.session_force_models_to_follow_weather;
        let owner = &self.window_manager_owner;
        let campaign = &mut self.campaign_manager;
        let menu = &mut self.main_menu;
        let host = &mut self.menu_host;
        let mut menus = self.menu_manager.receiving_menus(self.input_authority);
        let display = &mut self.script_display;
        let videos = &mut self.window_videos;
        let ime = &mut self.ime;
        let ui = self.subsystem_manager.in_game_ui.as_mut().ok_or_else(|| {
            GameClientError::SubsystemError(
                "message translation requires receiving InGameUI".into(),
            )
        })?;
        let mut mouse = self.subsystem_manager.input_mouse.as_mut();
        let mut dispatch = |request,
                            emitted: &mut Vec<GameMessage>,
                            client_ui: &mut dyn std::any::Any| {
            let ui = client_ui
                .downcast_mut::<crate::core::subsystems::InGameUISubsystem>()
                .expect("GameClient passes canonical UI to its translator driver");
            if let game_engine::common::message_stream::WindowInputDispatch::OpaqueWindowAtScreen{x,y} = request {
                return owner.try_with_manager_ref(|manager| game_engine::common::message_stream::WindowInputDispatchResult::OpaqueWindow(crate::gui::window_manager::opaque_window_at_screen(manager,x,y)));
            }

            if let game_engine::common::message_stream::WindowInputDispatch::Display(action) =
                request
            {
                return crate::message_stream::meta_event::execute_display_input_action(
                    action,
                    display,
                    owner,
                    Some(&mut videos.in_game),
                )
                .map(game_engine::common::message_stream::WindowInputDispatchResult::Display);
            }
            let result = driver(
                owner,
                campaign,
                menu,
                host,
                &mut menus,
                display,
                videos,
                ime,
                ui,
                mouse.as_deref_mut(),
                request,
                emitted,
            );
            yielded.set(host.has_requests());
            result
        };
        let mut progress = self
            .message_stream
            .propagate_messages_until_host_boundary(
                &mut dispatch,
                cursor,
                &mut || yielded.get(),
                &mut || {
                    owner.shell_is_active().map(|shell_active| {
                        game_engine::common::message_stream::TranslatorClientObservation {
                            frame,
                            logic_frame,
                            game_mode,
                            shell_active,
                            time_of_day,
                            weather_is_snowy,
                            force_models_to_follow_time_of_day,
                            force_models_to_follow_weather,
                        }
                    })
                },
                ui,
            )
            .map_err(|error| {
                GameClientError::SubsystemError(format!("Message stream update failed: {error}"))
            })?;
        if playback {
            if let PropagationProgress::Complete(messages) = &mut progress {
                messages.retain(|message| {
                    !is_network_command_message(message.get_type())
                        || matches!(message.get_type(), GameMessageType::LogicCRC(_))
                });
            }
        }
        Ok(progress)
    }

    pub fn pump_message_stream(
        &mut self,
        logic_frame: u32,
        game_mode: i32,
        view: Option<&mut crate::display::view::View>,
    ) -> GameClientResult<()> {
        let playback =
            game_engine::common::recorder::with_recorder(|recorder| recorder.is_playback())
                .unwrap_or(false);
        let completed_messages =
            self.take_completed_client_messages(playback, logic_frame, game_mode, view)?;
        if completed_messages.is_empty() {
            return Ok(());
        }
        // Standalone Core retains its original command queue route.
        route_commands_to_gamelogic(completed_messages, self.frame).map_err(|err| {
            GameClientError::SubsystemError(format!("Failed to route commands: {err}"))
        })?;
        Ok(())
    }

    /// Native Main lends the receiving command owner for this operation only.
    /// Do not also publish this batch to Core's independent player queues.
    pub fn pump_message_stream_to_receiving_owner(
        &mut self,
        recorder: &game_engine::common::recorder::Recorder,
        receive: &mut dyn FnMut(Vec<GameMessage>) -> GameClientResult<()>,
        logic_frame: u32,
        game_mode: i32,
    ) -> GameClientResult<()> {
        let completed_messages = self.take_completed_client_messages(
            recorder.is_playback(),
            logic_frame,
            game_mode,
            None,
        )?;
        if completed_messages.is_empty() {
            return Ok(());
        }
        receive(completed_messages)
    }

    pub fn admit_replay_control_visibility(&mut self, hidden: bool) {
        self.window_manager_owner.with_manager(|manager| {
            crate::gui::callbacks::replay_controls::apply_replay_control_visibility_owned(
                manager, hidden,
            )
        });
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
            // UI visibility is admitted by the lexical receiving-client boundary,
            // never a retained Send+Sync callback selecting a window manager.
        });
    }
}

#[cfg(test)]
mod native_world_install_tests {
    use super::*;

    #[test]
    fn whole_native_world_install_keeps_candidate_ids_order_and_peer() {
        let mut receiving = GameClient::new().unwrap();
        let mut peer = GameClient::new().unwrap();
        receiving
            .drawables
            .admit_native_drawable(DrawableId(7), Box::new(BasicDrawable::new(DrawableId(7))));
        peer.drawables
            .admit_native_drawable(DrawableId(7), Box::new(BasicDrawable::new(DrawableId(7))));
        peer.drawables.next_drawable_id = DrawableId(80);
        let mut candidate = NativeDrawableWorld::new();
        candidate
            .admit_native_drawable(DrawableId(41), Box::new(BasicDrawable::new(DrawableId(41))));
        candidate
            .admit_native_drawable(DrawableId(12), Box::new(BasicDrawable::new(DrawableId(12))));
        candidate.next_drawable_id = DrawableId(52);
        receiving
            .retire_native_drawable_world_for_replace()
            .unwrap();
        receiving.install_prepared_native_drawable_world(candidate);
        assert_eq!(
            receiving.drawables.native_drawable_order,
            vec![DrawableId(12), DrawableId(41)]
        );
        assert_eq!(receiving.drawables.next_drawable_id, DrawableId(52));
        assert!(
            !receiving
                .drawables
                .drawable_map
                .contains_key(&DrawableId(7))
        );
        assert_eq!(peer.drawables.native_drawable_order, vec![DrawableId(7)]);
        assert_eq!(peer.drawables.next_drawable_id, DrawableId(80));
        assert_eq!(receiving.drawables.alloc_drawable_id(), DrawableId(52));
        assert_eq!(peer.drawables.alloc_drawable_id(), DrawableId(80));
    }
}

#[cfg(test)]
mod load_screen_latch_owner_tests {
    use super::GameClient;

    #[test]
    fn shell_first_load_memory_is_owned_by_the_persistent_client() {
        let mut receiving = GameClient::new().unwrap();
        let peer = GameClient::new().unwrap();

        assert!(receiving.shell_game_first_load);
        assert!(peer.shell_game_first_load);
        receiving.shell_game_first_load = false;
        assert!(!receiving.shell_game_first_load);
        assert!(peer.shell_game_first_load);
    }
}

impl GameClient {
    pub fn set_ui_renderer(&mut self, renderer: crate::gui::UIRenderer) {
        self.ui_renderer = Some(renderer);
    }
    pub fn ui_renderer(&self) -> Option<&crate::gui::UIRenderer> {
        self.ui_renderer.as_ref()
    }
    pub fn ui_renderer_mut(&mut self) -> Option<&mut crate::gui::UIRenderer> {
        self.ui_renderer.as_mut()
    }
    pub fn split_script_display_and_ui_renderer(
        &mut self,
    ) -> (
        &mut crate::core::script_action_handler::ScriptDisplayReceiver,
        Option<&mut crate::gui::UIRenderer>,
    ) {
        (&mut self.script_display, self.ui_renderer.as_mut())
    }
}

impl GameClient {
    pub fn diplomacy(&self) -> &crate::gui::callbacks::diplomacy::DiplomacyCallbacks {
        self.menu_manager.diplomacy()
    }
}

impl GameClient {
    pub fn message_boxes(&self) -> &crate::gui::callbacks::message_box::MessageBoxSystem {
        self.menu_manager.message_boxes()
    }
}
