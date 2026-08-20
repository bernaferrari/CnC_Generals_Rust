// STALE MODULE: this directory is not exported from GameLogic/src/lib.rs and is
// not part of the active crate build. The active override implementation is
// contain_module_overrides/. Keep this compile fence so accidental reactivation
// fails loudly instead of silently compiling stale parity code.
compile_error!(
    "module_overrides.rs is stale/uncompiled; use contain_module_overrides.rs or intentionally rewire this module"
);

//! Stale ModuleFactory override dump, split by module family for the LOC cap.
//!
//! Live implementation: `contain_module_overrides/`.
//! C++ counterpart: `ModuleFactory.cpp` plus per-module factory wrappers
//! under `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/`.

use std::any::Any;
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};

use game_engine::common::ini::INI;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::AsciiString;
use game_engine::common::system::{object_status_types::ObjectStatusMaskType, Snapshotable, Xfer};
use game_engine::common::thing::module::{
    BaseModuleData, CreateInterface, Drawable as ModuleDrawableTrait, Module, ModuleData,
    ModuleInterfaceType, ModuleType, NameKeyType, Object as ModuleObjectTrait,
    Thing as ModuleThing,
};
use game_engine::common::thing::module_factory::{
    apply_module_overrides_to_existing_templates, get_module_factory, register_module_override,
    NewModuleDataProc,
};

