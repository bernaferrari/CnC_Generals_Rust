#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
impl CnCGameEngine {
    pub async fn new(window: Arc<Window>, command_line: Arc<CommandLineArgs>) -> Result<Self> {
        let total_timer = InitTimer::new("🎮 Engine initialization");
        info!("🎮 Initializing Command & Conquer Generals Zero Hour Game Engine");
        info!("📋 Starting subsystem initialization sequence...");

        let debug_overlay = command_line.wants_debug_overlay();
        let no_audio_command_line = command_line.no_audio && Self::allow_debug_startup_flags();
        if no_audio_command_line {
            info!("🔇 Audio disabled via -noaudio");
        }
        if command_line.quick_start {
            info!("⚡ QuickStart enabled: skipping intro sequences (handled by SAGE runtime).");
        }

        init_subsystem_manager()
            .map_err(|err| warn!("Subsystem manager initialization failed: {err}"))
            .ok();
        // C++ parity: GameEngine::init() line 679 — HideControlBar() after init completes.
        {
            let _ = game_client::gui::callbacks::control_bar_callbacks::hide_control_bar(true);
        }
        Self::apply_command_line_overrides(&command_line);
        // C++ GameEngine::init (GameEngine.cpp:360-361): TheGameLODManager
        // after parseCommandLine, before Water/Weather INI. init() loads
        // GameLOD.ini + GameLODPresets.ini, applies Options.ini, then
        // setStaticLODLevel. Live execute cap reads the resulting
        // GlobalData.writable.frames_per_second_limit / use_fps_limit
        // (GameEngine.cpp:535 setFramesPerSecondLimit).
        game_engine::common::game_lod::load_game_lod_ini_presets_and_options();
        // C++ GameEngine::init createAudioManager / TheAudio (GameEngine.cpp:410).
        let _ = game_engine::common::audio::game_audio::initialize_global_audio_manager();
        // C++ GameClient::init loads Data/INI/Eva.ini into TheEva (Eva.cpp:43-57).
        // Live ticks update_eva_system without leftover GameClient::init, so
        // playMessage would drop on an empty check-info table without this.
        #[cfg(feature = "game_client")]
        if let Err(err) = game_client::eva::initialize_eva_system() {
            warn!("Eva.ini load failed: {err}");
        }

        Self::apply_startup_audio_channel_flags();
        // C++ parity: initialize startup RNG stream during engine init.
        game_engine::common::random_value::init_random();
        Self::remove_legacy_duplicate_inizh_big_best_effort();

        init_game_state_system()
            .map_err(|err| warn!("Game state system init failed: {err}"))
            .ok();

        // Initialize subsystems first (matches C++ GameEngine initialization order)
        if let Some(handle) = get_subsystem_manager() {
            let manager = handle.lock();
            if manager.is_initialized() {
                info!("✅ Core subsystems initialized");
            } else {
                warn!("Subsystem manager available but not initialized");
            }
        } else {
            warn!("Subsystem manager missing after initialization attempt");
        }

        let runtime_host_headless = RuntimeHostBridge::is_headless_mode(command_line.as_ref());
        let runtime_host_active =
            RuntimeHostBridge::is_runtime_host_requested(command_line.as_ref());
        let size = window.inner_size();

        // Initialize WW3D engine to own the swapchain/device
        let mut engine_config = EngineConfig::default();
        engine_config.width = size.width.max(1);
        engine_config.height = size.height.max(1);

        if runtime_host_headless {
            if let Err(err) = ww3d_engine::init_headless(engine_config).await {
                if !matches!(err, EngineError::AlreadyInitialised) {
                    return Err(anyhow::anyhow!(
                        "Failed to initialize WW3D headless engine: {err:?}"
                    ));
                }
            }
        } else if let Err(err) = ww3d_engine::init_with_window(window.clone(), engine_config).await
        {
            if !matches!(err, EngineError::AlreadyInitialised) {
                return Err(anyhow::anyhow!("Failed to initialize WW3D engine: {err:?}"));
            }
        }

        // Initialize C++ SAGE equivalent graphics system
        info!("🎨 Initializing GraphicsSystem (C++ SAGE equivalent)...");
        let graphics_timer = InitTimer::new("✅ GraphicsSystem initialized");
        let device =
            ww3d_engine::device().map_err(|e| anyhow::anyhow!("WW3D device unavailable: {e:?}"))?;
        let queue =
            ww3d_engine::queue().map_err(|e| anyhow::anyhow!("WW3D queue unavailable: {e:?}"))?;
        let color_format = ww3d_engine::color_format()
            .map_err(|e| anyhow::anyhow!("WW3D color format unavailable: {e:?}"))?;
        let depth_format = ww3d_engine::depth_format()
            .map_err(|e| anyhow::anyhow!("WW3D depth format unavailable: {e:?}"))?;
        let graphics_system = GraphicsSystem::new(device, queue, color_format, depth_format)?;
        graphics_timer.finish();

        // Initialize render pipeline
        info!("🔧 Initializing RenderPipeline (C++ SAGE equivalent)...");
        let pipeline_timer = InitTimer::new("✅ RenderPipeline initialized");
        let mut render_pipeline = RenderPipeline::initialize(&graphics_system)?;
        pipeline_timer.finish();

        // C++ parity: BIG archives MUST be initialized BEFORE asset manager so textures/INI can be read
        info!("📦 Initializing BIG archive file system...");
        if let Err(err) = crate::assets::archive::init_archive_file_system().await {
            warn!(
                "BIG archive file system init failed: {err}. Continuing without archive support."
            );
        }

        // Host combat WeaponStore: guarantee init even if asset manager init fails.
        // (AssetManager also inits this; this is a hard early guarantee for create_object.)
        // Note: Common ini_weapon store (filled later from archives) is separate;
        // host create_object binds via gamelogic WeaponStore — see weapon_bootstrap.
        if let Err(e) = gamelogic::initialize_weapon_store() {
            warn!("Early WeaponStore init failed (will retry via assets): {e}");
        }
        let seeded = crate::game_logic::ensure_host_weapon_store();
        if seeded > 0 {
            info!(
                "Early host WeaponStore bootstrap registered {} templates (archive load may add more)",
                seeded
            );
        }
        // Host movement LocomotorStore: seed BasicHumanLocomotor (~20) etc. so
        // create_object binds retail-ish max_speed without golden_skirmish boost.
        let loco_seeded = crate::game_logic::ensure_host_locomotor_store();
        if loco_seeded > 0 {
            info!(
                "Early host LocomotorStore bootstrap registered {} templates (archive load may add more)",
                loco_seeded
            );
        }

        // C++ parity: initialize the asset manager during engine setup so startup loading
        // can reuse the live archive/definition caches immediately.
        info!("🎨 Initializing C&C Asset Manager during engine setup...");
        let asset_timer = Instant::now();
        crate::assets::manager::init_asset_manager(
            graphics_system.device_arc().as_ref(),
            graphics_system.queue_arc().as_ref(),
        )
        .await
        .map_err(|err| {
            warn!("Asset manager init failed: {err}. Continuing without assets.");
            err
        })
        .ok();
        let asset_duration = asset_timer.elapsed();

        if let Err(err) = crate::assets::archive::init_big_archive_file_reader() {
            warn!(
                "BIG archive texture reader init failed: {err}. Continuing without archive texture reader."
            );
        }
        info!(
            "BIG archive texture reader wired ({:.2}s total asset setup)",
            asset_duration.as_secs_f32()
        );

        // Model preloading will be done after graphics system is ready
        // This is handled in the run loop after engine creation
        // Models are preloaded later; keep placeholder timer for consistency if needed.

        // No direct wgpu initialization needed - graphics system handles this

        // Initialize platform-specific message handling
        let message_handler = create_platform_message_handler();
        let mut message_processor = WindowMessageProcessor::new(message_handler);
        message_processor.attach_window(window.clone());

        // Initialize audio system unless disabled
        let (audio_output, audio_handle) = if no_audio_command_line {
            (None, None)
        } else {
            match OutputStream::try_default() {
                Ok((output, handle)) => (Some(output), Some(handle)),
                Err(e) => {
                    warn!(
                        "Failed to initialize audio output: {e}; C++ init would quit when music is not ready, so startup will exit"
                    );
                    (None, None)
                }
            }
        };
        let audio_startup_requires_quit =
            Self::startup_audio_should_quit(no_audio_command_line, audio_handle.is_some());

        let mut ui_sound_cache: HashMap<String, Arc<[u8]>> = HashMap::new();
        if audio_handle.is_some() {
            if let Some(manager_arc) = crate::assets::manager::get_asset_manager() {
                let mut manager = manager_arc.lock().unwrap_or_else(|e| e.into_inner());
                for &path in &[
                    crate::ui::sound_files::BUTTON_CLICK,
                    crate::ui::sound_files::BUTTON_HOVER,
                ] {
                    match manager.extract_file(path).await {
                        Ok(data) => {
                            ui_sound_cache
                                .insert(path.to_string(), Arc::from(data.into_boxed_slice()));
                        }
                        Err(err) => {
                            debug!("UI sound '{}' unavailable: {}", path, err);
                        }
                    }
                }
            }
        }

        // Initialize game systems.
        // CombatSystem + PathfindingSystem live on GameLogic (sole host authority).
        let game_logic = GameLogic::initialize();
        let resource_manager = ResourceManager::new();
        let mut save_file_manager = SaveFileManager::new();
        save_file_manager
            .init()
            .map_err(|err| anyhow::anyhow!("Save file manager init failed: {err}"))?;

        // Initialize minimap renderer now that we know the world bounds.
        let world_bounds = game_logic.world_bounds();
        render_pipeline.initialize_minimap_renderer(
            graphics_system.device_arc(),
            graphics_system.queue_arc(),
            world_bounds,
        )?;

        let camera_target = Vec3::ZERO;
        let camera_position = Vec3::new(0.0, 310.0, -403.99988);
        let camera_zoom = 1.0;
        let aspect = size.width as f32 / size.height as f32;
        let projection_matrix = perspective_rh_from_horizontal_fov(
            DEFAULT_VIEW_FOV_RADIANS,
            aspect,
            DEFAULT_VIEW_NEAR_CLIP,
            DEFAULT_VIEW_FAR_CLIP,
        );

        let build_map_cache = {
            let global = game_engine::common::global_data::read();
            global.writable.build_map_cache
        };
        // Main owns AuthorityOnly physical-world input.  Snapshot the live
        // C++ GlobalData preference at creation, then accept later Options
        // changes only through its typed host bridge.
        let use_alternate_mouse = game_engine::common::global_data::read().use_alternate_mouse;
        let (draw_rmb_scroll_anchor, move_rmb_scroll_anchor) =
            game_engine::common::user_preferences::load_rmb_scroll_anchor_preferences();

        // C++ GameEngine::init updates MapCache before shell-map startup checks.
        game_client::map_util::refresh_map_cache();

        let startup_initial_file = Self::startup_initial_file_from_command_line(
            &command_line,
            Self::allow_debug_startup_flags(),
        );

        let (startup_initial_map, startup_initial_replay) =
            Self::split_startup_initial_file(startup_initial_file);

        if let Some(initial_map) = startup_initial_map.as_ref() {
            let mut global = game_engine::common::global_data::write();
            global.writable.shell_map_on = false;
            global.writable.play_intro = false;
            global.pending_file = initial_map.clone();
        }

        if let Some(initial_replay) = startup_initial_replay.as_ref() {
            // C++ parity: `.rep` startup is delegated to the recorder path and does not
            // force shell/intro flags or clear `pending_file` here.
            info!("Replay startup override requested: {}", initial_replay);
        }

        // C++ treats `-file` as the startup initial file, not as a direct `-map` request.
        let startup_map_requested_from_cli = false;
        let startup_map_requested_from_initial_file = startup_initial_map.is_some();
        let startup_replay_requested_from_initial_file = startup_initial_replay.is_some();
        let startup_requested_map = startup_initial_map.clone();
        let startup_requested_replay = startup_initial_replay.clone();
        let start_in_menu = startup_requested_map.is_none() && startup_requested_replay.is_none();
        let startup_shell_map = Self::configured_startup_shell_map();
        let map_to_load = if start_in_menu {
            startup_shell_map
        } else {
            startup_requested_map
        };

        // C++ parity (GameEngine.cpp): if intro is disabled by INI/flags, startup enters
        // the post-intro state after shell-map validation.
        Self::sync_after_intro_when_intro_disabled();

        let startup_load_state = if build_map_cache || audio_startup_requires_quit {
            StartupLoadState::Complete
        } else {
            Self::spawn_startup_map_load(
                start_in_menu,
                map_to_load,
                startup_map_requested_from_cli,
                startup_map_requested_from_initial_file,
                startup_requested_replay.clone(),
                startup_replay_requested_from_initial_file,
                command_line.player_name.clone(),
            )
        };

        let camera_offset = camera_position - camera_target;
        let camera_orbit_distance = camera_offset.length().max(1.0);
        let camera_pitch_radians = camera_offset
            .y
            .atan2(Vec2::new(camera_offset.x, camera_offset.z).length());
        let camera_yaw_radians = camera_offset.x.atan2(camera_offset.z);
        let view_matrix = Mat4::look_at_rh(camera_position, camera_target, Vec3::Y);

        let pending_shell_model_prewarm = if start_in_menu {
            // C++ shell startup does not run this extra Rust-only synchronous prewarm loop.
            // Keep shell-scene warmup disabled here and rely on the render pipeline's
            // incremental non-blocking budget instead so the menu can paint first.
            VecDeque::new()
        } else {
            info!("Skipping blocking startup model preload for gameplay startup");
            VecDeque::new()
        };

        let mut ui_manager = UIManager::new(size.width, size.height);
        if command_line.quick_start {
            ui_manager.enable_quick_start();
        }
        ui_manager
            .initialize()
            .map_err(|err| anyhow::anyhow!("failed to initialize startup UI: {err}"))?;
        // C++ parity: loading visuals are GameClient .wnd load screens
        // (ShellGameLoadScreen/SinglePlayerLoadScreen/etc.), not Main/src/ui.
        ui_manager.suspend_for_shell_overlay();
        let initial_state = GameState::Loading;
        let pending_state = None;

        let mut engine = Self {
            window: window.clone(),
            command_line,

            // C++ SAGE equivalent rendering subsystems
            graphics_system,
            render_pipeline,

            message_processor,
            audio_output,
            audio_handle,
            background_music: None,
            sound_effects: Vec::new(),
            ui_sound_cache,

            // Default boot flow should land in the menu unless explicitly quick-starting.
            current_state: initial_state,
            pending_state,
            startup_load_state,
            startup_target_state: Some(if start_in_menu {
                GameState::Menu
            } else {
                GameState::InGame
            }),
            startup_start_in_menu: start_in_menu,
            last_loading_title_update: None,
            startup_last_reported_progress: 0.0,
            startup_loading_phase: DEFAULT_LOADING_PHASE.to_string(),
            startup_last_progress_change_at: Instant::now(),
            startup_last_stall_warning_at: None,
            startup_stall_events: 0,
            startup_max_stall_duration: Duration::ZERO,
            startup_health_summary_logged: false,
            last_caustic_warmup_attempt: None,
            loading_overlay_active: false,
            #[cfg(feature = "game_client")]
            active_load_screen: None,
            shell_menu_active: false,

            #[cfg(feature = "game_client")]
            game_client: {
                game_client::helpers::register_live_control_bar_hooks();
                // Main owns the offline Rust simulation.  Route Control Bar
                // interactions into its typed host bridge rather than the
                // separate legacy GameLogic global queue.
                game_client::gui::control_bar::clear_host_control_bar_requests();
                game_client::gui::control_bar::set_host_control_bar_bridge_enabled(true);
                game_client::gui::options_host_bridge::set_host_options_bridge_enabled(true);
                game_client::gui::campaign_launch_host_bridge::set_host_campaign_launch_bridge_enabled(
                    true,
                );
                game_client::render_bridge::init_render_bridge();
                let _ = gamelogic::helpers::register_scene_submission(std::sync::Arc::new(
                    game_client::render_bridge::RenderBridge::new(),
                ));
                game_client::core::game_client::GameClient::new()
                    .map_err(|e| anyhow::anyhow!("Failed to create GameClient: {e}"))?
            },
            #[cfg(feature = "game_client")]
            control_bar: game_client::gui::control_bar::ControlBar::new(),

            game_logic,
            presentation_terrain_cache: PresentationTerrainCache::default(),
            last_presentation_frame: None,
            host_direct_visual_world_epoch: 1,
            host_match_game_mode: None,
            host_match_map_name: None,
            host_match_local_player_id: None,
            host_match_ai_difficulty: None,
            host_match_visual_speed: None,
            host_match_time_frozen: None,
            host_match_total_play_time: None,
            host_match_logic_frame: None,
            host_match_logic_steps: None,
            host_match_in_replay: None,
            host_match_in_shell: None,
            host_match_local_team: None,
            host_match_diplomacy_players: None,
            host_match_known_template_names: None,
            host_match_unlocked_sciences: None,
            host_match_camera_follow_active: None,
            host_match_camera_follow_position: None,
            host_match_camera_follow_id: None,
            camera_follow_factor: -1.0,
            host_match_local_barracks_ids: None,
            host_match_local_producer_ids: None,
            host_match_local_unfinished_producer_ids: None,
            host_match_local_team_sample_pos: None,
            host_match_over: None,
            host_match_victory_label: None,
            host_match_victory_winner: None,
            host_match_victory_summary: None,
            host_match_selected_ids: None,
            host_match_alive_object_ids: None,
            host_match_purchasable_sciences: None,
            host_match_local_science_purchase_points: None,
            host_match_local_supplies: None,
            host_match_special_power_ready_ids: None,
            host_match_boot_victory_condition: None,
            host_legal_build_cache_frame: None,
            host_legal_build_cache: std::collections::HashMap::new(),
            host_match_script_camera_max_height: None,
            host_match_script_camera_pitch: None,
            host_match_in_multiplayer: None,
            host_match_world_bounds: None,
            host_match_first_opponent_id: None,
            gameworld_shadow: if crate::gameworld_shadow::gameworld_shadow_enabled() {
                Some(crate::gameworld_shadow::GameWorldShadow::new(4096))
            } else {
                None
            },
            last_gameworld_presentation_entity_count: 0,
            last_ui_state: None,
            resource_manager,
            save_file_manager,
            camera_position,
            camera_target,
            scripted_camera_constraint_widen: None,
            camera_zoom,
            camera_zoom_target: None,
            camera_zoom_start: camera_zoom,
            camera_zoom_duration: 0.0,
            camera_zoom_elapsed: 0.0,
            camera_zoom_ease_in: 0.0,
            camera_zoom_ease_out: 0.0,
            camera_orbit_distance,
            camera_pitch_radians,
            camera_pitch_target: None,
            camera_pitch_start: camera_pitch_radians,
            camera_pitch_duration: 0.0,
            camera_pitch_elapsed: 0.0,
            camera_pitch_ease_in: 0.0,
            camera_pitch_ease_out: 0.0,
            camera_fx_pitch: 1.0,
            camera_yaw_radians,
            camera_yaw_target: None,
            camera_yaw_start: camera_yaw_radians,
            camera_yaw_duration: 0.0,
            camera_yaw_elapsed: 0.0,
            camera_yaw_ease_in: 0.0,
            camera_yaw_ease_out: 0.0,
            camera_shake_offset: Vec3::ZERO,
            camera_shake_rotation: Vec3::ZERO,
            screen_shake_intensity: 0.0,
            screen_shake_angle_cos: 0.0,
            screen_shake_angle_sin: 0.0,
            script_camera_shakers: Vec::new(),
            script_fps_limit: None,
            script_fps_limit_last_tick: None,
            camera_slave_mode: None,
            view_matrix,
            projection_matrix,
            keys_pressed: HashSet::new(),
            mouse_position: (0.0, 0.0),
            mouse_cursor_seen: false,

            mouse_world_position: Vec3::ZERO,
            last_context_cursor: None,
            last_eva_low_power_count: 0,
            last_eva_insufficient_funds_count: 0,
            last_eva_base_under_attack_count: 0,
            last_eva_ally_under_attack_count: 0,
            last_applied_eva_alert_frame: None,
            sticky_waypoint_mode: false,
            sticky_auto_attack: false,
            use_alternate_mouse,
            is_dragging: false,
            selection_start: None,
            selection_start_screen: None,
            last_click_time: None,
            last_click_position: None,
            last_right_click_time: None,
            last_right_click_position: None,
            left_click_release_behavior: LeftMouseReleaseBehavior::Selection,
            displayed_max_selection_warning: false,
            lmb_context_started_physically: false,
            is_windowed: window.fullscreen().is_none(),
            rmb_scroll_anchor: None,
            is_rmb_scrolling: false,
            move_rmb_scroll_anchor,
            draw_rmb_scroll_anchor,
            rmb_scroll_started_physically: false,
            rmb_deselect_down_at: None,
            rmb_deselect_down_screen: None,
            rmb_deselect_down_camera: None,

            is_mmb_rotating: false,
            mmb_anchor: None,
            selected_objects: Vec::new(),
            control_groups: HashMap::new(),
            last_control_group_select: None,
            camera_view_bookmarks: [None; 8],
            camera_rotate_left_held: false,
            camera_rotate_right_held: false,
            camera_zoom_in_held: false,
            camera_zoom_out_held: false,
            camera_tracking_selection: false,
            replay_fast_forward: false,
            diplomacy_panel: crate::ui::DiplomacyPanel::new(),
            chat_panel: crate::ui::ChatPanel::new(),
            current_player_id: 0,
            game_paused: false,
            quit_menu_host_active: false,
            popup_host_pause_owned: false,
            show_debug_info: debug_overlay,
            show_health_bars: true,
            show_fps: false,
            show_move_lines: true,
            show_attack_lines: true,
            frame_counter: 0,
            fps: 0.0,
            last_frame_timing: None,
            frame_clock: FrameClock::new(),
            menu_loading_tick_accumulator: Duration::ZERO,
            menu_loading_last_tick: Instant::now(),
            diagnostics_overlay: None,
            ui_manager,
            game_hud: GameHUD::new(),
            pending_structure_placement: None,
            pending_map_command: None,
            prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click: false,

            active_menu_shell_hook: None,
            runtime_host_headless,
            runtime_host_active,
            runtime_host_base_ui_screen: None,
            runtime_host_ui_screen_override: None,
            runtime_host_saw_skirmish_menu: false,
            runtime_host_last_gameplay_cmd: String::new(),
            popup_save_load_bridge_initialized: false,
            pending_popup_save_slot: None,
            pending_popup_save_display_name: None,
            interactive_playability: InteractivePlayabilityEvidence::default(),
            pending_match_start: None,
            physical_gather_carrier_ids: HashSet::new(),
            match_damage_applied: 0.0,
            match_kills: 0,
            runtime_host_pending_capture: false,
            models_loaded: true, // Already loaded during init
            pending_shell_model_prewarm,
            menu_enter_frame: None,
            shell_ui_enqueued_frame: None,
            last_shell_prewarm_log: None,
            shell_prewarm_completion_logged: false,
            menu_world_frames_rendered: 0,
            last_slow_menu_tick_log: None,
            ingame_entered_at: None,
            match_over: false,
            victory_summary: None,
        };

        if audio_startup_requires_quit {
            warn!(
                "Audio startup parity: music was not ready during init, marking engine as exiting"
            );
            engine.current_state = GameState::Exiting;
            engine.startup_target_state = None;
            engine.startup_load_state = StartupLoadState::Complete;
        }

        Self::initialize_cpp_startup_masks();

        // C++ parity: GameClient::init() creates WindowManager, Shell, FontLibrary, etc.
        // BUT it also tries to create a PlatformContext (new window + OpenGL context)
        // which deadlocks on macOS when called inside the winit event loop.
        // Only init the non-display subsystems that don't conflict with our wgpu pipeline.
        #[cfg(feature = "game_client")]
        {
            if let Err(e) = engine.game_client.init_core_subsystems() {
                warn!("GameClient core subsystems init failed: {}", e);
            }
            if let Err(e) = engine.game_client.init_asset_systems() {
                warn!("GameClient asset systems init failed: {}", e);
            }
            if let Err(e) = engine.game_client.init_message_translators() {
                warn!("GameClient message translators init failed: {}", e);
            }
            if let Err(e) = engine.game_client.init_input_subsystems() {
                warn!("GameClient input subsystems init failed: {}", e);
            }
            if let Err(e) = engine.game_client.init_display_subsystems() {
                warn!("GameClient display subsystems init failed: {}", e);
            }

            // Create UIRenderer from GraphicsSystem instead of PlatformContext.
            // C++ SAGE uses PlatformContext (OpenGL window) for UI rendering, but we skip
            // PlatformContext creation to avoid macOS winit deadlock (second EventLoop).
            // The GraphicsSystem holds the same wgpu device/queue we need.
            {
                use game_client::gui::ui_globals::set_ui_renderer;
                use game_client::gui::ui_renderer::UIRenderer;

                let device = engine.graphics_system.device_arc();
                let queue = engine.graphics_system.queue_arc();
                let format = engine.graphics_system.color_format();

                match UIRenderer::new(device, queue, format) {
                    Ok(renderer) => {
                        set_ui_renderer(Arc::new(std::sync::RwLock::new(renderer)));
                        info!(
                            "UIRenderer created from GraphicsSystem (format: {:?})",
                            format
                        );
                    }
                    Err(err) => {
                        warn!("UIRenderer creation from GraphicsSystem failed: {err}");
                    }
                }
            }

            if let Err(e) = engine.game_client.init_game_subsystems() {
                warn!("GameClient game subsystems init failed: {}", e);
            }
            if let Err(e) = engine.game_client.post_process_display_strings() {
                warn!("GameClient post-process display strings failed: {}", e);
            }

            if let Err(e) = engine.game_client.init_audio_subsystems() {
                warn!("GameClient audio subsystems init failed: {}", e);
            }

            engine.game_client.init_savegame_counter_bridge();

            engine.game_client.init_recorder_bridge();

            engine.game_client.mark_initialized();
            info!("GameClient: all subsystems initialized");
        }

        if let Some(subsystem_manager) = get_subsystem_manager() {
            let mut manager = subsystem_manager.lock();
            if let Err(err) = manager.reset_all() {
                warn!("Subsystem reset after startup init failed: {}", err);
            }
        }

        engine.hide_control_bar();

        if build_map_cache {
            engine.current_state = GameState::Exiting;
            engine.startup_load_state = StartupLoadState::Complete;
            engine.startup_target_state = None;
            return Ok(engine);
        }

        if audio_startup_requires_quit {
            return Ok(engine);
        }

        // Start background music
        // DISABLED: Using proper AssetManager audio system instead of synthetic tones
        // engine.start_background_music();

        // Display subsystem status
        if let Some(subsystem_manager) = get_subsystem_manager() {
            let stats = subsystem_manager.lock().get_stats();
            info!("📊 Subsystem Status:");
            info!("  ✅ {} subsystems initialized", stats.total_subsystems);
            if let Some(init_time) = stats.initialization_time {
                info!("  ⏱️ Total init time: {:.2}ms", init_time.as_millis());
            }
        }

        info!("🎉 C&C Game Engine with Enhanced Subsystem Architecture initialized successfully!");
        let total_duration = total_timer.finish();
        info!(
            "⏱️ Total Engine Initialization Time: {:.2}s",
            total_duration.as_secs_f32()
        );
        if asset_duration > Duration::ZERO {
            info!("   Asset Manager: {:.2}s", asset_duration.as_secs_f32());
        } else {
            info!("   Asset Manager: initialized during engine setup");
        }
        info!("🎮 Controls:");
        info!("  WASD - Move camera");
        info!("  Mouse - Select units");
        info!("  Right click - Move/Attack command");
        info!("  SPACE - Pause game");
        info!("  F1 - Toggle debug info");
        info!("  M - Toggle music");
        info!("  ESC - Exit game");

        engine
            .window
            .set_title("Command & Conquer Generals Zero Hour - Loading...");
        engine.ensure_shell_loading_overlay();
        engine.update_shell_loading_progress(0.0, Some("Loading assets..."));

        Ok(engine)
    }

