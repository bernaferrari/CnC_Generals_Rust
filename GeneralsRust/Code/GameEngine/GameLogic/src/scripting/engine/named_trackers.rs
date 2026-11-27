// Counters, flags, timers, named reveals, and named object trackers
//
// Split from `scripting/engine.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEngine {
    pub fn allocate_counter(&self, name: &str) -> GameLogicResult<usize> {
        self.runtime.frame_mut().variables.allocate_counter(name)
    }

    pub fn allocate_flag(&self, name: &str) -> GameLogicResult<usize> {
        self.runtime.frame_mut().variables.allocate_flag(name)
    }

    pub fn get_counter(&self, name: &str) -> Option<TCounter> {
        self.runtime.frame().variables.counter(name).cloned()
    }

    pub fn get_flag(&self, name: &str) -> Option<TFlag> {
        self.runtime.frame().variables.flag(name).cloned()
    }

    pub fn set_counter(&self, name: &str, value: i32) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables.set_counter(name, value)
    }

    pub fn set_flag(&self, name: &str, value: bool) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables.set_flag(name, value)
    }

    /// C++ `ScriptEngine::xfer` counter/flag tables.
    pub fn snapshot_named_trackers(&self) -> NamedTrackersSnapshot {
        self.runtime.frame().variables.snapshot()
    }

    pub fn restore_named_trackers(
        &self,
        counters: &[(String, i32, bool)],
        flags: &[(String, bool)],
    ) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables.restore(counters, flags)
    }

    /// C++ ScriptEngine::xfer writes each sequential script head.
    pub fn snapshot_sequential_scripts(&self) -> Vec<SequentialScriptSnapshot> {
        self.runtime.sequential().snapshot_heads()
    }

    pub fn restore_sequential_scripts(&self, scripts: &[SequentialScriptSnapshot]) {
        self.runtime.sequential().restore_heads(scripts.iter().map(|snap| {
            let mut script = SequentialScript::new();
            script.object_id = snap.object_id;
            script.current_instruction = snap.current_instruction;
            script.times_to_loop = snap.times_to_loop;
            script.frames_to_wait = snap.frames_to_wait;
            script.dont_advance_instruction = snap.dont_advance_instruction;
            script.team_to_exec_on = (snap.team_id != TEAM_ID_INVALID).then_some(snap.team_id);
            if !snap.script_name.is_empty() {
                script.script_to_execute_sequentially = Some(Box::new(
                    self.find_script_clone_by_name(&snap.script_name)
                        .unwrap_or_else(|| {
                            let mut stub = Script::new();
                            stub.script_name = snap.script_name.clone();
                            stub
                        }),
                ));
            }
            script
        }));
    }

    pub fn increment_counter(&self, name: &str, amount: i32) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables
            .increment_counter(name, amount)
    }

    pub fn decrement_counter(&self, name: &str, amount: i32) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables
            .decrement_counter(name, amount)
    }

    pub fn set_timer(&self, name: &str, frames: i32) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables.set_timer(name, frames)
    }

    pub fn set_timer_seconds(&self, name: &str, seconds: f32) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables
            .set_timer_seconds(name, seconds)
    }

    pub fn set_timer_millisecond_script_seconds(
        &self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables
            .set_timer_millisecond_script_seconds(name, seconds)
    }

    pub fn stop_timer(&self, name: &str) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables.stop_timer(name)
    }

    pub fn restart_timer(&self, name: &str) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables.restart_timer(name)
    }

    pub fn add_to_timer_millisecond_script_seconds(
        &self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables
            .add_to_timer_millisecond_script_seconds(name, seconds)
    }

    pub fn subtract_from_timer_millisecond_script_seconds(
        &self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables
            .subtract_from_timer_millisecond_script_seconds(name, seconds)
    }

    /// Start the normal win/lose timer.
    ///
    /// C++ `FRAMES_TO_SHOW_WIN_LOSE_MESSAGE` is 120 logic frames.  This is
    /// interior state so script actions can start the timer while the active
    /// engine is lexically scoped, without trying to lock the global engine
    /// again.
    pub fn start_end_game_timer(&self) {
        self.runtime.frame_mut().mission_timers.start_end_game();
        log::info!("End game timer started");
    }

    /// Start the one-frame quick-victory timer.
    pub fn start_quick_end_game_timer(&self) {
        self.runtime.frame_mut().mission_timers.start_quick_end_game();
        log::info!("Quick end game timer started");
    }

    /// Start the local-defeat close-window timer.
    pub fn start_close_window_timer(&self) {
        self.runtime.frame_mut().mission_timers.start_close_window();
        log::info!("Close window timer started");
    }

    /// C++ ScriptActions::closeWindows ScriptActions.cpp:156-163.
    pub fn close_windows(&self, suppress_new_windows: bool) {
        let handler = {
            let mut inner = self.lock_inner_mut();
            inner.suppress_new_windows = suppress_new_windows;
            inner.win_lose_window_layout.take();
            inner.action_handler.clone()
        };
        if let Some(handler) = handler {
            let _ = handler.destroy_win_lose_window();
        }
    }

    /// Receiving counterpart of closeWindows; update tracking before callbacks,
    /// release the engine inner loan, then destroy at the original action slot.
    pub fn close_windows_with_driver(
        &self,
        suppress_new_windows: bool,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> GameLogicResult<()> {
        {
            let mut inner = self.lock_inner_mut();
            inner.suppress_new_windows = suppress_new_windows;
            inner.win_lose_window_layout.take();
        }
        if let Some(result) = driver.destroy_win_lose_window() {
            return result;
        }
        let handler = self.with_inner(|inner| inner.action_handler.clone());
        if let Some(handler) = handler {
            handler.destroy_win_lose_window()
        } else {
            Err(crate::GameLogicError::Configuration(
                "script window destruction requires receiving UI or an explicit headless driver"
                    .into(),
            ))
        }
    }
    pub fn close_game_windows_with_driver(
        &self,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> GameLogicResult<()> {
        if let Some(result) = driver.close_game_windows() {
            return result;
        }
        let handler = self.with_inner(|inner| inner.action_handler.clone());
        if let Some(handler) = handler {
            handler.close_game_windows()
        } else {
            Err(crate::GameLogicError::Configuration(
                "script window destruction requires receiving UI or an explicit headless driver"
                    .into(),
            ))
        }
    }

    /// C++ GameLogic::closeWindows GameLogicDispatch.cpp:202-219.
    pub fn close_game_windows(&self) {
        let handler = self.with_inner(|inner| inner.action_handler.clone());
        if let Some(handler) = handler {
            let _ = handler.close_game_windows();
        }
    }

    /// C++ ScriptActions.cpp:196-205 / 220-229 / 244-247 winCreateFromScript.
    pub fn create_win_lose_window(&self, layout_filename: &str) {
        let handler = {
            let mut inner = self.lock_inner_mut();
            if inner.suppress_new_windows {
                return;
            }
            inner.win_lose_window_layout = Some(layout_filename.to_string());
            inner.action_handler.clone()
        };
        if let Some(handler) = handler {
            let _ = handler.create_win_lose_window(layout_filename);
        }
    }

    pub fn create_win_lose_window_with_driver(
        &self,
        layout_filename: &str,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> GameLogicResult<()> {
        {
            let mut inner = self.lock_inner_mut();
            if inner.suppress_new_windows {
                return Ok(());
            }
            inner.win_lose_window_layout = Some(layout_filename.to_owned());
        }
        if let Some(result) = driver.create_win_lose_window(layout_filename) {
            return result;
        }
        let handler = self.with_inner(|inner| inner.action_handler.clone());
        if let Some(handler) = handler {
            handler.create_win_lose_window(layout_filename)
        } else {
            Err(crate::GameLogicError::Configuration(
                "script window creation requires receiving UI or an explicit headless driver"
                    .into(),
            ))
        }
    }

    pub fn current_win_lose_window(&self) -> Option<String> {
        self.with_inner(|inner| inner.win_lose_window_layout.clone())
    }

    pub fn is_suppress_new_windows(&self) -> bool {
        self.with_inner(|inner| inner.suppress_new_windows)
    }

    pub fn should_show_observer_quit_window(&self) -> bool {
        let __player_list_arc = crate::player::player_list();
            let local_is_observer = __player_list_arc.read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|player| player.read().ok().map(|guard| guard.is_player_observer()))
            .unwrap_or(false);
        local_is_observer || self.has_shown_mp_local_defeat_window()
    }

    pub fn should_show_local_defeat_window(&self) -> bool {
        let __player_list_arc = crate::player::player_list();
            let local_is_observer = __player_list_arc.read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|player| player.read().ok().map(|guard| guard.is_player_observer()))
            .unwrap_or(false);
        !local_is_observer
    }

    /// Set whether the multiplayer local defeat window has been shown.
    pub fn set_shown_mp_local_defeat_window(&self, shown: bool) {
        let mut inner = self.lock_inner_mut();
        inner.shown_mp_local_defeat_window = shown;
    }

    /// Return whether the multiplayer local defeat window has been shown.
    pub fn has_shown_mp_local_defeat_window(&self) -> bool {
        self.with_inner(|inner| inner.shown_mp_local_defeat_window)
    }

    /// C++ ScriptActions.cpp:174/208/232/250 TheCampaignManager->SetVictorious.
    pub fn set_campaign_victorious(&self, victorious: bool) {
        let handler = {
            let mut inner = self.lock_inner_mut();
            inner.campaign_victorious = victorious;
            inner.action_handler.clone()
        };
        if let Some(handler) = handler {
            let _ = handler.set_campaign_victorious(victorious);
        }
    }

    /// Live campaign victory flag last set by doVictory/doDefeat.
    pub fn is_campaign_victorious(&self) -> bool {
        self.with_inner(|inner| inner.campaign_victorious)
    }

    /// C++ ScriptEngine::getQualifiedTriggerAreaByName (ScriptEngine.cpp:5888-5926).
    pub fn get_qualified_trigger_area_by_name(
        &self,
        area_name: &str,
    ) -> Option<crate::polygon_trigger::PolygonTrigger> {
        qualify_trigger_area_name(area_name, self.get_current_player_name().as_deref()).and_then(
            |resolved| {
                crate::terrain::get_terrain_logic()
                    .read()
                    .ok()
                    .and_then(|terrain| terrain.get_trigger_area_by_name(&resolved).cloned())
            },
        )
    }

    /// Check if game is ending
    pub fn is_game_ending(&self) -> bool {
        self.runtime.frame().mission_timers.end_game_active()
    }

    /// Freeze time
    pub fn do_freeze_time(&self) {
        let mut inner = self.lock_inner_mut();
        inner.freeze_by_script = true;
        log::info!("Time frozen by script");
    }

    /// Unfreeze time
    pub fn do_unfreeze_time(&self) {
        let mut inner = self.lock_inner_mut();
        inner.freeze_by_script = false;
        log::info!("Time unfrozen by script");
    }

    /// Check if time is frozen by script
    pub fn is_time_frozen_script(&self) -> bool {
        self.with_inner(|inner| inner.freeze_by_script)
    }

    /// Set debug freeze state.
    pub fn set_time_frozen_debug(&mut self, frozen: bool) {
        self.runtime.frame_mut().freeze_by_debug = frozen;
    }

    /// Check if time is frozen by debug controls.
    pub fn is_time_frozen_debug(&self) -> bool {
        self.runtime.frame().freeze_by_debug
    }

    /// Check if time is frozen by any mechanism (script or debug).
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3603-3604
    /// C++: `freezeTime = TheTacticalView->isTimeFrozen() ||
    ///        TheScriptEngine->isTimeFrozenDebug() ||
    ///        TheScriptEngine->isTimeFrozenScript();`
    pub fn is_time_frozen(&self) -> bool {
        self.is_time_frozen_debug() || self.is_time_frozen_script()
    }

    /// Get breeze info (owned snapshot — never a borrow into `UnsafeCell`).
    pub fn get_breeze_info(&self) -> BreezeInfo {
        self.with_inner(|inner| inner.breeze_info.clone())
    }

    /// Turn off breeze
    pub fn turn_breeze_off(&mut self) {
        let inner = self.inner.get_mut();
        inner.breeze_info.intensity = 0.0;
    }

    /// Mirrors C++ ScriptEngine::setSway.
    pub fn set_breeze_info(
        &self,
        direction: f32,
        intensity: f32,
        lean: f32,
        breeze_period: i32,
        randomness: f32,
    ) {
        let mut inner = self.lock_inner_mut();
        inner.breeze_info.breeze_version = inner.breeze_info.breeze_version.wrapping_add(1);
        inner.breeze_info.direction = direction;
        inner.breeze_info.direction_vec[0] = direction.sin();
        inner.breeze_info.direction_vec[1] = direction.cos();
        inner.breeze_info.intensity = intensity;
        inner.breeze_info.lean = lean;
        inner.breeze_info.breeze_period = breeze_period.max(1).clamp(1, i16::MAX as i32) as i16;
        inner.breeze_info.randomness = randomness;
    }

    /// Mirrors C++ ScriptEngine::setFade.
    pub fn set_fade_parameters(
        &self,
        fade: TFade,
        min_fade: f32,
        max_fade: f32,
        fade_frames_increase: i32,
        fade_frames_hold: i32,
        fade_frames_decrease: i32,
    ) {
        let mut frame = self.runtime.frame_mut();
        frame.fade = ScriptFade {
            mode: fade,
            min: min_fade,
            max: max_fade,
            value: min_fade,
            frame: 0,
            increase: fade_frames_increase,
            hold: fade_frames_hold,
            decrease: fade_frames_decrease,
        };
        if fade_frames_increase == 0 {
            frame.fade.advance();
        }
    }

    /// Get fade type
    pub fn get_fade(&self) -> TFade {
        self.runtime.frame().fade.mode
    }

    /// Completed fade observation; reading never advances the script clock.
    pub fn fade_observation(&self) -> (TFade, f32) {
        let frame = self.runtime.frame();
        (frame.fade.mode, frame.fade.value)
    }

    /// Get fade value
    pub fn get_fade_value(&self) -> f32 {
        self.runtime.frame().fade.value
    }

    /// Get current track name (owned snapshot — never a borrow into `UnsafeCell`).
    pub fn get_current_track_name(&self) -> String {
        self.with_inner(|inner| inner.current_track_name.clone())
    }

    /// Set current track name
    pub fn set_current_track_name(&self, name: String) {
        let mut inner = self.lock_inner_mut();
        inner.current_track_name = name;
    }

    pub fn set_global_difficulty(&self, difficulty: crate::player::GameDifficulty) {
        let mut inner = self.lock_inner_mut();
        inner.game_difficulty = difficulty;
    }

    pub fn get_global_difficulty(&self) -> crate::player::GameDifficulty {
        self.with_inner(|inner| inner.game_difficulty)
    }

    pub fn set_objects_should_receive_difficulty_bonus(&self, enable: bool) {
        // ScriptActions::doEnableOrDisableObjectDifficultyBonuses applies the
        // value to every object first, then records the script-engine state.
        // Keep the inner borrow out of that object walk so an update callback
        // cannot conflict with immediate script re-entry.
        if !dual_world_registry_unavailable() {
            for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
                let obj = match OBJECT_REGISTRY.get_object(obj_id) {
                    Some(v) => v,
                    None => continue,
                };
                let mut guard = match obj.write() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if true {
                    guard.set_receiving_difficulty_bonus(enable);
                }
            }
        }

        let mut inner = self.lock_inner_mut();
        inner.objects_should_receive_difficulty_bonus = enable;
    }

    /// C++ `getObjectsShouldReceiveDifficultyBonus` counterpart.
    pub fn get_objects_should_receive_difficulty_bonus(&self) -> bool {
        self.with_inner(|inner| inner.objects_should_receive_difficulty_bonus)
    }

    pub fn set_choose_victim_always_uses_normal(&self, enable: bool) {
        let mut inner = self.lock_inner_mut();
        inner.choose_victim_always_uses_normal = enable;
    }

    pub fn get_choose_victim_always_uses_normal(&self) -> bool {
        self.with_inner(|inner| inner.choose_victim_always_uses_normal)
    }

    /// Freeze only definition keys for one script read/write operation, in C++ enum order.
    pub fn script_template_lookup(
        &self,
    ) -> generals_sim::scripting::program::ScriptTemplateLookup {
        self.with_inner(|inner| {
            generals_sim::scripting::program::ScriptTemplateLookup::from_keys(
                inner
                    .templates
                    .conditions()
                    .iter()
                    .map(|t| t.base.internal_name_key)
                    .collect(),
                inner
                    .templates
                    .actions()
                    .iter()
                    .map(|t| t.base.internal_name_key)
                    .collect(),
            )
        })
    }

    /// Clone an action template from this interpreter's catalog.
    pub fn get_action_template(&self, index: usize) -> Option<ActionTemplate> {
        self.with_inner(|inner| inner.templates.actions().get(index).cloned())
    }

    /// Clone a condition template from this interpreter's catalog.
    pub fn get_condition_template(&self, index: usize) -> Option<ConditionTemplate> {
        self.with_inner(|inner| inner.templates.conditions().get(index).cloned())
    }

    pub fn find_condition_type_by_name_key(&self, name_key: u32) -> Option<ConditionType> {
        self.with_inner(|inner| {
            inner
                .templates
                .conditions()
                .iter()
                .enumerate()
                .find_map(|(idx, template)| {
                    if template.base.internal_name_key == name_key {
                        ConditionType::from_u32(idx as u32)
                    } else {
                        None
                    }
                })
        })
    }

    pub fn find_action_type_by_name_key(&self, name_key: u32) -> Option<ScriptActionType> {
        self.with_inner(|inner| {
            inner
                .templates
                .actions()
                .iter()
                .enumerate()
                .find_map(|(idx, template)| {
                    if template.base.internal_name_key == name_key {
                        ScriptActionType::from_u32(idx as u32)
                    } else {
                        None
                    }
                })
        })
    }

    pub fn append_sequential_script(&self, script: SequentialScript) {
        self.runtime.sequential().append(script);
    }

    pub fn remove_all_sequential_scripts_for_object(&self, object_id: ObjectID) {
        self.runtime.sequential().remove_object(object_id);
    }

    pub fn has_active_sequential_script_for_object(&self, object_id: ObjectID) -> bool {
        self.runtime.sequential().has_object(object_id)
    }

    pub fn has_active_sequential_script_for_team(&self, team_id: TeamID) -> bool {
        self.runtime.sequential().has_team(team_id)
    }

    pub fn remove_all_sequential_scripts_for_team(&self, team_id: TeamID) {
        self.runtime.remove_team_sequences(team_id);
    }

    pub fn set_sequential_timer_for_object(&mut self, object_id: ObjectID, frames: i32) {
        self.runtime.sequential().wait_for_object(object_id, frames);
    }

    pub fn set_sequential_timer_for_team(&mut self, team_id: TeamID, frames: i32) {
        self.runtime.sequential().wait_for_team(team_id, frames);
    }

    /// Signal UI interaction
    pub fn signal_ui_interact(&self, hook_name: &str) {
        self.runtime.frame_mut().ui_interactions.push(hook_name.to_string());
        log::debug!("UI interaction: {}", hook_name);
    }

    /// C++ `evaluateFlag` walks `m_uiInteractions` after the stored flag miss.
    /// `signalUIInteract` pulses the hook name true for one `ScriptEngine::update`.
    pub fn has_ui_interaction(&self, hook_name: &str) -> bool {
        self.runtime.frame().ui_interactions.iter().any(|name| name == hook_name)
    }

    pub fn create_named_map_reveal(
        &self,
        reveal_name: &str,
        waypoint_name: &str,
        radius: f32,
        player_name: &str,
    ) {
        if !self.lock_inner_mut().named_reveals.register(reveal_name, waypoint_name, radius, player_name) {
            log::warn!("Attempted to redefine named reveal '{reveal_name}'");
            return;
        }
        log::debug!("Created named map reveal: {reveal_name}");
    }

    pub fn do_named_map_reveal(&self, reveal_name: &str) {
        if let Some(reveal) = self.with_inner(|inner| inner.named_reveals.find(reveal_name).cloned()) {
            self.apply_named_reveal_in_core(NamedRevealOperation::Reveal, &reveal);
        }
    }

    pub fn undo_named_map_reveal(&self, reveal_name: &str) {
        if let Some(reveal) = self.with_inner(|inner| inner.named_reveals.find(reveal_name).cloned()) {
            self.apply_named_reveal_in_core(NamedRevealOperation::Undo, &reveal);
        }
    }

    pub(crate) fn apply_named_reveal_with_driver(
        &self,
        operation: NamedRevealOperation,
        name: &str,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> GameLogicResult<()> {
        let Some(reveal) = self.with_inner(|inner| inner.named_reveals.find(name).cloned()) else {
            return Ok(());
        };
        if let Some(result) = driver.named_map_visibility(
            operation,
            &reveal,
            self.get_current_player_name().as_deref(),
        ) {
            return result;
        }
        self.apply_named_reveal_in_core(operation, &reveal);
        Ok(())
    }

    fn apply_named_reveal_in_core(&self, operation: NamedRevealOperation, reveal: &NamedReveal) {
        let Some(world) = self.owner_world.upgrade() else {
            return;
        };
        let waypoint = AsciiString::from(reveal.waypoint_name.as_str());
        let position = world.terrain().read().ok().and_then(|terrain| {
            terrain.get_waypoint_by_name(&waypoint).map(|waypoint| *waypoint.get_location())
        });
        let Some(position) = position else {
            return;
        };
        let Ok(mut names) = world.common().name_key_namespace.lock() else {
            return;
        };
        let key = names.name_to_key(&reveal.player_name);
        drop(names);
        let Ok(players) = self.players.read() else {
            return;
        };
        let mask = players.iter().find_map(|player| {
            let player = player.read().ok()?;
            (player.get_player_name_key() == key).then(|| player.get_player_mask().bits())
        });
        let Some(mask) = mask else {
            return;
        };
        drop(players);
        if let Ok(mut shroud) = world.shroud().lock() {
            match operation {
                NamedRevealOperation::Reveal => shroud.do_shroud_reveal(&position, reveal.radius_to_reveal, mask),
                NamedRevealOperation::Undo => shroud.undo_shroud_reveal(&position, reveal.radius_to_reveal, mask),
            }
        }
    }

    pub fn remove_named_map_reveal(&self, reveal_name: &str) {
        self.lock_inner_mut().named_reveals.remove(reveal_name);
    }

    pub fn snapshot_named_reveals(&self) -> Vec<(String, String, f32, String)> {
        self.with_inner(|inner| inner.named_reveals.snapshot())
    }

    pub fn restore_named_reveals(&self, reveals: &[(String, String, f32, String)]) {
        self.lock_inner_mut().named_reveals.restore(reveals);
    }

    /// Recover lookers only for a historical archive without the shroud grid.
    pub fn reapply_named_reveals_with_driver(
        &self,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> GameLogicResult<()> {
        let names: Vec<String> = self.with_inner(|inner| {
            inner.named_reveals.entries().iter().map(|entry| entry.reveal_name.clone()).collect()
        });
        for name in names {
            self.apply_named_reveal_with_driver(NamedRevealOperation::Reveal, &name, driver)?;
        }
        Ok(())
    }

    /// Set or clear a named topple direction for scripted objects.
    /// Matches C++ ScriptEngine::setToppleDirection.
    pub fn set_topple_direction(
        &self,
        object_name: &str,
        direction: Option<crate::common::Coord3D>,
    ) {
        let mut inner = self.lock_inner_mut();
        if object_name.is_empty() {
            return;
        }

        if let Some(index) = inner
            .topple_directions
            .iter()
            .position(|(name, _)| name == object_name)
        {
            if let Some(dir) = direction {
                inner.topple_directions[index].1 = Coord3D::new(dir.x, dir.y, dir.z);
            } else {
                inner.topple_directions.remove(index);
            }
            return;
        }

        if let Some(dir) = direction {
            inner.topple_directions.insert(
                0,
                (object_name.to_string(), Coord3D::new(dir.x, dir.y, dir.z)),
            );
        }
    }

    /// Adjust a topple direction based on script overrides.
    /// Matches C++ ScriptEngine::adjustToppleDirection.
    pub fn adjust_topple_direction(
        &self,
        object: &crate::object::Object,
        direction: &mut crate::common::Coord3D,
    ) {
        self.with_inner(|inner| {
            let name = object.get_name();
            if name.is_empty() {
                return;
            }

            for (entry_name, entry_direction) in &inner.topple_directions {
                if entry_name == name.as_str() {
                    let mut new_dir = crate::common::Coord3D::new(
                        entry_direction.x,
                        entry_direction.y,
                        entry_direction.z,
                    );
                    if new_dir.length_squared() > 0.0 {
                        new_dir = new_dir.normalize();
                    }
                    *direction = new_dir;
                    return;
                }
            }
        })
    }

    /// Name-keyed lookup for live host `begin_structure_topple`.
    pub fn topple_direction_for_name(&self, object_name: &str) -> Option<crate::common::Coord3D> {
        if object_name.is_empty() {
            return None;
        }
        self.with_inner(|inner| {
            inner
                .topple_directions
                .iter()
                .find(|(name, _)| name == object_name)
                .map(|(_, dir)| {
                    let mut new_dir = crate::common::Coord3D::new(dir.x, dir.y, dir.z);
                    if new_dir.length_squared() > 0.0 {
                        new_dir = new_dir.normalize();
                    }
                    new_dir
                })
        })
    }

    /// Get statistics string
    #[cfg(feature = "script_profiling")]
    pub fn get_stats(&self) -> String {
        self.with_inner(|inner| {
        let avg_time = if inner.stats.num_frames > 0.0 {
            inner.stats.total_update_time / inner.stats.num_frames
        } else {
            0.0
        };

        format!(
            "ScriptEngine Stats: Frames: {:.0}, Total Time: {:.6}s, Avg Time: {:.6}s, Max Time: {:.6}s, Current: {:.6}s",
            inner.stats.num_frames,
            inner.stats.total_update_time,
            avg_time,
            inner.stats.max_update_time,
            inner.stats.cur_update_time
        )
            })
    }

    /// Get statistics (no profiling version)
    #[cfg(not(feature = "script_profiling"))]
    pub fn get_stats(&self) -> String {
        "ScriptEngine Stats: Profiling disabled".to_string()
    }

    pub fn set_action_handler(&mut self, handler: Option<Arc<dyn ScriptActionHandler>>) {
        let inner = self.inner.get_mut();
        inner.action_handler = handler;
    }

    pub fn action_handler(&self) -> Option<Arc<dyn ScriptActionHandler>> {
        self.with_inner(|inner| inner.action_handler.clone())
    }
    // =========================================================================
    // MISSING METHODS PORTED FROM C++ ScriptEngine
    // =========================================================================

    // PARITY_NOTE: C++ ScriptEngine::notifyOfObjectDestruction
    pub fn notify_of_object_destruction(&mut self, object_id: ObjectID) {
        let tracker = get_named_object_tracker();
        self.notify_of_object_destruction_in(object_id, &tracker);
    }

    pub(crate) fn notify_of_object_destruction_in(
        &mut self,
        object_id: ObjectID,
        tracker: &crate::scripting::events::NamedObjectTracker,
    ) {
        let name = tracker.get_object_name(object_id).ok().flatten();
        if let Some(name) = name {
            if !name.is_empty() {
                let _ = tracker.unregister_object(object_id);
            }
        }

        self.clear_destroyed_object_references(object_id);
    }

    pub(crate) fn notify_of_object_destruction_owned(
        &mut self,
        object_id: ObjectID,
        names: &mut crate::scripting::events::NamedObjectTrackerState,
    ) {
        if names.name(object_id).is_some_and(|name| !name.is_empty()) {
            names.remove(object_id);
        }
        self.clear_destroyed_object_references(object_id);
    }

    /// ScriptConditions.cpp1945: first observation records, a previous-frame
    /// nonempty→empty edge remains visible to every query in the current frame.
    pub fn observe_receiving_transport_empty(
        &self,
        object: ObjectID,
        frame: u32,
        count: usize,
    ) -> bool {
        self.try_observe_receiving_transport_empty(object, frame, count)
            .expect("transport history cannot be observed while engine state is borrowed")
    }

    pub(crate) fn try_observe_receiving_transport_empty(
        &self,
        object: ObjectID,
        frame: u32,
        count: usize,
    ) -> Option<bool> {
        let mut inner = self.inner.try_borrow_mut().ok()?;
        let status = inner
            .receiving_transport_statuses
            .entry(object)
            .or_insert((frame, count));
        if status.0 == frame.wrapping_sub(1) && status.1 > 0 && count == 0 {
            return Some(true);
        }
        *status = (frame, count);
        Some(false)
    }

    /// Receiving cache ownership is external to this interpreter. Preserve
    /// only ScriptEngine's original calling/condition pointer cleanup here.
    pub fn notify_receiving_object_destruction(&self, object_id: ObjectID) {
        self.clear_destroyed_object_references(object_id);
    }

    fn clear_destroyed_object_references(&self, object_id: ObjectID) {
        self.runtime.clear_destroyed_object(object_id);
    }

    // PARITY_NOTE: C++ ScriptEngine::notifyOfTeamDestruction
    pub fn notify_of_team_destruction(&self, team_id: TeamID) {
        if team_id == TEAM_ID_INVALID {
            return;
        }
        self.remove_all_sequential_scripts_for_team(team_id);
    }

    // PARITY_NOTE: C++ ScriptEngine::forceUnfreezeTime
    pub fn force_unfreeze_time(&self) {
        // Original calls the debug control's ForceAppContinue. Script and
        // tactical-camera freeze are separate owners and remain unchanged.
        self.runtime.frame_mut().freeze_by_debug = false;
    }

    // PARITY_NOTE: C++ ScriptEngine::clearFlag
    pub fn clear_flag(&self, name: &str) {
        self.runtime.frame_mut().variables
            .clear_flag(name, Self::MAX_PLAYER_COUNT);
    }

    // PARITY_NOTE: C++ ScriptEngine::clearTeamFlags
    pub fn clear_team_flags(&self) {
        self.clear_flag("USA Team is Building");
        self.clear_flag("USA Air Team Is Building");
        self.clear_flag("USA Inf Team Is Building");
        self.clear_flag("China Team is Building");
        self.clear_flag("China Air Team Is Building");
        self.clear_flag("China Inf Team Is Building");
        self.clear_flag("GLA Team is Building");
        self.clear_flag("GLA Inf Team Is Building");
    }

    // PARITY_NOTE: C++ ScriptEngine::didUnitExist
    pub fn did_unit_exist(&self, unit_name: &str) -> bool {
        let tracker = get_named_object_tracker();
        tracker.did_object_exist(unit_name).unwrap_or(false)
    }

    /// C++ ScriptEngine::runScript, using this interpreter's team and clock.
    pub fn run_script(&mut self, script_name: &str, team_id: Option<TeamID>) {
        if script_name.is_empty() || script_name == "<none>" {
            return;
        }
        let frame = self.owning_world_services().map_or(0, |world| world.logic_frame());
        self.run_standalone_team_script_at_frame(script_name, team_id, frame);
    }

    pub(crate) fn run_standalone_team_script_at_frame(
        &self,
        script_name: &str,
        team_id: Option<TeamID>,
        frame: u32,
    ) {
        if script_name.is_empty() || script_name == "<none>" {
            return;
        }
        let controller = team_id.and_then(|id| self.standalone_team_player(id));
        let _ = self.run_team_subroutine(
            script_name,
            team_id,
            controller.as_deref(),
            crate::scripting::executor::ScriptContext::at_frame(frame),
            &mut CanonicalScriptExecutionDriver,
        );
    }

    /// CPP runScript with a controller name supplied by the driving session.
    pub fn run_team_script_with_driver(
        &self,
        script_name: &str,
        team_id: TeamID,
        player_name: Option<&str>,
        context: crate::scripting::executor::ScriptContext,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> GameLogicResult<bool> {
        self.run_team_subroutine(script_name, Some(team_id), player_name, context, driver)
    }

    fn run_team_subroutine(
        &self,
        script_name: &str,
        team_id: Option<TeamID>,
        player_name: Option<&str>,
        context: crate::scripting::executor::ScriptContext,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> GameLogicResult<bool> {
        if script_name.is_empty() || script_name == "<none>" {
            return Ok(false);
        }
        self.runtime.set_condition_team(None); // C++ runScript leaves it cleared.
        let _restore = self.runtime.enter_team_call(team_id, player_name.map(str::to_owned));
        self.execute_subroutine_by_name_with_driver(script_name, context, driver)
    }

    // PARITY_NOTE: C++ ScriptEngine::runObjectScript
    pub fn run_object_script(&mut self, script_name: &str, object_id: ObjectID) {
        if script_name.is_empty() || script_name == "<none>" {
            return;
        }

        let _restore = self.runtime.enter_object_call(object_id);
        let _ = self.execute_subroutine_by_name(script_name);
    }

    /// Run an attached object's subroutine against the explicit driving owner.
    /// Calling-object identity remains visible across immediate nested effects.
    pub fn run_object_script_with_driver(
        &self,
        script_name: &str,
        object_id: ObjectID,
        context: crate::scripting::executor::ScriptContext,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> GameLogicResult<bool> {
        if script_name.is_empty() || script_name == "<none>" {
            return Ok(false);
        }
        let _restore = self.runtime.enter_object_call(object_id);
        self.execute_subroutine_by_name_with_driver(script_name, context, driver)
    }

    // PARITY_NOTE: C++ ScriptEngine::evaluateConditions
    pub fn evaluate_conditions(
        &mut self,
        script: &mut Script,
        team_id: Option<TeamID>,
        player_name: Option<&str>,
        current_frame: u32,
    ) -> bool {
        let _restore = self.runtime.preserve_team_call();
        let player = team_id
            .and_then(|id| self.standalone_team_player(id))
            .or_else(|| player_name.map(str::to_owned))
            .or_else(|| self.get_current_player_name());
        self.runtime.set_team_call(team_id, player);

        // C++ ScriptEngine::evaluateConditions walks OR/AND then evaluateCondition.
        // evaluateCondition handles TRUE/FALSE/COUNTER/FLAG/TIMER locally and
        // forwards every other type to TheScriptConditions (full switch).
        // Leftover ScriptConditionEvaluator::evaluate_or_condition is that switch
        // (PLAYER_HAS_CREDITS and the rest). with_active so leftover
        // with_script_engine_ref does not re-lock the global handle while team
        // production already holds write().
        let result = if let Some(or_cond) = script.condition.as_deref_mut() {
            self.with_active(|| {
                let state = std::cell::RefCell::new(
                    crate::scripting::executor::ScriptContext::at_frame(current_frame),
                );
                let mut leftover =
                    crate::scripting::executor::ScriptConditionEvaluator::new(self, &state);
                leftover.evaluate_or_condition(or_cond).unwrap_or(false)
            })
        } else {
            false
        };

        result
    }

    // PARITY_NOTE: C++ ScriptEngine::removeSequentialScript (empty body in C++)
    pub fn remove_sequential_script(&mut self, _script: &SequentialScript) {}

    // PARITY_NOTE: C++ ScriptEngine::adjustTimer
    pub fn adjust_timer(
        &mut self,
        counter_name: &str,
        value: i32,
        millisecond_timer: bool,
        add: bool,
    ) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables
            .adjust_timer(counter_name, value, millisecond_timer, add)
    }

    // PARITY_NOTE: C++ ScriptEngine::getStats
    pub fn get_stats_detailed(&self) -> (String, f32, f32, f32) {
        #[cfg(feature = "script_profiling")]
        {
            let stats = self.get_stats();
            let cur = self.with_inner(|inner| inner.stats.cur_update_time);
            (stats, cur, 0.0, 0.0)
        }
        #[cfg(not(feature = "script_profiling"))]
        {
            (
                "Script Engine Profiling disabled.".to_string(),
                0.0,
                0.0,
                0.0,
            )
        }
    }

    // PARITY_NOTE: C++ ScriptEngine::addObjectToCache
    pub fn add_object_to_cache(&mut self, object_id: ObjectID) {
        // Wave 348: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let Some(name) = OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                let name = obj.get_name();
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                }
            })
            .flatten()
        else {
            return;
        };
        let tracker = get_named_object_tracker();
        let _ = tracker.register_named_object(name, object_id);
    }

    // PARITY_NOTE: C++ ScriptEngine::removeObjectFromCache
    pub fn remove_object_from_cache(&mut self, object_id: ObjectID) {
        let tracker = get_named_object_tracker();
        let _ = tracker.unregister_object(object_id);
    }

    // PARITY_NOTE: C++ ScriptEngine::restartTimer (only restarts if value > 0)
    pub fn restart_timer_if_positive(&mut self, name: &str) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables
            .restart_timer_if_positive(name)
    }

    // PARITY_NOTE: C++ ScriptEngine::setTimer with random/msec params
    pub fn set_timer_with_params(
        &mut self,
        name: &str,
        value: f32,
        millisecond_timer: bool,
        random: bool,
        random_max: Option<f32>,
    ) -> GameLogicResult<()> {
        self.runtime.frame_mut().variables
            .set_timer_from(name, millisecond_timer, || {
                if random {
                    let max = random_max.unwrap_or(value);
                    crate::helpers::get_game_logic_random_value_real(value.min(max), value.max(max))
                } else {
                    value
                }
            })
    }

    // PARITY_NOTE: C++ always returns FALSE (no case in switch)
    pub fn has_unit_completed_sequential_script(
        &self,
        _object: ObjectID,
        _script_name: &str,
    ) -> bool {
        false
    }

    // PARITY_NOTE: C++ always returns FALSE (no case in switch)
    pub fn has_team_completed_sequential_script(
        &self,
        _team_name: &str,
        _script_name: &str,
    ) -> bool {
        false
    }

    /// Compatibility spelling for callers retained during the port.
    pub fn start_end_game_timer_cxx(&self) {
        self.start_end_game_timer();
    }

    /// Compatibility spelling for callers retained during the port.
    pub fn start_close_window_timer_cxx(&self) {
        self.start_close_window_timer();
    }

    /// Compatibility spelling for callers retained during the port.
    pub fn start_quick_end_game_timer_cxx(&self) {
        self.start_quick_end_game_timer();
    }

    /// C++ `ScriptEngine::xfer` tail after sequential/counters/flags.
    pub fn snapshot_xfer_tail(&self) -> ScriptEngineXferTail {
        let named_objects = self.named_objects
            .get_all_named_objects()
            .unwrap_or_default();
        self.snapshot_xfer_tail_with_named_objects(named_objects)
    }

    pub fn snapshot_xfer_tail_with_named_objects(
        &self,
        named_objects: Vec<(String, u32)>,
    ) -> ScriptEngineXferTail {
        let events = self.runtime.events().snapshot();
        let frame = self.runtime.frame();
        let priorities = self.runtime.attack_priorities();
        self.with_inner(|inner| {
            let attack_priorities = priorities
                .entries()
                .iter()
                .map(|info| {
                    let mut entries: Vec<(String, i32)> = info
                        .priority_map
                        .iter()
                        .map(|(name, priority)| (name.clone(), *priority))
                        .collect();
                    entries.sort_by(|a, b| a.0.cmp(&b.0));
                    (info.name.clone(), info.default_priority, entries)
                })
                .collect();
            let object_attack_priority_sets = priorities
                .assignments()
                .map(|(id, name)| (id, name.to_owned()))
                .collect();
            let mut object_types: Vec<(String, Vec<String>)> = inner
                .object_types
                .iter()
                .map(|(name, list)| {
                    (
                        name.clone(),
                        list.iter()
                            .map(|entry| entry.as_str().to_string())
                            .collect(),
                    )
                })
                .collect();
            object_types.sort_by(|a, b| a.0.cmp(&b.0));
            ScriptEngineXferTail {
                attack_priorities,
                object_attack_priority_sets,
                end_game_timer: frame.mission_timers.end_game,
                close_window_timer: frame.mission_timers.close_window,
                named_objects,
                first_update: frame.first_update,
                fade: frame.fade.mode as i32,
                min_fade: frame.fade.min,
                max_fade: frame.fade.max,
                cur_fade_value: frame.fade.value,
                cur_fade_frame: frame.fade.frame,
                fade_frames_increase: frame.fade.increase,
                fade_frames_hold: frame.fade.hold,
                fade_frames_decrease: frame.fade.decrease,
                completed_video: events.completed_video,
                testing_speech: events.testing_speech,
                testing_audio: events.testing_audio,
                ui_interactions: frame.ui_interactions.clone(),
                triggered_special_powers: events.triggered_special_powers,
                midway_special_powers: events.midway_special_powers,
                finished_special_powers: events.finished_special_powers,
                completed_upgrades: events.completed_upgrades,
                acquired_sciences: events.acquired_sciences,
                topple_directions: inner
                    .topple_directions
                    .iter()
                    .map(|(name, coord)| (name.clone(), [coord.x, coord.y, coord.z]))
                    .collect(),
                breeze_direction: inner.breeze_info.direction,
                breeze_direction_vec: inner.breeze_info.direction_vec,
                breeze_intensity: inner.breeze_info.intensity,
                breeze_lean: inner.breeze_info.lean,
                breeze_randomness: inner.breeze_info.randomness,
                breeze_period: inner.breeze_info.breeze_period,
                breeze_version: inner.breeze_info.breeze_version,
                game_difficulty: match inner.game_difficulty {
                    crate::player::GameDifficulty::Easy => 0,
                    crate::player::GameDifficulty::Normal => 1,
                    crate::player::GameDifficulty::Hard => 2,
                    crate::player::GameDifficulty::Brutal => 3,
                },
                freeze_by_script: inner.freeze_by_script,
                object_types,
                objects_should_receive_difficulty_bonus: inner
                    .objects_should_receive_difficulty_bonus,
                current_track_name: inner.current_track_name.clone(),
                choose_victim_always_uses_normal: inner.choose_victim_always_uses_normal,
            }
        })
    }

    /// Restore the leftover ScriptEngine xfer tail into the live engine.
    pub fn restore_xfer_tail(&self, tail: &ScriptEngineXferTail) {
        self.restore_xfer_tail_with_named_tracker(tail, true)
    }
    /// Supported historical Rust loads without a ScriptEngine continuation
    /// enter the original reset/first-update name phase. Transport observations
    /// have no Xfer payload; they cannot survive a load into the same owner.
    pub fn reset_receiving_named_state_for_legacy_load(&self) {
        let mut inner = self.lock_inner_mut();
        inner.receiving_transport_statuses.clear();
        self.runtime.frame_mut().first_update = true;
    }

    /// Main owns the ordered name cache; restoring its interpreter never
    /// clears or fills the standalone Core tracker.
    pub fn restore_receiving_xfer_tail(&self, tail: &ScriptEngineXferTail) {
        self.restore_xfer_tail_with_named_tracker(tail, false)
    }
    fn restore_xfer_tail_with_named_tracker(&self, tail: &ScriptEngineXferTail, standalone: bool) {
        self.runtime.events().restore(ScriptEventHistory {
            completed_video: tail.completed_video.clone(),
            testing_speech: tail.testing_speech.clone(),
            testing_audio: tail.testing_audio.clone(),
            triggered_special_powers: tail.triggered_special_powers.clone(),
            midway_special_powers: tail.midway_special_powers.clone(),
            finished_special_powers: tail.finished_special_powers.clone(),
            completed_upgrades: tail.completed_upgrades.clone(),
            acquired_sciences: tail.acquired_sciences.clone(),
        });
        {
            let mut frame = self.runtime.frame_mut();
            let mut priorities = self.runtime.attack_priorities_mut();
            let mut inner = self.lock_inner_mut();
            // Original ScriptConditions reset precedes load; this observation
            // history has no Xfer payload and must not retain the donor run.
            inner.receiving_transport_statuses.clear();
            priorities.restore(
                tail.attack_priorities
                    .iter()
                    .map(|(name, default_priority, entries)| {
                        let mut info = AttackPriorityInfo::new();
                        info.name = name.clone();
                        info.default_priority = *default_priority;
                        info.priority_map = entries.iter().cloned().collect();
                        info
                    })
                    .collect(),
            );
            priorities.clear_assignments();
            for (id, name) in &tail.object_attack_priority_sets {
                if !name.is_empty() {
                    priorities.assign(*id, name);
                }
            }
            frame.mission_timers.end_game = tail.end_game_timer;
            frame.mission_timers.close_window = tail.close_window_timer;
            frame.first_update = tail.first_update;
            frame.fade.mode = match tail.fade {
                1 => TFade::Subtract,
                2 => TFade::Add,
                3 => TFade::Saturate,
                4 => TFade::Multiply,
                _ => TFade::None,
            };
            frame.fade.min = tail.min_fade;
            frame.fade.max = tail.max_fade;
            frame.fade.value = tail.cur_fade_value;
            frame.fade.frame = tail.cur_fade_frame;
            frame.fade.increase = tail.fade_frames_increase;
            frame.fade.hold = tail.fade_frames_hold;
            frame.fade.decrease = tail.fade_frames_decrease;
            frame.ui_interactions = tail.ui_interactions.clone();
            inner.topple_directions = tail
                .topple_directions
                .iter()
                .map(|(name, xyz)| (name.clone(), Coord3D::new(xyz[0], xyz[1], xyz[2])))
                .collect();
            inner.breeze_info.direction = tail.breeze_direction;
            inner.breeze_info.direction_vec = tail.breeze_direction_vec;
            inner.breeze_info.intensity = tail.breeze_intensity;
            inner.breeze_info.lean = tail.breeze_lean;
            inner.breeze_info.randomness = tail.breeze_randomness;
            inner.breeze_info.breeze_period = tail.breeze_period;
            inner.breeze_info.breeze_version = tail.breeze_version;
            inner.game_difficulty = match tail.game_difficulty {
                0 => crate::player::GameDifficulty::Easy,
                2 => crate::player::GameDifficulty::Hard,
                3 => crate::player::GameDifficulty::Brutal,
                _ => crate::player::GameDifficulty::Normal,
            };
            inner.freeze_by_script = tail.freeze_by_script;
            inner.object_types.clear();
            for (list_name, types) in &tail.object_types {
                let mut list = ObjectTypes::with_list_name(AsciiString::from(list_name.as_str()));
                for object_type in types {
                    list.add_object_type(AsciiString::from(object_type.as_str()));
                }
                inner.object_types.insert(list_name.clone(), list);
            }
            inner.objects_should_receive_difficulty_bonus =
                tail.objects_should_receive_difficulty_bonus;
            inner.current_track_name = tail.current_track_name.clone();
            inner.choose_victim_always_uses_normal = tail.choose_victim_always_uses_normal;
            // hq-r9gaw / C++ ScriptEngine.cpp:9277-9286 — archive LOAD with
            // saved FADE_NONE starts the standard Multiply fade-in intro.
            // Core leftover Xfer already applies this; receiving restore must too.
            if !standalone && matches!(frame.fade.mode, TFade::None) {
                frame.fade = ScriptFade::map_intro();
            }
        }

        if !standalone {
            return;
        }
        let tracker = &self.named_objects;
        let _ = tracker.clear();
        for (name, object_id) in &tail.named_objects {
            if *object_id != crate::common::INVALID_ID && !name.is_empty() {
                let _ = tracker.register_named_object(name.clone(), *object_id);
            }
        }
    }
}

