//! Continuation of the executing side lists, in ScriptList::xfer order.
use super::{GameLogicError, GameLogicResult, Script, ScriptEngine, ScriptGroup, ScriptList};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SideScriptContinuation(Vec<Option<List>>);

#[derive(Debug, Serialize, Deserialize)]
struct List {
    roots: Vec<ScriptState>,
    groups: Vec<GroupState>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GroupState {
    name: String,
    active: bool,
    children: Vec<ScriptState>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScriptState {
    name: String,
    active: bool,
    // The original stores the active flag alone. A native save also retains
    // the evaluation deadline so periodic scripts keep their phase.
    evaluate_at: u32,
}

fn scripts(head: Option<&Script>) -> impl Iterator<Item = &Script> {
    std::iter::successors(head, |script| script.get_next())
}

fn groups(head: Option<&ScriptGroup>) -> impl Iterator<Item = &ScriptGroup> {
    std::iter::successors(head, |group| group.get_next())
}

fn capture_scripts(head: Option<&Script>) -> Vec<ScriptState> {
    scripts(head)
        .map(|script| ScriptState {
            name: script.get_name().to_owned(),
            active: script.is_active(),
            evaluate_at: script.frame_to_evaluate_at,
        })
        .collect()
}

fn matches_scripts(saved: &[ScriptState], head: Option<&Script>) -> bool {
    let mut live = scripts(head);
    saved.iter().all(|entry| {
        live.next()
            .is_some_and(|script| entry.name == script.get_name())
    }) && live.next().is_none()
}

fn restore_scripts(saved: &[ScriptState], mut head: Option<&mut Script>) {
    for entry in saved {
        let script = head.expect("validated script order");
        script.set_active(entry.active);
        script.frame_to_evaluate_at = entry.evaluate_at;
        head = script.next_script.as_deref_mut();
    }
}

impl List {
    fn capture(list: &ScriptList) -> Self {
        Self {
            roots: capture_scripts(list.get_script()),
            groups: groups(list.get_script_group())
                .map(|group| GroupState {
                    name: group.get_name().to_owned(),
                    active: group.is_active(),
                    children: capture_scripts(group.get_script()),
                })
                .collect(),
        }
    }

    fn matches(&self, list: &ScriptList) -> bool {
        let mut live = groups(list.get_script_group());
        matches_scripts(&self.roots, list.get_script())
            && self.groups.iter().all(|entry| {
                live.next().is_some_and(|group| {
                    entry.name == group.get_name()
                        && matches_scripts(&entry.children, group.get_script())
                })
            })
            && live.next().is_none()
    }

    fn restore(&self, list: &mut ScriptList) {
        restore_scripts(&self.roots, list.first_script.as_deref_mut());
        let mut group = list.first_group.as_deref_mut();
        for entry in &self.groups {
            let current = group.expect("validated group order");
            current.set_active(entry.active);
            restore_scripts(&entry.children, current.first_script.as_deref_mut());
            group = current.next_group.as_deref_mut();
        }
    }
}

impl ScriptEngine {
    fn check_side_script_save_boundary(slot_count: usize) -> GameLogicResult<()> {
        // The synchronous script walk temporarily takes this fixed-size list.
        if slot_count != Self::MAX_PLAYER_COUNT {
            return Err(GameLogicError::Configuration(
                "side scripts are executing during save or restore".into(),
            ));
        }
        Ok(())
    }

    pub fn capture_side_script_continuation(&self) -> GameLogicResult<SideScriptContinuation> {
        self.with_inner(|inner| {
            let lists = &inner.side_script_lists;
            Self::check_side_script_save_boundary(lists.len())?;
            Ok(SideScriptContinuation(
                lists
                    .iter()
                    .map(|list| list.as_deref().map(List::capture))
                    .collect(),
            ))
        })
    }

    pub fn validate_side_script_continuation(
        &self,
        saved: &SideScriptContinuation,
    ) -> GameLogicResult<()> {
        self.with_inner(|inner| {
            let lists = &inner.side_script_lists;
            Self::check_side_script_save_boundary(lists.len())?;
            if lists.len() != saved.0.len() {
                return Err(GameLogicError::Configuration(
                    "saved script slot count differs".into(),
                ));
            }
            for (slot, (saved, live)) in saved.0.iter().zip(lists).enumerate() {
                let valid = match (saved, live) {
                    (Some(saved), Some(live)) => saved.matches(live),
                    (None, None) => true,
                    _ => false,
                };
                if !valid {
                    return Err(GameLogicError::Configuration(format!(
                        "saved script layout differs in side slot {slot}"
                    )));
                }
            }
            Ok(())
        })
    }

    pub fn restore_side_script_continuation(
        &self,
        saved: &SideScriptContinuation,
    ) -> GameLogicResult<()> {
        self.validate_side_script_continuation(saved)?;
        let mut inner = self.lock_inner_mut();
        for (saved, list) in saved.0.iter().zip(&mut inner.side_script_lists) {
            if let (Some(saved), Some(list)) = (saved, list) {
                saved.restore(list);
            }
        }
        Ok(())
    }
}
