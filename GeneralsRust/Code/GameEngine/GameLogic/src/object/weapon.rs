//! Compatibility module for C++ GameLogic/Object/Weapon.cpp.
//! Re-exports the leftover crate::weapon stack (one Weapon / WeaponTemplate /
//! WeaponStore). Leftover `weapon/weapon.rs` is a private module, not a
//! second public type.

pub use crate::weapon::{Weapon, WeaponStore, WeaponTemplate};