/// C++ `ScriptEngine::xfer` fields after counters/flags (`ScriptEngine.cpp:8812+`).
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptEngineXferTail {
    pub attack_priorities: Vec<(String, i32, Vec<(String, i32)>)>,
    pub object_attack_priority_sets: Vec<(ObjectID, String)>,
    pub end_game_timer: i32,
    pub close_window_timer: i32,
    pub named_objects: Vec<(String, u32)>,
    pub first_update: bool,
    pub fade: i32,
    pub min_fade: f32,
    pub max_fade: f32,
    pub cur_fade_value: f32,
    pub cur_fade_frame: i32,
    pub fade_frames_increase: i32,
    pub fade_frames_hold: i32,
    pub fade_frames_decrease: i32,
    pub completed_video: Vec<String>,
    pub testing_speech: Vec<(String, u32)>,
    pub testing_audio: Vec<(String, u32)>,
    pub ui_interactions: Vec<String>,
    pub triggered_special_powers: Vec<Vec<(String, u32)>>,
    pub midway_special_powers: Vec<Vec<(String, u32)>>,
    pub finished_special_powers: Vec<Vec<(String, u32)>>,
    pub completed_upgrades: Vec<Vec<(String, u32)>>,
    pub acquired_sciences: Vec<Vec<i32>>,
    pub topple_directions: Vec<(String, [f32; 3])>,
    pub breeze_direction: f32,
    pub breeze_direction_vec: [f32; 2],
    pub breeze_intensity: f32,
    pub breeze_lean: f32,
    pub breeze_randomness: f32,
    pub breeze_period: i16,
    pub breeze_version: i16,
    pub game_difficulty: i32,
    pub freeze_by_script: bool,
    pub object_types: Vec<(String, Vec<String>)>,
    pub objects_should_receive_difficulty_bonus: bool,
    pub current_track_name: String,
    pub choose_victim_always_uses_normal: bool,
}

