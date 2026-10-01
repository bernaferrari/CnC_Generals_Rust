//! Object Behavior Modules - Rust conversion of C++ Object Behavior classes
//!
//! This module contains the Rust implementations of 60+ object behavior modules
//! that control how game objects behave, respond to damage, spawn other objects,
//! and handle various game mechanics.
//!
//! These modules implement the behavior pattern where game objects can have multiple
//! behaviors attached to them to control different aspects of their functionality.
//!
//! Original C++ Authors: Various EA developers (2001-2003)
//! Rust conversion: 2025

// Core behavior modules
pub mod auto_heal_behavior;
pub mod battle_bus_slow_death_behavior;
pub mod behavior_module;
pub mod bridge_behavior;
pub mod bridge_scaffold_behavior;
pub mod bridge_tower_behavior;
pub mod dumb_projectile_behavior;
pub mod fire_weapon_update;
pub mod fire_weapon_when_damaged_behavior;
pub mod fire_weapon_when_damaged_behavior_new;
pub mod fire_weapon_when_dead_behavior;
pub mod firing_tracker_behavior;
pub mod propaganda_center_behavior;
pub mod propaganda_tower_behavior;
pub mod slow_death_behavior;
pub mod spawn_behavior;
pub mod supply_warehouse_crippling_behavior;
pub mod tech_building_behavior;

// Stealth modules (5 modules)
pub mod grant_stealth_behavior;
pub mod spy_vision_update;
pub mod stealth_detector_update;
pub mod stealth_update;

// Base/Building modules (10 modules)
pub mod base_regenerate_update;
pub mod base_renerate_update;
pub mod bone_fx_update;
pub mod bunker_buster_behavior;
pub mod command_button_hunt_update;
pub mod default_production_exit_behavior;
pub mod dock_update;
pub mod overcharge_behavior;
pub mod parking_place_behavior;
pub mod power_plant_update;
pub mod production_update;
pub mod production_update_behavior;
pub mod queue_production_exit_behavior;
pub mod radar_update;
pub mod spawn_point_production_exit_behavior;
pub mod supply_center_production_exit_behavior;

// Combat/Weapon behaviors (4 modules)
pub mod countermeasures_behavior;
pub mod flight_deck_behavior;
pub mod generate_minefield_behavior;
pub mod minefield_behavior;
pub mod pow_truck_behavior;
pub mod prison_behavior;
pub mod rebuild_hole_behavior;

// Special Ability modules (10 modules)
pub mod assisted_targeting_update;
pub mod auto_deposit_update;
pub mod auto_find_healing_update;
pub mod cleanup_hazard_update;
pub mod emp_update;
pub mod enemy_near_update;
pub mod fire_spread_update;
pub mod flammable_update;
pub mod hijacker_update;
pub mod laser_update;
pub mod leaflet_drop_behavior;
pub mod physics_update;
pub mod pilot_find_vehicle_update;
pub mod poisoned_behavior;
pub mod special_ability_update;
pub mod special_power_update_module;
pub mod update_module;

// Horde/Formation modules (5 modules)
pub mod animation_steering_update;
pub mod battle_plan_update;
pub mod horde_update;
pub mod mob_member_slaved_update;
pub mod slaved_update;
pub mod tensile_formation_update;

// Special Vehicle modules (7 modules)
pub mod deletion_update;
pub mod height_die_update;
pub mod helicopter_slow_death_behavior;
pub mod helicopter_slow_death_update;
pub mod instant_death_behavior;
pub mod jet_slow_death_behavior;
pub mod lifetime_update;
pub mod neutron_blast_behavior;
pub mod neutron_missile_slow_death_update;
pub mod neutron_missile_update;
pub mod structure_collapse_update;
pub mod structure_topple_update;
pub mod topple_update;

