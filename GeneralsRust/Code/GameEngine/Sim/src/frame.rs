//! The ordered work inside an admitted simulation step (C++ GameLogic::update).

use generals_game_domain::ObjectId;

use crate::GameLogicResult;
use crate::recurring::{RecurringUpdate, RecurringUpdates, UPDATE_SLEEP_NONE};
use crate::step::FixedStepOutcome;

/// The source frame and timestep captured before scripts or commands run.
/// A native command cursor carries these values even across a world reset.
#[derive(Debug, Clone, Copy)]
pub struct LogicFrame {
    pub number: u32,
    pub step_seconds: f32,
}

/// Whether lifecycle commands can yield to the application during this step.
#[derive(Clone, Copy)]
pub enum CommandPhase {
    Standalone,
    Native,
}

/// The object order captured before command dispatch, unless a clear replaced it.
#[derive(Debug)]
pub enum FrameObjectRoster {
    BeforeCommands(Vec<ObjectId>),
    AfterReset,
}

/// Operations on the receiving match and its synchronous effect recipients.
/// The simulation controls the order and the points where a step may stop.
pub trait FrameWorld {
    type CommandBoundary;

    fn run_early_scripts(&mut self, frame: LogicFrame) -> GameLogicResult<()>;
    fn time_is_frozen(&self) -> bool;
    fn has_admitted_clear(&mut self) -> bool;
    fn continue_from_debug_freeze(&mut self);
    fn update_terrain(&mut self) -> GameLogicResult<()>;
    fn stamp_command_frame(&mut self);
    fn collect_object_ids(&self) -> Vec<ObjectId>;
    fn dispatch_commands(
        &mut self,
        mode: CommandPhase,
        frame: LogicFrame,
        roster: FrameObjectRoster,
    ) -> Result<FrameObjectRoster, Self::CommandBoundary>;

    fn recurring_updates(&mut self) -> &mut RecurringUpdates;
    fn run_recurring_update(
        &mut self,
        update: RecurringUpdate,
        object_ids: &[ObjectId],
        step_seconds: f32,
    ) -> GameLogicResult<()>;
    fn update_objects(&mut self, object_ids: &[ObjectId], step_seconds: f32)
    -> GameLogicResult<()>;
    fn update_match_ai(&mut self) -> GameLogicResult<()>;
    fn dispatch_ai_orders(&mut self, after_reset: bool);
    fn update_production(&mut self, step_seconds: f32) -> GameLogicResult<()>;
    fn update_economy(&mut self, step_seconds: f32) -> GameLogicResult<()>;
    fn update_partition(&mut self) -> GameLogicResult<()>;
    fn retire_objects(&mut self) -> GameLogicResult<()>;
    fn update_weapons(&mut self) -> GameLogicResult<()>;
    fn evaluate_victory(&mut self) -> GameLogicResult<()>;
    fn expire_disabled_statuses(&mut self);
    fn update_visibility(&mut self);
}

impl LogicFrame {
    /// Run the ordinary source update, or yield a native lifecycle command.
    pub fn run<W: FrameWorld>(
        self,
        world: &mut W,
        mode: CommandPhase,
    ) -> FixedStepOutcome<W::CommandBoundary, crate::GameLogicError> {
        if let Err(error) = world.run_early_scripts(self) {
            return FixedStepOutcome::Failed(error);
        }
        // GameLogic.cpp:3603–3616 checks the admitted CommandList, not the
        // MessageStream. A clear runs the frame without rechecking freeze.
        if world.time_is_frozen() {
            if !world.has_admitted_clear() {
                return FixedStepOutcome::Frozen;
            }
            world.continue_from_debug_freeze();
        }

        // Terrain observes early script changes before commands and objects.
        if let Err(error) = world.update_terrain() {
            return FixedStepOutcome::Failed(error);
        }
        world.stamp_command_frame();
        let roster = FrameObjectRoster::BeforeCommands(world.collect_object_ids());
        match world.dispatch_commands(mode, self, roster) {
            Ok(roster) => self.run_after_commands(world, roster),
            Err(boundary) => FixedStepOutcome::CommandBoundary(boundary),
        }
    }

    /// Finish the same update after its command list drains. A reset abandons
    /// the original roster, so the replacement match supplies its own IDs.
    pub fn run_after_commands<W: FrameWorld>(
        self,
        world: &mut W,
        roster: FrameObjectRoster,
    ) -> FixedStepOutcome<W::CommandBoundary, crate::GameLogicError> {
        let (object_ids, after_reset) = match roster {
            FrameObjectRoster::BeforeCommands(object_ids) => (object_ids, false),
            FrameObjectRoster::AfterReset => (world.collect_object_ids(), true),
        };
        match self.finish(world, &object_ids, after_reset) {
            Ok(()) => FixedStepOutcome::Advanced,
            Err(error) => FixedStepOutcome::Failed(error),
        }
    }

    fn finish(
        self,
        world: &mut impl FrameWorld,
        object_ids: &[ObjectId],
        after_reset: bool,
    ) -> GameLogicResult<()> {
        // The entry remains installed while its callback mutates the match.
        // On error, preserve the existing unfinished-update state.
        while let Some(token) = world.recurring_updates().next_due(self.number) {
            world.run_recurring_update(token.kind, object_ids, self.step_seconds)?;
            world
                .recurring_updates()
                .finish(token, self.number, UPDATE_SLEEP_NONE);
        }
        world.update_objects(object_ids, self.step_seconds)?;
        world.update_match_ai()?;
        world.dispatch_ai_orders(after_reset);
        world.update_production(self.step_seconds)?;
        world.update_economy(self.step_seconds)?;
        world.update_partition()?;
        world.retire_objects()?;
        world.update_weapons()?;
        world.evaluate_victory()?;
        world.expire_disabled_statuses();
        world.update_visibility();
        Ok(())
    }
}
