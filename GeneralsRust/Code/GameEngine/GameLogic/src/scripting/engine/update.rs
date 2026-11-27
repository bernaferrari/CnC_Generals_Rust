// Bind script-frame callbacks to the receiving interpreter and game.

impl ScriptEngine {
    pub(super) fn observe_object_count_changed_frame(&self, driver: &dyn ScriptExecutionDriver<ScriptEngine>) {
        match driver.object_count_changed_frame() {
            ScriptOwnerQuery::Present(frame) => self.set_frame_object_count_changed(frame),
            ScriptOwnerQuery::Missing => self.set_frame_object_count_changed(0),
            ScriptOwnerQuery::Unavailable => {}
        }
    }

    /// Update script engine
    pub fn update(&self) -> GameLogicResult<()> {
        let mut driver = CanonicalScriptExecutionDriver;
        self.update_with_driver(
            crate::scripting::executor::ScriptContext::new(),
            &mut driver,
        )
    }

    /// Execute against the supplied world frame and trigger state. C++
    /// executeActions applies each action before examining the next one.
    pub fn update_with_driver(
        &self,
        context: crate::scripting::executor::ScriptContext,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> GameLogicResult<()> {
        self.observe_object_count_changed_frame(driver);
        let context = std::cell::RefCell::new(context);
        let mut execution = ScriptExecution::new(&context, driver);
        self.with_active(|| self.update_active(&mut execution))
    }

    fn update_active(&self, execution: &mut ScriptExecution<'_>) -> GameLogicResult<()> {
        #[cfg(feature = "script_profiling")]
        let start_time = Instant::now();

        let progress = frame::advance(&mut ScriptFrameWorld {
            engine: self,
            execution,
        })?;
        if progress == frame::ScriptFrameProgress::Suspended {
            return Ok(());
        }

        #[cfg(feature = "script_profiling")]
        {
            let elapsed = start_time.elapsed();
            let mut inner = self.lock_inner_mut();
            inner.stats.cur_update_time = elapsed.as_secs_f64();
            inner.stats.total_update_time += inner.stats.cur_update_time;
            inner.stats.num_frames += 1.0;
            if inner.stats.cur_update_time > inner.stats.max_update_time {
                inner.stats.max_update_time = inner.stats.cur_update_time;
            }
        }

        Ok(())
    }

    /// Original first-UPDATE cache phase, shared with explicit receiving startup
    /// fixtures. No timers, team state or script actions advance in this phase.
    pub fn initialize_named_cache_on_first_update(&self, driver: &mut dyn ScriptExecutionDriver<ScriptEngine>) {
        let first_update = self.runtime.frame_mut().take_first_update();
        if first_update {
            self.initialize_named_cache(driver);
        }
    }

    fn initialize_named_cache(&self, driver: &mut dyn ScriptExecutionDriver<ScriptEngine>) {
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
        self.set_frame_object_count_changed(TheGameLogic::get_frame() as u32);
    }

    fn execute_side_scripts(&self, execution: &mut ScriptExecution<'_>) -> GameLogicResult<()> {
        self.runtime.execute_sides(&mut ScriptProgramWorld::new(self, execution))
    }

}

struct ScriptFrameWorld<'engine, 'context> {
    engine: &'engine ScriptEngine,
    execution: &'engine mut ScriptExecution<'context>,
}

impl ScriptFrameDriver for ScriptFrameWorld<'_, '_> {
    fn with_frame<R>(&self, edit: impl FnOnce(&mut ScriptFrame) -> R) -> R {
        edit(&mut self.engine.runtime.frame_mut())
    }

    fn create_named_cache(&mut self) {
        self.engine.initialize_named_cache(self.execution.driver);
    }

    fn close_windows(&mut self) -> GameLogicResult<()> {
        self.engine
            .close_windows_with_driver(false, self.execution.driver)?;
        self.execution.driver.after_action()
    }

    fn queue_clear_game_data(&mut self) {
        use game_engine::common::message_stream::GameMessageType;
        log::info!("End game timer expired, appending MSG_CLEAR_GAME_DATA");
        if let Ok(mut stream) = self.engine.common.application.message_stream.write() {
            stream.append_message(GameMessageType::ClearGameData);
        }
    }

    fn execute_sides(&mut self) -> GameLogicResult<()> {
        self.engine.execute_side_scripts(self.execution)
    }

    fn update_teams(&mut self) -> GameLogicResult<()> {
        if let Some(result) = self.execution.driver.update_team_states(self.engine) {
            return result;
        }
        let query = self.engine.host_query();
        if let Some(world) = query.world() {
            // The standalone factory still has generic-script callbacks that
            // read its local compatibility scope when the guard is released.
            crate::system::engine_stores::with_world_services(world, || {
                if let Ok(mut factory) = self.engine.team_factory.lock() {
                    factory.update(&query);
                }
            });
            crate::team::dispatch_pending_team_script_events(world, self.engine, self.execution.frame);
        }
        Ok(())
    }

    fn execute_sequences(&mut self) -> GameLogicResult<()> {
        self.engine
            .evaluate_and_progress_all_sequential_scripts(self.execution)
    }
}