use crate::common::{Coord3D, ObjectID, TheGameLogic, INVALID_ID};
use crate::modules::ContainModuleInterface;
use crate::object::collide::{
    CollideModule as CollideModuleTrait, CollisionError, GameObject, COLLISION_MANAGER,
};
use crate::object::contain::{
    CaveContain, CaveContainModuleData, GarrisonContain, GarrisonContainModuleData, HealContain,
    HealContainModuleData, HelixContain, HelixContainModuleData, InternetHackContain,
    InternetHackContainModuleData, MobNexusContain, MobNexusContainModuleData, OpenContain,
    OpenContainModuleData, OverlordContain, OverlordContainModuleData, ParachuteContain,
    ParachuteContainModuleData, RailedTransportContain, RailedTransportContainModuleData,
    RiderChangeContain, RiderChangeContainModuleData, TransportContain, TransportContainModuleData,
    TunnelContain, TunnelContainModuleData,
};
use crate::object::{
    behavior::assisted_targeting_update::{
        AssistedTargetingUpdate, AssistedTargetingUpdateModule, AssistedTargetingUpdateModuleData,
    },
    behavior::auto_deposit_update::{
        AutoDepositUpdate, AutoDepositUpdateModule, AutoDepositUpdateModuleData,
    },
    behavior::auto_find_healing_update::{
        AutoFindHealingUpdate, AutoFindHealingUpdateModule, AutoFindHealingUpdateModuleData,
    },
    behavior::auto_heal_behavior::{
        AutoHealBehavior, AutoHealBehaviorModule, AutoHealBehaviorModuleData,
    },
    behavior::base_regenerate_update::{
        BaseRegenerateUpdate, BaseRegenerateUpdateModule, BaseRegenerateUpdateModuleData,
    },
    behavior::battle_bus_slow_death_behavior::{
        battle_bus_slow_death_data_factory, battle_bus_slow_death_module_factory,
        BattleBusSlowDeathBehaviorModuleData,
    },
    behavior::battle_plan_update::{
        BattlePlanUpdate, BattlePlanUpdateModule, BattlePlanUpdateModuleData,
    },
    behavior::bridge_behavior::{BridgeBehavior, BridgeBehaviorModule, BridgeBehaviorModuleData},
    behavior::bridge_scaffold_behavior::{
        BridgeScaffoldBehavior, BridgeScaffoldBehaviorModule, BridgeScaffoldBehaviorModuleData,
    },
    behavior::bridge_tower_behavior::{
        BridgeTowerBehavior, BridgeTowerBehaviorModule, BridgeTowerBehaviorModuleData,
    },
    behavior::bunker_buster_behavior::{
        BunkerBusterBehavior, BunkerBusterBehaviorModule, BunkerBusterBehaviorModuleData,
    },
    behavior::cleanup_hazard_update::{
        CleanupHazardUpdate, CleanupHazardUpdateModule, CleanupHazardUpdateModuleData,
    },
    behavior::countermeasures_behavior::{
        CountermeasuresBehavior, CountermeasuresBehaviorModule, CountermeasuresBehaviorModuleData,
    },
    behavior::default_production_exit_behavior::{
        DefaultProductionExitBehavior, DefaultProductionExitBehaviorModule,
        DefaultProductionExitModuleData,
    },
    behavior::demo_trap_update::{DemoTrapUpdate, DemoTrapUpdateModule, DemoTrapUpdateModuleData},
    behavior::dynamic_shroud_clearing_range_update::{
        DynamicShroudClearingRangeUpdate, DynamicShroudClearingRangeUpdateModule,
        DynamicShroudClearingRangeUpdateModuleData,
    },
    behavior::emp_update::{EMPUpdate, EMPUpdateModule, EMPUpdateModuleData},
    behavior::enemy_near_update::{
        EnemyNearUpdate, EnemyNearUpdateModule, EnemyNearUpdateModuleData,
    },
    behavior::fire_ocl_after_weapon_cooldown_update::{
        FireOCLAfterWeaponCooldownUpdate, FireOCLAfterWeaponCooldownUpdateModule,
        FireOCLAfterWeaponCooldownUpdateModuleData,
    },
    behavior::fire_weapon_update::{
        FireWeaponUpdate, FireWeaponUpdateModule, FireWeaponUpdateModuleData,
    },
    behavior::fire_weapon_when_damaged_behavior_new::{
        FireWeaponWhenDamagedBehavior, FireWeaponWhenDamagedBehaviorModule,
        FireWeaponWhenDamagedBehaviorModuleData,
    },
    behavior::fire_weapon_when_dead_behavior_new::{
        FireWeaponWhenDeadBehavior, FireWeaponWhenDeadBehaviorModule,
        FireWeaponWhenDeadBehaviorModuleData,
    },
    behavior::firing_tracker_behavior::{
        FiringTrackerBehavior, FiringTrackerBehaviorModule, FiringTrackerBehaviorModuleData,
    },
    behavior::flight_deck_behavior::{
        FlightDeckBehavior, FlightDeckBehaviorModule, FlightDeckBehaviorModuleData,
    },
    behavior::float_update::{FloatUpdate, FloatUpdateModule, FloatUpdateModuleData},
    behavior::generate_minefield_behavior::{
        GenerateMinefieldBehavior, GenerateMinefieldBehaviorModule,
        GenerateMinefieldBehaviorModuleData,
    },
    behavior::horde_update::{HordeUpdate, HordeUpdateModule, HordeUpdateModuleData},
    behavior::lifetime_update::{LifetimeUpdate, LifetimeUpdateModule, LifetimeUpdateModuleData},
    behavior::missile_launcher_building_update::{
        MissileLauncherBuildingUpdate, MissileLauncherBuildingUpdateModule,
        MissileLauncherBuildingUpdateModuleData,
    },
    behavior::overcharge_behavior::{
        OverchargeBehavior, OverchargeBehaviorModule, OverchargeBehaviorModuleData,
    },
    behavior::parking_place_behavior::{
        ParkingPlaceBehavior, ParkingPlaceBehaviorModule, ParkingPlaceBehaviorModuleData,
    },
    behavior::particle_uplink_cannon_update::{
        ParticleUplinkCannonUpdate, ParticleUplinkCannonUpdateModule,
        ParticleUplinkCannonUpdateModuleData,
    },
    behavior::point_defense_laser_update::{
        PointDefenseLaserUpdate, PointDefenseLaserUpdateModule, PointDefenseLaserUpdateModuleData,
    },
    behavior::power_plant_update::{
        PowerPlantUpdate, PowerPlantUpdateModule, PowerPlantUpdateModuleData,
    },
    behavior::projectile_stream_update::{
        ProjectileStreamUpdate, ProjectileStreamUpdateModule, ProjectileStreamUpdateModuleData,
    },
    behavior::prone_update::{ProneUpdate, ProneUpdateModule, ProneUpdateModuleData},
    behavior::propaganda_tower_behavior::{
        PropagandaTowerBehavior, PropagandaTowerBehaviorModule, PropagandaTowerBehaviorModuleData,
    },
    behavior::queue_production_exit_behavior::{
        QueueProductionExitBehavior, QueueProductionExitBehaviorModule,
        QueueProductionExitModuleData,
    },
    behavior::radar_update::{RadarUpdate, RadarUpdateModule, RadarUpdateModuleData},
    behavior::radius_decal_update::{
        RadiusDecalUpdate, RadiusDecalUpdateModule, RadiusDecalUpdateModuleData,
    },
    behavior::rebuild_hole_behavior::{
        RebuildHoleBehavior, RebuildHoleBehaviorModule, RebuildHoleBehaviorModuleData,
    },
    behavior::smart_bomb_target_homing_update::{
        SmartBombTargetHomingUpdate, SmartBombTargetHomingUpdateModule,
        SmartBombTargetHomingUpdateModuleData,
    },
    behavior::spawn_behavior::{SpawnBehavior, SpawnBehaviorModule, SpawnBehaviorModuleData},
    behavior::spawn_point_production_exit_behavior::{
        SpawnPointProductionExitBehavior, SpawnPointProductionExitBehaviorModule,
        SpawnPointProductionExitModuleData,
    },
    behavior::special_ability_update::{
        SpecialAbilityUpdate, SpecialAbilityUpdateModule, SpecialAbilityUpdateModuleData,
    },
    behavior::spectre_gunship_deployment_update::{
        SpectreGunshipDeploymentUpdate, SpectreGunshipDeploymentUpdateModule,
        SpectreGunshipDeploymentUpdateModuleData,
    },
    behavior::spectre_gunship_update::{
        SpectreGunshipUpdate, SpectreGunshipUpdateModule, SpectreGunshipUpdateModuleData,
    },
    behavior::stealth_detector_update::{
        StealthDetectorUpdate, StealthDetectorUpdateModule, StealthDetectorUpdateModuleData,
    },
    behavior::sticky_bomb_update::{
        StickyBombUpdate, StickyBombUpdateModule, StickyBombUpdateModuleData,
    },
    behavior::structure_collapse_update::{
        StructureCollapseUpdate, StructureCollapseUpdateModule, StructureCollapseUpdateModuleData,
    },
    behavior::structure_topple_update::{
        StructureToppleUpdate, StructureToppleUpdateModule, StructureToppleUpdateModuleData,
    },
    behavior::supply_center_production_exit_behavior::{
        SupplyCenterProductionExitBehavior, SupplyCenterProductionExitBehaviorModule,
        SupplyCenterProductionExitModuleData,
    },
    behavior::supply_warehouse_crippling_behavior::{
        SupplyWarehouseCripplingBehavior, SupplyWarehouseCripplingBehaviorModule,
        SupplyWarehouseCripplingBehaviorModuleData,
    },
    behavior::tech_building_behavior::{
        TechBuildingBehavior, TechBuildingBehaviorModule, TechBuildingBehaviorModuleData,
    },
    behavior::tensile_formation_update::{
        TensileFormationUpdate, TensileFormationUpdateModule, TensileFormationUpdateModuleData,
    },
    behavior::topple_update::{ToppleUpdate, ToppleUpdateModule, ToppleUpdateModuleData},
    behavior::weapon_bonus_update::{
        WeaponBonusUpdate, WeaponBonusUpdateModule, WeaponBonusUpdateModuleData,
    },
    body::{
        active_body::{ActiveBody, ActiveBodyModuleData},
        body_module::{BodyModuleData, BodyModuleInterface},
        highlander_body::HighlanderBody,
        hive_structure_body::{HiveStructureBody, HiveStructureBodyModuleData},
        immortal_body::ImmortalBody,
        inactive_body::InactiveBody,
        structure_body::{StructureBody, StructureBodyModuleData},
        undead_body::{UndeadBody, UndeadBodyModuleData},
    },
    collide::crate_collide::shroud_crate_collide::{
        ShroudCrateCollide, ShroudCrateCollideModuleData,
    },
    collide::fire_weapon_collide::{FireWeaponCollide, FireWeaponCollideModuleData},
    collide::squish_collide::{SquishCollide, SquishCollideModuleData},
    create::{
        CreateModuleData, GrantUpgradeCreate, GrantUpgradeCreateModuleData, LockWeaponCreate,
        LockWeaponCreateModuleData, PreorderCreate, SpecialPowerCreate, SupplyCenterCreate,
        SupplyWarehouseCreate, VeterancyGainCreate, VeterancyGainCreateModuleData,
    },
    damage::transition_damage_fx::{
        TransitionDamageFX, TransitionDamageFXModule, TransitionDamageFXModuleData,
    },
    die::{
        CreateCrateDie, CreateCrateDieModuleData, CreateObjectDie, CreateObjectDieModuleData,
        CrushDie, CrushDieModuleData, DamDie, DamDieModuleData, DestroyDie, DieModuleData,
        DieModuleWrapper, EjectPilotDie, EjectPilotDieModuleData, FXListDie, FXListDieModuleData,
        KeepObjectDie, RebuildHoleExposeDie, RebuildHoleExposeDieModuleData,
        SpecialPowerCompletionDie, SpecialPowerCompletionDieModuleData, UpgradeDie,
        UpgradeDieModuleData,
    },
    draw::w3d_debris_draw::{W3DDebrisDraw, W3DDebrisDrawModuleData},
    draw::w3d_default_draw::{W3DDefaultDraw, W3DDefaultDrawModuleData},
    draw::w3d_dependency_model_draw::{W3DDependencyModelDraw, W3DDependencyModelDrawModuleData},
    draw::w3d_laser_draw::{W3DLaserDraw, W3DLaserDrawModuleData},
    draw::w3d_model_draw::{W3DModelDraw, W3DModelDrawModuleData},
    draw::w3d_overlord_aircraft_draw::{
        W3DOverlordAircraftDraw, W3DOverlordAircraftDrawModuleData,
    },
    draw::w3d_overlord_tank_draw::{W3DOverlordTankDraw, W3DOverlordTankDrawModuleData},
    draw::w3d_overlord_truck_draw::{W3DOverlordTruckDraw, W3DOverlordTruckDrawModuleData},
    draw::w3d_police_car_draw::{W3DPoliceCarDraw, W3DPoliceCarDrawModuleData},
    draw::w3d_projectile_draw::{W3DProjectileDraw, W3DProjectileDrawModuleData},
    draw::w3d_projectile_stream_draw::{
        W3DProjectileStreamDraw, W3DProjectileStreamDrawModuleData,
    },
    draw::w3d_rope_draw::{W3DRopeDraw, W3DRopeDrawModuleData},
    draw::w3d_science_model_draw::{W3DScienceModelDraw, W3DScienceModelDrawModuleData},
    draw::w3d_supply_draw::{W3DSupplyDraw, W3DSupplyDrawModuleData},
    draw::w3d_tank_draw::{W3DTankDraw, W3DTankDrawModuleData},
    draw::w3d_tank_truck_draw::{W3DTankTruckDraw, W3DTankTruckDrawModuleData},
    draw::w3d_tracer_draw::{W3DTracerDraw, W3DTracerDrawModuleData},
    draw::w3d_tree_draw::{W3DTreeDraw, W3DTreeDrawModuleData},
    draw::w3d_truck_draw::{W3DTruckDraw, W3DTruckDrawModuleData},
    production::production_update_complete::{
        ProductionUpdateCompleteModule,
        ProductionUpdateModuleData as ProductionUpdateCompleteModuleData,
    },
    production::{
        PrisonDockUpdate, PrisonDockUpdateData, PrisonDockUpdateModule, RailedTransportDockUpdate,
        RailedTransportDockUpdateData, RailedTransportDockUpdateModule, RepairDockUpdate,
        RepairDockUpdateData, RepairDockUpdateModule, SupplyCenterDockUpdate,
        SupplyCenterDockUpdateData, SupplyCenterDockUpdateModule, SupplyWarehouseDockUpdate,
        SupplyWarehouseDockUpdateData, SupplyWarehouseDockUpdateModule,
    },
    special_power_module::{SpecialPowerModule, SpecialPowerModuleData},
    special_powers::baikonur_launch_power::{BaikonurLaunchPower, BaikonurLaunchPowerModuleData},
    special_powers::cash_bounty_power::{CashBountyPower, CashBountyPowerModuleData},
    special_powers::cash_hack_special_power::{
        CashHackSpecialPower, CashHackSpecialPowerModuleData,
    },
    special_powers::cleanup_area_power::{CleanupAreaPower, CleanupAreaPowerModuleData},
    special_powers::defector_special_power::{
        DefectorSpecialPower, DefectorSpecialPowerModuleData,
    },
    special_powers::demoralize_special_power::{
        DemoralizeSpecialPower, DemoralizeSpecialPowerModuleData,
    },
    special_powers::fire_weapon_power::{FireWeaponPower, FireWeaponPowerModuleData},
    special_powers::ocl_special_power::{OclSpecialPower, OclSpecialPowerModuleData},
    special_powers::special_ability::{SpecialAbility, SpecialAbilityModuleData},
    special_powers::spy_vision_special_power::{
        SpyVisionSpecialPower, SpyVisionSpecialPowerModuleData,
    },
    update::ai_update::railroad_guide_ai_update::{
        RailroadBehaviorModule, RailroadBehaviorModuleData,
    },
    update::ai_update_interface::{AIUpdateInterfaceModule, AIUpdateModuleData},
    update::command_button_hunt_update::{
        CommandButtonHuntUpdate, CommandButtonHuntUpdateModule, CommandButtonHuntUpdateModuleData,
    },
    update::fire_spread_update::{
        FireSpreadUpdate, FireSpreadUpdateModule, FireSpreadUpdateModuleData,
    },
    update::mob_member_slaved_update::{
        MobMemberSlavedUpdate, MobMemberSlavedUpdateModule, MobMemberSlavedUpdateModuleData,
    },
    update::slaved_update::{SlavedUpdate, SlavedUpdateModule, SlavedUpdateModuleData},
    update::{
        bone_fx_update::{BoneFXUpdate, BoneFXUpdateModule, BoneFXUpdateModuleData},
        ocl_update::{OCLUpdateModule, OCLUpdateModuleData},
        special_power_update::{SpecialPowerUpdateModule, SpecialPowerUpdateModuleData},
        spy_vision_update::{SpyVisionUpdate, SpyVisionUpdateModule, SpyVisionUpdateModuleData},
        AnimatedParticleSysBoneClientUpdateModule, AssaultTransportAIUpdateModule,
        AssaultTransportAIUpdateModuleData, BeaconClientUpdateModule, BeaconClientUpdateModuleData,
        ChinookAIUpdateModule, ChinookAIUpdateModuleData, DeliverPayloadAIUpdateModule,
        DeliverPayloadAIUpdateModuleData, DeployStyleAIUpdateModule, DeployStyleAIUpdateModuleData,
        DozerAIUpdateModule, DozerAIUpdateModuleData, HackInternetAIUpdateModule,
        HackInternetAIUpdateModuleData, JetAIUpdateModule, JetAIUpdateModuleData,
        LaserUpdateModule as LaserClientUpdateModule,
        LaserUpdateModuleData as LaserClientUpdateModuleData, RailedTransportAIUpdateModule,
        RailedTransportAIUpdateModuleData, SupplyTruckAIUpdateModule,
        SupplyTruckAIUpdateModuleData, SwayClientUpdateModule, TransportAIUpdateModule,
        TransportAIUpdateModuleData, WanderAIUpdateModule, WanderAIUpdateModuleData,
        WorkerAIUpdateModule, WorkerAIUpdateModuleData,
    },
    upgrade::active_shroud_upgrade::{ActiveShroudUpgrade, ActiveShroudUpgradeModuleData},
    upgrade::armor_upgrade::{ArmorUpgrade, ArmorUpgradeModuleData},
    upgrade::command_set_upgrade::{CommandSetUpgrade, CommandSetUpgradeModuleData},
    upgrade::cost_modifier_upgrade::{CostModifierUpgrade, CostModifierUpgradeModuleData},
    upgrade::experience_scalar_upgrade::{
        ExperienceScalarUpgrade, ExperienceScalarUpgradeModuleData,
    },
    upgrade::grant_science_upgrade::{GrantScienceUpgrade, GrantScienceUpgradeModuleData},
    upgrade::locomotor_set_upgrade::{LocomotorSetUpgrade, LocomotorSetUpgradeModuleData},
    upgrade::max_health_upgrade::{MaxHealthUpgrade, MaxHealthUpgradeModuleData},
    upgrade::model_condition_upgrade::{ModelConditionUpgrade, ModelConditionUpgradeModuleData},
    upgrade::object_creation_upgrade::{ObjectCreationUpgrade, ObjectCreationUpgradeModuleData},
    upgrade::passengers_fire_upgrade::{PassengersFireUpgrade, PassengersFireUpgradeModuleData},
    upgrade::power_plant_upgrade::{PowerPlantUpgrade, PowerPlantUpgradeModuleData},
    upgrade::radar_upgrade::{RadarUpgrade, RadarUpgradeModuleData},
    upgrade::replace_object_upgrade::{ReplaceObjectUpgrade, ReplaceObjectUpgradeModuleData},
    upgrade::status_bits_upgrade::{StatusBitsUpgrade, StatusBitsUpgradeModuleData},
    upgrade::stealth_upgrade::{StealthUpgrade, StealthUpgradeModuleData},
    upgrade::subobjects_upgrade::{SubObjectsUpgrade, SubObjectsUpgradeModuleData},
    upgrade::unpause_special_power_upgrade::{
        UnpauseSpecialPowerUpgrade, UnpauseSpecialPowerUpgradeModuleData,
    },
    upgrade::weapon_bonus_upgrade::{WeaponBonusUpgrade, WeaponBonusUpgradeModuleData},
    upgrade::weapon_set_upgrade::{WeaponSetUpgrade, WeaponSetUpgradeModuleData},
};
use crate::stealth_update::{StealthUpdateModule, StealthUpdateModuleData};
use log::warn;

