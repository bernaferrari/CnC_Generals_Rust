//! State and execution scopes of one authored-script interpreter.

use super::attack_priorities::AttackPriorityTable;
use super::events::{ScriptEventHistory, ScriptEvents};
use super::frame::ScriptFrame;
use super::program::Script;
use super::program_execution::{self, ScriptProgramDriver};
use super::sequential_scripts::{SequentialDriver, SequentialScript, SequentialScripts};
use super::side_scripts::SideScripts;
use crate::GameLogicResult;
use std::cell::{Cell, Ref, RefCell, RefMut};

/// The receiving world resolves each side before the runtime walks its scripts.
pub trait SideScriptDriver {
    fn side_player(&mut self, side: usize) -> Option<String>;
    fn execute_side_script(&mut self, script: &mut Script) -> GameLogicResult<()>;
}

#[derive(Default)]
struct Invocation {
    current_player: RefCell<Option<String>>,
    calling_team: Cell<Option<u32>>,
    calling_object: Cell<Option<u32>>,
    condition_team: Cell<Option<u32>>,
    condition_object: Cell<Option<u32>>,
}

struct RestoreOnDrop<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> Drop for RestoreOnDrop<F> {
    fn drop(&mut self) {
        if let Some(restore) = self.0.take() {
            restore();
        }
    }
}

/// Per-session script execution state. No state borrow is retained across a
/// driver call, so synchronous subroutines and team callbacks can re-enter.
pub struct ScriptInterpreter {
    invocation: Invocation,
    frame: RefCell<ScriptFrame>,
    attack_priorities: RefCell<AttackPriorityTable>,
    object_count_changed_frame: Cell<u32>,
    sides: SideScripts,
    sequential: SequentialScripts,
    events: ScriptEvents,
}

impl ScriptInterpreter {
    pub fn new(player_slots: usize) -> Self {
        let mut frame = ScriptFrame::default();
        frame.variables.reserve_empty_slots();
        let mut attack_priorities = AttackPriorityTable::new();
        attack_priorities.ensure_default();
        Self {
            invocation: Invocation::default(),
            frame: RefCell::new(frame),
            attack_priorities: RefCell::new(attack_priorities),
            object_count_changed_frame: Cell::new(0),
            sides: SideScripts::new(player_slots),
            sequential: SequentialScripts::default(),
            events: ScriptEvents::new(player_slots),
        }
    }

    pub fn reset(&mut self) {
        self.events.clear();
        self.frame.get_mut().reset();
        self.invocation = Invocation::default();
        self.sequential.clear();
        self.sides.clear();
        let priorities = self.attack_priorities.get_mut();
        priorities.clear();
        priorities.ensure_default();
    }

    /// The original map intro clears frame and event state while keeping scripts.
    pub fn new_map(&mut self) {
        self.frame.get_mut().new_map();
        self.events.clear();
    }

