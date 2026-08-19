// ScriptEngine update, side-script execution, and sequential progress
//
// Split from `scripting/engine.rs` for module-size parity.
// Observable behavior is unchanged.

/// Restores the script context even if an immediate sequential action unwinds.
///
/// C++ leaves these fields scoped to sequential evaluation.  More
/// importantly, this guard is created before the dispatcher and every inner
/// borrow ends before an action runs, so restoration itself never overlaps a
/// re-entrant `SET_FLAG` / `CALL_SUBROUTINE` mutation.
struct SequentialContextRestore<'a> {
    engine: &'a ScriptEngine,
    current_player: Option<String>,
    condition_team: Option<String>,
    condition_object: Option<u32>,
}

impl Drop for SequentialContextRestore<'_> {
    fn drop(&mut self) {
        let mut inner = self.engine.lock_inner_mut();
        inner.current_player = self.current_player.clone();
        inner.condition_team = self.condition_team.clone();
        inner.condition_object = self.condition_object;
    }
}

impl ScriptEngine {
    /// Update script engine
    pub fn update(&self) -> GameLogicResult<()> {
        self.with_active(|| self.update_active())
    }

    fn update_active(&self) -> GameLogicResult<()> {
        #[cfg(feature = "script_profiling")]
        let start_time = Instant::now();

        // `update` has installed a lexical active engine, so nested
        // CALL_SUBROUTINE re-enters immediately without a second `&mut` borrow.

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
            self.create_named_cache();
            log::info!("ScriptEngine first update: named cache populated");
        }

        let close_expired = {
            let mut inner = self.lock_inner_mut();
            if inner.close_window_timer > 0 {
                inner.close_window_timer -= 1;
                inner.close_window_timer < 1
            } else {
                false
            }
        };
        if close_expired {
            // C++ ScriptEngine.cpp:5508-5512 TheScriptActions->closeWindows(FALSE).
            self.close_windows(false);
        }

        let end_expired = {
            let mut inner = self.lock_inner_mut();
            if inner.end_game_timer > 0 {
                inner.end_game_timer -= 1;
                inner.end_game_timer < 1
            } else {
                false
            }
        };
        if end_expired {
            // C++ ScriptEngine.cpp:5514-5518 appends MSG_CLEAR_GAME_DATA.
            // It does not call TheGameLogic::clearGameData() directly.
            log::info!("End game timer expired, appending MSG_CLEAR_GAME_DATA");
            if let Ok(mut stream) = game_engine::common::message_stream::get_message_stream().write()
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
            // Update counters that are countdown timers
            for counter in &mut inner.counters {
                if let Some(counter) = counter {
                    // C++ ScriptEngine.cpp:5547-5550: decrement while value >= 0;
                    // "Counters go to -1 and stop."
                    if counter.is_countdown_timer && counter.value >= 0 {
                        counter.value -= 1;
                    }
                }
            }
            inner.end_game_timer >= 0
        };

        // Update fade effects
        self.update_fades();

        // If the engine is in an end-game timing-down state, C++ returns early.
        if end_game_active {
            return Ok(());
        }

        // Evaluate scripts for each player/side, matching C++ `ScriptEngine::update()`.
        self.execute_side_scripts()?;

        // C++ ScriptEngine.cpp:5573-5575 ThePlayerList->updateTeamStates()
        if let Ok(mut team_factory) = crate::team::get_team_factory().lock() {
            team_factory.update();
        }
        crate::team::flush_pending_team_script_events();

        // Clear UI interaction flags (C++: m_uiInteractions.clear()).
        self.lock_inner_mut().ui_interactions.clear();

        // Process sequential scripts
        self.evaluate_and_progress_all_sequential_scripts()?;

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
    pub fn notify_of_object_creation_or_destruction(&mut self) {
        self.create_named_cache();
    }

    fn execute_side_scripts(&self) -> GameLogicResult<()> {
        self.with_active_script_lists(|| self.execute_side_scripts_from_active_lists())
    }

