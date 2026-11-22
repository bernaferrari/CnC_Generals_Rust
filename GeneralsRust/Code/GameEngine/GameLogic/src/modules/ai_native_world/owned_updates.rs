//! Native callbacks at the original GameLogic heap positions.
use super::*;
use crate::system::game_logic::{
    GameLogic, GameLogicError, NativeFrameDriver, NativeScheduledUpdate,
};
use game_engine::common::thing::update_module::UpdateSleepTime;

/// Give the admitted native callbacks the exact owners of this frame. Each
/// callback borrows the same scheduler that is driving the frame.
pub fn update_game_logic_with_native_modules(
    factory: &mut ObjectFactory,
    players: &PlayerList,
    services: AiNativeServices<'_>,
    scheduler: &mut GameLogic,
    observe_camera: impl FnOnce() -> Option<crate::helpers::CameraTimeObservation>,
) -> Result<(), GameLogicError> {
    BorrowedAiWorld::validate_receiving_services(players, &services)
        .map_err(GameLogicError::Generic)?;
    let frame = services.frame;
    let mut driver = ReceivingNativeFrame {
        factory,
        players,
        services,
    };
    scheduler.update_with_native_modules(frame, &mut driver, observe_camera)
}

struct ReceivingNativeFrame<'objects, 'services> {
    factory: &'objects mut ObjectFactory,
    players: &'objects PlayerList,
    services: AiNativeServices<'services>,
}

impl ReceivingNativeFrame<'_, '_> {
    fn borrow_world<'a>(&'a mut self, scheduler: &'a mut GameLogic) -> BorrowedAiWorld<'a> {
        BorrowedAiWorld::from_receiving_services(
            self.factory,
            self.players,
            self.services.reborrow(),
            scheduler,
            None,
        )
    }
}

impl NativeFrameDriver for ReceivingNativeFrame<'_, '_> {
    fn update_module(
        &mut self,
        scheduler: &mut GameLogic,
        ready: &NativeScheduledUpdate,
    ) -> Result<Option<UpdateSleepTime>, String> {
        self.borrow_world(scheduler).dispatch_native_callback(ready)
    }

    fn update_ai_phase(&mut self, scheduler: &mut GameLogic) -> Result<(), String> {
        self.borrow_world(scheduler)
            .process_enter_path_queue(crate::ai::pathfind_complete::PATHFIND_QUEUE_LEN)?;
        // C++ AI.cpp:332–342 updates the actual player list after its queue.
        for player in self.players.iter() {
            player
                .write()
                .map_err(|_| "receiving AI player poisoned")?
                .update();
        }
        Ok(())
    }
}

impl BorrowedAiWorld<'_> {
    fn dispatch_native_callback(
        &mut self,
        update: &NativeScheduledUpdate,
    ) -> Result<Option<UpdateSleepTime>, String> {
        let Some(owner) = self.object(update.object_id) else {
            // This frame can still contain legacy-only actors outside the
            // receiving native factory.
            return Ok(None);
        };
        if !self
            .update_scheduler
            .find_object_by_id(update.object_id)
            .is_some_and(|scheduled| Arc::ptr_eq(&scheduled, &owner))
        {
            return Err("native factory object differs from the scheduled owner".into());
        }
        let (is_slow, is_physics, ai) = {
            let object = owner
                .try_read()
                .map_err(|_| "native scheduled owner already borrowed")?;
            let is_slow = object.scheduled_native_slow_death(&update.module)?;
            let is_physics = match object.get_physics() {
                Some(physics) => {
                    Arc::ptr_eq(&physics.installed_update_proxy(&object)?, &update.module)
                }
                None => false,
            };
            let ai = if is_slow || is_physics {
                None
            } else {
                object.scheduled_native_ai(&update.module)?
            };
            (is_slow, is_physics, ai)
        };
        if !is_slow && !is_physics && ai.is_none() {
            return Ok(None);
        }
        let previous = self.current_update.replace(Arc::clone(&update.module));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match ai.as_ref() {
            Some(ai) => ai
                .try_lock()
                .map_err(|_| "native scheduled AI already borrowed".to_string())?
                .update_scheduled_tick_with_world(self),
            None if is_physics => self.update_physics_phase(&owner),
            None => self.update_slow_death_owned(&owner),
        }));
        self.current_update = previous;
        match result {
            Ok(result) => result.map(Some),
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    #[cfg(test)]
    pub(crate) fn tick_owned_updates(&mut self, frame: u32) -> Result<usize, String> {
        if self.current_update.is_some() {
            return Err("native scheduled update cannot recursively drive the heap".into());
        }
        if frame != self.services.frame {
            return Err("native update frame differs from driving context".into());
        }
        let mut executed = 0;
        while let Some(update) = self.update_scheduler.next_owned_scheduled_update(frame)? {
            let sleep = if update.should_process {
                self.dispatch_native_callback(&update)?
                    .ok_or("native scheduled callback family requires its owner runtime")?
            } else {
                UpdateSleepTime::None
            };
            self.update_scheduler
                .finish_owned_scheduled_update(frame, update, sleep);
            executed += 1;
        }
        Ok(executed)
    }
}
