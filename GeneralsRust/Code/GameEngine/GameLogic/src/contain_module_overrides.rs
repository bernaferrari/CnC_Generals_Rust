use std::any::Any;
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};

use game_engine::common::ini::INI;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    BaseModuleData, CreateInterface, Drawable as ModuleDrawableTrait, Module, ModuleData,
    ModuleType, NameKeyType, Object as ModuleObjectTrait, Thing as ModuleThing,
};
use game_engine::common::thing::module_factory::{
    apply_module_overrides_to_existing_templates, register_module_override,
};
use log::warn;

use crate::common::{
    AsciiString, Coord3D, ModuleData as LegacyModuleData, ObjectID, TheGameLogic, INVALID_ID,
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
use crate::object::collide::crate_collide::salvage_crate_collide::{
    SalvageCrateCollide, SalvageCrateCollideModuleData,
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
    CollideModule as CollideModuleTrait, CollisionError, Coord3D as CollisionCoord3D, GameObject,
    COLLISION_MANAGER,
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
use crate::object::damage::bone_fx_damage::{BoneFXDamage, BoneFXDamageModule};
use crate::object::damage::transition_damage_fx::{
    TransitionDamageFX, TransitionDamageFXModule, TransitionDamageFXModuleData,
};
use crate::object::damage::DamageModuleData;
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
    ArmorUpgrade, ArmorUpgradeModuleData, CommandSetUpgrade, CommandSetUpgradeModuleData,
    CostModifierUpgrade, CostModifierUpgradeModuleData, ExperienceScalarUpgrade,
    ExperienceScalarUpgradeModuleData, GrantScienceUpgrade, GrantScienceUpgradeModuleData,
    LocomotorSetUpgrade, LocomotorSetUpgradeModuleData, MaxHealthUpgrade,
    MaxHealthUpgradeModuleData, ModelConditionUpgrade, ModelConditionUpgradeModuleData,
    ObjectCreationUpgrade, ObjectCreationUpgradeModuleData, PassengersFireUpgrade,
    PassengersFireUpgradeModuleData, PowerPlantUpgrade, PowerPlantUpgradeModuleData, RadarUpgrade,
    RadarUpgradeModuleData, ReplaceObjectUpgrade, ReplaceObjectUpgradeModuleData, StealthUpgrade,
    StealthUpgradeModuleData, SubObjectsUpgrade, SubObjectsUpgradeModuleData,
    UnpauseSpecialPowerUpgrade, UnpauseSpecialPowerUpgradeModuleData, WeaponBonusUpgrade,
    WeaponBonusUpgradeModuleData, WeaponSetUpgrade, WeaponSetUpgradeModuleData,
};
use crate::stealth_update::{
    StealthUpdateModule as CoreStealthUpdateModule,
    StealthUpdateModuleData as CoreStealthUpdateModuleData,
};

fn resolve_owner_id(thing: &Arc<dyn ModuleThing>) -> ObjectID {
    thing
        .as_object()
        .map(ModuleObjectTrait::get_object_id)
        .unwrap_or(INVALID_ID)
}

fn resolve_owner_info(thing: &Arc<dyn ModuleThing>) -> (ObjectID, Coord3D) {
    let owner_id = resolve_owner_id(thing);
    let position = TheGameLogic::find_object_by_id(owner_id)
        .and_then(|object| object.read().ok().map(|guard| *guard.get_position()))
        .unwrap_or_default();
    (owner_id, position)
}

fn resolve_drawable_id(thing: &Arc<dyn ModuleThing>) -> u32 {
    thing
        .as_drawable()
        .map(ModuleDrawableTrait::get_drawable_id)
        .unwrap_or(INVALID_ID)
}

fn owner_weak(owner_id: ObjectID) -> Weak<RwLock<crate::object::Object>> {
    TheGameLogic::find_object_by_id(owner_id)
        .map(|arc| Arc::downgrade(&arc))
        .unwrap_or_else(Weak::new)
}

fn attach_contain_to_object(object_id: ObjectID, contain: Arc<Mutex<dyn ContainModuleInterface>>) {
    if let Some(object) = TheGameLogic::find_object_by_id(object_id) {
        if let Ok(mut guard) = object.write() {
            guard.set_contain(Some(contain));
        }
    }
}

fn attach_body_to_object(object_id: ObjectID, body: Arc<Mutex<dyn BodyModuleInterface>>) {
    if let Some(object) = TheGameLogic::find_object_by_id(object_id) {
        if let Ok(mut guard) = object.write() {
            guard.set_body_module(Some(body));
        }
    }
}

#[derive(Debug)]
struct ActiveBehaviorModule<T: BehaviorModuleInterface + Snapshotable + 'static> {
    module_name_key: NameKeyType,
    data: Arc<dyn ModuleData>,
    behavior: T,
}

impl<T: BehaviorModuleInterface + Snapshotable + 'static> ActiveBehaviorModule<T> {
    fn new(module_name: &str, data: Arc<dyn ModuleData>, behavior: T) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            data,
            behavior,
        }
    }
}

impl<T: BehaviorModuleInterface + Snapshotable + 'static> Module for ActiveBehaviorModule<T> {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }
}

impl<T: BehaviorModuleInterface + Snapshotable + 'static> Snapshotable for ActiveBehaviorModule<T> {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

fn active_behavior_module<TBehavior, TData>(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
    module_name: &str,
    create: fn(
        Arc<RwLock<crate::object::Object>>,
        Arc<dyn LegacyModuleData>,
    ) -> Result<TBehavior, Box<dyn std::error::Error + Send + Sync>>,
) -> Box<dyn Module>
where
    TBehavior: BehaviorModuleInterface + Snapshotable + 'static,
    TData: ModuleData + LegacyModuleData + Clone + 'static,
{
    let data_arc = cloned_module_data::<TData>(module_name, &module_data);
    let engine_data: Arc<dyn ModuleData> = data_arc.clone();
    let legacy_data: Arc<dyn LegacyModuleData> = data_arc;
    let owner_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .unwrap_or_else(|| panic!("{module_name} requires a valid object"));
    let behavior = create(object, legacy_data)
        .unwrap_or_else(|err| panic!("{module_name} init failed: {err}"));
    Box::new(ActiveBehaviorModule::new(
        module_name,
        engine_data,
        behavior,
    ))
}

fn cloned_module_data<TData>(module_name: &str, module_data: &Arc<dyn ModuleData>) -> Arc<TData>
where
    TData: ModuleData + Clone + 'static,
{
    Arc::new(
        module_data
            .as_any()
            .downcast_ref::<TData>()
            .unwrap_or_else(|| panic!("{module_name} module data type expected"))
            .clone(),
    )
}

#[derive(Debug)]
struct ActiveCreateModule<T: CreateInterface + Snapshotable + Send + Sync + 'static> {
    module_name_key: NameKeyType,
    data: Arc<dyn ModuleData>,
    create: T,
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> ActiveCreateModule<T> {
    fn new(module_name: &str, data: Arc<dyn ModuleData>, create: T) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            data,
            create,
        }
    }
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> Module for ActiveCreateModule<T> {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn get_create_interface(&self) -> Option<&dyn CreateInterface> {
        Some(&self.create)
    }
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> Snapshotable
    for ActiveCreateModule<T>
{
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.create.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.create.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.create.load_post_process()
    }
}

macro_rules! parsed_create_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        "Failed to parse {} module data at line {}: {}",
                        $module_name,
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc = cloned_module_data::<$data_ty>($module_name, &module_data);
            let engine_data: Arc<dyn ModuleData> = data_arc.clone();
            Box::new(ActiveCreateModule::new(
                $module_name,
                engine_data,
                <$module_ty>::new(thing, data_arc),
            ))
        }
    };
}

macro_rules! empty_create_factories {
    ($data_factory:ident, $module_factory:ident, $module_ty:ty, $module_name:literal) => {
        fn $data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            Box::new(CreateModuleData::default())
        }

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc = cloned_module_data::<CreateModuleData>($module_name, &module_data);
            let engine_data: Arc<dyn ModuleData> = data_arc;
            Box::new(ActiveCreateModule::new(
                $module_name,
                engine_data,
                <$module_ty>::new(thing),
            ))
        }
    };
}

parsed_create_factories!(
    grant_upgrade_create_data_factory,
    grant_upgrade_create_module_factory,
    GrantUpgradeCreateModuleData,
    GrantUpgradeCreate,
    "GrantUpgradeCreate"
);
parsed_create_factories!(
    lock_weapon_create_data_factory,
    lock_weapon_create_module_factory,
    LockWeaponCreateModuleData,
    LockWeaponCreate,
    "LockWeaponCreate"
);
empty_create_factories!(
    preorder_create_data_factory,
    preorder_create_module_factory,
    PreorderCreate,
    "PreorderCreate"
);
empty_create_factories!(
    special_power_create_data_factory,
    special_power_create_module_factory,
    SpecialPowerCreate,
    "SpecialPowerCreate"
);
empty_create_factories!(
    supply_center_create_data_factory,
    supply_center_create_module_factory,
    SupplyCenterCreate,
    "SupplyCenterCreate"
);
empty_create_factories!(
    supply_warehouse_create_data_factory,
    supply_warehouse_create_module_factory,
    SupplyWarehouseCreate,
    "SupplyWarehouseCreate"
);
parsed_create_factories!(
    veterancy_gain_create_data_factory,
    veterancy_gain_create_module_factory,
    VeterancyGainCreateModuleData,
    VeterancyGainCreate,
    "VeterancyGainCreate"
);

macro_rules! active_behavior_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $behavior_ty:ty, $module_name:literal) => {
        fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        "Failed to parse {} module data at line {}: {}",
                        $module_name,
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            active_behavior_module::<$behavior_ty, $data_ty>(
                thing,
                module_data,
                $module_name,
                <$behavior_ty>::new,
            )
        }
    };
}

active_behavior_factories!(
    animation_steering_update_data_factory,
    animation_steering_update_module_factory,
    AnimationSteeringUpdateModuleData,
    AnimationSteeringUpdate,
    "AnimationSteeringUpdate"
);
active_behavior_factories!(
    assisted_targeting_update_data_factory,
    assisted_targeting_update_module_factory,
    AssistedTargetingUpdateModuleData,
    AssistedTargetingUpdate,
    "AssistedTargetingUpdate"
);
active_behavior_factories!(
    auto_deposit_update_data_factory,
    auto_deposit_update_module_factory,
    AutoDepositUpdateModuleData,
    AutoDepositUpdate,
    "AutoDepositUpdate"
);
active_behavior_factories!(
    auto_find_healing_update_data_factory,
    auto_find_healing_update_module_factory,
    AutoFindHealingUpdateModuleData,
    AutoFindHealingUpdate,
    "AutoFindHealingUpdate"
);
active_behavior_factories!(
    base_regenerate_update_data_factory,
    base_regenerate_update_module_factory,
    BaseRegenerateUpdateModuleData,
    BaseRegenerateUpdate,
    "BaseRegenerateUpdate"
);

fn battle_plan_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BattlePlanUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BattlePlanUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn battle_plan_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<BattlePlanUpdateModuleData>("BattlePlanUpdate", &module_data);
    let engine_data: Arc<dyn LegacyModuleData> = data_arc.clone();
    let owner_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("BattlePlanUpdate requires a valid object");
    let behavior =
        BattlePlanUpdate::new(object, engine_data).expect("BattlePlanUpdate failed to initialize");
    Box::new(BattlePlanUpdateModule::new(
        behavior,
        &AsciiString::from("BattlePlanUpdate"),
        data_arc,
    ))
}

fn cleanup_hazard_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CleanupHazardUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CleanupHazardUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn cleanup_hazard_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<CleanupHazardUpdateModuleData>("CleanupHazardUpdate", &module_data);
    let engine_data: Arc<dyn crate::common::ModuleData> = data_arc.clone();
    let owner_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("CleanupHazardUpdate requires a valid object");
    let behavior = CleanupHazardUpdate::new(object, engine_data)
        .expect("CleanupHazardUpdate failed to initialize");
    Box::new(CleanupHazardUpdateModule::new(
        behavior,
        &AsciiString::from("CleanupHazardUpdate"),
        data_arc,
    ))
}

fn command_button_hunt_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CommandButtonHuntUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CommandButtonHuntUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn command_button_hunt_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CommandButtonHuntUpdateModuleData>(
        "CommandButtonHuntUpdate",
        &module_data,
    );
    let owner_id = resolve_owner_id(&thing);
    let behavior = CommandButtonHuntUpdate::new(owner_id, data_arc.clone());
    Box::new(CommandButtonHuntUpdateModule::new(
        behavior,
        &AsciiString::from("CommandButtonHuntUpdate"),
        data_arc,
    ))
}

fn spy_vision_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpyVisionUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpyVisionUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn spy_vision_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SpyVisionUpdateModuleData>("SpyVisionUpdate", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let module_name = AsciiString::from("SpyVisionUpdate");
    let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
    let behavior = SpyVisionUpdate::new(module_name_key, data_arc.clone(), owner_id);
    Box::new(SpyVisionUpdateModule::new(behavior, &module_name, data_arc))
}

fn slaved_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SlavedUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SlavedUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn slaved_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SlavedUpdateModuleData>("SlavedUpdate", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let behavior =
        SlavedUpdate::new(owner_id, data_arc.clone()).expect("SlavedUpdate failed to initialize");
    Box::new(SlavedUpdateModule::new(
        behavior,
        &AsciiString::from("SlavedUpdate"),
        data_arc,
    ))
}

fn mob_member_slaved_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MobMemberSlavedUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MobMemberSlavedUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn mob_member_slaved_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<MobMemberSlavedUpdateModuleData>(
        "MobMemberSlavedUpdate",
        &module_data,
    );
    let owner_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("MobMemberSlavedUpdate requires a valid object");
    let legacy_data: Arc<dyn LegacyModuleData> = data_arc.clone();
    let behavior = MobMemberSlavedUpdate::new(object, legacy_data)
        .expect("MobMemberSlavedUpdate failed to initialize");
    Box::new(MobMemberSlavedUpdateModule::new(
        behavior,
        &AsciiString::from("MobMemberSlavedUpdate"),
        data_arc,
    ))
}

fn fire_spread_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FireSpreadUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireSpreadUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn fire_spread_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<FireSpreadUpdateModuleData>("FireSpreadUpdate", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let behavior = FireSpreadUpdate::new(owner_id, (*data_arc).clone());
    Box::new(FireSpreadUpdateModule::new(
        behavior,
        &AsciiString::from("FireSpreadUpdate"),
        data_arc,
    ))
}

fn rebuild_hole_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RebuildHoleBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RebuildHoleBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn rebuild_hole_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<RebuildHoleBehaviorModuleData>("RebuildHoleBehavior", &module_data);
    let behavior = RebuildHoleBehavior::from_module_thing(thing, data_arc.clone());
    Box::new(RebuildHoleBehaviorModule::new(
        behavior,
        &AsciiString::from("RebuildHoleBehavior"),
        data_arc,
    ))
}

fn overcharge_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = OverchargeBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse OverchargeBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn overcharge_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<OverchargeBehaviorModuleData>("OverchargeBehavior", &module_data);
    let behavior = OverchargeBehavior::from_module_thing(thing, data_arc.clone());
    Box::new(OverchargeBehaviorModule::new(
        behavior,
        &"OverchargeBehavior".to_string(),
        data_arc,
    ))
}

fn auto_heal_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AutoHealBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AutoHealBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn auto_heal_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<AutoHealBehaviorModuleData>("AutoHealBehavior", &module_data);
    let behavior = AutoHealBehavior::from_module_thing(thing, data_arc.clone());
    Box::new(AutoHealBehaviorModule::new(
        behavior,
        &AsciiString::from("AutoHealBehavior"),
        data_arc,
    ))
}

fn countermeasures_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CountermeasuresBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CountermeasuresBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn countermeasures_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CountermeasuresBehaviorModuleData>(
        "CountermeasuresBehavior",
        &module_data,
    );
    let behavior = CountermeasuresBehavior::from_module_thing(thing, data_arc.clone())
        .expect("CountermeasuresBehavior requires a valid object owner");
    Box::new(CountermeasuresBehaviorModule::new(
        behavior,
        &AsciiString::from("CountermeasuresBehavior"),
        data_arc,
    ))
}

