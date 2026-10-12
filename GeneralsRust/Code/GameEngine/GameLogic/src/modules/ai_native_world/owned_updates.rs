//! The actual indexed scheduler with native callback loans released between phases.
use super::*;
use game_engine::common::thing::update_module::UpdateSleepTime;

impl BorrowedAiWorld<'_> {
    pub(crate) fn tick_owned_updates(&mut self, frame: u32) -> Result<usize, String> {
        if self.services.current_update.is_some() {
            return Err("native scheduled update cannot recursively drive the heap".into());
        }
        if frame != self.services.frame {
            return Err("native update frame differs from driving context".into());
        }
        let mut executed = 0;
        while let Some(update) = self
            .services
            .update_scheduler
            .next_owned_scheduled_update(frame)?
        {
            let sleep = if update.should_process {
                let owner = self
                    .object(update.object_id)
                    .ok_or("native scheduled owner absent")?;
                let is_slow = owner
                    .try_read()
                    .map_err(|_| "native scheduled owner already borrowed")?
                    .scheduled_native_slow_death(&update.module)?;
                if !is_slow {
                    return Err(
                        "native scheduled callback family requires its owner runtime".into(),
                    );
                }
                let previous = self
                    .services
                    .current_update
                    .replace(Arc::clone(&update.module));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.update_slow_death_owned(&owner)
                }));
                self.services.current_update = previous;
                match result {
                    Ok(result) => result?,
                    Err(panic) => std::panic::resume_unwind(panic),
                }
            } else {
                UpdateSleepTime::None
            };
            self.services
                .update_scheduler
                .finish_owned_scheduled_update(frame, update, sleep);
            executed += 1;
        }
        Ok(executed)
    }
}
