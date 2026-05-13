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
    behavior::laser_update::{
        LaserUpdate as LaserBehaviorUpdate, LaserUpdateModule as LaserBehaviorUpdateModule,
        LaserUpdateModuleData as LaserBehaviorUpdateModuleData,
    },
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
    collide::fire_weapon_collide::{FireWeaponCollide, FireWeaponCollideModuleData},
    collide::crate_collide::shroud_crate_collide::{
        ShroudCrateCollide, ShroudCrateCollideModuleData,
    },
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

fn resolve_owner_info(thing: &Arc<dyn ModuleThing>) -> (ObjectID, Coord3D) {
    let owner_id = thing
        .as_object()
        .map(ModuleObjectTrait::get_object_id)
        .unwrap_or(INVALID_ID);

    let owner_pos = TheGameLogic::find_object_by_id(owner_id)
        .and_then(|obj| obj.read().ok().map(|guard| *guard.get_position()))
        .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

    (owner_id, owner_pos)
}

fn resolve_drawable_id(thing: &Arc<dyn ModuleThing>) -> u32 {
    thing
        .as_drawable()
        .map(ModuleDrawableTrait::get_drawable_id)
        .unwrap_or(INVALID_ID)
}

fn module_data_proc_or(
    module_name: &str,
    module_type: ModuleType,
    fallback: NewModuleDataProc,
) -> NewModuleDataProc {
    if let Ok(factory_guard) = get_module_factory() {
        if let Some(factory) = factory_guard.as_ref() {
            if let Some(template) = factory.find_module_template(module_name, module_type) {
                if let Some(create_data_proc) = template.create_data_proc {
                    return create_data_proc;
                }
            }
        }
    }
    fallback
}

fn attach_body_to_object(object_id: ObjectID, body: Arc<Mutex<dyn BodyModuleInterface>>) {
    if let Some(object) = TheGameLogic::find_object_by_id(object_id) {
        if let Ok(mut guard) = object.write() {
            guard.set_body_module(Some(body));
        }
    }
}

fn attach_contain_to_object(object_id: ObjectID, contain: Arc<Mutex<dyn ContainModuleInterface>>) {
    if let Some(object) = TheGameLogic::find_object_by_id(object_id) {
        if let Ok(mut guard) = object.write() {
            guard.set_contain(Some(contain));
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ContainModuleDataAdapter<T: Clone + Send + Sync + std::fmt::Debug + 'static> {
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

    pub(crate) fn contain_data(&self) -> &T {
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

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> crate::common::types::ModuleData
    for ContainModuleDataAdapter<T>
{
}

fn contain_data_ref<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
    module_data: &dyn ModuleData,
) -> Option<T> {
    module_data
        .downcast_ref::<ContainModuleDataAdapter<T>>()
        .map(ContainModuleDataAdapter::contain_data)
        .cloned()
}

fn expect_contain_data<T: Clone + Send + Sync + std::fmt::Debug + Default + 'static>(
    module_data: &dyn ModuleData,
    module_name: &str,
) -> T {
    contain_data_ref::<T>(module_data).unwrap_or_else(|| {
        warn!("{module_name} module data adapter missing; using default data");
        T::default()
    })
}

#[derive(Debug)]
pub(crate) struct ContainBindingModule {
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

#[derive(Debug)]
struct CaveContainBindingModule {
    module_name_key: NameKeyType,
    module_data: Arc<dyn ModuleData>,
    contain: Arc<Mutex<CaveContain>>,
    owner_id: ObjectID,
}

impl CaveContainBindingModule {
    fn new(
        module_name_key: NameKeyType,
        module_data: Arc<dyn ModuleData>,
        contain: Arc<Mutex<CaveContain>>,
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

impl CreateInterface for CaveContainBindingModule {
    fn on_create(&self) {
        let Some(cave_data) = contain_data_ref::<CaveContainModuleData>(self.module_data.as_ref())
        else {
            return;
        };
        if let Ok(mut contain_guard) = self.contain.lock() {
            let _ = contain_guard.on_create(cave_data);
        }
    }

    fn on_build_complete(&self) {
        if let Ok(mut contain_guard) = self.contain.lock() {
            let _ = contain_guard.on_build_complete();
        }
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.contain
            .lock()
            .map(|guard| guard.should_do_on_build_complete())
            .unwrap_or(false)
    }
}

impl Module for CaveContainBindingModule {
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
        let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::clone(&self.contain);
        attach_contain_to_object(self.owner_id, contain);
        if let Ok(mut contain_guard) = self.contain.lock() {
            if let Err(err) = contain_guard.on_owner_created() {
                warn!(
                    "Cave contain on_owner_created failed for object {}: {}",
                    self.owner_id, err
                );
            }
        }
    }

    fn get_create_interface(&self) -> Option<&dyn CreateInterface> {
        Some(self)
    }
}

impl Snapshotable for CaveContainBindingModule {
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

fn make_contain_binding_module(
    module_name: &str,
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
) -> Box<dyn Module> {
    let module_name_key = NameKeyGenerator::name_to_key(module_name);
    let (owner_id, _) = resolve_owner_info(&thing);
    Box::new(ContainBindingModule::new(
        module_name_key,
        module_data,
        contain,
        owner_id,
    ))
}

fn make_owner_weak(owner_id: ObjectID) -> Weak<RwLock<crate::object::Object>> {
    TheGameLogic::find_object_by_id(owner_id)
        .map(|arc| Arc::downgrade(&arc))
        .unwrap_or_else(Weak::new)
}

#[derive(Debug, Clone)]
struct InactiveBodyModuleData {
    base: BodyModuleData,
}

impl Default for InactiveBodyModuleData {
    fn default() -> Self {
        Self {
            base: BodyModuleData::default(),
        }
    }
}

crate::impl_legacy_module_data_via_base!(InactiveBodyModuleData, base);

impl Snapshotable for InactiveBodyModuleData {
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
struct InactiveBodyModule {
    module_name_key: NameKeyType,
    data: Arc<InactiveBodyModuleData>,
    body: Arc<Mutex<InactiveBody>>,
    owner_id: ObjectID,
}

impl InactiveBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<InactiveBodyModuleData>,
        owner_id: ObjectID,
    ) -> Self {
        let body = Arc::new(Mutex::new(InactiveBody::new_with_owner(
            data.base.clone(),
            owner_id,
        )));
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for InactiveBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for InactiveBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(body) = self.body.lock() {
            body.crc(xfer)
        } else {
            Ok(())
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(mut body) = self.body.lock() {
            body.xfer(xfer)
        } else {
            Ok(())
        }
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Ok(mut body) = self.body.lock() {
            body.load_post_process()
        } else {
            Ok(())
        }
    }
}

fn inactive_body_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(InactiveBodyModuleData::default())
}

fn inactive_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .downcast_ref::<InactiveBodyModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("InactiveBodyModuleData expected, using default fallback");
            InactiveBodyModuleData::default()
        });
    let module_data_arc = Arc::new(typed_data);
    let module_name_key = NameKeyGenerator::name_to_key("InactiveBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let module = InactiveBodyModule::new(module_name_key, module_data_arc, owner_id);

    Box::new(module)
}

#[derive(Debug)]
struct ActiveBodyModule {
    module_name_key: NameKeyType,
    data: Arc<ActiveBodyModuleData>,
    body: Arc<Mutex<ActiveBody>>,
    owner_id: ObjectID,
}

impl ActiveBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<ActiveBodyModuleData>,
        body: Arc<Mutex<ActiveBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for ActiveBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for ActiveBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self.body.lock().map_err(|_| "ActiveBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "ActiveBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "ActiveBody lock poisoned")?;
        body.load_post_process()
    }
}

fn active_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ActiveBodyModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ActiveBody module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn active_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ActiveBodyModuleData>()
        .expect("ActiveBodyModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("ActiveBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(ActiveBody::new_with_owner(
        typed_data.clone(),
        owner_id,
    )));
    Box::new(ActiveBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

#[derive(Debug)]
struct StructureBodyModule {
    module_name_key: NameKeyType,
    data: Arc<StructureBodyModuleData>,
    body: Arc<Mutex<StructureBody>>,
    owner_id: ObjectID,
}

impl StructureBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<StructureBodyModuleData>,
        body: Arc<Mutex<StructureBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for StructureBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for StructureBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self
            .body
            .lock()
            .map_err(|_| "StructureBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "StructureBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "StructureBody lock poisoned")?;
        body.load_post_process()
    }
}

fn structure_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StructureBodyModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StructureBody module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn structure_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StructureBodyModuleData>()
        .expect("StructureBodyModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("StructureBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(StructureBody::new(typed_data.clone(), owner_id)));
    Box::new(StructureBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

#[derive(Debug)]
struct HighlanderBodyModule {
    module_name_key: NameKeyType,
    data: Arc<ActiveBodyModuleData>,
    body: Arc<Mutex<HighlanderBody>>,
    owner_id: ObjectID,
}

impl HighlanderBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<ActiveBodyModuleData>,
        body: Arc<Mutex<HighlanderBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for HighlanderBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for HighlanderBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self
            .body
            .lock()
            .map_err(|_| "HighlanderBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "HighlanderBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "HighlanderBody lock poisoned")?;
        body.load_post_process()
    }
}

fn highlander_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    active_body_module_data_factory(ini)
}

fn highlander_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ActiveBodyModuleData>()
        .expect("ActiveBodyModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("HighlanderBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(HighlanderBody::new(
        typed_data.clone(),
        owner_id,
    )));
    Box::new(HighlanderBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

#[derive(Debug)]
struct ImmortalBodyModule {
    module_name_key: NameKeyType,
    data: Arc<ActiveBodyModuleData>,
    body: Arc<Mutex<ImmortalBody>>,
    owner_id: ObjectID,
}

impl ImmortalBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<ActiveBodyModuleData>,
        body: Arc<Mutex<ImmortalBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for ImmortalBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for ImmortalBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self.body.lock().map_err(|_| "ImmortalBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "ImmortalBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "ImmortalBody lock poisoned")?;
        body.load_post_process()
    }
}

fn immortal_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    active_body_module_data_factory(ini)
}

fn immortal_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ActiveBodyModuleData>()
        .expect("ActiveBodyModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("ImmortalBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(ImmortalBody::new(typed_data.clone(), owner_id)));
    Box::new(ImmortalBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

#[derive(Debug)]
struct HiveStructureBodyModule {
    module_name_key: NameKeyType,
    data: Arc<HiveStructureBodyModuleData>,
    body: Arc<Mutex<HiveStructureBody>>,
    owner_id: ObjectID,
}

impl HiveStructureBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<HiveStructureBodyModuleData>,
        body: Arc<Mutex<HiveStructureBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for HiveStructureBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for HiveStructureBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self
            .body
            .lock()
            .map_err(|_| "HiveStructureBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "HiveStructureBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "HiveStructureBody lock poisoned")?;
        body.load_post_process()
    }
}

fn hive_structure_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HiveStructureBodyModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HiveStructureBody module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn hive_structure_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<HiveStructureBodyModuleData>()
        .expect("HiveStructureBodyModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("HiveStructureBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(HiveStructureBody::new(
        typed_data.clone(),
        owner_id,
    )));
    Box::new(HiveStructureBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

#[derive(Debug)]
struct UndeadBodyModule {
    module_name_key: NameKeyType,
    data: Arc<UndeadBodyModuleData>,
    body: Arc<Mutex<UndeadBody>>,
    owner_id: ObjectID,
}

impl UndeadBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<UndeadBodyModuleData>,
        body: Arc<Mutex<UndeadBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for UndeadBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for UndeadBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self.body.lock().map_err(|_| "UndeadBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "UndeadBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "UndeadBody lock poisoned")?;
        body.load_post_process()
    }
}

fn undead_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = UndeadBodyModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse UndeadBody module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn undead_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<UndeadBodyModuleData>()
        .expect("UndeadBodyModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("UndeadBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(UndeadBody::new(typed_data.clone(), owner_id)));
    Box::new(UndeadBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

#[derive(Debug)]
struct LockWeaponCreateModule {
    module_name_key: NameKeyType,
    data: Arc<LockWeaponCreateModuleData>,
    create: LockWeaponCreate,
}

impl LockWeaponCreateModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<LockWeaponCreateModuleData>,
        create: LockWeaponCreate,
    ) -> Self {
        Self {
            module_name_key,
            data,
            create,
        }
    }
}

impl Module for LockWeaponCreateModule {
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

impl Snapshotable for LockWeaponCreateModule {
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

fn lock_weapon_create_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = LockWeaponCreateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse LockWeaponCreate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn lock_weapon_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<LockWeaponCreateModuleData>()
        .expect("LockWeaponCreateModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("LockWeaponCreate");
    let create = LockWeaponCreate::new(thing, Arc::clone(&data_arc));
    Box::new(LockWeaponCreateModule::new(
        module_name_key,
        data_arc,
        create,
    ))
}

#[derive(Debug)]
struct GrantUpgradeCreateModule {
    module_name_key: NameKeyType,
    data: Arc<GrantUpgradeCreateModuleData>,
    create: GrantUpgradeCreate,
}

impl GrantUpgradeCreateModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<GrantUpgradeCreateModuleData>,
        create: GrantUpgradeCreate,
    ) -> Self {
        Self {
            module_name_key,
            data,
            create,
        }
    }
}

impl Module for GrantUpgradeCreateModule {
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

impl Snapshotable for GrantUpgradeCreateModule {
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

fn grant_upgrade_create_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = GrantUpgradeCreateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse GrantUpgradeCreate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn grant_upgrade_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<GrantUpgradeCreateModuleData>()
        .expect("GrantUpgradeCreateModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("GrantUpgradeCreate");
    let create = GrantUpgradeCreate::new(thing, Arc::clone(&data_arc));
    Box::new(GrantUpgradeCreateModule::new(
        module_name_key,
        data_arc,
        create,
    ))
}

#[derive(Debug)]
struct VeterancyGainCreateModule {
    module_name_key: NameKeyType,
    data: Arc<VeterancyGainCreateModuleData>,
    create: VeterancyGainCreate,
}

impl VeterancyGainCreateModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<VeterancyGainCreateModuleData>,
        create: VeterancyGainCreate,
    ) -> Self {
        Self {
            module_name_key,
            data,
            create,
        }
    }
}

impl Module for VeterancyGainCreateModule {
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

impl Snapshotable for VeterancyGainCreateModule {
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

fn veterancy_gain_create_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = VeterancyGainCreateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse VeterancyGainCreate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn veterancy_gain_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<VeterancyGainCreateModuleData>()
        .expect("VeterancyGainCreateModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("VeterancyGainCreate");
    let create = VeterancyGainCreate::new(thing, Arc::clone(&data_arc));
    Box::new(VeterancyGainCreateModule::new(
        module_name_key,
        data_arc,
        create,
    ))
}

#[derive(Debug)]
struct SimpleCreateModule<T> {
    module_name_key: NameKeyType,
    data: Arc<CreateModuleData>,
    create: T,
}

impl<T> SimpleCreateModule<T> {
    fn new(module_name_key: NameKeyType, data: Arc<CreateModuleData>, create: T) -> Self {
        Self {
            module_name_key,
            data,
            create,
        }
    }
}

impl<T> Module for SimpleCreateModule<T>
where
    T: CreateInterface + Snapshotable + 'static,
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

    fn get_create_interface(&self) -> Option<&dyn CreateInterface> {
        Some(&self.create)
    }
}

impl<T> Snapshotable for SimpleCreateModule<T>
where
    T: CreateInterface + Snapshotable + 'static,
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

fn simple_create_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(CreateModuleData::default())
}

fn preorder_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CreateModuleData>()
        .expect("CreateModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("PreorderCreate");
    let create = PreorderCreate::new(thing);
    Box::new(SimpleCreateModule::new(module_name_key, data_arc, create))
}

fn special_power_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CreateModuleData>()
        .expect("CreateModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("SpecialPowerCreate");
    let create = SpecialPowerCreate::new(thing);
    Box::new(SimpleCreateModule::new(module_name_key, data_arc, create))
}

fn supply_center_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CreateModuleData>()
        .expect("CreateModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("SupplyCenterCreate");
    let create = SupplyCenterCreate::new(thing);
    Box::new(SimpleCreateModule::new(module_name_key, data_arc, create))
}

