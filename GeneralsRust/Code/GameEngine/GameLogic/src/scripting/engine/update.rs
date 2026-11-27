// Bind script-frame callbacks to the receiving interpreter and game.

impl ScriptEngine {
    pub(super) fn observe_object_count_changed_frame(&self, driver: &dyn ScriptExecutionDriver) {
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
        driver: &mut dyn ScriptExecutionDriver,
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
    pub fn initialize_named_cache_on_first_update(&self, driver: &mut dyn ScriptExecutionDriver) {
        let first_update = self.lock_inner_mut().frame.take_first_update();
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
        self.set_frame_object_count_changed(TheGameLogic::get_frame() as u32);
    }

    fn execute_side_scripts(&self, execution: &mut ScriptExecution<'_>) -> GameLogicResult<()> {
        self.side_scripts
            .with_execution(|| self.execute_side_scripts_from_owned_lists(execution))
    }

    fn execute_side_scripts_from_owned_lists(
        &self,
        execution: &mut ScriptExecution<'_>,
    ) -> GameLogicResult<()> {
        // C++ ScriptEngine.cpp:5554-5571 walks every side's ScriptList even
        // when no Objects exist. TRUE/COUNTER/FLAG/TIMER scripts still fire.
        // Host takes the engine out of the global RwLock before update()
        // (scripts_camera.rs), so this walk cannot re-lock TheScriptEngine.

        let mut program = ScriptProgramWorld::new(self, execution);

        // Snapshot player names before dispatch.  A script action may change
        // player state or call a subroutine; no PlayerList lock may survive
        // into that immediate nested path. C++ iterates TheSidesList, not the
        // live player count — walk every side slot so installed map scripts
        // run even when leftover PlayerList is empty.
        let mut standalone_player_names = None;

        for i in 0..Self::MAX_PLAYER_COUNT {
            // Match C++: `m_currentPlayer` is the nth player for the side index.
            let current_player = match program.execution.driver.side_script_current_player(i) {
                ScriptOwnerQuery::Present(name) => name,
                ScriptOwnerQuery::Missing => None,
                ScriptOwnerQuery::Unavailable => {
                    let player_names = standalone_player_names.get_or_insert_with(|| {
                        match self.players.try_read() {
                            Ok(list_guard) => {
                                let player_count =
                                    list_guard.get_player_count().min(Self::MAX_PLAYER_COUNT);
                                (0..player_count)
                                    .map(|index| {
                                        list_guard.get_player(index as i32).and_then(|player| {
                                            player.read().ok().and_then(|player| {
                                                self.common
                                                    .name_key_namespace
                                                    .lock()
                                                    .unwrap_or_else(|poisoned| {
                                                        poisoned.into_inner()
                                                    })
                                                    .key_to_name(player.get_player_name_key())
                                            })
                                        })
                                    })
                                    .collect::<Vec<_>>()
                            }
                            Err(_) => Vec::new(),
                        }
                    });
                    player_names.get(i).cloned().flatten()
                }
            };
            self.lock_inner_mut().current_player = current_player;

            self.side_scripts
                .execute_side(i, |script| program.run_script(script))?;
        }

        self.lock_inner_mut().current_player = None;
        Ok(())
    }
}

struct ScriptFrameWorld<'engine, 'context> {
    engine: &'engine ScriptEngine,
    execution: &'engine mut ScriptExecution<'context>,
}

impl ScriptFrameDriver for ScriptFrameWorld<'_, '_> {
    fn with_frame<R>(&self, edit: impl FnOnce(&mut ScriptFrame) -> R) -> R {
        edit(&mut self.engine.lock_inner_mut().frame)
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