    pub fn frame(&self) -> Ref<'_, ScriptFrame> {
        self.frame.borrow()
    }

    pub fn frame_mut(&self) -> RefMut<'_, ScriptFrame> {
        self.frame.borrow_mut()
    }

    pub(super) fn try_frame_mut(&self) -> Result<RefMut<'_, ScriptFrame>, std::cell::BorrowMutError> {
        self.frame.try_borrow_mut()
    }

    /// The legacy reader skips execution while an exclusive state loan is live.
    pub fn can_read_state(&self) -> bool {
        self.frame.try_borrow().is_ok() && self.attack_priorities.try_borrow().is_ok()
    }

    pub fn attack_priorities(&self) -> Ref<'_, AttackPriorityTable> {
        self.attack_priorities.borrow()
    }

    pub fn attack_priorities_mut(&self) -> RefMut<'_, AttackPriorityTable> {
        self.attack_priorities.borrow_mut()
    }

    pub fn object_count_changed_frame(&self) -> u32 {
        self.object_count_changed_frame.get()
    }

    pub fn set_object_count_changed_frame(&self, frame: u32) {
        self.object_count_changed_frame.set(frame);
    }

    pub fn sides(&self) -> &SideScripts {
        &self.sides
    }

    pub fn clear_script_lists(&mut self) {
        self.sides.clear();
    }

    pub fn sequential(&self) -> &SequentialScripts {
        &self.sequential
    }

    pub fn events(&self) -> &ScriptEvents {
        &self.events
    }

    /// Direct field loans for the positional Xfer codec.
    pub fn legacy_xfer_fields(
        &mut self,
    ) -> (
        &mut ScriptFrame,
        &mut AttackPriorityTable,
        &mut ScriptEventHistory,
    ) {
        (
            self.frame.get_mut(),
            self.attack_priorities.get_mut(),
            self.events.legacy_history_mut(),
        )
    }

    pub fn legacy_sequence_heads(&mut self) -> &mut [SequentialScript] {
        self.sequential.legacy_heads_mut()
    }

    pub fn current_player(&self) -> Option<String> {
        self.invocation.current_player.borrow().clone()
    }

    pub fn set_current_player(&self, player: Option<String>) {
        self.invocation.current_player.replace(player);
    }

    pub fn calling_team(&self) -> Option<u32> {
        self.invocation.calling_team.get()
    }

    pub fn condition_team(&self) -> Option<u32> {
        self.invocation.condition_team.get()
    }

    /// THIS_OBJECT prefers the calling object over the condition object.
    pub fn script_object_id(&self) -> Option<u32> {
        self.invocation
            .calling_object
            .get()
            .or(self.invocation.condition_object.get())
    }

    pub fn set_condition_team(&self, team: Option<u32>) {
        self.invocation.condition_team.set(team);
    }

    pub fn set_condition_target(&self, team: Option<u32>, object: Option<u32>) {
        self.set_condition_team(team);
        self.invocation.condition_object.set(object);
    }

    pub fn swap_evaluation_context(
        &self,
        player: Option<String>,
        team: Option<u32>,
    ) -> (Option<String>, Option<u32>) {
        (
            self.invocation.current_player.replace(player),
            self.invocation.condition_team.replace(team),
        )
    }

    /// C++ evaluateConditions and friend_executeAction latch the calling team
    /// and player. The condition selectors have a separate lifetime.
    pub fn enter_team_call(&self, team: Option<u32>, player: Option<String>) -> impl Drop + '_ {
        let restore = self.preserve_team_call();
        self.set_team_call(team, player);
        restore
    }

    pub fn preserve_team_call(&self) -> impl Drop + '_ {
        let team = self.calling_team();
        let player = self.current_player();
        RestoreOnDrop(Some(move || {
            self.set_team_call(team, player);
        }))
    }

    pub fn set_team_call(&self, team: Option<u32>, player: Option<String>) {
        self.invocation.calling_team.set(team);
        self.set_current_player(player);
    }

    pub fn enter_object_call(&self, id: u32) -> impl Drop + '_ {
        let previous = self.invocation.calling_object.replace(Some(id));
        RestoreOnDrop(Some(move || self.invocation.calling_object.set(previous)))
    }

    pub fn clear_destroyed_object(&self, id: u32) {
        if self.invocation.calling_object.get() == Some(id) {
            self.invocation.calling_object.set(None);
        }
        if self.invocation.condition_object.get() == Some(id) {
            self.invocation.condition_object.set(None);
        }
    }

    pub fn remove_team_sequences(&self, id: u32) {
        self.sequential.remove_team(id);
        if self.calling_team() == Some(id) {
            self.invocation.calling_team.set(None);
        }
        if self.condition_team() == Some(id) {
            self.set_condition_team(None);
        }
    }

    /// ScriptEngine.cpp:5554–5573 binds players in side-slot order and clears
    /// the binding before the team phase. A failed walk restores its caller.
    pub fn execute_sides(&self, driver: &mut impl SideScriptDriver) -> GameLogicResult<()> {
        let result = {
            let player = self.current_player();
            let _restore = RestoreOnDrop(Some(move || self.set_current_player(player)));
            self.sides.with_execution(|| {
                for side in 0..self.sides.side_count() {
                    self.set_current_player(driver.side_player(side));
                    self.sides
                        .execute_side(side, |script| driver.execute_side_script(script))?;
                }
                Ok(())
            })
        };
        if result.is_ok() {
            self.set_current_player(None);
        }
        result
    }

    pub fn execute_script(
        &self,
        script: &mut Script,
        frame: u32,
        driver: &mut impl ScriptProgramDriver,
    ) -> GameLogicResult<()> {
        let condition = self.condition_team();
        let _restore = RestoreOnDrop(Some(move || self.set_condition_team(condition)));
        program_execution::execute_script(script, frame, driver)
    }

    pub fn advance_sequences(&self, driver: &mut impl SequentialDriver) -> GameLogicResult<()> {
        let player = self.current_player();
        let team = self.condition_team();
        let object = self.invocation.condition_object.get();
        let _restore = RestoreOnDrop(Some(move || {
            self.set_current_player(player);
            self.set_condition_target(team, object);
        }));
        self.sequential.tick(driver)
    }
}
