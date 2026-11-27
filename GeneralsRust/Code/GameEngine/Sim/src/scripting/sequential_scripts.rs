//! Ordered sequential actions and their per-interpreter queue.
//! C++ ScriptEngine.cpp:7714–8070, 8103–8198.

use super::ScriptActionResult;
use super::program::{Script, ScriptAction, ScriptActionType};
use crate::{GameLogicError, GameLogicResult};
use std::cell::{Cell, RefCell};
use std::num::NonZeroU64;

const INVALID_ID: u32 = 0;
const MAX_SPIN_COUNT: i32 = 20;

#[derive(Debug, Clone)]
pub struct SequentialScript {
    pub team_to_exec_on: Option<u32>,
    pub object_id: u32,
    pub script_to_execute_sequentially: Option<Box<Script>>,
    pub current_instruction: i32,
    /// Zero finishes this run, -1 repeats indefinitely, other values count down.
    pub times_to_loop: i32,
    pub frames_to_wait: i32,
    pub dont_advance_instruction: bool,
    pub next_script_in_sequence: Option<Box<SequentialScript>>,
    // Reconcile the same node across reentrant action dispatch. The identity
    // belongs to the live queue and does not participate in the save format.
    token: Option<NonZeroU64>,
}

impl Default for SequentialScript {
    fn default() -> Self {
        Self::new()
    }
}

impl SequentialScript {
    pub fn new() -> Self {
        Self {
            team_to_exec_on: None,
            object_id: INVALID_ID,
            script_to_execute_sequentially: None,
            current_instruction: -1,
            times_to_loop: 0,
            frames_to_wait: -1,
            dont_advance_instruction: false,
            next_script_in_sequence: None,
            token: None,
        }
    }

    fn advance(&mut self) -> Option<ScriptAction> {
        if self.dont_advance_instruction {
            self.dont_advance_instruction = false;
        } else {
            self.current_instruction += 1;
        }
        let index = usize::try_from(self.current_instruction).ok()?;
        let mut action = self
            .script_to_execute_sequentially
            .as_ref()?
            .action
            .as_deref()?;
        for _ in 0..index {
            action = action.get_next()?;
        }
        Some(action.clone())
    }
}

/// C++ SequentialScript::xfer serializes each head without its successors.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SequentialScriptSnapshot {
    pub team_id: u32,
    pub object_id: u32,
    pub script_name: String,
    pub current_instruction: i32,
    pub times_to_loop: i32,
    pub frames_to_wait: i32,
    pub dont_advance_instruction: bool,
}

#[derive(Clone, Copy)]
pub struct SequentialTarget {
    pub object_id: u32,
    pub team_id: Option<u32>,
}

pub struct SequentialReadiness {
    pub has_ai: bool,
    pub idle: bool,
    pub condition_object: Option<u32>,
}

pub enum SequentialAfterAction {
    Yield,
    Continue,
    RemoveChain,
}

pub trait SequentialDriver {
    type Actors;

    fn observe_actors(
        &mut self,
        target: SequentialTarget,
    ) -> Option<(Self::Actors, SequentialReadiness)>;

    fn execute_action(
        &mut self,
        target: SequentialTarget,
        condition_object: Option<u32>,
        action: &ScriptAction,
    ) -> GameLogicResult<ScriptActionResult>;

    fn observe_after_action(
        &mut self,
        target: SequentialTarget,
        actors: &Self::Actors,
    ) -> SequentialAfterAction;
}

pub struct SequentialScripts {
    heads: RefCell<Vec<SequentialScript>>,
    next_token: Cell<NonZeroU64>,
}

impl Default for SequentialScripts {
    fn default() -> Self {
        Self {
            heads: RefCell::default(),
            next_token: Cell::new(NonZeroU64::MIN),
        }
    }
}

impl SequentialScripts {
    pub fn len(&self) -> usize {
        self.heads.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.heads.borrow().is_empty()
    }

    pub fn clear(&self) {
        self.heads.borrow_mut().clear();
    }

    pub fn append(&self, mut script: SequentialScript) {
        script.current_instruction = -1;
        script.next_script_in_sequence = None;
        script.token = Some(self.allocate_token());
        let mut heads = self.heads.borrow_mut();
        if let Some(head) = heads.iter_mut().find(|head| {
            (script.object_id != INVALID_ID && head.object_id == script.object_id)
                || (script.team_to_exec_on.is_some()
                    && head.team_to_exec_on == script.team_to_exec_on)
        }) {
            let mut tail = &mut head.next_script_in_sequence;
            while let Some(next) = tail {
                tail = &mut next.next_script_in_sequence;
            }
            *tail = Some(Box::new(script));
        } else {
            heads.push(script);
        }
    }

    pub fn has_object(&self, id: u32) -> bool {
        self.heads.borrow().iter().any(|head| head.object_id == id)
    }

    pub fn has_team(&self, id: u32) -> bool {
        self.heads
            .borrow()
            .iter()
            .any(|head| head.team_to_exec_on == Some(id))
    }

