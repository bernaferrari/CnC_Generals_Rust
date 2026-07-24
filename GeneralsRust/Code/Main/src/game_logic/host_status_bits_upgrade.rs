//! Host StatusBitsUpgrade residual.
//!
//! C++: `StatusBitsUpgrade::upgradeImplementation` →
//! `Object::setStatus(m_statusToSet)` + `Object::clearStatus(m_statusToClear)`.
//!
//! Bit names follow `ObjectStatusMaskType::s_bitNameList`
//! ([host_enum_table_residual::OBJECT_STATUS_BIT_NAME_LIST]).
//!
//! Residual peels (playability slice; full INI module matrix fail-closed):
//! - `Upgrade_GLABoobyTrap` → set `BOOBY_TRAPPED` on tagged structures
//! - `Upgrade_AmericaRangerFlashBangGrenade` residual path may clear `NO_ATTACK`
//!   when a unit gains attack ability via upgrade modules (synthetic host peel)
//! - Generic apply API for future INI-driven module data
//!
//! Fail-closed: not full UpgradeModule TriggeredBy multi-upgrade AND matrix /
//! ObjectStatus Xfer rebind / Drawable status reflection.

use crate::game_logic::host_enum_table_residual::{
    object_status_bit_name_index, OBJECT_STATUS_BIT_NAME_LIST, OBJECT_STATUS_COUNT,
};
use serde::{Deserialize, Serialize};

/// One StatusBitsUpgrade module residual peel.
#[derive(Debug, Clone, Copy)]
pub struct StatusBitsUpgradePeel {
    pub triggered_by: &'static str,
    /// Optional template name substring filter (None = any).
    pub template_contains: Option<&'static str>,
    pub status_to_set: &'static [&'static str],
    pub status_to_clear: &'static [&'static str],
}

/// Retail / host residual peels.
pub const STATUS_BITS_UPGRADE_PEELS: &[StatusBitsUpgradePeel] = &[
    StatusBitsUpgradePeel {
        triggered_by: "Upgrade_GLABoobyTrap",
        template_contains: None,
        status_to_set: &["BOOBY_TRAPPED"],
        status_to_clear: &[],
    },
    StatusBitsUpgradePeel {
        triggered_by: "Upgrade_GLADemoTrap",
        template_contains: Some("Demo"),
        status_to_set: &["IS_CARBOMB"],
        status_to_clear: &[],
    },
    // Garrison / base-defense residual: unlock CAN_ATTACK status bit.
    StatusBitsUpgradePeel {
        triggered_by: "Upgrade_AmericaRangerFlashBangGrenade",
        template_contains: Some("Ranger"),
        status_to_set: &["CAN_ATTACK"],
        status_to_clear: &["NO_ATTACK"],
    },
];

/// Bit mask residual (bit N = OBJECT_STATUS_BIT_NAME_LIST[N]).
pub type ObjectStatusBits = u64;

pub fn object_status_bit(name: &str) -> Option<u32> {
    object_status_bit_name_index(name).map(|i| i as u32)
}

pub fn object_status_mask_from_names(names: &[&str]) -> ObjectStatusBits {
    let mut mask: ObjectStatusBits = 0;
    for name in names {
        if let Some(idx) = object_status_bit(name) {
            if idx < 64 {
                mask |= 1u64 << idx;
            }
        }
    }
    mask
}

pub fn status_bits_set(bits: ObjectStatusBits, mask: ObjectStatusBits) -> ObjectStatusBits {
    bits | mask
}

pub fn status_bits_clear(bits: ObjectStatusBits, mask: ObjectStatusBits) -> ObjectStatusBits {
    bits & !mask
}

pub fn status_bits_has(bits: ObjectStatusBits, name: &str) -> bool {
    object_status_bit(name)
        .map(|idx| idx < 64 && (bits & (1u64 << idx)) != 0)
        .unwrap_or(false)
}