fn dumb_projectile_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DumbProjectileBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DumbProjectileBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn dumb_projectile_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<DumbProjectileBehaviorModuleData>(
        "DumbProjectileBehavior",
        &module_data,
    );
    let behavior = DumbProjectileBehavior::from_module_thing(thing, data_arc.clone())
        .expect("DumbProjectileBehavior requires a valid object owner");
    Box::new(DumbProjectileBehaviorModule::new(
        behavior,
        &AsciiString::from("DumbProjectileBehavior"),
        data_arc,
    ))
}

fn bridge_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BridgeBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BridgeBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn bridge_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<BridgeBehaviorModuleData>("BridgeBehavior", &module_data);
    let behavior = BridgeBehavior::from_module_thing(thing, data_arc.clone())
        .expect("BridgeBehavior requires a valid object owner");
    Box::new(BridgeBehaviorModule::new(
        behavior,
        &AsciiString::from("BridgeBehavior"),
        data_arc,
    ))
}

fn bridge_scaffold_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BridgeScaffoldBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BridgeScaffoldBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn bridge_scaffold_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<BridgeScaffoldBehaviorModuleData>(
        "BridgeScaffoldBehavior",
        &module_data,
    );
    let behavior = BridgeScaffoldBehavior::from_module_thing(thing, data_arc.clone())
        .expect("BridgeScaffoldBehavior requires a valid object owner");
    Box::new(BridgeScaffoldBehaviorModule::new(
        behavior,
        &AsciiString::from("BridgeScaffoldBehavior"),
        data_arc,
    ))
}

fn bridge_tower_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BridgeTowerBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BridgeTowerBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn bridge_tower_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<BridgeTowerBehaviorModuleData>("BridgeTowerBehavior", &module_data);
    let behavior = BridgeTowerBehavior::from_module_thing(thing, data_arc.clone())
        .expect("BridgeTowerBehavior requires a valid object owner");
    Box::new(BridgeTowerBehaviorModule::new(
        behavior,
        &AsciiString::from("BridgeTowerBehavior"),
        data_arc,
    ))
}

fn structure_collapse_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StructureCollapseUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StructureCollapseUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn structure_collapse_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let owner = TheGameLogic::find_object_by_id(owner_id)
        .expect("StructureCollapseUpdate requires a valid object owner");
    let data_arc = cloned_module_data::<StructureCollapseUpdateModuleData>(
        "StructureCollapseUpdate",
        &module_data,
    );
    let behavior = StructureCollapseUpdate::new_with_data(owner, data_arc.clone())
        .expect("StructureCollapseUpdate requires a valid object owner");
    Box::new(StructureCollapseUpdateModule::new(
        behavior,
        &AsciiString::from("StructureCollapseUpdate"),
        data_arc,
    ))
}

fn structure_topple_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StructureToppleUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StructureToppleUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn structure_topple_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let owner = TheGameLogic::find_object_by_id(owner_id)
        .expect("StructureToppleUpdate requires a valid object owner");
    let data_arc = cloned_module_data::<StructureToppleUpdateModuleData>(
        "StructureToppleUpdate",
        &module_data,
    );
    let behavior = StructureToppleUpdate::new_with_data(owner, data_arc.clone())
        .expect("StructureToppleUpdate requires a valid object owner");
    Box::new(StructureToppleUpdateModule::new(
        behavior,
        &AsciiString::from("StructureToppleUpdate"),
        data_arc,
    ))
}

fn grant_stealth_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = GrantStealthBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse GrantStealthBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn grant_stealth_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let owner = TheGameLogic::find_object_by_id(owner_id)
        .expect("GrantStealthBehavior requires a valid object owner");
    let data_arc =
        cloned_module_data::<GrantStealthBehaviorModuleData>("GrantStealthBehavior", &module_data);
    let behavior = GrantStealthBehavior::new_with_data(owner, data_arc.clone())
        .expect("GrantStealthBehavior requires a valid object owner");
    Box::new(GrantStealthBehaviorModule::new(
        behavior,
        &AsciiString::from("GrantStealthBehavior"),
        data_arc,
    ))
}

fn stealth_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CoreStealthUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StealthUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn stealth_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CoreStealthUpdateModuleData>("StealthUpdate", &module_data);
    let object_id = resolve_owner_id(&thing);
    let module_name_key = NameKeyGenerator::name_to_key("StealthUpdate");
    Box::new(CoreStealthUpdateModule::new(
        module_name_key,
        data_arc,
        object_id,
    ))
}

fn transition_damage_fx_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = TransitionDamageFXModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse TransitionDamageFX module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn transition_damage_fx_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<TransitionDamageFXModuleData>("TransitionDamageFX", &module_data);
    let behavior = TransitionDamageFX::from_module_thing(thing, data_arc.clone())
        .expect("TransitionDamageFX requires a valid object owner");
    Box::new(TransitionDamageFXModule::new(
        behavior,
        &AsciiString::from("TransitionDamageFX"),
        data_arc,
    ))
}

fn emp_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = EMPUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse EMPUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn emp_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let owner =
        TheGameLogic::find_object_by_id(owner_id).expect("EMPUpdate requires a valid object owner");
    let data_arc = cloned_module_data::<EMPUpdateModuleData>("EMPUpdate", &module_data);
    let behavior =
        EMPUpdate::new_with_data(owner, data_arc.clone()).expect("EMPUpdate failed to initialize");
    Box::new(EMPUpdateModule::new(
        behavior,
        &AsciiString::from("EMPUpdate"),
        data_arc,
    ))
}

fn bone_fx_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BoneFXUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BoneFXUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn bone_fx_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let data_arc = cloned_module_data::<BoneFXUpdateModuleData>("BoneFXUpdate", &module_data);
    let behavior = BoneFXUpdate::new(owner_id, data_arc.clone());
    Box::new(BoneFXUpdateModule::new(
        behavior,
        &AsciiString::from("BoneFXUpdate"),
        data_arc,
    ))
}

fn bone_fx_damage_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(DamageModuleData::default())
}

fn bone_fx_damage_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let data_arc = cloned_module_data::<DamageModuleData>("BoneFXDamage", &module_data);
    let behavior = BoneFXDamage::new(owner_id);
    Box::new(BoneFXDamageModule::new(
        behavior,
        &AsciiString::from("BoneFXDamage"),
        data_arc,
    ))
}

fn spawn_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpawnBehaviorModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpawnBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn spawn_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let owner =
        TheGameLogic::find_object_by_id(owner_id).expect("SpawnBehavior requires a valid object");
    let data_arc = cloned_module_data::<SpawnBehaviorModuleData>("SpawnBehavior", &module_data);
    let behavior = SpawnBehavior::new_with_data(owner, data_arc.clone())
        .expect("SpawnBehavior failed to initialize");
    Box::new(SpawnBehaviorModule::new(
        behavior,
        &AsciiString::from("SpawnBehavior"),
        data_arc,
    ))
}

fn particle_uplink_cannon_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ParticleUplinkCannonUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ParticleUplinkCannonUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn particle_uplink_cannon_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let owner = TheGameLogic::find_object_by_id(owner_id)
        .expect("ParticleUplinkCannonUpdate requires a valid object");
    let data_arc = cloned_module_data::<ParticleUplinkCannonUpdateModuleData>(
        "ParticleUplinkCannonUpdate",
        &module_data,
    );
    let behavior = ParticleUplinkCannonUpdate::new_with_data(owner, data_arc.clone())
        .expect("ParticleUplinkCannonUpdate failed to initialize");
    Box::new(ParticleUplinkCannonUpdateModule::new(
        behavior,
        &AsciiString::from("ParticleUplinkCannonUpdate"),
        data_arc,
    ))
}

fn default_production_exit_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DefaultProductionExitModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DefaultProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn default_production_exit_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<DefaultProductionExitModuleData>(
        "DefaultProductionExitUpdate",
        &module_data,
    );
    let behavior = DefaultProductionExitBehavior::from_module_thing(thing, data_arc.clone())
        .expect("DefaultProductionExitUpdate requires an owning object");
    Box::new(DefaultProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("DefaultProductionExitUpdate"),
        data_arc,
    ))
}

fn queue_production_exit_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = QueueProductionExitModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse QueueProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn queue_production_exit_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<QueueProductionExitModuleData>(
        "QueueProductionExitUpdate",
        &module_data,
    );
    let behavior = QueueProductionExitBehavior::from_module_thing(thing, data_arc.clone())
        .expect("QueueProductionExitUpdate requires an owning object");
    Box::new(QueueProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("QueueProductionExitUpdate"),
        data_arc,
    ))
}

fn spawn_point_production_exit_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpawnPointProductionExitModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpawnPointProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn spawn_point_production_exit_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SpawnPointProductionExitModuleData>(
        "SpawnPointProductionExitUpdate",
        &module_data,
    );
    let behavior = SpawnPointProductionExitBehavior::from_module_thing(thing, data_arc.clone())
        .expect("SpawnPointProductionExitUpdate requires an owning object");
    Box::new(SpawnPointProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("SpawnPointProductionExitUpdate"),
        data_arc,
    ))
}

fn supply_center_production_exit_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SupplyCenterProductionExitModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SupplyCenterProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn supply_center_production_exit_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SupplyCenterProductionExitModuleData>(
        "SupplyCenterProductionExitUpdate",
        &module_data,
    );
    let behavior = SupplyCenterProductionExitBehavior::from_module_thing(thing, data_arc.clone())
        .expect("SupplyCenterProductionExitUpdate requires an owning object");
    Box::new(SupplyCenterProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("SupplyCenterProductionExitUpdate"),
        data_arc,
    ))
}

fn flight_deck_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FlightDeckBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FlightDeckBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn flight_deck_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<FlightDeckBehaviorModuleData>("FlightDeckBehavior", &module_data);
    let behavior = FlightDeckBehavior::from_module_thing(thing, data_arc.clone())
        .expect("FlightDeckBehavior requires an owning object");
    Box::new(FlightDeckBehaviorModule::new(
        behavior,
        &AsciiString::from("FlightDeckBehavior"),
        data_arc,
    ))
}

fn ai_update_interface_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AIUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AIUpdateInterface module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn ai_update_interface_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<AIUpdateModuleData>("AIUpdateInterface", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("AIUpdateInterface");
    Box::new(AIUpdateInterfaceModule::new(module_name_key, data_arc))
}

macro_rules! ai_update_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        "Failed to parse {} module data at line {}: {}",
                        $module_name,
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }

        fn $module_factory(
            _thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc = cloned_module_data::<$data_ty>($module_name, &module_data);
            let module_name_key = NameKeyGenerator::name_to_key($module_name);
            Box::new(<$module_ty>::new(module_name_key, data_arc))
        }
    };
}

macro_rules! empty_ai_update_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        fn $data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            Box::new(<$data_ty>::default())
        }

        fn $module_factory(
            _thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc = cloned_module_data::<$data_ty>($module_name, &module_data);
            let module_name_key = NameKeyGenerator::name_to_key($module_name);
            Box::new(<$module_ty>::new(module_name_key, data_arc))
        }
    };
}

empty_ai_update_factories!(
    transport_ai_update_data_factory,
    transport_ai_update_module_factory,
    TransportAIUpdateModuleData,
    TransportAIUpdateModule,
    "TransportAIUpdate"
);
ai_update_factories!(
    deploy_style_ai_update_data_factory,
    deploy_style_ai_update_module_factory,
    DeployStyleAIUpdateModuleData,
    DeployStyleAIUpdateModule,
    "DeployStyleAIUpdate"
);
empty_ai_update_factories!(
    wander_ai_update_data_factory,
    wander_ai_update_module_factory,
    WanderAIUpdateModuleData,
    WanderAIUpdateModule,
    "WanderAIUpdate"
);
ai_update_factories!(
    jet_ai_update_data_factory,
    jet_ai_update_module_factory,
    JetAIUpdateModuleData,
    JetAIUpdateModule,
    "JetAIUpdate"
);
ai_update_factories!(
    railed_transport_ai_update_data_factory,
    railed_transport_ai_update_module_factory,
    RailedTransportAIUpdateModuleData,
    RailedTransportAIUpdateModule,
    "RailedTransportAIUpdate"
);
ai_update_factories!(
    assault_transport_ai_update_data_factory,
    assault_transport_ai_update_module_factory,
    AssaultTransportAIUpdateModuleData,
    AssaultTransportAIUpdateModule,
    "AssaultTransportAIUpdate"
);
ai_update_factories!(
    deliver_payload_ai_update_data_factory,
    deliver_payload_ai_update_module_factory,
    DeliverPayloadAIUpdateModuleData,
    DeliverPayloadAIUpdateModule,
    "DeliverPayloadAIUpdate"
);
ai_update_factories!(
    hack_internet_ai_update_data_factory,
    hack_internet_ai_update_module_factory,
    HackInternetAIUpdateModuleData,
    HackInternetAIUpdateModule,
    "HackInternetAIUpdate"
);
ai_update_factories!(
    supply_truck_ai_update_data_factory,
    supply_truck_ai_update_module_factory,
    SupplyTruckAIUpdateModuleData,
    SupplyTruckAIUpdateModule,
    "SupplyTruckAIUpdate"
);
ai_update_factories!(
    chinook_ai_update_data_factory,
    chinook_ai_update_module_factory,
    ChinookAIUpdateModuleData,
    ChinookAIUpdateModule,
    "ChinookAIUpdate"
);
ai_update_factories!(
    worker_ai_update_data_factory,
    worker_ai_update_module_factory,
    WorkerAIUpdateModuleData,
    WorkerAIUpdateModule,
    "WorkerAIUpdate"
);
ai_update_factories!(
    dozer_ai_update_data_factory,
    dozer_ai_update_module_factory,
    DozerAIUpdateModuleData,
    DozerAIUpdateModule,
    "DozerAIUpdate"
);

macro_rules! upgrade_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        "Failed to parse {} module data at line {}: {}",
                        $module_name,
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc = cloned_module_data::<$data_ty>($module_name, &module_data);
            let module_name_key = NameKeyGenerator::name_to_key($module_name);
            let owner_id = resolve_owner_id(&thing);
            Box::new(<$module_ty>::new(module_name_key, data_arc, owner_id))
        }
    };
}

macro_rules! empty_upgrade_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        fn $data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            Box::new(<$data_ty>::default())
        }

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc = cloned_module_data::<$data_ty>($module_name, &module_data);
            let module_name_key = NameKeyGenerator::name_to_key($module_name);
            let owner_id = resolve_owner_id(&thing);
            Box::new(<$module_ty>::new(module_name_key, data_arc, owner_id))
        }
    };
}

