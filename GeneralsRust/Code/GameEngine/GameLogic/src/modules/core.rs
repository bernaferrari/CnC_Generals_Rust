// Imports, DestroyReason, AIAttitudeType, and re-exports
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

use crate::ai::states::AIStateType;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType, GuardMode};
use crate::common::audio::AudioEventRts;
use crate::common::*;
use crate::damage::{DamageInfo, DamageInfoInput};
use crate::helpers::TheGameLogic;
use crate::object::behavior::behavior_module::SpyVisionUpdate;
use crate::object::behavior::behavior_module::{
    BridgeBehaviorInterface, BridgeScaffoldBehaviorInterface, BridgeTowerBehaviorInterface,
    CaveInterface, LandMineInterface, OverchargeBehaviorInterface, ParkingPlaceBehaviorInterface,
    RebuildHoleBehaviorInterface, TransportPassengerInterface,
};
use crate::object::behavior::laser_update::LaserBehaviorControlInterface;
use crate::object::behavior::projectile_stream_update::ProjectileStreamUpdateInterface;
use crate::object::behavior::radius_decal_update::RadiusDecalUpdateInterface;
use crate::object::registry::OBJECT_REGISTRY;
pub use crate::object::update::special_power_update::SpecialPowerCommandOption;

/// Wave 340: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

pub type SpecialPowerCommandOptions = SpecialPowerCommandOption;
use crate::command_button::CommandButton;
use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::object::special_power_module::Waypoint;
use crate::object::update::ai_update::deliver_payload_data::DeliverPayloadData;
use crate::object::SpecialPowerTemplate;
use crate::player::PlayerIndex;
use game_engine::common::system::Xfer;
use std::any::Any;
/// Destroy reasons used by die/destroy modules (mirrors C++ DestroyReason).
#[derive(Debug, Clone, Copy)]
pub enum DestroyReason {
    Logical,
    Damage,
    Script,
}
use crate::locomotor::Locomotor;
use crate::object::body::body_module::ArmorSetType as BodyArmorSetType;
pub use crate::object::body::body_module::BodyModuleInterface;
use crate::object::Object;
pub use game_engine::common::thing::module::UpgradeMuxData;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock, Weak};

/// AI Attitude types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIAttitudeType {
    Normal,
    Aggressive,
    Defensive,
    Passive,
    Sleep,
}

// Forward declarations for convenience
pub use crate::ai::group::AIGroup;
pub use crate::player::Player;
pub use crate::team::Team;
pub use crate::weapon::{Weapon, WeaponSet};