    pub fn remove_object(&self, id: u32) {
        self.heads.borrow_mut().retain(|head| head.object_id != id);
    }

    pub fn remove_team(&self, id: u32) {
        self.heads
            .borrow_mut()
            .retain(|head| head.team_to_exec_on != Some(id));
    }

    pub fn wait_for_object(&self, id: u32, frames: i32) {
        if let Some(head) = self
            .heads
            .borrow_mut()
            .iter_mut()
            .find(|head| head.object_id == id)
        {
            head.frames_to_wait = frames;
        }
    }

    pub fn wait_for_team(&self, id: u32, frames: i32) {
        if let Some(head) = self
            .heads
            .borrow_mut()
            .iter_mut()
            .find(|head| head.team_to_exec_on == Some(id))
        {
            head.frames_to_wait = frames;
        }
    }

    pub fn snapshot_heads(&self) -> Vec<SequentialScriptSnapshot> {
        self.heads
            .borrow()
            .iter()
            .map(|head| SequentialScriptSnapshot {
                team_id: head.team_to_exec_on.unwrap_or(INVALID_ID),
                object_id: head.object_id,
                script_name: head
                    .script_to_execute_sequentially
                    .as_ref()
                    .map(|script| script.script_name.clone())
                    .unwrap_or_default(),
                current_instruction: head.current_instruction,
                times_to_loop: head.times_to_loop,
                frames_to_wait: head.frames_to_wait,
                dont_advance_instruction: head.dont_advance_instruction,
            })
            .collect()
    }

    pub fn restore_heads(&self, heads: impl IntoIterator<Item = SequentialScript>) {
        let heads = heads
            .into_iter()
            .map(|mut head| {
                head.token = Some(self.allocate_token());
                head
            })
            .collect();
        self.heads.replace(heads);
    }

    /// Xfer saves the legacy head list through its existing mutable scalar API.
    pub fn legacy_heads_mut(&mut self) -> &mut [SequentialScript] {
        self.heads.get_mut()
    }

    fn allocate_token(&self) -> NonZeroU64 {
        let token = self.next_token.get();
        let next = NonZeroU64::new(token.get().wrapping_add(1)).unwrap_or(NonZeroU64::MIN);
        self.next_token.set(next);
        token
    }

    fn index_of(&self, token: NonZeroU64) -> Option<usize> {
        self.heads
            .borrow()
            .iter()
            .position(|head| head.token == Some(token))
    }

    fn head_at(&self, index: usize) -> Option<(NonZeroU64, SequentialScript)> {
        let mut heads = self.heads.borrow_mut();
        let head = heads.get_mut(index)?;
        let token = *head.token.get_or_insert_with(|| self.allocate_token());
        Some((token, head.clone()))
    }

    fn with_head_mut<R>(
        &self,
        token: NonZeroU64,
        operation: impl FnOnce(&mut SequentialScript) -> R,
    ) -> Option<R> {
        let mut heads = self.heads.borrow_mut();
        heads
            .iter_mut()
            .find(|head| head.token == Some(token))
            .map(operation)
    }

    fn cleanup(&self, token: NonZeroU64, remove_chain: bool) -> bool {
        let Some(index) = self.index_of(token) else {
            return false;
        };
        let mut heads = self.heads.borrow_mut();
        if !remove_chain && let Some(next) = heads[index].next_script_in_sequence.take() {
            heads[index] = *next;
        } else {
            heads.remove(index);
        }
        true
    }