    pub(super) fn apply_command_line_overrides(command_line: &CommandLineArgs) {
        let allow_debug_flags = Self::allow_debug_startup_flags();
        let shell_map_override = if allow_debug_flags {
            Self::command_line_option_value_case_insensitive(command_line, "shellmap")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        } else {
            None
        };

        let initial_file_override = if allow_debug_flags {
            Self::command_line_option_value_case_insensitive(command_line, "file")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        } else {
            None
        };
        let map_name_override = if allow_debug_flags {
            command_line
                .map_name
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        } else {
            None
        };

        {
            let mut global = game_engine::common::global_data::write();
            if let Some(width) = command_line.width {
                global.writable.x_resolution = i32::try_from(width).unwrap_or(i32::MAX);
            }
            if let Some(height) = command_line.height {
                global.writable.y_resolution = i32::try_from(height).unwrap_or(i32::MAX);
            }
            if let Some(initial_file) = initial_file_override.as_ref() {
                global.writable.initial_file = initial_file.clone();
            }
            if let Some(map_name) = map_name_override.as_ref() {
                // C++ parseMapName updates the writable startup map path.
                global.writable.map_name = map_name.clone();
            }
            Self::apply_ordered_startup_overrides_from_raw_args(
                &command_line.raw_args,
                &mut global.writable,
                allow_debug_flags,
            );
            if let Some(shell_map_name) = shell_map_override {
                global.writable.shell_map_name = shell_map_name;
            }
            if let Some(lang) = command_line.language.as_deref() {
                global.set_override(
                    "language",
                    game_engine::common::global_data::GlobalValue::String(lang.to_string()),
                );
            }
            if command_line.has_option("mod") {
                if let Some(mod_dir) = command_line.mod_dir.as_deref() {
                    global.writable.mod_dir = mod_dir.to_string();
                    global.writable.mod_big.clear();
                    global.set_override(
                        "active_mod",
                        game_engine::common::global_data::GlobalValue::String(mod_dir.to_string()),
                    );
                } else if let Some(mod_big) = command_line.mod_big.as_deref() {
                    global.writable.mod_big = mod_big.to_string();
                    global.writable.mod_dir.clear();
                    global.set_override(
                        "active_mod",
                        game_engine::common::global_data::GlobalValue::String(mod_big.to_string()),
                    );
                } else if let Some(mod_name) = command_line.mod_name.as_deref() {
                    if mod_name.trim().is_empty() {
                        global.writable.mod_dir.clear();
                        global.writable.mod_big.clear();
                        global.clear_override("active_mod");
                    } else {
                        global.writable.mod_dir.clear();
                        global.writable.mod_big.clear();
                        global.set_override(
                            "active_mod",
                            game_engine::common::global_data::GlobalValue::String(
                                mod_name.to_string(),
                            ),
                        );
                    }
                } else {
                    // Invalid `-mod` path should behave like C++ parseMod: consume option
                    // but leave prior mod configuration untouched.
                }
            }
        }

        Self::load_mods_best_effort();

        let language = command_line.language.as_deref().unwrap_or("English");
        localization::set_language(language);
    }

