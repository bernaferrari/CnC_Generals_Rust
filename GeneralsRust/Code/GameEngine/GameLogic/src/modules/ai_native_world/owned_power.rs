//! Player.cpp power effects on the receiving team roster.
use super::*;
use crate::player::{OwnedPowerBrownout, PowerObjectOwner};

impl BorrowedAiWorld<'_> {
    pub(super) fn with_native_power<R>(
        &mut self,
        run: impl FnOnce(&mut OwnedPowerBrownout<'_>) -> Result<R, String>,
    ) -> Result<R, String> {
        let (players, teams) = self.player_service_owners();
        let frame = self.services.frame;
        // Loan presentation separately from the world. SpyVision and Dozer
        // callbacks borrow the remaining world synchronously during delivery.
        let native = self
            .services
            .power
            .take()
            .ok_or("native power callbacks require receiving radar and timer services")?;
        let result = run(&mut OwnedPowerBrownout {
            idle_worker_events: None,
            objects: PowerObjectOwner::World(self),
            teams,
            players,
            radar: &mut *native.radar_edges,
            frame,
            special_power_uses_delay: native.special_power_uses_delay,
            misc_audio: native.misc_audio,
            audio: None,
        });
        self.services.power = Some(native);
        result
    }
}
