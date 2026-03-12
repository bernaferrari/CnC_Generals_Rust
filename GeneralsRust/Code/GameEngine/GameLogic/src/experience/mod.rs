//! Complete Experience and Veterancy System
//!
//! This module provides the full veterancy and experience tracking system matching
//! the C++ implementation from Command & Conquer Generals Zero Hour.
//!
//! # System Components
//!
//! - **ExperienceTracker**: Lightweight per-object experience tracking
//! - **VeterancyBonuses**: Stat multipliers and special abilities per level
//! - **Experience Gain**: From damage dealt, kills, crates, and upgrades
//! - **Visual/Audio Feedback**: Promotion effects and UI updates
//!
//! # Veterancy Levels
//!
//! - **Regular (0)**: No bonuses
//! - **Veteran (1)**: +25% damage, +10% armor, +25% sight
//! - **Elite (2)**: +50% damage, +25% armor, +50% sight, +50% speed
//! - **Heroic (3)**: +100% damage, +50% armor, +100% sight, +100% speed, self-heal
//!
//! # Experience Formulas (matching C++ exactly)
//!
//! ```text
//! XP from damage = damage_dealt * 0.1
//! XP for kill = target_cost * 0.5
//! Veteran at: object_cost XP
//! Elite at: object_cost * 3 XP
//! Heroic at: object_cost * 6 XP
//!
//! Damage bonus = base_damage * (1.0 + level * 0.25)
//! Armor multiplier = 1.0 - (level * 0.1)  [takes less damage]
//! Speed bonus = base_speed * (1.0 + level * 0.25)
//! ```

mod bonuses;
mod gain;
mod integration;
mod requirements;
mod tracker;
mod visual;

pub use bonuses::*;
pub use gain::*;
pub use integration::*;
pub use requirements::*;
pub use tracker::*;
pub use visual::*;
