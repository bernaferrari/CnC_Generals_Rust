//! C++ OverlordContain/HelixContain `onBodyDamageStateChange` → `setDamageState`.
//!
//! Live addons are host flags (and an optional spawned portable occupant).
//! BODY_RUBBLE is skipped; death is handled separately.

use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use crate::game_logic::host_overlord_addons::OverlordAddonKind;
use crate::game_logic::host_overlord_addons::{
    OVERLORD_PAYLOAD_BUNKER, OVERLORD_PAYLOAD_GATTLING, OVERLORD_PAYLOAD_PROPAGANDA,
};

pub const HELIX_PAYLOAD_GATTLING: &str = "ChinaHelixGattlingCannon";
pub const HELIX_PAYLOAD_PROPAGANDA: &str = "ChinaHelixPropagandaTower";
pub const HELIX_PAYLOAD_BUNKER: &str = "ChinaHelixBattleBunker";

/// True when a portable gattling / speaker / bunker addon is installed.
/// Emperor innate propaganda is not a portable payload.
pub fn portable_addon_installed(
    has_gattling: bool,
    has_propaganda: bool,
    bunker_slots: usize,
    is_emperor: bool,
) -> bool {
    has_gattling || bunker_slots > 0 || (has_propaganda && !is_emperor)
}

/// Exclusive installed portable kind, if any.
pub fn installed_portable_addon_kind(
    has_gattling: bool,
    has_propaganda: bool,
    bunker_slots: usize,
    is_emperor: bool,
) -> Option<OverlordAddonKind> {
    if has_gattling {
        Some(OverlordAddonKind::Gattling)
    } else if bunker_slots > 0 {
        Some(OverlordAddonKind::Bunker)
    } else if has_propaganda && !is_emperor {
        Some(OverlordAddonKind::Propaganda)
    } else {
        None
    }
}

/// Payload template for the installed portable addon visual.
pub fn overlord_addon_payload_template(kind: OverlordAddonKind, is_helix: bool) -> &'static str {
    match (kind, is_helix) {
        (OverlordAddonKind::Gattling, false) => OVERLORD_PAYLOAD_GATTLING,
        (OverlordAddonKind::Gattling, true) => HELIX_PAYLOAD_GATTLING,
        (OverlordAddonKind::Propaganda, false) => OVERLORD_PAYLOAD_PROPAGANDA,
        (OverlordAddonKind::Propaganda, true) => HELIX_PAYLOAD_PROPAGANDA,
        (OverlordAddonKind::Bunker, false) => OVERLORD_PAYLOAD_BUNKER,
        (OverlordAddonKind::Bunker, true) => HELIX_PAYLOAD_BUNKER,
    }
}

/// C++ OverlordContain.cpp:148 / HelixContain.cpp:180: skip BODY_RUBBLE.
pub fn overlord_addon_mirrored_damage_state(
    host: HostBodyDamageType,
) -> Option<HostBodyDamageType> {
    match host {
        HostBodyDamageType::Rubble => None,
        other => Some(other),
    }
}

/// C++ ActiveBody::setDamageState health for a non-rubble body state.
pub fn overlord_addon_set_damage_state_health(
    max_health: f32,
    new_state: HostBodyDamageType,
) -> f32 {
    use crate::game_logic::host_enum_table_residual::{
        HOST_UNIT_DAMAGED_THRESH, HOST_UNIT_REALLY_DAMAGED_THRESH,
    };
    let ratio = match new_state {
        HostBodyDamageType::Pristine => 1.0,
        HostBodyDamageType::Damaged => HOST_UNIT_DAMAGED_THRESH,
        HostBodyDamageType::ReallyDamaged => HOST_UNIT_REALLY_DAMAGED_THRESH,
        HostBodyDamageType::Rubble => 0.0,
    };
    (max_health * ratio - 1.0).max(0.0)
}

/// C++ OverlordContain/HelixContain onContaining receiveGrant(true) when host STEALTHED.
pub fn should_grant_stealth_to_portable_addon(host_stealthed: bool, is_portable: bool) -> bool {
    host_stealthed && is_portable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rubble_is_not_mirrored() {
        assert_eq!(
            overlord_addon_mirrored_damage_state(HostBodyDamageType::Rubble),
            None
        );
        assert_eq!(
            overlord_addon_mirrored_damage_state(HostBodyDamageType::Damaged),
            Some(HostBodyDamageType::Damaged)
        );
        assert_eq!(
            overlord_addon_mirrored_damage_state(HostBodyDamageType::ReallyDamaged),
            Some(HostBodyDamageType::ReallyDamaged)
        );
        assert_eq!(
            overlord_addon_mirrored_damage_state(HostBodyDamageType::Pristine),
            Some(HostBodyDamageType::Pristine)
        );
    }

    #[test]
    fn set_damage_state_health_matches_active_body() {
        let max = 100.0;
        assert!(
            (overlord_addon_set_damage_state_health(max, HostBodyDamageType::Pristine) - 99.0)
                .abs()
                < 0.01
        );
        assert!(
            (overlord_addon_set_damage_state_health(max, HostBodyDamageType::Damaged) - 69.0).abs()
                < 0.01
        );
        assert!(
            (overlord_addon_set_damage_state_health(max, HostBodyDamageType::ReallyDamaged) - 34.0)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn emperor_innate_propaganda_is_not_portable() {
        assert!(!portable_addon_installed(false, true, 0, true));
        assert!(portable_addon_installed(true, true, 0, true));
        assert!(portable_addon_installed(false, true, 0, false));
        assert_eq!(
            installed_portable_addon_kind(true, false, 0, false),
            Some(OverlordAddonKind::Gattling)
        );
        assert_eq!(
            overlord_addon_payload_template(OverlordAddonKind::Gattling, false),
            OVERLORD_PAYLOAD_GATTLING
        );
        assert_eq!(
            overlord_addon_payload_template(OverlordAddonKind::Gattling, true),
            HELIX_PAYLOAD_GATTLING
        );
    }
}
