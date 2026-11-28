// Bind script-frame callbacks to the receiving interpreter and game.

impl ScriptEngine {
    /// Update script engine
    pub fn update(&self) -> GameLogicResult<()> {
        let mut driver = CanonicalScriptExecutionDriver;
        self.update_with_driver(
            ScriptContext::at_frame(self.owning_logic_frame()),
            &mut driver,
        )
    }

    /// Execute the standalone frame with the legacy facades available to its
    /// callbacks. The native host lends its own program driver directly.
    pub fn update_with_driver(
        &self,
        context: ScriptContext,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> GameLogicResult<()> {
        self.with_active(|| {
            self.with_script_program(context, driver, |runtime, frame, program| {
                runtime.advance_frame(frame, program)
            })
        })
    }

    /// Original first-UPDATE cache phase, shared with explicit receiving startup
    /// fixtures. No timers, team state or script actions advance in this phase.
    pub fn initialize_named_cache_on_first_update(&self, driver: &mut dyn ScriptExecutionDriver) {
        let first_update = self.runtime.frame_mut().take_first_update();
        if first_update {
            self.initialize_named_cache(driver);
        }
    }

    fn initialize_named_cache(&self, driver: &mut dyn ScriptExecutionDriver) {
        if driver.initialize_named_cache().is_none() {
            self.create_named_cache();
        }
        log::info!("ScriptEngine first update: named cache populated");
    }

    /// Populate the NamedObjectTracker from currently registered objects.
    ///
    /// C++ Reference: `ScriptEngine::createNamedCache()`.
    fn create_named_cache(&self) {
        for obj_id in self.object_registry.get_all_object_ids() {
            let obj_arc = match self.object_registry.get_object(obj_id) {
                Some(v) => v,
                None => continue,
            };
            let Ok(obj) = obj_arc.read() else { continue };
            let name = obj.get_name();
            if name.is_empty() {
                continue;
            }
            let _ = self
                .named_objects
                .register_named_object(name.to_string(), obj.get_id());
        }
    }

    /// Notify the script engine that objects were created or destroyed.
    /// Mirrors C++ ScriptEngine::notifyOfObjectCreationOrDestruction().
    pub fn notify_of_object_creation_or_destruction(&self) {
        // ScriptEngine.cpp:7785-7788 records the frame only. Scanning live
        // objects here both changes cache timing and re-enters initializing
        // objects; createNamedCache belongs to the first script update.
        self.set_frame_object_count_changed(self.owning_logic_frame());
    }
}

impl ScriptFrameDriver for ScriptProgramWorld<'_, '_> {
    fn create_named_cache(&mut self) {
        self.engine.initialize_named_cache(self.execution.driver);
    }

    fn destroy_win_lose_window(&mut self) -> GameLogicResult<()> {
        self.engine
            .destroy_recorded_window_with_driver(self.execution.driver)?;
        self.execution.driver.after_action()
    }

    fn queue_clear_game_data(&mut self) {
        use game_engine::common::message_stream::GameMessageType;
        log::info!("End game timer expired, appending MSG_CLEAR_GAME_DATA");
        if self.execution.driver.enqueue_clear_game_data().is_none()
            && let Ok(mut stream) = self.engine.common.application.message_stream.write()
        {
            stream.append_message(GameMessageType::ClearGameData);
        }
    }

    fn side_player(&mut self, side: usize) -> Option<String> {
        match self.execution.driver.side_script_current_player(side) {
            ScriptOwnerQuery::Present(player) => player,
            ScriptOwnerQuery::Missing => None,
            ScriptOwnerQuery::Unavailable => self
                .standalone_player_names
                .get_or_insert_with(|| self.engine.standalone_side_player_names())
                .get(side)
                .cloned()
                .flatten(),
        }
    }

    fn update_teams(&mut self, interpreter: &ScriptInterpreter) -> GameLogicResult<()> {
        if let Some(result) = self.execution.driver.update_team_states(interpreter) {
            return result;
        }
        let query = self.engine.host_query();
        if let Some(world) = query.world() {
            // The standalone factory still has generic-script callbacks that
            // read its local compatibility scope when the guard is released.
            self.engine.with_active(|| {
                crate::system::engine_stores::with_world_services(world, || {
                    if let Ok(mut factory) = self.engine.team_factory.lock() {
                        factory.update(&query);
                    }
                });
                crate::team::dispatch_pending_team_script_events(
                    world,
                    self.engine,
                    self.execution.frame,
                );
            });
        }
        Ok(())
    }

    #[cfg(feature = "script_profiling")]
    fn frame_completed(&mut self) {
        let elapsed = self.started_at.elapsed().as_secs_f64();
        let mut inner = self.engine.lock_inner_mut();
        inner.stats.cur_update_time = elapsed;
        inner.stats.total_update_time += elapsed;
        inner.stats.num_frames += 1.0;
        inner.stats.max_update_time = inner.stats.max_update_time.max(elapsed);
    }
}
