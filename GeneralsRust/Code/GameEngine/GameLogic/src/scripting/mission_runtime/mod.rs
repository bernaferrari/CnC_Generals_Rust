//! C++-ordered mission script runtime, driven through explicit match-owned dependencies.
use super::{
    core::{Script, ScriptList},
    evaluator::ScriptEvaluator,
};
use crate::{GameLogicError, GameLogicResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod execution;
#[cfg(test)]
mod tests;

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
    /// Construct an inert runtime around explicit dependencies from its owning match.
    pub fn new(
        evaluator: ScriptEvaluator,
        pending_script_enabled_updates: Arc<Mutex<Vec<(String, bool)>>>,
    ) -> Self {
        Self {
            evaluator,
            scripts: Vec::new(),
            groups: Vec::new(),
            script_lookup: HashMap::new(),
            original_lookup: HashMap::new(),
            group_lookup: HashMap::new(),
            pending_script_enabled_updates,
            frame_counter: 0,
            next_script_index: 0,
        }
    }

    pub fn install_lists(&mut self, lists: &[ScriptList]) {
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
}
