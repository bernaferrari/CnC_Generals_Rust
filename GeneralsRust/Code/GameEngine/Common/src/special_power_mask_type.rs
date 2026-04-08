// FILE: special_power_mask_type.rs
// Ported from: GeneralsMD/Code/GameEngine/Include/Common/SpecialPowerMaskType.h
// Author: JKMCD, Aug 2002

pub use crate::common::rts::special_power::SpecialPowerType;

/// SPECIALPOWER_COUNT from C++ SpecialPowerType.h
pub const SPECIALPOWER_COUNT: usize = 67;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecialPowerMaskType {
    bits: u128,
}

impl SpecialPowerMaskType {
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    pub const fn from_power(power_index: usize) -> Self {
        Self {
            bits: 1u128 << power_index,
        }
    }

    pub fn set(&mut self, power_index: usize) {
        self.bits |= 1u128 << power_index;
    }

    pub fn clear(&mut self, power_index: usize) {
        self.bits &= !(1u128 << power_index);
    }

    pub fn test(&self, power_index: usize) -> bool {
        (self.bits & (1u128 << power_index)) != 0
    }

    pub fn union(&self, other: &Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    pub fn intersection(&self, other: &Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    pub fn clear_all(&mut self) {
        self.bits = 0;
    }
}

impl Default for SpecialPowerMaskType {
    fn default() -> Self {
        Self::new()
    }
}
