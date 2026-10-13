//! Main-crate side-effect isolation for a staged save/map restore.
//!
//! GameLogic singleton contents are isolated by
//! `gamelogic::runtime_world_transaction`.  These thread-local host queues
//! sit outside that crate, so they need the same take/restore discipline:
//! never `clear` the active world at stage entry, discard candidate events on
//! rollback, and clear the old world's events only once commit is certain.

use super::{host_demo_mine_cheer_log, host_movement_log, host_spawn_log};
use std::cell::Cell;

thread_local! {
    static STAGED_WORLD_EFFECTS_DEPTH: Cell<u32> = const { Cell::new(0) };
}

pub(crate) fn world_stage_effects_active() -> bool {
    STAGED_WORLD_EFFECTS_DEPTH.with(|depth| depth.get() != 0)
}

/// Raw contents of every Main TLS queue that can be emitted while a saved
/// world is staged. Keep this list tied to the `load_map`/snapshot call graph:
/// it intentionally does not clear unrelated frame/tick queues, but it does
/// include transitive object-create and restore writers.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorldStageEffectsState {
    spawn: Vec<host_spawn_log::HostSpawnEvent>,

    movement: Vec<host_movement_log::HostMovementEvent>,
    demo_mine_cheer: Vec<host_demo_mine_cheer_log::HostDemoMineCheerEvent>,
}

impl WorldStageEffectsState {
    fn take_for_world_stage() -> Self {
        Self {
            spawn: host_spawn_log::take_for_world_stage(),

            movement: host_movement_log::take_for_world_stage(),
            demo_mine_cheer: host_demo_mine_cheer_log::take_for_world_stage(),
        }
    }

    fn replace_for_world_stage(next: Self) -> Self {
        Self {
            spawn: host_spawn_log::replace_for_world_stage(next.spawn),

            movement: host_movement_log::replace_for_world_stage(next.movement),
            demo_mine_cheer: host_demo_mine_cheer_log::replace_for_world_stage(
                next.demo_mine_cheer,
            ),
        }
    }

    fn discard_for_world_replace() {
        host_spawn_log::clear();

        host_movement_log::clear();
        host_demo_mine_cheer_log::clear();
    }

    #[cfg(test)]
    pub(crate) fn take_all_for_test() -> Self {
        Self::take_for_world_stage()
    }

    #[cfg(test)]
    pub(crate) fn replace_all_for_test(next: Self) -> Self {
        Self::replace_for_world_stage(next)
    }
}

/// Owns the active world's Main TLS queues while a candidate world is built.
/// The outermost scope restores them on both success (before commit) and
/// failure.  Nested scopes only mark the same stage active; they never move
/// queues a second time.
pub(crate) struct StagedWorldEffects {
    outermost: bool,
    active: bool,
    live: Option<WorldStageEffectsState>,
}

impl StagedWorldEffects {
    pub(crate) fn enter() -> Self {
        let outermost = STAGED_WORLD_EFFECTS_DEPTH.with(|depth| {
            let prior = depth.get();
            depth.set(
                prior
                    .checked_add(1)
                    .expect("staged world effects depth overflow"),
            );
            prior == 0
        });
        let live = outermost.then(WorldStageEffectsState::take_for_world_stage);
        Self {
            outermost,
            active: true,
            live,
        }
    }

    fn restore_live(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        let became_inactive = STAGED_WORLD_EFFECTS_DEPTH.with(|depth| {
            let prior = depth.get();
            debug_assert!(prior != 0, "unbalanced staged-world effects scope");
            let next = prior.saturating_sub(1);
            depth.set(next);
            next == 0
        });
        if !self.outermost || !became_inactive {
            return;
        }

        let Some(live) = self.live.take() else {
            return;
        };

        // Candidate events are intentionally dropped.  Restoring uses raw
        // replacement rather than draining so the active world gets precisely
        // the pending queue and presentation `LAST_DRAIN` it had before stage.
        drop(WorldStageEffectsState::replace_for_world_stage(live));
    }

    /// Restore the live queues before returning a successful candidate to the
    /// caller.  Commit will separately discard those now-stale old-world
    /// queues just before installing the new world.
    pub(crate) fn finish_and_restore_live(mut self) {
        self.restore_live();
    }
}

impl Drop for StagedWorldEffects {
    fn drop(&mut self) {
        self.restore_live();
    }
}

/// The old world is about to be replaced successfully.  Its pending shadow
/// mutation logs must not be applied to the new IDs/world after commit.
pub(crate) fn discard_live_for_world_replace() {
    debug_assert!(
        !world_stage_effects_active(),
        "world effect queues must be restored before a staged-world commit"
    );
    WorldStageEffectsState::discard_for_world_replace();
}