fn supply_warehouse_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CreateModuleData>()
        .expect("CreateModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("SupplyWarehouseCreate");
    let create = SupplyWarehouseCreate::new(thing);
    Box::new(SimpleCreateModule::new(module_name_key, data_arc, create))
}

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
        loc: &Coord3D,
        normal: &Coord3D,
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

#[derive(Debug)]
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

fn fire_weapon_collide_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireWeaponCollideModuleData>()
        .expect("FireWeaponCollideModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();

    let collide = FireWeaponCollide::new(object_id, module_data_arc.clone())
        .expect("FireWeaponCollide::new should not fail during module construction");
    let module_name_key = NameKeyGenerator::name_to_key("FireWeaponCollide");
    Box::new(FireWeaponCollideModule::new(
        module_name_key,
        module_data_arc,
        collide,
        object_id,
    ))
}

#[derive(Debug)]
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

    fn get_interface_mask(&self) -> ModuleInterfaceType {
        ModuleInterfaceType::COLLIDE
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

fn squish_collide_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(SquishCollideModuleData::default())
}

fn squish_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SquishCollideModuleData>()
        .expect("SquishCollideModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();

    let collide = SquishCollide::new(object_id, Arc::clone(&module_data_arc));
    let module_name_key = NameKeyGenerator::name_to_key("SquishCollide");
    Box::new(SquishCollideModule::new(
        module_name_key,
        module_data_arc,
        collide,
        object_id,
    ))
}

#[derive(Debug)]
struct ShroudCrateCollideModule {
    module_name_key: NameKeyType,
    data: Arc<ShroudCrateCollideModuleData>,
    collide: Arc<Mutex<ShroudCrateCollide>>,
    object_id: ObjectID,
}

impl ShroudCrateCollideModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<ShroudCrateCollideModuleData>,
        collide: ShroudCrateCollide,
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

impl Module for ShroudCrateCollideModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn get_interface_mask(&self) -> ModuleInterfaceType {
        ModuleInterfaceType::COLLIDE
    }

    fn on_object_created(&mut self) {
        if let Err(err) = COLLISION_MANAGER.register_collide_module(
            self.object_id,
            Box::new(SharedCollideModule::new(Arc::clone(&self.collide))),
        ) {
            warn!(
                "Failed to register ShroudCrateCollide module for object {}: {}",
                self.object_id, err
            );
        }
    }

    fn on_delete(&mut self) {
        let _ = COLLISION_MANAGER.unregister_object(self.object_id);
    }
}

impl Snapshotable for ShroudCrateCollideModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let collide = self
            .collide
            .lock()
            .map_err(|_| "ShroudCrateCollide lock poisoned".to_string())?;
        collide.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "ShroudCrateCollide lock poisoned".to_string())?;
        collide.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "ShroudCrateCollide lock poisoned".to_string())?;
        collide.load_post_process()
    }
}

fn shroud_crate_collide_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ShroudCrateCollideModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ShroudCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn shroud_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let config = module_data
        .get_shroud_crate_collide_config()
        .expect("ShroudCrateCollideModuleData expected");
    let module_data_arc = Arc::new(ShroudCrateCollideModuleData::from_config(
        config,
        module_data.get_module_tag_name_key(),
    ));
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();

    let collide = ShroudCrateCollide::new(object_id, module_data_arc.crate_data());
    let module_name_key = NameKeyGenerator::name_to_key("ShroudCrateCollide");
    Box::new(ShroudCrateCollideModule::new(
        module_name_key,
        module_data_arc,
        collide,
        object_id,
    ))
}

