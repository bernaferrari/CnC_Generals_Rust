use super::*;

fn delay_frames(seconds: i32) -> u64 {
    if seconds <= 0 {
        1
    } else {
        (seconds as u64 * 30).max(1)
    }
}

impl MissionScriptRuntime {
    pub fn update(&mut self, current_frame: u64) -> GameLogicResult<()> {
        self.update_budgeted(current_frame, None)
    }

    pub fn update_budgeted(
        &mut self,
        current_frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        self.update_budgeted_internal(current_frame, max_scripts_per_frame)
    }

    pub(super) fn update_budgeted_internal(
        &mut self,
        current_frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        if self.scripts.is_empty() {
            return Ok(());
        }
        self.frame_counter = current_frame;
        self.evaluator
            .sync_host_trigger_flags_from_snapshot(current_frame as u32);

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

    pub fn set_script_enabled(&mut self, name: &str, enabled: bool) -> GameLogicResult<()> {
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

    pub(super) fn apply_pending_script_enabled_updates(&mut self) -> GameLogicResult<()> {
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

    pub(super) fn set_runtime_script_active(&mut self, script_index: usize, enabled: bool) {
        let entry = &mut self.scripts[script_index];
        entry.enabled = enabled;
        entry.script.set_active(enabled);
        if enabled {
            entry.state.completed = false;
            entry.state.next_frame_allowed = self.frame_counter;
        }
    }

    pub(super) fn collect_chain(
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

    pub(super) fn is_regular_script_eligible(&self, script: &RuntimeScript) -> bool {
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
    pub(super) fn update_full_cxx_order(&mut self) -> GameLogicResult<()> {
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

    pub(super) fn evaluate_script(
        &mut self,
        index: usize,
        group_is_eligible: bool,
    ) -> GameLogicResult<()> {
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
