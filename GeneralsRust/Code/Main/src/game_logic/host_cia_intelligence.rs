//! Host CIA Intelligence / SpyVision special-power residual.
//!
//! Residual slice (playability):
//! - `DoSpecialPower(CiaIntelligence)` temporarily spies on all enemy units
//!   (retail SuperweaponCIAIntelligence / SpyVisionUpdate setUnitsVisionSpied).
//! - For each enemy unit: mark vision-spied, temporary FOW reveal at unit pos
//!   using residual vision radius (unit sight_range), and mark stealthed units
//!   DETECTED so they become visible/targetable.
//! - Fog returns after BaseDuration (undo lookers); vision-spied marks clear.
//! - Honesty counters/flags for residual gates and tests.
//!
//! Fail-closed honesty:
//! - Not full SpyVisionUpdate module timers / upgrade mux / self-powered path
//! - Not per-kindof SpyOnKindof filter / capture / sabotage-disable matrix
//! - Not multiplayer shared-synced timer / academy / shortcut UI parity
//! - Not Common Player::setUnitsVisionSpied full OBJECT_REGISTRY iteration

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const CIA_INTELLIGENCE_LOGIC_FPS: f32 = 30.0;

/// Retail SpyVisionSpecialPower BaseDuration = 30000 ms @ 30 FPS → 900 frames.
pub const CIA_INTELLIGENCE_DURATION_FRAMES: u32 = 900;

/// Residual FOW reveal radius when an enemy unit's sight_range is unset/0.
/// Matches default ThingTemplate::sight_range residual.
pub const CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS: f32 = 150.0;

/// Activate audio residual (SpecialPower.ini InitiateSound).
pub const CIA_INTELLIGENCE_ACTIVATE_AUDIO: &str = "CIAIntelligenceActivate";

/// One enemy unit temporarily vision-spied by an activation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostCiaIntelligenceSpiedUnit {
    pub object_id: ObjectId,
    pub location: Vec3,
    pub radius: f32,
    /// True after ShroudManager confirmed unit cell visible for spy player.
    pub fow_reveal_ok: bool,
    /// True if unit was stealthed and mark_detected was applied.
    pub detected_ok: bool,
}

/// One active residual CIA Intelligence activation (host-side bookkeeping).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostCiaIntelligence {
    pub id: u32,
    pub player_id: u32,
    pub player_mask: u32,
    pub spying_team: super::Team,
    pub activate_frame: u32,
    pub expires_frame: u32,
    pub caster_id: Option<ObjectId>,
    pub spied_units: Vec<HostCiaIntelligenceSpiedUnit>,
    /// True if at least one enemy unit was vision-spied this activation.
    pub vision_spied_ok: bool,
    /// True if at least one FOW reveal at an enemy unit was observed.
    pub fow_reveal_ok: bool,
    /// True if at least one stealthed enemy was marked DETECTED.
    pub detect_ok: bool,
}

impl HostCiaIntelligence {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn spied_object_ids(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.spied_units.iter().map(|u| u.object_id)
    }

    pub fn is_object_spied(&self, object_id: ObjectId) -> bool {
        self.spied_units.iter().any(|u| u.object_id == object_id)
    }
}

/// Host residual registry for CIA Intelligence special power activations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCiaIntelligenceRegistry {
    next_id: u32,
    /// Active (not yet expired) residual activations.
    active: Vec<HostCiaIntelligence>,
    /// Total activations (honesty).
    pub activations: u32,
    /// Activations that vision-spied at least one enemy unit.
    pub vision_spied: u32,
    /// Activations that observably cleared FOW at an enemy unit position.
    pub fow_reveals: u32,
    /// Activations that marked at least one stealthed enemy DETECTED.
    pub detects: u32,
    /// Total enemy units vision-spied across activations (honesty counter).
    pub units_spied: u32,
    /// Activations that have expired (undo applied / bookkeeping pruned).
    pub expirations: u32,
}