// Weapon modules (5 modules)
pub mod checkpoint_update;
pub mod demo_trap_update;
pub mod dynamic_geometry_info_update;
pub mod dynamic_shroud_clearing_range_update;
pub mod fire_ocl_after_weapon_cooldown_update;
pub mod firestorm_dynamic_geometry_info_update;
pub mod float_update;
pub mod missile_launcher_building_update;
pub mod particle_uplink_cannon_update;
pub mod point_defense_laser_update;
pub mod projectile_stream_update;
pub mod prone_update;
pub mod radius_decal_update;
pub mod smart_bomb_target_homing_update;
pub mod spectre_gunship_deployment_update;
pub mod spectre_gunship_update;
pub mod sticky_bomb_update;
pub mod wave_guide_update;
pub mod weapon_bonus_update;

// Modern behavior system (optional features)
#[cfg(feature = "modern_behaviors")]
pub mod advanced_behavior_system;
#[cfg(feature = "modern_behaviors")]
pub mod behavior_integration;
#[cfg(feature = "modern_behaviors")]
pub mod formation_behavior;
#[cfg(feature = "modern_behaviors")]
pub mod stealth_behavior;

// Re-export main types and interfaces
pub use slow_death_behavior::{
    DieMuxData as SlowDeathDieMuxData, SlowDeathBehavior, SlowDeathBehaviorInterface,
    SlowDeathBehaviorModuleData, SlowDeathPhaseType,
};

pub use spawn_behavior::{
    DieMuxData as SpawnDieMuxData, SpawnBehavior, SpawnBehaviorInterface, SpawnBehaviorModuleData,
};

pub use supply_warehouse_crippling_behavior::{
    SupplyWarehouseCripplingBehavior, SupplyWarehouseCripplingBehaviorModuleData,
};

pub use auto_heal_behavior::{AutoHealBehavior, AutoHealBehaviorModuleData};
pub use bridge_behavior::{BridgeBehavior, BridgeBehaviorModule, BridgeBehaviorModuleData};
pub use dumb_projectile_behavior::{DumbProjectileBehavior, DumbProjectileBehaviorModuleData};
pub use fire_weapon_when_dead_behavior::{
    FireWeaponWhenDeadBehavior, FireWeaponWhenDeadBehaviorFactory,
    FireWeaponWhenDeadBehaviorModule, FireWeaponWhenDeadBehaviorModuleData,
};
#[cfg(feature = "allow_surrender")]
pub use propaganda_center_behavior::{
    PropagandaCenterBehavior, PropagandaCenterBehaviorModule, PropagandaCenterBehaviorModuleData,
};
pub use propaganda_tower_behavior::{
    PropagandaTowerBehavior, PropagandaTowerBehaviorModule, PropagandaTowerBehaviorModuleData,
};
pub use rebuild_hole_behavior::{
    RebuildHoleBehavior, RebuildHoleBehaviorModule, RebuildHoleBehaviorModuleData,
};
pub use tech_building_behavior::{TechBuildingBehavior, TechBuildingBehaviorModuleData};

// Stealth module exports
pub use grant_stealth_behavior::{
    GrantStealthBehavior, GrantStealthBehaviorFactory, GrantStealthBehaviorModuleData,
};
pub use spy_vision_update::*;
pub use stealth_detector_update::{
    StealthDetectorUpdate, StealthDetectorUpdateFactory, StealthDetectorUpdateModuleData,
    stealth_detector_ctor_wake_frames, stealth_detector_kindof_allows,
    stealth_or_detector_update_processes,
};
pub use stealth_update::{StealthUpdate, StealthUpdateFactory, StealthUpdateModuleData};

