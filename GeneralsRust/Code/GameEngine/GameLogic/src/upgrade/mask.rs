//! Upgrade Mask System
//!
//! `UpgradeMask` is the canonical Common type (C++ `UpgradeMaskType`). Every
//! mask bit comes from the one UpgradeCenter (`UpgradeCenter::newUpgrade`);
//! there is no secondary allocator.

pub use game_engine::common::system::upgrade::UpgradeMask;

/// C++ `TheUpgradeCenter->findUpgrade(name)->getUpgradeMask()` on the active
/// world's center. Unknown names resolve to an empty mask, as C++ callers
/// treat a NULL `findUpgrade` result.
pub fn upgrade_mask_for_name(name: &str) -> UpgradeMask {
    let center = super::center::get_upgrade_center();
    let center = center
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match center.mask_for_name(name) {
        Some(mask) => mask,
        None => {
            if !name.is_empty() && !name.eq_ignore_ascii_case("None") {
                log::debug!("upgrade_mask_for_name: '{name}' is not an Upgrade");
            }
            UpgradeMask::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::AsciiString;

    #[test]
    fn unknown_name_has_no_mask() {
        assert_eq!(
            upgrade_mask_for_name("Upgrade_NeverDefinedMaskLookup"),
            UpgradeMask::none()
        );
    }

    #[test]
    fn known_name_resolves_center_bit() {
        let template = super::super::center::with_upgrade_center_mut(|center| {
            center.new_upgrade(AsciiString::from("Upgrade_MaskLookupDefined"))
        });
        assert!(template.get_mask().any());
        assert_eq!(
            upgrade_mask_for_name("Upgrade_MaskLookupDefined"),
            template.get_mask()
        );
    }
}
