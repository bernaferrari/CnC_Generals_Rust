//! Host-owned accepted weapon-discharge transport.
//!
//! This is intentionally not the existing `host_fire_intent_log`: an AI
//! intent can be written back without a physical discharge, while a drawable
//! recoil/muzzle event needs the exact WeaponSet slot and barrel that actually
//! fired.  The log belongs to one `GameLogic` world, so replacing/resetting a
//! world cannot leak raw ObjectIds into the next presentation frame.

use crate::game_logic::ObjectId;

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

/// Accepted-discharge transport owned by one host `GameLogic` instance.
///
/// The simulation appends through its mutable owner. A mutable publication
/// boundary moves one completed batch into frozen presentation; borrowed
/// queries never drain it. GPU passes read only the completed frame.
#[derive(Debug, Default)]
pub struct HostWeaponDischargeLog {
    pending: Vec<HostWeaponDischargeEvent>,
}

impl HostWeaponDischargeLog {
    pub fn record(&mut self, event: HostWeaponDischargeEvent) {
        self.pending.push(event);
    }

    /// Move every accepted shot accumulated across fixed steps to publication.
    pub fn take_for_presentation(&mut self) -> Vec<HostWeaponDischargeEvent> {
        std::mem::take(&mut self.pending)
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.pending.len()
    }
}