    /// C++ advances a stopped iterator on the current head again in the same
    /// frame if the actor is still idle. Every callback gets released borrows.
    pub fn tick(&self, driver: &mut impl SequentialDriver) -> GameLogicResult<()> {
        let mut index = 0;
        let mut last_index = None;
        let mut spin_count = 0;
        while let Some((token, mut sequence)) = self.head_at(index) {
            spin_count = if last_index == Some(index) {
                spin_count + 1
            } else {
                0
            };
            last_index = Some(index);
            if spin_count > MAX_SPIN_COUNT {
                if let Some(script) = &sequence.script_to_execute_sequentially {
                    log::warn!(
                        "Sequential script '{}' appears to be in an infinite loop",
                        script.script_name
                    );
                }
                index += 1;
                continue;
            }
            let target = SequentialTarget {
                object_id: sequence.object_id,
                team_id: sequence.team_to_exec_on,
            };
            let actors = sequence
                .script_to_execute_sequentially
                .as_ref()
                .and_then(|_| driver.observe_actors(target));
            let Some((actors, readiness)) = actors else {
                if !self.cleanup(token, false) {
                    index += 1;
                }
                continue;
            };
            if !readiness.has_ai {
                index += 1;
                continue;
            }
            if !(sequence.frames_to_wait == 0 || (readiness.idle && sequence.frames_to_wait < 1)) {
                if sequence.frames_to_wait > 0 {
                    self.with_head_mut(token, |head| {
                        if head.frames_to_wait > 0 {
                            head.frames_to_wait -= 1;
                        }
                    });
                }
                index += 1;
                continue;
            }
            let Some(action) = self.with_head_mut(token, |head| {
                let action = head.advance();
                if action.is_some() {
                    head.frames_to_wait = -1;
                }
                action
            }) else {
                continue;
            };
            let Some(action) = action else {
                if sequence.times_to_loop != 0 {
                    if sequence.times_to_loop != -1 {
                        sequence.times_to_loop -= 1;
                    }
                    sequence.frames_to_wait = -1;
                    self.append(sequence);
                }
                self.cleanup(token, false);
                continue;
            };

            let result = driver.execute_action(target, readiness.condition_object, &action)?;
            let Some(current_index) = self.index_of(token) else {
                log::debug!(
                    "Sequential script node {token} was removed during immediate action dispatch"
                );
                continue;
            };
            index = current_index;
            match result {
                ScriptActionResult::Success => {}
                ScriptActionResult::Pending(frames) => {
                    let repeat = repeats_instruction(action.action_type);
                    let wait = pending_wait_frames(frames, repeat);
                    self.with_head_mut(token, |head| {
                        head.dont_advance_instruction = repeat;
                        head.frames_to_wait = wait;
                    });
                }
                ScriptActionResult::Failed(message) => {
                    return Err(GameLogicError::Configuration(format!(
                        "Sequential script action failed: {message}"
                    )));
                }
            }
            if self.heads.borrow()[index].dont_advance_instruction {
                index += 1;
                continue;
            }
            match driver.observe_after_action(target, &actors) {
                SequentialAfterAction::Yield => index += 1,
                SequentialAfterAction::Continue => {}
                SequentialAfterAction::RemoveChain => {
                    self.cleanup(token, true);
                }
            }
        }
        Ok(())
    }
}

pub fn repeats_instruction(action: ScriptActionType) -> bool {
    matches!(
        action,
        ScriptActionType::SkirmishWaitForCommandbuttonAvailableAll
            | ScriptActionType::SkirmishWaitForCommandbuttonAvailablePartial
            | ScriptActionType::TeamWaitForNotContainedAll
            | ScriptActionType::TeamWaitForNotContainedPartial
    )
}

pub fn is_sequential_only_action(action: ScriptActionType) -> bool {
    repeats_instruction(action)
        || matches!(
            action,
            ScriptActionType::TeamGuardForFramecount
                | ScriptActionType::TeamIdleForFramecount
                | ScriptActionType::TeamSpinForFramecount
                | ScriptActionType::UnitGuardForFramecount
                | ScriptActionType::UnitIdleForFramecount
        )
}

pub fn pending_wait_frames(pending_frames: f32, repeat: bool) -> i32 {
    let wait = pending_frames.max(0.0).ceil() as i32;
    if repeat {
        wait.saturating_sub(1)
    } else {
        wait.max(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ReplaceDuringAction<'a> {
        queue: &'a SequentialScripts,
        actions: usize,
    }

    impl SequentialDriver for ReplaceDuringAction<'_> {
        type Actors = ();

        fn observe_actors(
            &mut self,
            target: SequentialTarget,
        ) -> Option<((), SequentialReadiness)> {
            Some((
                (),
                SequentialReadiness {
                    has_ai: self.actions == 0,
                    idle: true,
                    condition_object: Some(target.object_id),
                },
            ))
        }

        fn execute_action(
            &mut self,
            _target: SequentialTarget,
            _condition_object: Option<u32>,
            _action: &ScriptAction,
        ) -> GameLogicResult<ScriptActionResult> {
            self.actions += 1;
            let mut replacement = sequence("Replacement");
            replacement.frames_to_wait = 7;
            self.queue.restore_heads([replacement]);
            Ok(ScriptActionResult::Pending(1.0))
        }

        fn observe_after_action(&mut self, _: SequentialTarget, _: &()) -> SequentialAfterAction {
            panic!("a removed node has no post-action observation");
        }
    }

    fn sequence(name: &str) -> SequentialScript {
        let mut program = Script::new();
        program.script_name = name.into();
        program.action = Some(Box::new(ScriptAction::new(ScriptActionType::SetFlag)));
        SequentialScript {
            object_id: 17,
            script_to_execute_sequentially: Some(Box::new(program)),
            ..SequentialScript::new()
        }
    }

    #[test]
    fn reentrant_restore_does_not_apply_the_removed_heads_pending_result() {
        let queue = SequentialScripts::default();
        queue.append(sequence("Original"));
        let mut driver = ReplaceDuringAction {
            queue: &queue,
            actions: 0,
        };
        queue.tick(&mut driver).unwrap();
        assert_eq!(driver.actions, 1);
        let heads = queue.snapshot_heads();
        assert_eq!(heads[0].script_name, "Replacement");
        assert_eq!(heads[0].current_instruction, -1);
        assert_eq!(heads[0].frames_to_wait, 7);
    }
}
