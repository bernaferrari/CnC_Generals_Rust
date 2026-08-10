// ScriptEngine construction, templates, script lists, and reset
//
// Split from `scripting/engine.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEngine {
    fn enum_name_to_internal_name(name: &str) -> String {
        let mut out = String::with_capacity(name.len() * 2);
        let mut prev_is_upper = true;
        for ch in name.chars() {
            let is_upper = ch.is_ascii_uppercase();
            if !out.is_empty() && is_upper && !prev_is_upper {
                out.push('_');
            }
            out.push(ch.to_ascii_uppercase());
            prev_is_upper = is_upper;
        }
        out
    }

    fn seed_template_internal_names(&mut self) {
        for idx in 0..(ScriptActionType::NumItems as u32) {
            let Some(action_type) = ScriptActionType::from_u32(idx) else {
                continue;
            };
            if action_type == ScriptActionType::NumItems {
                continue;
            }
            if let Some(template) = self.action_templates.get_mut(idx as usize) {
                let internal_name = Self::enum_name_to_internal_name(&format!("{:?}", action_type));
                template.base.internal_name = internal_name.clone();
                template.base.internal_name_key = NameKeyGenerator::name_to_key(&internal_name);
                if template.base.ui_name.is_empty() {
                    template.base.ui_name = internal_name;
                }
            }
        }

        for idx in 0..(ConditionType::NumItems as u32) {
            let Some(condition_type) = ConditionType::from_u32(idx) else {
                continue;
            };
            if condition_type == ConditionType::NumItems {
                continue;
            }
            if let Some(template) = self.condition_templates.get_mut(idx as usize) {
                let internal_name =
                    Self::enum_name_to_internal_name(&format!("{:?}", condition_type));
                template.base.internal_name = internal_name.clone();
                template.base.internal_name_key = NameKeyGenerator::name_to_key(&internal_name);
                if template.base.ui_name.is_empty() {
                    template.base.ui_name = internal_name;
                }
            }
        }
    }

    const MAX_PLAYER_COUNT: usize = crate::common::MAX_PLAYER_COUNT;

    pub fn new() -> GameLogicResult<Self> {
        let mut engine = Self {
            inner: UnsafeCell::new(ScriptEngineInner {
            action_templates: Vec::with_capacity(ScriptActionType::NumItems as usize),
            condition_templates: Vec::with_capacity(ConditionType::NumItems as usize),

            counters: vec![None; MAX_COUNTERS],
            num_counters: 1,
            flags: vec![None; MAX_FLAGS],
            num_flags: 1,
            attack_priority_info: Vec::with_capacity(MAX_ATTACK_PRIORITIES),
            num_attack_info: 1,

            end_game_timer: -1,
            close_window_timer: -1,
            calling_team: None,
            calling_object: None,
            condition_team: None,
            condition_object: None,
            first_update: true,
            current_player: None,
            skirmish_human_player: None,
            current_track_name: String::new(),

            fade: TFade::None,
            min_fade: 0.0,
            max_fade: 1.0,
            cur_fade_value: 0.0,
            cur_fade_frame: 0,
            fade_frames_increase: 0,
            fade_frames_hold: 0,
            fade_frames_decrease: 0,

            frame_object_count_changed: 0,
            object_counts: HashMap::new(),
            object_types: HashMap::new(),
            object_attack_priority_sets: HashMap::new(),

            completed_video: Vec::new(),
            testing_speech: Vec::new(),
            testing_audio: Vec::new(),
            ui_interactions: Vec::new(),

            triggered_special_powers: vec![Vec::new(); Self::MAX_PLAYER_COUNT],
            midway_special_powers: vec![Vec::new(); Self::MAX_PLAYER_COUNT],
            finished_special_powers: vec![Vec::new(); Self::MAX_PLAYER_COUNT],
            completed_upgrades: vec![Vec::new(); Self::MAX_PLAYER_COUNT],
            acquired_sciences: vec![Vec::new(); Self::MAX_PLAYER_COUNT],

            topple_directions: Vec::new(),
            named_reveals: Vec::new(),
            breeze_info: BreezeInfo::new(),
            game_difficulty: crate::player::GameDifficulty::Normal,

            freeze_by_script: false,
            freeze_by_debug: false,
            objects_should_receive_difficulty_bonus: true,
            choose_victim_always_uses_normal: false,
            shown_mp_local_defeat_window: false,

            sequential_scripts: Vec::new(),

            side_script_lists: vec![None; Self::MAX_PLAYER_COUNT],

            #[cfg(feature = "script_profiling")]
            stats: ScriptStats::default(),

            action_handler: None,
            }),
            mut_live: AtomicBool::new(false),
        };

        if engine.counters[0].is_none() {
            engine.counters[0] = Some(TCounter::new(String::new()));
        }
        if engine.flags[0].is_none() {
            engine.flags[0] = Some(TFlag::new(String::new()));
        }
        if engine.attack_priority_info.is_empty() {
            engine.attack_priority_info.push(AttackPriorityInfo::new());
        }

        engine.initialize_templates()?;
        Ok(engine)
    }

    pub fn get_frame_object_count_changed(&self) -> u32 {
        self.frame_object_count_changed
    }

    pub fn get_current_player_name(&self) -> Option<&str> {
        self.current_player.as_deref()
    }

    pub fn get_calling_team_name(&self) -> Option<&str> {
        self.calling_team.as_deref()
    }

    /// C++ `ScriptEngine::friend_executeAction(action, pThisTeam)`.
    ///
    /// Saves calling team / current player, binds `pThisTeam` (by name) as the
    /// calling team and its controlling player as current player, runs the
    /// action chain, then restores prior context.
    pub fn friend_execute_action(
        &self,
        action: &crate::scripting::core::ScriptAction,
        team_name: Option<&str>,
    ) {
        let (saved_team, saved_player) = {
            let mut inner = self.lock_inner_mut();
            let saved_team = inner.calling_team.take();
            let saved_player = inner.current_player.take();
            inner.calling_team = team_name.map(|s| s.to_string());
            inner.current_player = None;
            (saved_team, saved_player)
        };

        if let Some(tname) = team_name {
            if let Ok(mut factory) = get_team_factory().lock() {
                if let Some(team_arc) = factory.find_team(tname) {
                    if let Ok(team_guard) = team_arc.read() {
                        if let Some(player_id) = team_guard.get_controlling_player_id() {
                            let current_player = crate::player::player_list()
                                .read()
                                .ok()
                                .and_then(|list| list.get_player(player_id as i32).cloned())
                                .and_then(|p| {
                                    p.read().ok().and_then(|pg| {
                                        game_engine::common::name_key_generator::NameKeyGenerator::key_to_name(
                                            pg.get_player_name_key(),
                                        )
                                    })
                                });
                            self.lock_inner_mut().current_player = current_player;
                        }
                    }
                }
            }
        }

        // TLS active-engine is `&ScriptEngine` only — nested CALL_SUBROUTINE
        // re-enters via lock_inner_mut, matching C++ immediate executeScript.
        let _active = self.enter_active();
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let exec_context = std::sync::Arc::new(std::sync::RwLock::new(
            crate::scripting::executor::ScriptContext {
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
            },
        ));
        let mut dispatcher = crate::scripting::executor::ScriptActionDispatcher::new(exec_context);
        if let Err(err) = self.execute_action_chain(action, &mut dispatcher) {
            log::warn!("friend_execute_action: {}", err);
        }

        let mut inner = self.lock_inner_mut();
        inner.calling_team = saved_team;
        inner.current_player = saved_player;
    }

    pub fn get_condition_team_name(&self) -> Option<&str> {
        self.condition_team.as_deref()
    }

    /// Set temporary runtime context used by external script evaluation helpers.
    ///
    /// Returns the previous `(current_player, condition_team)` tuple so callers can restore it.
    pub fn set_external_eval_context(
        &mut self,
        current_player: Option<String>,
        condition_team: Option<String>,
    ) -> (Option<String>, Option<String>) {
        let saved = (self.current_player.clone(), self.condition_team.clone());
        self.current_player = current_player;
        self.condition_team = condition_team;
        saved
    }

    /// Restore runtime context previously returned by `set_external_eval_context`.
    pub fn restore_external_eval_context(&mut self, saved: (Option<String>, Option<String>)) {
        self.current_player = saved.0;
        self.condition_team = saved.1;
    }

    pub fn set_frame_object_count_changed(&mut self, frame: u32) {
        self.frame_object_count_changed = frame;
    }

    /// Set the script list for a player index (side).
    ///
    /// C++ Reference: `SideInfo::getScriptList()` (ScriptEngine::update loops sides and executes
    /// the side's ScriptList + ScriptGroups).
    pub fn set_script_list_for_player(
        &self,
        player_index: usize,
        script_list: Option<Box<ScriptList>>,
    ) -> GameLogicResult<()> {
        if player_index >= Self::MAX_PLAYER_COUNT {
            return Err(GameLogicError::Configuration(format!(
                "Player index {} out of range for ScriptEngine",
                player_index
            )));
        }
        let mut script_list = script_list;
        if let Some(list) = script_list.as_deref_mut() {
            self.initialize_script_runtime_fields_in_list(list);
        }
        self.lock_inner_mut().side_script_lists[player_index] = script_list;
        Ok(())
    }

    pub fn clear_script_lists(&mut self) {
        for slot in &mut self.side_script_lists {
            *slot = None;
        }
    }

    fn set_script_active_in_chain(script: &mut Script, name: &str, active: bool) -> bool {
        let mut current: Option<&mut Script> = Some(script);
        let mut updated = false;
        while let Some(script_ref) = current {
            if script_ref.script_name == name {
                script_ref.is_active = active;
                updated = true;
            }
            current = script_ref.next_script.as_deref_mut();
        }
        updated
    }

    fn set_script_active_in_list(list: &mut ScriptList, name: &str, active: bool) -> bool {
        let mut updated = false;

        if let Some(script_head) = list.first_script.as_deref_mut() {
            updated |= Self::set_script_active_in_chain(script_head, name, active);
        }

        let mut group_opt = list.first_group.as_deref_mut();
        while let Some(group) = group_opt {
            if let Some(script_head) = group.first_script.as_deref_mut() {
                updated |= Self::set_script_active_in_chain(script_head, name, active);
            }
            group_opt = group.next_group.as_deref_mut();
        }

        updated
    }

    fn find_script_clone_in_chain(
        script: &Script,
        name: &str,
        require_subroutine: bool,
    ) -> Option<Script> {
        let mut current: Option<&Script> = Some(script);
        while let Some(script_ref) = current {
            if script_ref.script_name == name && (!require_subroutine || script_ref.is_subroutine) {
                return Some(script_ref.clone());
            }
            current = script_ref.next_script.as_deref();
        }
        None
    }

    fn find_script_clone_in_list(
        list: &ScriptList,
        name: &str,
        require_subroutine: bool,
    ) -> Option<Script> {
        if let Some(script_head) = list.first_script.as_deref() {
            if let Some(found) =
                Self::find_script_clone_in_chain(script_head, name, require_subroutine)
            {
                return Some(found);
            }
        }

        let mut group_opt = list.first_group.as_deref();
        while let Some(group) = group_opt {
            if let Some(script_head) = group.first_script.as_deref() {
                if let Some(found) =
                    Self::find_script_clone_in_chain(script_head, name, require_subroutine)
                {
                    return Some(found);
                }
            }
            group_opt = group.next_group.as_deref();
        }

        None
    }

    /// Enable/disable a script by name across all loaded ScriptLists.
    ///
    /// Matches the behavior of C++ `ENABLE_SCRIPT` / `DISABLE_SCRIPT` actions.
    pub fn set_script_active_by_name(&self, script_name: &str, active: bool) -> bool {
        let mut updated = false;
        let handler = {
            let mut inner = self.lock_inner_mut();
            for slot in &mut inner.side_script_lists {
                let Some(list) = slot.as_deref_mut() else {
                    continue;
                };
                updated |= Self::set_script_active_in_list(list, script_name, active);
            }
            inner.action_handler.clone()
        };

        if let Some(handler) = handler {
            let _ = handler.enable_script(script_name, active);
        }

        updated
    }

    /// Find a subroutine script by name and return a clone for immediate execution.
    pub fn get_subroutine_clone_by_name(&self, name: &str) -> Option<Script> {
        for slot in &self.side_script_lists {
            let Some(list) = slot.as_deref() else {
                continue;
            };
            if let Some(found) = Self::find_script_clone_in_list(list, name, true) {
                return Some(found);
            }
        }
        None
    }

    fn execute_named_subroutine_in_chain(
        &self,
        script_head: &mut Script,
        name: &str,
        condition_evaluator: &mut crate::scripting::executor::ScriptConditionEvaluator,
        action_dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
    ) -> GameLogicResult<bool> {
        let mut current: Option<&mut Script> = Some(script_head);
        while let Some(script_ref) = current {
            if script_ref.script_name == name {
                if script_ref.is_subroutine {
                    self.execute_script(script_ref, condition_evaluator, action_dispatcher)?;
                } else {
                    log::warn!(
                        "CALL_SUBROUTINE: script '{}' exists but is not a subroutine",
                        name
                    );
                }
                return Ok(true);
            }
            current = script_ref.next_script.as_deref_mut();
        }
        Ok(false)
    }

    fn execute_named_subroutine_in_list(
        &self,
        list: &mut ScriptList,
        name: &str,
        condition_evaluator: &mut crate::scripting::executor::ScriptConditionEvaluator,
        action_dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
    ) -> GameLogicResult<bool> {
        // C++ parity: look up a subroutine group by name first.
        let mut group_opt = list.first_group.as_deref_mut();
        while let Some(group) = group_opt {
            if group.group_name == name {
                if !group.is_group_subroutine {
                    log::warn!(
                        "CALL_SUBROUTINE: group '{}' exists but is not a subroutine group",
                        name
                    );
                    return Ok(true);
                }
                if group.is_group_active {
                    if let Some(script_head) = group.first_script.as_deref_mut() {
                        self.execute_scripts(script_head, condition_evaluator, action_dispatcher)?;
                    }
                }
                return Ok(true);
            }
            group_opt = group.next_group.as_deref_mut();
        }

        if let Some(script_head) = list.first_script.as_deref_mut() {
            if self.execute_named_subroutine_in_chain(
                script_head,
                name,
                condition_evaluator,
                action_dispatcher,
            )? {
                return Ok(true);
            }
        }

        let mut group_opt = list.first_group.as_deref_mut();
        while let Some(group) = group_opt {
            if let Some(script_head) = group.first_script.as_deref_mut() {
                if self.execute_named_subroutine_in_chain(
                    script_head,
                    name,
                    condition_evaluator,
                    action_dispatcher,
                )? {
                    return Ok(true);
                }
            }
            group_opt = group.next_group.as_deref_mut();
        }

        Ok(false)
    }

    /// Execute a subroutine script or subroutine group by name.
    ///
    /// Matches C++ `ScriptEngine::callSubroutine`: group lookup by name takes precedence over
    /// direct script lookup by name.
    ///
    /// Installs the TLS active-engine pointer so nested CALL_SUBROUTINE / flag /
    /// timer mutations can re-enter without deadlocking on the global RwLock.
    pub fn execute_subroutine_by_name(&self, name: &str) -> GameLogicResult<bool> {
        let Some(_depth_guard) = SubroutineDepthGuard::enter() else {
            return Ok(false);
        };
        let _active = self.enter_active();
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

        let mut action_dispatcher =
            crate::scripting::executor::ScriptActionDispatcher::new(exec_context.clone());
        let mut condition_evaluator =
            crate::scripting::executor::ScriptConditionEvaluator::new(exec_context);

        for i in 0..Self::MAX_PLAYER_COUNT {
            let Some(mut script_list) = self.lock_inner_mut().side_script_lists[i].take() else {
                continue;
            };

            let found = self.execute_named_subroutine_in_list(
                &mut script_list,
                name,
                &mut condition_evaluator,
                &mut action_dispatcher,
            )?;
            self.lock_inner_mut().side_script_lists[i] = Some(script_list);
            if found {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Find a script by name and return a clone (non-subroutine allowed).
    pub fn find_script_clone_by_name(&self, name: &str) -> Option<Script> {
        for slot in &self.side_script_lists {
            let Some(list) = slot.as_deref() else {
                continue;
            };
            if let Some(found) = Self::find_script_clone_in_list(list, name, false) {
                return Some(found);
            }
        }
        None
    }

    pub fn get_object_count(&self, player_index: i32, type_name: &str) -> i32 {
        self.object_counts
            .get(&(player_index, type_name.to_string()))
            .copied()
            .unwrap_or(0)
    }

    pub fn set_object_count(&mut self, player_index: i32, type_name: &str, count: i32) {
        self.object_counts
            .insert((player_index, type_name.to_string()), count);
    }

    /// Get a named ObjectTypes list (matches C++ ScriptEngine::getObjectTypes).
    pub fn get_object_types(&self, name: &str) -> Option<ObjectTypes> {
        self.object_types.get(name).cloned()
    }

    /// Register or replace a named ObjectTypes list.
    pub fn set_object_types(&mut self, name: String, types: ObjectTypes) {
        self.object_types.insert(name, types);
    }

    fn ensure_attack_priority_defaults(&mut self) {
        if self.attack_priority_info.is_empty() {
            self.attack_priority_info.push(AttackPriorityInfo::new());
        }
        if self.num_attack_info == 0 {
            self.num_attack_info = 1;
        }
    }

    fn find_attack_info_mut(
        &mut self,
        name: &str,
        add_if_missing: bool,
    ) -> Option<&mut AttackPriorityInfo> {
        self.ensure_attack_priority_defaults();
        let existing_index = (1..self.num_attack_info).find(|&i| {
            self.attack_priority_info
                .get(i)
                .map(|info| info.name == name)
                .unwrap_or(false)
        });
        if let Some(index) = existing_index {
            return self.attack_priority_info.get_mut(index);
        }

        if add_if_missing && self.num_attack_info < MAX_ATTACK_PRIORITIES {
            let mut info = AttackPriorityInfo::new();
            info.name = name.to_string();
            let index = self.num_attack_info;
            if self.attack_priority_info.len() <= index {
                self.attack_priority_info.push(info);
            } else {
                self.attack_priority_info[index] = info;
            }
            self.num_attack_info += 1;
            return self.attack_priority_info.get_mut(index);
        }

        None
    }

    pub fn get_attack_info(&self, name: &str) -> Option<&AttackPriorityInfo> {
        if self.attack_priority_info.is_empty() {
            return None;
        }
        for i in 1..self.num_attack_info {
            if let Some(info) = self.attack_priority_info.get(i) {
                if info.name == name {
                    return Some(info);
                }
            }
        }
        self.attack_priority_info.get(0)
    }

    pub fn set_object_attack_priority_set(&mut self, object_id: ObjectID, set_name: &str) {
        if object_id == INVALID_ID {
            return;
        }

        if set_name.is_empty() {
            self.object_attack_priority_sets.remove(&object_id);
            return;
        }

        self.object_attack_priority_sets
            .insert(object_id, set_name.to_string());
    }

    pub fn clear_object_attack_priority_set(&mut self, object_id: ObjectID) {
        self.object_attack_priority_sets.remove(&object_id);
    }

    pub fn get_object_attack_priority_set(&self, object_id: ObjectID) -> Option<&str> {
        self.object_attack_priority_sets
            .get(&object_id)
            .map(|name| name.as_str())
    }

    fn template_matches_kind(template: &EngineThingTemplate, kind: KindOf) -> bool {
        for idx in kind_of_indices(kind) {
            if template.is_kind_of((*idx) as u64) {
                return true;
            }
        }
        false
    }

    pub fn set_priority_thing(
        &mut self,
        set_name: &str,
        type_or_list: &str,
        priority: i32,
    ) -> bool {
        let object_types = self.get_object_types(type_or_list);
        let Some(info) = self.find_attack_info_mut(set_name, true) else {
            return false;
        };

        if let Some(list) = object_types {
            for type_name in list.iter() {
                if let Some(template) = TheThingFactory::find_template(type_name.as_str()) {
                    info.set_priority(template.get_name().as_str(), priority);
                } else {
                    return false;
                }
            }
            return true;
        }

        if let Some(template) = TheThingFactory::find_template(type_or_list) {
            info.set_priority(template.get_name().as_str(), priority);
            return true;
        }

        false
    }

    pub fn set_priority_kind(&mut self, set_name: &str, kind: KindOf, priority: i32) -> bool {
        let Some(info) = self.find_attack_info_mut(set_name, true) else {
            return false;
        };

        let Ok(factory_guard) = get_thing_factory() else {
            return false;
        };
        let Some(factory) = factory_guard.as_ref() else {
            return false;
        };

        let mut current = factory.first_template().cloned();
        while let Some(template) = current {
            if Self::template_matches_kind(&template, kind) {
                info.set_priority(template.get_name().as_str(), priority);
            }
            current = template.get_next_template().as_ref().cloned();
        }

        true
    }

    pub fn set_priority_default(&mut self, set_name: &str, priority: i32) -> bool {
        let Some(info) = self.find_attack_info_mut(set_name, true) else {
            return false;
        };
        info.default_priority = priority;
        true
    }

    /// Initialize action and condition templates
    fn initialize_templates(&mut self) -> GameLogicResult<()> {
        // Initialize action templates (this would normally be done from INI files)
        self.action_templates
            .resize(ScriptActionType::NumItems as usize, ActionTemplate::new());
        self.condition_templates
            .resize(ConditionType::NumItems as usize, ConditionTemplate::new());

        self.seed_template_internal_names();

        // Set up basic templates (in real implementation, this would be loaded from INI)
        self.setup_basic_templates()?;

        Ok(())
    }

    /// Set up basic templates for core actions and conditions
    fn setup_basic_templates(&mut self) -> GameLogicResult<()> {
        // Victory action
        if let Some(template) = self
            .action_templates
            .get_mut(ScriptActionType::Victory as usize)
        {
            template.base.ui_name = "Victory".to_string();
            template.base.internal_name = "Victory".to_string();
            template.base.help_text = "Triggers victory for the current player".to_string();
        }

        // Defeat action
        if let Some(template) = self
            .action_templates
            .get_mut(ScriptActionType::Defeat as usize)
        {
            template.base.ui_name = "Defeat".to_string();
            template.base.internal_name = "Defeat".to_string();
            template.base.help_text = "Triggers defeat for the current player".to_string();
        }

        // Set flag action
        if let Some(template) = self
            .action_templates
            .get_mut(ScriptActionType::SetFlag as usize)
        {
            template.base.ui_name = "Set Flag".to_string();
            template.base.internal_name = "SetFlag".to_string();
            template.base.help_text = "Sets a script flag to true or false".to_string();
            template.base.parameters = vec![ParameterType::Flag, ParameterType::Boolean];
            template.base.num_parameters = 2;
        }

        // Set counter action
        if let Some(template) = self
            .action_templates
            .get_mut(ScriptActionType::SetCounter as usize)
        {
            template.base.ui_name = "Set Counter".to_string();
            template.base.internal_name = "SetCounter".to_string();
            template.base.help_text = "Sets a script counter to a value".to_string();
            template.base.parameters = vec![ParameterType::Counter, ParameterType::Int];
            template.base.num_parameters = 2;
        }

        // Player all destroyed condition
        if let Some(template) = self
            .condition_templates
            .get_mut(ConditionType::PlayerAllDestroyed as usize)
        {
            template.base.ui_name = "Player All Destroyed".to_string();
            template.base.internal_name = "PlayerAllDestroyed".to_string();
            template.base.help_text = "True if all of a player's units are destroyed".to_string();
            template.base.parameters = vec![ParameterType::Side];
            template.base.num_parameters = 1;
        }

        // Counter condition
        if let Some(template) = self
            .condition_templates
            .get_mut(ConditionType::Counter as usize)
        {
            template.base.ui_name = "Counter".to_string();
            template.base.internal_name = "Counter".to_string();
            template.base.help_text = "Compares a counter value".to_string();
            template.base.parameters = vec![
                ParameterType::Counter,
                ParameterType::Comparison,
                ParameterType::Int,
            ];
            template.base.num_parameters = 3;
        }

        // Flag condition
        if let Some(template) = self
            .condition_templates
            .get_mut(ConditionType::Flag as usize)
        {
            template.base.ui_name = "Flag".to_string();
            template.base.internal_name = "Flag".to_string();
            template.base.help_text = "Checks if a flag is set".to_string();
            template.base.parameters = vec![ParameterType::Flag, ParameterType::Boolean];
            template.base.num_parameters = 2;
        }

        Ok(())
    }

    /// Reset the script engine
    pub fn reset(&mut self) {
        // Clear runtime state
        self.counters.iter_mut().for_each(|c| *c = None);
        self.num_counters = 1;
        self.flags.iter_mut().for_each(|f| *f = None);
        self.num_flags = 1;
        self.attack_priority_info.clear();
        self.num_attack_info = 1;

        self.end_game_timer = -1;
        self.close_window_timer = -1;
        self.calling_team = None;
        self.calling_object = None;
        self.condition_team = None;
        self.condition_object = None;
        self.first_update = true;
        self.current_player = None;
        self.skirmish_human_player = None;
        self.current_track_name.clear();

        self.fade = TFade::None;
        self.cur_fade_value = 0.0;
        self.cur_fade_frame = 0;

        self.completed_video.clear();
        self.testing_speech.clear();
        self.testing_audio.clear();
        self.ui_interactions.clear();

        for powers in &mut self.triggered_special_powers {
            powers.clear();
        }
        for powers in &mut self.midway_special_powers {
            powers.clear();
        }
        for powers in &mut self.finished_special_powers {
            powers.clear();
        }
        for upgrades in &mut self.completed_upgrades {
            upgrades.clear();
        }
        for sciences in &mut self.acquired_sciences {
            sciences.clear();
        }

        self.topple_directions.clear();
        self.named_reveals.clear();
        self.object_types.clear();
        self.object_attack_priority_sets.clear();
        self.breeze_info = BreezeInfo::new();
        self.game_difficulty = crate::player::GameDifficulty::Normal;

        self.freeze_by_script = false;
        self.freeze_by_debug = false;
        self.objects_should_receive_difficulty_bonus = true;
        self.choose_victim_always_uses_normal = false;
        self.shown_mp_local_defeat_window = false;

        self.sequential_scripts.clear();
        self.clear_script_lists();

        #[cfg(feature = "script_profiling")]
        {
            self.stats = ScriptStats::default();
        }

        if self.counters[0].is_none() {
            self.counters[0] = Some(TCounter::new(String::new()));
        }
        if self.flags[0].is_none() {
            self.flags[0] = Some(TFlag::new(String::new()));
        }
        if self.attack_priority_info.is_empty() {
            self.attack_priority_info.push(AttackPriorityInfo::new());
        }
    }
}
