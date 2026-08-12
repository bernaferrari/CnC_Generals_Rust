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
        let inner = self.inner.get_mut();
        for idx in 0..(ScriptActionType::NumItems as u32) {
            let Some(action_type) = ScriptActionType::from_u32(idx) else {
                continue;
            };
            if action_type == ScriptActionType::NumItems {
                continue;
            }
            if let Some(template) = inner.action_templates.get_mut(idx as usize) {
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
            if let Some(template) = inner.condition_templates.get_mut(idx as usize) {
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
            instance_id: NEXT_SCRIPT_ENGINE_INSTANCE_ID
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            inner: std::cell::RefCell::new(ScriptEngineInner {
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
        };

        {
            let inner = engine.inner.get_mut();
            if inner.counters[0].is_none() {
                inner.counters[0] = Some(TCounter::new(String::new()));
            }
            if inner.flags[0].is_none() {
                inner.flags[0] = Some(TFlag::new(String::new()));
            }
            if inner.attack_priority_info.is_empty() {
                inner.attack_priority_info.push(AttackPriorityInfo::new());
            }
        }

        engine.initialize_templates()?;
        Ok(engine)
    }

    pub fn get_frame_object_count_changed(&self) -> u32 {
        self.with_inner(|inner| inner.frame_object_count_changed)
    }

    /// Owned snapshot — never a borrow into `UnsafeCell`.
    pub fn get_current_player_name(&self) -> Option<String> {
        self.with_inner(|inner| inner.current_player.clone())
    }

    /// Owned snapshot — never a borrow into `UnsafeCell`.
    pub fn get_calling_team_name(&self) -> Option<String> {
        self.with_inner(|inner| inner.calling_team.clone())
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

        self.with_active(|| {
            self.friend_execute_action_active(action, team_name, saved_team, saved_player)
        });
    }

    fn friend_execute_action_active(
        &self,
        action: &crate::scripting::core::ScriptAction,
        team_name: Option<&str>,
        saved_team: Option<String>,
        saved_player: Option<String>,
    ) {
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

    /// Owned snapshot — never a borrow into `UnsafeCell`.
    pub fn get_condition_team_name(&self) -> Option<String> {
        self.with_inner(|inner| inner.condition_team.clone())
    }

    /// Set temporary runtime context used by external script evaluation helpers.
    ///
    /// Returns the previous `(current_player, condition_team)` tuple so callers can restore it.
    pub fn set_external_eval_context(
        &mut self,
        current_player: Option<String>,
        condition_team: Option<String>,
    ) -> (Option<String>, Option<String>) {
        let inner = self.inner.get_mut();
        let saved = (inner.current_player.clone(), inner.condition_team.clone());
        inner.current_player = current_player;
        inner.condition_team = condition_team;
        saved
    }

    /// Restore runtime context previously returned by `set_external_eval_context`.
    pub fn restore_external_eval_context(&mut self, saved: (Option<String>, Option<String>)) {
        let inner = self.inner.get_mut();
        inner.current_player = saved.0;
        inner.condition_team = saved.1;
    }

    pub fn set_frame_object_count_changed(&mut self, frame: u32) {
        let inner = self.inner.get_mut();
        inner.frame_object_count_changed = frame;
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
        if self
            .with_current_active_script_lists(|active_lists| {
                active_lists.lists[player_index] = script_list.take();
            })
            .is_some()
        {
            return Ok(());
        }
        self.lock_inner_mut().side_script_lists[player_index] = script_list;
        Ok(())
    }

    pub fn clear_script_lists(&mut self) {
        if self
            .with_current_active_script_lists(|active_lists| {
                for slot in &mut active_lists.lists {
                    *slot = None;
                }
            })
            .is_some()
        {
            return;
        }
        let inner = self.inner.get_mut();
        for slot in &mut inner.side_script_lists {
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
        if let Some(updated) = self.with_current_active_script_lists(|active_lists| {
            let mut updated = false;
            for slot in &mut active_lists.lists {
                let Some(list) = slot.as_deref_mut() else {
                    continue;
                };
                updated |= Self::set_script_active_in_list(list, script_name, active);
            }
            updated
        }) {
            let handler = self.with_inner(|inner| inner.action_handler.clone());
            if let Some(handler) = handler {
                let _ = handler.enable_script(script_name, active);
            }
            return updated;
        }

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
        if let Some(found) = self.with_current_active_script_lists(|active_lists| {
            for slot in &active_lists.lists {
                let Some(list) = slot.as_deref() else {
                    continue;
                };
                if let Some(found) = Self::find_script_clone_in_list(list, name, true) {
                    return Some(found);
                }
            }
            None
        }) {
            return found;
        }

        self.with_inner(|inner| {
            for slot in &inner.side_script_lists {
                let Some(list) = slot.as_deref() else {
                    continue;
                };
                if let Some(found) = Self::find_script_clone_in_list(list, name, true) {
                    return Some(found);
                }
            }
            None
        })
    }

    fn group_at_in_list(list: &ScriptList, group_index: usize) -> Option<&ScriptGroup> {
        let mut group = list.first_group.as_deref();
        for _ in 0..group_index {
            group = group?.next_group.as_deref();
        }
        group
    }

    fn group_at_in_list_mut(list: &mut ScriptList, group_index: usize) -> Option<&mut ScriptGroup> {
        let mut group = list.first_group.as_deref_mut();
        for _ in 0..group_index {
            group = group?.next_group.as_deref_mut();
        }
        group
    }

    fn script_head_in_container(list: &ScriptList, container: ScriptContainer) -> Option<&Script> {
        match container {
            ScriptContainer::Root => list.first_script.as_deref(),
            ScriptContainer::Group(group_index) => Self::group_at_in_list(list, group_index)?
                .first_script
                .as_deref(),
        }
    }

    fn script_head_in_container_mut(
        list: &mut ScriptList,
        container: ScriptContainer,
    ) -> Option<&mut Option<Box<Script>>> {
        match container {
            ScriptContainer::Root => Some(&mut list.first_script),
            ScriptContainer::Group(group_index) => {
                Some(&mut Self::group_at_in_list_mut(list, group_index)?.first_script)
            }
        }
    }

    fn script_at_in_chain(mut script: Option<&Script>, script_index: usize) -> Option<&Script> {
        for _ in 0..script_index {
            script = script?.next_script.as_deref();
        }
        script
    }

    fn script_link_at_mut(
        head: &mut Option<Box<Script>>,
        script_index: usize,
    ) -> Option<&mut Option<Box<Script>>> {
        let mut link = head;
        for _ in 0..script_index {
            link = &mut link.as_deref_mut()?.next_script;
        }
        Some(link)
    }

    fn script_count_in_chain(mut script: Option<&Script>) -> usize {
        let mut count = 0;
        while let Some(current) = script {
            count += 1;
            script = current.next_script.as_deref();
        }
        count
    }

    fn detached_before(active_lists: &ActiveScriptLists, location: ScriptLocation) -> usize {
        active_lists
            .executing_scripts
            .iter()
            .filter(|executing| {
                executing.location.side_index == location.side_index
                    && executing.location.container == location.container
                    && executing.location.script_index < location.script_index
            })
            .count()
    }

    fn is_script_detached(active_lists: &ActiveScriptLists, location: ScriptLocation) -> bool {
        active_lists
            .executing_scripts
            .iter()
            .any(|executing| executing.location == location)
    }

    fn stored_script_at_location<'a>(
        active_lists: &'a ActiveScriptLists,
        location: ScriptLocation,
    ) -> Option<&'a Script> {
        if Self::is_script_detached(active_lists, location) {
            return None;
        }
        let list = active_lists.lists.get(location.side_index)?.as_deref()?;
        let script_index = location
            .script_index
            .checked_sub(Self::detached_before(active_lists, location))?;
        Self::script_at_in_chain(
            Self::script_head_in_container(list, location.container),
            script_index,
        )
    }

    fn script_count_in_container(
        active_lists: &ActiveScriptLists,
        side_index: usize,
        container: ScriptContainer,
    ) -> usize {
        let stored = active_lists
            .lists
            .get(side_index)
            .and_then(|slot| slot.as_deref())
            .map(|list| {
                Self::script_count_in_chain(Self::script_head_in_container(list, container))
            })
            .unwrap_or(0);
        let detached = active_lists
            .executing_scripts
            .iter()
            .filter(|executing| {
                executing.location.side_index == side_index
                    && executing.location.container == container
            })
            .count();
        stored + detached
    }

    fn take_script_at_location(
        active_lists: &mut ActiveScriptLists,
        location: ScriptLocation,
    ) -> Option<Box<Script>> {
        if Self::is_script_detached(active_lists, location) {
            return None;
        }
        let stored_index = location
            .script_index
            .checked_sub(Self::detached_before(active_lists, location))?;
        let list = active_lists
            .lists
            .get_mut(location.side_index)?
            .as_deref_mut()?;
        let link = Self::script_link_at_mut(
            Self::script_head_in_container_mut(list, location.container)?,
            stored_index,
        )?;
        let mut script = link.take()?;
        *link = script.next_script.take();
        Some(script)
    }

    fn restore_script_at_location(
        active_lists: &mut ActiveScriptLists,
        location: ScriptLocation,
        mut script: Box<Script>,
    ) -> bool {
        let Some(stored_index) = location
            .script_index
            .checked_sub(Self::detached_before(active_lists, location))
        else {
            return false;
        };
        let Some(list) = active_lists
            .lists
            .get_mut(location.side_index)
            .and_then(|slot| slot.as_deref_mut())
        else {
            return false;
        };
        let Some(link) = Self::script_link_at_mut(
            match Self::script_head_in_container_mut(list, location.container) {
                Some(head) => head,
                None => return false,
            },
            stored_index,
        ) else {
            return false;
        };
        script.next_script = link.take();
        *link = Some(script);
        true
    }

    fn find_script_in_chain(
        active_lists: &ActiveScriptLists,
        side_index: usize,
        container: ScriptContainer,
        name: &str,
    ) -> Option<SubroutineLookup> {
        let script_count = Self::script_count_in_container(active_lists, side_index, container);
        for script_index in 0..script_count {
            let location = ScriptLocation {
                side_index,
                container,
                script_index,
            };
            if let Some(executing) = active_lists
                .executing_scripts
                .iter()
                .find(|executing| executing.location == location)
            {
                if executing.name == name {
                    return Some(SubroutineLookup::ReentrantScript {
                        location,
                        is_subroutine: executing.is_subroutine,
                    });
                }
                continue;
            }
            let Some(script) = Self::stored_script_at_location(active_lists, location) else {
                continue;
            };
            if script.script_name == name {
                return Some(SubroutineLookup::Script {
                    location,
                    is_subroutine: script.is_subroutine,
                });
            }
        }
        None
    }

    fn find_subroutine_lookup(&self, name: &str) -> GameLogicResult<SubroutineLookup> {
        self.with_current_active_script_lists(|active_lists| {
            // C++ `findGroup` scans all sides before `callSubroutine` ever
            // calls `findScript`; retain that global precedence exactly.
            for side_index in 0..active_lists.lists.len() {
                let Some(list) = active_lists.lists[side_index].as_deref() else {
                    continue;
                };
                let mut group = list.first_group.as_deref();
                let mut group_index = 0;
                while let Some(current) = group {
                    if current.group_name == name {
                        return SubroutineLookup::Group {
                            location: GroupLocation {
                                side_index,
                                group_index,
                            },
                            is_subroutine: current.is_group_subroutine,
                            is_active: current.is_group_active,
                        };
                    }
                    group = current.next_group.as_deref();
                    group_index += 1;
                }
            }

            // C++ `findScript` scans root scripts then group scripts for each
            // side, with sides in ascending index order.
            for side_index in 0..active_lists.lists.len() {
                let Some(list) = active_lists.lists[side_index].as_deref() else {
                    continue;
                };
                if let Some(found) = Self::find_script_in_chain(
                    active_lists,
                    side_index,
                    ScriptContainer::Root,
                    name,
                ) {
                    return found;
                }

                let mut group = list.first_group.as_deref();
                let mut group_index = 0;
                while let Some(current) = group {
                    if let Some(found) = Self::find_script_in_chain(
                        active_lists,
                        side_index,
                        ScriptContainer::Group(group_index),
                        name,
                    ) {
                        return found;
                    }
                    group = current.next_group.as_deref();
                    group_index += 1;
                }
            }

            SubroutineLookup::Missing
        })
        .ok_or_else(|| {
            GameLogicError::Configuration(
                "CALL_SUBROUTINE was invoked without this ScriptEngine's active list scope"
                    .to_string(),
            )
        })
    }

    fn with_detached_script<R>(
        &self,
        location: ScriptLocation,
        f: impl FnOnce(&mut Script) -> GameLogicResult<R>,
    ) -> GameLogicResult<R> {
        if !ACTIVE_SCRIPT_LISTS.is_set() {
            return Err(GameLogicError::Configuration(
                "immediate Script execution has no active list scope".to_string(),
            ));
        }
        ACTIVE_SCRIPT_LISTS.with(|active_lists| {
            if active_lists.borrow().owner_id != self.instance_id {
                return Err(GameLogicError::Configuration(
                    "immediate Script execution reached a different ScriptEngine's list scope"
                        .to_string(),
                ));
            }
            let mut script = DetachedScript::take(active_lists, location).ok_or_else(|| {
                GameLogicError::Configuration(format!(
                    "Script at side {}, location {:?} disappeared during immediate execution",
                    location.side_index, location
                ))
            })?;
            f(script.script_mut())
        })
    }

    fn with_executing_subroutine_group<R>(
        &self,
        location: GroupLocation,
        f: impl FnOnce() -> GameLogicResult<R>,
    ) -> GameLogicResult<R> {
        if !ACTIVE_SCRIPT_LISTS.is_set() {
            return Err(GameLogicError::Configuration(
                "subroutine group execution has no active list scope".to_string(),
            ));
        }
        ACTIVE_SCRIPT_LISTS.with(|active_lists| {
            let mut active_lists_mut = active_lists.borrow_mut();
            if active_lists_mut.owner_id != self.instance_id {
                return Err(GameLogicError::Configuration(
                    "subroutine group execution reached a different ScriptEngine's list scope"
                        .to_string(),
                ));
            }
            if active_lists_mut.executing_groups.contains(&location) {
                return Err(GameLogicError::Configuration(format!(
                    "recursive CALL_SUBROUTINE group at side {}, group {} is rejected by the safe re-entry boundary",
                    location.side_index, location.group_index
                )));
            }
            active_lists_mut.executing_groups.push(location);
            drop(active_lists_mut);
            let _executing_group = ExecutingGroupGuard {
                active_lists,
                location,
            };
            f()
        })
    }

    fn script_slot_at(&self, location: ScriptLocation) -> Option<(bool, bool)> {
        self.with_current_active_script_lists(|active_lists| {
            if let Some(executing) = active_lists
                .executing_scripts
                .iter()
                .find(|executing| executing.location == location)
            {
                return Some((executing.is_subroutine, true));
            }
            Self::stored_script_at_location(active_lists, location)
                .map(|script| (script.is_subroutine, false))
        })
        .flatten()
    }

    fn group_state_at(&self, location: GroupLocation) -> Option<(bool, bool)> {
        self.with_current_active_script_lists(|active_lists| {
            let list = active_lists.lists.get(location.side_index)?.as_deref()?;
            let group = Self::group_at_in_list(list, location.group_index)?;
            Some((group.is_group_active, group.is_group_subroutine))
        })
        .flatten()
    }

    /// Executes an entire linked Script chain without borrowing the chain over
    /// dispatch.  Each Script is detached/reinserted before its action chain,
    /// so sibling and other-side subroutines stay visible immediately.
    fn execute_non_subroutine_scripts_in_container(
        &self,
        side_index: usize,
        container: ScriptContainer,
        condition_evaluator: &mut crate::scripting::executor::ScriptConditionEvaluator,
        action_dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
    ) -> GameLogicResult<()> {
        let mut script_index = 0;
        while let Some((is_subroutine, is_executing)) = self.script_slot_at(ScriptLocation {
            side_index,
            container,
            script_index,
        }) {
            if !is_subroutine && !is_executing {
                self.with_detached_script(
                    ScriptLocation {
                        side_index,
                        container,
                        script_index,
                    },
                    |script| self.execute_script(script, condition_evaluator, action_dispatcher),
                )?;
            }
            script_index += 1;
        }
        Ok(())
    }

    fn execute_script_at_location(
        &self,
        location: ScriptLocation,
        condition_evaluator: &mut crate::scripting::executor::ScriptConditionEvaluator,
        action_dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
    ) -> GameLogicResult<()> {
        self.with_detached_script(location, |script| {
            self.execute_script(script, condition_evaluator, action_dispatcher)
        })
    }

    /// Execute a subroutine script or subroutine group by name.
    ///
    /// Matches C++ `ScriptEngine::callSubroutine`: group lookup by name takes precedence over
    /// direct script lookup by name.
    ///
    /// Installs this lexical active engine so nested CALL_SUBROUTINE / flag /
    /// timer mutations can re-enter without taking the global mutex again.
    pub fn execute_subroutine_by_name(&self, name: &str) -> GameLogicResult<bool> {
        self.with_active(|| self.execute_subroutine_by_name_active(name))
    }

    fn execute_subroutine_by_name_active(&self, name: &str) -> GameLogicResult<bool> {
        self.with_active_script_lists(|| self.execute_subroutine_by_name_from_active_lists(name))
    }

    fn execute_subroutine_by_name_from_active_lists(&self, name: &str) -> GameLogicResult<bool> {
        let Some(_depth_guard) = SubroutineDepthGuard::enter() else {
            return Ok(false);
        };
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

        match self.find_subroutine_lookup(name)? {
            SubroutineLookup::Group {
                location,
                is_subroutine,
                is_active,
            } => {
                if !is_subroutine {
                    log::warn!(
                        "CALL_SUBROUTINE: group '{}' exists but is not a subroutine group",
                        name
                    );
                    return Ok(true);
                }
                if is_active {
                    self.with_executing_subroutine_group(location, || {
                        self.execute_non_subroutine_scripts_in_container(
                            location.side_index,
                            ScriptContainer::Group(location.group_index),
                            &mut condition_evaluator,
                            &mut action_dispatcher,
                        )
                    })?;
                }
                Ok(true)
            }
            SubroutineLookup::Script {
                location,
                is_subroutine,
            } => {
                if !is_subroutine {
                    log::warn!(
                        "CALL_SUBROUTINE: script '{}' exists but is not a subroutine",
                        name
                    );
                    return Ok(true);
                }
                self.execute_script_at_location(
                    location,
                    &mut condition_evaluator,
                    &mut action_dispatcher,
                )?;
                Ok(true)
            }
            SubroutineLookup::ReentrantScript {
                location,
                is_subroutine,
            } => {
                if !is_subroutine {
                    log::warn!(
                        "CALL_SUBROUTINE: active script '{}' exists but is not a subroutine",
                        name
                    );
                    return Ok(true);
                }
                Err(GameLogicError::Configuration(format!(
                    "recursive CALL_SUBROUTINE '{}' at side {}, script {} is rejected by the safe re-entry boundary",
                    name, location.side_index, location.script_index
                )))
            }
            SubroutineLookup::Missing => {
                log::warn!("CALL_SUBROUTINE: script '{}' is not defined", name);
                Ok(false)
            }
        }
    }

    /// Find a script by name and return a clone (non-subroutine allowed).
    pub fn find_script_clone_by_name(&self, name: &str) -> Option<Script> {
        if let Some(found) = self.with_current_active_script_lists(|active_lists| {
            for slot in &active_lists.lists {
                let Some(list) = slot.as_deref() else {
                    continue;
                };
                if let Some(found) = Self::find_script_clone_in_list(list, name, false) {
                    return Some(found);
                }
            }
            None
        }) {
            return found;
        }

        self.with_inner(|inner| {
            for slot in &inner.side_script_lists {
                let Some(list) = slot.as_deref() else {
                    continue;
                };
                if let Some(found) = Self::find_script_clone_in_list(list, name, false) {
                    return Some(found);
                }
            }
            None
        })
    }

    pub fn get_object_count(&self, player_index: i32, type_name: &str) -> i32 {
        self.with_inner(|inner| {
            inner
                .object_counts
                .get(&(player_index, type_name.to_string()))
                .copied()
                .unwrap_or(0)
        })
    }

    pub fn set_object_count(&mut self, player_index: i32, type_name: &str, count: i32) {
        let inner = self.inner.get_mut();
        inner
            .object_counts
            .insert((player_index, type_name.to_string()), count);
    }

    /// Get a named ObjectTypes list (matches C++ ScriptEngine::getObjectTypes).
    pub fn get_object_types(&self, name: &str) -> Option<ObjectTypes> {
        self.with_inner(|inner| inner.object_types.get(name).cloned())
    }

    /// Register or replace a named ObjectTypes list.
    pub fn set_object_types(&mut self, name: String, types: ObjectTypes) {
        let inner = self.inner.get_mut();
        inner.object_types.insert(name, types);
    }

    fn ensure_attack_priority_defaults(&mut self) {
        let inner = self.inner.get_mut();
        if inner.attack_priority_info.is_empty() {
            inner.attack_priority_info.push(AttackPriorityInfo::new());
        }
        if inner.num_attack_info == 0 {
            inner.num_attack_info = 1;
        }
    }

    fn find_attack_info_mut(
        &mut self,
        name: &str,
        add_if_missing: bool,
    ) -> Option<&mut AttackPriorityInfo> {
        self.ensure_attack_priority_defaults();
        let inner = self.inner.get_mut();
        let existing_index = (1..inner.num_attack_info).find(|&i| {
            inner
                .attack_priority_info
                .get(i)
                .map(|info| info.name == name)
                .unwrap_or(false)
        });
        if let Some(index) = existing_index {
            return inner.attack_priority_info.get_mut(index);
        }

        if add_if_missing && inner.num_attack_info < MAX_ATTACK_PRIORITIES {
            let mut info = AttackPriorityInfo::new();
            info.name = name.to_string();
            let index = inner.num_attack_info;
            if inner.attack_priority_info.len() <= index {
                inner.attack_priority_info.push(info);
            } else {
                inner.attack_priority_info[index] = info;
            }
            inner.num_attack_info += 1;
            return inner.attack_priority_info.get_mut(index);
        }

        None
    }

    /// Owned snapshot — never a borrow into `UnsafeCell`.
    pub fn get_attack_info(&self, name: &str) -> Option<AttackPriorityInfo> {
        self.with_inner(|inner| {
            if inner.attack_priority_info.is_empty() {
                return None;
            }
            for i in 1..inner.num_attack_info {
                if let Some(info) = inner.attack_priority_info.get(i) {
                    if info.name == name {
                        return Some(info.clone());
                    }
                }
            }
            inner.attack_priority_info.get(0).cloned()
        })
    }

    pub fn set_object_attack_priority_set(&mut self, object_id: ObjectID, set_name: &str) {
        let inner = self.inner.get_mut();
        if object_id == INVALID_ID {
            return;
        }

        if set_name.is_empty() {
            inner.object_attack_priority_sets.remove(&object_id);
            return;
        }

        inner
            .object_attack_priority_sets
            .insert(object_id, set_name.to_string());
    }

    pub fn clear_object_attack_priority_set(&mut self, object_id: ObjectID) {
        let inner = self.inner.get_mut();
        inner.object_attack_priority_sets.remove(&object_id);
    }

    /// Owned snapshot — never a borrow into `UnsafeCell`.
    pub fn get_object_attack_priority_set(&self, object_id: ObjectID) -> Option<String> {
        self.with_inner(|inner| inner.object_attack_priority_sets.get(&object_id).cloned())
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
        let inner = self.inner.get_mut();
        // Initialize action templates (this would normally be done from INI files)
        inner
            .action_templates
            .resize(ScriptActionType::NumItems as usize, ActionTemplate::new());
        inner
            .condition_templates
            .resize(ConditionType::NumItems as usize, ConditionTemplate::new());

        self.seed_template_internal_names();

        // Set up basic templates (in real implementation, this would be loaded from INI)
        self.setup_basic_templates()?;

        Ok(())
    }

    /// Set up basic templates for core actions and conditions
    fn setup_basic_templates(&mut self) -> GameLogicResult<()> {
        let inner = self.inner.get_mut();
        // Victory action
        if let Some(template) = inner
            .action_templates
            .get_mut(ScriptActionType::Victory as usize)
        {
            template.base.ui_name = "Victory".to_string();
            template.base.internal_name = "Victory".to_string();
            template.base.help_text = "Triggers victory for the current player".to_string();
        }

        // Defeat action
        if let Some(template) = inner
            .action_templates
            .get_mut(ScriptActionType::Defeat as usize)
        {
            template.base.ui_name = "Defeat".to_string();
            template.base.internal_name = "Defeat".to_string();
            template.base.help_text = "Triggers defeat for the current player".to_string();
        }

        // Set flag action
        if let Some(template) = inner
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
        if let Some(template) = inner
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
        if let Some(template) = inner
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
        if let Some(template) = inner
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
        if let Some(template) = inner
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
        {
            let inner = self.inner.get_mut();
            // Clear runtime state
            inner.counters.iter_mut().for_each(|c| *c = None);
            inner.num_counters = 1;
            inner.flags.iter_mut().for_each(|f| *f = None);
            inner.num_flags = 1;
            inner.attack_priority_info.clear();
            inner.num_attack_info = 1;

            inner.end_game_timer = -1;
            inner.close_window_timer = -1;
            inner.calling_team = None;
            inner.calling_object = None;
            inner.condition_team = None;
            inner.condition_object = None;
            inner.first_update = true;
            inner.current_player = None;
            inner.skirmish_human_player = None;
            inner.current_track_name.clear();

            inner.fade = TFade::None;
            inner.cur_fade_value = 0.0;
            inner.cur_fade_frame = 0;

            inner.completed_video.clear();
            inner.testing_speech.clear();
            inner.testing_audio.clear();
            inner.ui_interactions.clear();

            for powers in &mut inner.triggered_special_powers {
                powers.clear();
            }
            for powers in &mut inner.midway_special_powers {
                powers.clear();
            }
            for powers in &mut inner.finished_special_powers {
                powers.clear();
            }
            for upgrades in &mut inner.completed_upgrades {
                upgrades.clear();
            }
            for sciences in &mut inner.acquired_sciences {
                sciences.clear();
            }

            inner.topple_directions.clear();
            inner.named_reveals.clear();
            inner.object_types.clear();
            inner.object_attack_priority_sets.clear();
            inner.breeze_info = BreezeInfo::new();
            inner.game_difficulty = crate::player::GameDifficulty::Normal;

            inner.freeze_by_script = false;
            inner.freeze_by_debug = false;
            inner.objects_should_receive_difficulty_bonus = true;
            inner.choose_victim_always_uses_normal = false;
            inner.shown_mp_local_defeat_window = false;

            inner.sequential_scripts.clear();
        }
        self.clear_script_lists();
        let inner = self.inner.get_mut();

        #[cfg(feature = "script_profiling")]
        {
            inner.stats = ScriptStats::default();
        }

        if inner.counters[0].is_none() {
            inner.counters[0] = Some(TCounter::new(String::new()));
        }
        if inner.flags[0].is_none() {
            inner.flags[0] = Some(TFlag::new(String::new()));
        }
        if inner.attack_priority_info.is_empty() {
            inner.attack_priority_info.push(AttackPriorityInfo::new());
        }
    }
}
