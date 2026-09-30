//! Shared, frozen data contracts between simulation construction and presentation consumers.
//!
//! This crate owns values that may cross the simulation/UI/render boundary. It contains no frame
//! builder, simulation adapter, UI behavior, renderer behavior, or dependency on `generals_main`.

pub mod command;
pub mod events;
pub mod hud;
pub mod objective;
pub mod weapon_visual_dispatch;

pub use command::{UnitCommandAvailability, UnitCommandButton};
pub use events::PresentationEvent;
pub use generals_game_domain::{AIDifficulty, CombatParticleKind, ObjectId, Team};
pub use hud::{
    PresentationHudFrame, PresentationPlayerInfo, PresentationPopupMessage,
    PresentationSuperweaponTimer,
};
pub use objective::{ObjectiveCategory, ObjectiveDisplay, ObjectiveStatus};

use serde::{Deserialize, Serialize};

/// Logic-frame index (30 Hz authority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LogicFrame(pub u32);