upgrade_factories!(
    armor_upgrade_data_factory,
    armor_upgrade_module_factory,
    ArmorUpgradeModuleData,
    ArmorUpgrade,
    "ArmorUpgrade"
);
upgrade_factories!(
    command_set_upgrade_data_factory,
    command_set_upgrade_module_factory,
    CommandSetUpgradeModuleData,
    CommandSetUpgrade,
    "CommandSetUpgrade"
);
upgrade_factories!(
    cost_modifier_upgrade_data_factory,
    cost_modifier_upgrade_module_factory,
    CostModifierUpgradeModuleData,
    CostModifierUpgrade,
    "CostModifierUpgrade"
);
upgrade_factories!(
    experience_scalar_upgrade_data_factory,
    experience_scalar_upgrade_module_factory,
    ExperienceScalarUpgradeModuleData,
    ExperienceScalarUpgrade,
    "ExperienceScalarUpgrade"
);
upgrade_factories!(
    grant_science_upgrade_data_factory,
    grant_science_upgrade_module_factory,
    GrantScienceUpgradeModuleData,
    GrantScienceUpgrade,
    "GrantScienceUpgrade"
);
upgrade_factories!(
    locomotor_set_upgrade_data_factory,
    locomotor_set_upgrade_module_factory,
    LocomotorSetUpgradeModuleData,
    LocomotorSetUpgrade,
    "LocomotorSetUpgrade"
);
upgrade_factories!(
    max_health_upgrade_data_factory,
    max_health_upgrade_module_factory,
    MaxHealthUpgradeModuleData,
    MaxHealthUpgrade,
    "MaxHealthUpgrade"
);
upgrade_factories!(
    model_condition_upgrade_data_factory,
    model_condition_upgrade_module_factory,
    ModelConditionUpgradeModuleData,
    ModelConditionUpgrade,
    "ModelConditionUpgrade"
);
upgrade_factories!(
    object_creation_upgrade_data_factory,
    object_creation_upgrade_module_factory,
    ObjectCreationUpgradeModuleData,
    ObjectCreationUpgrade,
    "ObjectCreationUpgrade"
);
upgrade_factories!(
    passengers_fire_upgrade_data_factory,
    passengers_fire_upgrade_module_factory,
    PassengersFireUpgradeModuleData,
    PassengersFireUpgrade,
    "PassengersFireUpgrade"
);
upgrade_factories!(
    power_plant_upgrade_data_factory,
    power_plant_upgrade_module_factory,
    PowerPlantUpgradeModuleData,
    PowerPlantUpgrade,
    "PowerPlantUpgrade"
);
upgrade_factories!(
    radar_upgrade_data_factory,
    radar_upgrade_module_factory,
    RadarUpgradeModuleData,
    RadarUpgrade,
    "RadarUpgrade"
);
upgrade_factories!(
    replace_object_upgrade_data_factory,
    replace_object_upgrade_module_factory,
    ReplaceObjectUpgradeModuleData,
    ReplaceObjectUpgrade,
    "ReplaceObjectUpgrade"
);
empty_upgrade_factories!(
    stealth_upgrade_data_factory,
    stealth_upgrade_module_factory,
    StealthUpgradeModuleData,
    StealthUpgrade,
    "StealthUpgrade"
);
upgrade_factories!(
    subobjects_upgrade_data_factory,
    subobjects_upgrade_module_factory,
    SubObjectsUpgradeModuleData,
    SubObjectsUpgrade,
    "SubObjectsUpgrade"
);
upgrade_factories!(
    unpause_special_power_upgrade_data_factory,
    unpause_special_power_upgrade_module_factory,
    UnpauseSpecialPowerUpgradeModuleData,
    UnpauseSpecialPowerUpgrade,
    "UnpauseSpecialPowerUpgrade"
);
upgrade_factories!(
    weapon_bonus_upgrade_data_factory,
    weapon_bonus_upgrade_module_factory,
    WeaponBonusUpgradeModuleData,
    WeaponBonusUpgrade,
    "WeaponBonusUpgrade"
);
upgrade_factories!(
    weapon_set_upgrade_data_factory,
    weapon_set_upgrade_module_factory,
    WeaponSetUpgradeModuleData,
    WeaponSetUpgrade,
    "WeaponSetUpgrade"
);

#[derive(Clone)]
struct SharedCollideModule<T> {
    inner: Arc<Mutex<T>>,
}

impl<T> SharedCollideModule<T> {
    fn new(inner: Arc<Mutex<T>>) -> Self {
        Self { inner }
    }

    fn lock_inner(&self) -> Result<std::sync::MutexGuard<'_, T>, CollisionError> {
        self.inner.lock().map_err(|_| {
            CollisionError::InvalidObject("SharedCollideModule lock poisoned".to_string())
        })
    }
}

impl<T> CollideModuleTrait for SharedCollideModule<T>
where
    T: CollideModuleTrait + Send + Sync + 'static,
{
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        loc: &CollisionCoord3D,
        normal: &CollisionCoord3D,
    ) -> Result<(), CollisionError> {
        let mut inner = self.lock_inner()?;
        inner.on_collide(other, loc, normal)
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        self.lock_inner()
            .map(|inner| inner.would_like_to_collide_with(other))
            .unwrap_or(false)
    }

    fn is_hijacked_vehicle_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_hijacked_vehicle_crate_collide())
            .unwrap_or(false)
    }

    fn is_sabotage_building_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_sabotage_building_crate_collide())
            .unwrap_or(false)
    }

    fn is_car_bomb_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_car_bomb_crate_collide())
            .unwrap_or(false)
    }

    fn is_railroad(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_railroad())
            .unwrap_or(false)
    }

    fn is_salvage_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_salvage_crate_collide())
            .unwrap_or(false)
    }
}

impl ModuleData for FireWeaponCollideModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        crate::common::LegacyModuleData::set_module_tag_name_key(self, key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        crate::common::LegacyModuleData::get_module_tag_name_key(self)
    }
}

impl ModuleData for SquishCollideModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        crate::common::LegacyModuleData::set_module_tag_name_key(self, key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        crate::common::LegacyModuleData::get_module_tag_name_key(self)
    }
}

#[derive(Debug, Clone)]
struct CrateCollideDataAdapter<T: Clone + Send + Sync + std::fmt::Debug + 'static> {
    base: BaseModuleData,
    data: T,
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> CrateCollideDataAdapter<T> {
    fn new(data: T) -> Self {
        Self {
            base: BaseModuleData::new(),
            data,
        }
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> ModuleData for CrateCollideDataAdapter<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> Snapshotable
    for CrateCollideDataAdapter<T>
{
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

struct LegacyCrateCollideModule<T, TData>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
    TData: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    module_name_key: NameKeyType,
    data: Arc<CrateCollideDataAdapter<TData>>,
    collide: Arc<Mutex<T>>,
    object_id: ObjectID,
}

impl<T, TData> LegacyCrateCollideModule<T, TData>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
    TData: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    fn new(
        module_name: &str,
        data: Arc<CrateCollideDataAdapter<TData>>,
        collide: T,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            data,
            collide: Arc::new(Mutex::new(collide)),
            object_id,
        }
    }
}

impl<T, TData> Module for LegacyCrateCollideModule<T, TData>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
    TData: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        if let Err(err) = COLLISION_MANAGER.register_collide_module(
            self.object_id,
            Box::new(SharedCollideModule::new(Arc::clone(&self.collide))),
        ) {
            warn!(
                "Failed to register crate collide module for object {}: {}",
                self.object_id, err
            );
        }
    }

    fn on_delete(&mut self) {
        let _ = COLLISION_MANAGER.unregister_object(self.object_id);
    }
}

impl<T, TData> Snapshotable for LegacyCrateCollideModule<T, TData>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
    TData: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let collide = self
            .collide
            .lock()
            .map_err(|_| "crate collide lock poisoned".to_string())?;
        collide.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "crate collide lock poisoned".to_string())?;
        collide.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "crate collide lock poisoned".to_string())?;
        collide.load_post_process()
    }
}

macro_rules! legacy_object_crate_collide_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        fn $data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            Box::new(CrateCollideDataAdapter::new(<$data_ty>::default()))
        }

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc =
                cloned_module_data::<CrateCollideDataAdapter<$data_ty>>($module_name, &module_data);
            let object_id = resolve_owner_id(&thing);
            let object = TheGameLogic::find_object_by_id(object_id)
                .unwrap_or_else(|| panic!("{} requires a valid object", $module_name));
            let collide = <$module_ty>::new(object, data_arc.data.clone());
            Box::new(LegacyCrateCollideModule::new(
                $module_name,
                data_arc,
                collide,
                object_id,
            ))
        }
    };
}

macro_rules! object_id_crate_collide_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        fn $data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            Box::new(CrateCollideDataAdapter::new(<$data_ty>::default()))
        }

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc =
                cloned_module_data::<CrateCollideDataAdapter<$data_ty>>($module_name, &module_data);
            let object_id = resolve_owner_id(&thing);
            let collide = <$module_ty>::new(object_id, data_arc.data.clone());
            Box::new(LegacyCrateCollideModule::new(
                $module_name,
                data_arc,
                collide,
                object_id,
            ))
        }
    };
}

legacy_object_crate_collide_factories!(
    convert_to_car_bomb_crate_collide_data_factory,
    convert_to_car_bomb_crate_collide_module_factory,
    ConvertToCarBombCrateCollideModuleData,
    ConvertToCarBombCrateCollide,
    "ConvertToCarBombCrateCollide"
);
legacy_object_crate_collide_factories!(
    convert_to_hijacked_vehicle_crate_collide_data_factory,
    convert_to_hijacked_vehicle_crate_collide_module_factory,
    ConvertToHijackedVehicleCrateCollideModuleData,
    ConvertToHijackedVehicleCrateCollide,
    "ConvertToHijackedVehicleCrateCollide"
);
legacy_object_crate_collide_factories!(
    sabotage_command_center_crate_collide_data_factory,
    sabotage_command_center_crate_collide_module_factory,
    SabotageCommandCenterCrateCollideModuleData,
    SabotageCommandCenterCrateCollide,
    "SabotageCommandCenterCrateCollide"
);
legacy_object_crate_collide_factories!(
    sabotage_fake_building_crate_collide_data_factory,
    sabotage_fake_building_crate_collide_module_factory,
    SabotageFakeBuildingCrateCollideModuleData,
    SabotageFakeBuildingCrateCollide,
    "SabotageFakeBuildingCrateCollide"
);
legacy_object_crate_collide_factories!(
    sabotage_internet_center_crate_collide_data_factory,
    sabotage_internet_center_crate_collide_module_factory,
    SabotageInternetCenterCrateCollideModuleData,
    SabotageInternetCenterCrateCollide,
    "SabotageInternetCenterCrateCollide"
);
legacy_object_crate_collide_factories!(
    sabotage_military_factory_crate_collide_data_factory,
    sabotage_military_factory_crate_collide_module_factory,
    SabotageMilitaryFactoryCrateCollideModuleData,
    SabotageMilitaryFactoryCrateCollide,
    "SabotageMilitaryFactoryCrateCollide"
);
legacy_object_crate_collide_factories!(
    sabotage_power_plant_crate_collide_data_factory,
    sabotage_power_plant_crate_collide_module_factory,
    SabotagePowerPlantCrateCollideModuleData,
    SabotagePowerPlantCrateCollide,
    "SabotagePowerPlantCrateCollide"
);
legacy_object_crate_collide_factories!(
    sabotage_superweapon_crate_collide_data_factory,
    sabotage_superweapon_crate_collide_module_factory,
    SabotageSuperweaponCrateCollideModuleData,
    SabotageSuperweaponCrateCollide,
    "SabotageSuperweaponCrateCollide"
);
legacy_object_crate_collide_factories!(
    sabotage_supply_center_crate_collide_data_factory,
    sabotage_supply_center_crate_collide_module_factory,
    SabotageSupplyCenterCrateCollideModuleData,
    SabotageSupplyCenterCrateCollide,
    "SabotageSupplyCenterCrateCollide"
);
object_id_crate_collide_factories!(
    money_crate_collide_data_factory,
    money_crate_collide_module_factory,
    MoneyCrateCollideModuleData,
    MoneyCrateCollide,
    "MoneyCrateCollide"
);
object_id_crate_collide_factories!(
    salvage_crate_collide_data_factory,
    salvage_crate_collide_module_factory,
    SalvageCrateCollideModuleData,
    SalvageCrateCollide,
    "SalvageCrateCollide"
);
object_id_crate_collide_factories!(
    unit_crate_collide_data_factory,
    unit_crate_collide_module_factory,
    UnitCrateCollideModuleData,
    UnitCrateCollide,
    "UnitCrateCollide"
);
object_id_crate_collide_factories!(
    veterancy_crate_collide_data_factory,
    veterancy_crate_collide_module_factory,
    VeterancyCrateCollideModuleData,
    VeterancyCrateCollide,
    "VeterancyCrateCollide"
);

struct FireWeaponCollideModule {
    module_name_key: NameKeyType,
    data: Arc<FireWeaponCollideModuleData>,
    collide: Arc<Mutex<FireWeaponCollide>>,
    object_id: ObjectID,
}

impl FireWeaponCollideModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<FireWeaponCollideModuleData>,
        collide: FireWeaponCollide,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            collide: Arc::new(Mutex::new(collide)),
            object_id,
        }
    }

    fn register_collide_module(&self) -> Result<(), CollisionError> {
        COLLISION_MANAGER.register_collide_module(
            self.object_id,
            Box::new(SharedCollideModule::new(Arc::clone(&self.collide))),
        )
    }
}

impl Module for FireWeaponCollideModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        if let Err(err) = self.register_collide_module() {
            warn!(
                "Failed to register FireWeaponCollide module for object {}: {}",
                self.object_id, err
            );
        }
    }

    fn on_delete(&mut self) {
        let _ = COLLISION_MANAGER.unregister_object(self.object_id);
    }
}

impl Snapshotable for FireWeaponCollideModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let collide = self
            .collide
            .lock()
            .map_err(|_| "FireWeaponCollide lock poisoned".to_string())?;
        collide.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "FireWeaponCollide lock poisoned".to_string())?;
        collide.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "FireWeaponCollide lock poisoned".to_string())?;
        collide.load_post_process()
    }
}

fn fire_weapon_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FireWeaponCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireWeaponCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn fire_weapon_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<FireWeaponCollideModuleData>("FireWeaponCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let collide = FireWeaponCollide::new(object_id, Arc::clone(&data_arc))
        .expect("FireWeaponCollide requires a valid collide weapon template");
    let module_name_key = NameKeyGenerator::name_to_key("FireWeaponCollide");
    Box::new(FireWeaponCollideModule::new(
        module_name_key,
        data_arc,
        collide,
        object_id,
    ))
}

struct SquishCollideModule {
    module_name_key: NameKeyType,
    data: Arc<SquishCollideModuleData>,
    collide: Arc<Mutex<SquishCollide>>,
    object_id: ObjectID,
}

impl SquishCollideModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<SquishCollideModuleData>,
        collide: SquishCollide,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            collide: Arc::new(Mutex::new(collide)),
            object_id,
        }
    }
}

impl Module for SquishCollideModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        if let Err(err) = COLLISION_MANAGER.register_collide_module(
            self.object_id,
            Box::new(SharedCollideModule::new(Arc::clone(&self.collide))),
        ) {
            warn!(
                "Failed to register SquishCollide module for object {}: {}",
                self.object_id, err
            );
        }
    }

    fn on_delete(&mut self) {
        let _ = COLLISION_MANAGER.unregister_object(self.object_id);
    }
}

impl Snapshotable for SquishCollideModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn squish_collide_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(SquishCollideModuleData::default())
}

fn squish_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SquishCollideModuleData>("SquishCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let collide = SquishCollide::new(object_id, Arc::clone(&data_arc));
    let module_name_key = NameKeyGenerator::name_to_key("SquishCollide");
    Box::new(SquishCollideModule::new(
        module_name_key,
        data_arc,
        collide,
        object_id,
    ))
}