// Base/Building module exports
pub use base_regenerate_update::{
    BaseRegenerateUpdate, BaseRegenerateUpdateFactory, BaseRegenerateUpdateModule,
    BaseRegenerateUpdateModuleData,
};
pub use base_renerate_update::*;
pub use bone_fx_update::*;
pub use bunker_buster_behavior::{
    BUNKER_BUSTER_HARM_AND_FORCE_EXIT_AMOUNT, BunkerBusterBehavior, BunkerBusterBehaviorFactory,
    BunkerBusterBehaviorModule, BunkerBusterBehaviorModuleData,
};
pub use command_button_hunt_update::*;
pub use default_production_exit_behavior::{
    DefaultProductionExitBehavior, DefaultProductionExitBehaviorModule,
    DefaultProductionExitModuleData,
};
pub use overcharge_behavior::{
    OverchargeBehavior, OverchargeBehaviorModule, OverchargeBehaviorModuleData,
};
pub use parking_place_behavior::{
    ParkingPlaceBehavior, ParkingPlaceBehaviorFactory, ParkingPlaceBehaviorModule,
    ParkingPlaceBehaviorModuleData,
};
pub use poisoned_behavior::{PoisonedBehavior, PoisonedBehaviorModule, PoisonedBehaviorModuleData};
pub use power_plant_update::{
    PowerPlantUpdate, PowerPlantUpdateFactory, PowerPlantUpdateModuleData,
};
pub use production_update::{
    ProductionUpdate, ProductionUpdateFactory, ProductionUpdateModuleData,
};
pub use production_update_behavior::{
    CanMakeType, ProductionEntry, ProductionID, ProductionType, ProductionUpdateBehavior,
    ProductionUpdateModuleData as ProductionUpdateBehaviorData, QuantityModifier,
};
pub use queue_production_exit_behavior::{
    ExitResult, QueueProductionExitBehavior, QueueProductionExitModuleData,
};
pub use radar_update::{RadarUpdate, RadarUpdateFactory, RadarUpdateModuleData};
pub use spawn_point_production_exit_behavior::{
    SpawnPointProductionExitBehavior, SpawnPointProductionExitBehaviorModule,
    SpawnPointProductionExitModuleData,
};
pub use supply_center_production_exit_behavior::{
    SupplyCenterProductionExitBehavior, SupplyCenterProductionExitBehaviorModule,
    SupplyCenterProductionExitModuleData,
};

// Combat/Weapon behavior exports
pub use countermeasures_behavior::{
    CountermeasuresBehavior, CountermeasuresBehaviorFactory, CountermeasuresBehaviorModuleData,
};
pub use flight_deck_behavior::{
    FlightDeckBehavior, FlightDeckBehaviorFactory, FlightDeckBehaviorModuleData,
};
pub use generate_minefield_behavior::{
    GenerateMinefieldBehavior, GenerateMinefieldBehaviorFactory, GenerateMinefieldBehaviorModule,
    GenerateMinefieldBehaviorModuleData,
};
pub use minefield_behavior::{
    MinefieldBehavior, MinefieldBehaviorFactory, MinefieldBehaviorModule,
    MinefieldBehaviorModuleData,
};

// Special Ability module exports
pub use assisted_targeting_update::{
    AssistedTargetingUpdate, AssistedTargetingUpdateFactory, AssistedTargetingUpdateModule,
    AssistedTargetingUpdateModuleData,
};
pub use auto_deposit_update::{
    AutoDepositUpdate, AutoDepositUpdateFactory, AutoDepositUpdateModule,
    AutoDepositUpdateModuleData,
};
pub use auto_find_healing_update::{
    AutoFindHealingUpdate, AutoFindHealingUpdateFactory, AutoFindHealingUpdateModule,
    AutoFindHealingUpdateModuleData,
};
pub use cleanup_hazard_update::{
    CleanupHazardUpdate, CleanupHazardUpdateFactory, CleanupHazardUpdateModule,
    CleanupHazardUpdateModuleData,
};
pub use emp_update::{EMPUpdate, EMPUpdateFactory, EMPUpdateModule, EMPUpdateModuleData};
pub use enemy_near_update::{
    EnemyNearUpdate, EnemyNearUpdateFactory, EnemyNearUpdateModule, EnemyNearUpdateModuleData,
};
pub use fire_spread_update::*;
pub use fire_weapon_when_damaged_behavior::*;
pub use fire_weapon_when_damaged_behavior_new::{
    FireWeaponWhenDamagedBehavior, FireWeaponWhenDamagedBehaviorFactory,
    FireWeaponWhenDamagedBehaviorModule, FireWeaponWhenDamagedBehaviorModuleData,
};
pub use flammable_update::{FlammableUpdate, FlammableUpdateFactory, FlammableUpdateModuleData};
pub use hijacker_update::{HijackerUpdate, HijackerUpdateFactory, HijackerUpdateModuleData};
pub use physics_update::{
    PhysicsBehaviorFactory, PhysicsBehaviorModuleData, PhysicsBehaviorUpdate,
};
pub use pilot_find_vehicle_update::{
    PilotFindVehicleUpdate, PilotFindVehicleUpdateFactory, PilotFindVehicleUpdateModuleData,
};
pub use special_ability_update::{
    SpecialAbilityUpdate, SpecialAbilityUpdateFactory, SpecialAbilityUpdateModuleData,
};
pub use special_power_update_module::*;
pub use update_module::*;