    fn execute_side_scripts_from_active_lists(&self) -> GameLogicResult<()> {
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        // Host GAME_SHELL ticks ScriptEngine with an empty crate OBJECT_REGISTRY.
        // Walking side lists still re-enters get_script_engine()/PlayerList and
        // hung the windowed Menu after "named cache populated" (hq-9e2w).
        // C++ still runs scripts; host objects are not crate Objects, so
        // conditions cannot resolve them here anyway.
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        // Prepare executor context for this frame (shared by action/condition evaluation).
        let exec_context = Arc::new(RwLock::new(crate::scripting::executor::ScriptContext {
            game_logic_id: 0,
            object_manager_id: 0,
            player_manager_id: 0,
            event_system_id: 0,
            camera_system_id: 0,
            audio_system_id: 0,
            partition_manager_id: 0,
            special_powers_id: 0,
            current_frame,
            suppress_new_windows: false,
        }));

        let mut action_dispatcher =
            crate::scripting::executor::ScriptActionDispatcher::new(exec_context.clone());
        let mut condition_evaluator =
            crate::scripting::executor::ScriptConditionEvaluator::new(exec_context);

        // Snapshot player names before dispatch.  A script action may change
        // player state or call a subroutine; no PlayerList lock may survive
        // into that immediate nested path.
        let player_names = {
            let player_list = crate::player::player_list();
            let Ok(list_guard) = player_list.try_read() else {
                return Ok(());
            };
            let player_count = list_guard.get_player_count().min(Self::MAX_PLAYER_COUNT);
            (0..player_count)
                .map(|index| {
                    list_guard.get_player(index as i32).and_then(|player| {
                        player.read().ok().and_then(|player| {
                            NameKeyGenerator::key_to_name(player.get_player_name_key())
                        })
                    })
                })
                .collect::<Vec<_>>()
        };


        for (i, player_name) in player_names.into_iter().enumerate() {
            // Match C++: `m_currentPlayer` is the nth player for the side index.
            self.lock_inner_mut().current_player = player_name;

            // Every side list remains searchable through the lexical active
            // store while an individual script is detached for dispatch.
            self.execute_non_subroutine_scripts_in_container(
                i,
                ScriptContainer::Root,
                &mut condition_evaluator,
                &mut action_dispatcher,
            )?;

            // Execute active non-subroutine groups.
            let mut group_index = 0;
            while let Some((is_active, is_subroutine)) = self.group_state_at(GroupLocation {
                side_index: i,
                group_index,
            }) {
                if is_active && !is_subroutine {
                    self.execute_non_subroutine_scripts_in_container(
                        i,
                        ScriptContainer::Group(group_index),
                        &mut condition_evaluator,
                        &mut action_dispatcher,
                    )?;
                }
                group_index += 1;
            }
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

                    let Some(prototype) = get_team_factory()
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

    /// Execute a single script, matching C++ `ScriptEngine::executeScript`.
    fn execute_script(
        &self,
        script: &mut Script,
        condition_evaluator: &mut crate::scripting::executor::ScriptConditionEvaluator,
        action_dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
    ) -> GameLogicResult<()> {
        // If script is not active, return.
        if !script.is_active {
            return Ok(());
        }

        // Difficulty gating (C++ uses `m_currentPlayer->getPlayerDifficulty()` when available).
        let difficulty = self
            .with_inner(|inner| inner.current_player.clone())
            .as_deref()
            .and_then(|name| {
                crate::player::player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.find_player_by_name(name))
                    .and_then(|p| p.read().ok().map(|p| p.get_player_difficulty()))
            })
            .unwrap_or(crate::player::GameDifficulty::Normal);

        match difficulty {
            crate::player::GameDifficulty::Easy if !script.easy => return Ok(()),
            crate::player::GameDifficulty::Normal if !script.normal => return Ok(()),
            crate::player::GameDifficulty::Hard | crate::player::GameDifficulty::Brutal
                if !script.hard =>
            {
                return Ok(());
            }
            _ => {}
        }

        // Periodic evaluation gate.
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        if current_frame < script.frame_to_evaluate_at {
            return Ok(());
        }

        // If delay is configured, schedule the next evaluation time.
        if script.delay_evaluation_seconds > 0 {
            script.frame_to_evaluate_at = current_frame
                + (script.delay_evaluation_seconds as u32) * (LOGICFRAMES_PER_SECOND as u32);
        }

        // Team-scoped condition evaluation (C++ uses `conditionTeamName` to iterate instances).
        let saved_condition_team = self.lock_inner_mut().condition_team.take();

        let condition_team_name = script.condition_team_name.trim().to_string();
        if !condition_team_name.is_empty() {
            let instances = crate::team::get_team_factory()
                .lock()
                .ok()
                .map(|factory| factory.find_team_instances(&condition_team_name))
                .unwrap_or_default();

            if !instances.is_empty() {
                for team_arc in instances {
                    let team_name = team_arc
                        .read()
                        .ok()
                        .map(|t| t.get_name().to_string())
                        .unwrap_or_else(|| condition_team_name.clone());
                    self.lock_inner_mut().condition_team = Some(team_name);
                    self.evaluate_and_execute_script(
                        script,
                        condition_evaluator,
                        action_dispatcher,
                        false,
                    )?;
                }
                self.lock_inner_mut().condition_team = saved_condition_team;
                return Ok(());
            }
        }

        self.lock_inner_mut().condition_team = None;
        self.evaluate_and_execute_script(script, condition_evaluator, action_dispatcher, true)?;
        self.lock_inner_mut().condition_team = saved_condition_team;
        Ok(())
    }

    fn evaluate_and_execute_script(
        &self,
        script: &mut Script,
        condition_evaluator: &mut crate::scripting::executor::ScriptConditionEvaluator,
        action_dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
        deactivate_one_shot_on_false_action: bool,
    ) -> GameLogicResult<()> {
        // If no conditions, C++ treats as false (no AND chain).
        let mut condition_true = false;
        if let Some(or_cond) = script.condition.as_deref_mut() {
            condition_true = condition_evaluator
                .evaluate_or_condition(or_cond)
                .map_err(|e| {
                    GameLogicError::Configuration(format!("Script condition error: {}", e))
                })?;
        }

        if condition_true {
            let mut action_state = ActionChainExecution::Completed;
            if let Some(action_head) = script.action.as_deref() {
                action_state = self.execute_action_chain(action_head, action_dispatcher)?;
            }
            match action_state {
                ActionChainExecution::Completed => {
                    if script.is_one_shot {
                        script.is_active = false;
                    }
                }
                ActionChainExecution::Pending(frames) => {
                    self.schedule_script_pending_frames(script, frames);
                }
            }
        } else if let Some(false_action) = script.action_false.as_deref() {
            match self.execute_action_chain(false_action, action_dispatcher)? {
                ActionChainExecution::Completed => {
                    if script.is_one_shot && deactivate_one_shot_on_false_action {
                        script.is_active = false;
                    }
                }
                ActionChainExecution::Pending(frames) => {
                    self.schedule_script_pending_frames(script, frames);
                }
            }
        }

        Ok(())
    }

    fn execute_action_chain(
        &self,
        action_head: &ScriptAction,
        dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
    ) -> GameLogicResult<ActionChainExecution> {
        let mut cur: Option<&ScriptAction> = Some(action_head);
        while let Some(action) = cur {
            let result = dispatcher.execute_action(action).map_err(|e| {
                GameLogicError::Configuration(format!("Script action error: {}", e))
            })?;
            match result {
                crate::scripting::executor::ScriptActionResult::Success => {}
                crate::scripting::executor::ScriptActionResult::Pending(frames) => {
                    if Self::pending_is_sequential_only_action(action.action_type) {
                        // C++ parity: these actions are implemented as sequential timers/checks and
                        // should not pause standard script action chains.
                        cur = action.next_action.as_deref();
                        continue;
                    }
                    return Ok(ActionChainExecution::Pending(frames));
                }
                crate::scripting::executor::ScriptActionResult::Failed(msg) => {
                    return Err(GameLogicError::Configuration(format!(
                        "Script action failed: {}",
                        msg
                    )));
                }
            }
            cur = action.next_action.as_deref();
        }
        Ok(ActionChainExecution::Completed)
    }

    fn schedule_script_pending_frames(&self, script: &mut Script, pending_frames: f32) {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let pending_resume_frame = Self::pending_resume_frame(current_frame, pending_frames);
        script.frame_to_evaluate_at = script.frame_to_evaluate_at.max(pending_resume_frame);
    }

    fn pending_resume_frame(current_frame: u32, pending_frames: f32) -> u32 {
        let wait_frames = pending_frames.max(1.0).ceil() as u32;
        current_frame.saturating_add(wait_frames)
    }

    fn pending_repeats_current_sequential_instruction(action_type: ScriptActionType) -> bool {
        matches!(
            action_type,
            ScriptActionType::SkirmishWaitForCommandbuttonAvailableAll
                | ScriptActionType::SkirmishWaitForCommandbuttonAvailablePartial
                | ScriptActionType::TeamWaitForNotContainedAll
                | ScriptActionType::TeamWaitForNotContainedPartial
        )
    }

    fn pending_is_sequential_only_action(action_type: ScriptActionType) -> bool {
        Self::pending_repeats_current_sequential_instruction(action_type)
            || matches!(
                action_type,
                ScriptActionType::TeamGuardForFramecount
                    | ScriptActionType::TeamIdleForFramecount
                    | ScriptActionType::TeamSpinForFramecount
                    | ScriptActionType::UnitGuardForFramecount
                    | ScriptActionType::UnitIdleForFramecount
            )
    }

    fn pending_to_sequential_wait_frames(
        pending_frames: f32,
        repeat_current_instruction: bool,
    ) -> i32 {
        let wait_frames = pending_frames.max(0.0).ceil() as i32;
        if repeat_current_instruction {
            wait_frames.saturating_sub(1)
        } else {
            wait_frames.max(0)
        }
    }

    /// Update the victory condition manager with the current context
    pub fn update_victory_manager(
        &self,
        _context: crate::scripting::ScriptContext,
    ) -> GameLogicResult<()> {
        Ok(())
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

    /// Evaluate and progress sequential scripts
    fn evaluate_and_progress_all_sequential_scripts(&self) -> GameLogicResult<()> {
        // Wave 348: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let restore = self.with_inner(|inner| SequentialContextRestore {
            engine: self,
            current_player: inner.current_player.clone(),
            condition_team: inner.condition_team.clone(),
            condition_object: inner.condition_object,
        });

        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let exec_context = Arc::new(RwLock::new(crate::scripting::executor::ScriptContext {
            game_logic_id: 0,
            object_manager_id: 0,
            player_manager_id: 0,
            event_system_id: 0,
            camera_system_id: 0,
            audio_system_id: 0,
            partition_manager_id: 0,
            special_powers_id: 0,
            current_frame,
            suppress_new_windows: false,
        }));
        let mut dispatcher = crate::scripting::executor::ScriptActionDispatcher::new(exec_context);

        let mut i: usize = 0;
        let mut last_i: Option<usize> = None;
        let mut spin_count: i32 = 0;

        while i < self.sequential_script_count() {
            let Some(sequence) = self.sequential_script_snapshot_at(i) else {
                // There is no concurrent mutation of a ScriptEngine. Reaching
                // this branch means a malformed runtime list, so stop rather
                // than risking an unbounded retry against an absent node.
                break;
            };
            let token = sequence.runtime_token;

            if last_i == Some(i) {
                spin_count += 1;
            } else {
                spin_count = 0;
            }
            last_i = Some(i);

            if spin_count > MAX_SEQUENTIAL_SPIN_COUNT {
                if let Some(seq_name) = sequence
                    .script_to_execute_sequentially
                    .as_ref()
                    .map(|script| script.script_name.as_str())
                {
                    log::warn!(
                        "Sequential script '{}' appears to be in an infinite loop",
                        seq_name
                    );
                }
                i += 1;
                continue;
            }

            if sequence.script_to_execute_sequentially.is_none() {
                if self
                    .cleanup_sequential_script_by_token(token, false)
                    .is_none()
                {
                    i += 1;
                }
                continue;
            }

            let mut it_advanced = false;
            let team_name = sequence.team_to_exec_on.clone();
            let object_id = sequence.object_id;
            let team_arc = team_name.as_ref().and_then(|name| {
                get_team_factory()
                    .lock()
                    .ok()
                    .and_then(|mut factory| factory.find_team(name))
            });
            let object_arc = (object_id != INVALID_ID)
                .then(|| TheGameLogic::find_object_by_id(object_id))
                .flatten();

            if object_arc.is_none() && team_arc.is_none() {
                if self
                    .cleanup_sequential_script_by_token(token, false)
                    .is_none()
                {
                    i += 1;
                }
                continue;
            }

            {
                let mut inner = self.lock_inner_mut();
                inner.current_player =
                    self.resolve_sequential_current_player(object_arc.as_ref(), team_arc.as_ref());
            }

            let (obj_has_ai, obj_idle, _) = object_arc
                .as_ref()
                .map(Self::object_ai_status)
                .unwrap_or((false, false, false));
            let (team_has_group, team_idle, _) = team_arc
                .as_ref()
                .map(|team| {
                    let (idle, dead) = Self::team_ai_status(team);
                    (true, idle, dead)
                })
                .unwrap_or((false, false, false));

            if obj_has_ai || team_has_group {
                let should_progress = (((obj_has_ai && obj_idle) || (team_has_group && team_idle))
                    && sequence.frames_to_wait < 1)
                    || sequence.frames_to_wait == 0;

                if should_progress {
                    let Some((_instruction, action)) = self.advance_sequential_instruction(token)
                    else {
                        // An immediately nested mutation removed this node.
                        // Leave `i` unchanged so its successor is considered.
                        continue;
                    };

                    if let Some(action) = action {
                        if !self.prepare_sequential_action_context(
                            token,
                            team_name.clone(),
                            object_arc.as_ref().map(|_| object_id),
                        ) {
                            continue;
                        }

                        // Invariant: every helper above returns owned state and
                        // drops its `RefCell` guard before this call. Actions
                        // therefore retain C++ immediate nesting without
                        // aliasing the sequential Vec entry.
                        let result = dispatcher.execute_action(&action).map_err(|error| {
                            GameLogicError::Configuration(format!(
                                "Sequential script action error: {}",
                                error
                            ))
                        })?;

                        // An action can re-enter the engine. Reconcile by the
                        // opaque node identity instead of treating whatever is
                        // now at `i` as the original sequence.
                        let Some(current_index) = self.sequential_script_index_by_token(token)
                        else {
                            log::debug!(
                                "Sequential script node {} was removed during immediate action dispatch",
                                token
                            );
                            continue;
                        };
                        i = current_index;

                        match result {
                            crate::scripting::executor::ScriptActionResult::Success => {}
                            crate::scripting::executor::ScriptActionResult::Pending(frames) => {
                                let repeats_instruction =
                                    Self::pending_repeats_current_sequential_instruction(
                                        action.action_type,
                                    );
                                let wait_frames = Self::pending_to_sequential_wait_frames(
                                    frames,
                                    repeats_instruction,
                                );
                                if !self.set_sequential_pending_result(
                                    token,
                                    repeats_instruction,
                                    wait_frames,
                                ) {
                                    continue;
                                }
                            }
                            crate::scripting::executor::ScriptActionResult::Failed(message) => {
                                return Err(GameLogicError::Configuration(format!(
                                    "Sequential script action failed: {}",
                                    message
                                )));
                            }
                        }

                        let Some(post_action) = self.sequential_script_snapshot_by_token(token)
                        else {
                            continue;
                        };
                        if post_action.dont_advance_instruction {
                            i += 1;
                            continue;
                        }

                        let obj_idle_now = object_arc
                            .as_ref()
                            .map(|object| Self::object_ai_status(object).1)
                            .unwrap_or(false);
                        let team_idle_now = team_arc
                            .as_ref()
                            .map(|team| Self::team_ai_status(team).0)
                            .unwrap_or(false);
                        if (obj_has_ai && obj_idle_now) || (team_has_group && team_idle_now) {
                            it_advanced = true;
                        }

                        if it_advanced {
                            let obj_dead_now = object_arc
                                .as_ref()
                                .map(|object| Self::object_ai_status(object).2)
                                .unwrap_or(false);
                            let team_dead_now = team_arc
                                .as_ref()
                                .map(|team| Self::team_ai_status(team).1)
                                .unwrap_or(false);
                            if obj_dead_now || team_dead_now {
                                let _ = self.cleanup_sequential_script_by_token(token, true);
                                continue;
                            }
                        }
                    } else {
                        if sequence.times_to_loop != 0 {
                            let mut loop_script = sequence.clone();
                            if loop_script.times_to_loop != -1 {
                                loop_script.times_to_loop -= 1;
                            }
                            loop_script.frames_to_wait = -1;
                            self.append_sequential_script(loop_script);
                        }
                        let _ = self.cleanup_sequential_script_by_token(token, false);
                        it_advanced = true;
                    }
                } else if sequence.frames_to_wait > 0 {
                    let _ = self.decrement_sequential_wait(token);
                }
            }

            if !it_advanced {
                i += 1;
            }
        }

        drop(restore);
        Ok(())
    }

