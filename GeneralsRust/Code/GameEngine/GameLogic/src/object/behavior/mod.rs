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
pub mod fire_weapon_when_dead_behavior_new;
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
pub mod lifetime_update;
pub mod neuton_blast_behavior;
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
pub use fire_weapon_when_dead_behavior_new::{
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
    BunkerBusterBehavior, BunkerBusterBehaviorFactory, BunkerBusterBehaviorModule,
    BunkerBusterBehaviorModuleData,
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
    MinefieldBehavior, MinefieldBehaviorFactory, MinefieldBehaviorModuleData,
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
pub use lifetime_update::{LifetimeUpdate, LifetimeUpdateFactory, LifetimeUpdateModuleData};
pub use neuton_blast_behavior::*;
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
    FirestormDynamicGeometryInfoUpdateModuleData,
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
};
pub use wave_guide_update::{WaveGuideUpdate, WaveGuideUpdateFactory, WaveGuideUpdateModuleData};
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
use rhai::Locked;
use std::sync::Arc;

/// Trait for creating behavior modules from module data
pub trait BehaviorModuleFactory {
    /// Create a new behavior module instance
    fn create_behavior(
        thing: Arc<Locked<Object>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<
        Box<dyn crate::modules::BehaviorModuleInterface>,
        Box<dyn std::error::Error + Send + Sync>,
    >;
}

/// Registry for behavior module factories
pub struct BehaviorModuleRegistry {
    factories: std::collections::HashMap<
        String,
        Box<
            dyn Fn(
                    Arc<Locked<Object>>,
                    Arc<dyn ModuleData>,
                ) -> Result<
                    Box<dyn crate::modules::BehaviorModuleInterface>,
                    Box<dyn std::error::Error + Send + Sync>,
                > + Send
                + Sync,
        >,
    >,
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
            Box::new(|thing, data| {
                SlowDeathBehavior::new(thing, data)
                    .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        registry.register_factory(
            "SpawnBehavior",
            Box::new(|thing, data| {
                SpawnBehavior::new(thing, data)
                    .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        registry.register_factory(
            "FireWeaponWhenDeadBehavior",
            Box::new(|thing, data| FireWeaponWhenDeadBehaviorFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "SupplyWarehouseCripplingBehavior",
            Box::new(|thing, data| {
                SupplyWarehouseCripplingBehavior::new(thing, data)
                    .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        registry.register_factory(
            "TechBuildingBehavior",
            Box::new(|thing, data| {
                TechBuildingBehavior::new(thing, data)
                    .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        registry.register_factory(
            "PropagandaTowerBehavior",
            Box::new(|thing, data| {
                PropagandaTowerBehavior::new(thing, data)
                    .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        #[cfg(feature = "allow_surrender")]
        registry.register_factory(
            "PropagandaCenterBehavior",
            Box::new(|thing, data| {
                let typed = data
                    .as_any()
                    .downcast_ref::<PropagandaCenterBehaviorModuleData>()
                    .ok_or_else(|| "PropagandaCenterBehaviorModuleData expected".into())?;
                let module_data = Arc::new(typed.clone());
                PropagandaCenterBehavior::new(thing, module_data)
                    .map(|b| Box::new(b) as Box<dyn crate::modules::BehaviorModuleInterface>)
            }),
        );

        // Stealth modules
        registry.register_factory(
            "StealthUpdate",
            Box::new(|thing, data| StealthUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "StealthDetectorUpdate",
            Box::new(|thing, data| StealthDetectorUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "GrantStealthBehavior",
            Box::new(|thing, data| GrantStealthBehaviorFactory::create_behavior(thing, data)),
        );

        // Base/Building modules
        registry.register_factory(
            "PowerPlantUpdate",
            Box::new(|thing, data| PowerPlantUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "RadarUpdate",
            Box::new(|thing, data| RadarUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "ProductionUpdate",
            Box::new(|thing, data| ProductionUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "BaseRegenerateUpdate",
            Box::new(|thing, data| BaseRegenerateUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "BunkerBusterBehavior",
            Box::new(|thing, data| BunkerBusterBehaviorFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "ParkingPlaceBehavior",
            Box::new(|thing, data| ParkingPlaceBehaviorFactory::create_behavior(thing, data)),
        );

        // Combat/Weapon behaviors
        registry.register_factory(
            "CountermeasuresBehavior",
            Box::new(|thing, data| CountermeasuresBehaviorFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "FlightDeckBehavior",
            Box::new(|thing, data| FlightDeckBehaviorFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "GenerateMinefieldBehavior",
            Box::new(|thing, data| GenerateMinefieldBehaviorFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "MinefieldBehavior",
            Box::new(|thing, data| MinefieldBehaviorFactory::create_behavior(thing, data)),
        );

        // Special Ability modules
        registry.register_factory(
            "AutoDepositUpdate",
            Box::new(|thing, data| AutoDepositUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "AutoFindHealingUpdate",
            Box::new(|thing, data| AutoFindHealingUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "HijackerUpdate",
            Box::new(|thing, data| HijackerUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "PilotFindVehicleUpdate",
            Box::new(|thing, data| PilotFindVehicleUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "LaserUpdate",
            Box::new(|thing, data| LaserUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "EMPUpdate",
            Box::new(|thing, data| EMPUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "AssistedTargetingUpdate",
            Box::new(|thing, data| AssistedTargetingUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "FlammableUpdate",
            Box::new(|thing, data| FlammableUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "CleanupHazardUpdate",
            Box::new(|thing, data| CleanupHazardUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "EnemyNearUpdate",
            Box::new(|thing, data| EnemyNearUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "SpecialAbilityUpdate",
            Box::new(|thing, data| SpecialAbilityUpdateFactory::create_behavior(thing, data)),
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
            Box::new(|thing, data| PhysicsBehaviorFactory::create_behavior(thing, data)),
        );

        // Horde/Formation modules
        registry.register_factory(
            "HordeUpdate",
            Box::new(|thing, data| HordeUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "MobMemberSlavedUpdate",
            Box::new(|thing, data| MobMemberSlavedUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "TensileFormationUpdate",
            Box::new(|thing, data| TensileFormationUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "BattlePlanUpdate",
            Box::new(|thing, data| BattlePlanUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "AnimationSteeringUpdate",
            Box::new(|thing, data| AnimationSteeringUpdateFactory::create_behavior(thing, data)),
        );

        // Special Vehicle modules
        registry.register_factory(
            "HelicopterSlowDeathBehavior",
            Box::new(|thing, data| {
                HelicopterSlowDeathBehaviorFactory::create_behavior(thing, data)
            }),
        );
        registry.register_factory(
            "NeutronMissileSlowDeathUpdate",
            Box::new(|thing, data| {
                NeutronMissileSlowDeathUpdateFactory::create_behavior(thing, data)
            }),
        );
        registry.register_factory(
            "NeutronMissileSlowDeathBehavior",
            Box::new(|thing, data| {
                NeutronMissileSlowDeathUpdateFactory::create_behavior(thing, data)
            }),
        );
        registry.register_factory(
            "NeutronBlastBehavior",
            Box::new(|thing, data| NeutronBlastBehaviorFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "ToppleUpdate",
            Box::new(|thing, data| ToppleUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "StructureCollapseUpdate",
            Box::new(|thing, data| StructureCollapseUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "HeightDieUpdate",
            Box::new(|thing, data| HeightDieUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "LifetimeUpdate",
            Box::new(|thing, data| LifetimeUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "DeletionUpdate",
            Box::new(|thing, data| DeletionUpdateFactory::create_behavior(thing, data)),
        );

        // Weapon modules
        registry.register_factory(
            "FireOCLAfterWeaponCooldownUpdate",
            Box::new(|thing, data| {
                FireOCLAfterWeaponCooldownUpdateFactory::create_behavior(thing, data)
            }),
        );
        registry.register_factory(
            "WeaponBonusUpdate",
            Box::new(|thing, data| WeaponBonusUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "ProjectileStreamUpdate",
            Box::new(|thing, data| ProjectileStreamUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "PointDefenseLaserUpdate",
            Box::new(|thing, data| PointDefenseLaserUpdateFactory::create_behavior(thing, data)),
        );
        registry.register_factory(
            "FireWeaponWhenDamagedBehavior",
            Box::new(|thing, data| {
                FireWeaponWhenDamagedBehaviorFactory::create_behavior(thing, data)
            }),
        );
        registry.register_factory(
            "StickyBombUpdate",
            Box::new(|thing, data| StickyBombUpdateFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "LeafletDropBehavior",
            Box::new(|thing, data| LeafletDropBehaviorFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "DemoTrapUpdate",
            Box::new(|thing, data| DemoTrapUpdateFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "FloatUpdate",
            Box::new(|thing, data| FloatUpdateFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "CheckpointUpdate",
            Box::new(|thing, data| CheckpointUpdateFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "ProneUpdate",
            Box::new(|thing, data| ProneUpdateFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "DynamicGeometryInfoUpdate",
            Box::new(|thing, data| DynamicGeometryInfoUpdateFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "FirestormDynamicGeometryInfoUpdate",
            Box::new(|thing, data| {
                FirestormDynamicGeometryInfoUpdateFactory::create_behavior(thing, data)
            }),
        );

        registry.register_factory(
            "RadiusDecalUpdate",
            Box::new(|thing, data| RadiusDecalUpdateFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "DynamicShroudClearingRangeUpdate",
            Box::new(|thing, data| {
                DynamicShroudClearingRangeUpdateFactory::create_behavior(thing, data)
            }),
        );

        registry.register_factory(
            "SmartBombTargetHomingUpdate",
            Box::new(|thing, data| {
                SmartBombTargetHomingUpdateFactory::create_behavior(thing, data)
            }),
        );

        registry.register_factory(
            "WaveGuideUpdate",
            Box::new(|thing, data| WaveGuideUpdateFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "SpectreGunshipUpdate",
            Box::new(|thing, data| SpectreGunshipUpdateFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "SpectreGunshipDeploymentUpdate",
            Box::new(|thing, data| {
                SpectreGunshipDeploymentUpdateFactory::create_behavior(thing, data)
            }),
        );

        registry.register_factory(
            "StructureToppleUpdate",
            Box::new(|thing, data| StructureToppleUpdateFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "MissileLauncherBuildingUpdate",
            Box::new(|thing, data| {
                MissileLauncherBuildingUpdateFactory::create_behavior(thing, data)
            }),
        );

        registry.register_factory(
            "MissileAIUpdate",
            Box::new(|thing, data| MissileAIUpdateFactory::create_behavior(thing, data)),
        );

        registry.register_factory(
            "ParticleUplinkCannonUpdate",
            Box::new(|thing, data| ParticleUplinkCannonUpdateFactory::create_behavior(thing, data)),
        );

        registry
    }

    /// Register a new behavior factory
    pub fn register_factory<F>(&mut self, name: &str, factory: F)
    where
        F: Fn(
                Arc<Locked<Object>>,
                Arc<dyn ModuleData>,
            ) -> Result<
                Box<dyn crate::modules::BehaviorModuleInterface>,
                Box<dyn std::error::Error + Send + Sync>,
            > + Send
            + Sync
            + 'static,
    {
        self.factories.insert(name.to_string(), Box::new(factory));
    }

    /// Create a behavior module by name
    pub fn create_behavior(
        &self,
        name: &str,
        thing: Arc<Locked<Object>>,
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

// Thread safety
unsafe impl Send for BehaviorModuleRegistry {}
unsafe impl Sync for BehaviorModuleRegistry {}

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