// Horde/Formation module exports
pub use animation_steering_update::{
    AnimationSteeringUpdate, AnimationSteeringUpdateFactory, AnimationSteeringUpdateModuleData,
};
pub use battle_plan_update::{
    BattlePlanUpdate, BattlePlanUpdateFactory, BattlePlanUpdateModuleData,
};
pub use horde_update::{HordeUpdate, HordeUpdateFactory, HordeUpdateModuleData};
pub use mob_member_slaved_update::{
    MobMemberSlavedUpdate, MobMemberSlavedUpdateFactory, MobMemberSlavedUpdateModuleData,
};
pub use slaved_update::*;
pub use tensile_formation_update::{
    TensileFormationUpdate, TensileFormationUpdateFactory, TensileFormationUpdateModule,
    TensileFormationUpdateModuleData,
};

// Special Vehicle module exports
pub use deletion_update::{DeletionUpdate, DeletionUpdateFactory, DeletionUpdateModuleData};
pub use height_die_update::{HeightDieUpdate, HeightDieUpdateFactory, HeightDieUpdateModuleData};
pub use helicopter_slow_death_behavior::{
    HelicopterSlowDeathBehavior, HelicopterSlowDeathBehaviorFactory,
    HelicopterSlowDeathBehaviorModuleData,
};
pub use helicopter_slow_death_update::*;
pub use jet_slow_death_behavior::{
    JetSlowDeathBehavior, JetSlowDeathBehaviorModule, JetSlowDeathBehaviorModuleData,
};
pub use lifetime_update::{LifetimeUpdate, LifetimeUpdateFactory, LifetimeUpdateModuleData};
pub use neutron_blast_behavior::{
    NeutronBlastBehavior, NeutronBlastBehaviorFactory, NeutronBlastBehaviorModuleData,
};
pub use neutron_missile_slow_death_update::{
    NeutronMissileSlowDeathUpdate, NeutronMissileSlowDeathUpdateFactory,
    NeutronMissileSlowDeathUpdateModuleData,
};
pub use neutron_missile_update::*;
pub use structure_collapse_update::{
    StructureCollapseUpdate, StructureCollapseUpdateFactory, StructureCollapseUpdateModule,
    StructureCollapseUpdateModuleData,
};
pub use topple_update::{ToppleUpdate, ToppleUpdateFactory, ToppleUpdateModuleData};