fn upgrade_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = UpgradeDieModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse UpgradeDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn upgrade_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<UpgradeDieModuleData>()
        .expect("UpgradeDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("UpgradeDie requires owning object");

    let die_module = UpgradeDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("UpgradeDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DieModuleData at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn destroy_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DieModuleData>()
        .expect("DieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("DestroyDie requires owning object");

    let die_module = DestroyDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("DestroyDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn keep_object_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DieModuleData>()
        .expect("DieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("KeepObjectDie requires owning object");

    let die_module = KeepObjectDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("KeepObjectDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn create_object_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CreateObjectDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CreateObjectDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn create_object_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CreateObjectDieModuleData>()
        .expect("CreateObjectDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("CreateObjectDie requires owning object");

    let die_module = CreateObjectDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("CreateObjectDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn create_crate_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CreateCrateDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CreateCrateDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn create_crate_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CreateCrateDieModuleData>()
        .expect("CreateCrateDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("CreateCrateDie requires owning object");

    let die_module = CreateCrateDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("CreateCrateDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn fx_list_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FXListDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FXListDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn fx_list_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FXListDieModuleData>()
        .expect("FXListDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("FXListDie requires owning object");

    let die_module = FXListDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("FXListDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn crush_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CrushDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CrushDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn crush_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CrushDieModuleData>()
        .expect("CrushDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("CrushDie requires owning object");

    let die_module = CrushDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("CrushDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn eject_pilot_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = EjectPilotDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse EjectPilotDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn eject_pilot_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<EjectPilotDieModuleData>()
        .expect("EjectPilotDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("EjectPilotDie requires owning object");

    let die_module = EjectPilotDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("EjectPilotDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn rebuild_hole_expose_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RebuildHoleExposeDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RebuildHoleExposeDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn rebuild_hole_expose_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RebuildHoleExposeDieModuleData>()
        .expect("RebuildHoleExposeDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id)
        .expect("RebuildHoleExposeDie requires owning object");

    let die_module = RebuildHoleExposeDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("RebuildHoleExposeDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn special_power_completion_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpecialPowerCompletionDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpecialPowerCompletionDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn special_power_completion_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpecialPowerCompletionDieModuleData>()
        .expect("SpecialPowerCompletionDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id)
        .expect("SpecialPowerCompletionDie requires owning object");

    let die_module = SpecialPowerCompletionDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("SpecialPowerCompletionDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn demoralize_special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DemoralizeSpecialPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DemoralizeSpecialPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn demoralize_special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DemoralizeSpecialPowerModuleData>()
        .expect("DemoralizeSpecialPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("DemoralizeSpecialPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(DemoralizeSpecialPower::new(
        module_name_key,
        owner_id,
        data_arc,
    ))
}

fn cash_hack_special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CashHackSpecialPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CashHackSpecialPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn cash_hack_special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CashHackSpecialPowerModuleData>()
        .expect("CashHackSpecialPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("CashHackSpecialPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(CashHackSpecialPower::new(
        module_name_key,
        owner_id,
        data_arc,
    ))
}

fn spy_vision_special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpyVisionSpecialPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpyVisionSpecialPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn spy_vision_special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpyVisionSpecialPowerModuleData>()
        .expect("SpyVisionSpecialPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("SpyVisionSpecialPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(SpyVisionSpecialPower::new(
        module_name_key,
        owner_id,
        data_arc,
    ))
}

fn defector_special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DefectorSpecialPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DefectorSpecialPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn defector_special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DefectorSpecialPowerModuleData>()
        .expect("DefectorSpecialPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("DefectorSpecialPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(DefectorSpecialPower::new(
        module_name_key,
        owner_id,
        data_arc,
    ))
}

fn cash_bounty_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CashBountyPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CashBountyPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn cash_bounty_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CashBountyPowerModuleData>()
        .expect("CashBountyPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("CashBountyPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(CashBountyPower::new(module_name_key, owner_id, data_arc))
}

fn cleanup_area_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CleanupAreaPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CleanupAreaPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn cleanup_area_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CleanupAreaPowerModuleData>()
        .expect("CleanupAreaPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("CleanupAreaPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(CleanupAreaPower::new(module_name_key, owner_id, data_arc))
}

fn fire_weapon_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FireWeaponPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireWeaponPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn fire_weapon_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireWeaponPowerModuleData>()
        .expect("FireWeaponPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("FireWeaponPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(FireWeaponPower::new(module_name_key, owner_id, data_arc))
}

fn special_ability_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpecialAbilityModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpecialAbility module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn special_ability_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpecialAbilityModuleData>()
        .expect("SpecialAbilityModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("SpecialAbility");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(SpecialAbility::new(module_name_key, owner_id, data_arc))
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BaikonurLaunchPowerModuleData>()
        .expect("BaikonurLaunchPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("BaikonurLaunchPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(BaikonurLaunchPower::new(
        module_name_key,
        owner_id,
        data_arc,
    ))
}

fn ocl_special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = OclSpecialPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse OCLSpecialPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn ocl_special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<OclSpecialPowerModuleData>()
        .expect("OclSpecialPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("OCLSpecialPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(OclSpecialPower::new(module_name_key, owner_id, data_arc))
}

fn dam_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DamDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DamDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn dam_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DamDieModuleData>()
        .expect("DamDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id).expect("DamDie requires owning object");

    let die_module = DamDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("DamDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn status_bits_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StatusBitsUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StatusBitsUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn status_bits_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StatusBitsUpgradeModuleData>()
        .expect("StatusBitsUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("StatusBitsUpgrade");
    Box::new(StatusBitsUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn passengers_fire_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PassengersFireUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PassengersFireUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn passengers_fire_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PassengersFireUpgradeModuleData>()
        .expect("PassengersFireUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("PassengersFireUpgrade");

    Box::new(PassengersFireUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn subobjects_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SubObjectsUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SubObjectsUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn subobjects_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SubObjectsUpgradeModuleData>()
        .expect("SubObjectsUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("SubObjectsUpgrade");

    Box::new(SubObjectsUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn grant_science_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = GrantScienceUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse GrantScienceUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn grant_science_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<GrantScienceUpgradeModuleData>()
        .expect("GrantScienceUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("GrantScienceUpgrade");

    Box::new(GrantScienceUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn object_creation_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ObjectCreationUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ObjectCreationUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn object_creation_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ObjectCreationUpgradeModuleData>()
        .expect("ObjectCreationUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ObjectCreationUpgrade");

    Box::new(ObjectCreationUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn active_shroud_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ActiveShroudUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ActiveShroudUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn active_shroud_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ActiveShroudUpgrade");

    Box::new(
        ActiveShroudUpgrade::from_module_data(module_name_key, module_data, object_id)
            .expect("ActiveShroudUpgradeModuleData expected"),
    )
}

fn armor_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ArmorUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ArmorUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn armor_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ArmorUpgradeModuleData>()
        .expect("ArmorUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ArmorUpgrade");

    Box::new(ArmorUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn command_set_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CommandSetUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CommandSetUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn command_set_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CommandSetUpgradeModuleData>()
        .expect("CommandSetUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("CommandSetUpgrade");

    Box::new(CommandSetUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn cost_modifier_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CostModifierUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CostModifierUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn cost_modifier_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CostModifierUpgradeModuleData>()
        .expect("CostModifierUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("CostModifierUpgrade");

    Box::new(CostModifierUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn experience_scalar_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ExperienceScalarUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ExperienceScalarUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn experience_scalar_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ExperienceScalarUpgradeModuleData>()
        .expect("ExperienceScalarUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ExperienceScalarUpgrade");

    Box::new(ExperienceScalarUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn locomotor_set_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = LocomotorSetUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse LocomotorSetUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn locomotor_set_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<LocomotorSetUpgradeModuleData>()
        .expect("LocomotorSetUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("LocomotorSetUpgrade");

    Box::new(LocomotorSetUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn max_health_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MaxHealthUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MaxHealthUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn max_health_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<MaxHealthUpgradeModuleData>()
        .expect("MaxHealthUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("MaxHealthUpgrade");

    Box::new(MaxHealthUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn model_condition_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ModelConditionUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ModelConditionUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn model_condition_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ModelConditionUpgradeModuleData>()
        .expect("ModelConditionUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ModelConditionUpgrade");

    Box::new(ModelConditionUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn power_plant_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PowerPlantUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PowerPlantUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn power_plant_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PowerPlantUpgradeModuleData>()
        .expect("PowerPlantUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("PowerPlantUpgrade");

    Box::new(PowerPlantUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn radar_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RadarUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RadarUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn radar_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("RadarUpgrade");

    Box::new(
        RadarUpgrade::from_module_data(module_name_key, module_data, object_id)
            .expect("RadarUpgradeModuleData expected"),
    )
}

fn replace_object_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ReplaceObjectUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ReplaceObjectUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn replace_object_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ReplaceObjectUpgradeModuleData>()
        .expect("ReplaceObjectUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ReplaceObjectUpgrade");

    Box::new(ReplaceObjectUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn stealth_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StealthUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StealthUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn stealth_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StealthUpgradeModuleData>()
        .expect("StealthUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("StealthUpgrade");

    Box::new(StealthUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn unpause_special_power_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = UnpauseSpecialPowerUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse UnpauseSpecialPowerUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn unpause_special_power_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<UnpauseSpecialPowerUpgradeModuleData>()
        .expect("UnpauseSpecialPowerUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("UnpauseSpecialPowerUpgrade");

    Box::new(UnpauseSpecialPowerUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn weapon_bonus_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = WeaponBonusUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse WeaponBonusUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn weapon_bonus_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WeaponBonusUpgradeModuleData>()
        .expect("WeaponBonusUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("WeaponBonusUpgrade");

    Box::new(WeaponBonusUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn weapon_set_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = WeaponSetUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse WeaponSetUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn weapon_set_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WeaponSetUpgradeModuleData>()
        .expect("WeaponSetUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("WeaponSetUpgrade");

    Box::new(WeaponSetUpgrade::new(
        module_name_key,
        module_data_arc,
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<TransitionDamageFXModuleData>()
        .expect("TransitionDamageFXModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        TransitionDamageFX::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("TransitionDamageFX requires an owning object");

    let module_name = AsciiString::from("TransitionDamageFX");
    Box::new(TransitionDamageFXModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn stealth_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StealthUpdateModuleData::default();

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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StealthUpdateModuleData>()
        .expect("StealthUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();

    let module_name_key = NameKeyGenerator::name_to_key("StealthUpdate");
    Box::new(StealthUpdateModule::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn ai_update_interface_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AIUpdateModuleData>()
        .expect("AIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("AIUpdateInterface");
    Box::new(AIUpdateInterfaceModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn railed_transport_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RailedTransportAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RailedTransportAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn railed_transport_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RailedTransportAIUpdateModuleData>()
        .expect("RailedTransportAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("RailedTransportAIUpdate");
    Box::new(RailedTransportAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn railroad_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RailroadBehaviorModuleData>()
        .expect("RailroadBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("RailroadBehavior");
    let object_id = thing
        .as_object()
        .map(|obj| obj.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id)
        .expect("RailroadBehavior requires valid object handle");
    Box::new(
        RailroadBehaviorModule::new(module_name_key, module_data_arc, object)
            .expect("Failed to create RailroadBehaviorModule"),
    )
}

fn special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpecialPowerModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpecialPowerModule data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpecialPowerModuleData>()
        .expect("SpecialPowerModuleData expected");

    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    let module = SpecialPowerModule::new(owner_id, typed_data.clone());
    Box::new(module)
}

fn production_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ProductionUpdateCompleteModuleData::default();

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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ProductionUpdateCompleteModuleData>()
        .expect("ProductionUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name = AsciiString::from("ProductionUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    Box::new(ProductionUpdateCompleteModule::new(
        &module_name,
        module_data_arc,
        owner_id,
    ))
}

fn assault_transport_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AssaultTransportAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AssaultTransportAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn transport_ai_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(TransportAIUpdateModuleData::default())
}

fn transport_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<TransportAIUpdateModuleData>()
        .expect("TransportAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("TransportAIUpdate");
    Box::new(TransportAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn assault_transport_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AssaultTransportAIUpdateModuleData>()
        .expect("AssaultTransportAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("AssaultTransportAIUpdate");
    Box::new(AssaultTransportAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn deliver_payload_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DeliverPayloadAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DeliverPayloadAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn deploy_style_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DeployStyleAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DeployStyleAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn deploy_style_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DeployStyleAIUpdateModuleData>()
        .expect("DeployStyleAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("DeployStyleAIUpdate");
    Box::new(DeployStyleAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn wander_ai_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(WanderAIUpdateModuleData::default())
}

fn wander_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WanderAIUpdateModuleData>()
        .expect("WanderAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("WanderAIUpdate");
    Box::new(WanderAIUpdateModule::new(module_name_key, module_data_arc))
}

fn deliver_payload_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DeliverPayloadAIUpdateModuleData>()
        .expect("DeliverPayloadAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("DeliverPayloadAIUpdate");
    Box::new(DeliverPayloadAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn hack_internet_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HackInternetAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HackInternetAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn hack_internet_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<HackInternetAIUpdateModuleData>()
        .expect("HackInternetAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("HackInternetAIUpdate");
    Box::new(HackInternetAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn supply_truck_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SupplyTruckAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SupplyTruckAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn supply_truck_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SupplyTruckAIUpdateModuleData>()
        .expect("SupplyTruckAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("SupplyTruckAIUpdate");
    Box::new(SupplyTruckAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn chinook_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ChinookAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ChinookAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn chinook_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ChinookAIUpdateModuleData>()
        .expect("ChinookAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("ChinookAIUpdate");
    Box::new(ChinookAIUpdateModule::new(module_name_key, module_data_arc))
}

fn jet_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = JetAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse JetAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn jet_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<JetAIUpdateModuleData>()
        .expect("JetAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("JetAIUpdate");
    Box::new(JetAIUpdateModule::new(module_name_key, module_data_arc))
}

fn worker_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = WorkerAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse WorkerAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn worker_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WorkerAIUpdateModuleData>()
        .expect("WorkerAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("WorkerAIUpdate");
    Box::new(WorkerAIUpdateModule::new(module_name_key, module_data_arc))
}

fn dozer_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DozerAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DozerAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn dozer_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DozerAIUpdateModuleData>()
        .expect("DozerAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("DozerAIUpdate");
    Box::new(DozerAIUpdateModule::new(module_name_key, module_data_arc))
}

#[cfg(feature = "allow_surrender")]
fn pow_truck_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = POWTruckAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse POWTruckAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

#[cfg(feature = "allow_surrender")]
fn pow_truck_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<POWTruckAIUpdateModuleData>()
        .expect("POWTruckAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("POWTruckAIUpdate");
    Box::new(POWTruckAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn auto_heal_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AutoHealBehaviorModuleData>()
        .expect("AutoHealBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = AutoHealBehavior::from_module_thing(thing, module_data_arc.clone());

    let module_name = AsciiString::from("AutoHealBehavior");
    Box::new(AutoHealBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn horde_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HordeUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HordeUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn horde_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<HordeUpdateModuleData>()
        .expect("HordeUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("HordeUpdate requires a valid object");
    let behavior = HordeUpdate::new_from_object_handle(object, module_data_arc.clone());

    let module_name = AsciiString::from("HordeUpdate");
    Box::new(HordeUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn spawn_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpawnBehaviorModuleData>()
        .expect("SpawnBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("SpawnBehavior requires a valid object");
    let behavior = SpawnBehavior::new(object, module_data_arc.clone())
        .expect("SpawnBehavior failed to initialize");

    let module_name = AsciiString::from("SpawnBehavior");
    Box::new(SpawnBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

// ============================================================================
// Generic Behavior Module Wrapper
// ============================================================================

#[derive(Debug)]
struct GenericBehaviorModule<T: crate::modules::BehaviorModuleInterface + 'static> {
    module_name_key: NameKeyType,
    data: Arc<dyn ModuleData>,
    behavior: T,
}

impl<T: crate::modules::BehaviorModuleInterface + 'static> GenericBehaviorModule<T> {
    fn new(module_name: &str, data: Arc<dyn ModuleData>, behavior: T) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name);
        Self {
            module_name_key,
            data,
            behavior,
        }
    }
}

impl<T: crate::modules::BehaviorModuleInterface + 'static> Module for GenericBehaviorModule<T> {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_deletion_lifetime_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::DeletionLifetimeInterface> {
        self.behavior.get_deletion_lifetime_interface()
    }

    fn get_bone_fx_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::BoneFxControlInterface> {
        self.behavior.get_bone_fx_control_interface()
    }

    fn get_prone_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::ProneControlInterface> {
        self.behavior.get_prone_control_interface()
    }

    fn get_sticky_bomb_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::StickyBombControlInterface> {
        self.behavior.get_sticky_bomb_control_interface()
    }
}

impl<T: crate::modules::BehaviorModuleInterface + Snapshotable + 'static> Snapshotable
    for GenericBehaviorModule<T>
{
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

// ============================================================================
// SlowDeathBehavior Factory Functions
// ============================================================================

fn slow_death_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SlowDeathBehaviorModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SlowDeathBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
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
        .downcast_ref::<SlowDeathBehaviorModuleData>()
        .expect("SlowDeathBehaviorModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("SlowDeathBehavior requires a valid object");
    let behavior = SlowDeathBehavior::new(object, module_data_arc.clone())
        .expect("SlowDeathBehavior failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "SlowDeathBehavior",
        module_data_arc,
        behavior,
    ))
}

// ============================================================================
// MinefieldBehavior Factory Functions
// ============================================================================

fn minefield_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<MinefieldBehaviorModuleData>()
        .expect("MinefieldBehaviorModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("MinefieldBehavior requires a valid object");
    let behavior = MinefieldBehaviorFactory::create_behavior(object, module_data_arc.clone())
        .expect("MinefieldBehavior failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "MinefieldBehavior",
        module_data_arc,
        behavior,
    ))
}

// ============================================================================
// GrantStealthBehavior Factory Functions
// ============================================================================

fn grant_stealth_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<GrantStealthBehaviorModuleData>()
        .expect("GrantStealthBehaviorModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("GrantStealthBehavior requires a valid object");
    let behavior = GrantStealthBehaviorFactory::create_behavior(object, module_data_arc.clone())
        .expect("GrantStealthBehavior failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "GrantStealthBehavior",
        module_data_arc,
        behavior,
    ))
}

// ============================================================================
// PhysicsUpdate Factory Functions
// ============================================================================

fn physics_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PhysicsBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PhysicsUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn physics_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PhysicsBehaviorModuleData>()
        .expect("PhysicsBehaviorModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("PhysicsUpdate requires a valid object");
    let behavior = PhysicsBehaviorFactory::create_behavior(object, module_data_arc.clone())
        .expect("PhysicsUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "PhysicsUpdate",
        module_data_arc,
        behavior,
    ))
}

// ============================================================================
// Additional Update Module Factory Functions
// ============================================================================

macro_rules! simple_behavior_factory {
    ($name:ident, $data_type:ty, $factory:ty, $module_name:expr) => {
        fn $name($($arg:ident: $arg_ty:ty),*) -> Box<dyn Module> {
            // Implementation
        }
    };
}

fn height_die_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HeightDieUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HeightDieUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn height_die_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<HeightDieUpdateModuleData>()
        .expect("HeightDieUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("HeightDieUpdate requires a valid object");
    let behavior = HeightDieUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("HeightDieUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "HeightDieUpdate",
        module_data_arc,
        behavior,
    ))
}

fn deletion_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DeletionUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DeletionUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn deletion_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DeletionUpdateModuleData>()
        .expect("DeletionUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("DeletionUpdate requires a valid object");
    let behavior = DeletionUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("DeletionUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "DeletionUpdate",
        module_data_arc,
        behavior,
    ))
}

fn wave_guide_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = WaveGuideUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse WaveGuideUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn wave_guide_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WaveGuideUpdateModuleData>()
        .expect("WaveGuideUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("WaveGuideUpdate requires a valid object");
    let behavior = WaveGuideUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("WaveGuideUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "WaveGuideUpdate",
        module_data_arc,
        behavior,
    ))
}

fn checkpoint_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CheckpointUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CheckpointUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn checkpoint_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CheckpointUpdateModuleData>()
        .expect("CheckpointUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("CheckpointUpdate requires a valid object");
    let behavior = CheckpointUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("CheckpointUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "CheckpointUpdate",
        module_data_arc,
        behavior,
    ))
}

fn animation_steering_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AnimationSteeringUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AnimationSteeringUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn animation_steering_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AnimationSteeringUpdateModuleData>()
        .expect("AnimationSteeringUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("AnimationSteeringUpdate requires a valid object");
    let behavior = AnimationSteeringUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("AnimationSteeringUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "AnimationSteeringUpdate",
        module_data_arc,
        behavior,
    ))
}

fn pilot_find_vehicle_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PilotFindVehicleUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PilotFindVehicleUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn pilot_find_vehicle_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PilotFindVehicleUpdateModuleData>()
        .expect("PilotFindVehicleUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("PilotFindVehicleUpdate requires a valid object");
    let behavior = PilotFindVehicleUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("PilotFindVehicleUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "PilotFindVehicleUpdate",
        module_data_arc,
        behavior,
    ))
}

fn hijacker_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HijackerUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HijackerUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn hijacker_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<HijackerUpdateModuleData>()
        .expect("HijackerUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("HijackerUpdate requires a valid object");
    let behavior = HijackerUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("HijackerUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "HijackerUpdate",
        module_data_arc,
        behavior,
    ))
}

fn helicopter_slow_death_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = HelicopterSlowDeathBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HelicopterSlowDeathBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn helicopter_slow_death_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<HelicopterSlowDeathBehaviorModuleData>()
        .expect("HelicopterSlowDeathBehaviorModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("HelicopterSlowDeathBehavior requires a valid object");
    let behavior =
        HelicopterSlowDeathBehaviorFactory::create_behavior(object, module_data_arc.clone())
            .expect("HelicopterSlowDeathBehavior failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "HelicopterSlowDeathBehavior",
        module_data_arc,
        behavior,
    ))
}

fn neutron_missile_slow_death_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = NeutronMissileSlowDeathUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse NeutronMissileSlowDeathUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn neutron_missile_slow_death_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<NeutronMissileSlowDeathUpdateModuleData>()
        .expect("NeutronMissileSlowDeathUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("NeutronMissileSlowDeathUpdate requires a valid object");
    let behavior =
        NeutronMissileSlowDeathUpdateFactory::create_behavior(object, module_data_arc.clone())
            .expect("NeutronMissileSlowDeathUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "NeutronMissileSlowDeathUpdate",
        module_data_arc,
        behavior,
    ))
}

fn neutron_missile_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = NeutronMissileUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse NeutronMissileUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn neutron_missile_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<NeutronMissileUpdateModuleData>()
        .expect("NeutronMissileUpdateModuleData expected");
    let (owner_id, _) = resolve_owner_info(&thing);
    Box::new(NeutronMissileUpdate::new(
        owner_id,
        typed_data.clone(),
        &AsciiString::from("NeutronMissileUpdate"),
    ))
}

fn firestorm_dynamic_geometry_info_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = FirestormDynamicGeometryInfoUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FirestormDynamicGeometryInfoUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn firestorm_dynamic_geometry_info_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FirestormDynamicGeometryInfoUpdateModuleData>()
        .expect("FirestormDynamicGeometryInfoUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FirestormDynamicGeometryInfoUpdate requires a valid object");
    let behavior =
        FirestormDynamicGeometryInfoUpdateFactory::create_behavior(object, module_data_arc.clone())
            .expect("FirestormDynamicGeometryInfoUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "FirestormDynamicGeometryInfoUpdate",
        module_data_arc,
        behavior,
    ))
}

fn dynamic_geometry_info_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DynamicGeometryInfoUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DynamicGeometryInfoUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn dynamic_geometry_info_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DynamicGeometryInfoUpdateModuleData>()
        .expect("DynamicGeometryInfoUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("DynamicGeometryInfoUpdate requires a valid object");
    let behavior =
        DynamicGeometryInfoUpdateFactory::create_behavior(object, module_data_arc.clone())
            .expect("DynamicGeometryInfoUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "DynamicGeometryInfoUpdate",
        module_data_arc,
        behavior,
    ))
}

fn radar_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RadarUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RadarUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn radar_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("RadarUpdate requires a valid object");

    let module_name = AsciiString::from("RadarUpdate");
    Box::new(
        RadarUpdateModule::from_module_data(object, &module_name, module_data)
            .expect("RadarUpdateModuleData expected"),
    )
}

fn stealth_detector_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StealthDetectorUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StealthDetectorUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn stealth_detector_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StealthDetectorUpdateModuleData>()
        .expect("StealthDetectorUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("StealthDetectorUpdate requires a valid object");
    let behavior = StealthDetectorUpdate::new(object, module_data_arc.clone())
        .expect("StealthDetectorUpdate failed to initialize");

    let module_name = AsciiString::from("StealthDetectorUpdate");
    Box::new(StealthDetectorUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn radius_decal_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(RadiusDecalUpdateModuleData::default())
}

fn radius_decal_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RadiusDecalUpdateModuleData>()
        .expect("RadiusDecalUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("RadiusDecalUpdate requires a valid object");
    let behavior = RadiusDecalUpdate::new(object, module_data_arc.clone())
        .expect("RadiusDecalUpdate failed to initialize");

    let module_name = AsciiString::from("RadiusDecalUpdate");
    Box::new(RadiusDecalUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn sticky_bomb_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StickyBombUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StickyBombUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn sticky_bomb_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StickyBombUpdateModuleData>()
        .expect("StickyBombUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("StickyBombUpdate requires a valid object");
    let behavior = StickyBombUpdate::new(object, module_data_arc.clone())
        .expect("StickyBombUpdate failed to initialize");

    let module_name = AsciiString::from("StickyBombUpdate");
    Box::new(StickyBombUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn prone_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ProneUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ProneUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn prone_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ProneUpdateModuleData>()
        .expect("ProneUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("ProneUpdate requires a valid object");
    let behavior = ProneUpdate::new(object, module_data_arc.clone())
        .expect("ProneUpdate failed to initialize");

    let module_name = AsciiString::from("ProneUpdate");
    Box::new(ProneUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn projectile_stream_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ProjectileStreamUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ProjectileStreamUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn projectile_stream_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ProjectileStreamUpdateModuleData>()
        .expect("ProjectileStreamUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("ProjectileStreamUpdate requires a valid object");
    let behavior = ProjectileStreamUpdate::new(object, module_data_arc.clone())
        .expect("ProjectileStreamUpdate failed to initialize");

    let module_name = AsciiString::from("ProjectileStreamUpdate");
    Box::new(ProjectileStreamUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn point_defense_laser_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PointDefenseLaserUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PointDefenseLaserUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn point_defense_laser_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PointDefenseLaserUpdateModuleData>()
        .expect("PointDefenseLaserUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("PointDefenseLaserUpdate requires a valid object");
    let behavior = PointDefenseLaserUpdate::new(object, module_data_arc.clone())
        .expect("PointDefenseLaserUpdate failed to initialize");

    let module_name = AsciiString::from("PointDefenseLaserUpdate");
    Box::new(PointDefenseLaserUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn laser_behavior_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = LaserBehaviorUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse LaserUpdate (behavior) module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn laser_behavior_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<LaserBehaviorUpdateModuleData>()
        .expect("LaserBehaviorUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("LaserUpdate (behavior) requires a valid object");
    let behavior = LaserBehaviorUpdate::new(object, module_data_arc.clone())
        .expect("LaserUpdate (behavior) failed to initialize");

    let module_name = AsciiString::from("LaserUpdate");
    Box::new(LaserBehaviorUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn bone_fx_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BoneFXUpdateModuleData>()
        .expect("BoneFXUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let behavior = BoneFXUpdate::new(owner_id, module_data_arc.clone());

    let module_name = AsciiString::from("BoneFXUpdate");
    Box::new(BoneFXUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn demo_trap_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DemoTrapUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DemoTrapUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn demo_trap_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DemoTrapUpdateModuleData>()
        .expect("DemoTrapUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("DemoTrapUpdate requires a valid object");
    let behavior = DemoTrapUpdate::new(object, module_data_arc.clone())
        .expect("DemoTrapUpdate failed to initialize");

    let module_name = AsciiString::from("DemoTrapUpdate");
    Box::new(DemoTrapUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn smart_bomb_target_homing_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SmartBombTargetHomingUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SmartBombTargetHomingUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn smart_bomb_target_homing_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SmartBombTargetHomingUpdateModuleData>()
        .expect("SmartBombTargetHomingUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("SmartBombTargetHomingUpdate requires a valid object");
    let behavior = SmartBombTargetHomingUpdate::new(object, module_data_arc.clone())
        .expect("SmartBombTargetHomingUpdate failed to initialize");

    let module_name = AsciiString::from("SmartBombTargetHomingUpdate");
    Box::new(SmartBombTargetHomingUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn tensile_formation_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = TensileFormationUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse TensileFormationUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn tensile_formation_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<TensileFormationUpdateModuleData>()
        .expect("TensileFormationUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("TensileFormationUpdate requires a valid object");
    let behavior = TensileFormationUpdate::new(object, module_data_arc.clone())
        .expect("TensileFormationUpdate failed to initialize");

    let module_name = AsciiString::from("TensileFormationUpdate");
    Box::new(TensileFormationUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn generate_minefield_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = GenerateMinefieldBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse GenerateMinefieldBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn generate_minefield_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<GenerateMinefieldBehaviorModuleData>()
        .expect("GenerateMinefieldBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("GenerateMinefieldBehavior requires a valid object");
    let behavior = GenerateMinefieldBehavior::new(object, module_data_arc.clone())
        .expect("GenerateMinefieldBehavior failed to initialize");

    let module_name = AsciiString::from("GenerateMinefieldBehavior");
    Box::new(GenerateMinefieldBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn special_ability_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpecialAbilityUpdateModuleData>()
        .expect("SpecialAbilityUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("SpecialAbilityUpdate requires object");
    let object_ptr = Arc::downgrade(&object);
    let behavior = SpecialAbilityUpdate::new(object_ptr, module_data_arc.clone());
    let module_name = AsciiString::from("SpecialAbilityUpdate");
    Box::new(SpecialAbilityUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn spectre_gunship_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpectreGunshipUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpectreGunshipUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn spectre_gunship_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpectreGunshipUpdateModuleData>()
        .expect("SpectreGunshipUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("SpectreGunshipUpdate requires object");
    let behavior = SpectreGunshipUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create SpectreGunshipUpdate");
    let module_name = AsciiString::from("SpectreGunshipUpdate");
    Box::new(SpectreGunshipUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn spectre_gunship_deployment_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SpectreGunshipDeploymentUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpectreGunshipDeploymentUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn spectre_gunship_deployment_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpectreGunshipDeploymentUpdateModuleData>()
        .expect("SpectreGunshipDeploymentUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("SpectreGunshipDeploymentUpdate requires object");
    let behavior = SpectreGunshipDeploymentUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create SpectreGunshipDeploymentUpdate");
    let module_name = AsciiString::from("SpectreGunshipDeploymentUpdate");
    Box::new(SpectreGunshipDeploymentUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn particle_uplink_cannon_update_module_data_factory(
    _ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    Box::new(ParticleUplinkCannonUpdateModuleData::default())
}

fn particle_uplink_cannon_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ParticleUplinkCannonUpdateModuleData>()
        .expect("ParticleUplinkCannonUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("ParticleUplinkCannonUpdate requires object");
    let behavior = ParticleUplinkCannonUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create ParticleUplinkCannonUpdate");
    let module_name = AsciiString::from("ParticleUplinkCannonUpdate");
    Box::new(ParticleUplinkCannonUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn battle_plan_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(BattlePlanUpdateModuleData::default())
}

fn battle_plan_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BattlePlanUpdateModuleData>()
        .expect("BattlePlanUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("BattlePlanUpdate requires object");
    let behavior = BattlePlanUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create BattlePlanUpdate");
    let module_name = AsciiString::from("BattlePlanUpdate");
    Box::new(BattlePlanUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn lifetime_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = LifetimeUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse LifetimeUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn lifetime_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<LifetimeUpdateModuleData>()
        .expect("LifetimeUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("LifetimeUpdate requires object");
    let behavior = LifetimeUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create LifetimeUpdate");
    let module_name = AsciiString::from("LifetimeUpdate");
    Box::new(LifetimeUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn missile_launcher_building_update_module_data_factory(
    _ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    Box::new(MissileLauncherBuildingUpdateModuleData::default())
}

fn missile_launcher_building_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<MissileLauncherBuildingUpdateModuleData>()
        .expect("MissileLauncherBuildingUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("MissileLauncherBuildingUpdate requires object");
    let behavior = MissileLauncherBuildingUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create MissileLauncherBuildingUpdate");
    let module_name = AsciiString::from("MissileLauncherBuildingUpdate");
    Box::new(MissileLauncherBuildingUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn spy_vision_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpyVisionUpdateModuleData>()
        .expect("SpyVisionUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let module_name_key = NameKeyGenerator::name_to_key("SpyVisionUpdate");
    let behavior = SpyVisionUpdate::new(module_name_key, module_data_arc.clone(), owner_id);

    let module_name = AsciiString::from("SpyVisionUpdate");
    Box::new(SpyVisionUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn fire_weapon_when_dead_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = FireWeaponWhenDeadBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireWeaponWhenDeadBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn fire_weapon_when_dead_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireWeaponWhenDeadBehaviorModuleData>()
        .expect("FireWeaponWhenDeadBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FireWeaponWhenDeadBehavior requires a valid object");
    let behavior = FireWeaponWhenDeadBehavior::new(object, module_data_arc.clone())
        .expect("FireWeaponWhenDeadBehavior failed to initialize");

    let module_name = AsciiString::from("FireWeaponWhenDeadBehavior");
    Box::new(FireWeaponWhenDeadBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn fire_weapon_when_damaged_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = FireWeaponWhenDamagedBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireWeaponWhenDamagedBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn fire_weapon_when_damaged_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireWeaponWhenDamagedBehaviorModuleData>()
        .expect("FireWeaponWhenDamagedBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FireWeaponWhenDamagedBehavior requires a valid object");
    let behavior = FireWeaponWhenDamagedBehavior::new(object, module_data_arc.clone())
        .expect("FireWeaponWhenDamagedBehavior failed to initialize");

    let module_name = AsciiString::from("FireWeaponWhenDamagedBehavior");
    Box::new(FireWeaponWhenDamagedBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn fire_weapon_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FireWeaponUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireWeaponUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn fire_weapon_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireWeaponUpdateModuleData>()
        .expect("FireWeaponUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FireWeaponUpdate requires a valid object");
    let behavior = FireWeaponUpdate::new(object, module_data_arc.clone())
        .expect("FireWeaponUpdate failed to initialize");

    let module_name = AsciiString::from("FireWeaponUpdate");
    Box::new(FireWeaponUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn fire_ocl_after_weapon_cooldown_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = FireOCLAfterWeaponCooldownUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireOCLAfterWeaponCooldownUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn fire_ocl_after_weapon_cooldown_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireOCLAfterWeaponCooldownUpdateModuleData>()
        .expect("FireOCLAfterWeaponCooldownUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FireOCLAfterWeaponCooldownUpdate requires a valid object");
    let behavior = FireOCLAfterWeaponCooldownUpdate::new(object, module_data_arc.clone())
        .expect("FireOCLAfterWeaponCooldownUpdate failed to initialize");

    let module_name = AsciiString::from("FireOCLAfterWeaponCooldownUpdate");
    Box::new(FireOCLAfterWeaponCooldownUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn weapon_bonus_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = WeaponBonusUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse WeaponBonusUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn weapon_bonus_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WeaponBonusUpdateModuleData>()
        .expect("WeaponBonusUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("WeaponBonusUpdate requires a valid object");
    let behavior = WeaponBonusUpdate::new(object, module_data_arc.clone())
        .expect("WeaponBonusUpdate failed to initialize");

    let module_name = AsciiString::from("WeaponBonusUpdate");
    Box::new(WeaponBonusUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn emp_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<EMPUpdateModuleData>()
        .expect("EMPUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("EMPUpdate requires a valid object");
    let behavior =
        EMPUpdate::new(object, module_data_arc.clone()).expect("EMPUpdate failed to initialize");

    let module_name = AsciiString::from("EMPUpdate");
    Box::new(EMPUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn structure_collapse_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StructureCollapseUpdateModuleData>()
        .expect("StructureCollapseUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("StructureCollapseUpdate requires a valid object");
    let behavior = StructureCollapseUpdate::new(object, module_data_arc.clone())
        .expect("StructureCollapseUpdate failed to initialize");

    let module_name = AsciiString::from("StructureCollapseUpdate");
    Box::new(StructureCollapseUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn float_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FloatUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FloatUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn float_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FloatUpdateModuleData>()
        .expect("FloatUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("FloatUpdate requires a valid object");
    let behavior = FloatUpdate::new(object, module_data_arc.clone())
        .expect("FloatUpdate failed to initialize");

    let module_name = AsciiString::from("FloatUpdate");
    Box::new(FloatUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn enemy_near_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = EnemyNearUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse EnemyNearUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn enemy_near_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<EnemyNearUpdateModuleData>()
        .expect("EnemyNearUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("EnemyNearUpdate requires a valid object");
    let behavior = EnemyNearUpdate::new(object, module_data_arc.clone())
        .expect("EnemyNearUpdate failed to initialize");

    let module_name = AsciiString::from("EnemyNearUpdate");
    Box::new(EnemyNearUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn auto_find_healing_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AutoFindHealingUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AutoFindHealingUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn auto_find_healing_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AutoFindHealingUpdateModuleData>()
        .expect("AutoFindHealingUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("AutoFindHealingUpdate requires a valid object");
    let behavior = AutoFindHealingUpdate::new(object, module_data_arc.clone())
        .expect("AutoFindHealingUpdate failed to initialize");

    let module_name = AsciiString::from("AutoFindHealingUpdate");
    Box::new(AutoFindHealingUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn supply_warehouse_crippling_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SupplyWarehouseCripplingBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SupplyWarehouseCripplingBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn supply_warehouse_crippling_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SupplyWarehouseCripplingBehaviorModuleData>()
        .expect("SupplyWarehouseCripplingBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("SupplyWarehouseCripplingBehavior requires a valid object");
    let behavior = SupplyWarehouseCripplingBehavior::new(object, module_data_arc.clone())
        .expect("SupplyWarehouseCripplingBehavior failed to initialize");

    let module_name = AsciiString::from("SupplyWarehouseCripplingBehavior");
    Box::new(SupplyWarehouseCripplingBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn base_regenerate_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BaseRegenerateUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BaseRegenerateUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn base_regenerate_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BaseRegenerateUpdateModuleData>()
        .expect("BaseRegenerateUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("BaseRegenerateUpdate requires a valid object");
    let behavior = BaseRegenerateUpdate::new(object, module_data_arc.clone())
        .expect("BaseRegenerateUpdate failed to initialize");

    let module_name = AsciiString::from("BaseRegenerateUpdate");
    Box::new(BaseRegenerateUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn auto_deposit_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AutoDepositUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AutoDepositUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn auto_deposit_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AutoDepositUpdateModuleData>()
        .expect("AutoDepositUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("AutoDepositUpdate requires a valid object");
    let behavior = AutoDepositUpdate::new(object, module_data_arc.clone())
        .expect("AutoDepositUpdate failed to initialize");

    let module_name = AsciiString::from("AutoDepositUpdate");
    Box::new(AutoDepositUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn power_plant_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PowerPlantUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PowerPlantUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn power_plant_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PowerPlantUpdateModuleData>()
        .expect("PowerPlantUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("PowerPlantUpdate requires a valid object");
    let behavior = PowerPlantUpdate::new(object, module_data_arc.clone())
        .expect("PowerPlantUpdate failed to initialize");

    let module_name = AsciiString::from("PowerPlantUpdate");
    Box::new(PowerPlantUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn tech_building_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = TechBuildingBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse TechBuildingBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn tech_building_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<TechBuildingBehaviorModuleData>()
        .expect("TechBuildingBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("TechBuildingBehavior requires a valid object");
    let behavior = TechBuildingBehavior::new(object, module_data_arc.clone())
        .expect("TechBuildingBehavior failed to initialize");

    let module_name = AsciiString::from("TechBuildingBehavior");
    Box::new(TechBuildingBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn propaganda_tower_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PropagandaTowerBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PropagandaTowerBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn propaganda_tower_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PropagandaTowerBehaviorModuleData>()
        .expect("PropagandaTowerBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("PropagandaTowerBehavior requires a valid object");
    let behavior = PropagandaTowerBehavior::new(object, module_data_arc.clone())
        .expect("PropagandaTowerBehavior failed to initialize");

    let module_name = AsciiString::from("PropagandaTowerBehavior");
    Box::new(PropagandaTowerBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn assisted_targeting_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AssistedTargetingUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AssistedTargetingUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn assisted_targeting_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AssistedTargetingUpdateModuleData>()
        .expect("AssistedTargetingUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("AssistedTargetingUpdate requires a valid object");
    let behavior = AssistedTargetingUpdate::new(object, module_data_arc.clone())
        .expect("AssistedTargetingUpdate failed to initialize");

    let module_name = AsciiString::from("AssistedTargetingUpdate");
    Box::new(AssistedTargetingUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn dynamic_shroud_clearing_range_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = DynamicShroudClearingRangeUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DynamicShroudClearingRangeUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn dynamic_shroud_clearing_range_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let config = module_data
        .get_dynamic_shroud_clearing_range_update_config()
        .expect("DynamicShroudClearingRangeUpdateModuleData expected");
    let module_data_arc = Arc::new(DynamicShroudClearingRangeUpdateModuleData::from_config(
        config,
        module_data.get_module_tag_name_key(),
    ));
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("DynamicShroudClearingRangeUpdate requires a valid object");
    let behavior = DynamicShroudClearingRangeUpdate::new_with_data(object, module_data_arc.clone())
        .expect("DynamicShroudClearingRangeUpdate failed to initialize");

    let module_name = AsciiString::from("DynamicShroudClearingRangeUpdate");
    Box::new(DynamicShroudClearingRangeUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn cleanup_hazard_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CleanupHazardUpdateModuleData>()
        .expect("CleanupHazardUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("CleanupHazardUpdate requires a valid object");
    let behavior = CleanupHazardUpdate::new(object, module_data_arc.clone())
        .expect("CleanupHazardUpdate failed to initialize");

    let module_name = AsciiString::from("CleanupHazardUpdate");
    Box::new(CleanupHazardUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn fire_spread_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireSpreadUpdateModuleData>()
        .expect("FireSpreadUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FireSpreadUpdate requires a valid object");
    let behavior = FireSpreadUpdate::new(owner_id, (*module_data_arc).clone());

    let module_name = AsciiString::from("FireSpreadUpdate");
    Box::new(FireSpreadUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn slaved_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SlavedUpdateModuleData>()
        .expect("SlavedUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let behavior = SlavedUpdate::new(owner_id, module_data_arc.clone())
        .expect("SlavedUpdate failed to initialize");

    let module_name = AsciiString::from("SlavedUpdate");
    Box::new(SlavedUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn mob_member_slaved_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<MobMemberSlavedUpdateModuleData>()
        .expect("MobMemberSlavedUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("MobMemberSlavedUpdate requires a valid object");
    let behavior = MobMemberSlavedUpdate::new(object, module_data_arc.clone())
        .expect("MobMemberSlavedUpdate failed to initialize");

    let module_name = AsciiString::from("MobMemberSlavedUpdate");
    Box::new(MobMemberSlavedUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn command_button_hunt_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CommandButtonHuntUpdateModuleData>()
        .expect("CommandButtonHuntUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let behavior = CommandButtonHuntUpdate::new(owner_id, module_data_arc.clone());

    let module_name = AsciiString::from("CommandButtonHuntUpdate");
    Box::new(CommandButtonHuntUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn topple_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ToppleUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ToppleUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn topple_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ToppleUpdateModuleData>()
        .expect("ToppleUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("ToppleUpdate requires a valid object");
    let behavior = ToppleUpdate::new_from_object_handle(object, module_data_arc.clone());

    let module_name = AsciiString::from("ToppleUpdate");
    Box::new(ToppleUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn structure_topple_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StructureToppleUpdateModuleData>()
        .expect("StructureToppleUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("StructureToppleUpdate requires a valid object");
    let behavior = StructureToppleUpdate::new(object, module_data_arc.clone())
        .expect("StructureToppleUpdate failed to initialize");

    let module_name = AsciiString::from("StructureToppleUpdate");
    Box::new(StructureToppleUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn rebuild_hole_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RebuildHoleBehaviorModuleData>()
        .expect("RebuildHoleBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = RebuildHoleBehavior::from_module_thing(thing, module_data_arc.clone());

    let module_name = AsciiString::from("RebuildHoleBehavior");
    Box::new(RebuildHoleBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn overcharge_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn firing_tracker_behavior_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(FiringTrackerBehaviorModuleData::default())
}

fn firing_tracker_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FiringTrackerBehaviorModuleData>()
        .expect("FiringTrackerBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    let behavior = FiringTrackerBehavior::new(owner_id);

    let module_name = AsciiString::from("FiringTracker");
    Box::new(FiringTrackerBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn overcharge_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<OverchargeBehaviorModuleData>()
        .expect("OverchargeBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = OverchargeBehavior::from_module_thing(thing, module_data_arc.clone());

    let module_name = AsciiString::from("OverchargeBehavior");
    Box::new(OverchargeBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn countermeasures_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CountermeasuresBehaviorModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CountermeasuresBehaviorModuleData>()
        .expect("CountermeasuresBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        CountermeasuresBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("CountermeasuresBehavior requires an owning object");

    let module_name = AsciiString::from("CountermeasuresBehavior");
    Box::new(CountermeasuresBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn bunker_buster_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BunkerBusterBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BunkerBusterBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn bunker_buster_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BunkerBusterBehaviorModuleData>()
        .expect("BunkerBusterBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        BunkerBusterBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("BunkerBusterBehavior requires an owning object");

    let module_name = AsciiString::from("BunkerBusterBehavior");
    Box::new(BunkerBusterBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn flight_deck_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FlightDeckBehaviorModuleData>()
        .expect("FlightDeckBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        FlightDeckBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("FlightDeckBehavior requires an owning object");

    let module_name = AsciiString::from("FlightDeckBehavior");
    Box::new(FlightDeckBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

#[cfg(feature = "allow_surrender")]
fn pow_truck_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = POWTruckBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse POWTruckBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

#[cfg(feature = "allow_surrender")]
fn pow_truck_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<POWTruckBehaviorModuleData>()
        .expect("POWTruckBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id)
        .expect("POWTruckBehavior requires owning object");
    let behavior = POWTruckBehavior::new(object, module_data_arc.clone())
        .expect("POWTruckBehavior::new failed");

    let module_name = AsciiString::from("POWTruckBehavior");
    Box::new(POWTruckBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

#[cfg(feature = "allow_surrender")]
fn prison_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PrisonBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PrisonBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

#[cfg(feature = "allow_surrender")]
fn prison_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PrisonBehaviorModuleData>()
        .expect("PrisonBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("PrisonBehavior requires owning object");
    let behavior =
        PrisonBehavior::new(object, module_data_arc.clone()).expect("PrisonBehavior::new failed");

    let module_name = AsciiString::from("PrisonBehavior");
    Box::new(PrisonBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

#[cfg(feature = "allow_surrender")]
fn propaganda_center_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PropagandaCenterBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PropagandaCenterBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

#[cfg(feature = "allow_surrender")]
fn propaganda_center_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PropagandaCenterBehaviorModuleData>()
        .expect("PropagandaCenterBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id)
        .expect("PropagandaCenterBehavior requires owning object");
    let behavior = PropagandaCenterBehavior::new(object, module_data_arc.clone())
        .expect("PropagandaCenterBehavior::new failed");

    let module_name = AsciiString::from("PropagandaCenterBehavior");
    Box::new(PropagandaCenterBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn queue_production_exit_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = QueueProductionExitModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse QueueProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn queue_production_exit_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<QueueProductionExitModuleData>()
        .expect("QueueProductionExitModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        QueueProductionExitBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("QueueProductionExitUpdate requires an owning object");

    let module_name = AsciiString::from("QueueProductionExitUpdate");
    Box::new(QueueProductionExitBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn default_production_exit_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = DefaultProductionExitModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse DefaultProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn default_production_exit_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DefaultProductionExitModuleData>()
        .expect("DefaultProductionExitModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = DefaultProductionExitBehavior::from_module_thing(
        Arc::clone(&thing),
        module_data_arc.clone(),
    )
    .expect("DefaultProductionExitUpdate requires an owning object");

    let module_name = AsciiString::from("DefaultProductionExitUpdate");
    Box::new(DefaultProductionExitBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn spawn_point_production_exit_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SpawnPointProductionExitModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse SpawnPointProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn spawn_point_production_exit_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpawnPointProductionExitModuleData>()
        .expect("SpawnPointProductionExitModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = SpawnPointProductionExitBehavior::from_module_thing(
        Arc::clone(&thing),
        module_data_arc.clone(),
    )
    .expect("SpawnPointProductionExitUpdate requires an owning object");

    let module_name = AsciiString::from("SpawnPointProductionExitUpdate");
    Box::new(SpawnPointProductionExitBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn supply_center_production_exit_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SupplyCenterProductionExitModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse SupplyCenterProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn supply_center_production_exit_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SupplyCenterProductionExitModuleData>()
        .expect("SupplyCenterProductionExitModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = SupplyCenterProductionExitBehavior::from_module_thing(
        Arc::clone(&thing),
        module_data_arc.clone(),
    )
    .expect("SupplyCenterProductionExitUpdate requires an owning object");

    let module_name = AsciiString::from("SupplyCenterProductionExitUpdate");
    Box::new(SupplyCenterProductionExitBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn battle_bus_slow_death_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    battle_bus_slow_death_data_factory(ini)
}

fn battle_bus_slow_death_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    battle_bus_slow_death_module_factory(thing, module_data)
}

fn bridge_scaffold_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BridgeScaffoldBehaviorModuleData>()
        .expect("BridgeScaffoldBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        BridgeScaffoldBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("BridgeScaffoldBehavior requires an owning object");

    let module_name = AsciiString::from("BridgeScaffoldBehavior");
    Box::new(BridgeScaffoldBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn bridge_tower_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BridgeTowerBehaviorModuleData>()
        .expect("BridgeTowerBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        BridgeTowerBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("BridgeTowerBehavior requires an owning object");

    let module_name = AsciiString::from("BridgeTowerBehavior");
    Box::new(BridgeTowerBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn bridge_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BridgeBehaviorModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BridgeBehaviorModuleData>()
        .expect("BridgeBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = BridgeBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
        .expect("BridgeBehavior requires an owning object");

    let module_name = AsciiString::from("BridgeBehavior");
    Box::new(BridgeBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn parking_place_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ParkingPlaceBehaviorModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse ParkingPlaceBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn parking_place_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ParkingPlaceBehaviorModuleData>()
        .expect("ParkingPlaceBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("ParkingPlaceBehavior requires owning object");
    let behavior = ParkingPlaceBehavior::new(object, module_data_arc.clone())
        .expect("Failed to create ParkingPlaceBehavior");

    let module_name = AsciiString::from("ParkingPlaceBehavior");
    Box::new(ParkingPlaceBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn repair_dock_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RepairDockUpdateData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RepairDockUpdateData>()
        .expect("RepairDockUpdateData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = RepairDockUpdate::new(typed_data.clone(), owner_id, &owner_pos);

    let module_name = AsciiString::from("RepairDockUpdate");
    Box::new(RepairDockUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

#[cfg(feature = "allow_surrender")]
fn prison_dock_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(PrisonDockUpdateData::default())
}

#[cfg(feature = "allow_surrender")]
fn prison_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PrisonDockUpdateData>()
        .expect("PrisonDockUpdateData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = PrisonDockUpdate::new(typed_data.clone(), owner_id, &owner_pos);

    let module_name = AsciiString::from("PrisonDockUpdate");
    Box::new(PrisonDockUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn railed_transport_dock_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RailedTransportDockUpdateData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RailedTransportDockUpdateData>()
        .expect("RailedTransportDockUpdateData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = RailedTransportDockUpdate::new(typed_data.clone(), owner_id, &owner_pos);

    let module_name = AsciiString::from("RailedTransportDockUpdate");
    Box::new(RailedTransportDockUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn supply_center_dock_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SupplyCenterDockUpdateData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SupplyCenterDockUpdateData>()
        .expect("SupplyCenterDockUpdateData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = SupplyCenterDockUpdate::new(typed_data.clone(), owner_id, &owner_pos);

    let module_name = AsciiString::from("SupplyCenterDockUpdate");
    Box::new(SupplyCenterDockUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn supply_warehouse_dock_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SupplyWarehouseDockUpdateData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
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
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SupplyWarehouseDockUpdateData>()
        .expect("SupplyWarehouseDockUpdateData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = SupplyWarehouseDockUpdate::new(typed_data.clone(), owner_id, &owner_pos);

    let module_name = AsciiString::from("SupplyWarehouseDockUpdate");
    Box::new(SupplyWarehouseDockUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn w3d_model_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    // Preserve existing parser proc when available (see install_module_overrides), so this
    // fallback only applies if no prior module-data implementation exists.
    let mut data = W3DModelDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DModelDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_model_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DModelDrawModuleData>()
        .cloned()
        .or_else(|| {
            module_data
                .get_as_w3d_model_draw_module_data()
                .and_then(|any| any.downcast_ref::<W3DModelDrawModuleData>())
                .cloned()
        })
        .unwrap_or_else(|| {
            warn!("W3DModelDrawModuleData expected; using defaults");
            W3DModelDrawModuleData::new()
        });

    let mut module = W3DModelDraw::new(data);
    let (owner_id, _) = resolve_owner_info(&thing);
    if owner_id != INVALID_ID {
        module.bind_owner_id(owner_id);
    }
    Box::new(module)
}

macro_rules! w3d_owner_bound_draw_factories {
    (
        $data_factory:ident,
        $module_factory:ident,
        $data_ty:ty,
        $module_ty:ty,
        $module_name:literal
    ) => {
        fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data = module_data
                .as_ref()
                .downcast_ref::<$data_ty>()
                .cloned()
                .unwrap_or_else(|| {
                    warn!(concat!($module_name, "ModuleData expected; using defaults"));
                    <$data_ty>::new()
                });

            let mut module = <$module_ty>::new(data);
            let (owner_id, _) = resolve_owner_info(&thing);
            if owner_id != INVALID_ID {
                module.bind_owner_id(owner_id);
            }
            Box::new(module)
        }
    };
}

w3d_owner_bound_draw_factories!(
    w3d_default_draw_module_data_factory,
    w3d_default_draw_module_factory,
    W3DDefaultDrawModuleData,
    W3DDefaultDraw,
    "W3DDefaultDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_dependency_model_draw_module_data_factory,
    w3d_dependency_model_draw_module_factory,
    W3DDependencyModelDrawModuleData,
    W3DDependencyModelDraw,
    "W3DDependencyModelDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_overlord_aircraft_draw_module_data_factory,
    w3d_overlord_aircraft_draw_module_factory,
    W3DOverlordAircraftDrawModuleData,
    W3DOverlordAircraftDraw,
    "W3DOverlordAircraftDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_overlord_truck_draw_module_data_factory,
    w3d_overlord_truck_draw_module_factory,
    W3DOverlordTruckDrawModuleData,
    W3DOverlordTruckDraw,
    "W3DOverlordTruckDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_police_car_draw_module_data_factory,
    w3d_police_car_draw_module_factory,
    W3DPoliceCarDrawModuleData,
    W3DPoliceCarDraw,
    "W3DPoliceCarDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_science_model_draw_module_data_factory,
    w3d_science_model_draw_module_factory,
    W3DScienceModelDrawModuleData,
    W3DScienceModelDraw,
    "W3DScienceModelDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_supply_draw_module_data_factory,
    w3d_supply_draw_module_factory,
    W3DSupplyDrawModuleData,
    W3DSupplyDraw,
    "W3DSupplyDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_truck_draw_module_data_factory,
    w3d_truck_draw_module_factory,
    W3DTruckDrawModuleData,
    W3DTruckDraw,
    "W3DTruckDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_tank_truck_draw_module_data_factory,
    w3d_tank_truck_draw_module_factory,
    W3DTankTruckDrawModuleData,
    W3DTankTruckDraw,
    "W3DTankTruckDraw"
);

fn w3d_tank_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DTankDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DTankDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_tank_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DTankDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DTankDrawModuleData expected; using defaults");
            W3DTankDrawModuleData::new()
        });

    let mut module = W3DTankDraw::new(data);
    let (owner_id, _) = resolve_owner_info(&thing);
    if owner_id != INVALID_ID {
        module.bind_owner_id(owner_id);
    }
    Box::new(module)
}

fn w3d_overlord_tank_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DOverlordTankDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DOverlordTankDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_overlord_tank_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DOverlordTankDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DOverlordTankDrawModuleData expected; using defaults");
            W3DOverlordTankDrawModuleData::new()
        });

    let mut module = W3DOverlordTankDraw::new(data);
    let (owner_id, _) = resolve_owner_info(&thing);
    if owner_id != INVALID_ID {
        module.bind_owner_id(owner_id);
    }
    Box::new(module)
}

fn w3d_projectile_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DProjectileDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DProjectileDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_projectile_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DProjectileDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DProjectileDrawModuleData expected; using defaults");
            W3DProjectileDrawModuleData::new()
        });

    let mut module = W3DProjectileDraw::new(data);
    let (owner_id, _) = resolve_owner_info(&thing);
    if owner_id != INVALID_ID {
        module.bind_owner_id(owner_id);
    }
    Box::new(module)
}

fn w3d_laser_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DLaserDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DLaserDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_laser_draw_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DLaserDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DLaserDrawModuleData expected; using defaults");
            W3DLaserDrawModuleData::new()
        });
    Box::new(W3DLaserDraw::new(data))
}

fn w3d_rope_draw_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(W3DRopeDrawModuleData::new())
}

fn w3d_rope_draw_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DRopeDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DRopeDrawModuleData expected; using defaults");
            W3DRopeDrawModuleData::new()
        });
    Box::new(W3DRopeDraw::new(data))
}

fn w3d_projectile_stream_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DProjectileStreamDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DProjectileStreamDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_projectile_stream_draw_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DProjectileStreamDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DProjectileStreamDrawModuleData expected; using defaults");
            W3DProjectileStreamDrawModuleData::new()
        });
    Box::new(W3DProjectileStreamDraw::new(data))
}

fn w3d_tree_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DTreeDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DTreeDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_tree_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DTreeDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DTreeDrawModuleData expected; using defaults");
            W3DTreeDrawModuleData::new()
        });
    let mut module = W3DTreeDraw::new(data);
    let drawable_id = resolve_drawable_id(&thing);
    if drawable_id != INVALID_ID {
        module.bind_drawable_id(drawable_id);
    }
    Box::new(module)
}

fn w3d_tracer_draw_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(W3DTracerDrawModuleData::new())
}

fn w3d_tracer_draw_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DTracerDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DTracerDrawModuleData expected; using defaults");
            W3DTracerDrawModuleData::new()
        });
    Box::new(W3DTracerDraw::new(data))
}

fn w3d_debris_draw_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(W3DDebrisDrawModuleData::new())
}

fn w3d_debris_draw_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DDebrisDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DDebrisDrawModuleData expected; using defaults");
            W3DDebrisDrawModuleData::new()
        });
    Box::new(W3DDebrisDraw::new(data))
}

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
        .downcast_ref::<LaserClientUpdateModuleData>()
        .expect("LaserUpdateModuleData expected");
    let module_name_key = NameKeyGenerator::name_to_key("LaserUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    let module_data_arc = Arc::new(typed_data.clone());
    Box::new(LaserClientUpdateModule::new(
        module_name_key,
        module_data_arc,
        Some(owner_id),
    ))
}

fn ocl_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = OCLUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse OCLUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn ocl_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<OCLUpdateModuleData>()
        .expect("OCLUpdateModuleData expected");
    let module_name_key = NameKeyGenerator::name_to_key("OCLUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    let module_data_arc = Arc::new(typed_data.clone());
    Box::new(OCLUpdateModule::new(
        module_name_key,
        module_data_arc,
        owner_id,
    ))
}

fn special_power_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(SpecialPowerUpdateModuleData::default())
}

fn special_power_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpecialPowerUpdateModuleData>()
        .expect("SpecialPowerUpdateModuleData expected");
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .map(Arc::downgrade)
        .unwrap_or_else(std::sync::Weak::new);

    let mut module = SpecialPowerUpdateModule::new(owner_id, object);
    module.set_module_data(typed_data.clone());
    Box::new(module)
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
        .downcast_ref::<BeaconClientUpdateModuleData>()
        .expect("BeaconClientUpdateModuleData expected");
    let module_name_key = NameKeyGenerator::name_to_key("BeaconClientUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    let module_data_arc = Arc::new(typed_data.clone());
    Box::new(BeaconClientUpdateModule::new(
        module_name_key,
        module_data_arc,
        owner_id,
    ))
}

fn sway_client_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(game_engine::common::thing::module::BaseModuleData::new())
}

fn sway_client_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let module_name_key = NameKeyGenerator::name_to_key("SwayClientUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    Box::new(SwayClientUpdateModule::new(
        module_name_key,
        module_data,
        owner_id,
    ))
}

fn animated_particle_sys_bone_client_update_module_data_factory(
    _ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    Box::new(game_engine::common::thing::module::BaseModuleData::new())
}

fn animated_particle_sys_bone_client_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let module_name_key = NameKeyGenerator::name_to_key("AnimatedParticleSysBoneClientUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    Box::new(AnimatedParticleSysBoneClientUpdateModule::new(
        module_name_key,
        module_data,
        owner_id,
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
    let typed_data =
        expect_contain_data::<OpenContainModuleData>(module_data.as_ref(), "OpenContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain = OpenContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
        warn!(
            "Failed to create OpenContain for object {}: {}",
            owner_id, err
        );
        OpenContain::new(Weak::new(), &OpenContainModuleData::default())
            .expect("OpenContain default construction failed")
    });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("OpenContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<TransportContainModuleData>(module_data.as_ref(), "TransportContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        TransportContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create TransportContain for object {}: {}",
                owner_id, err
            );
            TransportContain::new(Weak::new(), &TransportContainModuleData::default())
                .expect("TransportContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("TransportContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<GarrisonContainModuleData>(module_data.as_ref(), "GarrisonContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        GarrisonContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create GarrisonContain for object {}: {}",
                owner_id, err
            );
            GarrisonContain::new(Weak::new(), &GarrisonContainModuleData::default())
                .expect("GarrisonContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("GarrisonContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<TunnelContainModuleData>(module_data.as_ref(), "TunnelContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain = TunnelContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
        warn!(
            "Failed to create TunnelContain for object {}: {}",
            owner_id, err
        );
        TunnelContain::new(Weak::new(), &TunnelContainModuleData::default())
            .expect("TunnelContain default construction failed")
    });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("TunnelContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<OverlordContainModuleData>(module_data.as_ref(), "OverlordContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        OverlordContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create OverlordContain for object {}: {}",
                owner_id, err
            );
            OverlordContain::new(Weak::new(), &OverlordContainModuleData::default())
                .expect("OverlordContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("OverlordContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<HelixContainModuleData>(module_data.as_ref(), "HelixContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain = HelixContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
        warn!(
            "Failed to create HelixContain for object {}: {}",
            owner_id, err
        );
        HelixContain::new(Weak::new(), &HelixContainModuleData::default())
            .expect("HelixContain default construction failed")
    });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("HelixContain", thing, module_data, contain)
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
    let typed_data = expect_contain_data::<RailedTransportContainModuleData>(
        module_data.as_ref(),
        "RailedTransportContain",
    );
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain = RailedTransportContain::new(make_owner_weak(owner_id), typed_data)
        .unwrap_or_else(|err| {
            warn!(
                "Failed to create RailedTransportContain for object {}: {}",
                owner_id, err
            );
            RailedTransportContain::new(Weak::new(), &RailedTransportContainModuleData::default())
                .expect("RailedTransportContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("RailedTransportContain", thing, module_data, contain)
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
    let typed_data = expect_contain_data::<RiderChangeContainModuleData>(
        module_data.as_ref(),
        "RiderChangeContain",
    );
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        RiderChangeContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create RiderChangeContain for object {}: {}",
                owner_id, err
            );
            RiderChangeContain::new(Weak::new(), &RiderChangeContainModuleData::default())
                .expect("RiderChangeContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("RiderChangeContain", thing, module_data, contain)
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
    let typed_data = expect_contain_data::<InternetHackContainModuleData>(
        module_data.as_ref(),
        "InternetHackContain",
    );
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        InternetHackContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create InternetHackContain for object {}: {}",
                owner_id, err
            );
            InternetHackContain::new(Weak::new(), &InternetHackContainModuleData::default())
                .expect("InternetHackContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("InternetHackContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<HealContainModuleData>(module_data.as_ref(), "HealContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain = HealContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
        warn!(
            "Failed to create HealContain for object {}: {}",
            owner_id, err
        );
        HealContain::new(Weak::new(), &HealContainModuleData::default())
            .expect("HealContain default construction failed")
    });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("HealContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<CaveContainModuleData>(module_data.as_ref(), "CaveContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        CaveContain::new(make_owner_weak(owner_id), typed_data, None).unwrap_or_else(|err| {
            warn!(
                "Failed to create CaveContain for object {}: {}",
                owner_id, err
            );
            CaveContain::new(Weak::new(), &CaveContainModuleData::default(), None)
                .expect("CaveContain default construction failed")
        });
    let contain: Arc<Mutex<CaveContain>> = Arc::new(Mutex::new(contain));
    let module_name_key = NameKeyGenerator::name_to_key("CaveContain");
    Box::new(CaveContainBindingModule::new(
        module_name_key,
        module_data,
        contain,
        owner_id,
    ))
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
    let typed_data =
        expect_contain_data::<ParachuteContainModuleData>(module_data.as_ref(), "ParachuteContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        ParachuteContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create ParachuteContain for object {}: {}",
                owner_id, err
            );
            ParachuteContain::new(Weak::new(), &ParachuteContainModuleData::default())
                .expect("ParachuteContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("ParachuteContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<MobNexusContainModuleData>(module_data.as_ref(), "MobNexusContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        MobNexusContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create MobNexusContain for object {}: {}",
                owner_id, err
            );
            MobNexusContain::new(Weak::new(), &MobNexusContainModuleData::default())
                .expect("MobNexusContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("MobNexusContain", thing, module_data, contain)
}

pub fn install_module_overrides() -> Result<(), String> {
    register_module_override(
        "InactiveBody",
        ModuleType::Body,
        inactive_body_module_factory,
        inactive_body_module_data_factory,
    )?;

    register_module_override(
        "ActiveBody",
        ModuleType::Body,
        active_body_module_factory,
        active_body_module_data_factory,
    )?;

    register_module_override(
        "StructureBody",
        ModuleType::Body,
        structure_body_module_factory,
        structure_body_module_data_factory,
    )?;

    register_module_override(
        "HighlanderBody",
        ModuleType::Body,
        highlander_body_module_factory,
        highlander_body_module_data_factory,
    )?;

    register_module_override(
        "ImmortalBody",
        ModuleType::Body,
        immortal_body_module_factory,
        immortal_body_module_data_factory,
    )?;

    register_module_override(
        "HiveStructureBody",
        ModuleType::Body,
        hive_structure_body_module_factory,
        hive_structure_body_module_data_factory,
    )?;

    register_module_override(
        "UndeadBody",
        ModuleType::Body,
        undead_body_module_factory,
        undead_body_module_data_factory,
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
        "LockWeaponCreate",
        ModuleType::Behavior,
        lock_weapon_create_module_factory,
        lock_weapon_create_module_data_factory,
    )?;

    register_module_override(
        "PreorderCreate",
        ModuleType::Behavior,
        preorder_create_module_factory,
        simple_create_module_data_factory,
    )?;

    register_module_override(
        "SupplyCenterCreate",
        ModuleType::Behavior,
        supply_center_create_module_factory,
        simple_create_module_data_factory,
    )?;

    register_module_override(
        "SupplyWarehouseCreate",
        ModuleType::Behavior,
        supply_warehouse_create_module_factory,
        simple_create_module_data_factory,
    )?;

    register_module_override(
        "SpecialPowerCreate",
        ModuleType::Behavior,
        special_power_create_module_factory,
        simple_create_module_data_factory,
    )?;

    register_module_override(
        "SpecialPowerModule",
        ModuleType::Behavior,
        special_power_module_factory,
        special_power_module_data_factory,
    )?;

    register_module_override(
        "ProductionUpdate",
        ModuleType::Behavior,
        production_update_module_factory,
        production_update_module_data_factory,
    )?;

    register_module_override(
        "DemoralizeSpecialPower",
        ModuleType::Behavior,
        demoralize_special_power_module_factory,
        demoralize_special_power_module_data_factory,
    )?;

    register_module_override(
        "CashHackSpecialPower",
        ModuleType::Behavior,
        cash_hack_special_power_module_factory,
        cash_hack_special_power_module_data_factory,
    )?;

    register_module_override(
        "SpyVisionSpecialPower",
        ModuleType::Behavior,
        spy_vision_special_power_module_factory,
        spy_vision_special_power_module_data_factory,
    )?;

    register_module_override(
        "DefectorSpecialPower",
        ModuleType::Behavior,
        defector_special_power_module_factory,
        defector_special_power_module_data_factory,
    )?;

    register_module_override(
        "CashBountyPower",
        ModuleType::Behavior,
        cash_bounty_power_module_factory,
        cash_bounty_power_module_data_factory,
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
        "SpecialAbility",
        ModuleType::Behavior,
        special_ability_module_factory,
        special_ability_module_data_factory,
    )?;

    register_module_override(
        "BaikonurLaunchPower",
        ModuleType::Behavior,
        baikonur_launch_power_module_factory,
        baikonur_launch_power_module_data_factory,
    )?;

    register_module_override(
        "OCLSpecialPower",
        ModuleType::Behavior,
        ocl_special_power_module_factory,
        ocl_special_power_module_data_factory,
    )?;

    register_module_override(
        "GrantUpgradeCreate",
        ModuleType::Behavior,
        grant_upgrade_create_module_factory,
        grant_upgrade_create_module_data_factory,
    )?;

    register_module_override(
        "VeterancyGainCreate",
        ModuleType::Behavior,
        veterancy_gain_create_module_factory,
        veterancy_gain_create_module_data_factory,
    )?;

    register_module_override(
        "FireWeaponCollide",
        ModuleType::Behavior,
        fire_weapon_collide_module_factory,
        fire_weapon_collide_module_data_factory,
    )?;

    register_module_override(
        "ShroudCrateCollide",
        ModuleType::Behavior,
        shroud_crate_collide_module_factory,
        shroud_crate_collide_module_data_factory,
    )?;

    register_module_override(
        "W3DModelDraw",
        ModuleType::Draw,
        w3d_model_draw_module_factory,
        module_data_proc_or(
            "W3DModelDraw",
            ModuleType::Draw,
            w3d_model_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DDefaultDraw",
        ModuleType::Draw,
        w3d_default_draw_module_factory,
        module_data_proc_or(
            "W3DDefaultDraw",
            ModuleType::Draw,
            w3d_default_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DDependencyModelDraw",
        ModuleType::Draw,
        w3d_dependency_model_draw_module_factory,
        module_data_proc_or(
            "W3DDependencyModelDraw",
            ModuleType::Draw,
            w3d_dependency_model_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DOverlordAircraftDraw",
        ModuleType::Draw,
        w3d_overlord_aircraft_draw_module_factory,
        module_data_proc_or(
            "W3DOverlordAircraftDraw",
            ModuleType::Draw,
            w3d_overlord_aircraft_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DTankDraw",
        ModuleType::Draw,
        w3d_tank_draw_module_factory,
        module_data_proc_or(
            "W3DTankDraw",
            ModuleType::Draw,
            w3d_tank_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DOverlordTankDraw",
        ModuleType::Draw,
        w3d_overlord_tank_draw_module_factory,
        module_data_proc_or(
            "W3DOverlordTankDraw",
            ModuleType::Draw,
            w3d_overlord_tank_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DOverlordTruckDraw",
        ModuleType::Draw,
        w3d_overlord_truck_draw_module_factory,
        module_data_proc_or(
            "W3DOverlordTruckDraw",
            ModuleType::Draw,
            w3d_overlord_truck_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DPoliceCarDraw",
        ModuleType::Draw,
        w3d_police_car_draw_module_factory,
        module_data_proc_or(
            "W3DPoliceCarDraw",
            ModuleType::Draw,
            w3d_police_car_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DProjectileDraw",
        ModuleType::Draw,
        w3d_projectile_draw_module_factory,
        module_data_proc_or(
            "W3DProjectileDraw",
            ModuleType::Draw,
            w3d_projectile_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DLaserDraw",
        ModuleType::Draw,
        w3d_laser_draw_module_factory,
        w3d_laser_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DRopeDraw",
        ModuleType::Draw,
        w3d_rope_draw_module_factory,
        w3d_rope_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DProjectileStreamDraw",
        ModuleType::Draw,
        w3d_projectile_stream_draw_module_factory,
        w3d_projectile_stream_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DTreeDraw",
        ModuleType::Draw,
        w3d_tree_draw_module_factory,
        w3d_tree_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DTracerDraw",
        ModuleType::Draw,
        w3d_tracer_draw_module_factory,
        w3d_tracer_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DDebrisDraw",
        ModuleType::Draw,
        w3d_debris_draw_module_factory,
        w3d_debris_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DScienceModelDraw",
        ModuleType::Draw,
        w3d_science_model_draw_module_factory,
        module_data_proc_or(
            "W3DScienceModelDraw",
            ModuleType::Draw,
            w3d_science_model_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DSupplyDraw",
        ModuleType::Draw,
        w3d_supply_draw_module_factory,
        module_data_proc_or(
            "W3DSupplyDraw",
            ModuleType::Draw,
            w3d_supply_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DTruckDraw",
        ModuleType::Draw,
        w3d_truck_draw_module_factory,
        module_data_proc_or(
            "W3DTruckDraw",
            ModuleType::Draw,
            w3d_truck_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DTankTruckDraw",
        ModuleType::Draw,
        w3d_tank_truck_draw_module_factory,
        module_data_proc_or(
            "W3DTankTruckDraw",
            ModuleType::Draw,
            w3d_tank_truck_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "LaserUpdate",
        ModuleType::ClientUpdate,
        laser_update_module_factory,
        laser_update_module_data_factory,
    )?;

    register_module_override(
        "OCLUpdate",
        ModuleType::Behavior,
        ocl_update_module_factory,
        ocl_update_module_data_factory,
    )?;

    register_module_override(
        "SpecialPowerUpdate",
        ModuleType::Behavior,
        special_power_update_module_factory,
        special_power_update_module_data_factory,
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
        sway_client_update_module_data_factory,
    )?;

    register_module_override(
        "AnimatedParticleSysBoneClientUpdate",
        ModuleType::ClientUpdate,
        animated_particle_sys_bone_client_update_module_factory,
        animated_particle_sys_bone_client_update_module_data_factory,
    )?;

    register_module_override(
        "SquishCollide",
        ModuleType::Behavior,
        squish_collide_module_factory,
        squish_collide_module_data_factory,
    )?;

    register_module_override(
        "UpgradeDie",
        ModuleType::Behavior,
        upgrade_die_module_factory,
        upgrade_die_module_data_factory,
    )?;
    register_module_override(
        "DestroyDie",
        ModuleType::Behavior,
        destroy_die_module_factory,
        die_module_data_factory,
    )?;
    register_module_override(
        "KeepObjectDie",
        ModuleType::Behavior,
        keep_object_die_module_factory,
        die_module_data_factory,
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
        "StatusBitsUpgrade",
        ModuleType::Behavior,
        status_bits_upgrade_module_factory,
        status_bits_upgrade_module_data_factory,
    )?;

    register_module_override(
        "PassengersFireUpgrade",
        ModuleType::Behavior,
        passengers_fire_upgrade_module_factory,
        passengers_fire_upgrade_module_data_factory,
    )?;

    register_module_override(
        "SubObjectsUpgrade",
        ModuleType::Behavior,
        subobjects_upgrade_module_factory,
        subobjects_upgrade_module_data_factory,
    )?;

    register_module_override(
        "GrantScienceUpgrade",
        ModuleType::Behavior,
        grant_science_upgrade_module_factory,
        grant_science_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ObjectCreationUpgrade",
        ModuleType::Behavior,
        object_creation_upgrade_module_factory,
        object_creation_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ActiveShroudUpgrade",
        ModuleType::Behavior,
        active_shroud_upgrade_module_factory,
        active_shroud_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ArmorUpgrade",
        ModuleType::Behavior,
        armor_upgrade_module_factory,
        armor_upgrade_module_data_factory,
    )?;

    register_module_override(
        "CommandSetUpgrade",
        ModuleType::Behavior,
        command_set_upgrade_module_factory,
        command_set_upgrade_module_data_factory,
    )?;

    register_module_override(
        "CostModifierUpgrade",
        ModuleType::Behavior,
        cost_modifier_upgrade_module_factory,
        cost_modifier_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ExperienceScalarUpgrade",
        ModuleType::Behavior,
        experience_scalar_upgrade_module_factory,
        experience_scalar_upgrade_module_data_factory,
    )?;

    register_module_override(
        "LocomotorSetUpgrade",
        ModuleType::Behavior,
        locomotor_set_upgrade_module_factory,
        locomotor_set_upgrade_module_data_factory,
    )?;

    register_module_override(
        "MaxHealthUpgrade",
        ModuleType::Behavior,
        max_health_upgrade_module_factory,
        max_health_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ModelConditionUpgrade",
        ModuleType::Behavior,
        model_condition_upgrade_module_factory,
        model_condition_upgrade_module_data_factory,
    )?;

    register_module_override(
        "PowerPlantUpgrade",
        ModuleType::Behavior,
        power_plant_upgrade_module_factory,
        power_plant_upgrade_module_data_factory,
    )?;

    register_module_override(
        "RadarUpgrade",
        ModuleType::Behavior,
        radar_upgrade_module_factory,
        radar_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ReplaceObjectUpgrade",
        ModuleType::Behavior,
        replace_object_upgrade_module_factory,
        replace_object_upgrade_module_data_factory,
    )?;

    register_module_override(
        "StealthUpgrade",
        ModuleType::Behavior,
        stealth_upgrade_module_factory,
        stealth_upgrade_module_data_factory,
    )?;

    register_module_override(
        "UnpauseSpecialPowerUpgrade",
        ModuleType::Behavior,
        unpause_special_power_upgrade_module_factory,
        unpause_special_power_upgrade_module_data_factory,
    )?;

    register_module_override(
        "WeaponBonusUpgrade",
        ModuleType::Behavior,
        weapon_bonus_upgrade_module_factory,
        weapon_bonus_upgrade_module_data_factory,
    )?;

    register_module_override(
        "WeaponSetUpgrade",
        ModuleType::Behavior,
        weapon_set_upgrade_module_factory,
        weapon_set_upgrade_module_data_factory,
    )?;

    register_module_override(
        "TransitionDamageFX",
        ModuleType::Behavior,
        transition_damage_fx_module_factory,
        transition_damage_fx_module_data_factory,
    )?;

    register_module_override(
        "StealthUpdate",
        ModuleType::Behavior,
        stealth_update_module_factory,
        stealth_update_module_data_factory,
    )?;

    register_module_override(
        "StickyBombUpdate",
        ModuleType::Behavior,
        sticky_bomb_update_module_factory,
        sticky_bomb_update_module_data_factory,
    )?;

    register_module_override(
        "ProneUpdate",
        ModuleType::Behavior,
        prone_update_module_factory,
        prone_update_module_data_factory,
    )?;

    register_module_override(
        "ProjectileStreamUpdate",
        ModuleType::Behavior,
        projectile_stream_update_module_factory,
        projectile_stream_update_module_data_factory,
    )?;

    register_module_override(
        "PointDefenseLaserUpdate",
        ModuleType::Behavior,
        point_defense_laser_update_module_factory,
        point_defense_laser_update_module_data_factory,
    )?;

    register_module_override(
        "LaserUpdate",
        ModuleType::Behavior,
        laser_behavior_update_module_factory,
        laser_behavior_update_module_data_factory,
    )?;

    register_module_override(
        "BoneFXUpdate",
        ModuleType::Behavior,
        bone_fx_update_module_factory,
        bone_fx_update_module_data_factory,
    )?;

    register_module_override(
        "DemoTrapUpdate",
        ModuleType::Behavior,
        demo_trap_update_module_factory,
        demo_trap_update_module_data_factory,
    )?;

    register_module_override(
        "SmartBombTargetHomingUpdate",
        ModuleType::Behavior,
        smart_bomb_target_homing_update_module_factory,
        smart_bomb_target_homing_update_module_data_factory,
    )?;

    register_module_override(
        "TensileFormationUpdate",
        ModuleType::Behavior,
        tensile_formation_update_module_factory,
        tensile_formation_update_module_data_factory,
    )?;

    register_module_override(
        "GenerateMinefieldBehavior",
        ModuleType::Behavior,
        generate_minefield_behavior_module_factory,
        generate_minefield_behavior_module_data_factory,
    )?;

    register_module_override(
        "SpecialAbilityUpdate",
        ModuleType::Behavior,
        special_ability_update_module_factory,
        special_ability_update_module_data_factory,
    )?;

    register_module_override(
        "SpectreGunshipUpdate",
        ModuleType::Behavior,
        spectre_gunship_update_module_factory,
        spectre_gunship_update_module_data_factory,
    )?;

    register_module_override(
        "SpectreGunshipDeploymentUpdate",
        ModuleType::Behavior,
        spectre_gunship_deployment_update_module_factory,
        spectre_gunship_deployment_update_module_data_factory,
    )?;

    register_module_override(
        "ParticleUplinkCannonUpdate",
        ModuleType::Behavior,
        particle_uplink_cannon_update_module_factory,
        particle_uplink_cannon_update_module_data_factory,
    )?;

    register_module_override(
        "BattlePlanUpdate",
        ModuleType::Behavior,
        battle_plan_update_module_factory,
        battle_plan_update_module_data_factory,
    )?;

    register_module_override(
        "LifetimeUpdate",
        ModuleType::Behavior,
        lifetime_update_module_factory,
        lifetime_update_module_data_factory,
    )?;

    register_module_override(
        "MissileLauncherBuildingUpdate",
        ModuleType::Behavior,
        missile_launcher_building_update_module_factory,
        missile_launcher_building_update_module_data_factory,
    )?;

    register_module_override(
        "SpyVisionUpdate",
        ModuleType::Behavior,
        spy_vision_update_module_factory,
        spy_vision_update_module_data_factory,
    )?;

    register_module_override(
        "FireWeaponWhenDeadBehavior",
        ModuleType::Behavior,
        fire_weapon_when_dead_behavior_module_factory,
        fire_weapon_when_dead_behavior_module_data_factory,
    )?;

    register_module_override(
        "FireWeaponWhenDamagedBehavior",
        ModuleType::Behavior,
        fire_weapon_when_damaged_behavior_module_factory,
        fire_weapon_when_damaged_behavior_module_data_factory,
    )?;

    register_module_override(
        "FireWeaponUpdate",
        ModuleType::Behavior,
        fire_weapon_update_module_factory,
        fire_weapon_update_module_data_factory,
    )?;

    register_module_override(
        "FireOCLAfterWeaponCooldownUpdate",
        ModuleType::Behavior,
        fire_ocl_after_weapon_cooldown_update_module_factory,
        fire_ocl_after_weapon_cooldown_update_module_data_factory,
    )?;

    register_module_override(
        "WeaponBonusUpdate",
        ModuleType::Behavior,
        weapon_bonus_update_module_factory,
        weapon_bonus_update_module_data_factory,
    )?;

    register_module_override(
        "EMPUpdate",
        ModuleType::Behavior,
        emp_update_module_factory,
        emp_update_module_data_factory,
    )?;

    register_module_override(
        "StructureCollapseUpdate",
        ModuleType::Behavior,
        structure_collapse_update_module_factory,
        structure_collapse_update_module_data_factory,
    )?;

    register_module_override(
        "FloatUpdate",
        ModuleType::Behavior,
        float_update_module_factory,
        float_update_module_data_factory,
    )?;

    register_module_override(
        "EnemyNearUpdate",
        ModuleType::Behavior,
        enemy_near_update_module_factory,
        enemy_near_update_module_data_factory,
    )?;

    register_module_override(
        "AutoFindHealingUpdate",
        ModuleType::Behavior,
        auto_find_healing_update_module_factory,
        auto_find_healing_update_module_data_factory,
    )?;

    register_module_override(
        "SupplyWarehouseCripplingBehavior",
        ModuleType::Behavior,
        supply_warehouse_crippling_behavior_module_factory,
        supply_warehouse_crippling_behavior_module_data_factory,
    )?;

    register_module_override(
        "BaseRegenerateUpdate",
        ModuleType::Behavior,
        base_regenerate_update_module_factory,
        base_regenerate_update_module_data_factory,
    )?;

    register_module_override(
        "AutoDepositUpdate",
        ModuleType::Behavior,
        auto_deposit_update_module_factory,
        auto_deposit_update_module_data_factory,
    )?;

    register_module_override(
        "PowerPlantUpdate",
        ModuleType::Behavior,
        power_plant_update_module_factory,
        power_plant_update_module_data_factory,
    )?;

    register_module_override(
        "TechBuildingBehavior",
        ModuleType::Behavior,
        tech_building_behavior_module_factory,
        tech_building_behavior_module_data_factory,
    )?;

    register_module_override(
        "PropagandaTowerBehavior",
        ModuleType::Behavior,
        propaganda_tower_behavior_module_factory,
        propaganda_tower_behavior_module_data_factory,
    )?;

    register_module_override(
        "AssistedTargetingUpdate",
        ModuleType::Behavior,
        assisted_targeting_update_module_factory,
        assisted_targeting_update_module_data_factory,
    )?;

    register_module_override(
        "DynamicShroudClearingRangeUpdate",
        ModuleType::Behavior,
        dynamic_shroud_clearing_range_update_module_factory,
        dynamic_shroud_clearing_range_update_module_data_factory,
    )?;

    register_module_override(
        "CleanupHazardUpdate",
        ModuleType::Behavior,
        cleanup_hazard_update_module_factory,
        cleanup_hazard_update_module_data_factory,
    )?;

    register_module_override(
        "FireSpreadUpdate",
        ModuleType::Behavior,
        fire_spread_update_module_factory,
        fire_spread_update_module_data_factory,
    )?;

    register_module_override(
        "CommandButtonHuntUpdate",
        ModuleType::Behavior,
        command_button_hunt_update_module_factory,
        command_button_hunt_update_module_data_factory,
    )?;

    register_module_override(
        "SlavedUpdate",
        ModuleType::Behavior,
        slaved_update_module_factory,
        slaved_update_module_data_factory,
    )?;

    register_module_override(
        "MobMemberSlavedUpdate",
        ModuleType::Behavior,
        mob_member_slaved_update_module_factory,
        mob_member_slaved_update_module_data_factory,
    )?;

    register_module_override(
        "AIUpdateInterface",
        ModuleType::Behavior,
        ai_update_interface_module_factory,
        ai_update_interface_module_data_factory,
    )?;

    register_module_override(
        "TransportAIUpdate",
        ModuleType::Behavior,
        transport_ai_update_module_factory,
        transport_ai_update_module_data_factory,
    )?;

    register_module_override(
        "DeployStyleAIUpdate",
        ModuleType::Behavior,
        deploy_style_ai_update_module_factory,
        deploy_style_ai_update_module_data_factory,
    )?;

    register_module_override(
        "WanderAIUpdate",
        ModuleType::Behavior,
        wander_ai_update_module_factory,
        wander_ai_update_module_data_factory,
    )?;

    register_module_override(
        "JetAIUpdate",
        ModuleType::Behavior,
        jet_ai_update_module_factory,
        jet_ai_update_module_data_factory,
    )?;

    register_module_override(
        "RailedTransportAIUpdate",
        ModuleType::Behavior,
        railed_transport_ai_update_module_factory,
        railed_transport_ai_update_module_data_factory,
    )?;

    register_module_override(
        "RailroadBehavior",
        ModuleType::Behavior,
        railroad_behavior_module_factory,
        railroad_behavior_module_data_factory,
    )?;

    register_module_override(
        "AssaultTransportAIUpdate",
        ModuleType::Behavior,
        assault_transport_ai_update_module_factory,
        assault_transport_ai_update_module_data_factory,
    )?;

    register_module_override(
        "DeliverPayloadAIUpdate",
        ModuleType::Behavior,
        deliver_payload_ai_update_module_factory,
        deliver_payload_ai_update_module_data_factory,
    )?;

    register_module_override(
        "HackInternetAIUpdate",
        ModuleType::Behavior,
        hack_internet_ai_update_module_factory,
        hack_internet_ai_update_module_data_factory,
    )?;

    register_module_override(
        "SupplyTruckAIUpdate",
        ModuleType::Behavior,
        supply_truck_ai_update_module_factory,
        supply_truck_ai_update_module_data_factory,
    )?;

    register_module_override(
        "ChinookAIUpdate",
        ModuleType::Behavior,
        chinook_ai_update_module_factory,
        chinook_ai_update_module_data_factory,
    )?;

    register_module_override(
        "WorkerAIUpdate",
        ModuleType::Behavior,
        worker_ai_update_module_factory,
        worker_ai_update_module_data_factory,
    )?;

    register_module_override(
        "DozerAIUpdate",
        ModuleType::Behavior,
        dozer_ai_update_module_factory,
        dozer_ai_update_module_data_factory,
    )?;

    #[cfg(feature = "allow_surrender")]
    register_module_override(
        "POWTruckAIUpdate",
        ModuleType::Behavior,
        pow_truck_ai_update_module_factory,
        pow_truck_ai_update_module_data_factory,
    )?;

    register_module_override(
        "BridgeScaffoldBehavior",
        ModuleType::Behavior,
        bridge_scaffold_behavior_module_factory,
        bridge_scaffold_behavior_module_data_factory,
    )?;

    register_module_override(
        "BridgeTowerBehavior",
        ModuleType::Behavior,
        bridge_tower_behavior_module_factory,
        bridge_tower_behavior_module_data_factory,
    )?;

    register_module_override(
        "BridgeBehavior",
        ModuleType::Behavior,
        bridge_behavior_module_factory,
        bridge_behavior_module_data_factory,
    )?;

    register_module_override(
        "CountermeasuresBehavior",
        ModuleType::Behavior,
        countermeasures_behavior_module_factory,
        countermeasures_behavior_module_data_factory,
    )?;

    register_module_override(
        "BunkerBusterBehavior",
        ModuleType::Behavior,
        bunker_buster_behavior_module_factory,
        bunker_buster_behavior_module_data_factory,
    )?;

    register_module_override(
        "FlightDeckBehavior",
        ModuleType::Behavior,
        flight_deck_behavior_module_factory,
        flight_deck_behavior_module_data_factory,
    )?;

    register_module_override(
        "ParkingPlaceBehavior",
        ModuleType::Behavior,
        parking_place_behavior_module_factory,
        parking_place_behavior_module_data_factory,
    )?;

    register_module_override(
        "BattleBusSlowDeathBehavior",
        ModuleType::Behavior,
        battle_bus_slow_death_behavior_module_factory,
        battle_bus_slow_death_behavior_module_data_factory,
    )?;

    register_module_override(
        "DumbProjectileBehavior",
        ModuleType::Behavior,
        dumb_projectile_behavior_module_factory,
        dumb_projectile_behavior_module_data_factory,
    )?;

    register_module_override(
        "AutoHealBehavior",
        ModuleType::Behavior,
        auto_heal_behavior_module_factory,
        auto_heal_behavior_module_data_factory,
    )?;

    register_module_override(
        "HordeUpdate",
        ModuleType::Behavior,
        horde_update_module_factory,
        horde_update_module_data_factory,
    )?;

    register_module_override(
        "RadarUpdate",
        ModuleType::Behavior,
        radar_update_module_factory,
        radar_update_module_data_factory,
    )?;

    register_module_override(
        "SpawnBehavior",
        ModuleType::Behavior,
        spawn_behavior_module_factory,
        spawn_behavior_module_data_factory,
    )?;

    register_module_override(
        "StealthDetectorUpdate",
        ModuleType::Behavior,
        stealth_detector_update_module_factory,
        stealth_detector_update_module_data_factory,
    )?;

    register_module_override(
        "RadiusDecalUpdate",
        ModuleType::Behavior,
        radius_decal_update_module_factory,
        radius_decal_update_module_data_factory,
    )?;

    register_module_override(
        "ToppleUpdate",
        ModuleType::Behavior,
        topple_update_module_factory,
        topple_update_module_data_factory,
    )?;

    register_module_override(
        "StructureToppleUpdate",
        ModuleType::Behavior,
        structure_topple_update_module_factory,
        structure_topple_update_module_data_factory,
    )?;

    register_module_override(
        "FiringTracker",
        ModuleType::Behavior,
        firing_tracker_behavior_module_factory,
        firing_tracker_behavior_module_data_factory,
    )?;

    register_module_override(
        "OverchargeBehavior",
        ModuleType::Behavior,
        overcharge_behavior_module_factory,
        overcharge_behavior_module_data_factory,
    )?;

    register_module_override(
        "RebuildHoleBehavior",
        ModuleType::Behavior,
        rebuild_hole_behavior_module_factory,
        rebuild_hole_behavior_module_data_factory,
    )?;

    register_module_override(
        "QueueProductionExitUpdate",
        ModuleType::Behavior,
        queue_production_exit_behavior_module_factory,
        queue_production_exit_behavior_module_data_factory,
    )?;

    register_module_override(
        "DefaultProductionExitUpdate",
        ModuleType::Behavior,
        default_production_exit_behavior_module_factory,
        default_production_exit_behavior_module_data_factory,
    )?;

    register_module_override(
        "SpawnPointProductionExitUpdate",
        ModuleType::Behavior,
        spawn_point_production_exit_behavior_module_factory,
        spawn_point_production_exit_behavior_module_data_factory,
    )?;

    register_module_override(
        "SupplyCenterProductionExitUpdate",
        ModuleType::Behavior,
        supply_center_production_exit_behavior_module_factory,
        supply_center_production_exit_behavior_module_data_factory,
    )?;

    register_module_override(
        "RepairDockUpdate",
        ModuleType::Behavior,
        repair_dock_update_module_factory,
        repair_dock_update_module_data_factory,
    )?;

    #[cfg(feature = "allow_surrender")]
    register_module_override(
        "PrisonDockUpdate",
        ModuleType::Behavior,
        prison_dock_update_module_factory,
        prison_dock_update_module_data_factory,
    )?;

    register_module_override(
        "RailedTransportDockUpdate",
        ModuleType::Behavior,
        railed_transport_dock_update_module_factory,
        railed_transport_dock_update_module_data_factory,
    )?;

    register_module_override(
        "SupplyCenterDockUpdate",
        ModuleType::Behavior,
        supply_center_dock_update_module_factory,
        supply_center_dock_update_module_data_factory,
    )?;

    register_module_override(
        "SupplyWarehouseDockUpdate",
        ModuleType::Behavior,
        supply_warehouse_dock_update_module_factory,
        supply_warehouse_dock_update_module_data_factory,
    )?;

    #[cfg(feature = "allow_surrender")]
    register_module_override(
        "POWTruckBehavior",
        ModuleType::Behavior,
        pow_truck_behavior_module_factory,
        pow_truck_behavior_module_data_factory,
    )?;

    #[cfg(feature = "allow_surrender")]
    register_module_override(
        "PrisonBehavior",
        ModuleType::Behavior,
        prison_behavior_module_factory,
        prison_behavior_module_data_factory,
    )?;

    #[cfg(feature = "allow_surrender")]
    register_module_override(
        "PropagandaCenterBehavior",
        ModuleType::Behavior,
        propaganda_center_behavior_module_factory,
        propaganda_center_behavior_module_data_factory,
    )?;

    // ========================================================================
    // Additional Missing Module Registrations
    // ========================================================================

    register_module_override(
        "SlowDeathBehavior",
        ModuleType::Behavior,
        slow_death_behavior_module_factory,
        slow_death_behavior_module_data_factory,
    )?;

    register_module_override(
        "MinefieldBehavior",
        ModuleType::Behavior,
        minefield_behavior_module_factory,
        minefield_behavior_module_data_factory,
    )?;

    register_module_override(
        "GrantStealthBehavior",
        ModuleType::Behavior,
        grant_stealth_behavior_module_factory,
        grant_stealth_behavior_module_data_factory,
    )?;

    register_module_override(
        "PhysicsUpdate",
        ModuleType::Behavior,
        physics_update_module_factory,
        physics_update_module_data_factory,
    )?;

    register_module_override(
        "HeightDieUpdate",
        ModuleType::Behavior,
        height_die_update_module_factory,
        height_die_update_module_data_factory,
    )?;

    register_module_override(
        "DeletionUpdate",
        ModuleType::Behavior,
        deletion_update_module_factory,
        deletion_update_module_data_factory,
    )?;

    register_module_override(
        "WaveGuideUpdate",
        ModuleType::Behavior,
        wave_guide_update_module_factory,
        wave_guide_update_module_data_factory,
    )?;

    register_module_override(
        "CheckpointUpdate",
        ModuleType::Behavior,
        checkpoint_update_module_factory,
        checkpoint_update_module_data_factory,
    )?;

    register_module_override(
        "AnimationSteeringUpdate",
        ModuleType::Behavior,
        animation_steering_update_module_factory,
        animation_steering_update_module_data_factory,
    )?;

    register_module_override(
        "PilotFindVehicleUpdate",
        ModuleType::Behavior,
        pilot_find_vehicle_update_module_factory,
        pilot_find_vehicle_update_module_data_factory,
    )?;

    register_module_override(
        "HijackerUpdate",
        ModuleType::Behavior,
        hijacker_update_module_factory,
        hijacker_update_module_data_factory,
    )?;

    register_module_override(
        "HelicopterSlowDeathBehavior",
        ModuleType::Behavior,
        helicopter_slow_death_behavior_module_factory,
        helicopter_slow_death_behavior_module_data_factory,
    )?;

    register_module_override(
        "NeutronMissileSlowDeathUpdate",
        ModuleType::Behavior,
        neutron_missile_slow_death_update_module_factory,
        neutron_missile_slow_death_update_module_data_factory,
    )?;

    register_module_override(
        "NeutronMissileUpdate",
        ModuleType::Behavior,
        neutron_missile_update_module_factory,
        neutron_missile_update_module_data_factory,
    )?;

    register_module_override(
        "FirestormDynamicGeometryInfoUpdate",
        ModuleType::Behavior,
        firestorm_dynamic_geometry_info_update_module_factory,
        firestorm_dynamic_geometry_info_update_module_data_factory,
    )?;

    register_module_override(
        "DynamicGeometryInfoUpdate",
        ModuleType::Behavior,
        dynamic_geometry_info_update_module_factory,
        dynamic_geometry_info_update_module_data_factory,
    )?;

    Ok(())
}

static MODULE_OVERRIDES_READY: OnceLock<Result<(), String>> = OnceLock::new();

pub fn ensure_module_overrides_installed() -> Result<(), String> {
    MODULE_OVERRIDES_READY
        .get_or_init(|| {
            install_module_overrides()?;
            apply_module_overrides_to_existing_templates()?;
            Ok(())
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_bits_upgrade_parses_status_lists() {
        let mut data = StatusBitsUpgradeModuleData::default();
        data.set_status_to_set_from_tokens(&["STEALTHED", "DETECTED"])
            .expect("set mask parsed");
        data.set_status_to_clear_from_tokens(&["+MASKED"])
            .expect("clear mask parsed");

        let set_mask = data.status_to_set();
        assert!(set_mask.contains(ObjectStatusMaskType::STEALTHED));
        assert!(set_mask.contains(ObjectStatusMaskType::DETECTED));
        let clear_mask = data.status_to_clear();
        assert!(clear_mask.contains(ObjectStatusMaskType::MASKED));
    }

    #[test]
    fn status_bits_upgrade_data_factory_sets_defaults() {
        let data = status_bits_upgrade_module_data_factory(None);
        let typed = data
            .as_ref()
            .downcast_ref::<StatusBitsUpgradeModuleData>()
            .unwrap();
        assert!(typed.status_to_set().is_empty());
        assert!(typed.status_to_clear().is_empty());
    }

    #[test]
    fn module_factory_uses_status_bits_override() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module_factory::ModuleFactory;

        install_module_overrides().expect("install overrides");

        let mut factory = ModuleFactory::new();
        let name = AsciiString::from("StatusBitsUpgrade");
        factory.add_module_internal(
            None,
            None,
            ModuleType::Behavior,
            &name,
            ModuleInterfaceType::UPGRADE.0 as i32,
        );

        let module_tag = AsciiString::from("TagStatusBits");
        let data = factory
            .new_module_data_from_ini(None, &name, ModuleType::Behavior, &module_tag)
            .expect("module data via override");

        #[derive(Debug)]
        struct StubThing {
            id: ObjectID,
        }

        impl ModuleObjectTrait for StubThing {
            fn get_object_id(&self) -> ObjectID {
                self.id
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                None
            }
        }

        impl ModuleThing for StubThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let thing: Arc<dyn ModuleThing> = Arc::new(StubThing { id: 99 });

        let module = factory
            .new_module(thing, &name, data.clone(), ModuleType::Behavior)
            .expect("module via override");

        assert!(data.as_ref().as_any().is::<StatusBitsUpgradeModuleData>());
        assert!(module
            .get_module_data()
            .as_any()
            .is::<StatusBitsUpgradeModuleData>());
    }

    #[test]
    fn stealth_update_data_parses_status_tokens() {
        let mut data = StealthUpdateModuleData::default();
        data.set_hint_detectable_states_from_tokens(&["STEALTHED", "DETECTED"])
            .expect("hint detectable parsed");
        data.set_required_status_from_tokens(&["CAN_STEALTH"])
            .expect("required parsed");
        data.set_forbidden_status_from_tokens(&["+MASKED"])
            .expect("forbidden parsed");

        assert!(data
            .hint_detectable_states()
            .contains(ObjectStatusMaskType::STEALTHED));
        assert!(data
            .hint_detectable_states()
            .contains(ObjectStatusMaskType::DETECTED));
        assert!(data
            .required_status()
            .contains(ObjectStatusMaskType::CAN_STEALTH));
        assert!(data
            .forbidden_status()
            .contains(ObjectStatusMaskType::MASKED));
    }

    #[test]
    fn status_bits_upgrade_factory_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;

        #[derive(Debug)]
        struct StubThing;

        impl ModuleObjectTrait for StubThing {
            fn get_object_id(&self) -> ObjectID {
                0
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                None
            }
        }

        impl ModuleThing for StubThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let mut data = StatusBitsUpgradeModuleData::default();
        data.set_status_to_set_from_tokens(&["STEALTHED"])
            .expect("status to set parsed");
        let module = status_bits_upgrade_module_factory(
            Arc::new(StubThing) as Arc<dyn ModuleThing>,
            Arc::new(data) as Arc<dyn ModuleData>,
        );

        let typed_data = module
            .get_module_data()
            .as_any()
            .downcast_ref::<StatusBitsUpgradeModuleData>()
            .expect("upgrade module data downcasts");
        assert!(typed_data
            .status_to_set()
            .contains(ObjectStatusMaskType::STEALTHED));
    }

    #[test]
    fn stealth_update_factory_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;

        #[derive(Debug)]
        struct StubThing;

        impl ModuleObjectTrait for StubThing {
            fn get_object_id(&self) -> ObjectID {
                0
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                None
            }
        }

        impl ModuleThing for StubThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let data = Arc::new(StealthUpdateModuleData::default());
        let module = stealth_update_module_factory(
            Arc::new(StubThing) as Arc<dyn ModuleThing>,
            data.clone() as Arc<dyn ModuleData>,
        );

        let typed_data = module
            .get_module_data()
            .as_any()
            .downcast_ref::<StealthUpdateModuleData>()
            .expect("stealth module data downcasts");
        assert_eq!(typed_data.required_status(), ObjectStatusMaskType::none());
    }

    #[test]
    fn auto_heal_override_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;
        use std::sync::RwLock;

        #[derive(Debug, Clone)]
        struct StubHealThing {
            id: ObjectID,
        }

        impl ModuleObjectTrait for StubHealThing {
            fn get_object_id(&self) -> ObjectID {
                self.id
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                let arc: Arc<RwLock<StubHealThing>> = Arc::new(RwLock::new(self.clone()));
                Some(arc as Arc<RwLock<dyn ModuleObjectTrait>>)
            }
        }

        impl ModuleThing for StubHealThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let thing: Arc<dyn ModuleThing> =
            Arc::new(StubHealThing { id: 777 }) as Arc<dyn ModuleThing>;

        let data_box = auto_heal_behavior_module_data_factory(None);
        let module_data: Arc<dyn ModuleData> = data_box.into();

        let module = auto_heal_behavior_module_factory(thing, module_data);
        assert!(
            module
                .get_module_data()
                .as_any()
                .downcast_ref::<AutoHealBehaviorModuleData>()
                .is_some(),
            "AutoHeal override should return typed module data"
        );
    }

    #[test]
    fn dumb_projectile_override_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;
        use std::sync::RwLock;

        #[derive(Debug, Clone)]
        struct StubProjectileThing {
            id: ObjectID,
        }

        impl ModuleObjectTrait for StubProjectileThing {
            fn get_object_id(&self) -> ObjectID {
                self.id
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                let arc: Arc<RwLock<StubProjectileThing>> = Arc::new(RwLock::new(self.clone()));
                Some(arc as Arc<RwLock<dyn ModuleObjectTrait>>)
            }
        }

        impl ModuleThing for StubProjectileThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let thing: Arc<dyn ModuleThing> =
            Arc::new(StubProjectileThing { id: 456 }) as Arc<dyn ModuleThing>;

        let data_box = dumb_projectile_behavior_module_data_factory(None);
        let module_data: Arc<dyn ModuleData> = data_box.into();

        let module = dumb_projectile_behavior_module_factory(thing, module_data);
        assert!(
            module
                .get_module_data()
                .as_any()
                .downcast_ref::<DumbProjectileBehaviorModuleData>()
                .is_some(),
            "DumbProjectile override should return typed module data"
        );
    }

    #[test]
    fn countermeasures_override_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;
        use std::sync::RwLock;

        #[derive(Debug, Clone)]
        struct StubCounterThing {
            id: ObjectID,
        }

        impl ModuleObjectTrait for StubCounterThing {
            fn get_object_id(&self) -> ObjectID {
                self.id
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                let arc: Arc<RwLock<StubCounterThing>> = Arc::new(RwLock::new(self.clone()));
                Some(arc as Arc<RwLock<dyn ModuleObjectTrait>>)
            }
        }

        impl ModuleThing for StubCounterThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let thing: Arc<dyn ModuleThing> =
            Arc::new(StubCounterThing { id: 123 }) as Arc<dyn ModuleThing>;

        let data_box = countermeasures_behavior_module_data_factory(None);
        let module_data: Arc<dyn ModuleData> = data_box.into();

        let module = countermeasures_behavior_module_factory(thing, module_data);
        assert!(
            module
                .get_module_data()
                .as_any()
                .downcast_ref::<CountermeasuresBehaviorModuleData>()
                .is_some(),
            "Countermeasures override should return typed module data"
        );
    }

    #[test]
    fn bunker_buster_override_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;
        use std::sync::RwLock;

        #[derive(Debug, Clone)]
        struct StubBunkerThing {
            id: ObjectID,
        }

        impl ModuleObjectTrait for StubBunkerThing {
            fn get_object_id(&self) -> ObjectID {
                self.id
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                let arc: Arc<RwLock<StubBunkerThing>> = Arc::new(RwLock::new(self.clone()));
                Some(arc as Arc<RwLock<dyn ModuleObjectTrait>>)
            }
        }

        impl ModuleThing for StubBunkerThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let thing: Arc<dyn ModuleThing> =
            Arc::new(StubBunkerThing { id: 456 }) as Arc<dyn ModuleThing>;

        let data_box = bunker_buster_behavior_module_data_factory(None);
        let module_data: Arc<dyn ModuleData> = data_box.into();

        let module = bunker_buster_behavior_module_factory(thing, module_data);
        assert!(
            module
                .get_module_data()
                .as_any()
                .downcast_ref::<BunkerBusterBehaviorModuleData>()
                .is_some(),
            "BunkerBuster override should return typed module data"
        );
    }
}
#[test]
fn battle_bus_slow_death_override_produces_concrete_module() {
    use crate::common::ObjectID;
    use game_engine::common::thing::module::ModuleData;
    use std::sync::RwLock;

    #[derive(Debug, Clone)]
    struct StubBusThing {
        id: ObjectID,
    }

    impl ModuleObjectTrait for StubBusThing {
        fn get_object_id(&self) -> ObjectID {
            self.id
        }

        fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
            let arc: Arc<RwLock<StubBusThing>> = Arc::new(RwLock::new(self.clone()));
            Some(arc as Arc<RwLock<dyn ModuleObjectTrait>>)
        }
    }

    impl ModuleThing for StubBusThing {
        fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
            Some(self)
        }

        fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
            None
        }
    }

    let thing: Arc<dyn ModuleThing> = Arc::new(StubBusThing { id: 321 }) as Arc<dyn ModuleThing>;

    let data_box = battle_bus_slow_death_behavior_module_data_factory(None);
    let module_data: Arc<dyn ModuleData> = data_box.into();

    let module = battle_bus_slow_death_behavior_module_factory(thing, module_data);
    assert!(
        module
            .get_module_data()
            .as_any()
            .downcast_ref::<BattleBusSlowDeathBehaviorModuleData>()
            .is_some(),
        "BattleBusSlowDeath override should return typed module data"
    );
}
