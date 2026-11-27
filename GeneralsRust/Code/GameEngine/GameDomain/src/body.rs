//! ActiveBody damage state shared by simulation, INI definitions, and drawing.

use serde::{Deserialize, Serialize};

/// C++ `BodyDamageType` (BodyModule.h), ordered from healthiest to destroyed.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum BodyDamageType {
    #[default]
    Pristine = 0,
    Damaged = 1,
    ReallyDamaged = 2,
    Rubble = 3,
}

impl BodyDamageType {
    pub const COUNT: usize = 4;

    pub const fn ordinal(self) -> u8 {
        self as u8
    }

    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Pristine),
            1 => Some(Self::Damaged),
            2 => Some(Self::ReallyDamaged),
            3 => Some(Self::Rubble),
            _ => None,
        }
    }

    /// The saved presentation state defaults to pristine for an unknown ordinal.
    pub const fn from_ordinal(value: u8) -> Self {
        match Self::from_index(value as usize) {
            Some(state) => state,
            None => Self::Pristine,
        }
    }
}
