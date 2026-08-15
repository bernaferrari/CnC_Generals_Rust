//! Host-owned accepted weapon-discharge transport.
//!
//! This is intentionally not the existing `host_fire_intent_log`: an AI
//! intent can be written back without a physical discharge, while a drawable
//! recoil/muzzle event needs the exact WeaponSet slot and barrel that actually
//! fired.  The log belongs to one `GameLogic` world, so replacing/resetting a
//! world cannot leak raw ObjectIds into the next presentation frame.

use crate::game_logic::ObjectId;
use std::sync::Mutex;

/// A successful live weapon discharge, normalized after the concrete weapon
/// has consumed ammo and before its barrel cursor advances.
#[derive(Debug, Clone, PartialEq)]
pub struct HostWeaponDischargeEvent {
    pub source: ObjectId,
    pub weapon_slot: u8,
    pub fired_barrel: u8,
    pub sequence: u64,
    pub logic_frame: u32,
    pub visual_plan: Option<crate::presentation_frame::FrozenWeaponVisualDispatchPlan>,
}

/// Renderer-facing event queue owned by a single host `GameLogic` instance.
///
/// `PresentationFrame::build_from_logic` intentionally has only `&GameLogic`.
/// This narrow synchronized queue lets it consume each accepted discharge
/// exactly once without making a visual frame build authoritative game
/// simulation work, while preserving `GameLogic: Sync` for its existing
/// `Arc<RwLock<_>>` hosts.
#[derive(Debug, Default)]
pub struct HostWeaponDischargeLog {
    pending: Mutex<Vec<HostWeaponDischargeEvent>>,
}

impl HostWeaponDischargeLog {
    pub fn record(&self, event: HostWeaponDischargeEvent) {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(event);
    }

    /// Drain every event accumulated since the preceding presentation frame.
    ///
    /// Unlike older `take_last_drain` residual channels, multiple fixed logic
    /// steps can legitimately occur before one GPU frame.  Dropping all but
    /// the last batch would skip an accepted physical discharge.
    pub fn take_for_presentation(&self) -> Vec<HostWeaponDischargeEvent> {
        std::mem::take(
            &mut *self
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }

    pub fn clear(&self) {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}