active_behavior_factories!(
    bunker_buster_behavior_data_factory,
    bunker_buster_behavior_module_factory,
    BunkerBusterBehaviorModuleData,
    BunkerBusterBehavior,
    "BunkerBusterBehavior"
);
active_behavior_factories!(
    checkpoint_update_data_factory,
    checkpoint_update_module_factory,
    CheckpointUpdateModuleData,
    CheckpointUpdate,
    "CheckpointUpdate"
);
active_behavior_factories!(
    deletion_update_data_factory,
    deletion_update_module_factory,
    DeletionUpdateModuleData,
    DeletionUpdate,
    "DeletionUpdate"
);
active_behavior_factories!(
    dynamic_shroud_clearing_range_update_data_factory,
    dynamic_shroud_clearing_range_update_module_factory,
    DynamicShroudClearingRangeUpdateModuleData,
    DynamicShroudClearingRangeUpdate,
    "DynamicShroudClearingRangeUpdate"
);
active_behavior_factories!(
    enemy_near_update_data_factory,
    enemy_near_update_module_factory,
    EnemyNearUpdateModuleData,
    EnemyNearUpdate,
    "EnemyNearUpdate"
);
active_behavior_factories!(
    fire_ocl_after_weapon_cooldown_update_data_factory,
    fire_ocl_after_weapon_cooldown_update_module_factory,
    FireOCLAfterWeaponCooldownUpdateModuleData,
    FireOCLAfterWeaponCooldownUpdate,
    "FireOCLAfterWeaponCooldownUpdate"
);
active_behavior_factories!(
    fire_weapon_when_damaged_behavior_data_factory,
    fire_weapon_when_damaged_behavior_module_factory,
    FireWeaponWhenDamagedBehaviorModuleData,
    FireWeaponWhenDamagedBehavior,
    "FireWeaponWhenDamagedBehavior"
);
active_behavior_factories!(
    fire_weapon_when_dead_behavior_data_factory,
    fire_weapon_when_dead_behavior_module_factory,
    FireWeaponWhenDeadBehaviorModuleData,
    FireWeaponWhenDeadBehavior,
    "FireWeaponWhenDeadBehavior"
);
active_behavior_factories!(
    fire_weapon_update_data_factory,
    fire_weapon_update_module_factory,
    FireWeaponUpdateModuleData,
    FireWeaponUpdate,
    "FireWeaponUpdate"
);
active_behavior_factories!(
    firestorm_dynamic_geometry_info_update_data_factory,
    firestorm_dynamic_geometry_info_update_module_factory,
    FirestormDynamicGeometryInfoUpdateModuleData,
    FirestormDynamicGeometryInfoUpdate,
    "FirestormDynamicGeometryInfoUpdate"
);
active_behavior_factories!(
    float_update_data_factory,
    float_update_module_factory,
    FloatUpdateModuleData,
    FloatUpdate,
    "FloatUpdate"
);
active_behavior_factories!(
    generate_minefield_behavior_data_factory,
    generate_minefield_behavior_module_factory,
    GenerateMinefieldBehaviorModuleData,
    GenerateMinefieldBehavior,
    "GenerateMinefieldBehavior"
);
fn minefield_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MinefieldBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MinefieldBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn minefield_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<MinefieldBehaviorModuleData>("MinefieldBehavior", &module_data);
    let object_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(object_id)
        .unwrap_or_else(|| panic!("MinefieldBehavior requires owning object {object_id}"));
    let behavior = MinefieldBehavior::new(object, Arc::clone(&data_arc))
        .expect("MinefieldBehavior failed to initialize");
    let module_name = AsciiString::from("MinefieldBehavior");
    Box::new(MinefieldBehaviorModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

active_behavior_factories!(
    height_die_update_data_factory,
    height_die_update_module_factory,
    HeightDieUpdateModuleData,
    HeightDieUpdate,
    "HeightDieUpdate"
);
active_behavior_factories!(
    hijacker_update_data_factory,
    hijacker_update_module_factory,
    HijackerUpdateModuleData,
    HijackerUpdate,
    "HijackerUpdate"
);
active_behavior_factories!(
    horde_update_data_factory,
    horde_update_module_factory,
    HordeUpdateModuleData,
    HordeUpdate,
    "HordeUpdate"
);
active_behavior_factories!(
    neutron_blast_behavior_data_factory,
    neutron_blast_behavior_module_factory,
    NeutronBlastBehaviorModuleData,
    NeutronBlastBehavior,
    "NeutronBlastBehavior"
);
active_behavior_factories!(
    leaflet_drop_behavior_data_factory,
    leaflet_drop_behavior_module_factory,
    LeafletDropBehaviorModuleData,
    LeafletDropBehavior,
    "LeafletDropBehavior"
);

fn missile_launcher_building_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MissileLauncherBuildingUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MissileLauncherBuildingUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn missile_launcher_building_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<MissileLauncherBuildingUpdateModuleData>(
        "MissileLauncherBuildingUpdate",
        &module_data,
    );
    let engine_data: Arc<dyn LegacyModuleData> = data_arc.clone();
    let owner_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("MissileLauncherBuildingUpdate requires a valid object");
    let behavior = MissileLauncherBuildingUpdate::new(object, engine_data)
        .expect("MissileLauncherBuildingUpdate failed to initialize");
    Box::new(MissileLauncherBuildingUpdateModule::new(
        behavior,
        &AsciiString::from("MissileLauncherBuildingUpdate"),
        data_arc,
    ))
}

active_behavior_factories!(
    parking_place_behavior_data_factory,
    parking_place_behavior_module_factory,
    ParkingPlaceBehaviorModuleData,
    ParkingPlaceBehavior,
    "ParkingPlaceBehavior"
);
active_behavior_factories!(
    pilot_find_vehicle_update_data_factory,
    pilot_find_vehicle_update_module_factory,
    PilotFindVehicleUpdateModuleData,
    PilotFindVehicleUpdate,
    "PilotFindVehicleUpdate"
);
active_behavior_factories!(
    power_plant_update_data_factory,
    power_plant_update_module_factory,
    PowerPlantUpdateModuleData,
    PowerPlantUpdate,
    "PowerPlantUpdate"
);
active_behavior_factories!(
    propaganda_tower_behavior_data_factory,
    propaganda_tower_behavior_module_factory,
    PropagandaTowerBehaviorModuleData,
    PropagandaTowerBehavior,
    "PropagandaTowerBehavior"
);
active_behavior_factories!(
    radar_update_data_factory,
    radar_update_module_factory,
    RadarUpdateModuleData,
    RadarUpdate,
    "RadarUpdate"
);
active_behavior_factories!(
    spectre_gunship_deployment_update_data_factory,
    spectre_gunship_deployment_update_module_factory,
    SpectreGunshipDeploymentUpdateModuleData,
    SpectreGunshipDeploymentUpdate,
    "SpectreGunshipDeploymentUpdate"
);
active_behavior_factories!(
    spectre_gunship_update_data_factory,
    spectre_gunship_update_module_factory,
    SpectreGunshipUpdateModuleData,
    SpectreGunshipUpdate,
    "SpectreGunshipUpdate"
);
active_behavior_factories!(
    stealth_detector_update_data_factory,
    stealth_detector_update_module_factory,
    StealthDetectorUpdateModuleData,
    StealthDetectorUpdate,
    "StealthDetectorUpdate"
);
active_behavior_factories!(
    tech_building_behavior_data_factory,
    tech_building_behavior_module_factory,
    TechBuildingBehaviorModuleData,
    TechBuildingBehavior,
    "TechBuildingBehavior"
);
active_behavior_factories!(
    wave_guide_update_data_factory,
    wave_guide_update_module_factory,
    WaveGuideUpdateModuleData,
    WaveGuideUpdate,
    "WaveGuideUpdate"
);
active_behavior_factories!(
    weapon_bonus_update_data_factory,
    weapon_bonus_update_module_factory,
    WeaponBonusUpdateModuleData,
    WeaponBonusUpdate,
    "WeaponBonusUpdate"
);
active_behavior_factories!(
    physics_behavior_data_factory,
    physics_behavior_module_factory,
    PhysicsBehaviorModuleData,
    PhysicsBehaviorUpdate,
    "PhysicsBehavior"
);
active_behavior_factories!(
    flammable_update_data_factory,
    flammable_update_module_factory,
    FlammableUpdateModuleData,
    FlammableUpdate,
    "FlammableUpdate"
);

fn special_ability_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpecialAbilityUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpecialAbilityUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn special_ability_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<SpecialAbilityUpdateModuleData>("SpecialAbilityUpdate", &module_data);
    let engine_data: Arc<dyn LegacyModuleData> = data_arc.clone();
    let owner_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("SpecialAbilityUpdate requires a valid object");
    let behavior = SpecialAbilityUpdate::new(Arc::downgrade(&object), engine_data);
    let module_name = AsciiString::from("SpecialAbilityUpdate");
    Box::new(SpecialAbilityUpdateModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

impl Snapshotable for MissileAIUpdateBehavior {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn missile_ai_update_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(MissileAIUpdateModuleData::default())
}

fn missile_ai_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    active_behavior_module::<MissileAIUpdateBehavior, MissileAIUpdateModuleData>(
        thing,
        module_data,
        "MissileAIUpdate",
        MissileAIUpdateBehavior::new,
    )
}

fn railroad_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RailroadBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RailroadBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn railroad_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<RailroadBehaviorModuleData>("RailroadBehavior", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("RailroadBehavior requires a valid object");
    let module_name_key = NameKeyGenerator::name_to_key("RailroadBehavior");
    Box::new(
        RailroadBehaviorModule::new(module_name_key, data_arc, object)
            .expect("RailroadBehavior init failed"),
    )
}

fn production_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ProductionUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ProductionUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn production_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<ProductionUpdateModuleData>("ProductionUpdate", &module_data);
    let module_name = AsciiString::from("ProductionUpdate");
    let owner_id = resolve_owner_id(&thing);
    Box::new(ProductionUpdateCompleteModule::new(
        &module_name,
        data_arc,
        owner_id,
    ))
}