    pub(super) fn initialize_cpp_startup_masks() {
        game_engine::common::system::kind_of::init_kind_of_masks();
        Self::init_disabled_masks();
        gamelogic::damage::init_damage_type_flags();
    }

    pub(super) fn init_disabled_masks() {
        game_engine::common::system::disabled_types::init_disabled_masks();
    }

    pub(super) fn startup_water_weather_ini_paths() -> [&'static str; 4] {
        [
            "Data/INI/Default/Water.ini",
            "Data/INI/Water.ini",
            "Data/INI/Default/Weather.ini",
            "Data/INI/Weather.ini",
        ]
    }

    /// Disk roots that hold extracted `Data/INI/...` trees (INIZH).
    pub(super) fn startup_ini_disk_roots() -> [&'static str; 6] {
        [
            ".",
            "windows_game/extracted_big_files/INIZH",
            "windows_game/extracted_big_files_v2/INIZH",
            "../windows_game/extracted_big_files/INIZH",
            "../windows_game/extracted_big_files_v2/INIZH",
            "../../windows_game/extracted_big_files/INIZH",
        ]
    }

    /// Read a virtual INI path from extracted disk. Fail-open (None) — never hang.
    pub(crate) fn read_startup_ini_from_disk(virtual_path: &str) -> Option<String> {
        let virtual_path = virtual_path.replace('\\', "/");
        for root in Self::startup_ini_disk_roots() {
            let candidate = if root == "." {
                std::path::PathBuf::from(&virtual_path)
            } else {
                std::path::Path::new(root).join(&virtual_path)
            };
            match std::fs::read_to_string(&candidate) {
                Ok(text) => return Some(text),
                Err(_) => continue,
            }
        }
        None
    }

