//! Module-override factories split by family from the former
//! `contain_module_overrides.rs` dump. Public factory function names are unchanged.

use std::any::Any;
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};

use game_engine::common::ini::INI;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    BaseModuleData, CreateInterface, DeletionLifetimeInterface, Drawable as ModuleDrawableTrait,
    Module, ModuleData, ModuleType, NameKeyType, Object as ModuleObjectTrait, Thing as ModuleThing,
};
use game_engine::common::thing::module_factory::{
    apply_module_overrides_to_existing_templates, register_module_override,
};
use log::warn;

use crate::common::{
    AsciiString, Coord3D, INVALID_ID, ModuleData as LegacyModuleData, ObjectID, TheGameLogic,
};
use crate::modules::{BehaviorModuleInterface, ContainModuleInterface};
use crate::object::behavior::animation_steering_update::{
    AnimationSteeringUpdate, AnimationSteeringUpdateModuleData,
};
use crate::object::behavior::assisted_targeting_update::{
    AssistedTargetingUpdate, AssistedTargetingUpdateModuleData,
};
use crate::object::behavior::auto_deposit_update::{
    AutoDepositUpdate, AutoDepositUpdateModuleData,
};
use crate::object::behavior::auto_find_healing_update::{
    AutoFindHealingUpdate, AutoFindHealingUpdateModuleData,
};
use crate::object::behavior::auto_heal_behavior::{
    AutoHealBehavior, AutoHealBehaviorModule, AutoHealBehaviorModuleData,
};
use crate::object::behavior::base_regenerate_update::{
    BaseRegenerateUpdate, BaseRegenerateUpdateModuleData,
};
use crate::object::behavior::battle_bus_slow_death_behavior::{
    battle_bus_slow_death_data_factory, battle_bus_slow_death_module_factory,
};
use crate::object::behavior::battle_plan_update::{
    BattlePlanUpdate, BattlePlanUpdateModule, BattlePlanUpdateModuleData,
};
use crate::object::behavior::bridge_behavior::{
    BridgeBehavior, BridgeBehaviorModule, BridgeBehaviorModuleData,
};
use crate::object::behavior::bridge_scaffold_behavior::{
    BridgeScaffoldBehavior, BridgeScaffoldBehaviorModule, BridgeScaffoldBehaviorModuleData,
};
use crate::object::behavior::bridge_tower_behavior::{
    BridgeTowerBehavior, BridgeTowerBehaviorModule, BridgeTowerBehaviorModuleData,
};
use crate::object::behavior::bunker_buster_behavior::{
    BunkerBusterBehavior, BunkerBusterBehaviorModuleData,
};
use crate::object::behavior::checkpoint_update::{CheckpointUpdate, CheckpointUpdateModuleData};
use crate::object::behavior::cleanup_hazard_update::{
    CleanupHazardUpdate, CleanupHazardUpdateModule, CleanupHazardUpdateModuleData,
};
use crate::object::behavior::countermeasures_behavior::{
    CountermeasuresBehavior, CountermeasuresBehaviorModule, CountermeasuresBehaviorModuleData,
};
use crate::object::behavior::default_production_exit_behavior::{
    DefaultProductionExitBehavior, DefaultProductionExitBehaviorModule,
    DefaultProductionExitModuleData,
};
use crate::object::behavior::deletion_update::{DeletionUpdate, DeletionUpdateModuleData};
use crate::object::behavior::demo_trap_update::{
    demo_trap_update_data_factory, demo_trap_update_module_factory,
};
use crate::object::behavior::dumb_projectile_behavior::{
    DumbProjectileBehavior, DumbProjectileBehaviorModule, DumbProjectileBehaviorModuleData,
};
use crate::object::behavior::dynamic_geometry_info_update::{
    DynamicGeometryInfoUpdate, DynamicGeometryInfoUpdateModuleData,
};
use crate::object::behavior::dynamic_shroud_clearing_range_update::{
    DynamicShroudClearingRangeUpdate, DynamicShroudClearingRangeUpdateModuleData,
};
use crate::object::behavior::emp_update::{EMPUpdate, EMPUpdateModule, EMPUpdateModuleData};
use crate::object::behavior::enemy_near_update::{EnemyNearUpdate, EnemyNearUpdateModuleData};
use crate::object::behavior::fire_ocl_after_weapon_cooldown_update::{
    FireOCLAfterWeaponCooldownUpdate, FireOCLAfterWeaponCooldownUpdateModuleData,
};
use crate::object::behavior::fire_weapon_update::{FireWeaponUpdate, FireWeaponUpdateModuleData};
use crate::object::behavior::fire_weapon_when_damaged_behavior_new::{
    FireWeaponWhenDamagedBehavior, FireWeaponWhenDamagedBehaviorModuleData,
};
use crate::object::behavior::fire_weapon_when_dead_behavior_new::{
    FireWeaponWhenDeadBehavior, FireWeaponWhenDeadBehaviorModuleData,
};