// Weapon module exports
pub use crate::object::update::missile_ai_update::{
    MissileAIUpdateBehavior, MissileAIUpdateFactory, MissileAIUpdateModuleData,
};
pub use checkpoint_update::{
    CheckpointUpdate, CheckpointUpdateFactory, CheckpointUpdateModuleData,
};
pub use demo_trap_update::DemoTrapUpdateModule;
pub use demo_trap_update::{DemoTrapUpdate, DemoTrapUpdateFactory, DemoTrapUpdateModuleData};
pub use dynamic_geometry_info_update::{
    DynamicGeometryInfoUpdate, DynamicGeometryInfoUpdateFactory,
    DynamicGeometryInfoUpdateModuleData,
};
pub use dynamic_shroud_clearing_range_update::{
    DynamicShroudClearingRangeUpdate, DynamicShroudClearingRangeUpdateFactory,
    DynamicShroudClearingRangeUpdateModule, DynamicShroudClearingRangeUpdateModuleData,
};
pub use fire_ocl_after_weapon_cooldown_update::{
    FireOCLAfterWeaponCooldownUpdate, FireOCLAfterWeaponCooldownUpdateFactory,
    FireOCLAfterWeaponCooldownUpdateModule, FireOCLAfterWeaponCooldownUpdateModuleData,
};
pub use fire_weapon_update::{
    FireWeaponUpdate, FireWeaponUpdateFactory, FireWeaponUpdateModule, FireWeaponUpdateModuleData,
};
pub use firestorm_dynamic_geometry_info_update::{
    FirestormDynamicGeometryInfoUpdate, FirestormDynamicGeometryInfoUpdateFactory,
    FirestormDynamicGeometryInfoUpdateModuleData, MAX_FIRESTORM_SYSTEMS,
};
pub use float_update::{FloatUpdate, FloatUpdateFactory, FloatUpdateModule, FloatUpdateModuleData};
pub use laser_update::{LaserUpdate, LaserUpdateFactory, LaserUpdateModule, LaserUpdateModuleData};
pub use leaflet_drop_behavior::{
    LeafletDropBehavior, LeafletDropBehaviorFactory, LeafletDropBehaviorModuleData,
};
pub use missile_launcher_building_update::{
    MissileLauncherBuildingUpdate, MissileLauncherBuildingUpdateFactory,
    MissileLauncherBuildingUpdateModuleData,
};
pub use particle_uplink_cannon_update::{
    ParticleUplinkCannonUpdate, ParticleUplinkCannonUpdateFactory,
    ParticleUplinkCannonUpdateModuleData,
};
pub use point_defense_laser_update::{
    PointDefenseLaserUpdate, PointDefenseLaserUpdateFactory, PointDefenseLaserUpdateModule,
    PointDefenseLaserUpdateModuleData,
};
pub use projectile_stream_update::{
    ProjectileStreamUpdate, ProjectileStreamUpdateFactory, ProjectileStreamUpdateModule,
    ProjectileStreamUpdateModuleData,
};
pub use prone_update::{
    ProneUpdate, ProneUpdateFactory, ProneUpdateInterface, ProneUpdateModule, ProneUpdateModuleData,
};
pub use radius_decal_update::{
    RadiusDecalUpdate, RadiusDecalUpdateFactory, RadiusDecalUpdateInterface,
    RadiusDecalUpdateModuleData,
};
pub use smart_bomb_target_homing_update::{
    SmartBombTargetHomingUpdate, SmartBombTargetHomingUpdateFactory,
    SmartBombTargetHomingUpdateInterface, SmartBombTargetHomingUpdateModule,
    SmartBombTargetHomingUpdateModuleData,
};
pub use spectre_gunship_deployment_update::{
    SpectreGunshipDeploymentUpdate, SpectreGunshipDeploymentUpdateFactory,
    SpectreGunshipDeploymentUpdateModuleData,
};
pub use spectre_gunship_update::{
    SpectreGunshipUpdate, SpectreGunshipUpdateFactory, SpectreGunshipUpdateModuleData,
};
pub use sticky_bomb_update::{
    StickyBombUpdate, StickyBombUpdateFactory, StickyBombUpdateModule, StickyBombUpdateModuleData,
};
pub use structure_topple_update::{
    StructureToppleUpdate, StructureToppleUpdateFactory, StructureToppleUpdateModuleData,
    leftover_apply_crushing_damage_js, leftover_do_damage_line_offsets,
    leftover_structure_topple_crush_points, leftover_structure_topple_facing_width,
    leftover_structure_topple_max_crush_distance,
};
pub use wave_guide_update::{
    LeftoverWaveGuideAudio, WAVE_GUIDE_LOOPING_SOUND, WAVE_GUIDE_RANDOM_SPLASH_FREQUENCY,
    WAVE_GUIDE_RANDOM_SPLASH_SOUND, WaveGuideUpdate, WaveGuideUpdateFactory,
    WaveGuideUpdateModuleData, leftover_play_wave_guide_audio_event,
    leftover_play_wave_guide_named_audio, leftover_wave_guide_audio_from_template,
    leftover_wave_guide_splash_due, leftover_wave_guide_splash_roll,
};
pub use weapon_bonus_update::{
    WeaponBonusUpdate, WeaponBonusUpdateFactory, WeaponBonusUpdateModule,
    WeaponBonusUpdateModuleData,
};