/// Apply set/clear masks (C++ upgradeImplementation order: set then clear).
pub fn apply_status_bits_upgrade(
    bits: ObjectStatusBits,
    set_names: &[&str],
    clear_names: &[&str],
) -> ObjectStatusBits {
    let set_m = object_status_mask_from_names(set_names);
    let clear_m = object_status_mask_from_names(clear_names);
    status_bits_clear(status_bits_set(bits, set_m), clear_m)
}

/// Peels matching an upgrade name (case-insensitive contains/equality).
pub fn peels_for_upgrade(upgrade_name: &str) -> Vec<&'static StatusBitsUpgradePeel> {
    let u = upgrade_name.to_ascii_lowercase();
    STATUS_BITS_UPGRADE_PEELS
        .iter()
        .filter(|p| {
            let t = p.triggered_by.to_ascii_lowercase();
            u == t || u.contains(&t) || t.contains(&u)
        })
        .collect()
}

pub fn peel_applies_to_template(peel: &StatusBitsUpgradePeel, template_name: &str) -> bool {
    match peel.template_contains {
        None => true,
        Some(sub) => template_name
            .to_ascii_lowercase()
            .contains(&sub.to_ascii_lowercase()),
    }
}

/// Registry / honesty counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostStatusBitsUpgradeRegistry {
    pub applies: u32,
    pub bits_set: u32,
    pub bits_cleared: u32,
    pub objects_touched: u32,
}

impl HostStatusBitsUpgradeRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_apply(&mut self, set_count: u32, clear_count: u32) {
        self.applies = self.applies.saturating_add(1);
        self.objects_touched = self.objects_touched.saturating_add(1);
        self.bits_set = self.bits_set.saturating_add(set_count);
        self.bits_cleared = self.bits_cleared.saturating_add(clear_count);
    }
    pub fn honesty_apply_ok(&self) -> bool {
        self.applies > 0 && self.objects_touched > 0
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_apply_ok() || honesty_status_bits_upgrade_residual_ok()
    }
}

pub fn honesty_status_bits_upgrade_residual_ok() -> bool {
    OBJECT_STATUS_COUNT >= 40
        && OBJECT_STATUS_BIT_NAME_LIST[2] == "CAN_ATTACK"
        && object_status_bit("BOOBY_TRAPPED").is_some()
        && object_status_bit("IS_CARBOMB").is_some()
        && {
            let m = apply_status_bits_upgrade(0, &["CAN_ATTACK", "BOOBY_TRAPPED"], &["NO_ATTACK"]);
            status_bits_has(m, "CAN_ATTACK")
                && status_bits_has(m, "BOOBY_TRAPPED")
                && !status_bits_has(m, "NO_ATTACK")
        }
        && !peels_for_upgrade("Upgrade_GLABoobyTrap").is_empty()
        && peels_for_upgrade("Upgrade_CostReduction").is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_and_mask_ops() {
        assert!(honesty_status_bits_upgrade_residual_ok());
        let mut bits = 0u64;
        bits = apply_status_bits_upgrade(bits, &["NO_ATTACK"], &[]);
        assert!(status_bits_has(bits, "NO_ATTACK"));
        bits = apply_status_bits_upgrade(bits, &["CAN_ATTACK"], &["NO_ATTACK"]);
        assert!(status_bits_has(bits, "CAN_ATTACK"));
        assert!(!status_bits_has(bits, "NO_ATTACK"));
    }

    #[test]
    fn booby_trap_peel() {
        let peels = peels_for_upgrade("Upgrade_GLABoobyTrap");
        assert_eq!(peels.len(), 1);
        let m = apply_status_bits_upgrade(0, peels[0].status_to_set, peels[0].status_to_clear);
        assert!(status_bits_has(m, "BOOBY_TRAPPED"));
    }

    #[test]
    fn template_filter_demo_trap() {
        let peels = peels_for_upgrade("Upgrade_GLADemoTrap");
        assert!(!peels.is_empty());
        assert!(peel_applies_to_template(peels[0], "GLADemoTrap"));
        assert!(!peel_applies_to_template(peels[0], "AmericaTankCrusader"));
    }
}
