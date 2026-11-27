// ScriptEngine update, side-script execution, and sequential progress
//
// Split from `scripting/engine.rs` for module-size parity.
// Observable behavior is unchanged.

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

        // `update` has installed a lexical active engine, so nested
        // CALL_SUBROUTINE re-enters immediately without a second `&mut` borrow.

        self.initialize_named_cache_on_first_update(execution.driver);

        let close_expired = self.lock_inner_mut().mission_timers.tick_close_window();
        if close_expired {
            // C++ ScriptEngine.cpp:5508-5512 TheScriptActions->closeWindows(FALSE).
            self.close_windows_with_driver(false, execution.driver)?;
            // Expiry runs before authored trigger evaluation. Deliver the
            // callback now so later script conditions/actions observe the same
            // window order as C++ rather than waiting for end-of-update.
            execution.driver.after_action()?;
        }

        let end_expired = self.lock_inner_mut().mission_timers.tick_end_game();
        if end_expired {
            // C++ ScriptEngine.cpp:5514-5518 appends MSG_CLEAR_GAME_DATA.
            // It does not call TheGameLogic::clearGameData() directly.
            log::info!("End game timer expired, appending MSG_CLEAR_GAME_DATA");
            if let Ok(mut stream) =
                game_engine::common::message_stream::get_message_stream().write()
            {
                stream.append_message(
                    game_engine::common::message_stream::GameMessageType::ClearGameData,
                );
            }
        }

        // C++ parity: freeze-by-debug stops further script update progression.
        if self.is_time_frozen_debug() {
            return Ok(());
        }

        let end_game_active = {
            let mut inner = self.lock_inner_mut();
            inner.variables.tick();
            inner.mission_timers.end_game_active()
        };

        // Update fade effects
        self.update_fades();

        // If the engine is in an end-game timing-down state, C++ returns early.
        if end_game_active {
            return Ok(());
        }

        // Evaluate scripts for each player/side, matching C++ `ScriptEngine::update()`.
        self.execute_side_scripts(execution)?;

        // C++5573: team callbacks run after sides and before UI clearing/sequentials.
        if let Some(result) = execution.driver.update_team_states(self) {
            result?;
        } else {
            if let Ok(mut team_factory) = crate::team::get_team_factory().lock() {
                team_factory.update(&self.host_query());
            }
            crate::team::flush_pending_team_script_events();
        }

        // Clear UI interaction flags (C++: m_uiInteractions.clear()).
        self.lock_inner_mut().ui_interactions.clear();

        // Process sequential scripts
        self.evaluate_and_progress_all_sequential_scripts(execution)?;

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
        let first_update = {
            let mut inner = self.lock_inner_mut();
            if inner.first_update {
                inner.first_update = false;
                true
            } else {
                false
            }
        };
        if first_update {
            if driver.initialize_named_cache().is_none() {
                self.create_named_cache();
            }
            log::info!("ScriptEngine first update: named cache populated");
        }
    }

    /// Populate the NamedObjectTracker from currently registered objects.
    ///
    /// C++ Reference: `ScriptEngine::createNamedCache()`.
    fn create_named_cache(&self) {
        // Wave 348: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }
        let tracker = get_named_object_tracker();
        for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
            let obj_arc = match OBJECT_REGISTRY.get_object(obj_id) {
                Some(v) => v,
                None => continue,
            };
            let Ok(obj) = obj_arc.read() else { continue };
            let name = obj.get_name();
            if name.is_empty() {
                continue;
            }
            let _ = tracker.register_named_object(name.to_string(), obj.get_id());
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
        self.side_scripts.with_execution(|| self.execute_side_scripts_from_owned_lists(execution))
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
                        let player_list = crate::player::player_list();
                        match player_list.try_read() {
                            Ok(list_guard) => {
                                let player_count =
                                    list_guard.get_player_count().min(Self::MAX_PLAYER_COUNT);
                                (0..player_count)
                                    .map(|index| {
                                        list_guard.get_player(index as i32).and_then(|player| {
                                            player.read().ok().and_then(|player| {
                                                NameKeyGenerator::key_to_name(
                                                    player.get_player_name_key(),
                                                )
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

            self.side_scripts.execute_side(i, |script| program.run_script(script))?;
        }

        self.lock_inner_mut().current_player = None;
        Ok(())
    }

    fn initialize_script_runtime_fields_in_list(&self, script_list: &mut ScriptList) {
        if let Some(script_head) = script_list.first_script.as_deref_mut() {
            self.initialize_script_runtime_fields_in_chain(script_head);
        }

        let mut group_opt = script_list.first_group.as_deref_mut();
        while let Some(group) = group_opt {
            if let Some(script_head) = group.first_script.as_deref_mut() {
                self.initialize_script_runtime_fields_in_chain(script_head);
            }
            group_opt = group.next_group.as_deref_mut();
        }
    }

    fn initialize_script_runtime_fields_in_chain(&self, script_head: &mut Script) {
        let mut current = Some(script_head);
        while let Some(script) = current {
            self.initialize_script_runtime_fields(script);
            current = script.next_script.as_deref_mut();
        }
    }

    fn initialize_script_runtime_fields(&self, script: &mut Script) {
        self.initialize_script_evaluation_frame(script);
        self.infer_script_condition_team_name(script);
    }

    fn initialize_script_evaluation_frame(&self, script: &mut Script) {
        if script.delay_evaluation_seconds > 0 {
            let max_offset = (2 * LOGICFRAMES_PER_SECOND as i32).max(0);
            let random_offset = crate::helpers::get_game_logic_random_value(0, max_offset).max(0);
            script.frame_to_evaluate_at = random_offset as u32;
        } else {
            script.frame_to_evaluate_at = 0;
        }
    }

    fn infer_script_condition_team_name(&self, script: &mut Script) {
        let mut singleton_team_name = String::new();
        let mut multi_team_name = String::new();
        let script_name = script.script_name.clone();

        let mut or_condition = script.condition.as_deref();
        while let Some(or_node) = or_condition {
            let mut and_condition = or_node.first_and.as_deref();
            while let Some(condition) = and_condition {
                for index in 0..condition.get_num_parameters() {
                    let Some(param) = condition.get_parameter(index) else {
                        continue;
                    };
                    if param.get_parameter_type() != ParameterType::Team {
                        continue;
                    }

                    let team_name = param.get_string().trim();
                    if team_name.is_empty() {
                        continue;
                    }

                    let published_team_factory = get_team_factory();
                    let Some(prototype) = published_team_factory
                        .lock()
                        .ok()
                        .and_then(|factory| factory.find_team_prototype(team_name))
                    else {
                        continue;
                    };

                    let is_singleton =
                        prototype.is_singleton() || prototype.get_max_instances() < 2;
                    if is_singleton {
                        singleton_team_name = team_name.to_string();
                    } else if multi_team_name.is_empty() {
                        multi_team_name = team_name.to_string();
                    } else if multi_team_name != team_name {
                        log::warn!(
                            "Script '{}' contains multiple non-singleton team conditions: '{}' and '{}'",
                            script_name,
                            multi_team_name,
                            team_name
                        );
                    }
                }
                and_condition = condition.get_next();
            }
            or_condition = or_node.get_next_or_condition();
        }

        if !multi_team_name.is_empty() {
            script.condition_team_name = multi_team_name;
        } else if !singleton_team_name.is_empty() {
            script.condition_team_name = singleton_team_name;
        }
    }

    /// Update fade effects
    fn update_fades(&self) {
        let mut inner = self.lock_inner_mut();
        if inner.fade == TFade::None {
            return;
        }

        inner.cur_fade_frame += 1;
        let mut fade = inner.cur_fade_frame;

        if fade <= inner.fade_frames_increase {
            let factor = inner.cur_fade_frame as f32 / inner.fade_frames_increase as f32;
            inner.cur_fade_value = inner.min_fade + factor * (inner.max_fade - inner.min_fade);
            return;
        }

        fade -= inner.fade_frames_increase;
        if fade <= inner.fade_frames_hold {
            inner.cur_fade_value = inner.max_fade;
            return;
        }

        fade -= inner.fade_frames_hold;
        if fade <= inner.fade_frames_decrease {
            let mut divisor = inner.fade_frames_decrease + 1;
            if divisor == 0 {
                divisor = 1;
            }
            let factor = fade as f32 / divisor as f32;
            inner.cur_fade_value = inner.max_fade + factor * (inner.min_fade - inner.max_fade);
            return;
        }

        inner.fade = TFade::None;
    }
}
