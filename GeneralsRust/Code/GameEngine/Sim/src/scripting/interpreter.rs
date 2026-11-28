//! State and execution scopes of one authored-script interpreter.

use super::attack_priorities::AttackPriorityTable;
use super::driver::ScriptExecutionDriver;
use super::events::{ScriptEventHistory, ScriptEvents};
use super::frame::{ScriptFrame, ScriptFrameDriver};
use super::map_reveals::{NamedReveal, NamedReveals};
use super::presentation::{ScriptActionHandler, ScriptPresentation};
use super::program::Script;
use super::program_execution::{self, ScriptProgramDriver};
use super::sequential_scripts::{SequentialDriver, SequentialScript, SequentialScripts};
use super::side_scripts::SideScripts;
use super::world::ScriptOwnerQuery;
use crate::GameLogicResult;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::sync::Arc;

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
    presentation: ScriptPresentation,
    current_track_name: RefCell<String>,
    named_reveals: RefCell<NamedReveals>,
    topple_directions: RefCell<Vec<(String, glam::Vec3)>>,
    transport_observations: RefCell<HashMap<u32, (u32, usize)>>,
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
            presentation: ScriptPresentation::default(),
            current_track_name: RefCell::new(String::new()),
            named_reveals: RefCell::default(),
            topple_directions: RefCell::default(),
            transport_observations: RefCell::new(HashMap::new()),
        }
    }

    pub fn reset(&mut self) {
        self.events.clear();
        self.presentation.reset();
        self.current_track_name.get_mut().clear();
        self.named_reveals.get_mut().clear();
        self.topple_directions.get_mut().clear();
        self.transport_observations.get_mut().clear();
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

    pub fn set_topple_direction(&self, name: &str, direction: Option<glam::Vec3>) {
        if name.is_empty() {
            return;
        }
        let mut entries = self.topple_directions.borrow_mut();
        match (entries.iter().position(|(key, _)| key == name), direction) {
            (Some(index), Some(direction)) => entries[index].1 = direction,
            (Some(index), None) => {
                entries.remove(index);
            }
            (None, Some(direction)) => entries.insert(0, (name.to_owned(), direction)),
            (None, None) => {}
        }
    }

    pub fn topple_direction_for_name(&self, name: &str) -> Option<glam::Vec3> {
        if name.is_empty() {
            return None;
        }
        self.topple_directions
            .borrow()
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, direction)| {
                if direction.length_squared() > 0.0 {
                    direction.normalize()
                } else {
                    *direction
                }
            })
    }

    /// Iteration order is also the legacy Xfer list order.
    pub fn topple_directions(&self) -> Ref<'_, [(String, glam::Vec3)]> {
        Ref::map(self.topple_directions.borrow(), Vec::as_slice)
    }

    /// The legacy codecs restore the original list without normalizing its coordinates.
    pub fn topple_directions_mut(&self) -> RefMut<'_, Vec<(String, glam::Vec3)>> {
        self.topple_directions.borrow_mut()
    }

    pub fn frame(&self) -> Ref<'_, ScriptFrame> {
        self.frame.borrow()
    }

    pub fn frame_mut(&self) -> RefMut<'_, ScriptFrame> {
        self.frame.borrow_mut()
    }

    pub(super) fn try_frame_mut(
        &self,
    ) -> Result<RefMut<'_, ScriptFrame>, std::cell::BorrowMutError> {
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

    pub fn observe_object_count_changed_frame(&self, world: &dyn ScriptExecutionDriver) {
        match world.object_count_changed_frame() {
            ScriptOwnerQuery::Present(frame) => self.set_object_count_changed_frame(frame),
            ScriptOwnerQuery::Missing => self.set_object_count_changed_frame(0),
            ScriptOwnerQuery::Unavailable => {}
        }
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

    pub fn presentation(&self) -> &ScriptPresentation {
        &self.presentation
    }

    pub fn set_action_handler(&mut self, handler: Option<Arc<dyn ScriptActionHandler>>) {
        self.presentation.set_action_handler(handler);
    }

    pub fn current_track_name(&self) -> String {
        self.current_track_name.borrow().clone()
    }

    pub fn set_current_track_name(&self, name: String) {
        *self.current_track_name.borrow_mut() = name;
    }

    pub fn register_named_reveal(&self, name: &str, waypoint: &str, radius: f32, player: &str) {
        if !self
            .named_reveals
            .borrow_mut()
            .register(name, waypoint, radius, player)
        {
            log::warn!("Attempted to redefine named reveal '{name}'");
        }
    }

    pub fn named_reveal(&self, name: &str) -> Option<NamedReveal> {
        self.named_reveals.borrow().find(name).cloned()
    }

    pub fn remove_named_reveal(&self, name: &str) {
        self.named_reveals.borrow_mut().remove(name);
    }

    pub fn snapshot_named_reveals(&self) -> Vec<(String, String, f32, String)> {
        self.named_reveals.borrow().snapshot()
    }

    pub fn restore_named_reveals(&self, rows: &[(String, String, f32, String)]) {
        self.named_reveals.borrow_mut().restore(rows);
    }

    /// Ordered names for replaying legacy lookers after archive load.
    pub fn named_reveal_names(&self) -> Vec<String> {
        self.named_reveals
            .borrow()
            .entries()
            .iter()
            .map(|row| row.reveal_name.clone())
            .collect()
    }

    /// CPP1945 leaves the previous observation visible on every successful
    /// query in the same logic frame.
    pub fn observe_transport_empty(&self, object: u32, frame: u32, count: usize) -> bool {
        let mut observations = self.transport_observations.borrow_mut();
        let status = observations.entry(object).or_insert((frame, count));
        if status.0 == frame.wrapping_sub(1) && status.1 > 0 && count == 0 {
            return true;
        }
        *status = (frame, count);
        false
    }

    /// The original ScriptConditions transport observations have no Xfer payload.
    pub fn clear_transport_observations(&self) {
        self.transport_observations.borrow_mut().clear();
    }

    /// Direct field loans for the positional Xfer codec.
    pub fn legacy_xfer_fields(
        &mut self,
    ) -> (
        &mut ScriptFrame,
        &mut AttackPriorityTable,
        &mut ScriptEventHistory,
        &mut String,
        &mut NamedReveals,
        &mut Vec<(String, glam::Vec3)>,
    ) {
        (
            self.frame.get_mut(),
            self.attack_priorities.get_mut(),
            self.events.legacy_history_mut(),
            self.current_track_name.get_mut(),
            self.named_reveals.get_mut(),
            self.topple_directions.get_mut(),
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

    /// Team destruction cancels this interpreter's sequence and invocation references.
    pub fn notify_team_destruction(&self, id: u32) {
        if id != game_engine::common::rts::team::TEAM_ID_INVALID {
            self.remove_team_sequences(id);
        }
    }

    /// ScriptEngine.cpp:5554–5573 binds players in side-slot order and clears
    /// the binding before the team phase. A failed walk restores its caller.
    pub(super) fn execute_sides(
        &self,
        frame: u32,
        driver: &mut impl ScriptFrameDriver,
    ) -> GameLogicResult<()> {
        let result = {
            let player = self.current_player();
            let _restore = RestoreOnDrop(Some(move || self.set_current_player(player)));
            self.sides.with_execution(|| {
                for side in 0..self.sides.side_count() {
                    self.set_current_player(driver.side_player(side));
                    self.sides
                        .execute_side(side, |script| self.execute_script(script, frame, driver))?;
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
        program_execution::execute_script(self, script, frame, driver)
    }

    /// ScriptEngine.cpp:6829 inherits the current invocation when calling a subroutine.
    pub fn call_subroutine(
        &self,
        name: &str,
        frame: u32,
        driver: &mut impl ScriptProgramDriver,
    ) -> GameLogicResult<bool> {
        self.sides
            .call_subroutine(name, |script| self.execute_script(script, frame, driver))
    }

    /// ScriptEngine.cpp:6008 latches the Team and controller and clears the condition Team.
    pub fn run_team_subroutine(
        &self,
        name: &str,
        team: Option<u32>,
        player: Option<&str>,
        frame: u32,
        driver: &mut impl ScriptProgramDriver,
    ) -> GameLogicResult<bool> {
        if name.is_empty() || name == "<none>" {
            return Ok(false);
        }
        self.set_condition_team(None);
        let _restore = self.enter_team_call(team, team.and(player).map(str::to_owned));
        self.call_subroutine(name, frame, driver)
    }

    /// ScriptEngine.cpp:6064 changes the calling object for an attached script.
    pub fn run_object_subroutine(
        &self,
        name: &str,
        object: u32,
        frame: u32,
        driver: &mut impl ScriptProgramDriver,
    ) -> GameLogicResult<bool> {
        if name.is_empty() || name == "<none>" {
            return Ok(false);
        }
        let _restore = self.enter_object_call(object);
        self.call_subroutine(name, frame, driver)
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
