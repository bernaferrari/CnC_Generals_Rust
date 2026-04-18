////////////////////////////////////////////////////////////////////////////////
//																			//
//  (c) 2001-2003 Electronic Arts Inc.										//
//																			//
////////////////////////////////////////////////////////////////////////////////

//! Thing module - Contains the core entity system for the C&C Generals game engine
//!
//! This module provides the base classes and systems for managing game entities:
//! - **Thing**: Base class for all game entities (objects and drawables)
//! - **Module**: Pluggable component system for extending entity behavior
//! - **ThingTemplate**: Configuration data for creating entities
//! - **Factories**: Creation and management of entities and modules
//!
//! The Thing system is designed around composition over inheritance, using a
//! module-based architecture where different behaviors can be attached to entities
//! through modules. This allows for flexible and reusable game object definitions.
//!
//! ## Core Components
//!
//! ### Things
//! Things are the base entities in the game world. They come in two primary forms:
//! - **Objects**: Game logic entities that handle simulation, AI, and gameplay
//! - **Drawables**: Rendering entities that handle visual representation
//!
//! ### Modules
//! Modules provide specific functionality that can be attached to Things:
//! - **BehaviorModules**: Game logic behaviors (AI, movement, combat, etc.)
//! - **DrawModules**: Rendering behaviors (models, particles, effects, etc.)
//! - **ClientUpdateModules**: Client-side update logic
//!
//! ### Templates
//! ThingTemplates define the configuration data needed to create Things:
//! - Module lists and configurations
//! - Physical properties (geometry, scale, etc.)
//! - Audio assignments
//! - Build requirements and costs
//! - Weapon and armor configurations
//!
//! ## Architecture
//!
//! The module system follows these principles:
//! 1. **Composition**: Things are composed of modules rather than inheriting behavior
//! 2. **Data-Driven**: Configuration comes from INI files and templates
//! 3. **Factory Pattern**: Centralized creation through factories
//! 4. **Interface Segregation**: Modules implement specific interfaces for their functionality
//!
//! ## Usage Examples
//!
//! ```rust
//! use rust_game_engine::common::thing::*;
//!
//! // Create a new thing template
//! let mut template = ThingTemplate::new();
//! template.set_template_name("MyUnit".into());
//!
//! // Configure the template through INI parsing
//! // template.parse_from_ini(&mut ini_data);
//!
//! // Create objects from the template
//! // let factory = get_thing_factory()?;
//! // let object = factory.new_object(&template, Some(team), status_bits)?;
//! ```

pub mod draw_module;
pub mod module;
pub mod module_factory;
pub mod sparse_match_finder;
pub mod thing;
pub mod thing_factory;
pub mod thing_template;

// Re-export the main types for easier access
pub use draw_module::{
    DebrisDrawInterface, DrawModule, DrawableModuleTrait, LaserDrawInterface, ModuleInterfaceFlags,
    ModuleType, ObjectDrawInterface, RgbColor, RopeDrawInterface, ShadowType, TerrainDecalType,
    TracerDrawInterface,
};

#[cfg(any(debug_assertions, feature = "internal"))]
pub use draw_module::RenderCost;

pub use module::{
    BaseDrawableModule, BaseModule, BaseModuleData, BaseObjectModule, Drawable, DrawableModule,
    Module, ModuleData, ModuleInterfaceType, Object, ObjectModule, Player, StaticGameLodLevel,
    Thing as ThingTrait, TimeOfDay, UpgradeMuxData,
};

pub use module_factory::{
    get_module_factory, init_module_factory, shutdown_module_factory, ModuleFactory,
    ModuleTemplate, NewModuleDataProc, NewModuleProc,
};

pub use thing::{
    register_terrain_height_provider, register_underwater_provider, BaseThing, KindOfMaskType,
    KindOfType,
};

pub use thing_factory::{
    get_thing_factory, init_thing_factory, load_templates_from_ini_text, shutdown_thing_factory,
    DrawableStatus, ObjectStatusMaskType, Team, ThingCreationError, ThingFactory, ThingLoadType,
    DRAWABLE_STATUS_NONE, OBJECT_STATUS_MASK_NONE,
};

