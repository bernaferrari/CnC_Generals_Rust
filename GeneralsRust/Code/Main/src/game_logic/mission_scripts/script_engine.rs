// C++ ownership: ScriptEngine.cpp/Scripts.cpp mission runtime — script/group install, C++-ordered frame walk, condition gating, enable toggles.

#[derive(Debug, Clone)]
struct ScriptState {
    completed: bool,
    next_frame_allowed: u64,
}

impl ScriptState {
    fn new() -> Self {
        Self {
            completed: false,
            next_frame_allowed: 0,
        }
    }
}

#[derive(Clone)]
struct RuntimeScript {
    name: String,
    original_name: Option<String>,
    script: Script,
    state: ScriptState,
    /// `None` means a root ScriptList entry.  A group index preserves C++'s
    /// per-group active gate rather than baking it into the script at load.
    group_index: Option<usize>,
    is_subroutine: bool,
    enabled: bool,
}

/// Runtime identity/state for one C++ `ScriptGroup`.
///
/// `ENABLE_SCRIPT` / `DISABLE_SCRIPT` can name a group.  C++ looks up groups
/// independently from scripts and toggles only the group active bit; members
/// retain their own active/one-shot state.
#[derive(Clone)]
struct RuntimeScriptGroup {
    name: String,
    active: bool,
    is_subroutine: bool,
}

pub struct MissionScriptRuntime {
    evaluator: ScriptEvaluator,
    scripts: Vec<RuntimeScript>,
    groups: Vec<RuntimeScriptGroup>,
    script_lookup: HashMap<String, usize>,
    original_lookup: HashMap<String, usize>,
    group_lookup: HashMap<String, usize>,
    /// Action handlers cannot take the runtime mutex recursively.  They queue
    /// ENABLE/DISABLE requests here; the regular C++-ordered walk applies the
    /// queue immediately after each completed script before visiting the next
    /// declaration.
    pending_script_enabled_updates: Arc<Mutex<Vec<(String, bool)>>>,
    frame_counter: u64,
    next_script_index: usize,
}

impl MissionScriptRuntime {
    fn new() -> GameLogicResult<Self> {
        Self::new_with_pending_script_enabled_updates(Arc::new(Mutex::new(Vec::new())))
    }

