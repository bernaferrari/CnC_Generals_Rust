//! Legacy upgrade compatibility helpers.
//!
//! The main upgrade system lives in `crate::upgrade`. This module keeps older call sites
//! working by forwarding to the canonical implementation and aligning mask allocation.

use crate::common::UpgradeMaskType;

pub use crate::upgrade::{Upgrade, UpgradeTemplate};

/// Returns (and allocates if needed) the mask for the provided upgrade name.
///
/// This is a compatibility wrapper that ensures legacy callers share the same
/// mask registry as the modern upgrade system.
pub fn upgrade_mask_for_ascii(name: &str) -> UpgradeMaskType {
    let mask = crate::upgrade::mask::upgrade_mask_for_name(name);
    UpgradeMaskType::from_bits_retain(mask.bits())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_mask_matches_upgrade_mask() {
        let legacy_mask = upgrade_mask_for_ascii("Upgrade_TestLegacy");
        let new_mask = crate::upgrade::mask::upgrade_mask_for_name("Upgrade_TestLegacy");
        assert_eq!(legacy_mask.bits(), new_mask.bits());
    }
}
