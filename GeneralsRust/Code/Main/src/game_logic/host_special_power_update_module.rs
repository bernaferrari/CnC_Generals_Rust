//! Host SpecialPowerUpdateModule residual (C++ interface base).
//!
//! C++: `SpecialPowerUpdateModule` / `SpecialPowerUpdateInterface` — abstract
//! base for special-power-driven updates (Spectre deployment, particle Uplink,
//! etc.). Not a standalone INI behavior; concrete hosts implement initiate.
//!
//! Residual honesty:
//! - Interface method names / science gate residual
//! - Disabled masks processed during SP (SUBDUED / UNDERPOWERED / EMP / HACKED)
//!
//! Fail-closed: not full Module virtual table / Xfer of every SP update.

use serde::{Deserialize, Serialize};

/// C++ DisabledType bits commonly processed by SP updates (subset).
pub const SP_UPDATE_PROCESS_SUBDUED: u32 = 1 << 0;
pub const SP_UPDATE_PROCESS_UNDERPOWERED: u32 = 1 << 1;
pub const SP_UPDATE_PROCESS_EMP: u32 = 1 << 2;
pub const SP_UPDATE_PROCESS_HACKED: u32 = 1 << 3;

/// Retail Spectre deployment disabled mask residual (4 bits).
pub const SPECTRE_DEPLOY_DISABLED_MASK: u32 = SP_UPDATE_PROCESS_SUBDUED
    | SP_UPDATE_PROCESS_UNDERPOWERED
    | SP_UPDATE_PROCESS_EMP
    | SP_UPDATE_PROCESS_HACKED;

/// Thin host state for objects that own a SpecialPowerUpdateModule residual.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostSpecialPowerUpdateModuleData {
    pub is_special_power: bool,
    pub is_special_ability: bool,
    pub extra_required_science: Option<String>,
    pub disabled_types_to_process: u32,
    pub power_currently_in_use: bool,
}

impl HostSpecialPowerUpdateModuleData {
    pub fn spectre_deployment_defaults() -> Self {
        Self {
            is_special_power: true,
            is_special_ability: false,
            extra_required_science: None,
            disabled_types_to_process: SPECTRE_DEPLOY_DISABLED_MASK,
            power_currently_in_use: false,
        }
    }

    pub fn does_special_power_update_pass_science_test(
        &self,
        player_has_science: impl Fn(&str) -> bool,
    ) -> bool {
        match self.extra_required_science.as_deref() {
            None | Some("") => true,
            Some(s) => player_has_science(s),
        }
    }
}

pub fn honesty_special_power_update_module_residual_ok() -> bool {
    SPECTRE_DEPLOY_DISABLED_MASK.count_ones() == 4
        && {
            let d = HostSpecialPowerUpdateModuleData::spectre_deployment_defaults();
            d.is_special_power
                && !d.is_special_ability
                && d.does_special_power_update_pass_science_test(|_| false)
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack() {
        assert!(honesty_special_power_update_module_residual_ok());
        let mut d = HostSpecialPowerUpdateModuleData::spectre_deployment_defaults();
        d.extra_required_science = Some("SCIENCE_SpectreGunship".into());
        assert!(!d.does_special_power_update_pass_science_test(|_| false));
        assert!(d.does_special_power_update_pass_science_test(|s| s.contains("Spectre")));
    }
}
