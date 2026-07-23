//! Host ModelConditionUpgrade residual (upgrade sets a model condition bit).
//!
//! C++: `ModelConditionUpgrade::upgradeImplementation` →
//! `setModelConditionState(ConditionFlag)`.
//!
//! Residual playability slice:
//! - Demo armor upgrade peels set GARRISONED visual residual
//! - Common ConditionFlag name → model condition bit residual
//! - Applied when matching upgrade completes on object
//!
//! Fail-closed: not full UpgradeMux triggers / exclusive upgrade matrix.

use serde::{Deserialize, Serialize};

/// Map ConditionFlag INI name to model condition bit index residual.
pub fn model_condition_flag_bit(flag_name: &str) -> Option<u32> {
    use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
    model_condition_bit_name_index(flag_name).map(|i| i as u32)
}

/// Retail peels: upgrade name → ConditionFlag residual.
pub fn condition_flag_for_upgrade(upgrade_name: &str) -> Option<&'static str> {
    let n = upgrade_name.to_ascii_lowercase();
    // Demo General armor visual residual uses GARRISONED flag.
    if n.contains("armor")
        && (n.contains("demo")
            || n.contains("apc")
            || n.contains("technical")
            || n.contains("humvee"))
    {
        return Some("GARRISONED");
    }
    if n.contains("camo") || n.contains("camouflage") {
        return Some("DISGUISED");
    }
    if n.contains("radar") && n.contains("upgrade") {
        return Some("RADAR_UPGRADED");
    }
    if n.contains("overcharge") {
        return Some("POWER_PLANT_UPGRADED");
    }
    None
}

/// Apply ModelConditionUpgrade residual to model_condition_bits.
pub fn apply_model_condition_upgrade(model_condition_bits: &mut u128, upgrade_name: &str) -> bool {
    let Some(flag) = condition_flag_for_upgrade(upgrade_name) else {
        return false;
    };
    let Some(bit) = model_condition_flag_bit(flag) else {
        return false;
    };
    *model_condition_bits |= 1u128 << bit;
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostModelConditionUpgradeLog {
    pub applications: u32,
    pub last_flag: String,
}

impl HostModelConditionUpgradeLog {
    pub fn record(&mut self, flag: &str) {
        self.applications = self.applications.saturating_add(1);
        self.last_flag = flag.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_armor_sets_garrisoned_bit() {
        let mut bits = 0u128;
        assert!(apply_model_condition_upgrade(
            &mut bits,
            "Upgrade_DemoArmor"
        ));
        assert_ne!(bits, 0);
    }

    #[test]
    fn unknown_upgrade_skipped() {
        let mut bits = 0u128;
        assert!(!apply_model_condition_upgrade(&mut bits, "Upgrade_Nothing"));
    }
}