    pub(super) fn preload_startup_water_weather_inis() {
        for path in Self::startup_water_weather_ini_paths() {
            // Disk first (extracted INIZH). Archive `INI::load` can block on
            // FileSystem/asset locks held by the main thread during Loading.
            if let Some(text) = Self::read_startup_ini_from_disk(path) {
                let mut ini = game_engine::common::ini::INI::new();
                match ini.with_inline_source(&text, |ini| ini.parse_current_file()) {
                    Ok(()) => info!("Preloaded startup INI from disk: {}", path),
                    Err(err) => warn!(
                        "Failed to parse startup INI '{}' from disk; continuing: {}",
                        path, err
                    ),
                }
                continue;
            }
            let mut ini = game_engine::common::ini::INI::new();
            match ini.load(path, game_engine::common::ini::INILoadType::Overwrite) {
                Ok(()) => info!("Preloaded startup INI: {}", path),
                Err(err) => warn!(
                    "Failed to preload startup INI '{}' during init; continuing: {}",
                    path, err
                ),
            }
        }
    }

    /// C++ parity: GameEngine.cpp:480 — AIData.ini load paths.
    /// Loaded after Upgrade, before Crate.
    pub(super) fn startup_ai_data_ini_paths() -> [&'static str; 2] {
        ["Data/INI/Default/AIData.ini", "Data/INI/AIData.ini"]
    }

