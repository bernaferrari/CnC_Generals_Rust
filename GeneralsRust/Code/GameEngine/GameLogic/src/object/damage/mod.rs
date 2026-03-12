//! Damage Module - Rust conversion of C++ DamageModule base class
//!
//! This module provides the base interface for damage processing modules.
//! Damage modules control how objects receive and process damage.
//!
//! Original C++ location: GameLogic/Module/DamageModule.h/.cpp
//! Original C++ Author: Colin Day, September 2002
//! Rust conversion: 2025

pub mod bone_fx_damage;
pub mod damage_module;
pub mod transition_damage_fx;

// Re-export main types
pub use bone_fx_damage::BoneFXDamage;
pub use damage_module::{DamageModule, DamageModuleData};
pub use transition_damage_fx::{
    TransitionDamageFX, TransitionDamageFXModule, TransitionDamageFXModuleData,
};
