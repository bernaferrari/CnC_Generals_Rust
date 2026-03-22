//! Legacy command translation compatibility shim.
//!
//! The canonical command translator lives in `translators.rs`. This module keeps the old
//! `command_xlat` path available for callers that still import it, but behavior is now owned by
//! the active translator implementation.

use super::game_message::{Coord3D, DrawableID, ObjectID};
pub use super::translators::CommandTranslator;

/// Command evaluation type kept for compatibility with the legacy command-xlat API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandEvaluateType {
    DoCommand,
    DoHint,
    EvaluateOnly,
}

/// Can-attack result kept for compatibility with the legacy command-xlat API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanAttackResult {
    Possible,
    PossibleAfterMoving,
    NotPossible,
}

/// Legacy compatibility shape for object metadata used by the old translator surface.
#[derive(Debug, Clone)]
pub struct CommandableObject {
    pub id: ObjectID,
    pub position: Coord3D,
    pub is_dead: bool,
    pub is_locally_controlled: bool,
    pub can_attack: bool,
    pub can_repair: bool,
    pub can_capture: bool,
    pub can_enter: bool,
    pub is_dozer: bool,
    pub is_salvager: bool,
    pub kind_of_flags: u32,
    pub relationship_to_target: Relationship,
}

/// Legacy relationship enum retained for compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relationship {
    Neutral,
    Allies,
    Enemies,
}

/// Legacy pick-and-play metadata. The active translator owns the actual behavior.
#[derive(Debug, Clone)]
pub struct PickAndPlayInfo {
    pub air: bool,
    pub draw_target: Option<DrawableID>,
    pub weapon_slot: Option<WeaponSlotType>,
    pub special_power_type: Option<u32>,
}

impl Default for PickAndPlayInfo {
    fn default() -> Self {
        Self {
            air: false,
            draw_target: None,
            weapon_slot: None,
            special_power_type: None,
        }
    }
}

/// Legacy weapon slot type retained for API compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSlotType {
    Primary,
    Secondary,
    Tertiary,
}

/// KINDOF flags from the legacy command-xlat surface.
pub const KINDOF_IMMOBILE: u32 = 0x0000_0100;
pub const KINDOF_SALVAGE_CRATE: u32 = 0x0000_0200;
pub const KINDOF_HEAL_PAD: u32 = 0x0000_0400;
pub const KINDOF_SPAWNS_ARE_THE_WEAPONS: u32 = 0x0000_0800;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_translator_alias_is_available() {
        let _translator = CommandTranslator::new();
    }

    #[test]
    fn legacy_metadata_defaults_are_stable() {
        let info = PickAndPlayInfo::default();
        assert!(!info.air);
        assert!(info.draw_target.is_none());
        assert!(info.weapon_slot.is_none());
        assert!(info.special_power_type.is_none());
    }
}