    /// Load AIData.ini (Default + override) into the AI data store.
    /// C++ parity: GameEngine.cpp:480 initSubsystem(TheAI, ..., "Data\\INI\\Default\\AIData.ini", "Data\\INI\\AIData.ini")
    pub(super) fn preload_startup_ai_data_inis() {
        let mut loaded_any = false;
        for path in Self::startup_ai_data_ini_paths() {
            // Disk first (extracted INIZH). Archive `INI::load` can block on
            // FileSystem/asset locks held by the main thread during Loading.
            if let Some(text) = Self::read_startup_ini_from_disk(path) {
                let mut ini = game_engine::common::ini::INI::new();
                match ini.with_inline_source(&text, |ini| ini.parse_current_file()) {
                    Ok(()) => {
                        loaded_any = true;
                        info!("Preloaded AIData INI from disk: {}", path);
                    }
                    Err(err) => warn!(
                        "Failed to parse AIData INI '{}' from disk; continuing: {}",
                        path, err
                    ),
                }
                continue;
            }
            let mut ini = game_engine::common::ini::INI::new();
            match ini.load(path, game_engine::common::ini::INILoadType::Overwrite) {
                Ok(()) => {
                    loaded_any = true;
                    info!("Preloaded AIData INI: {}", path);
                }
                Err(err) => warn!(
                    "Failed to preload AIData INI '{}' during init; continuing: {}",
                    path, err
                ),
            }
        }
        if loaded_any {
            crate::game_logic::host_repulsor_gate::mark_aidata_ini_applied();
        }
        // C++ initSubsystem(TheAI) copies parsed AIData into TheAI.
        if let Ok(mut ai) = gamelogic::ai::THE_AI.write() {
            ai.init();
        }
        crate::game_logic::host_repulsor_gate::apply_resolved_to_leftover_and_gate();
    }

    /// After archive extract of AIData.ini (Upgrade-style path in shell.rs).
    pub(super) fn apply_startup_ai_data_enable_repulsors() {
        crate::game_logic::host_repulsor_gate::mark_aidata_ini_applied();
        if let Ok(mut ai) = gamelogic::ai::THE_AI.write() {
            ai.init();
        }
        let enabled = crate::game_logic::host_repulsor_gate::apply_resolved_to_leftover_and_gate();
        info!(
            "AIData EnableRepulsors={} (C++ TAiData::m_enableRepulsors)",
            enabled
        );
    }

    pub(super) fn startup_audio_should_quit(no_audio: bool, audio_ready: bool) -> bool {
        !no_audio && !audio_ready
    }