fn repair_dock_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RepairDockUpdateData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RepairDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn repair_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<RepairDockUpdateData>("RepairDockUpdate", &module_data);
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = RepairDockUpdate::new((*data_arc).clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("RepairDockUpdate");
    Box::new(RepairDockUpdateModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

fn railed_transport_dock_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RailedTransportDockUpdateData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RailedTransportDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn railed_transport_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<RailedTransportDockUpdateData>(
        "RailedTransportDockUpdate",
        &module_data,
    );
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = RailedTransportDockUpdate::new((*data_arc).clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("RailedTransportDockUpdate");
    Box::new(RailedTransportDockUpdateModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

fn supply_center_dock_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SupplyCenterDockUpdateData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SupplyCenterDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn supply_center_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<SupplyCenterDockUpdateData>("SupplyCenterDockUpdate", &module_data);
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = SupplyCenterDockUpdate::new((*data_arc).clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("SupplyCenterDockUpdate");
    Box::new(SupplyCenterDockUpdateModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

fn supply_warehouse_dock_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SupplyWarehouseDockUpdateData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SupplyWarehouseDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn supply_warehouse_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SupplyWarehouseDockUpdateData>(
        "SupplyWarehouseDockUpdate",
        &module_data,
    );
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = SupplyWarehouseDockUpdate::new((*data_arc).clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("SupplyWarehouseDockUpdate");
    Box::new(SupplyWarehouseDockUpdateModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

active_behavior_factories!(
    supply_warehouse_crippling_behavior_data_factory,
    supply_warehouse_crippling_behavior_module_factory,
    SupplyWarehouseCripplingBehaviorModuleData,
    SupplyWarehouseCripplingBehavior,
    "SupplyWarehouseCripplingBehavior"
);

#[derive(Debug, Clone)]
pub struct ContainModuleDataAdapter<T: Clone + Send + Sync + std::fmt::Debug + 'static> {
    base: BaseModuleData,
    contain: T,
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> ContainModuleDataAdapter<T> {
    fn new(contain: T) -> Self {
        Self {
            base: BaseModuleData::new(),
            contain,
        }
    }

    pub fn contain_data(&self) -> &T {
        &self.contain
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> Snapshotable
    for ContainModuleDataAdapter<T>
{
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> ModuleData
    for ContainModuleDataAdapter<T>
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
}

/// Closed set of contain module data variants used by this port.
///
/// C++ has a finite list of contain module classes; representing that list as an enum keeps
/// call sites typed and avoids scattered `as_any().downcast_*` logic.
pub enum ContainModuleDataKind<'a> {
    Open(&'a OpenContainModuleData),
    Transport(&'a TransportContainModuleData),
    Garrison(&'a GarrisonContainModuleData),
    Tunnel(&'a TunnelContainModuleData),
    Overlord(&'a OverlordContainModuleData),
    Helix(&'a HelixContainModuleData),
    RailedTransport(&'a RailedTransportContainModuleData),
    RiderChange(&'a RiderChangeContainModuleData),
    InternetHack(&'a InternetHackContainModuleData),
    Heal(&'a HealContainModuleData),
    Cave(&'a CaveContainModuleData),
    Parachute(&'a ParachuteContainModuleData),
    MobNexus(&'a MobNexusContainModuleData),
}

impl<'a> ContainModuleDataKind<'a> {
    pub fn from_module_data(module_data: &'a dyn ModuleData) -> Option<Self> {
        // Prefer direct concrete module data first, then adapter-backed module data.
        if let Some(data) = module_data.as_any().downcast_ref::<OpenContainModuleData>() {
            return Some(Self::Open(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<TransportContainModuleData>()
        {
            return Some(Self::Transport(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<GarrisonContainModuleData>()
        {
            return Some(Self::Garrison(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<TunnelContainModuleData>()
        {
            return Some(Self::Tunnel(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<OverlordContainModuleData>()
        {
            return Some(Self::Overlord(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<HelixContainModuleData>()
        {
            return Some(Self::Helix(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<RailedTransportContainModuleData>()
        {
            return Some(Self::RailedTransport(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<RiderChangeContainModuleData>()
        {
            return Some(Self::RiderChange(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<InternetHackContainModuleData>()
        {
            return Some(Self::InternetHack(data));
        }
        if let Some(data) = module_data.as_any().downcast_ref::<HealContainModuleData>() {
            return Some(Self::Heal(data));
        }
        if let Some(data) = module_data.as_any().downcast_ref::<CaveContainModuleData>() {
            return Some(Self::Cave(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<ParachuteContainModuleData>()
        {
            return Some(Self::Parachute(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<MobNexusContainModuleData>()
        {
            return Some(Self::MobNexus(data));
        }

        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<OpenContainModuleData>>()
        {
            return Some(Self::Open(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<TransportContainModuleData>>()
        {
            return Some(Self::Transport(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<GarrisonContainModuleData>>()
        {
            return Some(Self::Garrison(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<TunnelContainModuleData>>()
        {
            return Some(Self::Tunnel(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<OverlordContainModuleData>>()
        {
            return Some(Self::Overlord(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<HelixContainModuleData>>()
        {
            return Some(Self::Helix(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<RailedTransportContainModuleData>>()
        {
            return Some(Self::RailedTransport(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<RiderChangeContainModuleData>>()
        {
            return Some(Self::RiderChange(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<InternetHackContainModuleData>>()
        {
            return Some(Self::InternetHack(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<HealContainModuleData>>()
        {
            return Some(Self::Heal(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<CaveContainModuleData>>()
        {
            return Some(Self::Cave(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<ParachuteContainModuleData>>()
        {
            return Some(Self::Parachute(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<MobNexusContainModuleData>>()
        {
            return Some(Self::MobNexus(adapter.contain_data()));
        }

        None
    }
}

struct BodyBindingModule<T>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
{
    module_name_key: NameKeyType,
    owner_id: ObjectID,
    data: Arc<T>,
    create_body: fn(T, ObjectID) -> Arc<Mutex<dyn BodyModuleInterface>>,
}

impl<T> BodyBindingModule<T>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
{
    fn new(
        module_name: &str,
        owner_id: ObjectID,
        data: Arc<T>,
        create_body: fn(T, ObjectID) -> Arc<Mutex<dyn BodyModuleInterface>>,
    ) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            owner_id,
            data,
            create_body,
        }
    }
}

impl<T> Module for BodyBindingModule<T>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
{
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        let body = (self.create_body)((*self.data).clone(), self.owner_id);
        attach_body_to_object(self.owner_id, body);
    }
}

impl<T> Snapshotable for BodyBindingModule<T>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
{
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug)]
struct ContainBindingModule {
    module_name_key: NameKeyType,
    module_data: Arc<dyn ModuleData>,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
    owner_id: ObjectID,
}

impl ContainBindingModule {
    fn new(
        module_name_key: NameKeyType,
        module_data: Arc<dyn ModuleData>,
        contain: Arc<Mutex<dyn ContainModuleInterface>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            module_data,
            contain,
            owner_id,
        }
    }
}

impl Module for ContainBindingModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {
        attach_contain_to_object(self.owner_id, Arc::clone(&self.contain));
        if let Ok(mut contain_guard) = self.contain.lock() {
            if let Err(err) = contain_guard.on_owner_created() {
                warn!(
                    "Contain module on_owner_created failed for object {}: {}",
                    self.owner_id, err
                );
            }
        }
    }
}

impl Snapshotable for ContainBindingModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn build_contain_module(
    module_name: &str,
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
) -> Box<dyn Module> {
    let module_name_key = NameKeyGenerator::name_to_key(module_name);
    let owner_id = resolve_owner_id(&thing);
    Box::new(ContainBindingModule::new(
        module_name_key,
        module_data,
        contain,
        owner_id,
    ))
}

fn inactive_body_instance(
    data: BodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(InactiveBody::new_with_owner(data, owner_id)))
}

fn active_body_instance(
    data: ActiveBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(ActiveBody::new_with_owner(data, owner_id)))
}

fn structure_body_instance(
    data: StructureBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(StructureBody::new(data, owner_id)))
}

fn highlander_body_instance(
    data: ActiveBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(HighlanderBody::new(data, owner_id)))
}

fn immortal_body_instance(
    data: ActiveBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(ImmortalBody::new(data, owner_id)))
}

fn hive_structure_body_instance(
    data: HiveStructureBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(HiveStructureBody::new(data, owner_id)))
}

fn undead_body_instance(
    data: UndeadBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(UndeadBody::new(data, owner_id)))
}

fn parse_active_body_data(ini: &mut INI, data: &mut ActiveBodyModuleData) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_structure_body_data(
    ini: &mut INI,
    data: &mut StructureBodyModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_hive_structure_body_data(
    ini: &mut INI,
    data: &mut HiveStructureBodyModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_undead_body_data(ini: &mut INI, data: &mut UndeadBodyModuleData) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_slow_death_behavior_data(
    ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_instant_death_behavior_data(
    ini: &mut INI,
    data: &mut InstantDeathBehaviorModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

macro_rules! body_factories {
    (
        $data_factory:ident,
        $module_factory:ident,
        $data_ty:ty,
        $module_name:literal,
        $body_ctor:expr,
        $parse_data:expr
    ) => {
        fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Some(parse_data) = $parse_data {
                    if let Err(err) = parse_data(ini, &mut data) {
                        warn!("Failed to parse {} module data: {}", $module_name, err);
                    }
                }
            }
            Box::new(data)
        }

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let typed_data = module_data
                .as_ref()
                .as_any()
                .downcast_ref::<$data_ty>()
                .cloned()
                .unwrap_or_else(|| {
                    warn!(concat!(
                        $module_name,
                        " module data expected; using defaults"
                    ));
                    <$data_ty>::default()
                });
            Box::new(BodyBindingModule::new(
                $module_name,
                resolve_owner_id(&thing),
                Arc::new(typed_data),
                $body_ctor,
            ))
        }
    };
}

body_factories!(
    inactive_body_module_data_factory,
    inactive_body_module_factory,
    BodyModuleData,
    "InactiveBody",
    inactive_body_instance,
    None::<fn(&mut INI, &mut BodyModuleData) -> Result<(), String>>
);
body_factories!(
    active_body_module_data_factory,
    active_body_module_factory,
    ActiveBodyModuleData,
    "ActiveBody",
    active_body_instance,
    Some(parse_active_body_data)
);
body_factories!(
    structure_body_module_data_factory,
    structure_body_module_factory,
    StructureBodyModuleData,
    "StructureBody",
    structure_body_instance,
    Some(parse_structure_body_data)
);
body_factories!(
    highlander_body_module_data_factory,
    highlander_body_module_factory,
    ActiveBodyModuleData,
    "HighlanderBody",
    highlander_body_instance,
    Some(parse_active_body_data)
);
body_factories!(
    immortal_body_module_data_factory,
    immortal_body_module_factory,
    ActiveBodyModuleData,
    "ImmortalBody",
    immortal_body_instance,
    Some(parse_active_body_data)
);
body_factories!(
    hive_structure_body_module_data_factory,
    hive_structure_body_module_factory,
    HiveStructureBodyModuleData,
    "HiveStructureBody",
    hive_structure_body_instance,
    Some(parse_hive_structure_body_data)
);
body_factories!(
    undead_body_module_data_factory,
    undead_body_module_factory,
    UndeadBodyModuleData,
    "UndeadBody",
    undead_body_instance,
    Some(parse_undead_body_data)
);

fn parse_die_data(ini: &mut INI, data: &mut DieModuleData) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_upgrade_die_data(ini: &mut INI, data: &mut UpgradeDieModuleData) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_create_object_die_data(
    ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_create_crate_die_data(
    ini: &mut INI,
    data: &mut CreateCrateDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_fx_list_die_data(ini: &mut INI, data: &mut FXListDieModuleData) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_crush_die_data(ini: &mut INI, data: &mut CrushDieModuleData) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_eject_pilot_die_data(
    ini: &mut INI,
    data: &mut EjectPilotDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_rebuild_hole_expose_die_data(
    ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_special_power_completion_die_data(
    ini: &mut INI,
    data: &mut SpecialPowerCompletionDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn parse_dam_die_data(ini: &mut INI, data: &mut DamDieModuleData) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

fn build_die_module<T>(
    module_name: &str,
    thing: Arc<dyn ModuleThing>,
    data: T,
    create_die: fn(Arc<RwLock<crate::object::Object>>, Arc<T>) -> Box<dyn DieModuleInterface>,
) -> Box<dyn Module>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
{
    let object_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(object_id).unwrap_or_else(|| {
        panic!("{module_name} requires owning object {object_id}");
    });
    let typed_data = Arc::new(data);
    let module_data: Arc<dyn ModuleData> = typed_data.clone();
    let die_module = create_die(Arc::clone(&object), typed_data);
    Box::new(DieModuleWrapper::new(
        &AsciiString::from(module_name),
        module_data,
        object,
        die_module,
    ))
}

macro_rules! die_factories {
    (
        $data_factory:ident,
        $module_factory:ident,
        $data_ty:ty,
        $module_name:literal,
        $die_ty:ty,
        $parse_data:expr
    ) => {
        fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = $parse_data(ini, &mut data) {
                    warn!("Failed to parse {} module data: {}", $module_name, err);
                }
            }
            Box::new(data)
        }

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let typed_data = module_data
                .as_ref()
                .as_any()
                .downcast_ref::<$data_ty>()
                .cloned()
                .unwrap_or_else(|| {
                    warn!(concat!(
                        $module_name,
                        " module data expected; using defaults"
                    ));
                    <$data_ty>::default()
                });
            build_die_module($module_name, thing, typed_data, |object, data| {
                Box::new(<$die_ty>::new(object, data))
            })
        }
    };
}

die_factories!(
    destroy_die_module_data_factory,
    destroy_die_module_factory,
    DieModuleData,
    "DestroyDie",
    DestroyDie,
    parse_die_data
);
die_factories!(
    keep_object_die_module_data_factory,
    keep_object_die_module_factory,
    DieModuleData,
    "KeepObjectDie",
    KeepObjectDie,
    parse_die_data
);
die_factories!(
    upgrade_die_module_data_factory,
    upgrade_die_module_factory,
    UpgradeDieModuleData,
    "UpgradeDie",
    UpgradeDie,
    parse_upgrade_die_data
);
die_factories!(
    create_object_die_module_data_factory,
    create_object_die_module_factory,
    CreateObjectDieModuleData,
    "CreateObjectDie",
    CreateObjectDie,
    parse_create_object_die_data
);
die_factories!(
    create_crate_die_module_data_factory,
    create_crate_die_module_factory,
    CreateCrateDieModuleData,
    "CreateCrateDie",
    CreateCrateDie,
    parse_create_crate_die_data
);
die_factories!(
    fx_list_die_module_data_factory,
    fx_list_die_module_factory,
    FXListDieModuleData,
    "FXListDie",
    FXListDie,
    parse_fx_list_die_data
);
die_factories!(
    crush_die_module_data_factory,
    crush_die_module_factory,
    CrushDieModuleData,
    "CrushDie",
    CrushDie,
    parse_crush_die_data
);
die_factories!(
    eject_pilot_die_module_data_factory,
    eject_pilot_die_module_factory,
    EjectPilotDieModuleData,
    "EjectPilotDie",
    EjectPilotDie,
    parse_eject_pilot_die_data
);
die_factories!(
    rebuild_hole_expose_die_module_data_factory,
    rebuild_hole_expose_die_module_factory,
    RebuildHoleExposeDieModuleData,
    "RebuildHoleExposeDie",
    RebuildHoleExposeDie,
    parse_rebuild_hole_expose_die_data
);
die_factories!(
    special_power_completion_die_module_data_factory,
    special_power_completion_die_module_factory,
    SpecialPowerCompletionDieModuleData,
    "SpecialPowerCompletionDie",
    SpecialPowerCompletionDie,
    parse_special_power_completion_die_data
);
die_factories!(
    dam_die_module_data_factory,
    dam_die_module_factory,
    DamDieModuleData,
    "DamDie",
    DamDie,
    parse_dam_die_data
);
die_factories!(
    instant_death_behavior_module_data_factory,
    instant_death_behavior_module_factory,
    InstantDeathBehaviorModuleData,
    "InstantDeathBehavior",
    InstantDeathBehavior,
    parse_instant_death_behavior_data
);

fn slow_death_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SlowDeathBehaviorModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = parse_slow_death_behavior_data(ini, &mut data) {
            warn!("Failed to parse SlowDeathBehavior module data: {}", err);
        }
    }
    Box::new(data)
}

fn slow_death_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<SlowDeathBehaviorModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("SlowDeathBehavior module data expected; using defaults");
            SlowDeathBehaviorModuleData::new()
        });
    let object_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(object_id).unwrap_or_else(|| {
        panic!("SlowDeathBehavior requires owning object {object_id}");
    });
    let data: Arc<dyn crate::common::ModuleData> = Arc::new(typed_data);
    Box::new(
        SlowDeathBehavior::new(object, data)
            .expect("SlowDeathBehavior failed to initialize from module data"),
    )
}

fn helicopter_slow_death_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(HelicopterSlowDeathBehaviorModuleData::default())
}

fn helicopter_slow_death_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<HelicopterSlowDeathBehaviorModuleData>(
        "HelicopterSlowDeathBehavior",
        &module_data,
    );
    let object_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(object_id).unwrap_or_else(|| {
        panic!("HelicopterSlowDeathBehavior requires owning object {object_id}")
    });
    let behavior = HelicopterSlowDeathBehavior::new(object, Arc::clone(&data_arc));
    let module_name = AsciiString::from("HelicopterSlowDeathBehavior");
    Box::new(HelicopterSlowDeathBehaviorModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

fn poisoned_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PoisonedBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PoisonedBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn poisoned_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<PoisonedBehaviorModuleData>("PoisonedBehavior", &module_data);
    let object_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(object_id)
        .unwrap_or_else(|| panic!("PoisonedBehavior requires owning object {object_id}"));
    let behavior = PoisonedBehavior::new(object, Arc::clone(&data_arc));
    let module_name = AsciiString::from("PoisonedBehavior");
    Box::new(PoisonedBehaviorModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

fn jet_slow_death_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = JetSlowDeathBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse JetSlowDeathBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn jet_slow_death_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<JetSlowDeathBehaviorModuleData>("JetSlowDeathBehavior", &module_data);
    let object_id = resolve_owner_id(&thing);
    let object = TheGameLogic::find_object_by_id(object_id)
        .unwrap_or_else(|| panic!("JetSlowDeathBehavior requires owning object {object_id}"));
    let behavior = JetSlowDeathBehavior::new(object, Arc::clone(&data_arc));
    let module_name = AsciiString::from("JetSlowDeathBehavior");
    Box::new(JetSlowDeathBehaviorModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

fn open_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = OpenContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse OpenContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn open_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<OpenContainModuleData>>()
        .expect("OpenContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain =
        OpenContain::new(owner_weak(owner_id), typed_data.contain_data()).unwrap_or_else(|_| {
            OpenContain::new(Weak::new(), &OpenContainModuleData::default())
                .expect("OpenContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("OpenContain", thing, module_data, contain)
}

fn transport_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = TransportContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse TransportContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn transport_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<TransportContainModuleData>>()
        .expect("TransportContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = TransportContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            TransportContain::new(Weak::new(), &TransportContainModuleData::default())
                .expect("TransportContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("TransportContain", thing, module_data, contain)
}

fn garrison_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = GarrisonContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse GarrisonContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn garrison_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<GarrisonContainModuleData>>()
        .expect("GarrisonContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = GarrisonContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            GarrisonContain::new(Weak::new(), &GarrisonContainModuleData::default())
                .expect("GarrisonContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("GarrisonContain", thing, module_data, contain)
}

fn tunnel_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = TunnelContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse TunnelContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn tunnel_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<TunnelContainModuleData>>()
        .expect("TunnelContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = TunnelContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            TunnelContain::new(Weak::new(), &TunnelContainModuleData::default())
                .expect("TunnelContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("TunnelContain", thing, module_data, contain)
}

fn overlord_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = OverlordContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse OverlordContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn overlord_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<OverlordContainModuleData>>()
        .expect("OverlordContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = OverlordContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            OverlordContain::new(Weak::new(), &OverlordContainModuleData::default())
                .expect("OverlordContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("OverlordContain", thing, module_data, contain)
}

fn helix_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HelixContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HelixContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn helix_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<HelixContainModuleData>>()
        .expect("HelixContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = HelixContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            HelixContain::new(Weak::new(), &HelixContainModuleData::default())
                .expect("HelixContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("HelixContain", thing, module_data, contain)
}

fn railed_transport_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RailedTransportContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RailedTransportContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn railed_transport_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<RailedTransportContainModuleData>>()
        .expect("RailedTransportContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = RailedTransportContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            RailedTransportContain::new(Weak::new(), &RailedTransportContainModuleData::default())
                .expect("RailedTransportContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("RailedTransportContain", thing, module_data, contain)
}

fn rider_change_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RiderChangeContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RiderChangeContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn rider_change_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<RiderChangeContainModuleData>>()
        .expect("RiderChangeContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = RiderChangeContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            RiderChangeContain::new(Weak::new(), &RiderChangeContainModuleData::default())
                .expect("RiderChangeContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("RiderChangeContain", thing, module_data, contain)
}

fn internet_hack_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = InternetHackContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse InternetHackContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn internet_hack_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<InternetHackContainModuleData>>()
        .expect("InternetHackContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = InternetHackContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            InternetHackContain::new(Weak::new(), &InternetHackContainModuleData::default())
                .expect("InternetHackContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("InternetHackContain", thing, module_data, contain)
}

fn heal_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HealContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HealContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn heal_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<HealContainModuleData>>()
        .expect("HealContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain =
        HealContain::new(owner_weak(owner_id), typed_data.contain_data()).unwrap_or_else(|_| {
            HealContain::new(Weak::new(), &HealContainModuleData::default())
                .expect("HealContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("HealContain", thing, module_data, contain)
}

fn cave_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CaveContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CaveContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn cave_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<CaveContainModuleData>>()
        .expect("CaveContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = CaveContain::new(owner_weak(owner_id), typed_data.contain_data(), None)
        .unwrap_or_else(|_| {
            CaveContain::new(Weak::new(), &CaveContainModuleData::default(), None)
                .expect("CaveContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("CaveContain", thing, module_data, contain)
}

fn parachute_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ParachuteContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ParachuteContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn parachute_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<ParachuteContainModuleData>>()
        .expect("ParachuteContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = ParachuteContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            ParachuteContain::new(Weak::new(), &ParachuteContainModuleData::default())
                .expect("ParachuteContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("ParachuteContain", thing, module_data, contain)
}

fn mob_nexus_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MobNexusContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MobNexusContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

fn mob_nexus_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<MobNexusContainModuleData>>()
        .expect("MobNexusContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = MobNexusContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            MobNexusContain::new(Weak::new(), &MobNexusContainModuleData::default())
                .expect("MobNexusContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("MobNexusContain", thing, module_data, contain)
}

macro_rules! draw_data_factory {
    ($factory:ident, $data_ty:ty, $module_name:literal, parse) => {
        fn $factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::new();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        concat!(
                            "Failed to parse ",
                            $module_name,
                            " module data at line {}: {}"
                        ),
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }
    };
    ($factory:ident, $data_ty:ty, $module_name:literal, no_parse) => {
        fn $factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            Box::new(<$data_ty>::new())
        }
    };
}

macro_rules! owner_bound_draw_factory {
    ($factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        fn $factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data = module_data
                .as_ref()
                .as_any()
                .downcast_ref::<$data_ty>()
                .cloned()
                .unwrap_or_else(|| {
                    warn!(concat!(
                        $module_name,
                        " module data expected; using defaults"
                    ));
                    <$data_ty>::new()
                });
            let mut module = <$module_ty>::new(data);
            let owner_id = resolve_owner_id(&thing);
            if owner_id != INVALID_ID {
                module.bind_owner_id(owner_id);
            }
            Box::new(module)
        }
    };
}

macro_rules! plain_draw_factory {
    ($factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        fn $factory(
            _thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data = module_data
                .as_ref()
                .as_any()
                .downcast_ref::<$data_ty>()
                .cloned()
                .unwrap_or_else(|| {
                    warn!(concat!(
                        $module_name,
                        " module data expected; using defaults"
                    ));
                    <$data_ty>::new()
                });
            Box::new(<$module_ty>::new(data))
        }
    };
}

draw_data_factory!(
    w3d_model_draw_module_data_factory,
    W3DModelDrawModuleData,
    "W3DModelDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_model_draw_module_factory,
    W3DModelDrawModuleData,
    W3DModelDraw,
    "W3DModelDraw"
);

draw_data_factory!(
    w3d_default_draw_module_data_factory,
    W3DDefaultDrawModuleData,
    "W3DDefaultDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_default_draw_module_factory,
    W3DDefaultDrawModuleData,
    W3DDefaultDraw,
    "W3DDefaultDraw"
);

draw_data_factory!(
    w3d_dependency_model_draw_module_data_factory,
    W3DDependencyModelDrawModuleData,
    "W3DDependencyModelDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_dependency_model_draw_module_factory,
    W3DDependencyModelDrawModuleData,
    W3DDependencyModelDraw,
    "W3DDependencyModelDraw"
);

draw_data_factory!(
    w3d_tank_draw_module_data_factory,
    W3DTankDrawModuleData,
    "W3DTankDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_tank_draw_module_factory,
    W3DTankDrawModuleData,
    W3DTankDraw,
    "W3DTankDraw"
);

draw_data_factory!(
    w3d_overlord_tank_draw_module_data_factory,
    W3DOverlordTankDrawModuleData,
    "W3DOverlordTankDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_overlord_tank_draw_module_factory,
    W3DOverlordTankDrawModuleData,
    W3DOverlordTankDraw,
    "W3DOverlordTankDraw"
);

draw_data_factory!(
    w3d_overlord_aircraft_draw_module_data_factory,
    W3DOverlordAircraftDrawModuleData,
    "W3DOverlordAircraftDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_overlord_aircraft_draw_module_factory,
    W3DOverlordAircraftDrawModuleData,
    W3DOverlordAircraftDraw,
    "W3DOverlordAircraftDraw"
);

draw_data_factory!(
    w3d_overlord_truck_draw_module_data_factory,
    W3DOverlordTruckDrawModuleData,
    "W3DOverlordTruckDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_overlord_truck_draw_module_factory,
    W3DOverlordTruckDrawModuleData,
    W3DOverlordTruckDraw,
    "W3DOverlordTruckDraw"
);

draw_data_factory!(
    w3d_police_car_draw_module_data_factory,
    W3DPoliceCarDrawModuleData,
    "W3DPoliceCarDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_police_car_draw_module_factory,
    W3DPoliceCarDrawModuleData,
    W3DPoliceCarDraw,
    "W3DPoliceCarDraw"
);

draw_data_factory!(
    w3d_projectile_stream_draw_module_data_factory,
    W3DProjectileStreamDrawModuleData,
    "W3DProjectileStreamDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_projectile_stream_draw_module_factory,
    W3DProjectileStreamDrawModuleData,
    W3DProjectileStreamDraw,
    "W3DProjectileStreamDraw"
);

draw_data_factory!(
    w3d_rope_draw_module_data_factory,
    W3DRopeDrawModuleData,
    "W3DRopeDraw",
    no_parse
);
plain_draw_factory!(
    w3d_rope_draw_module_factory,
    W3DRopeDrawModuleData,
    W3DRopeDraw,
    "W3DRopeDraw"
);

draw_data_factory!(
    w3d_science_model_draw_module_data_factory,
    W3DScienceModelDrawModuleData,
    "W3DScienceModelDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_science_model_draw_module_factory,
    W3DScienceModelDrawModuleData,
    W3DScienceModelDraw,
    "W3DScienceModelDraw"
);

draw_data_factory!(
    w3d_supply_draw_module_data_factory,
    W3DSupplyDrawModuleData,
    "W3DSupplyDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_supply_draw_module_factory,
    W3DSupplyDrawModuleData,
    W3DSupplyDraw,
    "W3DSupplyDraw"
);

draw_data_factory!(
    w3d_tank_truck_draw_module_data_factory,
    W3DTankTruckDrawModuleData,
    "W3DTankTruckDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_tank_truck_draw_module_factory,
    W3DTankTruckDrawModuleData,
    W3DTankTruckDraw,
    "W3DTankTruckDraw"
);

draw_data_factory!(
    w3d_tracer_draw_module_data_factory,
    W3DTracerDrawModuleData,
    "W3DTracerDraw",
    no_parse
);
plain_draw_factory!(
    w3d_tracer_draw_module_factory,
    W3DTracerDrawModuleData,
    W3DTracerDraw,
    "W3DTracerDraw"
);

draw_data_factory!(
    w3d_tree_draw_module_data_factory,
    W3DTreeDrawModuleData,
    "W3DTreeDraw",
    parse
);
fn w3d_tree_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<W3DTreeDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DTreeDraw module data expected; using defaults");
            W3DTreeDrawModuleData::new()
        });
    let mut module = W3DTreeDraw::new(data);
    let drawable_id = resolve_drawable_id(&thing);
    if drawable_id != INVALID_ID {
        module.bind_drawable_id(drawable_id);
    }
    Box::new(module)
}

draw_data_factory!(
    w3d_truck_draw_module_data_factory,
    W3DTruckDrawModuleData,
    "W3DTruckDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_truck_draw_module_factory,
    W3DTruckDrawModuleData,
    W3DTruckDraw,
    "W3DTruckDraw"
);

draw_data_factory!(
    w3d_laser_draw_module_data_factory,
    W3DLaserDrawModuleData,
    "W3DLaserDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_laser_draw_module_factory,
    W3DLaserDrawModuleData,
    W3DLaserDraw,
    "W3DLaserDraw"
);

draw_data_factory!(
    w3d_debris_draw_module_data_factory,
    W3DDebrisDrawModuleData,
    "W3DDebrisDraw",
    no_parse
);
owner_bound_draw_factory!(
    w3d_debris_draw_module_factory,
    W3DDebrisDrawModuleData,
    W3DDebrisDraw,
    "W3DDebrisDraw"
);

fn laser_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = LaserClientUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse LaserUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn laser_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<LaserClientUpdateModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("LaserUpdate module data expected; using defaults");
            LaserClientUpdateModuleData::default()
        });
    let module_data = Arc::new(typed_data);
    Box::new(LaserClientUpdateModule::new(
        NameKeyGenerator::name_to_key("LaserUpdate"),
        module_data,
        Some(resolve_owner_id(&thing)),
    ))
}

fn beacon_client_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BeaconClientUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BeaconClientUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn beacon_client_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<BeaconClientUpdateModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("BeaconClientUpdate module data expected; using defaults");
            BeaconClientUpdateModuleData::default()
        });
    let module_data = Arc::new(typed_data);
    Box::new(BeaconClientUpdateModule::new(
        NameKeyGenerator::name_to_key("BeaconClientUpdate"),
        module_data,
        resolve_owner_id(&thing),
    ))
}

fn base_client_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(BaseModuleData::new())
}

fn sway_client_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    Box::new(SwayClientUpdateModule::new(
        NameKeyGenerator::name_to_key("SwayClientUpdate"),
        module_data,
        resolve_owner_id(&thing),
    ))
}

fn animated_particle_sys_bone_client_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    Box::new(AnimatedParticleSysBoneClientUpdateModule::new(
        NameKeyGenerator::name_to_key("AnimatedParticleSysBoneClientUpdate"),
        module_data,
        resolve_owner_id(&thing),
    ))
}

macro_rules! special_power_factories {
    (
        $data_factory:ident,
        $module_factory:ident,
        $data_ty:ty,
        $module_ty:ty,
        $module_name:literal
    ) => {
        fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        concat!(
                            "Failed to parse ",
                            $module_name,
                            " module data at line {}: {}"
                        ),
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let typed_data = module_data
                .as_ref()
                .as_any()
                .downcast_ref::<$data_ty>()
                .cloned()
                .unwrap_or_else(|| {
                    warn!(concat!(
                        $module_name,
                        " module data expected; using defaults"
                    ));
                    <$data_ty>::default()
                });
            Box::new(<$module_ty>::new(
                NameKeyGenerator::name_to_key($module_name),
                resolve_owner_id(&thing),
                Arc::new(typed_data),
            ))
        }
    };
}

fn baikonur_launch_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BaikonurLaunchPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BaikonurLaunchPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn baikonur_launch_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<BaikonurLaunchPowerModuleData>("BaikonurLaunchPower", &module_data);
    Box::new(BaikonurLaunchPower::new(
        NameKeyGenerator::name_to_key("BaikonurLaunchPower"),
        resolve_owner_id(&thing),
        data_arc,
    ))
}

special_power_factories!(
    cash_bounty_power_module_data_factory,
    cash_bounty_power_module_factory,
    CashBountyPowerModuleData,
    CashBountyPower,
    "CashBountyPower"
);
special_power_factories!(
    cash_hack_special_power_module_data_factory,
    cash_hack_special_power_module_factory,
    CashHackSpecialPowerModuleData,
    CashHackSpecialPower,
    "CashHackSpecialPower"
);
special_power_factories!(
    cleanup_area_power_module_data_factory,
    cleanup_area_power_module_factory,
    CleanupAreaPowerModuleData,
    CleanupAreaPower,
    "CleanupAreaPower"
);
special_power_factories!(
    fire_weapon_power_module_data_factory,
    fire_weapon_power_module_factory,
    FireWeaponPowerModuleData,
    FireWeaponPower,
    "FireWeaponPower"
);
special_power_factories!(
    ocl_special_power_module_data_factory,
    ocl_special_power_module_factory,
    OclSpecialPowerModuleData,
    OclSpecialPower,
    "OCLSpecialPower"
);
special_power_factories!(
    special_ability_module_data_factory,
    special_ability_module_factory,
    SpecialAbilityModuleData,
    SpecialAbility,
    "SpecialAbility"
);
special_power_factories!(
    spy_vision_special_power_module_data_factory,
    spy_vision_special_power_module_factory,
    SpyVisionSpecialPowerModuleData,
    SpyVisionSpecialPower,
    "SpyVisionSpecialPower"
);

fn install_contain_overrides() -> Result<(), String> {
    register_module_override(
        "InactiveBody",
        ModuleType::Behavior,
        inactive_body_module_factory,
        inactive_body_module_data_factory,
    )?;
    register_module_override(
        "ActiveBody",
        ModuleType::Behavior,
        active_body_module_factory,
        active_body_module_data_factory,
    )?;
    register_module_override(
        "StructureBody",
        ModuleType::Behavior,
        structure_body_module_factory,
        structure_body_module_data_factory,
    )?;
    register_module_override(
        "HighlanderBody",
        ModuleType::Behavior,
        highlander_body_module_factory,
        highlander_body_module_data_factory,
    )?;
    register_module_override(
        "ImmortalBody",
        ModuleType::Behavior,
        immortal_body_module_factory,
        immortal_body_module_data_factory,
    )?;
    register_module_override(
        "HiveStructureBody",
        ModuleType::Behavior,
        hive_structure_body_module_factory,
        hive_structure_body_module_data_factory,
    )?;
    register_module_override(
        "UndeadBody",
        ModuleType::Behavior,
        undead_body_module_factory,
        undead_body_module_data_factory,
    )?;
    register_module_override(
        "DestroyDie",
        ModuleType::Behavior,
        destroy_die_module_factory,
        destroy_die_module_data_factory,
    )?;
    register_module_override(
        "KeepObjectDie",
        ModuleType::Behavior,
        keep_object_die_module_factory,
        keep_object_die_module_data_factory,
    )?;
    register_module_override(
        "UpgradeDie",
        ModuleType::Behavior,
        upgrade_die_module_factory,
        upgrade_die_module_data_factory,
    )?;
    register_module_override(
        "CreateObjectDie",
        ModuleType::Behavior,
        create_object_die_module_factory,
        create_object_die_module_data_factory,
    )?;
    register_module_override(
        "CreateCrateDie",
        ModuleType::Behavior,
        create_crate_die_module_factory,
        create_crate_die_module_data_factory,
    )?;
    register_module_override(
        "FXListDie",
        ModuleType::Behavior,
        fx_list_die_module_factory,
        fx_list_die_module_data_factory,
    )?;
    register_module_override(
        "CrushDie",
        ModuleType::Behavior,
        crush_die_module_factory,
        crush_die_module_data_factory,
    )?;
    register_module_override(
        "EjectPilotDie",
        ModuleType::Behavior,
        eject_pilot_die_module_factory,
        eject_pilot_die_module_data_factory,
    )?;
    register_module_override(
        "RebuildHoleExposeDie",
        ModuleType::Behavior,
        rebuild_hole_expose_die_module_factory,
        rebuild_hole_expose_die_module_data_factory,
    )?;
    register_module_override(
        "SpecialPowerCompletionDie",
        ModuleType::Behavior,
        special_power_completion_die_module_factory,
        special_power_completion_die_module_data_factory,
    )?;
    register_module_override(
        "DamDie",
        ModuleType::Behavior,
        dam_die_module_factory,
        dam_die_module_data_factory,
    )?;
    register_module_override(
        "HelicopterSlowDeathBehavior",
        ModuleType::Behavior,
        helicopter_slow_death_module_factory,
        helicopter_slow_death_data_factory,
    )?;
    register_module_override(
        "SlowDeathBehavior",
        ModuleType::Behavior,
        slow_death_behavior_module_factory,
        slow_death_behavior_module_data_factory,
    )?;
    register_module_override(
        "PoisonedBehavior",
        ModuleType::Behavior,
        poisoned_behavior_module_factory,
        poisoned_behavior_data_factory,
    )?;
    register_module_override(
        "JetSlowDeathBehavior",
        ModuleType::Behavior,
        jet_slow_death_behavior_module_factory,
        jet_slow_death_behavior_data_factory,
    )?;
    register_module_override(
        "InstantDeathBehavior",
        ModuleType::Behavior,
        instant_death_behavior_module_factory,
        instant_death_behavior_module_data_factory,
    )?;
    register_module_override(
        "BattleBusSlowDeathBehavior",
        ModuleType::Behavior,
        battle_bus_slow_death_module_factory,
        battle_bus_slow_death_data_factory,
    )?;
    register_module_override(
        "NeutronMissileSlowDeathBehavior",
        ModuleType::Behavior,
        neutron_missile_slow_death_module_factory,
        neutron_missile_slow_death_data_factory,
    )?;
    register_module_override(
        "NeutronMissileUpdate",
        ModuleType::Behavior,
        neutron_missile_update_module_factory,
        neutron_missile_update_data_factory,
    )?;
    register_module_override(
        "ToppleUpdate",
        ModuleType::Behavior,
        topple_update_module_factory,
        topple_update_data_factory,
    )?;
    register_module_override(
        "LifetimeUpdate",
        ModuleType::Behavior,
        lifetime_update_module_factory,
        lifetime_update_data_factory,
    )?;
    register_module_override(
        "OCLUpdate",
        ModuleType::Behavior,
        ocl_update_module_factory,
        ocl_update_data_factory,
    )?;
    register_module_override(
        "RadiusDecalUpdate",
        ModuleType::Behavior,
        radius_decal_update_module_factory,
        radius_decal_update_data_factory,
    )?;
    register_module_override(
        "StickyBombUpdate",
        ModuleType::Behavior,
        sticky_bomb_update_module_factory,
        sticky_bomb_update_data_factory,
    )?;
    register_module_override(
        "DemoTrapUpdate",
        ModuleType::Behavior,
        demo_trap_update_module_factory,
        demo_trap_update_data_factory,
    )?;
    register_module_override(
        "PointDefenseLaserUpdate",
        ModuleType::Behavior,
        point_defense_laser_update_module_factory,
        point_defense_laser_update_data_factory,
    )?;
    register_module_override(
        "ProjectileStreamUpdate",
        ModuleType::Behavior,
        projectile_stream_update_module_factory,
        projectile_stream_update_data_factory,
    )?;
    register_module_override(
        "SmartBombTargetHomingUpdate",
        ModuleType::Behavior,
        smart_bomb_target_homing_update_module_factory,
        smart_bomb_target_homing_update_data_factory,
    )?;
    register_module_override(
        "TensileFormationUpdate",
        ModuleType::Behavior,
        tensile_formation_update_module_factory,
        tensile_formation_update_data_factory,
    )?;
    register_module_override(
        "AnimationSteeringUpdate",
        ModuleType::Behavior,
        animation_steering_update_module_factory,
        animation_steering_update_data_factory,
    )?;
    register_module_override(
        "AssistedTargetingUpdate",
        ModuleType::Behavior,
        assisted_targeting_update_module_factory,
        assisted_targeting_update_data_factory,
    )?;
    register_module_override(
        "AutoDepositUpdate",
        ModuleType::Behavior,
        auto_deposit_update_module_factory,
        auto_deposit_update_data_factory,
    )?;
    register_module_override(
        "AutoFindHealingUpdate",
        ModuleType::Behavior,
        auto_find_healing_update_module_factory,
        auto_find_healing_update_data_factory,
    )?;
    register_module_override(
        "BaseRegenerateUpdate",
        ModuleType::Behavior,
        base_regenerate_update_module_factory,
        base_regenerate_update_data_factory,
    )?;
    register_module_override(
        "BattlePlanUpdate",
        ModuleType::Behavior,
        battle_plan_update_module_factory,
        battle_plan_update_data_factory,
    )?;
    register_module_override(
        "CleanupHazardUpdate",
        ModuleType::Behavior,
        cleanup_hazard_update_module_factory,
        cleanup_hazard_update_data_factory,
    )?;
    register_module_override(
        "CommandButtonHuntUpdate",
        ModuleType::Behavior,
        command_button_hunt_update_module_factory,
        command_button_hunt_update_data_factory,
    )?;
    register_module_override(
        "SpyVisionUpdate",
        ModuleType::Behavior,
        spy_vision_update_module_factory,
        spy_vision_update_data_factory,
    )?;
    register_module_override(
        "SlavedUpdate",
        ModuleType::Behavior,
        slaved_update_module_factory,
        slaved_update_data_factory,
    )?;
    register_module_override(
        "MobMemberSlavedUpdate",
        ModuleType::Behavior,
        mob_member_slaved_update_module_factory,
        mob_member_slaved_update_data_factory,
    )?;
    register_module_override(
        "FireSpreadUpdate",
        ModuleType::Behavior,
        fire_spread_update_module_factory,
        fire_spread_update_data_factory,
    )?;
    register_module_override(
        "RebuildHoleBehavior",
        ModuleType::Behavior,
        rebuild_hole_behavior_module_factory,
        rebuild_hole_behavior_data_factory,
    )?;
    register_module_override(
        "OverchargeBehavior",
        ModuleType::Behavior,
        overcharge_behavior_module_factory,
        overcharge_behavior_data_factory,
    )?;
    register_module_override(
        "AutoHealBehavior",
        ModuleType::Behavior,
        auto_heal_behavior_module_factory,
        auto_heal_behavior_data_factory,
    )?;
    register_module_override(
        "CountermeasuresBehavior",
        ModuleType::Behavior,
        countermeasures_behavior_module_factory,
        countermeasures_behavior_data_factory,
    )?;
    register_module_override(
        "DumbProjectileBehavior",
        ModuleType::Behavior,
        dumb_projectile_behavior_module_factory,
        dumb_projectile_behavior_data_factory,
    )?;
    register_module_override(
        "BridgeBehavior",
        ModuleType::Behavior,
        bridge_behavior_module_factory,
        bridge_behavior_data_factory,
    )?;
    register_module_override(
        "BridgeScaffoldBehavior",
        ModuleType::Behavior,
        bridge_scaffold_behavior_module_factory,
        bridge_scaffold_behavior_data_factory,
    )?;
    register_module_override(
        "BridgeTowerBehavior",
        ModuleType::Behavior,
        bridge_tower_behavior_module_factory,
        bridge_tower_behavior_data_factory,
    )?;
    register_module_override(
        "StructureCollapseUpdate",
        ModuleType::Behavior,
        structure_collapse_update_module_factory,
        structure_collapse_update_data_factory,
    )?;
    register_module_override(
        "StructureToppleUpdate",
        ModuleType::Behavior,
        structure_topple_update_module_factory,
        structure_topple_update_data_factory,
    )?;
    register_module_override(
        "GrantStealthBehavior",
        ModuleType::Behavior,
        grant_stealth_behavior_module_factory,
        grant_stealth_behavior_data_factory,
    )?;
    register_module_override(
        "StealthUpdate",
        ModuleType::Behavior,
        stealth_update_module_factory,
        stealth_update_module_data_factory,
    )?;
    register_module_override(
        "TransitionDamageFX",
        ModuleType::Behavior,
        transition_damage_fx_module_factory,
        transition_damage_fx_module_data_factory,
    )?;
    register_module_override(
        "EMPUpdate",
        ModuleType::Behavior,
        emp_update_module_factory,
        emp_update_data_factory,
    )?;
    register_module_override(
        "BoneFXUpdate",
        ModuleType::Behavior,
        bone_fx_update_module_factory,
        bone_fx_update_data_factory,
    )?;
    register_module_override(
        "BoneFXDamage",
        ModuleType::Behavior,
        bone_fx_damage_module_factory,
        bone_fx_damage_data_factory,
    )?;
    register_module_override(
        "SpawnBehavior",
        ModuleType::Behavior,
        spawn_behavior_module_factory,
        spawn_behavior_data_factory,
    )?;
    register_module_override(
        "ParticleUplinkCannonUpdate",
        ModuleType::Behavior,
        particle_uplink_cannon_update_module_factory,
        particle_uplink_cannon_update_data_factory,
    )?;
    register_module_override(
        "DefaultProductionExitUpdate",
        ModuleType::Behavior,
        default_production_exit_update_module_factory,
        default_production_exit_update_data_factory,
    )?;
    register_module_override(
        "QueueProductionExitUpdate",
        ModuleType::Behavior,
        queue_production_exit_update_module_factory,
        queue_production_exit_update_data_factory,
    )?;
    register_module_override(
        "SpawnPointProductionExitUpdate",
        ModuleType::Behavior,
        spawn_point_production_exit_update_module_factory,
        spawn_point_production_exit_update_data_factory,
    )?;
    register_module_override(
        "SupplyCenterProductionExitUpdate",
        ModuleType::Behavior,
        supply_center_production_exit_update_module_factory,
        supply_center_production_exit_update_data_factory,
    )?;
    register_module_override(
        "FlightDeckBehavior",
        ModuleType::Behavior,
        flight_deck_behavior_module_factory,
        flight_deck_behavior_data_factory,
    )?;
    register_module_override(
        "AIUpdateInterface",
        ModuleType::Behavior,
        ai_update_interface_module_factory,
        ai_update_interface_data_factory,
    )?;
    register_module_override(
        "TransportAIUpdate",
        ModuleType::Behavior,
        transport_ai_update_module_factory,
        transport_ai_update_data_factory,
    )?;
    register_module_override(
        "DeployStyleAIUpdate",
        ModuleType::Behavior,
        deploy_style_ai_update_module_factory,
        deploy_style_ai_update_data_factory,
    )?;
    register_module_override(
        "WanderAIUpdate",
        ModuleType::Behavior,
        wander_ai_update_module_factory,
        wander_ai_update_data_factory,
    )?;
    register_module_override(
        "JetAIUpdate",
        ModuleType::Behavior,
        jet_ai_update_module_factory,
        jet_ai_update_data_factory,
    )?;
    register_module_override(
        "RailedTransportAIUpdate",
        ModuleType::Behavior,
        railed_transport_ai_update_module_factory,
        railed_transport_ai_update_data_factory,
    )?;
    register_module_override(
        "AssaultTransportAIUpdate",
        ModuleType::Behavior,
        assault_transport_ai_update_module_factory,
        assault_transport_ai_update_data_factory,
    )?;
    register_module_override(
        "DeliverPayloadAIUpdate",
        ModuleType::Behavior,
        deliver_payload_ai_update_module_factory,
        deliver_payload_ai_update_data_factory,
    )?;
    register_module_override(
        "HackInternetAIUpdate",
        ModuleType::Behavior,
        hack_internet_ai_update_module_factory,
        hack_internet_ai_update_data_factory,
    )?;
    register_module_override(
        "SupplyTruckAIUpdate",
        ModuleType::Behavior,
        supply_truck_ai_update_module_factory,
        supply_truck_ai_update_data_factory,
    )?;
    register_module_override(
        "ChinookAIUpdate",
        ModuleType::Behavior,
        chinook_ai_update_module_factory,
        chinook_ai_update_data_factory,
    )?;
    register_module_override(
        "WorkerAIUpdate",
        ModuleType::Behavior,
        worker_ai_update_module_factory,
        worker_ai_update_data_factory,
    )?;
    register_module_override(
        "DozerAIUpdate",
        ModuleType::Behavior,
        dozer_ai_update_module_factory,
        dozer_ai_update_data_factory,
    )?;
    register_module_override(
        "MissileAIUpdate",
        ModuleType::Behavior,
        missile_ai_update_module_factory,
        missile_ai_update_data_factory,
    )?;
    register_module_override(
        "PhysicsBehavior",
        ModuleType::Behavior,
        physics_behavior_module_factory,
        physics_behavior_data_factory,
    )?;
    register_module_override(
        "FlammableUpdate",
        ModuleType::Behavior,
        flammable_update_module_factory,
        flammable_update_data_factory,
    )?;
    register_module_override(
        "SpecialAbilityUpdate",
        ModuleType::Behavior,
        special_ability_update_module_factory,
        special_ability_update_data_factory,
    )?;
    register_module_override(
        "RailroadBehavior",
        ModuleType::Behavior,
        railroad_behavior_module_factory,
        railroad_behavior_data_factory,
    )?;
    register_module_override(
        "ProductionUpdate",
        ModuleType::Behavior,
        production_update_module_factory,
        production_update_data_factory,
    )?;
    register_module_override(
        "RepairDockUpdate",
        ModuleType::Behavior,
        repair_dock_update_module_factory,
        repair_dock_update_data_factory,
    )?;
    register_module_override(
        "RailedTransportDockUpdate",
        ModuleType::Behavior,
        railed_transport_dock_update_module_factory,
        railed_transport_dock_update_data_factory,
    )?;
    register_module_override(
        "SupplyCenterDockUpdate",
        ModuleType::Behavior,
        supply_center_dock_update_module_factory,
        supply_center_dock_update_data_factory,
    )?;
    register_module_override(
        "SupplyWarehouseDockUpdate",
        ModuleType::Behavior,
        supply_warehouse_dock_update_module_factory,
        supply_warehouse_dock_update_data_factory,
    )?;
    register_module_override(
        "SupplyWarehouseCripplingBehavior",
        ModuleType::Behavior,
        supply_warehouse_crippling_behavior_module_factory,
        supply_warehouse_crippling_behavior_data_factory,
    )?;
    register_module_override(
        "ArmorUpgrade",
        ModuleType::Behavior,
        armor_upgrade_module_factory,
        armor_upgrade_data_factory,
    )?;
    register_module_override(
        "CommandSetUpgrade",
        ModuleType::Behavior,
        command_set_upgrade_module_factory,
        command_set_upgrade_data_factory,
    )?;
    register_module_override(
        "CostModifierUpgrade",
        ModuleType::Behavior,
        cost_modifier_upgrade_module_factory,
        cost_modifier_upgrade_data_factory,
    )?;
    register_module_override(
        "ExperienceScalarUpgrade",
        ModuleType::Behavior,
        experience_scalar_upgrade_module_factory,
        experience_scalar_upgrade_data_factory,
    )?;
    register_module_override(
        "GrantScienceUpgrade",
        ModuleType::Behavior,
        grant_science_upgrade_module_factory,
        grant_science_upgrade_data_factory,
    )?;
    register_module_override(
        "LocomotorSetUpgrade",
        ModuleType::Behavior,
        locomotor_set_upgrade_module_factory,
        locomotor_set_upgrade_data_factory,
    )?;
    register_module_override(
        "MaxHealthUpgrade",
        ModuleType::Behavior,
        max_health_upgrade_module_factory,
        max_health_upgrade_data_factory,
    )?;
    register_module_override(
        "ModelConditionUpgrade",
        ModuleType::Behavior,
        model_condition_upgrade_module_factory,
        model_condition_upgrade_data_factory,
    )?;
    register_module_override(
        "ObjectCreationUpgrade",
        ModuleType::Behavior,
        object_creation_upgrade_module_factory,
        object_creation_upgrade_data_factory,
    )?;
    register_module_override(
        "PassengersFireUpgrade",
        ModuleType::Behavior,
        passengers_fire_upgrade_module_factory,
        passengers_fire_upgrade_data_factory,
    )?;
    register_module_override(
        "PowerPlantUpgrade",
        ModuleType::Behavior,
        power_plant_upgrade_module_factory,
        power_plant_upgrade_data_factory,
    )?;
    register_module_override(
        "RadarUpgrade",
        ModuleType::Behavior,
        radar_upgrade_module_factory,
        radar_upgrade_data_factory,
    )?;
    register_module_override(
        "ReplaceObjectUpgrade",
        ModuleType::Behavior,
        replace_object_upgrade_module_factory,
        replace_object_upgrade_data_factory,
    )?;
    register_module_override(
        "StealthUpgrade",
        ModuleType::Behavior,
        stealth_upgrade_module_factory,
        stealth_upgrade_data_factory,
    )?;
    register_module_override(
        "SubObjectsUpgrade",
        ModuleType::Behavior,
        subobjects_upgrade_module_factory,
        subobjects_upgrade_data_factory,
    )?;
    register_module_override(
        "UnpauseSpecialPowerUpgrade",
        ModuleType::Behavior,
        unpause_special_power_upgrade_module_factory,
        unpause_special_power_upgrade_data_factory,
    )?;
    register_module_override(
        "WeaponBonusUpgrade",
        ModuleType::Behavior,
        weapon_bonus_upgrade_module_factory,
        weapon_bonus_upgrade_data_factory,
    )?;
    register_module_override(
        "WeaponSetUpgrade",
        ModuleType::Behavior,
        weapon_set_upgrade_module_factory,
        weapon_set_upgrade_data_factory,
    )?;
    register_module_override(
        "FireWeaponCollide",
        ModuleType::Behavior,
        fire_weapon_collide_module_factory,
        fire_weapon_collide_data_factory,
    )?;
    register_module_override(
        "SquishCollide",
        ModuleType::Behavior,
        squish_collide_module_factory,
        squish_collide_data_factory,
    )?;
    register_module_override(
        "GrantUpgradeCreate",
        ModuleType::Behavior,
        grant_upgrade_create_module_factory,
        grant_upgrade_create_data_factory,
    )?;
    register_module_override(
        "LockWeaponCreate",
        ModuleType::Behavior,
        lock_weapon_create_module_factory,
        lock_weapon_create_data_factory,
    )?;
    register_module_override(
        "PreorderCreate",
        ModuleType::Behavior,
        preorder_create_module_factory,
        preorder_create_data_factory,
    )?;
    register_module_override(
        "SpecialPowerCreate",
        ModuleType::Behavior,
        special_power_create_module_factory,
        special_power_create_data_factory,
    )?;
    register_module_override(
        "SupplyCenterCreate",
        ModuleType::Behavior,
        supply_center_create_module_factory,
        supply_center_create_data_factory,
    )?;
    register_module_override(
        "SupplyWarehouseCreate",
        ModuleType::Behavior,
        supply_warehouse_create_module_factory,
        supply_warehouse_create_data_factory,
    )?;
    register_module_override(
        "VeterancyGainCreate",
        ModuleType::Behavior,
        veterancy_gain_create_module_factory,
        veterancy_gain_create_data_factory,
    )?;
    register_module_override(
        "ConvertToCarBombCrateCollide",
        ModuleType::Behavior,
        convert_to_car_bomb_crate_collide_module_factory,
        convert_to_car_bomb_crate_collide_data_factory,
    )?;
    register_module_override(
        "ConvertToHijackedVehicleCrateCollide",
        ModuleType::Behavior,
        convert_to_hijacked_vehicle_crate_collide_module_factory,
        convert_to_hijacked_vehicle_crate_collide_data_factory,
    )?;
    register_module_override(
        "SabotageCommandCenterCrateCollide",
        ModuleType::Behavior,
        sabotage_command_center_crate_collide_module_factory,
        sabotage_command_center_crate_collide_data_factory,
    )?;
    register_module_override(
        "SabotageFakeBuildingCrateCollide",
        ModuleType::Behavior,
        sabotage_fake_building_crate_collide_module_factory,
        sabotage_fake_building_crate_collide_data_factory,
    )?;
    register_module_override(
        "SabotageInternetCenterCrateCollide",
        ModuleType::Behavior,
        sabotage_internet_center_crate_collide_module_factory,
        sabotage_internet_center_crate_collide_data_factory,
    )?;
    register_module_override(
        "SabotageMilitaryFactoryCrateCollide",
        ModuleType::Behavior,
        sabotage_military_factory_crate_collide_module_factory,
        sabotage_military_factory_crate_collide_data_factory,
    )?;
    register_module_override(
        "SabotagePowerPlantCrateCollide",
        ModuleType::Behavior,
        sabotage_power_plant_crate_collide_module_factory,
        sabotage_power_plant_crate_collide_data_factory,
    )?;
    register_module_override(
        "SabotageSuperweaponCrateCollide",
        ModuleType::Behavior,
        sabotage_superweapon_crate_collide_module_factory,
        sabotage_superweapon_crate_collide_data_factory,
    )?;
    register_module_override(
        "SabotageSupplyCenterCrateCollide",
        ModuleType::Behavior,
        sabotage_supply_center_crate_collide_module_factory,
        sabotage_supply_center_crate_collide_data_factory,
    )?;
    register_module_override(
        "MoneyCrateCollide",
        ModuleType::Behavior,
        money_crate_collide_module_factory,
        money_crate_collide_data_factory,
    )?;
    register_module_override(
        "SalvageCrateCollide",
        ModuleType::Behavior,
        salvage_crate_collide_module_factory,
        salvage_crate_collide_data_factory,
    )?;
    register_module_override(
        "UnitCrateCollide",
        ModuleType::Behavior,
        unit_crate_collide_module_factory,
        unit_crate_collide_data_factory,
    )?;
    register_module_override(
        "VeterancyCrateCollide",
        ModuleType::Behavior,
        veterancy_crate_collide_module_factory,
        veterancy_crate_collide_data_factory,
    )?;
    register_module_override(
        "BunkerBusterBehavior",
        ModuleType::Behavior,
        bunker_buster_behavior_module_factory,
        bunker_buster_behavior_data_factory,
    )?;
    register_module_override(
        "CheckpointUpdate",
        ModuleType::Behavior,
        checkpoint_update_module_factory,
        checkpoint_update_data_factory,
    )?;
    register_module_override(
        "DeletionUpdate",
        ModuleType::Behavior,
        deletion_update_module_factory,
        deletion_update_data_factory,
    )?;
    register_module_override(
        "DynamicShroudClearingRangeUpdate",
        ModuleType::Behavior,
        dynamic_shroud_clearing_range_update_module_factory,
        dynamic_shroud_clearing_range_update_data_factory,
    )?;
    register_module_override(
        "EnemyNearUpdate",
        ModuleType::Behavior,
        enemy_near_update_module_factory,
        enemy_near_update_data_factory,
    )?;
    register_module_override(
        "FireOCLAfterWeaponCooldownUpdate",
        ModuleType::Behavior,
        fire_ocl_after_weapon_cooldown_update_module_factory,
        fire_ocl_after_weapon_cooldown_update_data_factory,
    )?;
    register_module_override(
        "FireWeaponWhenDamagedBehavior",
        ModuleType::Behavior,
        fire_weapon_when_damaged_behavior_module_factory,
        fire_weapon_when_damaged_behavior_data_factory,
    )?;
    register_module_override(
        "FireWeaponWhenDeadBehavior",
        ModuleType::Behavior,
        fire_weapon_when_dead_behavior_module_factory,
        fire_weapon_when_dead_behavior_data_factory,
    )?;
    register_module_override(
        "FireWeaponUpdate",
        ModuleType::Behavior,
        fire_weapon_update_module_factory,
        fire_weapon_update_data_factory,
    )?;
    register_module_override(
        "FirestormDynamicGeometryInfoUpdate",
        ModuleType::Behavior,
        firestorm_dynamic_geometry_info_update_module_factory,
        firestorm_dynamic_geometry_info_update_data_factory,
    )?;
    register_module_override(
        "FloatUpdate",
        ModuleType::Behavior,
        float_update_module_factory,
        float_update_data_factory,
    )?;
    register_module_override(
        "GenerateMinefieldBehavior",
        ModuleType::Behavior,
        generate_minefield_behavior_module_factory,
        generate_minefield_behavior_data_factory,
    )?;
    register_module_override(
        "MinefieldBehavior",
        ModuleType::Behavior,
        minefield_behavior_module_factory,
        minefield_behavior_data_factory,
    )?;
    register_module_override(
        "HeightDieUpdate",
        ModuleType::Behavior,
        height_die_update_module_factory,
        height_die_update_data_factory,
    )?;
    register_module_override(
        "HijackerUpdate",
        ModuleType::Behavior,
        hijacker_update_module_factory,
        hijacker_update_data_factory,
    )?;
    register_module_override(
        "HordeUpdate",
        ModuleType::Behavior,
        horde_update_module_factory,
        horde_update_data_factory,
    )?;
    register_module_override(
        "NeutronBlastBehavior",
        ModuleType::Behavior,
        neutron_blast_behavior_module_factory,
        neutron_blast_behavior_data_factory,
    )?;
    register_module_override(
        "LeafletDropBehavior",
        ModuleType::Behavior,
        leaflet_drop_behavior_module_factory,
        leaflet_drop_behavior_data_factory,
    )?;
    register_module_override(
        "MissileLauncherBuildingUpdate",
        ModuleType::Behavior,
        missile_launcher_building_update_module_factory,
        missile_launcher_building_update_data_factory,
    )?;
    register_module_override(
        "ParkingPlaceBehavior",
        ModuleType::Behavior,
        parking_place_behavior_module_factory,
        parking_place_behavior_data_factory,
    )?;
    register_module_override(
        "PilotFindVehicleUpdate",
        ModuleType::Behavior,
        pilot_find_vehicle_update_module_factory,
        pilot_find_vehicle_update_data_factory,
    )?;
    register_module_override(
        "PowerPlantUpdate",
        ModuleType::Behavior,
        power_plant_update_module_factory,
        power_plant_update_data_factory,
    )?;
    register_module_override(
        "PropagandaTowerBehavior",
        ModuleType::Behavior,
        propaganda_tower_behavior_module_factory,
        propaganda_tower_behavior_data_factory,
    )?;
    register_module_override(
        "RadarUpdate",
        ModuleType::Behavior,
        radar_update_module_factory,
        radar_update_data_factory,
    )?;
    register_module_override(
        "SpectreGunshipDeploymentUpdate",
        ModuleType::Behavior,
        spectre_gunship_deployment_update_module_factory,
        spectre_gunship_deployment_update_data_factory,
    )?;
    register_module_override(
        "SpectreGunshipUpdate",
        ModuleType::Behavior,
        spectre_gunship_update_module_factory,
        spectre_gunship_update_data_factory,
    )?;
    register_module_override(
        "StealthDetectorUpdate",
        ModuleType::Behavior,
        stealth_detector_update_module_factory,
        stealth_detector_update_data_factory,
    )?;
    register_module_override(
        "TechBuildingBehavior",
        ModuleType::Behavior,
        tech_building_behavior_module_factory,
        tech_building_behavior_data_factory,
    )?;
    register_module_override(
        "WaveGuideUpdate",
        ModuleType::Behavior,
        wave_guide_update_module_factory,
        wave_guide_update_data_factory,
    )?;
    register_module_override(
        "WeaponBonusUpdate",
        ModuleType::Behavior,
        weapon_bonus_update_module_factory,
        weapon_bonus_update_data_factory,
    )?;
    register_module_override(
        "OpenContain",
        ModuleType::Behavior,
        open_contain_module_factory,
        open_contain_module_data_factory,
    )?;
    register_module_override(
        "TransportContain",
        ModuleType::Behavior,
        transport_contain_module_factory,
        transport_contain_module_data_factory,
    )?;
    register_module_override(
        "GarrisonContain",
        ModuleType::Behavior,
        garrison_contain_module_factory,
        garrison_contain_module_data_factory,
    )?;
    register_module_override(
        "TunnelContain",
        ModuleType::Behavior,
        tunnel_contain_module_factory,
        tunnel_contain_module_data_factory,
    )?;
    register_module_override(
        "OverlordContain",
        ModuleType::Behavior,
        overlord_contain_module_factory,
        overlord_contain_module_data_factory,
    )?;
    register_module_override(
        "HelixContain",
        ModuleType::Behavior,
        helix_contain_module_factory,
        helix_contain_module_data_factory,
    )?;
    register_module_override(
        "ParachuteContain",
        ModuleType::Behavior,
        parachute_contain_module_factory,
        parachute_contain_module_data_factory,
    )?;
    register_module_override(
        "MobNexusContain",
        ModuleType::Behavior,
        mob_nexus_contain_module_factory,
        mob_nexus_contain_module_data_factory,
    )?;
    register_module_override(
        "RailedTransportContain",
        ModuleType::Behavior,
        railed_transport_contain_module_factory,
        railed_transport_contain_module_data_factory,
    )?;
    register_module_override(
        "RiderChangeContain",
        ModuleType::Behavior,
        rider_change_contain_module_factory,
        rider_change_contain_module_data_factory,
    )?;
    register_module_override(
        "InternetHackContain",
        ModuleType::Behavior,
        internet_hack_contain_module_factory,
        internet_hack_contain_module_data_factory,
    )?;
    register_module_override(
        "HealContain",
        ModuleType::Behavior,
        heal_contain_module_factory,
        heal_contain_module_data_factory,
    )?;
    register_module_override(
        "CaveContain",
        ModuleType::Behavior,
        cave_contain_module_factory,
        cave_contain_module_data_factory,
    )?;
    register_module_override(
        "W3DModelDraw",
        ModuleType::Draw,
        w3d_model_draw_module_factory,
        w3d_model_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DDefaultDraw",
        ModuleType::Draw,
        w3d_default_draw_module_factory,
        w3d_default_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DDependencyModelDraw",
        ModuleType::Draw,
        w3d_dependency_model_draw_module_factory,
        w3d_dependency_model_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DTankDraw",
        ModuleType::Draw,
        w3d_tank_draw_module_factory,
        w3d_tank_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DOverlordTankDraw",
        ModuleType::Draw,
        w3d_overlord_tank_draw_module_factory,
        w3d_overlord_tank_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DOverlordAircraftDraw",
        ModuleType::Draw,
        w3d_overlord_aircraft_draw_module_factory,
        w3d_overlord_aircraft_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DOverlordTruckDraw",
        ModuleType::Draw,
        w3d_overlord_truck_draw_module_factory,
        w3d_overlord_truck_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DPoliceCarDraw",
        ModuleType::Draw,
        w3d_police_car_draw_module_factory,
        w3d_police_car_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DProjectileStreamDraw",
        ModuleType::Draw,
        w3d_projectile_stream_draw_module_factory,
        w3d_projectile_stream_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DRopeDraw",
        ModuleType::Draw,
        w3d_rope_draw_module_factory,
        w3d_rope_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DScienceModelDraw",
        ModuleType::Draw,
        w3d_science_model_draw_module_factory,
        w3d_science_model_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DSupplyDraw",
        ModuleType::Draw,
        w3d_supply_draw_module_factory,
        w3d_supply_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DTankTruckDraw",
        ModuleType::Draw,
        w3d_tank_truck_draw_module_factory,
        w3d_tank_truck_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DTracerDraw",
        ModuleType::Draw,
        w3d_tracer_draw_module_factory,
        w3d_tracer_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DTreeDraw",
        ModuleType::Draw,
        w3d_tree_draw_module_factory,
        w3d_tree_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DTruckDraw",
        ModuleType::Draw,
        w3d_truck_draw_module_factory,
        w3d_truck_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DLaserDraw",
        ModuleType::Draw,
        w3d_laser_draw_module_factory,
        w3d_laser_draw_module_data_factory,
    )?;
    register_module_override(
        "W3DDebrisDraw",
        ModuleType::Draw,
        w3d_debris_draw_module_factory,
        w3d_debris_draw_module_data_factory,
    )?;
    register_module_override(
        "LaserUpdate",
        ModuleType::ClientUpdate,
        laser_update_module_factory,
        laser_update_module_data_factory,
    )?;
    register_module_override(
        "BeaconClientUpdate",
        ModuleType::ClientUpdate,
        beacon_client_update_module_factory,
        beacon_client_update_module_data_factory,
    )?;
    register_module_override(
        "SwayClientUpdate",
        ModuleType::ClientUpdate,
        sway_client_update_module_factory,
        base_client_update_module_data_factory,
    )?;
    register_module_override(
        "AnimatedParticleSysBoneClientUpdate",
        ModuleType::ClientUpdate,
        animated_particle_sys_bone_client_update_module_factory,
        base_client_update_module_data_factory,
    )?;
    register_module_override(
        "BaikonurLaunchPower",
        ModuleType::Behavior,
        baikonur_launch_power_module_factory,
        baikonur_launch_power_module_data_factory,
    )?;
    register_module_override(
        "CashBountyPower",
        ModuleType::Behavior,
        cash_bounty_power_module_factory,
        cash_bounty_power_module_data_factory,
    )?;
    register_module_override(
        "CashHackSpecialPower",
        ModuleType::Behavior,
        cash_hack_special_power_module_factory,
        cash_hack_special_power_module_data_factory,
    )?;
    register_module_override(
        "CleanupAreaPower",
        ModuleType::Behavior,
        cleanup_area_power_module_factory,
        cleanup_area_power_module_data_factory,
    )?;
    register_module_override(
        "FireWeaponPower",
        ModuleType::Behavior,
        fire_weapon_power_module_factory,
        fire_weapon_power_module_data_factory,
    )?;
    register_module_override(
        "OCLSpecialPower",
        ModuleType::Behavior,
        ocl_special_power_module_factory,
        ocl_special_power_module_data_factory,
    )?;
    register_module_override(
        "SpecialAbility",
        ModuleType::Behavior,
        special_ability_module_factory,
        special_ability_module_data_factory,
    )?;
    register_module_override(
        "SpyVisionSpecialPower",
        ModuleType::Behavior,
        spy_vision_special_power_module_factory,
        spy_vision_special_power_module_data_factory,
    )?;
    Ok(())
}

static CONTAIN_OVERRIDES_READY: OnceLock<Result<(), String>> = OnceLock::new();

pub fn ensure_module_overrides_installed() -> Result<(), String> {
    CONTAIN_OVERRIDES_READY
        .get_or_init(|| {
            install_contain_overrides()?;
            apply_module_overrides_to_existing_templates()?;
            Ok(())
        })
        .clone()
}