// Additional missing module imports
use crate::object::behavior::animation_steering_update::{
    AnimationSteeringUpdate, AnimationSteeringUpdateFactory, AnimationSteeringUpdateModuleData,
};
use crate::object::behavior::checkpoint_update::{
    CheckpointUpdate, CheckpointUpdateFactory, CheckpointUpdateModuleData,
};
use crate::object::behavior::deletion_update::{
    DeletionUpdate, DeletionUpdateFactory, DeletionUpdateModuleData,
};
use crate::object::behavior::dynamic_geometry_info_update::{
    DynamicGeometryInfoUpdate, DynamicGeometryInfoUpdateFactory,
    DynamicGeometryInfoUpdateModuleData,
};
use crate::object::behavior::firestorm_dynamic_geometry_info_update::{
    FirestormDynamicGeometryInfoUpdate, FirestormDynamicGeometryInfoUpdateFactory,
    FirestormDynamicGeometryInfoUpdateModuleData,
};
use crate::object::behavior::grant_stealth_behavior::{
    GrantStealthBehavior, GrantStealthBehaviorFactory, GrantStealthBehaviorModuleData,
};
use crate::object::behavior::height_die_update::{
    HeightDieUpdate, HeightDieUpdateFactory, HeightDieUpdateModuleData,
};
use crate::object::behavior::helicopter_slow_death_behavior::{
    HelicopterSlowDeathBehavior, HelicopterSlowDeathBehaviorFactory,
    HelicopterSlowDeathBehaviorModuleData,
};
use crate::object::behavior::hijacker_update::{
    HijackerUpdate, HijackerUpdateFactory, HijackerUpdateModuleData,
};
use crate::object::behavior::minefield_behavior::{
    MinefieldBehavior, MinefieldBehaviorFactory, MinefieldBehaviorModuleData,
};
use crate::object::behavior::neutron_missile_slow_death_update::{
    NeutronMissileSlowDeathUpdate, NeutronMissileSlowDeathUpdateFactory,
    NeutronMissileSlowDeathUpdateModuleData,
};
use crate::object::behavior::physics_update::{
    PhysicsBehaviorFactory, PhysicsBehaviorModuleData, PhysicsBehaviorUpdate,
};
use crate::object::behavior::pilot_find_vehicle_update::{
    PilotFindVehicleUpdate, PilotFindVehicleUpdateFactory, PilotFindVehicleUpdateModuleData,
};
use crate::object::behavior::slow_death_behavior::{
    SlowDeathBehavior, SlowDeathBehaviorModuleData,
};
use crate::object::behavior::wave_guide_update::{
    WaveGuideUpdate, WaveGuideUpdateFactory, WaveGuideUpdateModuleData,
};
use crate::object::update::neutron_missile_update::{
    NeutronMissileUpdate, NeutronMissileUpdateModuleData,
};

#[cfg(feature = "allow_surrender")]
use crate::object::behavior::pow_truck_behavior::{
    POWTruckBehavior, POWTruckBehaviorModule, POWTruckBehaviorModuleData,
};

#[cfg(feature = "allow_surrender")]
use crate::object::behavior::propaganda_center_behavior::{
    PropagandaCenterBehavior, PropagandaCenterBehaviorModule, PropagandaCenterBehaviorModuleData,
};

#[cfg(feature = "allow_surrender")]
use crate::object::behavior::prison_behavior::{
    PrisonBehavior, PrisonBehaviorModule, PrisonBehaviorModuleData,
};

#[cfg(feature = "allow_surrender")]
use crate::pow_truck_ai_update::{POWTruckAIUpdateModule, POWTruckAIUpdateModuleData};


mod helpers;
mod body;
mod create;
mod collide;
mod die;
mod special_power;
mod upgrade;
mod ai;
mod behavior;
mod update;
mod production;
mod draw_client;
mod contain;
mod install;

#[cfg(test)]
mod tests;

pub(crate) use helpers::{ContainBindingModule, ContainModuleDataAdapter};
pub use install::{ensure_module_overrides_installed, install_module_overrides};