    pub(super) fn apply_startup_audio_channel_flags() {
        let global = game_engine::common::global_data::read();
        let audio_on = global.writable.audio_on;
        let music_on = global.writable.music_on;
        let sounds_on = global.writable.sounds_on;
        let speech_on = global.writable.speech_on;
        let sounds_3d_on = global.sounds_3d_on;
        drop(global);

        if let Some(manager) = game_engine::common::audio::game_audio::get_global_audio_manager() {
            if let Ok(mut audio) = manager.lock() {
                // C++ GameEngine.cpp:537-540 TheAudio->setOn per affect.
                audio.set_on(
                    audio_on && music_on,
                    game_engine::common::audio::game_audio::AudioAffect::Music,
                );
                audio.set_on(
                    audio_on && sounds_on,
                    game_engine::common::audio::game_audio::AudioAffect::Sound,
                );
                audio.set_on(
                    audio_on && sounds_3d_on,
                    game_engine::common::audio::game_audio::AudioAffect::Sound3D,
                );
                audio.set_on(
                    audio_on && speech_on,
                    game_engine::common::audio::game_audio::AudioAffect::Speech,
                );
            }
        }

        with_subsystem_mut::<AudioManagerSubsystem, _>(|audio| {
            audio.apply_startup_channel_flags(
                audio_on,
                music_on,
                sounds_on,
                sounds_3d_on,
                speech_on,
            );
        });
    }

    pub(super) fn allow_debug_startup_flags() -> bool {
        // PARITY_NOTE: C++ gates debug flags on internal builds only.
        // Rust allows -noaudio and similar flags in all builds for cross-platform compatibility.
        true
    }

    pub(super) fn remove_legacy_duplicate_inizh_big_best_effort() {
        let legacy_path = std::path::Path::new("Data").join("INI").join("INIZH.big");
        if !legacy_path.exists() {
            return;
        }

        match std::fs::remove_file(&legacy_path) {
            Ok(()) => info!(
                "Removed legacy duplicate INI archive to match C++ startup cleanup: {}",
                legacy_path.display()
            ),
            Err(err) => warn!(
                "Failed to remove legacy duplicate INI archive '{}': {}",
                legacy_path.display(),
                err
            ),
        }
    }

    pub(super) fn hide_control_bar(&mut self) {
        // C++ GameEngine::init / HideControlBar — hide the live WND parent.
        #[cfg(feature = "game_client")]
        {
            let _ = game_client::gui::callbacks::control_bar_callbacks::hide_control_bar(true);
        }
        // Soft GameHUD visibility stays paired for construction-ghost residual.
        if self.game_hud.hud_visible() {
            self.game_hud.toggle_visibility();
        }
    }

    pub(super) fn load_mods_best_effort() {
        let (mod_dir, mod_big) = {
            let global = game_engine::common::global_data::read();
            let mod_dir = global.writable.mod_dir.trim().to_string();
            let mod_big = global.writable.mod_big.trim().to_string();
            (mod_dir, mod_big)
        };
        if mod_dir.is_empty() && mod_big.is_empty() {
            return;
        }
        // C++ TheArchiveFileSystem->loadMods() overwrites the same live tree
        // AssetManager extract uses — not a second Common singleton.
        if let Some(manager_arc) = crate::assets::manager::get_asset_manager() {
            match manager_arc.lock() {
                Ok(mut manager) => {
                    if let Err(err) = manager.load_user_mods(&mod_dir, &mod_big) {
                        warn!("Best-effort mod archive load failed: {}", err);
                    }
                }
                Err(err) => warn!("Asset manager busy while loading mods: {}", err),
            }
        } else {
            warn!("Asset manager missing; -mod archives were not mounted");
        }
    }

    pub(super) fn apply_fps_limit_overrides_from_raw_args(
        raw_args: &[String],
        writable: &mut game_engine::common::command_line::WritableGlobalData,
    ) {
        // C++ parity: process `-nofpslimit`/`-fps` in original argv order.
        // This preserves precedence when both are present.
        let mut arg_index = 1usize;
        while arg_index < raw_args.len() {
            let raw = raw_args[arg_index].trim();
            if !raw.starts_with('-') {
                arg_index += 1;
                continue;
            }

            let mut option = raw.trim_start_matches('-');
            let mut inline_value: Option<&str> = None;
            if let Some((name, value)) = option.split_once('=') {
                option = name;
                inline_value = Some(value);
            }

            match option.to_ascii_lowercase().as_str() {
                "nofpslimit" => {
                    writable.use_fps_limit = false;
                    writable.frames_per_second_limit = 30000;
                }
                "fps" => {
                    if let Some(value) =
                        Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                    {
                        writable.frames_per_second_limit =
                            Self::parse_startup_i32_like_atoi(&value);
                    }
                }
                _ => {}
            }

            arg_index += 1;
        }
    }

    pub(super) fn apply_ordered_startup_overrides_from_raw_args(
        raw_args: &[String],
        writable: &mut game_engine::common::command_line::WritableGlobalData,
        allow_debug_flags: bool,
    ) {
        let mut arg_index = 1usize;
        while arg_index < raw_args.len() {
            let raw = raw_args[arg_index].trim();
            if !raw.starts_with('-') {
                arg_index += 1;
                continue;
            }

            let mut option = raw.trim_start_matches('-');
            let mut inline_value: Option<&str> = None;
            if let Some((name, value)) = option.split_once('=') {
                option = name;
                inline_value = Some(value);
            }

            match option.to_ascii_lowercase().as_str() {
                "win" | "windowed" | "w" => {
                    writable.windowed = true;
                }
                "fullscreen" | "f" | "nowin" => {
                    writable.windowed = false;
                }
                "particleedit" => {
                    writable.particle_edit = true;
                    writable.win_cursors = true;
                    writable.windowed = true;
                }
                "quickstart" => {
                    writable.shell_map_on = false;
                    writable.animate_windows = false;
                    writable.play_sizzle = false;

                    if cfg!(any(debug_assertions, feature = "internal")) {
                        writable.play_intro = false;
                        writable.after_intro = true;
                    }
                }
                "nologo" | "nointro" => {
                    if allow_debug_flags {
                        writable.play_intro = false;
                        writable.after_intro = true;
                        writable.play_sizzle = false;
                    }
                }
                "noshellmap" => {
                    writable.shell_map_on = false;
                }
                "noshellanim" => {
                    if allow_debug_flags {
                        writable.animate_windows = false;
                    }
                }
                "noaudio" => {
                    if allow_debug_flags {
                        writable.audio_on = false;
                        writable.speech_on = false;
                        writable.sounds_on = false;
                        writable.music_on = false;
                    }
                }
                "novideo" => {
                    if allow_debug_flags {
                        writable.video_on = false;
                    }
                }
                "scriptdebug" => {
                    writable.script_debug = true;
                    writable.win_cursors = true;
                }
                "wincursors" => {
                    if allow_debug_flags {
                        writable.win_cursors = true;
                    }
                }
                "nomusic" => {
                    if allow_debug_flags {
                        writable.music_on = false;
                    }
                }
                "nodraw" => {
                    if allow_debug_flags {
                        writable.no_draw = true;
                    }
                }
                "noshaders" => {
                    writable.chip_set_type = 1;
                }
                "forcebenchmark" => {
                    if allow_debug_flags {
                        writable.force_benchmark = true;
                    }
                }
                "nomovecamera" => {
                    if allow_debug_flags {
                        writable.disable_camera_movement = true;
                    }
                }
                "constantdebug" => {
                    if allow_debug_flags {
                        writable.constant_debug_update = true;
                    }
                }
                "showteamdot" => {
                    if allow_debug_flags {
                        writable.show_team_dot = true;
                    }
                }
                "nofpslimit" => {
                    if allow_debug_flags {
                        writable.use_fps_limit = false;
                        writable.frames_per_second_limit = 30000;
                    }
                }
                "buildmapcache" | "buildcache" => {
                    if allow_debug_flags {
                        writable.build_map_cache = true;
                    }
                }
                "updateimages" | "updatedds" => {
                    if allow_debug_flags {
                        writable.should_update_tga_to_dds = true;
                    }
                }
                "fps" => {
                    if allow_debug_flags {
                        if let Some(value) =
                            Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                        {
                            writable.frames_per_second_limit =
                                Self::parse_startup_i32_like_atoi(&value);
                        }
                    }
                }
                "seed" => {
                    if allow_debug_flags {
                        if let Some(value) =
                            Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                        {
                            writable.fixed_seed = Self::parse_startup_i32_like_atoi(&value);
                        }
                    }
                }
                "jumptoframe" => {
                    if allow_debug_flags {
                        if let Some(value) =
                            Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                        {
                            writable.no_draw = Self::parse_startup_i32_like_atoi(&value) != 0;
                            writable.use_fps_limit = false;
                            writable.frames_per_second_limit = 30000;
                        }
                    }
                }
                "netminplayers" => {
                    if allow_debug_flags {
                        if let Some(value) =
                            Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                        {
                            writable.net_min_players = Self::parse_startup_i32_like_atoi(&value);
                        }
                    }
                }
                "playstats" => {
                    if let Some(value) =
                        Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                    {
                        writable.play_stats = Self::parse_startup_i32_like_atoi(&value);
                    }
                }
                "benchmark" if allow_debug_flags => {
                    if let Some(value) =
                        Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                    {
                        let parsed = Self::parse_startup_i32_like_atoi(&value);
                        writable.benchmark_timer = parsed;
                        writable.play_stats = parsed;
                    }
                }
                _ => {}
            }

            arg_index += 1;
        }
    }