impl HostCiaIntelligenceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_scans(&self) -> &[HostCiaIntelligence] {
        &self.active
    }

    pub fn activations(&self) -> u32 {
        self.activations
    }

    pub fn vision_spied(&self) -> u32 {
        self.vision_spied
    }

    pub fn fow_reveals(&self) -> u32 {
        self.fow_reveals
    }

    pub fn detects(&self) -> u32 {
        self.detects
    }

    pub fn units_spied(&self) -> u32 {
        self.units_spied
    }

    pub fn expirations(&self) -> u32 {
        self.expirations
    }

    /// Record a successful residual activation.
    pub fn record_activation(&mut self, act: HostCiaIntelligence) {
        self.activations = self.activations.saturating_add(1);
        if act.vision_spied_ok {
            self.vision_spied = self.vision_spied.saturating_add(1);
        }
        if act.fow_reveal_ok {
            self.fow_reveals = self.fow_reveals.saturating_add(1);
        }
        if act.detect_ok {
            self.detects = self.detects.saturating_add(1);
        }
        self.units_spied = self
            .units_spied
            .saturating_add(act.spied_units.len() as u32);
        self.active.push(act);
    }

    /// Drop expired bookkeeping entries. Returns object ids that were spied
    /// by just-expired activations (for clearing vision_spied residual marks).
    pub fn prune_expired(&mut self, current_frame: u32) -> Vec<ObjectId> {
        let mut cleared = Vec::new();
        let mut kept = Vec::with_capacity(self.active.len());
        for act in self.active.drain(..) {
            if act.is_expired(current_frame) {
                for u in &act.spied_units {
                    cleared.push(u.object_id);
                }
                self.expirations = self.expirations.saturating_add(1);
            } else {
                kept.push(act);
            }
        }
        self.active = kept;
        cleared
    }

    /// Allocate the next residual activation id.
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Residual honesty: at least one activation recorded.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activations > 0
    }

    /// Residual honesty: at least one enemy unit was vision-spied.
    pub fn honesty_vision_spied_ok(&self) -> bool {
        self.vision_spied > 0 && self.units_spied > 0
    }

    /// Residual honesty: FOW reveal was observed at least once at an enemy unit.
    pub fn honesty_fow_reveal_ok(&self) -> bool {
        self.fow_reveals > 0
    }

    /// Combined host path: activated + vision-spied residual.
    /// Fail-closed: FOW is preferred but vision-spied alone is the core
    /// setUnitsVisionSpied residual (enemy units visible/detectable).
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_vision_spied_ok()
    }

    /// True if any active residual still spies `object_id` for `player_id`.
    pub fn is_object_vision_spied(&self, player_id: u32, object_id: ObjectId) -> bool {
        self.active.iter().any(|a| {
            a.player_id == player_id && a.is_object_spied(object_id)
        })
    }

    /// True if any active residual for `player_id` covers horizontal `pos`
    /// (FOW residual footprint around a spied unit).
    pub fn is_position_in_active_spy(&self, player_id: u32, pos: Vec3) -> bool {
        self.active.iter().any(|a| {
            if a.player_id != player_id {
                return false;
            }
            a.spied_units.iter().any(|u| {
                let dx = pos.x - u.location.x;
                let dz = pos.z - u.location.z;
                dx * dx + dz * dz <= u.radius * u.radius
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn registry_records_activation_and_honesty() {
        let mut reg = HostCiaIntelligenceRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.alloc_id();
        reg.record_activation(HostCiaIntelligence {
            id,
            player_id: 0,
            player_mask: 1,
            spying_team: Team::USA,
            activate_frame: 0,
            expires_frame: CIA_INTELLIGENCE_DURATION_FRAMES,
            caster_id: Some(ObjectId(1)),
            spied_units: vec![HostCiaIntelligenceSpiedUnit {
                object_id: ObjectId(42),
                location: Vec3::new(200.0, 0.0, 200.0),
                radius: CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS,
                fow_reveal_ok: true,
                detected_ok: true,
            }],
            vision_spied_ok: true,
            fow_reveal_ok: true,
            detect_ok: true,
        });
        assert_eq!(reg.activations(), 1);
        assert_eq!(reg.vision_spied(), 1);
        assert_eq!(reg.fow_reveals(), 1);
        assert_eq!(reg.detects(), 1);
        assert_eq!(reg.units_spied(), 1);
        assert_eq!(reg.active_count(), 1);
        assert!(reg.honesty_host_path_ok());
        assert!(reg.is_object_vision_spied(0, ObjectId(42)));
        assert!(reg.is_position_in_active_spy(0, Vec3::new(200.0, 0.0, 200.0)));
        assert!(!reg.is_object_vision_spied(0, ObjectId(99)));

        let cleared = reg.prune_expired(CIA_INTELLIGENCE_DURATION_FRAMES);
        assert_eq!(cleared, vec![ObjectId(42)]);
        assert_eq!(reg.active_count(), 0);
        assert_eq!(reg.expirations(), 1);
        // Honesty remains after expiry (historical).
        assert!(reg.honesty_host_path_ok());
        assert!(!reg.is_object_vision_spied(0, ObjectId(42)));
    }
}