use crate::object::behavior::firestorm_dynamic_geometry_info_update::{
    FirestormDynamicGeometryInfoUpdate, FirestormDynamicGeometryInfoUpdateModuleData,
};
use crate::object::behavior::flammable_update::{FlammableUpdate, FlammableUpdateModuleData};
use crate::object::behavior::flight_deck_behavior::{
    FlightDeckBehavior, FlightDeckBehaviorModule, FlightDeckBehaviorModuleData,
};
use crate::object::behavior::float_update::{FloatUpdate, FloatUpdateModuleData};
use crate::object::behavior::generate_minefield_behavior::{
    GenerateMinefieldBehavior, GenerateMinefieldBehaviorModuleData,
};
use crate::object::behavior::grant_stealth_behavior::{
    GrantStealthBehavior, GrantStealthBehaviorModule, GrantStealthBehaviorModuleData,
};
use crate::object::behavior::height_die_update::{HeightDieUpdate, HeightDieUpdateModuleData};
use crate::object::behavior::helicopter_slow_death_behavior::{
    HelicopterSlowDeathBehavior, HelicopterSlowDeathBehaviorModule,
    HelicopterSlowDeathBehaviorModuleData,
};
use crate::object::behavior::hijacker_update::{HijackerUpdate, HijackerUpdateModuleData};
use crate::object::behavior::horde_update::{HordeUpdate, HordeUpdateModuleData};
use crate::object::behavior::instant_death_behavior::{
    InstantDeathBehavior, InstantDeathBehaviorModuleData,
};
use crate::object::behavior::jet_slow_death_behavior::{
    JetSlowDeathBehavior, JetSlowDeathBehaviorModule, JetSlowDeathBehaviorModuleData,
};
use crate::object::behavior::leaflet_drop_behavior::{
    LeafletDropBehavior, LeafletDropBehaviorModuleData,
};
use crate::object::behavior::lifetime_update::{
    lifetime_update_data_factory, lifetime_update_module_factory,
};
use crate::object::behavior::minefield_behavior::{
    MinefieldBehavior, MinefieldBehaviorModule, MinefieldBehaviorModuleData,
};
use crate::object::behavior::missile_launcher_building_update::{
    MissileLauncherBuildingUpdate, MissileLauncherBuildingUpdateModule,
    MissileLauncherBuildingUpdateModuleData,
};
use crate::object::behavior::mob_member_slaved_update::{
    MobMemberSlavedUpdate, MobMemberSlavedUpdateModule, MobMemberSlavedUpdateModuleData,
};
use crate::object::behavior::neutron_blast_behavior::{
    NeutronBlastBehavior, NeutronBlastBehaviorModuleData,
};
use crate::object::behavior::neutron_missile_slow_death_update::{
    neutron_missile_slow_death_data_factory, neutron_missile_slow_death_module_factory,
};
use crate::object::behavior::overcharge_behavior::{
    OverchargeBehavior, OverchargeBehaviorModule, OverchargeBehaviorModuleData,
};
use crate::object::behavior::parking_place_behavior::{
    ParkingPlaceBehavior, ParkingPlaceBehaviorModuleData,
};
use crate::object::behavior::particle_uplink_cannon_update::{
    ParticleUplinkCannonUpdate, ParticleUplinkCannonUpdateModule,
    ParticleUplinkCannonUpdateModuleData,
};
use crate::object::behavior::physics_update::{PhysicsBehaviorModuleData, PhysicsBehaviorUpdate};
use crate::object::behavior::pilot_find_vehicle_update::{
    PilotFindVehicleUpdate, PilotFindVehicleUpdateModuleData,
};
use crate::object::behavior::point_defense_laser_update::{
    point_defense_laser_update_data_factory, point_defense_laser_update_module_factory,
};
use crate::object::behavior::poisoned_behavior::{
    PoisonedBehavior, PoisonedBehaviorModule, PoisonedBehaviorModuleData,
};
use crate::object::behavior::power_plant_update::{PowerPlantUpdate, PowerPlantUpdateModuleData};
use crate::object::behavior::prone_update::{
    ProneUpdate, ProneUpdateModule, ProneUpdateModuleData,
};