    fn new_with_pending_script_enabled_updates(
        pending_script_enabled_updates: Arc<Mutex<Vec<(String, bool)>>>,
    ) -> GameLogicResult<Self> {
        let _ = initialize_script_engine();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine.clone());
        Ok(Self {
            evaluator,
            scripts: Vec::new(),
            groups: Vec::new(),
            script_lookup: HashMap::new(),
            original_lookup: HashMap::new(),
            group_lookup: HashMap::new(),
            pending_script_enabled_updates,
            frame_counter: 0,
            next_script_index: 0,
        })
    }

    fn install_lists(&mut self, lists: &[ScriptList]) {
        self.scripts.clear();
        self.groups.clear();
        self.script_lookup.clear();
        self.original_lookup.clear();
        self.group_lookup.clear();
        self.frame_counter = 0;
        self.next_script_index = 0;

        for (list_index, list) in lists.iter().enumerate() {
            self.collect_chain(
                format!("List{}", list_index),
                list.first_script.as_deref(),
                None,
            );

            let mut group = list.first_group.as_deref();
            let mut group_index = 0usize;
            while let Some(script_group) = group {
                let group_prefix = if script_group.get_name().is_empty() {
                    format!("List{}::Group{}", list_index, group_index)
                } else {
                    format!(
                        "List{}::{}",
                        list_index,
                        script_group.get_name().replace(' ', "_")
                    )
                };
                let runtime_group_index = self.groups.len();
                self.group_lookup
                    .entry(script_group.get_name().to_string())
                    .or_insert(runtime_group_index);
                self.groups.push(RuntimeScriptGroup {
                    name: script_group.get_name().to_string(),
                    active: script_group.is_active(),
                    is_subroutine: script_group.is_subroutine(),
                });
                self.collect_chain(
                    group_prefix,
                    script_group.get_script(),
                    Some(runtime_group_index),
                );
                group = script_group.get_next();
                group_index += 1;
            }
        }

        log::info!(
            "Mission script runtime registered {} WW3D scripts",
            self.scripts.len()
        );
        let enabled_count = self
            .scripts
            .iter()
            .filter(|script| self.is_regular_script_eligible(script) && script.enabled)
            .count();
        log::info!(
            "Mission script runtime has {} frame-eligible scripts at install",
            enabled_count
        );
        for script in self.scripts.iter().filter(|script| {
            self.is_regular_script_eligible(script)
                && (script.name.contains("Move_Camera")
                    || script.original_name.as_deref().is_some_and(|name| {
                        matches!(
                            name.to_ascii_lowercase().as_str(),
                            "move camera"
                                | "restart camera script"
                                | "restart camera"
                                | "restart camera really"
                                | "unshroud"
                                | "turn off sirens"
                        )
                    }))
        }) {
            log::debug!(
                "Mission script install: runtime='{}' original={:?} enabled={} script_active={}",
                script.name,
                script.original_name,
                script.enabled,
                script.script.is_active()
            );
        }
    }

    fn update(&mut self, current_frame: u64) -> GameLogicResult<()> {
        self.update_budgeted(current_frame, None)
    }

    fn update_budgeted(
        &mut self,
        current_frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        self.update_budgeted_internal(current_frame, max_scripts_per_frame)
    }

    fn update_budgeted_internal(
        &mut self,
        current_frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        if self.scripts.is_empty() {
            return Ok(());
        }
        self.frame_counter = current_frame;
        gamelogic::scripting::sync_host_trigger_flags_from_snapshot(current_frame as u32);

        self.apply_pending_script_enabled_updates()?;
        if current_frame <= 2 {
            let enabled: Vec<_> = self
                .scripts
                .iter()
                .filter(|script| self.is_regular_script_eligible(script) && script.enabled)
                .map(|script| script.name.as_str())
                .collect();
            log::debug!(
                "Mission script runtime frame {} enabled scripts sample: {:?}",
                current_frame,
                enabled.into_iter().take(24).collect::<Vec<_>>()
            );
        }
        match max_scripts_per_frame {
            Some(0) => return Ok(()),
            Some(budget) => {
                let len = self.scripts.len();
                let to_evaluate = budget.min(len);
                for _ in 0..to_evaluate {
                    let index = self.next_script_index % len;
                    let group_is_eligible = self.is_regular_script_eligible(&self.scripts[index]);
                    self.evaluate_script(index, group_is_eligible)?;
                    self.apply_pending_script_enabled_updates()?;
                    self.next_script_index = (self.next_script_index + 1) % len;
                }
            }
            None => {
                self.update_full_cxx_order()?;
                self.next_script_index = 0;
            }
        }
        Ok(())
    }

    fn set_script_enabled(&mut self, name: &str, enabled: bool) -> GameLogicResult<()> {
        let script_index = self
            .script_lookup
            .get(name)
            .copied()
            .or_else(|| self.original_lookup.get(name).copied());
        let group_index = self.group_lookup.get(name).copied();

        // C++ ScriptEngine.cpp:6800-6823 finds groups and scripts separately.
        // Keep the mutation order visible to immediate/re-entrant actions:
        // ENABLE toggles group then script; DISABLE toggles script then group.
        if enabled {
            if let Some(group_index) = group_index {
                self.groups[group_index].active = true;
            }
            if let Some(script_index) = script_index {
                self.set_runtime_script_active(script_index, true);
            }
        } else {
            if let Some(script_index) = script_index {
                self.set_runtime_script_active(script_index, false);
            }
            if let Some(group_index) = group_index {
                self.groups[group_index].active = false;
            }
        }

        if let Some(script_index) = script_index {
            log::debug!(
                "Mission script runtime set '{}' enabled={} (runtime='{}')",
                name,
                enabled,
                self.scripts[script_index].name
            );
        }
        if let Some(group_index) = group_index {
            log::debug!(
                "Mission script runtime set group '{}' active={} (runtime='{}')",
                name,
                enabled,
                self.groups[group_index].name
            );
        }
        if script_index.is_none() && group_index.is_none() {
            log::warn!(
                "Enable/Disable requested for unknown script/group '{}'",
                name
            );
        }
        Ok(())
    }

    fn apply_pending_script_enabled_updates(&mut self) -> GameLogicResult<()> {
        let pending = self
            .pending_script_enabled_updates
            .lock()
            .map(|mut queue| queue.drain(..).collect::<Vec<_>>())
            .map_err(|_| {
                GameLogicError::Configuration(
                    "Mission script enable queue mutex poisoned".to_string(),
                )
            })?;
        for (name, enabled) in pending {
            self.set_script_enabled(&name, enabled)?;
        }
        Ok(())
    }

    fn set_runtime_script_active(&mut self, script_index: usize, enabled: bool) {
        let entry = &mut self.scripts[script_index];
        entry.enabled = enabled;
        entry.script.set_active(enabled);
        if enabled {
            entry.state.completed = false;
            entry.state.next_frame_allowed = self.frame_counter;
        }
    }

    fn collect_chain(
        &mut self,
        prefix: String,
        script: Option<&Script>,
        group_index: Option<usize>,
    ) {
        let mut current = script;
        let mut ordinal = 0usize;

        while let Some(node) = current {
            let base = node.get_name().trim();
            let mut name = if base.is_empty() {
                format!("{}::Script{}", prefix, ordinal)
            } else {
                format!("{}::{}", prefix, base.replace(' ', "_"))
            };

            if self.script_lookup.contains_key(&name) {
                let suffix = format!("#{}", self.script_lookup.len());
                name.push_str(&suffix);
            }

            // C++ `findScript` compares its AsciiString name verbatim.  The
            // generated runtime path below may normalize display whitespace,
            // but action lookup must retain authored spelling and case.
            let original_key = if node.get_name().is_empty() {
                None
            } else {
                Some(node.get_name().to_string())
            };

            if let Some(ref key) = original_key {
                self.original_lookup
                    .entry(key.clone())
                    .or_insert(self.scripts.len());
            }

            self.script_lookup.insert(name.clone(), self.scripts.len());
            self.scripts.push(RuntimeScript {
                name,
                original_name: original_key,
                script: node.clone(),
                state: ScriptState::new(),
                group_index,
                is_subroutine: node.is_subroutine(),
                enabled: node.is_active(),
            });

            current = node.get_next();
            ordinal += 1;
        }
    }

    fn is_regular_script_eligible(&self, script: &RuntimeScript) -> bool {
        if script.is_subroutine {
            return false;
        }
        script.group_index.map_or(true, |group_index| {
            self.groups
                .get(group_index)
                .is_some_and(|group| group.active && !group.is_subroutine)
        })
    }

    /// C++ samples an ordinary group's active/subroutine gate when it reaches
    /// that group in `ScriptEngine::update`, then walks the whole chain.  A
    /// member that disables its own group therefore affects the next frame,
    /// not remaining siblings in the already-entered chain.
    fn update_full_cxx_order(&mut self) -> GameLogicResult<()> {
        let mut current_group = None;
        let mut entered_group_is_eligible = true;

        for index in 0..self.scripts.len() {
            let group_index = self.scripts[index].group_index;
            if group_index != current_group {
                current_group = group_index;
                entered_group_is_eligible = group_index.map_or(true, |group_index| {
                    self.groups
                        .get(group_index)
                        .is_some_and(|group| group.active && !group.is_subroutine)
                });
            }

            if !entered_group_is_eligible || self.scripts[index].is_subroutine {
                continue;
            }
            self.evaluate_script(index, true)?;
            self.apply_pending_script_enabled_updates()?;
        }
        Ok(())
    }

    fn evaluate_script(&mut self, index: usize, group_is_eligible: bool) -> GameLogicResult<()> {
        let entry = &mut self.scripts[index];
        if !group_is_eligible || entry.is_subroutine || !entry.enabled || !entry.script.is_active()
        {
            return Ok(());
        }

        if entry.script.is_one_shot() && entry.state.completed {
            return Ok(());
        }

        if self.frame_counter < entry.state.next_frame_allowed {
            return Ok(());
        }

        let condition_result = self.evaluator.evaluate_script(&mut entry.script)?;

        if condition_result && entry.script.is_one_shot() {
            entry.state.completed = true;
        } else {
            entry.state.next_frame_allowed =
                self.frame_counter + delay_frames(entry.script.delay_evaluation_seconds);
        }

        Ok(())
    }
}