    fn script_action_at_instruction(
        script: &SequentialScript,
        instruction: i32,
    ) -> Option<ScriptAction> {
        if instruction < 0 {
            return None;
        }

        let mut action = script
            .script_to_execute_sequentially
            .as_ref()
            .and_then(|seq| seq.action.as_deref());
        let mut remaining = instruction;
        while remaining > 0 {
            action = action.and_then(|node| node.get_next());
            remaining -= 1;
        }
        action.cloned()
    }

    fn object_ai_status(object_arc: &Arc<RwLock<crate::object::Object>>) -> (bool, bool, bool) {
        let Ok(object) = object_arc.read() else {
            return (false, false, true);
        };
        let has_ai = object.get_ai_update_interface().is_some();
        let idle = object.is_idle();
        let dead = object.is_effectively_dead();
        (has_ai, idle, dead)
    }

    fn team_ai_status(team_arc: &Arc<RwLock<crate::team::Team>>) -> (bool, bool) {
        // Wave 348: empty dual-world → (false, true).
        if dual_world_registry_unavailable() {
            return (false, true);
        }

        let Ok(team) = team_arc.read() else {
            return (false, true);
        };

        let idle = team.is_idle();
        let mut all_dead = true;
        for &member_id in team.get_members() {
            let Some(object_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(object) = object_arc.read() else {
                continue;
            };
            if !object.is_effectively_dead() {
                all_dead = false;
                break;
            }
        }

        (idle, all_dead)
    }

    fn resolve_sequential_current_player(
        &self,
        object_arc: Option<&Arc<RwLock<crate::object::Object>>>,
        team_arc: Option<&Arc<RwLock<crate::team::Team>>>,
    ) -> Option<String> {
        let player_id = if let Some(object_arc) = object_arc {
            object_arc
                .read()
                .ok()
                .and_then(|object| object.get_controlling_player_id())
        } else if let Some(team_arc) = team_arc {
            team_arc
                .read()
                .ok()
                .and_then(|team| team.get_controlling_player_id())
        } else {
            None
        }?;

        crate::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_id as i32).cloned())
            .and_then(|player| {
                player.read().ok().and_then(|player| {
                    if player.is_skirmish_ai() {
                        NameKeyGenerator::key_to_name(player.get_player_name_key())
                    } else {
                        None
                    }
                })
            })
    }

    fn sequential_script_count(&self) -> usize {
        self.with_inner(|inner| inner.sequential_scripts.len())
    }

    /// Returns an owned runtime snapshot. The `Vec` borrow ends before the
    /// caller can dispatch an action.
    fn sequential_script_snapshot_at(&self, index: usize) -> Option<SequentialScript> {
        let mut inner = self.lock_inner_mut();
        Self::ensure_sequential_runtime_token_at(&mut inner, index)?;
        inner.sequential_scripts.get(index).cloned()
    }

    fn sequential_script_index_by_token(&self, token: u64) -> Option<usize> {
        self.with_inner(|inner| {
            inner
                .sequential_scripts
                .iter()
                .position(|script| script.runtime_token == token)
        })
    }

    fn sequential_script_snapshot_by_token(&self, token: u64) -> Option<SequentialScript> {
        self.with_inner(|inner| {
            inner
                .sequential_scripts
                .iter()
                .find(|script| script.runtime_token == token)
                .cloned()
        })
    }

    /// Advance one node exactly as C++ does, returning an owned action. No
    /// reference into `sequential_scripts` reaches the dispatcher.
    fn advance_sequential_instruction(&self, token: u64) -> Option<(i32, Option<ScriptAction>)> {
        let mut inner = self.lock_inner_mut();
        let index = inner
            .sequential_scripts
            .iter()
            .position(|script| script.runtime_token == token)?;
        let script = inner.sequential_scripts.get_mut(index)?;
        if script.dont_advance_instruction {
            script.dont_advance_instruction = false;
        } else {
            script.current_instruction += 1;
        }
        let instruction = script.current_instruction;
        let action = Self::script_action_at_instruction(script, instruction);
        Some((instruction, action))
    }

    fn prepare_sequential_action_context(
        &self,
        token: u64,
        team_name: Option<String>,
        object_id: Option<u32>,
    ) -> bool {
        let mut inner = self.lock_inner_mut();
        inner.condition_team = team_name;
        inner.condition_object = object_id;
        let Some(script) = inner
            .sequential_scripts
            .iter_mut()
            .find(|script| script.runtime_token == token)
        else {
            return false;
        };
        script.frames_to_wait = -1;
        true
    }

    fn set_sequential_pending_result(
        &self,
        token: u64,
        dont_advance_instruction: bool,
        frames_to_wait: i32,
    ) -> bool {
        let mut inner = self.lock_inner_mut();
        let Some(script) = inner
            .sequential_scripts
            .iter_mut()
            .find(|script| script.runtime_token == token)
        else {
            return false;
        };
        script.dont_advance_instruction = dont_advance_instruction;
        script.frames_to_wait = frames_to_wait;
        true
    }

    fn decrement_sequential_wait(&self, token: u64) -> bool {
        let mut inner = self.lock_inner_mut();
        let Some(script) = inner
            .sequential_scripts
            .iter_mut()
            .find(|script| script.runtime_token == token)
        else {
            return false;
        };
        if script.frames_to_wait > 0 {
            script.frames_to_wait -= 1;
        }
        true
    }

    /// C++ `cleanupSequentialScript` by stable Rust runtime identity. Returning
    /// the current index mirrors C++'s iterator result: callers can continue
    /// on a promoted chained node without touching a successor that replaced a
    /// removed node during an immediate action.
    fn cleanup_sequential_script_by_token(
        &self,
        token: u64,
        clean_danglers: bool,
    ) -> Option<usize> {
        let mut inner = self.lock_inner_mut();
        let index = inner
            .sequential_scripts
            .iter()
            .position(|script| script.runtime_token == token)?;

        if clean_danglers {
            inner.sequential_scripts.remove(index);
            return Some(index);
        }

        let next = inner.sequential_scripts[index]
            .next_script_in_sequence
            .take();
        if let Some(next_script) = next {
            inner.sequential_scripts[index] = *next_script;
        } else {
            inner.sequential_scripts.remove(index);
        }
        Some(index)
    }
}
