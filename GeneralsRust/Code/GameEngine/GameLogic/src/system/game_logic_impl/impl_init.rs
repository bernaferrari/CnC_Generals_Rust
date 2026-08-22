impl GameLogic {
    /// Create a new GameLogic instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Get whether draw icon UI indicators are enabled.
    pub fn get_draw_icon_ui(&self) -> Bool {
        self.draw_icon_ui
    }

    /// Set whether draw icon UI indicators are enabled.
    pub fn set_draw_icon_ui(&mut self, enabled: Bool) {
        self.draw_icon_ui = enabled;
    }

    /// Get whether behind-building markers (occlusion markers) are enabled.
    pub fn get_show_behind_building_markers(&self) -> Bool {
        self.show_behind_building_markers
    }

    /// Set whether behind-building markers (occlusion markers) are enabled.
    pub fn set_show_behind_building_markers(&mut self, enabled: Bool) {
        self.show_behind_building_markers = enabled;
    }

    /// Get whether dynamic LOD is enabled.
    pub fn get_show_dynamic_lod(&self) -> Bool {
        self.show_dynamic_lod
    }

    /// Set whether dynamic LOD is enabled.
    pub fn set_show_dynamic_lod(&mut self, enabled: Bool) {
        self.show_dynamic_lod = enabled;
    }

    /// Get whether scoring is enabled.
    pub fn is_scoring_enabled(&self) -> Bool {
        self.is_scoring_enabled
    }

    /// Enable/disable scoring updates and score screen accumulation.
    pub fn set_scoring_enabled(&mut self, enabled: Bool) {
        self.is_scoring_enabled = enabled;
    }

    /// Get the global map/script rank level cap.
    /// C++ reference: GameLogic::getRankLevelLimit()
    pub fn get_rank_level_limit(&self) -> Int {
        self.rank_level_limit
    }

    /// Set a runtime buildability override for a template name.
    /// Mirrors C++ GameLogic::setBuildableStatusOverride.
    pub fn set_buildable_status_override(&mut self, template_name: &str, status: Int) {
        if template_name.is_empty() {
            return;
        }
        self.buildable_status_overrides
            .insert(template_name.to_string(), status);
    }

    /// Find a runtime buildability override for a template name.
    /// Mirrors C++ GameLogic::findBuildableStatusOverride.
    pub fn find_buildable_status_override(&self, template_name: &str) -> Option<Int> {
        self.buildable_status_overrides.get(template_name).copied()
    }

    /// Set the global map/script rank level cap.
    /// C++ reference: GameLogic::setRankLevelLimit()
    pub fn set_rank_level_limit(&mut self, mut level: Int) {
        if level < 1 {
            level = 1;
        }
        self.rank_level_limit = level;
    }

    /// Initialize the GameLogic system
    ///
    /// ## C++ Reference: GameLogic::init() (GameLogic.cpp)
    pub fn init(&mut self) {
        info!("GameLogic::init() - Initializing game logic system");
        self.reset();
        if let Err(err) = game_engine::common::thing::init_thing_system() {
            warn!("Thing system initialization failed during init: {}", err);
        }
        crate::system::thing_factory_bridge::install_thing_factory_bridge();
        if let Err(err) = crate::contain_module_overrides::ensure_module_overrides_installed() {
            warn!("Failed to install module overrides during init: {}", err);
        }
        self.refresh_global_weapon_bonuses();
        install_energy_integration();
        crate::system::object_data_provider::install_object_data_provider();

        init_build_assistant();
        crate::system::build_assistant_bridge::install_build_assistant_backend();
        crate::terrain::init_terrain_physics_integration();

        crate::special_power_module::initialize();
        crate::control_bar::register_academy_template_context_provider();
        if let Err(e) = crate::control_bar::initialize_control_bar_bridge_from_common() {
            warn!("Control bar bridge initialization failed: {}", e);
        }

        if let Err(e) =
            crate::commands::initialize_command_system(crate::common::MAX_PLAYER_COUNT as i32)
        {
            warn!("Command system initialization failed: {}", e);
        }

        if let Err(e) = initialize_script_engine() {
            warn!("Script engine initialization failed: {}", e);
        }
        crate::system::object_data_provider::ensure_object_data_provider();
    }

    /// Reset the GameLogic to default state
    ///
    /// ## C++ Reference: GameLogic::reset() (GameLogic.cpp)
    pub fn reset(&mut self) {
        info!("GameLogic::reset() - Resetting game state");

        self.frame = 0;
        self.game_time = 0.0;
        self.is_in_update = false;
        self.last_update_was_empty_noop = false;
        self.empty_world_tick = 0;
        self.next_object_id = 1;
        self.all_objects.clear();
        self.dead_objects.clear();
        self.objects.clear();
        self.event_queue.clear();
        self.command_queue.clear();
        self.radar_updates.clear();
        self.game_mode = GAME_NONE;
        self.game_paused = false;
        self.loading_map = false;
        self.loading_save = false;
        crate::helpers::TheGameLogic::clear_start_new_game_request();
        self.is_scoring_enabled = true;
        self.show_behind_building_markers = true;
        self.draw_icon_ui = true;
        self.show_dynamic_lod = true;
        self.rank_level_limit = 1000;
        self.buildable_status_overrides.clear();
        self.partition_manager = PartitionManager::new();
        self.physics_world = PhysicsWorld::new();
        self.sleepy_updates.clear();
        self.normal_updates.clear();
        self.module_lookup.clear();
        if let Err(err) = game_engine::common::thing::init_thing_system() {
            warn!("Thing system initialization failed during reset: {}", err);
        }
        crate::system::thing_factory_bridge::install_thing_factory_bridge();
        if let Err(err) = crate::contain_module_overrides::ensure_module_overrides_installed() {
            warn!("Failed to install module overrides during reset: {}", err);
        }
        self.refresh_global_weapon_bonuses();
        install_energy_integration();
        crate::system::object_data_provider::install_object_data_provider();

        init_build_assistant();
        crate::system::build_assistant_bridge::install_build_assistant_backend();
        crate::terrain::init_terrain_physics_integration();

        // Keep global subsystems in a C++-like "reset, don't recreate" state.
        if let Err(e) = initialize_script_engine() {
            warn!("Script engine initialization failed during reset: {}", e);
        }

        crate::special_power_module::initialize();
        if let Err(e) = crate::control_bar::refresh_control_bar_bridge_from_common() {
            warn!("Control bar bridge refresh failed during reset: {}", e);
        }

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.reset();
            }
        }

        // C++ line 413: m_controlBarOverrides.clear()
        self.control_bar_overrides.clear();

        // C++ lines 447-451: delete TheStatsCollector; TheStatsCollector = NULL;
        game_engine::common::stats_collector::with_stats_collector_mut(|collector| {
            collector.reset();
        });

        // C++ line 462: m_scriptHulkMaxLifetimeOverride = -1
        crate::helpers::TheGameLogic::set_hulk_max_lifetime_override(-1);

        // C++ line 472: m_rankPointsToAddAtGameStart = 0
        crate::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(0);

        // C++ lines 465-466: clean up water transparency overrides
        game_engine::common::ini::ini_water::clear_water_transparency_overrides();

        // C++ lines 469-470: clean up weather overrides
        game_engine::common::ini::ini_weather::clear_weather_setting_overrides();
    }
}