#[cfg(feature = "modern_behaviors")]
pub use advanced_behavior_system::{
    AsyncBehavior, BehaviorEvent, BehaviorManager, BehaviorOutcome, BehaviorState,
};

#[cfg(feature = "modern_behaviors")]
pub use stealth_behavior::{StealthBehavior, StealthConfig, StealthState};

#[cfg(feature = "modern_behaviors")]
pub use formation_behavior::{
    FormationBehavior, FormationConfig, FormationState, FormationType, LeaderStrategy,
};

#[cfg(feature = "modern_behaviors")]
pub use behavior_integration::{
    BehaviorConfiguration, BehaviorConfigurationBuilder, BehaviorFactory, IntegratedBehaviorSystem,
    LegacyBehaviorAdapter,
};

use crate::common::ModuleData;
use crate::object::Object;
use std::sync::{Arc, RwLock};

/// Trait for creating behavior modules from module data
pub trait BehaviorModuleFactory {
    /// Create a new behavior module instance
    fn create_behavior(
        thing: Arc<RwLock<Object>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<
        Box<dyn crate::modules::BehaviorModuleInterface>,
        Box<dyn std::error::Error + Send + Sync>,
    >;
}

/// A registered behavior factory closure.
pub type BehaviorModuleFactoryFn = Box<
    dyn Fn(
            Arc<RwLock<Object>>,
            Arc<dyn ModuleData>,
        ) -> Result<
            Box<dyn crate::modules::BehaviorModuleInterface>,
            Box<dyn std::error::Error + Send + Sync>,
        > + Send
        + Sync,
>;

/// Registry for behavior module factories
pub struct BehaviorModuleRegistry {
    factories: std::collections::HashMap<String, BehaviorModuleFactoryFn>,
}

impl BehaviorModuleRegistry {
    /// Create a new registry with all behavior factories
    pub fn new() -> Self {
        let mut registry = Self {
            factories: std::collections::HashMap::new(),
        };

        // Core behavior factories
        registry.register_factory(
            "SlowDeathBehavior",
            Box::new(|thing: Arc<RwLock<Object>>, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                SlowDeathBehavior::new(object_id, data)
                    .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        registry.register_factory(
            "SpawnBehavior",
            Box::new(
                |thing: std::sync::Arc<std::sync::RwLock<crate::object::Object>>, data| {
                    let object_id = thing
                        .read()
                        .ok()
                        .map(|g| g.get_id())
                        .unwrap_or(crate::common::INVALID_ID);
                    SpawnBehavior::new(object_id, data)
                        .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
                },
            ),
        );

        registry.register_factory(
            "FireWeaponWhenDeadBehavior",
            Box::new(|thing: Arc<RwLock<Object>>, data: Arc<dyn ModuleData>| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                let mut behavior = FireWeaponWhenDeadBehavior::new(object_id, data)?;
                if let Ok(mut obj) = thing.write() {
                    behavior.give_self_upgrade(&mut obj);
                }
                Ok(Box::new(behavior) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        registry.register_factory(
            "SupplyWarehouseCripplingBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                SupplyWarehouseCripplingBehavior::new(object_id, data)
                    .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        registry.register_factory(
            "TechBuildingBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                TechBuildingBehavior::new(object_id, data)
                    .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        registry.register_factory(
            "PropagandaTowerBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                PropagandaTowerBehavior::new(object_id, data)
                    .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        #[cfg(feature = "allow_surrender")]
        registry.register_factory(
            "PropagandaCenterBehavior",
            Box::new(|thing: Arc<RwLock<Object>>, data: Arc<dyn ModuleData>| {
                let typed = data
                    .as_any()
                    .downcast_ref::<PropagandaCenterBehaviorModuleData>()
                    .ok_or_else(|| {
                        Box::<dyn std::error::Error + Send + Sync>::from(
                            "PropagandaCenterBehaviorModuleData expected",
                        )
                    })?;
                let module_data = Arc::new(typed.clone());
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                PropagandaCenterBehavior::new(object_id, module_data)
                    .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        // Stealth modules
        registry.register_factory(
            "StealthUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                StealthUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "StealthDetectorUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                StealthDetectorUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "GrantStealthBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                GrantStealthBehaviorFactory::create_behavior(object_id, data)
            }),
        );

        // Base/Building modules
        registry.register_factory(
            "PowerPlantUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                PowerPlantUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "RadarUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                RadarUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "ProductionUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                ProductionUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "BaseRegenerateUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                BaseRegenerateUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "BunkerBusterBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                BunkerBusterBehaviorFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "ParkingPlaceBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                ParkingPlaceBehaviorFactory::create_behavior(object_id, data)
            }),
        );

        // Combat/Weapon behaviors
        registry.register_factory(
            "CountermeasuresBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                CountermeasuresBehaviorFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "FlightDeckBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                FlightDeckBehaviorFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "GenerateMinefieldBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                GenerateMinefieldBehaviorFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "MinefieldBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                MinefieldBehaviorFactory::create_behavior(object_id, data)
            }),
        );

        // Special Ability modules
        registry.register_factory(
            "AutoDepositUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                AutoDepositUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "AutoFindHealingUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                AutoFindHealingUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "HijackerUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                HijackerUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "PilotFindVehicleUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                PilotFindVehicleUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "EMPUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                EMPUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "AssistedTargetingUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                AssistedTargetingUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "FlammableUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                FlammableUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "CleanupHazardUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                CleanupHazardUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "EnemyNearUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                EnemyNearUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "SpecialAbilityUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                SpecialAbilityUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "SpecialPowerUpdateModule",
            Box::new(|thing, data| {
                Ok(
                    crate::object::update::special_power_update::SpecialPowerUpdateModuleFactory
                        .create_module(std::sync::Arc::downgrade(&thing), data),
                )
            }),
        );
        registry.register_factory(
            "PhysicsBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                PhysicsBehaviorFactory::create_behavior(object_id, data)
            }),
        );

        // Horde/Formation modules
        registry.register_factory(
            "HordeUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                HordeUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "MobMemberSlavedUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                MobMemberSlavedUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "TensileFormationUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                TensileFormationUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "BattlePlanUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                BattlePlanUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "AnimationSteeringUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                AnimationSteeringUpdateFactory::create_behavior(object_id, data)
            }),
        );

        // Special Vehicle modules
        registry.register_factory(
            "HelicopterSlowDeathBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                HelicopterSlowDeathBehaviorFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "NeutronMissileSlowDeathUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                NeutronMissileSlowDeathUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "NeutronMissileSlowDeathBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                NeutronMissileSlowDeathUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "NeutronBlastBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                NeutronBlastBehaviorFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "ToppleUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                ToppleUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "StructureCollapseUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                StructureCollapseUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "HeightDieUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                HeightDieUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "LifetimeUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                LifetimeUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "DeletionUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                DeletionUpdateFactory::create_behavior(object_id, data)
            }),
        );