use crate::object::behavior::projectile_stream_update::{
    projectile_stream_update_data_factory, projectile_stream_update_module_factory,
};
use crate::object::behavior::propaganda_tower_behavior::{
    PropagandaTowerBehavior, PropagandaTowerBehaviorModuleData,
};
use crate::object::behavior::queue_production_exit_behavior::{
    QueueProductionExitBehavior, QueueProductionExitBehaviorModule, QueueProductionExitModuleData,
};
use crate::object::behavior::radar_update::{RadarUpdate, RadarUpdateModuleData};
use crate::object::behavior::radius_decal_update::{
    radius_decal_update_data_factory, radius_decal_update_module_factory,
};
use crate::object::behavior::rebuild_hole_behavior::{
    RebuildHoleBehavior, RebuildHoleBehaviorModule, RebuildHoleBehaviorModuleData,
};
use crate::object::behavior::slow_death_behavior::{
    SlowDeathBehavior, SlowDeathBehaviorModuleData,
};
use crate::object::behavior::smart_bomb_target_homing_update::{
    smart_bomb_target_homing_update_data_factory, smart_bomb_target_homing_update_module_factory,
};
use crate::object::behavior::spawn_behavior::{
    SpawnBehavior, SpawnBehaviorModule, SpawnBehaviorModuleData,
};
use crate::object::behavior::spawn_point_production_exit_behavior::{
    SpawnPointProductionExitBehavior, SpawnPointProductionExitBehaviorModule,
    SpawnPointProductionExitModuleData,
};
use crate::object::behavior::special_ability_update::{
    SpecialAbilityUpdate, SpecialAbilityUpdateModule, SpecialAbilityUpdateModuleData,
};
use crate::object::behavior::spectre_gunship_deployment_update::{
    SpectreGunshipDeploymentUpdate, SpectreGunshipDeploymentUpdateModuleData,
};
use crate::object::behavior::spectre_gunship_update::{
    SpectreGunshipUpdate, SpectreGunshipUpdateModuleData,
};
use crate::object::behavior::stealth_detector_update::{
    StealthDetectorUpdate, StealthDetectorUpdateModuleData,
};
use crate::object::behavior::sticky_bomb_update::{
    sticky_bomb_update_data_factory, sticky_bomb_update_module_factory,
};
use crate::object::behavior::structure_collapse_update::{
    StructureCollapseUpdate, StructureCollapseUpdateModule, StructureCollapseUpdateModuleData,
};
use crate::object::behavior::structure_topple_update::{
    StructureToppleUpdate, StructureToppleUpdateModule, StructureToppleUpdateModuleData,
};
use crate::object::behavior::supply_center_production_exit_behavior::{
    SupplyCenterProductionExitBehavior, SupplyCenterProductionExitBehaviorModule,
    SupplyCenterProductionExitModuleData,
};
use crate::object::behavior::supply_warehouse_crippling_behavior::{
    SupplyWarehouseCripplingBehavior, SupplyWarehouseCripplingBehaviorModuleData,
};
use crate::object::behavior::tech_building_behavior::{
    TechBuildingBehavior, TechBuildingBehaviorModuleData,
};
use crate::object::behavior::tensile_formation_update::{
    tensile_formation_update_data_factory, tensile_formation_update_module_factory,
};
use crate::object::behavior::topple_update::{
    topple_update_data_factory, topple_update_module_factory,
};
use crate::object::behavior::wave_guide_update::{WaveGuideUpdate, WaveGuideUpdateModuleData};
use crate::object::behavior::weapon_bonus_update::{
    WeaponBonusUpdate, WeaponBonusUpdateModuleData,
};
use crate::object::body::active_body::{ActiveBody, ActiveBodyModuleData};
use crate::object::body::body_module::{BodyModuleData, BodyModuleInterface};
use crate::object::body::highlander_body::HighlanderBody;
use crate::object::body::hive_structure_body::{HiveStructureBody, HiveStructureBodyModuleData};
use crate::object::body::immortal_body::ImmortalBody;
use crate::object::body::inactive_body::InactiveBody;
use crate::object::body::structure_body::{StructureBody, StructureBodyModuleData};
use crate::object::body::undead_body::{UndeadBody, UndeadBodyModuleData};
use crate::object::collide::crate_collide::convert_to_car_bomb_crate_collide::{
    ConvertToCarBombCrateCollide, ConvertToCarBombCrateCollideModuleData,
};
use crate::object::collide::crate_collide::convert_to_hijacked_vehicle_crate_collide::{
    ConvertToHijackedVehicleCrateCollide, ConvertToHijackedVehicleCrateCollideModuleData,
};
use crate::object::collide::crate_collide::heal_crate_collide::{
    HealCrateCollide, HealCrateCollideModuleData,
};
use crate::object::collide::crate_collide::money_crate_collide::{
    MoneyCrateCollide, MoneyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::sabotage_command_center_crate_collide::{
    SabotageCommandCenterCrateCollide, SabotageCommandCenterCrateCollideModuleData,
};
use crate::object::collide::crate_collide::sabotage_fake_building_crate_collide::{
    SabotageFakeBuildingCrateCollide, SabotageFakeBuildingCrateCollideModuleData,
};
use crate::object::collide::crate_collide::sabotage_internet_center_crate_collide::{
    SabotageInternetCenterCrateCollide, SabotageInternetCenterCrateCollideModuleData,
};
use crate::object::collide::crate_collide::sabotage_military_factory_crate_collide::{
    SabotageMilitaryFactoryCrateCollide, SabotageMilitaryFactoryCrateCollideModuleData,
};
use crate::object::collide::crate_collide::sabotage_power_plant_crate_collide::{
    SabotagePowerPlantCrateCollide, SabotagePowerPlantCrateCollideModuleData,
};
use crate::object::collide::crate_collide::sabotage_superweapon_crate_collide::{
    SabotageSuperweaponCrateCollide, SabotageSuperweaponCrateCollideModuleData,
};
use crate::object::collide::crate_collide::sabotage_supply_center_crate_collide::{
    SabotageSupplyCenterCrateCollide, SabotageSupplyCenterCrateCollideModuleData,
};
use crate::object::collide::crate_collide::sabotage_supply_dropzone_crate_collide::{
    SabotageSupplyDropzoneCrateCollide, SabotageSupplyDropzoneCrateCollideModuleData,
};
use crate::object::collide::crate_collide::salvage_crate_collide::{
    SalvageCrateCollide, SalvageCrateCollideModuleData,
};
use crate::object::collide::crate_collide::shroud_crate_collide::{
    ShroudCrateCollide, ShroudCrateCollideModuleData,
};
use crate::object::collide::crate_collide::unit_crate_collide::{
    UnitCrateCollide, UnitCrateCollideModuleData,
};
use crate::object::collide::crate_collide::veterancy_crate_collide::{
    VeterancyCrateCollide, VeterancyCrateCollideModuleData,
};
use crate::object::collide::fire_weapon_collide::{FireWeaponCollide, FireWeaponCollideModuleData};
use crate::object::collide::squish_collide::{SquishCollide, SquishCollideModuleData};
use crate::object::collide::{
    COLLISION_MANAGER, CollideModule as CollideModuleTrait, CollisionError,
    Coord3D as CollisionCoord3D, GameObject,
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
use crate::object::create::{
    CreateModuleData, GrantUpgradeCreate, GrantUpgradeCreateModuleData, LockWeaponCreate,
    LockWeaponCreateModuleData, PreorderCreate, SpecialPowerCreate, SupplyCenterCreate,
    SupplyWarehouseCreate, VeterancyGainCreate, VeterancyGainCreateModuleData,
};
use crate::object::damage::DamageModuleData;
use crate::object::damage::bone_fx_damage::{BoneFXDamage, BoneFXDamageModule};
use crate::object::damage::transition_damage_fx::{
    TransitionDamageFX, TransitionDamageFXModule, TransitionDamageFXModuleData,
};
use crate::object::die::{
    CreateCrateDie, CreateCrateDieModuleData, CreateObjectDie, CreateObjectDieModuleData, CrushDie,
    CrushDieModuleData, DamDie, DamDieModuleData, DestroyDie, DieModuleData, DieModuleInterface,
    DieModuleWrapper, EjectPilotDie, EjectPilotDieModuleData, FXListDie, FXListDieModuleData,
    KeepObjectDie, RebuildHoleExposeDie, RebuildHoleExposeDieModuleData, SpecialPowerCompletionDie,
    SpecialPowerCompletionDieModuleData, UpgradeDie, UpgradeDieModuleData,
};
use crate::object::draw::*;
use crate::object::production::production_update_complete::ProductionUpdateCompleteModule;
use crate::object::production::{
    ProductionUpdateModuleData, RailedTransportDockUpdate, RailedTransportDockUpdateData,
    RailedTransportDockUpdateModule, RepairDockUpdate, RepairDockUpdateData,
    RepairDockUpdateModule, SupplyCenterDockUpdate, SupplyCenterDockUpdateData,
    SupplyCenterDockUpdateModule, SupplyWarehouseDockUpdate, SupplyWarehouseDockUpdateData,
    SupplyWarehouseDockUpdateModule,
};
use crate::object::special_powers::*;
use crate::object::update::ai_update::railroad_guide_ai_update::{
    RailroadBehaviorModule, RailroadBehaviorModuleData,
};
use crate::object::update::ai_update::{
    AssaultTransportAIUpdateModule, AssaultTransportAIUpdateModuleData, ChinookAIUpdateModule,
    ChinookAIUpdateModuleData, DeliverPayloadAIUpdateModule, DeliverPayloadAIUpdateModuleData,
    DeployStyleAIUpdateModule, DeployStyleAIUpdateModuleData, DozerAIUpdateModule,
    DozerAIUpdateModuleData, HackInternetAIUpdateModule, HackInternetAIUpdateModuleData,
    JetAIUpdateModule, JetAIUpdateModuleData, RailedTransportAIUpdateModule,
    RailedTransportAIUpdateModuleData, SupplyTruckAIUpdateModule, SupplyTruckAIUpdateModuleData,
    TransportAIUpdateModule, TransportAIUpdateModuleData, WanderAIUpdateModule,
    WanderAIUpdateModuleData, WorkerAIUpdateModule, WorkerAIUpdateModuleData,
};
use crate::object::update::ai_update_interface::{AIUpdateInterfaceModule, AIUpdateModuleData};
use crate::object::update::bone_fx_update::{
    BoneFXUpdate, BoneFXUpdateModule, BoneFXUpdateModuleData,
};
use crate::object::update::command_button_hunt_update::{
    CommandButtonHuntUpdate, CommandButtonHuntUpdateModule, CommandButtonHuntUpdateModuleData,
};
use crate::object::update::fire_spread_update::{
    FireSpreadUpdate, FireSpreadUpdateModule, FireSpreadUpdateModuleData,
};
use crate::object::update::missile_ai_update::{
    MissileAIUpdateBehavior, MissileAIUpdateModuleData,
};
use crate::object::update::neutron_missile_update::{
    neutron_missile_update_data_factory, neutron_missile_update_module_factory,
};
use crate::object::update::ocl_update::{ocl_update_data_factory, ocl_update_module_factory};
use crate::object::update::slaved_update::{
    SlavedUpdate, SlavedUpdateModule, SlavedUpdateModuleData,
};
use crate::object::update::spy_vision_update::{
    SpyVisionUpdate, SpyVisionUpdateModule, SpyVisionUpdateModuleData,
};
use crate::object::update::{
    AnimatedParticleSysBoneClientUpdateModule, BeaconClientUpdateModule,
    BeaconClientUpdateModuleData, LaserUpdateModule as LaserClientUpdateModule,
    LaserUpdateModuleData as LaserClientUpdateModuleData, SwayClientUpdateModule,
};
use crate::object::upgrade::{
    ActiveShroudUpgrade, ActiveShroudUpgradeModuleData, ArmorUpgrade, ArmorUpgradeModuleData,
    CommandSetUpgrade, CommandSetUpgradeModuleData, CostModifierUpgrade,
    CostModifierUpgradeModuleData, ExperienceScalarUpgrade, ExperienceScalarUpgradeModuleData,
    GrantScienceUpgrade, GrantScienceUpgradeModuleData, LocomotorSetUpgrade,
    LocomotorSetUpgradeModuleData, MaxHealthUpgrade, MaxHealthUpgradeModuleData,
    ModelConditionUpgrade, ModelConditionUpgradeModuleData, ObjectCreationUpgrade,
    ObjectCreationUpgradeModuleData, PassengersFireUpgrade, PassengersFireUpgradeModuleData,
    PowerPlantUpgrade, PowerPlantUpgradeModuleData, RadarUpgrade, RadarUpgradeModuleData,
    ReplaceObjectUpgrade, ReplaceObjectUpgradeModuleData, StatusBitsUpgrade,
    StatusBitsUpgradeModuleData, StealthUpgrade, StealthUpgradeModuleData, SubObjectsUpgrade,
    SubObjectsUpgradeModuleData, UnpauseSpecialPowerUpgrade, UnpauseSpecialPowerUpgradeModuleData,
    WeaponBonusUpgrade, WeaponBonusUpgradeModuleData, WeaponSetUpgrade, WeaponSetUpgradeModuleData,
};

use crate::stealth_update::{
    StealthUpdateModule as CoreStealthUpdateModule,
    StealthUpdateModuleData as CoreStealthUpdateModuleData,
};

#[macro_use]
mod helpers;
mod behavior;
mod body;
mod collide_crates;
mod contain;
mod death;
mod draw_client;
mod factory_gaps;
mod install;
mod leftover;
mod production;
mod template_locomotor;
mod update_modules;

pub use contain::{ContainModuleDataAdapter, ContainModuleDataKind};
pub(crate) use helpers::ActiveBehaviorModule;
pub use install::ensure_module_overrides_installed;

/// Concatenated live sources for residual `include_str!` scans.
pub const CONTAIN_OVERRIDES_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("behavior.rs"),
    include_str!("body.rs"),
    include_str!("collide_crates.rs"),
    include_str!("contain.rs"),
    include_str!("death.rs"),
    include_str!("draw_client.rs"),
    include_str!("factory_gaps.rs"),
    include_str!("helpers.rs"),
    include_str!("install.rs"),
    include_str!("leftover.rs"),
    include_str!("production.rs"),
    include_str!("template_locomotor.rs"),
    include_str!("update_modules.rs"),
);
