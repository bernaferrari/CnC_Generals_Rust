// Counters, flags, timers, named reveals, and named object trackers
//
// Split from `scripting/engine.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEngine {
    pub fn allocate_counter(&self, name: &str) -> GameLogicResult<usize> {
        let mut inner = self.lock_inner_mut();
        // Check if counter already exists
        for (i, counter) in inner.counters.iter().enumerate() {
            if let Some(counter) = counter {
                if counter.name == name {
                    return Ok(i);
                }
            }
        }

        // Find empty slot
        for i in 0..MAX_COUNTERS {
            if inner.counters[i].is_none() {
                inner.counters[i] = Some(TCounter::new(name.to_string()));
                if i >= inner.num_counters {
                    inner.num_counters = i + 1;
                }
                return Ok(i);
            }
        }

        Err(GameLogicError::Configuration(
            "Maximum counters exceeded".to_string(),
        ))
    }

    /// Allocate a flag
    pub fn allocate_flag(&self, name: &str) -> GameLogicResult<usize> {
        let mut inner = self.lock_inner_mut();
        // Check if flag already exists
        for (i, flag) in inner.flags.iter().enumerate() {
            if let Some(flag) = flag {
                if flag.name == name {
                    return Ok(i);
                }
            }
        }

        // Find empty slot
        for i in 0..MAX_FLAGS {
            if inner.flags[i].is_none() {
                inner.flags[i] = Some(TFlag::new(name.to_string()));
                if i >= inner.num_flags {
                    inner.num_flags = i + 1;
                }
                return Ok(i);
            }
        }

        Err(GameLogicError::Configuration(
            "Maximum flags exceeded".to_string(),
        ))
    }

    /// Get counter by name (owned snapshot — never a borrow into `UnsafeCell`).
    pub fn get_counter(&self, name: &str) -> Option<TCounter> {
        self.with_inner(|inner| {
            for counter in &inner.counters {
                if let Some(counter) = counter {
                    if counter.name == name {
                        return Some(counter.clone());
                    }
                }
            }
            None
        })
    }

    /// Get flag by name (owned snapshot — never a borrow into `UnsafeCell`).
    pub fn get_flag(&self, name: &str) -> Option<TFlag> {
        self.with_inner(|inner| {
            for flag in &inner.flags {
                if let Some(flag) = flag {
                    if flag.name == name {
                        return Some(flag.clone());
                    }
                }
            }
            None
        })
    }

    /// Set counter value
    pub fn set_counter(&self, name: &str, value: i32) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        let mut inner = self.lock_inner_mut();
        if let Some(counter) = &mut inner.counters[index] {
            counter.value = value;
        }
        Ok(())
    }

    /// Set flag value
    pub fn set_flag(&self, name: &str, value: bool) -> GameLogicResult<()> {
        let index = self.allocate_flag(name)?;
        let mut inner = self.lock_inner_mut();
        if let Some(flag) = &mut inner.flags[index] {
            flag.value = value;
        }
        Ok(())
    }

    /// Increment counter value
    pub fn increment_counter(&self, name: &str) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        let mut inner = self.lock_inner_mut();
        if let Some(counter) = &mut inner.counters[index] {
            counter.value = counter.value.saturating_add(1);
        }
        Ok(())
    }

    /// Decrement counter value
    pub fn decrement_counter(&self, name: &str) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        let mut inner = self.lock_inner_mut();
        if let Some(counter) = &mut inner.counters[index] {
            counter.value = counter.value.saturating_sub(1);
        }
        Ok(())
    }

    /// Set timer (countdown counter) in frames (1 second = 30 frames at standard logic rate)
    /// C++ Reference: ScriptActions::doSetTimer() - timers count down each frame
    pub fn set_timer(&self, name: &str, frames: i32) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        let mut inner = self.lock_inner_mut();
        if let Some(counter) = &mut inner.counters[index] {
            counter.value = frames;
            counter.is_countdown_timer = true;
        }
        Ok(())
    }

    /// Set timer in seconds (converts to frames at logic frame rate)
    pub fn set_timer_seconds(&self, name: &str, seconds: f32) -> GameLogicResult<()> {
        let frames = (seconds * LOGICFRAMES_PER_SECOND as f32) as i32;
        self.set_timer(name, frames)
    }

    /// C++ `SET_MILLISECOND_TIMER` script actions actually pass a real-valued second duration
    /// through the mission/script layer and then ceil the converted frame count.
    fn frames_from_millisecond_script_seconds(seconds: f32) -> i32 {
        (seconds.max(0.0) * LOGICFRAMES_PER_SECOND as f32).ceil() as i32
    }

    /// Set timer using the legacy script "msec" path semantics from C++.
    pub fn set_timer_millisecond_script_seconds(
        &self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        let frames = Self::frames_from_millisecond_script_seconds(seconds);
        self.set_timer(name, frames)
    }

    /// Stop/pause a timer without clearing its remaining value.
    pub fn stop_timer(&self, name: &str) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        let mut inner = self.lock_inner_mut();
        if let Some(counter) = &mut inner.counters[index] {
            counter.is_countdown_timer = false;
        }
        Ok(())
    }

    /// Restart a timer (reset to its original value - keeps is_countdown_timer=true)
    /// Note: Without storing original value, this just re-enables countdown at current value
    pub fn restart_timer(&self, name: &str) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        let mut inner = self.lock_inner_mut();
        if let Some(counter) = &mut inner.counters[index] {
            counter.is_countdown_timer = true;
        }
        Ok(())
    }

    /// Add legacy script "msec" seconds to timer.
    pub fn add_to_timer_millisecond_script_seconds(
        &self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        let frames = Self::frames_from_millisecond_script_seconds(seconds);
        let index = self.allocate_counter(name)?;
        let mut inner = self.lock_inner_mut();
        if let Some(counter) = &mut inner.counters[index] {
            counter.value += frames;
        }
        Ok(())
    }

    /// Subtract legacy script "msec" seconds from timer.
    pub fn subtract_from_timer_millisecond_script_seconds(
        &self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        let frames = Self::frames_from_millisecond_script_seconds(seconds);
        let index = self.allocate_counter(name)?;
        let mut inner = self.lock_inner_mut();
        if let Some(counter) = &mut inner.counters[index] {
            counter.value -= frames;
        }
        Ok(())
    }

    /// Start the normal win/lose timer.
    ///
    /// C++ `FRAMES_TO_SHOW_WIN_LOSE_MESSAGE` is 120 logic frames.  This is
    /// interior state so script actions can start the timer while the active
    /// engine is lexically scoped, without trying to lock the global engine
    /// again.
    pub fn start_end_game_timer(&self) {
        let mut inner = self.lock_inner_mut();
        inner.end_game_timer = 120;
        log::info!("End game timer started");
    }

    /// Start the one-frame quick-victory timer.
    pub fn start_quick_end_game_timer(&self) {
        let mut inner = self.lock_inner_mut();
        inner.end_game_timer = 1;
        log::info!("Quick end game timer started");
    }

    /// Start the local-defeat close-window timer.
    pub fn start_close_window_timer(&self) {
        let mut inner = self.lock_inner_mut();
        inner.close_window_timer = 120;
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

    pub fn current_win_lose_window(&self) -> Option<String> {
        self.with_inner(|inner| inner.win_lose_window_layout.clone())
    }

    pub fn is_suppress_new_windows(&self) -> bool {
        self.with_inner(|inner| inner.suppress_new_windows)
    }

    pub fn should_show_observer_quit_window(&self) -> bool {
        let local_is_observer = crate::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|player| player.read().ok().map(|guard| guard.is_player_observer()))
            .unwrap_or(false);
        local_is_observer || self.has_shown_mp_local_defeat_window()
    }

    pub fn should_show_local_defeat_window(&self) -> bool {
        let local_is_observer = crate::player::player_list()
            .read()
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
        qualify_trigger_area_name(area_name, self.get_current_player_name().as_deref())
            .and_then(|resolved| {
                crate::terrain::get_terrain_logic()
                    .read()
                    .ok()
                    .and_then(|terrain| terrain.get_trigger_area_by_name(&resolved).cloned())
            })
    }

    /// Check if game is ending
    pub fn is_game_ending(&self) -> bool {
        self.with_inner(|inner| inner.end_game_timer >= 0)
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
        let inner = self.inner.get_mut();
        inner.freeze_by_debug = frozen;
    }

    /// Check if time is frozen by debug controls.
    pub fn is_time_frozen_debug(&self) -> bool {
        self.with_inner(|inner| inner.freeze_by_debug)
    }

    /// Check if time is frozen by any mechanism (script or debug).
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3603-3604
    /// C++: `freezeTime = TheTacticalView->isTimeFrozen() ||
    ///        TheScriptEngine->isTimeFrozenDebug() ||
    ///        TheScriptEngine->isTimeFrozenScript();`
    pub fn is_time_frozen(&self) -> bool {
        self.with_inner(|inner| inner.freeze_by_debug || inner.freeze_by_script)
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
        let mut inner = self.lock_inner_mut();
        inner.fade = fade;
        inner.cur_fade_frame = 0;
        inner.min_fade = min_fade;
        inner.max_fade = max_fade;
        inner.fade_frames_increase = fade_frames_increase;
        inner.fade_frames_hold = fade_frames_hold;
        inner.fade_frames_decrease = fade_frames_decrease;
        inner.cur_fade_value = inner.min_fade;

        let update_immediately = inner.fade_frames_increase == 0;
        drop(inner);
        if update_immediately {
            self.update_fades();
        }
    }

    /// Get fade type
    pub fn get_fade(&self) -> TFade {
        self.with_inner(|inner| inner.fade)
    }

    /// Get fade value
    pub fn get_fade_value(&self) -> f32 {
        self.with_inner(|inner| inner.cur_fade_value)
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

    pub fn set_global_difficulty(&mut self, difficulty: crate::player::GameDifficulty) {
        let inner = self.inner.get_mut();
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

    /// Mirrors C++ `ScriptEngine::setEnableVTune`.
    ///
    /// C++ stores this as static script-engine runtime state; Rust keeps the same
    /// singleton-style behavior in `engine.rs` shared state.
    pub fn set_enable_vtune(&mut self, enabled: bool) {
        set_enable_vtune(enabled);
    }

    /// Mirrors C++ `ScriptEngine::getEnableVTune`.
    pub fn get_enable_vtune(&self) -> bool {
        get_enable_vtune()
    }

    /// Command-level parity hook for `TheSkateDistOverride` style debug state.
    pub fn set_skate_distance_override(&mut self, value: f32) {
        set_skate_distance_override(value);
    }

    /// Command-level parity hook for `TheSkateDistOverride` style debug state.
    pub fn adjust_skate_distance_override(&mut self, delta: f32) -> f32 {
        adjust_skate_distance_override(delta)
    }

    /// Command-level parity hook for `TheSkateDistOverride` style debug state.
    pub fn get_skate_distance_override(&self) -> f32 {
        get_skate_distance_override()
    }

    /// Get action template (owned snapshot — never a borrow into `UnsafeCell`).
    pub fn get_action_template(&self, index: usize) -> Option<ActionTemplate> {
        self.with_inner(|inner| inner.action_templates.get(index).cloned())
    }

    /// Get condition template (owned snapshot — never a borrow into `UnsafeCell`).
    pub fn get_condition_template(&self, index: usize) -> Option<ConditionTemplate> {
        self.with_inner(|inner| inner.condition_templates.get(index).cloned())
    }

    pub fn find_condition_type_by_name_key(&self, name_key: u32) -> Option<ConditionType> {
        self.with_inner(|inner| {
            inner
                .condition_templates
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
                .action_templates
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

    /// Append sequential script
    pub fn append_sequential_script(&self, mut script: SequentialScript) {
        script.next_script_in_sequence = None;
        script.current_instruction = -1;

        let target_object = script.object_id;
        let target_team = script.team_to_exec_on.clone();

        let mut inner = self.lock_inner_mut();
        // `SequentialScript` is cloned when it is appended in C++. Give this
        // Rust node a fresh process-local identity rather than retaining the
        // source node's reconciliation token.
        script.runtime_token = Self::allocate_sequential_runtime_token(&mut inner);
        for existing in &mut inner.sequential_scripts {
            let object_match = target_object != INVALID_ID && existing.object_id == target_object;
            let team_match = target_team.is_some() && existing.team_to_exec_on == target_team;
            if !(object_match || team_match) {
                continue;
            }

            let mut cursor = &mut existing.next_script_in_sequence;
            loop {
                match cursor {
                    Some(next) => {
                        cursor = &mut next.next_script_in_sequence;
                    }
                    None => {
                        *cursor = Some(Box::new(script));
                        break;
                    }
                }
            }
            return;
        }

        inner.sequential_scripts.push(script);
    }

    /// Remove all sequential scripts bound to a specific object.
    pub fn remove_all_sequential_scripts_for_object(&self, object_id: ObjectID) {
        let mut inner = self.lock_inner_mut();
        inner
            .sequential_scripts
            .retain(|script| script.object_id != object_id);
    }

    /// Check if a specific object has any active sequential scripts running.
    /// PARITY_NOTE: C++ ScriptConditions does not have a case for UNIT_COMPLETED_SEQUENTIAL_EXECUTION
    /// in its evaluateCondition switch (hits default DEBUG_CRASH returning false). This provides
    /// the intended semantics: returns false if scripts are still active, true if none remain.
    pub fn has_active_sequential_script_for_object(&self, object_id: ObjectID) -> bool {
        self.with_inner(|inner| {
            inner
                .sequential_scripts
                .iter()
                .any(|script| script.object_id == object_id)
        })
    }

    /// Check if a specific team has any active sequential scripts running.
    pub fn has_active_sequential_script_for_team(&self, team_name: &str) -> bool {
        self.with_inner(|inner| {
            inner
                .sequential_scripts
                .iter()
                .any(|script| script.team_to_exec_on.as_deref() == Some(team_name))
        })
    }

    /// Remove all sequential scripts bound to a specific team.
    pub fn remove_all_sequential_scripts_for_team(&self, team_name: &str) {
        let mut inner = self.lock_inner_mut();
        inner
            .sequential_scripts
            .retain(|script| script.team_to_exec_on.as_deref() != Some(team_name));
    }

    /// Set frame wait timer for all sequential scripts running on an object.
    pub fn set_sequential_timer_for_object(&mut self, object_id: ObjectID, frame_count: i32) {
        let inner = self.inner.get_mut();
        for script in &mut inner.sequential_scripts {
            if script.object_id == object_id {
                script.frames_to_wait = frame_count;
                return;
            }
        }
    }

    /// Set frame wait timer for all sequential scripts running on a team.
    pub fn set_sequential_timer_for_team(&mut self, team_name: &str, frame_count: i32) {
        let inner = self.inner.get_mut();
        for script in &mut inner.sequential_scripts {
            if script.team_to_exec_on.as_deref() == Some(team_name) {
                script.frames_to_wait = frame_count;
                return;
            }
        }
    }

    /// Notify of completed video
    pub fn notify_of_completed_video(&mut self, video_name: &str) {
        let inner = self.inner.get_mut();
        inner.completed_video.push(video_name.to_string());
        log::debug!("Video completed: {}", video_name);
    }

    /// Notify the script engine that a special power was triggered.
    pub fn notify_of_triggered_special_power(
        &self,
        player_index: usize,
        power_name: &str,
        source_obj: ObjectId,
    ) {
        let mut inner = self.lock_inner_mut();
        if player_index >= Self::MAX_PLAYER_COUNT {
            log::warn!(
                "notify_of_triggered_special_power: player index {} out of range",
                player_index
            );
            return;
        }
        inner.triggered_special_powers[player_index].push((power_name.to_string(), source_obj));
    }

    /// Notify the script engine that a special power reached its midway trigger.
    pub fn notify_of_midway_special_power(
        &self,
        player_index: usize,
        power_name: &str,
        source_obj: ObjectId,
    ) {
        let mut inner = self.lock_inner_mut();
        if player_index >= Self::MAX_PLAYER_COUNT {
            log::warn!(
                "notify_of_midway_special_power: player index {} out of range",
                player_index
            );
            return;
        }
        inner.midway_special_powers[player_index].push((power_name.to_string(), source_obj));
    }

    /// Notify the script engine that a special power finished executing.
    pub fn notify_of_completed_special_power(
        &self,
        player_index: usize,
        power_name: &str,
        source_obj: ObjectId,
    ) {
        let mut inner = self.lock_inner_mut();
        if player_index >= Self::MAX_PLAYER_COUNT {
            log::warn!(
                "notify_of_completed_special_power: player index {} out of range",
                player_index
            );
            return;
        }
        inner.finished_special_powers[player_index].push((power_name.to_string(), source_obj));
    }

    /// Notify the script engine that an upgrade completed.
    pub fn notify_of_completed_upgrade(
        &self,
        player_index: usize,
        upgrade_name: &str,
        source_obj: ObjectId,
    ) {
        let mut inner = self.lock_inner_mut();
        if player_index >= Self::MAX_PLAYER_COUNT {
            log::warn!(
                "notify_of_completed_upgrade: player index {} out of range",
                player_index
            );
            return;
        }
        inner.completed_upgrades[player_index].push((upgrade_name.to_string(), source_obj));
    }

    fn is_named_event_in_list(
        list: &mut Vec<(String, ObjectId)>,
        event_name: &str,
        remove_from_list: bool,
        source_obj: ObjectId,
    ) -> bool {
        let matched_pos = list.iter().position(|(name, obj_id)| {
            if name != event_name {
                return false;
            }
            if source_obj == crate::common::INVALID_ID {
                return true;
            }
            *obj_id == source_obj
        });
        if let Some(pos) = matched_pos {
            if remove_from_list {
                list.remove(pos);
            }
            return true;
        }
        false
    }

    pub fn is_special_power_triggered(
        &self,
        player_index: usize,
        power_name: &str,
        remove_from_list: bool,
        source_obj: ObjectId,
    ) -> bool {
        let mut inner = self.lock_inner_mut();
        let Some(list) = inner.triggered_special_powers.get_mut(player_index) else {
            return false;
        };
        Self::is_named_event_in_list(list, power_name, remove_from_list, source_obj)
    }

    pub fn is_special_power_midway(
        &self,
        player_index: usize,
        power_name: &str,
        remove_from_list: bool,
        source_obj: ObjectId,
    ) -> bool {
        let mut inner = self.lock_inner_mut();
        let Some(list) = inner.midway_special_powers.get_mut(player_index) else {
            return false;
        };
        Self::is_named_event_in_list(list, power_name, remove_from_list, source_obj)
    }

    pub fn is_special_power_complete(
        &self,
        player_index: usize,
        power_name: &str,
        remove_from_list: bool,
        source_obj: ObjectId,
    ) -> bool {
        let mut inner = self.lock_inner_mut();
        let Some(list) = inner.finished_special_powers.get_mut(player_index) else {
            return false;
        };
        Self::is_named_event_in_list(list, power_name, remove_from_list, source_obj)
    }

    pub fn is_upgrade_complete(
        &self,
        player_index: usize,
        upgrade_name: &str,
        remove_from_list: bool,
        source_obj: ObjectId,
    ) -> bool {
        let mut inner = self.lock_inner_mut();
        let Some(list) = inner.completed_upgrades.get_mut(player_index) else {
            return false;
        };
        Self::is_named_event_in_list(list, upgrade_name, remove_from_list, source_obj)
    }

    /// Check if video is complete
    pub fn is_video_complete(&self, video_name: &str, remove_from_list: bool) -> bool {
        let mut inner = self.lock_inner_mut();
        if let Some(pos) = inner.completed_video.iter().position(|v| v == video_name) {
            if remove_from_list {
                inner.completed_video.remove(pos);
            }
            true
        } else {
            false
        }
    }

    fn is_timed_audio_complete(
        list: &mut Vec<(String, u32)>,
        event_name: &str,
        remove_from_list: bool,
    ) -> bool {
        if event_name.trim().is_empty() {
            return false;
        }

        let position = if let Some(pos) = list.iter().position(|(name, _)| name == event_name) {
            pos
        } else {
            let audio_length_ms = TheAudio::get()
                .map(|audio| {
                    let event = crate::common::audio::AudioEventRts::new(event_name);
                    audio.get_audio_length_ms(&event)
                })
                .unwrap_or(0.0)
                .max(0.0);
            // C++ uses REAL_TO_UNSIGNEDINT(audioLength / MSEC_PER_LOGICFRAME_REAL): truncate.
            let frame_count = ((audio_length_ms / 1000.0) * LOGICFRAMES_PER_SECOND as f32) as u32;
            let completion_frame = TheGameLogic::get_frame().saturating_add(frame_count);
            list.push((event_name.to_string(), completion_frame));
            list.len() - 1
        };

        let current_frame = TheGameLogic::get_frame();
        let completed = current_frame >= list[position].1;
        if completed && remove_from_list {
            list.remove(position);
        }
        completed
    }

    pub fn is_speech_complete(&self, speech_name: &str, remove_from_list: bool) -> bool {
        let mut inner = self.lock_inner_mut();
        Self::is_timed_audio_complete(&mut inner.testing_speech, speech_name, remove_from_list)
    }

    pub fn is_audio_complete(&self, audio_name: &str, remove_from_list: bool) -> bool {
        let mut inner = self.lock_inner_mut();
        Self::is_timed_audio_complete(&mut inner.testing_audio, audio_name, remove_from_list)
    }

    /// Signal UI interaction
    pub fn signal_ui_interact(&mut self, hook_name: &str) {
        let inner = self.inner.get_mut();
        inner.ui_interactions.push(hook_name.to_string());
        log::debug!("UI interaction: {}", hook_name);
    }

    /// Create named map reveal
    pub fn create_named_map_reveal(
        &self,
        reveal_name: &str,
        waypoint_name: &str,
        radius: f32,
        player_name: &str,
    ) {
        let mut inner = self.lock_inner_mut();
        if inner
            .named_reveals
            .iter()
            .any(|reveal| reveal.reveal_name == reveal_name)
        {
            log::warn!(
                "create_named_map_reveal: attempted to redefine named reveal '{}'",
                reveal_name
            );
            return;
        }

        let reveal = NamedReveal {
            reveal_name: reveal_name.to_string(),
            waypoint_name: waypoint_name.to_string(),
            radius_to_reveal: radius,
            player_name: player_name.to_string(),
        };
        inner.named_reveals.push(reveal);
        log::debug!("Created named map reveal: {}", reveal_name);
    }

    /// Apply a named map reveal (matches C++ ScriptEngine::doNamedMapReveal).
    pub fn do_named_map_reveal(&self, reveal_name: &str) {
        let reveal = self.with_inner(|inner| {
            inner
                .named_reveals
                .iter()
                .find(|entry| entry.reveal_name == reveal_name)
                .cloned()
        });
        let Some(reveal) = reveal else {
            return;
        };

        let waypoint_ascii = AsciiString::from(reveal.waypoint_name.as_str());
        let target = crate::terrain::get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            });
        let Some(target) = target else {
            return;
        };

        let Ok(players) = crate::player::player_list().read() else {
            return;
        };
        let Some(player_arc) = players.find_player_by_name(&reveal.player_name) else {
            return;
        };
        let Ok(player) = player_arc.read() else {
            return;
        };
        let player_mask = player.get_player_mask().bits();

        let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
        if let Ok(mut shroud_mgr) = shroud_mgr.lock() {
            shroud_mgr.do_shroud_reveal(&target, reveal.radius_to_reveal, player_mask);
        }
    }

    /// Undo a named map reveal (matches C++ ScriptEngine::undoNamedMapReveal).
    pub fn undo_named_map_reveal(&self, reveal_name: &str) {
        let reveal = self.with_inner(|inner| {
            inner
                .named_reveals
                .iter()
                .find(|entry| entry.reveal_name == reveal_name)
                .cloned()
        });
        let Some(reveal) = reveal else {
            return;
        };

        let waypoint_ascii = AsciiString::from(reveal.waypoint_name.as_str());
        let target = crate::terrain::get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            });
        let Some(target) = target else {
            return;
        };

        let Ok(players) = crate::player::player_list().read() else {
            return;
        };
        let Some(player_arc) = players.find_player_by_name(&reveal.player_name) else {
            return;
        };
        let Ok(player) = player_arc.read() else {
            return;
        };
        let player_mask = player.get_player_mask().bits();

        let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
        if let Ok(mut shroud_mgr) = shroud_mgr.lock() {
            shroud_mgr.undo_shroud_reveal(&target, reveal.radius_to_reveal, player_mask);
        }
    }

    /// Remove a named map reveal (matches C++ ScriptEngine::removeNamedMapReveal).
    pub fn remove_named_map_reveal(&self, reveal_name: &str) {
        let mut inner = self.lock_inner_mut();
        if let Some(index) = inner
            .named_reveals
            .iter()
            .position(|entry| entry.reveal_name == reveal_name)
        {
            inner.named_reveals.remove(index);
        }
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
    pub fn notify_of_acquired_science(&mut self, player_index: usize, science: ScienceType) {
        let inner = self.inner.get_mut();
        if player_index < inner.acquired_sciences.len() {
            inner.acquired_sciences[player_index].push(science);
            log::debug!("Player {} acquired science: {:?}", player_index, science);
        }
    }

    /// Check if a science was acquired by a player (optionally remove the entry).
    pub fn is_science_acquired(
        &self,
        player_index: usize,
        science: ScienceType,
        remove: bool,
    ) -> bool {
        let mut inner = self.lock_inner_mut();
        let Some(list) = inner.acquired_sciences.get_mut(player_index) else {
            return false;
        };

        if let Some(pos) = list.iter().position(|s| *s == science) {
            if remove {
                list.remove(pos);
            }
            return true;
        }

        false
    }

    // =========================================================================
    // MISSING METHODS PORTED FROM C++ ScriptEngine
    // =========================================================================

    // PARITY_NOTE: C++ ScriptEngine::notifyOfObjectDestruction
    pub fn notify_of_object_destruction(&mut self, object_id: ObjectID) {
        let inner = self.inner.get_mut();
        let tracker = get_named_object_tracker();
        let name = tracker.get_object_name(object_id).ok().flatten();
        if let Some(name) = name {
            if !name.is_empty() {
                let _ = tracker.unregister_object(object_id);
            }
        }

        if inner.condition_object == Some(object_id) {
            inner.condition_object = None;
        }
        if inner.calling_object == Some(object_id) {
            inner.calling_object = None;
        }
    }

    // PARITY_NOTE: C++ ScriptEngine::notifyOfTeamDestruction
    pub fn notify_of_team_destruction(&mut self, team_name: &str) {
        if team_name.is_empty() {
            return;
        }

        self.remove_all_sequential_scripts_for_team(team_name);
        let inner = self.inner.get_mut();

        if inner.calling_team.as_deref() == Some(team_name) {
            inner.calling_team = None;
        }
        if inner.condition_team.as_deref() == Some(team_name) {
            inner.condition_team = None;
        }
    }

    // PARITY_NOTE: C++ ScriptEngine::forceUnfreezeTime
    pub fn force_unfreeze_time(&mut self) {}

    // PARITY_NOTE: C++ ScriptEngine::clearFlag
    pub fn clear_flag(&mut self, name: &str) {
        let inner = self.inner.get_mut();
        for j in 0..Self::MAX_PLAYER_COUNT {
            let mod_name = format!("{}{}", name, j);
            for i in 1..inner.num_flags {
                if let Some(flag) = &mut inner.flags[i] {
                    if flag.name == mod_name {
                        flag.value = false;
                    }
                }
            }
        }
    }

    // PARITY_NOTE: C++ ScriptEngine::clearTeamFlags
    pub fn clear_team_flags(&mut self) {
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

    // PARITY_NOTE: C++ ScriptEngine::runScript
    pub fn run_script(&mut self, script_name: &str, team_name: Option<&str>) {
        if script_name.is_empty() || script_name == "<none>" {
            return;
        }

        let (saved_current_player, saved_calling_team) = {
            let inner = self.inner.get_mut();
            let saved_current_player = inner.current_player.clone();
            let saved_calling_team = inner.calling_team.take();
            inner.condition_team = None;
            inner.current_player = None;

            if let Some(team_name_str) = team_name {
                inner.calling_team = Some(team_name_str.to_string());
                if let Ok(mut factory) = get_team_factory().lock() {
                    if let Some(team_arc) = factory.find_team(team_name_str) {
                        if let Ok(team_guard) = team_arc.read() {
                            if let Some(player_id) = team_guard.get_controlling_player_id() {
                                inner.current_player = crate::player::player_list()
                                    .read()
                                    .ok()
                                    .and_then(|list| list.get_player(player_id as i32).cloned())
                                    .and_then(|p| {
                                        p.read().ok().and_then(|p| {
                                            game_engine::common::name_key_generator::NameKeyGenerator::key_to_name(
                                                p.get_player_name_key(),
                                            )
                                        })
                                    });
                            }
                        }
                    }
                }
            }
            (saved_current_player, saved_calling_team)
        };

        let _found = self
            .execute_subroutine_by_name(script_name)
            .unwrap_or(false);

        let inner = self.inner.get_mut();
        inner.calling_team = saved_calling_team;
        inner.current_player = saved_current_player;
    }

    // PARITY_NOTE: C++ ScriptEngine::runObjectScript
    pub fn run_object_script(&mut self, script_name: &str, object_id: ObjectID) {
        if script_name.is_empty() || script_name == "<none>" {
            return;
        }

        let saved_calling_object = {
            let inner = self.inner.get_mut();
            let saved = inner.calling_object;
            inner.calling_object = Some(object_id);
            saved
        };

        let _found = self
            .execute_subroutine_by_name(script_name)
            .unwrap_or(false);

        self.inner.get_mut().calling_object = saved_calling_object;
    }

    // PARITY_NOTE: C++ ScriptEngine::evaluateConditions
    pub fn evaluate_conditions(
        &mut self,
        script: &mut Script,
        team_name: Option<&str>,
        player_name: Option<&str>,
    ) -> bool {
        let (saved_calling_team, saved_current_player) = {
            let inner = self.inner.get_mut();
            let saved_calling_team = inner.calling_team.take();
            let saved_current_player = inner.current_player.clone();

            inner.calling_team = team_name.map(|s| s.to_string());

            if player_name.is_some() {
                inner.current_player = player_name.map(|s| s.to_string());
            } else if let Some(ref tname) = inner.calling_team {
                if let Ok(mut factory) = get_team_factory().lock() {
                    if let Some(team_arc) = factory.find_team(tname) {
                        if let Ok(team_guard) = team_arc.read() {
                            if let Some(pid) = team_guard.get_controlling_player_id() {
                                inner.current_player = crate::player::player_list()
                                    .read()
                                    .ok()
                                    .and_then(|list| list.get_player(pid as i32).cloned())
                                    .and_then(|p| {
                                        p.read().ok().and_then(|p| {
                                            game_engine::common::name_key_generator::NameKeyGenerator::key_to_name(
                                                p.get_player_name_key(),
                                            )
                                        })
                                    });
                            }
                        }
                    }
                }
            }
            (saved_calling_team, saved_current_player)
        };

        let result = if let Some(or_cond) = script.condition.as_deref_mut() {
            let mut test_value = false;
            let mut current_or = Some(or_cond);
            while let Some(or_node) = current_or {
                if let Some(and_cond) = or_node.first_and.as_deref_mut() {
                    let mut and_term = true;
                    let mut current_and: Option<&mut Condition> = Some(and_cond);
                    while let Some(cond) = current_and {
                        let cond_type = cond.get_condition_type();
                        let cond_result = match cond_type {
                            ConditionType::Counter => self.evaluate_counter_condition_inline(cond),
                            ConditionType::Flag => self.evaluate_flag_condition_inline(cond),
                            ConditionType::TimerExpired => {
                                self.evaluate_timer_condition_inline(cond)
                            }
                            ConditionType::ConditionTrue => true,
                            ConditionType::ConditionFalse => false,
                            _ => false,
                        };
                        if !cond_result {
                            and_term = false;
                            break;
                        }
                        current_and = cond.next_and_condition.as_deref_mut();
                    }
                    if and_term {
                        test_value = true;
                        break;
                    }
                }
                current_or = or_node.next_or.as_deref_mut();
            }
            test_value
        } else {
            false
        };

        let inner = self.inner.get_mut();
        inner.calling_team = saved_calling_team;
        inner.current_player = saved_current_player;
        result
    }

    fn evaluate_counter_condition_inline(&self, condition: &Condition) -> bool {
        let Some(param0) = condition.get_parameter(0) else {
            return false;
        };
        let Some(param1) = condition.get_parameter(1) else {
            return false;
        };
        let Some(param2) = condition.get_parameter(2) else {
            return false;
        };

        let counter_name = param0.get_string();
        let comparison = param1.get_int();
        let target_value = param2.get_int();
        let counter_value = self.get_counter(counter_name).map(|c| c.value).unwrap_or(0);

        match comparison {
            0 => counter_value < target_value,
            1 => counter_value <= target_value,
            2 => counter_value == target_value,
            3 => counter_value >= target_value,
            4 => counter_value > target_value,
            5 => counter_value != target_value,
            _ => false,
        }
    }

    fn evaluate_flag_condition_inline(&self, condition: &Condition) -> bool {
        let Some(param0) = condition.get_parameter(0) else {
            return false;
        };
        let Some(param1) = condition.get_parameter(1) else {
            return false;
        };

        let flag_name = param0.get_string();
        let target_value = param1.get_int() != 0;
        self.get_flag(flag_name).map(|f| f.value).unwrap_or(false) == target_value
    }

    fn evaluate_timer_condition_inline(&self, condition: &Condition) -> bool {
        let Some(param0) = condition.get_parameter(0) else {
            return false;
        };
        let counter_name = param0.get_string();
        self.get_counter(counter_name)
            .map(|c| c.is_countdown_timer && c.value < 1)
            .unwrap_or(false)
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
        let index = self.allocate_counter(counter_name)?;
        let inner = self.inner.get_mut();
        let Some(counter) = &mut inner.counters[index] else {
            return Ok(());
        };
        if millisecond_timer {
            let msec_frames = Self::frames_from_millisecond_script_seconds(value as f32);
            let delta = if add { msec_frames } else { -msec_frames };
            counter.value += delta;
        } else {
            let delta = if add { value } else { -value };
            counter.value += delta;
        }
        Ok(())
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
        let index = self.allocate_counter(name)?;
        let inner = self.inner.get_mut();
        if let Some(counter) = &mut inner.counters[index] {
            if counter.value > 0 {
                counter.is_countdown_timer = true;
            }
        }
        Ok(())
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
        let index = self.allocate_counter(name)?;
        let inner = self.inner.get_mut();
        let Some(counter) = &mut inner.counters[index] else {
            return Ok(());
        };

        let effective_value = if random {
            let max = random_max.unwrap_or(value);
            crate::helpers::get_game_logic_random_value_real(value.min(max), value.max(max))
        } else {
            value
        };

        if millisecond_timer {
            counter.value = Self::frames_from_millisecond_script_seconds(effective_value);
        } else {
            counter.value = effective_value as i32;
        }
        counter.is_countdown_timer = true;
        Ok(())
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
}