        // Weapon modules
        registry.register_factory(
            "FireOCLAfterWeaponCooldownUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                FireOCLAfterWeaponCooldownUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "WeaponBonusUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                WeaponBonusUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "ProjectileStreamUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                ProjectileStreamUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "PointDefenseLaserUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                PointDefenseLaserUpdateFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "FireWeaponWhenDamagedBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                FireWeaponWhenDamagedBehaviorFactory::create_behavior(object_id, data)
            }),
        );
        registry.register_factory(
            "StickyBombUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                StickyBombUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "LeafletDropBehavior",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                LeafletDropBehaviorFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "DemoTrapUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                DemoTrapUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "FloatUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                FloatUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "CheckpointUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                CheckpointUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "ProneUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                ProneUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "DynamicGeometryInfoUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                DynamicGeometryInfoUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "FirestormDynamicGeometryInfoUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                FirestormDynamicGeometryInfoUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "RadiusDecalUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                RadiusDecalUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "DynamicShroudClearingRangeUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                DynamicShroudClearingRangeUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "SmartBombTargetHomingUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                SmartBombTargetHomingUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "WaveGuideUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                WaveGuideUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "SpectreGunshipUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                SpectreGunshipUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "SpectreGunshipDeploymentUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                SpectreGunshipDeploymentUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "StructureToppleUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                StructureToppleUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "MissileLauncherBuildingUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                MissileLauncherBuildingUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "MissileAIUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                MissileAIUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry.register_factory(
            "ParticleUplinkCannonUpdate",
            Box::new(|thing, data| {
                let object_id = thing
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                ParticleUplinkCannonUpdateFactory::create_behavior(object_id, data)
            }),
        );

        registry
    }

    /// Register a new behavior factory.
    ///
    /// The concrete alias is deliberate: a generic `F: Fn(..)` bound does not
    /// propagate the parameter types into the 60+ registration closures, so
    /// every closure would need manual type annotations. The expected
    /// signature now flows from this parameter type.
    pub fn register_factory(&mut self, name: &str, factory: BehaviorModuleFactoryFn) {
        self.factories.insert(name.to_string(), factory);
    }

    /// Create a behavior module by name
    pub fn create_behavior(
        &self,
        name: &str,
        thing: Arc<RwLock<Object>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<
        Box<dyn crate::modules::BehaviorModuleInterface>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        if let Some(factory) = self.factories.get(name) {
            factory(thing, module_data)
        } else {
            Err(format!("Unknown behavior module: {}", name).into())
        }
    }

    /// Get a list of all registered behavior module names
    pub fn get_registered_behaviors(&self) -> Vec<String> {
        self.factories.keys().cloned().collect()
    }
}

impl Default for BehaviorModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = BehaviorModuleRegistry::new();
        let behaviors = registry.get_registered_behaviors();

        // Verify all major categories are registered
        assert!(behaviors.contains(&"SlowDeathBehavior".to_string()));
        assert!(behaviors.contains(&"SpawnBehavior".to_string()));
        assert!(behaviors.contains(&"StealthUpdate".to_string()));
        assert!(behaviors.contains(&"PowerPlantUpdate".to_string()));
        assert!(behaviors.contains(&"HordeUpdate".to_string()));
        assert!(behaviors.contains(&"HelicopterSlowDeathBehavior".to_string()));
        assert!(behaviors.contains(&"ProjectileStreamUpdate".to_string()));

        // Should have 35+ modules registered
        assert!(
            behaviors.len() >= 35,
            "Expected at least 35 behavior modules, found {}",
            behaviors.len()
        );
    }

    #[test]
    fn test_registry_default() {
        let registry = BehaviorModuleRegistry::default();
        let behaviors = registry.get_registered_behaviors();
        assert!(behaviors.len() >= 35);
    }
}