impl Default for ScriptEngineXferTail {
    fn default() -> Self {
        let breeze = BreezeInfo::new();
        Self {
            attack_priorities: Vec::new(),
            object_attack_priority_sets: Vec::new(),
            end_game_timer: -1,
            close_window_timer: -1,
            named_objects: Vec::new(),
            first_update: true,
            fade: TFade::None as i32,
            min_fade: 0.0,
            max_fade: 1.0,
            cur_fade_value: 0.0,
            cur_fade_frame: 0,
            fade_frames_increase: 0,
            fade_frames_hold: 0,
            fade_frames_decrease: 0,
            completed_video: Vec::new(),
            testing_speech: Vec::new(),
            testing_audio: Vec::new(),
            ui_interactions: Vec::new(),
            triggered_special_powers: Vec::new(),
            midway_special_powers: Vec::new(),
            finished_special_powers: Vec::new(),
            completed_upgrades: Vec::new(),
            acquired_sciences: Vec::new(),
            topple_directions: Vec::new(),
            breeze_direction: breeze.direction,
            breeze_direction_vec: breeze.direction_vec,
            breeze_intensity: breeze.intensity,
            breeze_lean: breeze.lean,
            breeze_randomness: breeze.randomness,
            breeze_period: breeze.breeze_period,
            breeze_version: breeze.breeze_version,
            game_difficulty: 1,
            freeze_by_script: false,
            object_types: Vec::new(),
            objects_should_receive_difficulty_bonus: true,
            current_track_name: String::new(),
            choose_victim_always_uses_normal: false,
        }
    }
}

