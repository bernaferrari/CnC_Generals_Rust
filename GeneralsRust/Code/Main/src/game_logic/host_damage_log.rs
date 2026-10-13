//! Damage-event payloads and owner health observations for the Main host/shadow bridge.
//!
//! Completed receipts and per-match counters live on `HostHealthEvents` in
//! `host_health_log`; this module intentionally contains no ambient state.

use super::ObjectId;

/// Immutable result of a completed body operation, never another mutable body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BodyHealthObservation {
    pub current: f32,
    pub maximum: f32,
}

/// One damage application observed on the host authority.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostDamageEvent {
    pub target: ObjectId,
    /// Actual HP removed after armor/battle-plan scalars.
    pub amount: f32,
    pub source: Option<ObjectId>,
    pub destroyed: bool,
    /// C++ `DamageType` ordinal (`DAMAGE_EXPLOSION` = 0).
    pub damage_type_ordinal: u32,
    owner_body: BodyHealthObservation,
}

impl HostDamageEvent {
    pub(crate) fn applied(
        target: ObjectId,
        amount: f32,
        source: Option<ObjectId>,
        destroyed: bool,
        damage_type_ordinal: u32,
        body: BodyHealthObservation,
    ) -> Self {
        Self {
            target,
            amount,
            source,
            destroyed,
            damage_type_ordinal,
            owner_body: body,
        }
    }

    pub(crate) fn body_observation(&self) -> BodyHealthObservation {
        self.owner_body
    }
}