    pub(super) fn consume_startup_value(
        raw_args: &[String],
        arg_index: &mut usize,
        inline_value: Option<&str>,
    ) -> Option<String> {
        if let Some(value) = inline_value {
            return Some(value.to_string());
        }

        if *arg_index + 1 < raw_args.len() {
            *arg_index += 1;
            return Some(raw_args[*arg_index].trim().to_string());
        }

        None
    }

    pub(super) fn parse_startup_i32_like_atoi(value: &str) -> i32 {
        value.trim().parse::<i32>().unwrap_or(0)
    }

    pub(super) fn has_command_line_option_case_insensitive(
        command_line: &CommandLineArgs,
        option: &str,
    ) -> bool {
        command_line
            .options
            .keys()
            .any(|name| name.eq_ignore_ascii_case(option))
    }

    pub(super) fn command_line_option_value_case_insensitive(
        command_line: &CommandLineArgs,
        option: &str,
    ) -> Option<String> {
        command_line.options.iter().find_map(|(name, value)| {
            if name.eq_ignore_ascii_case(option) {
                value.clone()
            } else {
                None
            }
        })
    }

    pub(super) fn startup_initial_file_from_command_line(
        command_line: &CommandLineArgs,
        allow_debug_flags: bool,
    ) -> Option<String> {
        if !allow_debug_flags {
            return None;
        }

        let runtime_initial_file = {
            let global = game_engine::common::global_data::read();
            global.writable.initial_file.trim().to_string()
        };
        if !runtime_initial_file.is_empty() {
            return Some(runtime_initial_file);
        }

        Self::command_line_option_value_case_insensitive(command_line, "file")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    pub(super) fn split_startup_initial_file(
        initial_file: Option<String>,
    ) -> (Option<String>, Option<String>) {
        let Some(initial_file) = initial_file else {
            return (None, None);
        };

        let lower = initial_file.to_ascii_lowercase();
        if lower.ends_with(".map") {
            (Some(initial_file), None)
        } else if lower.ends_with(".rep") {
            (None, Some(initial_file))
        } else {
            (None, None)
        }
    }

    pub(super) fn sync_after_intro_when_intro_disabled() {
        let mut global = game_engine::common::global_data::write();
        if !global.writable.play_intro {
            global.writable.after_intro = true;
        }
    }

    /// Pre-load all unit models into the graphics system
    pub(super) async fn preload_unit_models_to_graphics_system(
        graphics_system: &mut GraphicsSystem,
    ) -> Result<()> {
        info!("🎮 Pre-loading C&C unit models into graphics system...");

        // Initialize a temporary game logic instance to get the templates
        let mut temp_game_logic = GameLogic::initialize();
        // Need to setup templates since initialize() doesn't do it
        temp_game_logic.start_new_game(crate::game_logic::GameMode::Skirmish);
        let templates = temp_game_logic.get_templates();

        // List of all unit types that need models loaded
        let unit_types = vec![
            // USA units
            "USA_Ranger",
            "USA_MissileDefender",
            "USA_Humvee",
            "USA_CrusaderTank",
            "USA_PaladinTank",
            "USA_Raptor",
            // GLA units
            "GLA_Soldier",
            "GLA_RPGTrooper",
            "GLA_Technical",
            "GLA_ScorpionTank",
            "GLA_MarauderTank",
            // China units
            "China_RedGuard",
            "China_TankHunter",
            "China_BattlemasterTank",
            "China_OverlordTank",
            "China_MiG",
            "China_Helix",
            // Buildings
            "CommandCenter",
            "SupplyCenter",
            "PowerPlant",
            "Barracks",
            "WarFactory",
        ];

        if let Some(asset_manager_arc) = get_asset_manager() {
            let mut asset_manager = asset_manager_arc.lock().unwrap_or_else(|e| e.into_inner());
            let mut loaded_count = 0;
            let total_units = unit_types.len();

            for unit_type in &unit_types {
                println!("📋 Loading W3D model for template: {}", unit_type);

                // Look up the template to get the correct model name
                if let Some(template) = templates.get(*unit_type) {
                    if let Some(model_name) = &template.model_name {
                        println!(
                            "🎯 Template '{}' maps to W3D model: '{}'",
                            unit_type, model_name
                        );

                        // Try to load the W3D model using the correct filename
                        match asset_manager.load_w3d_model_async(model_name).await {
                            Ok(model) => {
                                println!(
                                    "✅ Successfully loaded W3D model: '{}' for template '{}' ({} meshes, {} total vertices)",
                                    model_name,
                                    unit_type,
                                    model.meshes.len(),
                                    model.meshes.iter().map(|m| m.vertices.len()).sum::<usize>()
                                );
                                // Cache the model in graphics system using both keys
                                graphics_system.cache_model(unit_type.to_string(), model.clone());
                                graphics_system.cache_model(model_name.clone(), model);
                                loaded_count += 1;
                            }
                            Err(e) => {
                                println!(
                                    "❌ CRITICAL: Failed to load W3D model '{}' for template '{}': {}",
                                    model_name, unit_type, e
                                );
                                println!(
                                    "❌ This means '{}' units will not be visible in game!",
                                    unit_type
                                );
                                // Continue loading other models even if one fails
                            }
                        }
                    } else {
                        println!(
                            "⚠️ CRITICAL: Template '{}' has no model_name defined - units will be invisible!",
                            unit_type
                        );
                    }
                } else {
                    println!(
                        "❌ CRITICAL: Template '{}' not found in templates!",
                        unit_type
                    );
                }
            }

            info!(
                "📦 Successfully pre-loaded {}/{} unit models into graphics system",
                loaded_count, total_units
            );
        } else {
            error!("❌ Asset manager not available for model preloading");
        }

        Ok(())
    }

    /// Pre-load all unit models that will be used in the game
    pub(super) async fn preload_unit_models(
        loaded_models: &mut HashMap<String, Arc<W3DModel>>,
    ) -> Result<()> {
        info!("🎮 Pre-loading C&C unit models...");

        // Initialize a temporary game logic instance to get the templates
        let mut temp_game_logic = GameLogic::initialize();
        // Need to setup templates since initialize() doesn't do it
        temp_game_logic.start_new_game(crate::game_logic::GameMode::Skirmish);
        let templates = temp_game_logic.get_templates();

        // List of all unit types that need models loaded
        let unit_types = vec![
            // USA units
            "USA_Ranger",
            "USA_MissileDefender",
            "USA_Humvee",
            "USA_CrusaderTank",
            "USA_PaladinTank",
            "USA_Raptor",
            // GLA units
            "GLA_Soldier",
            "GLA_RPGTrooper",
            "GLA_Technical",
            "GLA_ScorpionTank",
            "GLA_MarauderTank",
            // China units
            "China_RedGuard",
            "China_TankHunter",
            "China_BattlemasterTank",
            "China_OverlordTank",
            "China_MiG",
            "China_Helix",
            // Buildings
            "CommandCenter",
            "SupplyCenter",
            "PowerPlant",
            "Barracks",
            "WarFactory",
        ];

        if let Some(asset_manager_arc) = get_asset_manager() {
            let mut asset_manager = asset_manager_arc.lock().unwrap_or_else(|e| e.into_inner());
            let mut loaded_count = 0;
            let total_units = unit_types.len();

            for unit_type in &unit_types {
                println!("📋 Loading W3D model for template: {}", unit_type);

                // Look up the template to get the correct model name
                if let Some(template) = templates.get(*unit_type) {
                    if let Some(model_name) = &template.model_name {
                        println!(
                            "🎯 Template '{}' maps to W3D model: '{}'",
                            unit_type, model_name
                        );

                        // Try to load the W3D model using the correct filename
                        match asset_manager.load_w3d_model_async(model_name).await {
                            Ok(model) => {
                                println!(
                                    "✅ Successfully loaded W3D model: '{}' for template '{}' ({} meshes, {} total vertices)",
                                    model_name,
                                    unit_type,
                                    model.meshes.len(),
                                    model.meshes.iter().map(|m| m.vertices.len()).sum::<usize>()
                                );
                                // Store the model using both the template name AND the model name as keys
                                // This ensures compatibility with both template-based and model-based lookups
                                loaded_models
                                    .insert(unit_type.to_string(), Arc::new(model.clone()));
                                loaded_models.insert(model_name.clone(), Arc::new(model));
                                loaded_count += 1;
                            }
                            Err(e) => {
                                println!(
                                    "❌ CRITICAL: Failed to load W3D model '{}' for template '{}': {}",
                                    model_name, unit_type, e
                                );
                                println!(
                                    "❌ This means '{}' units will not be visible in game!",
                                    unit_type
                                );
                                // Continue loading other models even if one fails
                            }
                        }
                    } else {
                        println!(
                            "⚠️ CRITICAL: Template '{}' has no model_name defined - units will be invisible!",
                            unit_type
                        );
                    }
                } else {
                    println!(
                        "❌ CRITICAL: Template '{}' not found in templates!",
                        unit_type
                    );
                }
            }

            info!(
                "📦 Successfully pre-loaded {}/{} unit models",
                loaded_count, total_units
            );
        } else {
            error!("❌ Asset manager not available for model preloading");
        }

        Ok(())
    }

    /// Create GPU buffers for all loaded W3D models
    pub(super) fn create_model_buffers(
        loaded_models: &HashMap<String, Arc<W3DModel>>,
        device: &wgpu::Device,
        model_buffers: &mut HashMap<String, (wgpu::Buffer, wgpu::Buffer, u32)>,
    ) -> Result<()> {
        info!(
            "🔧 Creating GPU buffers for {} loaded models...",
            loaded_models.len()
        );

        // Keep track of processed models to avoid duplicates
        let mut processed_models: std::collections::HashSet<*const W3DModel> =
            std::collections::HashSet::new();

        for (model_key, w3d_model) in loaded_models {
            // Skip if we've already processed this exact model instance
            let model_ptr = w3d_model.as_ref() as *const W3DModel;
            if processed_models.contains(&model_ptr) {
                continue;
            }
            processed_models.insert(model_ptr);

            for (mesh_idx, mesh) in w3d_model.meshes.iter().enumerate() {
                let mesh_key = format!("{}_{}", model_key, mesh_idx);

                // Skip if buffer already exists
                if model_buffers.contains_key(&mesh_key) {
                    continue;
                }

                // Convert W3D vertices to C++ SAGE VertexFormatXYZNDUV2 format
                let material_color = mesh.material.diffuse_color;
                let vertices: Vec<VertexXYZNDUV2> = mesh
                    .vertices
                    .iter()
                    .map(|v| {
                        // Pack diffuse color as RGBA bytes (D3D8 style)
                        let r = ((v.color[0] * material_color.x * 255.0) as u32).min(255);
                        let g = ((v.color[1] * material_color.y * 255.0) as u32).min(255);
                        let b = ((v.color[2] * material_color.z * 255.0) as u32).min(255);
                        let a = ((v.color[3] * 255.0) as u32).min(255);
                        let diffuse_packed = (a << 24) | (r << 16) | (g << 8) | b;

                        VertexXYZNDUV2 {
                            position: v.position,
                            normal: v.normal,
                            diffuse: diffuse_packed,
                            tex_coords0: v.uv,       // Primary texture coordinates
                            tex_coords1: [0.0, 0.0], // Secondary UV for multi-texturing
                        }
                    })
                    .collect();

                // Create vertex buffer
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{} Vertex Buffer", mesh_key)),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                // Convert indices to u16 format
                let indices: Vec<u16> = mesh.indices.iter().map(|&i| i as u16).collect();
                let index_count = indices.len() as u32;

                // Create index buffer
                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{} Index Buffer", mesh_key)),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

                let buffer_data = (vertex_buffer, index_buffer, index_count);
                model_buffers.insert(mesh_key.clone(), buffer_data);

                info!(
                    "✅ Created GPU buffers for mesh: {} ({} vertices, {} indices)",
                    mesh_key,
                    vertices.len(),
                    index_count
                );
            }
        }

        info!(
            "📦 Created GPU buffers for {} model meshes total",
            model_buffers.len()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn live_boot_inits_game_lod_manager_after_command_line() {
        let src = include_str!("boot.rs");
        let live = src.split("#[cfg(test)]").next().expect("boot live path");
        let cmdline = live
            .find("Self::apply_command_line_overrides(&command_line);")
            .expect("parseCommandLine analog");
        let lod = live
            .find("load_game_lod_ini_presets_and_options()")
            .expect("GameLODManager::init analog");
        let water = live
            .find("fn preload_startup_water_weather_inis")
            .expect("Water/Weather INI preload");
        assert!(
            cmdline < lod && lod < water,
            "C++ GameEngine.cpp:357-373: parseCommandLine, GameLODManager::init, then Water/Weather"
        );
    }

    #[test]
    fn preload_aidata_applies_enable_repulsors_to_host_gate() {
        let src = include_str!("boot.rs");
        let live = src.split("#[cfg(test)]").next().expect("boot live path");
        let preload = live
            .find("fn preload_startup_ai_data_inis")
            .expect("AIData preload");
        let disk = live
            .find("Preloaded AIData INI from disk")
            .expect("disk-first AIData");
        let the_ai = live.find("ai.init()").expect("TheAI::init after AIData");
        let apply = live
            .find("apply_resolved_to_leftover_and_gate")
            .expect("host EnableRepulsors apply");
        assert!(
            preload < disk && disk < the_ai && the_ai < apply,
            "C++ GameEngine.cpp:480: load AIData.ini, TheAI::init, then EnableRepulsors"
        );
    }
}