#[cfg(test)]
mod receiving_window_driver_tests {
    use super::*;
    struct Windows<'a> {
        engine: &'a ScriptEngine,
        events: Vec<String>,
    }
    impl ScriptExecutionDriver<ScriptEngine> for Windows<'_> {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn create_win_lose_window(&mut self, layout: &str) -> Option<GameLogicResult<()>> {
            assert_eq!(
                self.engine.current_win_lose_window().as_deref(),
                Some(layout)
            );
            self.events.push(format!("create:{layout}"));
            Some(Ok(()))
        }
        fn destroy_win_lose_window(&mut self) -> Option<GameLogicResult<()>> {
            assert_eq!(self.engine.current_win_lose_window(), None);
            self.events.push("destroy".into());
            Some(Ok(()))
        }
        fn close_game_windows(&mut self) -> Option<GameLogicResult<()>> {
            self.events.push("close-game".into());
            Some(Ok(()))
        }
    }
    #[test]
    fn window_tracking_precedes_callbacks_and_peer_execution_is_isolated() {
        let a = ScriptEngine::new_in_published_world().unwrap();
        let b = ScriptEngine::new_in_published_world().unwrap();
        let mut a_ui = Windows {
            engine: &a,
            events: Vec::new(),
        };
        let mut b_ui = Windows {
            engine: &b,
            events: Vec::new(),
        };
        a.create_win_lose_window_with_driver("a.wnd", &mut a_ui)
            .unwrap();
        b.create_win_lose_window_with_driver("b.wnd", &mut b_ui)
            .unwrap();
        a.close_windows_with_driver(true, &mut a_ui).unwrap();
        a.close_game_windows_with_driver(&mut a_ui).unwrap();
        a.create_win_lose_window_with_driver("suppressed.wnd", &mut a_ui)
            .unwrap();
        assert_eq!(a_ui.events, ["create:a.wnd", "destroy", "close-game"]);
        assert_eq!(a.current_win_lose_window(), None);
        assert_eq!(b_ui.events, ["create:b.wnd"]);
        assert_eq!(b.current_win_lose_window().as_deref(), Some("b.wnd"));
    }
}