pub use thing_template::{
    ArmorTemplateSet, AudioArray, BuildCompletionType, BuildableStatus, EditorSortingType,
    ModuleInfo, PerUnitFxMap, PerUnitSoundMap, RadarPriorityType, ThingTemplate,
    ThingTemplateAudioType, WeaponTemplateSet, LEVEL_COUNT, MAX_UPGRADE_CAMEO_UPGRADES,
};

/// Initialize the entire Thing subsystem
///
/// This function initializes both the ModuleFactory and ThingFactory singletons
/// and should be called during engine startup.
///
/// # Errors
///
/// Returns an error if either factory fails to initialize.
///
/// # Examples
///
/// ```rust
/// use rust_game_engine::common::thing;
///
/// fn initialize_engine() -> Result<(), String> {
///     thing::init_thing_system()?;
///     // ... initialize other subsystems
///     Ok(())
/// }
/// ```
pub fn init_thing_system() -> Result<(), String> {
    init_module_factory()?;
    init_thing_factory()?;
    Ok(())
}

/// Shutdown the entire Thing subsystem
///
/// This function shuts down both factories and should be called during
/// engine cleanup to ensure proper resource deallocation.
///
/// # Examples
///
/// ```rust
/// use rust_game_engine::common::thing;
///
/// fn shutdown_engine() {
///     thing::shutdown_thing_system();
///     // ... shutdown other subsystems
/// }
/// ```
pub fn shutdown_thing_system() {
    shutdown_thing_factory();
    shutdown_module_factory();
}

/// Convenience function to check if Thing system is initialized
///
/// Returns true if both the ModuleFactory and ThingFactory are initialized
/// and ready for use.
///
/// # Examples
///
/// ```rust
/// use rust_game_engine::common::thing;
///
/// if thing::is_thing_system_initialized() {
///     // Safe to create objects and modules
/// } else {
///     // Need to initialize first
///     thing::init_thing_system()?;
/// }
/// ```
pub fn is_thing_system_initialized() -> bool {
    let module_factory_ok = get_module_factory()
        .map(|guard| guard.is_some())
        .unwrap_or(false);

    let thing_factory_ok = get_thing_factory()
        .map(|guard| guard.is_some())
        .unwrap_or(false);

    module_factory_ok && thing_factory_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thing_system_initialization() {
        // Test system initialization
        assert!(init_thing_system().is_ok());
        assert!(is_thing_system_initialized());

        // Test shutdown
        shutdown_thing_system();
    }

    #[test]
    fn test_thing_template_creation() {
        let template = ThingTemplate::new();
        assert!(template.get_name().is_empty());
        assert_eq!(template.get_template_id(), 0);
        assert_eq!(template.get_build_cost(), 0);
        assert_eq!(template.get_build_time(), 1.0);
    }

    #[test]
    fn test_module_info() {
        let mut module_info = ModuleInfo::new();
        assert_eq!(module_info.get_count(), 0);

        module_info.clear();
        assert_eq!(module_info.get_count(), 0);
    }

    #[test]
    fn test_audio_array() {
        let mut audio_array = AudioArray::new();
        assert!(audio_array
            .get(ThingTemplateAudioType::VoiceSelect)
            .is_none());

        // Would need actual AudioEventRts to test setting
        // audio_array.set(ThingTemplateAudioType::VoiceSelect, audio_event);
        // assert!(audio_array.get(ThingTemplateAudioType::VoiceSelect).is_some());
    }

    #[test]
    fn test_module_interface_flags() {
        let update_flag = ModuleInterfaceFlags::UPDATE;
        let draw_flag = ModuleInterfaceFlags::DRAW;

        assert_ne!(update_flag.0, draw_flag.0);

        let combined = ModuleInterfaceFlags(update_flag.0 | draw_flag.0);
        assert_ne!(combined.0, update_flag.0);
        assert_ne!(combined.0, draw_flag.0);
    }

    #[test]
    fn test_base_thing_creation() {
        use std::sync::Arc;

        let template = Arc::new(ThingTemplate::new());

        // This would fail in the real implementation since the template is "null"
        // let thing = BaseThing::new(template);
        // We can't test this without a proper template
    }
}
