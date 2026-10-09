//! C++ `UpgradeMaskType` (Upgrade.h): `BitFlags<UPGRADE_MAX_COUNT>`.
//!
//! Each `UpgradeTemplate` owns exactly one bit, assigned by
//! `UpgradeCenter::newUpgrade` in allocation order (Upgrade.cpp:345-353).

use bitflags::bitflags;

/// C++ `UPGRADE_MAX_COUNT` (Upgrade.h).
pub const UPGRADE_MAX_COUNT: usize = 128;

bitflags! {
    /// Bit mask for tracking upgrades
    /// Matches C++ BitFlags<UPGRADE_MAX_COUNT>
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct UpgradeMask: u128 {
        const NONE = 0;
    }
}

impl UpgradeMask {
    /// Create an empty mask
    pub fn none() -> Self {
        UpgradeMask::NONE
    }

    /// Check if any bits are set
    pub fn any(&self) -> bool {
        !self.is_empty()
    }

    /// Test if all bits in the mask are set
    pub fn test_for_all(&self, mask: UpgradeMask) -> bool {
        self.contains(mask)
    }

    /// Test if any bits in the mask are set
    pub fn test_for_any(&self, mask: UpgradeMask) -> bool {
        self.intersects(mask)
    }

    /// Test that required bits are set and conflicting bits are clear
    /// Matches C++ TEST_UPGRADE_MASK_MULTI
    pub fn test_set_and_clear(&self, must_be_set: UpgradeMask, must_be_clear: UpgradeMask) -> bool {
        self.contains(must_be_set) && !self.intersects(must_be_clear)
    }

    /// Set a specific bit by index
    pub fn set_bit(&mut self, index: usize) {
        if index < UPGRADE_MAX_COUNT {
            let bit = 1u128 << index;
            *self = UpgradeMask::from_bits_retain(self.bits() | bit);
        }
    }

    /// Clear a specific bit by index
    pub fn clear_bit(&mut self, index: usize) {
        if index < UPGRADE_MAX_COUNT {
            let bit = 1u128 << index;
            *self = UpgradeMask::from_bits_retain(self.bits() & !bit);
        }
    }

    /// Test a specific bit by index
    pub fn test_bit(&self, index: usize) -> bool {
        if index < UPGRADE_MAX_COUNT {
            let bit = 1u128 << index;
            (self.bits() & bit) != 0
        } else {
            false
        }
    }

    /// Flip all bits
    pub fn flip(&mut self) {
        *self = UpgradeMask::from_bits_retain(!self.bits());
    }

    /// Convert to raw bits (alias for serialization)
    pub fn to_bits(&self) -> u128 {
        self.bits()
    }

    /// Create from raw bits (alias for deserialization)
    pub fn from_bits_value(bits: u128) -> Self {
        UpgradeMask::from_bits_retain(bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_basic() {
        let mut mask = UpgradeMask::none();
        assert!(!mask.any());

        mask.set_bit(5);
        assert!(mask.any());
        assert!(mask.test_bit(5));
        assert!(!mask.test_bit(4));

        mask.clear_bit(5);
        assert!(!mask.any());
    }

    #[test]
    fn test_mask_operations() {
        let mut mask1 = UpgradeMask::none();
        mask1.set_bit(1);
        mask1.set_bit(3);

        let mut mask2 = UpgradeMask::none();
        mask2.set_bit(3);
        mask2.set_bit(5);

        assert!(mask1.test_for_any(mask2));
        assert!(!mask1.test_for_all(mask2));
    }

    #[test]
    fn test_mask_set_and_clear() {
        let mut current = UpgradeMask::none();
        current.set_bit(1);
        current.set_bit(2);

        let mut required = UpgradeMask::none();
        required.set_bit(1);

        let mut conflicting = UpgradeMask::none();
        conflicting.set_bit(5);

        assert!(current.test_set_and_clear(required, conflicting));

        conflicting.set_bit(2);
        assert!(!current.test_set_and_clear(required, conflicting));
    }
}
