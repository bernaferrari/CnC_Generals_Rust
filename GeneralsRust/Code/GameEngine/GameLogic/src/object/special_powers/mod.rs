// FILE: mod.rs
// Special powers module - Complete port of C++ special power system
// Author: Rust Port
// Desc: Module aggregation for special power system

// Core types and templates are in parent object module
use crate::object::special_power_module;
use crate::object::special_power_template;
use crate::object::special_power_types;

// Concrete special power implementations
pub mod baikonur_launch_power;
pub mod cash_bounty_power;
pub mod cash_hack_special_power;
pub mod cleanup_area_power;
pub mod defector_special_power;
pub mod demoralize_special_power;
pub mod fire_weapon_power;
pub mod ocl_special_power;
pub mod special_ability;
pub mod spy_vision_special_power;

// Re-export commonly used types
pub use crate::common::Coord3D;
pub use special_power_module::{
    FrameCount, ObjectId, SpecialPowerCommandOptions, SpecialPowerModule, SpecialPowerModuleData,
    SpecialPowerModuleInterface, Waypoint,
};
pub use special_power_template::{
    AcademyClassificationType, AudioEventRts, SpecialPowerStore, SpecialPowerTemplate,
};
pub use special_power_types::{SpecialPowerMask, SpecialPowerType};

// Re-export concrete special power types
pub use crate::object_creation_list::ObjectCreationList;
pub use baikonur_launch_power::{BaikonurLaunchPower, BaikonurLaunchPowerModuleData};
pub use cash_bounty_power::{CashBountyPower, CashBountyPowerModuleData};
pub use cash_hack_special_power::{
    CashHackSpecialPower, CashHackSpecialPowerModuleData, CashHackUpgrade,
};
pub use cleanup_area_power::{CleanupAreaPower, CleanupAreaPowerModuleData};
pub use defector_special_power::{DefectorSpecialPower, DefectorSpecialPowerModuleData};
pub use demoralize_special_power::{DemoralizeSpecialPower, DemoralizeSpecialPowerModuleData};
pub use fire_weapon_power::{FireWeaponPower, FireWeaponPowerModuleData};
pub use ocl_special_power::{OclCreateLocType, OclSpecialPower, OclSpecialPowerModuleData};
pub use special_ability::{SpecialAbility, SpecialAbilityModuleData};
pub use spy_vision_special_power::{SpyVisionSpecialPower, SpyVisionSpecialPowerModuleData};

/// Special power system summary
///
/// This module provides a complete port of the C&C Generals Zero Hour special power system.
///
/// ## Architecture
///
/// The special power system is organized in a hierarchy:
///
/// 1. **Special Power Template** - Defines the properties of a special power (reload time, science requirements, etc.)
/// 2. **Special Power Module** - Base module that manages cooldown, pausing, and execution
/// 3. **Special Power Module Interface** - Trait defining the contract for all special powers
/// 4. **Concrete Implementations** - Specific power types (OCL, Defector, Cash Hack, Spy Vision, etc.)
///
/// ## Key Concepts
///
/// ### Cooldown Management
///
/// Special powers have a reload time after use. The system tracks:
/// - Available frame (when power becomes usable)
/// - Paused count (reference counting for multiple pause sources)
/// - Paused percent (progress when paused for UI display)
///
/// ### Shared vs Local Powers
///
/// Powers can be:
/// - **Local** - Each building has its own cooldown
/// - **Shared/Synced** - All buildings of same type share one cooldown (e.g., superweapons)
///
/// ### Targeting
///
/// Powers can target:
/// - No target (self-cast)
/// - Object (unit or building)
/// - Location (ground target)
/// - Waypoints (for aircraft delivery)
///
/// ### Science Requirements
///
/// Powers may require research/upgrades before use. Upgrades can also modify:
/// - OCL objects spawned
/// - Cash hack amount stolen
/// - Effect duration
///
/// ## C++ Compatibility
///
/// This port faithfully matches the C++ implementation:
///
/// - Frame timing matches exactly
/// - Cooldown algorithm is identical
/// - State machine transitions match
/// - Upgrade system mirrors C++ logic
///
/// ## Usage Example
///
/// ```rust,ignore
/// use special_powers::*;
///
/// // Create a special power template
/// let template = SpecialPowerTemplate::new("CarpetBomb".to_string(), 1)
///     .with_power_type(SpecialPowerType::CarpetBomb)
///     .with_reload_time(3000)  // 3000 frames
///     .with_public_timer(true);
///
/// // Create an OCL special power
/// let mut data = OclSpecialPowerModuleData::default();
/// data.base.special_power_template = Some(Arc::new(template));
/// data.create_loc = OclCreateLocType::CreateAtEdgeNearTarget;
///
/// let mut power = OclSpecialPower::new(module_name_key, object_id, Arc::new(data));
///
/// // Initialize the power
/// power.on_special_power_creation();
///
/// // Check if ready
/// if power.is_ready() {
///     // Execute at location
///     power.do_special_power_at_location(&target_pos, 0.0, 0);
/// }
///
/// // Check progress
/// let progress = power.get_percent_ready();  // 0.0 to 1.0
/// ```
///
/// ## Special Power Types
///
/// ### OCL Special Power
/// Spawns objects from an Object Creation List at various locations (edges, target, above).
/// Used for: Carpet bomb, paradrop, A-10 strike, etc.
///
/// ### Defector Special Power
/// Converts enemy units to player's side with detection delay.
/// Used for: Defector special ability
///
/// ### Cash Hack Special Power
/// Steals money from enemy player's building, with upgrades increasing amount.
/// Used for: Cash hack, Black Lotus steal cash
///
/// ### Spy Vision Special Power
/// Reveals enemy vision, duration scales with captured prisoners.
/// Used for: Spy satellite
///
/// ## Testing
///
/// All modules include comprehensive unit tests. Run with:
/// ```bash
/// cargo test --lib special_powers
/// ```
///
/// ## References
///
/// C++ Source Files:
/// - `/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/SpecialPowerModule.cpp`
/// - `/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/OCLSpecialPower.cpp`
/// - `/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/DefectorSpecialPower.cpp`
/// - `/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/CashHackSpecialPower.cpp`
/// - `/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/SpyVisionSpecialPower.cpp`
/// - `/GeneralsMD/Code/GameEngine/Include/Common/SpecialPower.h`
/// - `/GeneralsMD/Code/GameEngine/Include/Common/SpecialPowerType.h`

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_power_type_enum() {
        assert_eq!(SpecialPowerType::CarpetBomb.as_str(), "SPECIAL_CARPET_BOMB");
        assert_eq!(
            SpecialPowerType::from_str("SPECIAL_CARPET_BOMB"),
            Some(SpecialPowerType::CarpetBomb)
        );
    }

    #[test]
    fn test_special_power_template_creation() {
        let template = SpecialPowerTemplate::new("TestPower".to_string(), 1).with_reload_time(1000);

        assert_eq!(template.get_name(), "TestPower");
        assert_eq!(template.get_reload_time(), 1000);
    }

    #[test]
    fn test_special_power_store() {
        let mut store = SpecialPowerStore::new();

        let template = SpecialPowerTemplate::new("Power1".to_string(), 1);
        store.add_template(template);

        assert_eq!(store.get_num_special_powers(), 1);
        assert!(store.find_special_power_template("Power1").is_some());
    }
}
