//! Memory initialization and pool configuration
//!
//! This module provides memory pool initialization functionality similar to
//! the C++ MemoryInit.cpp file. It defines default pool sizes and configuration
//! for the memory management system.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Pool initialization record structure
#[derive(Debug, Clone)]
pub struct PoolInitRec {
    /// Name of the memory pool
    pub name: String,
    /// Size of individual allocations
    pub alloc_size: usize,
    /// Initial number of objects to allocate
    pub initial_count: usize,
    /// Number of objects to allocate when pool overflows
    pub overflow_count: usize,
}

impl PoolInitRec {
    /// Create a new pool initialization record
    pub fn new(name: &str, alloc_size: usize, initial_count: usize, overflow_count: usize) -> Self {
        Self {
            name: name.to_string(),
            alloc_size,
            initial_count,
            overflow_count,
        }
    }
}

/// Memory pool size record for configuration
#[derive(Debug, Clone)]
pub struct PoolSizeRec {
    /// Pool name
    pub name: String,
    /// Initial allocation count
    pub initial: usize,
    /// Overflow allocation count
    pub overflow: usize,
}

impl PoolSizeRec {
    /// Create a new pool size record
    pub fn new(name: &str, initial: usize, overflow: usize) -> Self {
        Self {
            name: name.to_string(),
            initial,
            overflow,
        }
    }
}

/// Default DMA pool parameters
///
/// These correspond to the defaultDMA array in the C++ code.
pub fn get_default_dma_params() -> Vec<PoolInitRec> {
    vec![
        PoolInitRec::new("dmaPool_16", 16, 130000, 10000),
        PoolInitRec::new("dmaPool_32", 32, 250000, 10000),
        PoolInitRec::new("dmaPool_64", 64, 100000, 10000),
        PoolInitRec::new("dmaPool_128", 128, 80000, 10000),
        PoolInitRec::new("dmaPool_256", 256, 20000, 5000),
        PoolInitRec::new("dmaPool_512", 512, 16000, 5000),
        PoolInitRec::new("dmaPool_1024", 1024, 6000, 1024),
    ]
}

/// Default pool sizes for game objects
///
/// This corresponds to the sizes array in the C++ MemoryInit.cpp file.
/// It includes pool sizes for various game object types.
pub fn get_default_pool_sizes() -> Vec<PoolSizeRec> {
    vec![
        PoolSizeRec::new("PartitionContactListNode", 2048, 512),
        PoolSizeRec::new("BattleshipUpdate", 32, 32),
        PoolSizeRec::new("FlyToDestAndDestroyUpdate", 32, 32),
        PoolSizeRec::new("MusicTrack", 32, 32),
        PoolSizeRec::new("PositionalSoundPool", 32, 32),
        PoolSizeRec::new("GameMessage", 2048, 32),
        PoolSizeRec::new("NameKeyBucketPool", 9000, 1024),
        PoolSizeRec::new("ObjectSellInfo", 16, 16),
        PoolSizeRec::new("ProductionPrerequisitePool", 1024, 32),
        PoolSizeRec::new("RadarObject", 512, 32),
        PoolSizeRec::new("ResourceGatheringManager", 16, 16),
        PoolSizeRec::new("SightingInfo", 8192, 2048),
        PoolSizeRec::new("SpecialPowerTemplate", 84, 32),
        PoolSizeRec::new("StateMachinePool", 32, 32),
        PoolSizeRec::new("TeamPool", 128, 32),
        PoolSizeRec::new("PlayerRelationMapPool", 128, 32),
        PoolSizeRec::new("TeamRelationMapPool", 128, 32),
        PoolSizeRec::new("TeamPrototypePool", 256, 32),
        PoolSizeRec::new("TerrainType", 256, 32),
        PoolSizeRec::new("ThingTemplatePool", 2120, 32),
        PoolSizeRec::new("TunnelTracker", 16, 16),
        PoolSizeRec::new("Upgrade", 16, 16),
        PoolSizeRec::new("UpgradeTemplate", 128, 16),
        PoolSizeRec::new("Anim2D", 32, 32),
        PoolSizeRec::new("CommandButton", 1024, 256),
        PoolSizeRec::new("CommandSet", 820, 16),
        PoolSizeRec::new("DisplayString", 32, 32),
        PoolSizeRec::new("WebBrowserURL", 16, 16),
        PoolSizeRec::new("Drawable", 4096, 32),
        PoolSizeRec::new("Image", 2048, 32),
        PoolSizeRec::new("ParticlePool", 1400, 1024),
        PoolSizeRec::new("ParticleSystemTemplatePool", 1100, 32),
        PoolSizeRec::new("ParticleSystemPool", 1024, 32),
        PoolSizeRec::new("TerrainRoadType", 100, 32),
        PoolSizeRec::new("WindowLayoutPool", 32, 32),
        PoolSizeRec::new("AnimatedParticleSysBoneClientUpdate", 16, 16),
        PoolSizeRec::new("SwayClientUpdate", 32, 32),
        PoolSizeRec::new("BeaconClientUpdate", 64, 32),
        PoolSizeRec::new("AIGroupPool", 64, 32),
        PoolSizeRec::new("AIDockMachinePool", 256, 32),
        PoolSizeRec::new("AIGuardMachinePool", 32, 32),
        PoolSizeRec::new("AIGuardRetaliateMachinePool", 32, 32),
        PoolSizeRec::new("AITNGuardMachinePool", 32, 32),
        PoolSizeRec::new("PathNodePool", 8192, 1024),
        PoolSizeRec::new("PathPool", 256, 16),
        PoolSizeRec::new("WorkOrder", 32, 32),
        PoolSizeRec::new("TeamInQueue", 32, 32),
        PoolSizeRec::new("AIPlayer", 12, 4),
        PoolSizeRec::new("AISkirmishPlayer", 8, 8),
        PoolSizeRec::new("AIStateMachine", 600, 32),
        PoolSizeRec::new("JetAIStateMachine", 64, 32),
        PoolSizeRec::new("HeliAIStateMachine", 64, 32),
        PoolSizeRec::new("AIAttackMoveStateMachine", 2048, 32),
        PoolSizeRec::new("AIAttackThenIdleStateMachine", 512, 32),
        PoolSizeRec::new("AttackStateMachine", 512, 32),
        PoolSizeRec::new("CrateTemplate", 32, 32),
        PoolSizeRec::new("ExperienceTrackerPool", 2048, 512),
        PoolSizeRec::new("FiringTrackerPool", 4096, 256),
        PoolSizeRec::new("ObjectRepulsorHelper", 1024, 256),
        PoolSizeRec::new("ObjectSMCHelperPool", 2048, 256),
        PoolSizeRec::new("ObjectWeaponStatusHelperPool", 4096, 256),
        PoolSizeRec::new("ObjectDefectionHelperPool", 2048, 256),
        PoolSizeRec::new("StatusDamageHelper", 1500, 256),
        PoolSizeRec::new("SubdualDamageHelper", 1500, 256),
        PoolSizeRec::new("TempWeaponBonusHelper", 4096, 256),
        PoolSizeRec::new("Locomotor", 2048, 32),
        PoolSizeRec::new("LocomotorTemplate", 192, 32),
        PoolSizeRec::new("ObjectPool", 1500, 256),
        PoolSizeRec::new("SimpleObjectIteratorPool", 32, 32),
        PoolSizeRec::new("SimpleObjectIteratorClumpPool", 4096, 32),
        PoolSizeRec::new("PartitionDataPool", 2048, 512),
        PoolSizeRec::new("BuildEntry", 32, 32),
        PoolSizeRec::new("Weapon", 4096, 32),
        PoolSizeRec::new("WeaponTemplate", 360, 32),
        PoolSizeRec::new("AIUpdateInterface", 600, 32),
        PoolSizeRec::new("ActiveBody", 1024, 32),
        PoolSizeRec::new("ActiveShroudUpgrade", 32, 32),
        PoolSizeRec::new("AssistedTargetingUpdate", 32, 32),
        PoolSizeRec::new("AudioEventInfo", 4096, 64),
        PoolSizeRec::new("AudioRequest", 256, 8),
        PoolSizeRec::new("AutoHealBehavior", 1024, 256),
        PoolSizeRec::new("WeaponBonusUpdate", 16, 16),
        PoolSizeRec::new("GrantStealthBehavior", 4096, 32),
        PoolSizeRec::new("NeutronBlastBehavior", 4096, 32),
        PoolSizeRec::new("CountermeasuresBehavior", 256, 32),
        PoolSizeRec::new("BaseRegenerateUpdate", 128, 32),
        PoolSizeRec::new("BoneFXDamage", 64, 32),
        PoolSizeRec::new("BoneFXUpdate", 64, 32),
        PoolSizeRec::new("BridgeBehavior", 4, 4),
        PoolSizeRec::new("BridgeTowerBehavior", 32, 32),
        PoolSizeRec::new("BridgeScaffoldBehavior", 32, 32),
        PoolSizeRec::new("CaveContain", 16, 16),
        PoolSizeRec::new("HealContain", 32, 32),
        PoolSizeRec::new("CreateCrateDie", 256, 128),
        PoolSizeRec::new("CreateObjectDie", 1024, 32),
        PoolSizeRec::new("EjectPilotDie", 1024, 32),
        PoolSizeRec::new("CrushDie", 1024, 32),
        PoolSizeRec::new("DamDie", 8, 8),
        PoolSizeRec::new("DeliverPayloadStateMachine", 32, 32),
        PoolSizeRec::new("DeliverPayloadAIUpdate", 32, 32),
        PoolSizeRec::new("DeletionUpdate", 128, 32),
        PoolSizeRec::new("SmartBombTargetHomingUpdate", 8, 8),
        PoolSizeRec::new("DynamicAudioEventInfo", 16, 256),
        PoolSizeRec::new("HackInternetStateMachine", 32, 32),
        PoolSizeRec::new("HackInternetAIUpdate", 32, 32),
        PoolSizeRec::new("MissileAIUpdate", 512, 32),
        PoolSizeRec::new("DumbProjectileBehavior", 64, 32),
        PoolSizeRec::new("DestroyDie", 1024, 32),
        PoolSizeRec::new("UpgradeDie", 128, 32),
        PoolSizeRec::new("KeepObjectDie", 128, 32),
        PoolSizeRec::new("DozerAIUpdate", 32, 32),
        PoolSizeRec::new("DynamicGeometryInfoUpdate", 16, 16),
        PoolSizeRec::new("DynamicShroudClearingRangeUpdate", 128, 16),
        PoolSizeRec::new("FXListDie", 1024, 32),
        PoolSizeRec::new("FireSpreadUpdate", 2048, 128),
        PoolSizeRec::new("FirestormDynamicGeometryInfoUpdate", 16, 16),
        PoolSizeRec::new("FireWeaponCollide", 2048, 32),
        PoolSizeRec::new("FireWeaponUpdate", 32, 32),
        PoolSizeRec::new("FlammableUpdate", 512, 256),
        PoolSizeRec::new("FloatUpdate", 512, 128),
        PoolSizeRec::new("TensileFormationUpdate", 256, 32),
        PoolSizeRec::new("GarrisonContain", 256, 32),
        PoolSizeRec::new("HealCrateCollide", 32, 32),
        PoolSizeRec::new("HeightDieUpdate", 32, 32),
        PoolSizeRec::new("FireWeaponWhenDamagedBehavior", 32, 32),
        PoolSizeRec::new("FireWeaponWhenDeadBehavior", 128, 64),
        PoolSizeRec::new("GenerateMinefieldBehavior", 32, 32),
        PoolSizeRec::new("HelicopterSlowDeathBehavior", 64, 32),
        PoolSizeRec::new("ParkingPlaceBehavior", 32, 32),
        PoolSizeRec::new("FlightDeckBehavior", 8, 8),
        PoolSizeRec::new("POWTruckAIUpdate", 32, 32),
        PoolSizeRec::new("POWTruckBehavior", 32, 32),
        PoolSizeRec::new("PrisonBehavior", 32, 32),
        PoolSizeRec::new("PrisonVisual", 32, 32),
        PoolSizeRec::new("PropagandaCenterBehavior", 16, 16),
        PoolSizeRec::new("PropagandaTowerBehavior", 16, 16),
        PoolSizeRec::new("BunkerBusterBehavior", 16, 16),
        PoolSizeRec::new("ObjectTracker", 128, 32),
        PoolSizeRec::new("OCLUpdate", 16, 16),
        PoolSizeRec::new("BodyParticleSystem", 196, 64),
        PoolSizeRec::new("HighlanderBody", 2048, 128),
        PoolSizeRec::new("UndeadBody", 32, 32),
        PoolSizeRec::new("HordeUpdate", 128, 32),
        PoolSizeRec::new("ImmortalBody", 128, 256),
        PoolSizeRec::new("InactiveBody", 2048, 32),
        PoolSizeRec::new("InstantDeathBehavior", 512, 32),
        PoolSizeRec::new("LaserUpdate", 32, 32),
        PoolSizeRec::new("PointDefenseLaserUpdate", 32, 32),
        PoolSizeRec::new("CleanupHazardUpdate", 32, 32),
        PoolSizeRec::new("AutoFindHealingUpdate", 256, 32),
        PoolSizeRec::new("CommandButtonHuntUpdate", 512, 8),
        PoolSizeRec::new("PilotFindVehicleUpdate", 256, 32),
        PoolSizeRec::new("DemoTrapUpdate", 32, 32),
        PoolSizeRec::new("ParticleUplinkCannonUpdate", 16, 16),
        PoolSizeRec::new("SpectreGunshipUpdate", 8, 8),
        PoolSizeRec::new("SpectreGunshipDeploymentUpdate", 8, 8),
        PoolSizeRec::new("BaikonurLaunchPower", 4, 4),
        PoolSizeRec::new("RadiusDecalUpdate", 16, 16),
        PoolSizeRec::new("BattlePlanUpdate", 32, 32),
        PoolSizeRec::new("LifetimeUpdate", 32, 32),
        PoolSizeRec::new("LocomotorSetUpgrade", 512, 128),
        PoolSizeRec::new("LockWeaponCreate", 64, 128),
        PoolSizeRec::new("AutoDepositUpdate", 256, 32),
        PoolSizeRec::new("NeutronMissileUpdate", 512, 32),
        PoolSizeRec::new("MoneyCrateCollide", 48, 16),
        PoolSizeRec::new("NeutronMissileSlowDeathBehavior", 8, 8),
        PoolSizeRec::new("OpenContain", 128, 32),
        PoolSizeRec::new("OverchargeBehavior", 32, 32),
        PoolSizeRec::new("OverlordContain", 32, 32),
        PoolSizeRec::new("HelixContain", 32, 32),
        PoolSizeRec::new("ParachuteContain", 128, 32),
        PoolSizeRec::new("PhysicsBehavior", 600, 32),
        PoolSizeRec::new("PoisonedBehavior", 512, 64),
        PoolSizeRec::new("ProductionEntry", 32, 32),
        PoolSizeRec::new("ProductionUpdate", 256, 32),
        PoolSizeRec::new("ProjectileStreamUpdate", 32, 32),
        PoolSizeRec::new("ProneUpdate", 128, 32),
        PoolSizeRec::new("QueueProductionExitUpdate", 32, 32),
        PoolSizeRec::new("RadarUpdate", 16, 16),
        PoolSizeRec::new("RadarUpgrade", 16, 16),
        PoolSizeRec::new("AnimationSteeringUpdate", 1024, 32),
        PoolSizeRec::new("SupplyWarehouseCripplingBehavior", 16, 16),
        PoolSizeRec::new("CostModifierUpgrade", 32, 32),
        PoolSizeRec::new("CashBountyPower", 32, 32),
        PoolSizeRec::new("CleanupAreaPower", 32, 32),
        PoolSizeRec::new("ObjectCreationUpgrade", 196, 32),
        PoolSizeRec::new("MinefieldBehavior", 256, 32),
        PoolSizeRec::new("JetSlowDeathBehavior", 64, 32),
        PoolSizeRec::new("BattleBusSlowDeathBehavior", 64, 32),
        PoolSizeRec::new("RebuildHoleBehavior", 64, 32),
        PoolSizeRec::new("RebuildHoleExposeDie", 64, 32),
        PoolSizeRec::new("RepairDockUpdate", 32, 32),
        PoolSizeRec::new("PrisonDockUpdate", 32, 32),
        PoolSizeRec::new("RailedTransportDockUpdate", 16, 16),
        PoolSizeRec::new("RailedTransportAIUpdate", 16, 16),
        PoolSizeRec::new("RailedTransportContain", 16, 16),
        PoolSizeRec::new("RailroadBehavior", 16, 16),
        PoolSizeRec::new("SalvageCrateCollide", 32, 32),
        PoolSizeRec::new("ShroudCrateCollide", 32, 32),
        PoolSizeRec::new("SlavedUpdate", 64, 32),
        PoolSizeRec::new("SlowDeathBehavior", 1400, 256),
        PoolSizeRec::new("SpyVisionUpdate", 16, 16),
        PoolSizeRec::new("DefaultProductionExitUpdate", 32, 32),
        PoolSizeRec::new("SpawnPointProductionExitUpdate", 32, 32),
        PoolSizeRec::new("SpawnBehavior", 32, 32),
        PoolSizeRec::new("SpecialPowerCompletionDie", 32, 32),
        PoolSizeRec::new("SpecialPowerCreate", 32, 32),
        PoolSizeRec::new("PreorderCreate", 32, 32),
        PoolSizeRec::new("SpecialAbility", 512, 32),
        PoolSizeRec::new("SpecialAbilityUpdate", 512, 32),
        PoolSizeRec::new("MissileLauncherBuildingUpdate", 32, 32),
        PoolSizeRec::new("SquishCollide", 512, 32),
        PoolSizeRec::new("StructureBody", 512, 64),
        PoolSizeRec::new("HiveStructureBody", 64, 32),
        PoolSizeRec::new("StructureCollapseUpdate", 32, 32),
        PoolSizeRec::new("StructureToppleUpdate", 32, 32),
        PoolSizeRec::new("SupplyCenterCreate", 32, 32),
        PoolSizeRec::new("SupplyCenterDockUpdate", 32, 32),
        PoolSizeRec::new("SupplyCenterProductionExitUpdate", 32, 32),
        PoolSizeRec::new("SupplyTruckStateMachine", 256, 32),
        PoolSizeRec::new("SupplyTruckAIUpdate", 32, 32),
        PoolSizeRec::new("SupplyWarehouseCreate", 48, 16),
        PoolSizeRec::new("SupplyWarehouseDockUpdate", 48, 16),
        PoolSizeRec::new("EnemyNearUpdate", 1024, 32),
        PoolSizeRec::new("TechBuildingBehavior", 32, 32),
        PoolSizeRec::new("ToppleUpdate", 256, 128),
        PoolSizeRec::new("TransitionDamageFX", 384, 128),
        PoolSizeRec::new("TransportAIUpdate", 64, 32),
        PoolSizeRec::new("TransportContain", 128, 32),
        PoolSizeRec::new("RiderChangeContain", 128, 32),
        PoolSizeRec::new("InternetHackContain", 16, 16),
        PoolSizeRec::new("TunnelContain", 8, 8),
        PoolSizeRec::new("TunnelContainDie", 32, 32),
        PoolSizeRec::new("TunnelCreate", 32, 32),
        PoolSizeRec::new("TurretAI", 256, 32),
        PoolSizeRec::new("TurretStateMachine", 128, 32),
        PoolSizeRec::new("TurretSwapUpgrade", 512, 128),
        PoolSizeRec::new("UnitCrateCollide", 32, 32),
        PoolSizeRec::new("UnpauseSpecialPowerUpgrade", 32, 32),
        PoolSizeRec::new("VeterancyCrateCollide", 32, 32),
        PoolSizeRec::new("VeterancyGainCreate", 512, 128),
        PoolSizeRec::new("ConvertToCarBombCrateCollide", 256, 128),
        PoolSizeRec::new("ConvertToHijackedVehicleCrateCollide", 256, 128),
        PoolSizeRec::new("SabotageCommandCenterCrateCollide", 256, 128),
        PoolSizeRec::new("SabotageFakeBuildingCrateCollide", 256, 128),
        PoolSizeRec::new("SabotageInternetCenterCrateCollide", 256, 128),
        PoolSizeRec::new("SabotageMilitaryFactoryCrateCollide", 256, 128),
        PoolSizeRec::new("SabotagePowerPlantCrateCollide", 256, 128),
        PoolSizeRec::new("SabotageSuperweaponCrateCollide", 256, 128),
        PoolSizeRec::new("SabotageSupplyCenterCrateCollide", 256, 128),
        PoolSizeRec::new("SabotageSupplyDropzoneCrateCollide", 256, 128),
        PoolSizeRec::new("JetAIUpdate", 64, 32),
        PoolSizeRec::new("ChinookAIUpdate", 32, 32),
        PoolSizeRec::new("WanderAIUpdate", 32, 32),
        PoolSizeRec::new("WaveGuideUpdate", 16, 16),
        PoolSizeRec::new("WeaponBonusUpgrade", 512, 128),
        PoolSizeRec::new("WeaponSetUpgrade", 512, 128),
        PoolSizeRec::new("ArmorUpgrade", 512, 128),
        PoolSizeRec::new("WorkerAIUpdate", 128, 128),
        PoolSizeRec::new("WorkerStateMachine", 128, 128),
        PoolSizeRec::new("ChinookAIStateMachine", 32, 32),
        PoolSizeRec::new("DeployStyleAIUpdate", 32, 32),
        PoolSizeRec::new("AssaultTransportAIUpdate", 64, 32),
        PoolSizeRec::new("StreamingArchiveFile", 8, 8),
        PoolSizeRec::new("DozerActionStateMachine", 256, 32),
        PoolSizeRec::new("DozerPrimaryStateMachine", 256, 32),
        PoolSizeRec::new("W3DDisplayString", 1400, 128),
        PoolSizeRec::new("W3DDefaultDraw", 1024, 128),
        PoolSizeRec::new("W3DDebrisDraw", 128, 128),
        PoolSizeRec::new("W3DDependencyModelDraw", 64, 64),
        PoolSizeRec::new("W3DLaserDraw", 32, 32),
        PoolSizeRec::new("W3DModelDraw", 2048, 512),
        PoolSizeRec::new("W3DOverlordTankDraw", 64, 64),
        PoolSizeRec::new("W3DOverlordTruckDraw", 64, 64),
        PoolSizeRec::new("W3DOverlordAircraftDraw", 64, 64),
        PoolSizeRec::new("W3DPoliceCarDraw", 32, 32),
        PoolSizeRec::new("W3DProjectileStreamDraw", 32, 32),
        PoolSizeRec::new("W3DRopeDraw", 32, 32),
        PoolSizeRec::new("W3DScienceModelDraw", 32, 32),
        PoolSizeRec::new("W3DSupplyDraw", 40, 16),
        PoolSizeRec::new("W3DTankDraw", 256, 32),
        PoolSizeRec::new("W3DTreeDraw", 16, 16),
        PoolSizeRec::new("W3DPropDraw", 16, 16),
        PoolSizeRec::new("W3DTracerDraw", 64, 32),
        PoolSizeRec::new("W3DTruckDraw", 128, 32),
        PoolSizeRec::new("W3DTankTruckDraw", 32, 16),
        PoolSizeRec::new("W3DTreeTextureClass", 4, 4),
        PoolSizeRec::new("DefaultSpecialPower", 32, 32),
        PoolSizeRec::new("OCLSpecialPower", 96, 32),
        PoolSizeRec::new("FireWeaponPower", 32, 32),
        PoolSizeRec::new("DemoralizeSpecialPower", 16, 16),
        PoolSizeRec::new("CashHackSpecialPower", 32, 32),
        PoolSizeRec::new("CommandSetUpgrade", 32, 32),
        PoolSizeRec::new("PassengersFireUpgrade", 32, 32),
        PoolSizeRec::new("GrantUpgradeCreate", 256, 32),
        PoolSizeRec::new("GrantScienceUpgrade", 256, 32),
        PoolSizeRec::new("ReplaceObjectUpgrade", 32, 32),
        PoolSizeRec::new("ModelConditionUpgrade", 32, 32),
        PoolSizeRec::new("SpyVisionSpecialPower", 256, 32),
        PoolSizeRec::new("StealthDetectorUpdate", 256, 32),
        PoolSizeRec::new("StealthUpdate", 512, 128),
        PoolSizeRec::new("StealthUpgrade", 256, 32),
        PoolSizeRec::new("StatusBitsUpgrade", 128, 128),
        PoolSizeRec::new("SubObjectsUpgrade", 128, 128),
        PoolSizeRec::new("ExperienceScalarUpgrade", 256, 128),
        PoolSizeRec::new("MaxHealthUpgrade", 128, 128),
        PoolSizeRec::new("WeaponBonusUpgrade", 128, 64),
        PoolSizeRec::new("StickyBombUpdate", 64, 32),
        PoolSizeRec::new("FireOCLAfterWeaponCooldownUpdate", 64, 32),
        PoolSizeRec::new("HijackerUpdate", 64, 32),
        PoolSizeRec::new("ChinaMinesUpgrade", 64, 32),
        PoolSizeRec::new("PowerPlantUpdate", 48, 16),
        PoolSizeRec::new("PowerPlantUpgrade", 48, 16),
        PoolSizeRec::new("DefectorSpecialPower", 16, 16),
        PoolSizeRec::new("CheckpointUpdate", 16, 16),
        PoolSizeRec::new("MobNexusContain", 128, 32),
        PoolSizeRec::new("MobMemberSlavedUpdate", 64, 32),
        PoolSizeRec::new("EMPUpdate", 64, 32),
        PoolSizeRec::new("LeafletDropBehavior", 64, 32),
        PoolSizeRec::new("Overridable", 32, 32),
        PoolSizeRec::new("W3DGameWindow", 700, 256),
        PoolSizeRec::new("SuccessState", 32, 32),
        PoolSizeRec::new("FailureState", 32, 32),
        PoolSizeRec::new("ContinueState", 32, 32),
        PoolSizeRec::new("SleepState", 32, 32),
        PoolSizeRec::new("AIDockWaitForClearanceState", 256, 32),
        PoolSizeRec::new("AIDockProcessDockState", 256, 32),
        PoolSizeRec::new("AIGuardInnerState", 32, 32),
        PoolSizeRec::new("AIGuardIdleState", 32, 32),
        PoolSizeRec::new("AIGuardOuterState", 32, 32),
        PoolSizeRec::new("AIGuardReturnState", 32, 32),
        PoolSizeRec::new("AIGuardPickUpCrateState", 32, 32),
        PoolSizeRec::new("AIGuardAttackAggressorState", 32, 32),
        PoolSizeRec::new("AIGuardRetaliateInnerState", 32, 32),
        PoolSizeRec::new("AIGuardRetaliateIdleState", 32, 32),
        PoolSizeRec::new("AIGuardRetaliateOuterState", 32, 32),
        PoolSizeRec::new("AIGuardRetaliateReturnState", 32, 32),
        PoolSizeRec::new("AIGuardRetaliatePickUpCrateState", 32, 32),
        PoolSizeRec::new("AIGuardRetaliateAttackAggressorState", 32, 32),
        PoolSizeRec::new("AITNGuardInnerState", 32, 32),
        PoolSizeRec::new("AITNGuardIdleState", 32, 32),
        PoolSizeRec::new("AITNGuardOuterState", 32, 32),
        PoolSizeRec::new("AITNGuardReturnState", 32, 32),
        PoolSizeRec::new("AITNGuardPickUpCrateState", 32, 32),
        PoolSizeRec::new("AITNGuardAttackAggressorState", 32, 32),
        PoolSizeRec::new("AIIdleState", 2400, 32),
        PoolSizeRec::new("AIRappelState", 600, 32),
        PoolSizeRec::new("AIBusyState", 600, 32),
        PoolSizeRec::new("AIWaitState", 600, 32),
        PoolSizeRec::new("AIAttackState", 4096, 32),
        PoolSizeRec::new("AIAttackSquadState", 600, 32),
        PoolSizeRec::new("AIDeadState", 600, 32),
        PoolSizeRec::new("AIDockState", 600, 32),
        PoolSizeRec::new("AIExitState", 600, 32),
        PoolSizeRec::new("AIExitInstantlyState", 600, 32),
        PoolSizeRec::new("AIGuardState", 600, 32),
        PoolSizeRec::new("AIGuardRetaliateState", 600, 32),
        PoolSizeRec::new("AITunnelNetworkGuardState", 600, 32),
        PoolSizeRec::new("AIHuntState", 600, 32),
        PoolSizeRec::new("AIAttackAreaState", 600, 32),
        PoolSizeRec::new("AIFaceState", 1200, 32),
        PoolSizeRec::new("ApproachState", 600, 32),
        PoolSizeRec::new("DeliveringState", 600, 32),
        PoolSizeRec::new("ConsiderNewApproachState", 600, 32),
        PoolSizeRec::new("RecoverFromOffMapState", 600, 32),
        PoolSizeRec::new("HeadOffMapState", 600, 32),
        PoolSizeRec::new("CleanUpState", 600, 32),
        PoolSizeRec::new("HackInternetState", 600, 32),
        PoolSizeRec::new("PackingState", 600, 32),
        PoolSizeRec::new("UnpackingState", 600, 32),
        PoolSizeRec::new("SupplyTruckWantsToPickUpOrDeliverBoxesState", 600, 32),
        PoolSizeRec::new("RegroupingState", 600, 32),
        PoolSizeRec::new("DockingState", 600, 32),
        PoolSizeRec::new("ChinookEvacuateState", 32, 32),
        PoolSizeRec::new("ChinookHeadOffMapState", 32, 32),
        PoolSizeRec::new("ChinookTakeoffOrLandingState", 32, 32),
        PoolSizeRec::new("ChinookCombatDropState", 32, 32),
        PoolSizeRec::new("DozerActionPickActionPosState", 256, 32),
        PoolSizeRec::new("DozerActionMoveToActionPosState", 256, 32),
        PoolSizeRec::new("DozerActionDoActionState", 256, 32),
        PoolSizeRec::new("DozerPrimaryIdleState", 256, 32),
        PoolSizeRec::new("DozerActionState", 256, 32),
        PoolSizeRec::new("DozerPrimaryGoingHomeState", 256, 32),
        PoolSizeRec::new("JetAwaitingRunwayState", 64, 32),
        PoolSizeRec::new("JetOrHeliCirclingDeadAirfieldState", 64, 32),
        PoolSizeRec::new("HeliTakeoffOrLandingState", 64, 32),
        PoolSizeRec::new("JetOrHeliParkOrientState", 64, 32),
        PoolSizeRec::new("JetOrHeliReloadAmmoState", 64, 32),
        PoolSizeRec::new("SupplyTruckBusyState", 600, 32),
        PoolSizeRec::new("SupplyTruckIdleState", 600, 32),
        PoolSizeRec::new("ActAsDozerState", 600, 32),
        PoolSizeRec::new("ActAsSupplyTruckState", 600, 32),
        PoolSizeRec::new("AIDockApproachState", 256, 32),
        PoolSizeRec::new("AIDockAdvancePositionState", 256, 32),
        PoolSizeRec::new("AIDockMoveToEntryState", 256, 32),
        PoolSizeRec::new("AIDockMoveToDockState", 256, 32),
        PoolSizeRec::new("AIDockMoveToExitState", 256, 32),
        PoolSizeRec::new("AIDockMoveToRallyState", 256, 32),
        PoolSizeRec::new("AIMoveToState", 600, 32),
        PoolSizeRec::new("AIMoveOutOfTheWayState", 600, 32),
        PoolSizeRec::new("AIMoveAndTightenState", 600, 32),
        PoolSizeRec::new("AIMoveAwayFromRepulsorsState", 600, 32),
        PoolSizeRec::new("AIAttackApproachTargetState", 96, 32),
        PoolSizeRec::new("AIAttackPursueTargetState", 96, 32),
        PoolSizeRec::new("AIAttackAimAtTargetState", 96, 32),
        PoolSizeRec::new("AIAttackFireWeaponState", 256, 32),
        PoolSizeRec::new("AIPickUpCrateState", 4096, 32),
        PoolSizeRec::new("AIFollowWaypointPathState", 1200, 32),
        PoolSizeRec::new("AIFollowWaypointPathExactState", 1200, 32),
        PoolSizeRec::new("AIWanderInPlaceState", 600, 32),
        PoolSizeRec::new("AIFollowPathState", 1200, 32),
        PoolSizeRec::new("AIMoveAndEvacuateState", 1200, 32),
        PoolSizeRec::new("AIMoveAndDeleteState", 600, 32),
        PoolSizeRec::new("AIEnterState", 600, 32),
        PoolSizeRec::new("JetOrHeliReturningToDeadAirfieldState", 64, 32),
        PoolSizeRec::new("JetOrHeliReturnForLandingState", 64, 32),
        PoolSizeRec::new("TurretAIIdleState", 600, 32),
        PoolSizeRec::new("TurretAIIdleScanState", 600, 32),
        PoolSizeRec::new("TurretAIAimTurretState", 600, 32),
        PoolSizeRec::new("TurretAIRecenterTurretState", 600, 32),
        PoolSizeRec::new("TurretAIHoldTurretState", 600, 32),
        PoolSizeRec::new("JetOrHeliTaxiState", 64, 32),
        PoolSizeRec::new("JetTakeoffOrLandingState", 64, 32),
        PoolSizeRec::new("JetPauseBeforeTakeoffState", 64, 32),
        PoolSizeRec::new("AIAttackMoveToState", 600, 32),
        PoolSizeRec::new("AIAttackFollowWaypointPathState", 1200, 32),
        PoolSizeRec::new("AIWanderState", 600, 32),
        PoolSizeRec::new("AIPanicState", 600, 32),
        PoolSizeRec::new("ChinookMoveToBldgState", 32, 32),
        PoolSizeRec::new("ChinookRecordCreationState", 32, 32),
        PoolSizeRec::new("ScienceInfo", 96, 32),
        PoolSizeRec::new("RankInfo", 32, 32),
        PoolSizeRec::new("FireWeaponNugget", 32, 32),
        PoolSizeRec::new("AttackNugget", 32, 32),
        PoolSizeRec::new("DeliverPayloadNugget", 48, 32),
        PoolSizeRec::new("ApplyRandomForceNugget", 32, 32),
        PoolSizeRec::new("GenericObjectCreationNugget", 632, 32),
        PoolSizeRec::new("SoundFXNugget", 320, 32),
        PoolSizeRec::new("TracerFXNugget", 32, 32),
        PoolSizeRec::new("RayEffectFXNugget", 32, 32),
        PoolSizeRec::new("LightPulseFXNugget", 68, 32),
        PoolSizeRec::new("ViewShakeFXNugget", 140, 32),
        PoolSizeRec::new("TerrainScorchFXNugget", 48, 32),
        PoolSizeRec::new("ParticleSystemFXNugget", 832, 32),
        PoolSizeRec::new("FXListAtBonePosFXNugget", 32, 32),
        PoolSizeRec::new("Squad", 256, 32),
        PoolSizeRec::new("BuildListInfo", 400, 64),
        PoolSizeRec::new("ScriptGroup", 128, 32),
        PoolSizeRec::new("OrCondition", 1024, 256),
        PoolSizeRec::new("ScriptAction", 2600, 512),
        PoolSizeRec::new("Script", 1024, 256),
        PoolSizeRec::new("Parameter", 8192, 1024),
        PoolSizeRec::new("Condition", 2048, 256),
        PoolSizeRec::new("Template", 32, 32),
        PoolSizeRec::new("ScriptList", 32, 32),
        PoolSizeRec::new("AttackPriorityInfo", 32, 32),
        PoolSizeRec::new("SequentialScript", 32, 32),
        PoolSizeRec::new("Win32LocalFile", 1024, 256),
        PoolSizeRec::new("RAMFile", 32, 32),
        PoolSizeRec::new("BattlePlanBonuses", 32, 32),
        PoolSizeRec::new("KindOfPercentProductionChange", 32, 32),
        PoolSizeRec::new("UserParser", 4096, 256),
        PoolSizeRec::new("XferBlockData", 32, 32),
        PoolSizeRec::new("EvaCheckInfo", 52, 16),
        PoolSizeRec::new("SuperweaponInfo", 32, 32),
        PoolSizeRec::new("NamedTimerInfo", 32, 32),
        PoolSizeRec::new("PopupMessageData", 32, 32),
        PoolSizeRec::new("FloatingTextData", 32, 32),
        PoolSizeRec::new("MapObject", 5000, 1024),
        PoolSizeRec::new("Waypoint", 1024, 32),
        PoolSizeRec::new("PolygonTrigger", 64, 64),
        PoolSizeRec::new("Bridge", 32, 32),
        PoolSizeRec::new("Mapping", 384, 64),
        PoolSizeRec::new("OutputChunk", 32, 32),
        PoolSizeRec::new("InputChunk", 32, 32),
        PoolSizeRec::new("AnimateWindow", 32, 32),
        PoolSizeRec::new("GameFont", 32, 32),
        PoolSizeRec::new("NetCommandRef", 256, 32),
        PoolSizeRec::new("GameMessageArgument", 1024, 256),
        PoolSizeRec::new("GameMessageParserArgumentType", 32, 32),
        PoolSizeRec::new("GameMessageParser", 32, 32),
        PoolSizeRec::new("WeaponBonusSet", 96, 32),
        PoolSizeRec::new("Campaign", 32, 32),
        PoolSizeRec::new("Mission", 88, 32),
        PoolSizeRec::new("ModalWindow", 32, 32),
        PoolSizeRec::new("NetPacket", 32, 32),
        PoolSizeRec::new("AISideInfo", 32, 32),
        PoolSizeRec::new("AISideBuildList", 32, 32),
        PoolSizeRec::new("MetaMapRec", 256, 32),
        PoolSizeRec::new("TransportStatus", 32, 32),
        PoolSizeRec::new("Anim2DTemplate", 32, 32),
        PoolSizeRec::new("ObjectTypes", 32, 32),
        PoolSizeRec::new("NetCommandList", 512, 32),
        PoolSizeRec::new("TurretAIData", 256, 32),
        PoolSizeRec::new("NetCommandMsg", 32, 32),
        PoolSizeRec::new("NetGameCommandMsg", 64, 32),
        PoolSizeRec::new("NetAckBothCommandMsg", 32, 32),
        PoolSizeRec::new("NetAckStage1CommandMsg", 32, 32),
        PoolSizeRec::new("NetAckStage2CommandMsg", 32, 32),
        PoolSizeRec::new("NetFrameCommandMsg", 32, 32),
        PoolSizeRec::new("NetPlayerLeaveCommandMsg", 32, 32),
        PoolSizeRec::new("NetRunAheadMetricsCommandMsg", 32, 32),
        PoolSizeRec::new("NetRunAheadCommandMsg", 32, 32),
        PoolSizeRec::new("NetDestroyPlayerCommandMsg", 32, 32),
        PoolSizeRec::new("NetDisconnectFrameCommandMsg", 32, 32),
        PoolSizeRec::new("NetDisconnectScreenOffCommandMsg", 32, 32),
        PoolSizeRec::new("NetFrameResendRequestCommandMsg", 32, 32),
        PoolSizeRec::new("NetKeepAliveCommandMsg", 32, 32),
        PoolSizeRec::new("NetDisconnectKeepAliveCommandMsg", 32, 32),
        PoolSizeRec::new("NetDisconnectPlayerCommandMsg", 32, 32),
        PoolSizeRec::new("NetPacketRouterQueryCommandMsg", 32, 32),
        PoolSizeRec::new("NetPacketRouterAckCommandMsg", 32, 32),
        PoolSizeRec::new("NetDisconnectChatCommandMsg", 32, 32),
        PoolSizeRec::new("NetChatCommandMsg", 32, 32),
        PoolSizeRec::new("NetDisconnectVoteCommandMsg", 32, 32),
        PoolSizeRec::new("NetProgressCommandMsg", 32, 32),
        PoolSizeRec::new("NetWrapperCommandMsg", 32, 32),
        PoolSizeRec::new("NetFileCommandMsg", 32, 32),
        PoolSizeRec::new("NetFileAnnounceCommandMsg", 32, 32),
        PoolSizeRec::new("NetFileProgressCommandMsg", 32, 32),
        PoolSizeRec::new("NetCommandWrapperListNode", 32, 32),
        PoolSizeRec::new("NetCommandWrapperList", 32, 32),
        PoolSizeRec::new("Connection", 32, 32),
        PoolSizeRec::new("User", 32, 32),
        PoolSizeRec::new("FrameDataManager", 32, 32),
        PoolSizeRec::new("DrawableIconInfo", 32, 32),
        PoolSizeRec::new("TintEnvelope", 128, 32),
        PoolSizeRec::new("DynamicAudioEventRTS", 4000, 256),
        PoolSizeRec::new("DrawableLocoInfo", 128, 32),
        PoolSizeRec::new("W3DPrototypeClass", 512, 256),
        PoolSizeRec::new("EnumeratedIP", 32, 32),
        PoolSizeRec::new("WaterTransparencySetting", 4, 4),
        PoolSizeRec::new("WeatherSetting", 4, 4),
        PoolSizeRec::new("BoxPrototypeClass", 128, 128),
        PoolSizeRec::new("SpherePrototypeClass", 32, 32),
        PoolSizeRec::new("SoundRenderObjPrototypeClass", 32, 32),
        PoolSizeRec::new("RingPrototypeClass", 32, 32),
        PoolSizeRec::new("PrimitivePrototypeClass", 8192, 32),
        PoolSizeRec::new("HModelPrototypeClass", 256, 32),
        PoolSizeRec::new("ParticleEmitterPrototypeClass", 32, 32),
        PoolSizeRec::new("NullPrototypeClass", 32, 32),
        PoolSizeRec::new("HLodPrototypeClass", 700, 128),
        PoolSizeRec::new("HLodDefClass", 700, 128),
        PoolSizeRec::new("DistLODPrototypeClass", 32, 32),
        PoolSizeRec::new("DazzlePrototypeClass", 32, 32),
        PoolSizeRec::new("CollectionPrototypeClass", 32, 32),
        PoolSizeRec::new("BoxPrototypeClass", 256, 32),
        PoolSizeRec::new("AggregatePrototypeClass", 32, 32),
        PoolSizeRec::new("OBBoxRenderObjClass", 512, 128),
        PoolSizeRec::new("AABoxRenderObjClass", 32, 32),
        PoolSizeRec::new("VertexMaterialClass", 6000, 2048),
        PoolSizeRec::new("TextureClass", 1200, 256),
        PoolSizeRec::new("CloudMapTerrainTextureClass", 4, 4),
        PoolSizeRec::new("ScorchTextureClass", 4, 4),
        PoolSizeRec::new("LightMapTerrainTextureClass", 4, 4),
        PoolSizeRec::new("AlphaEdgeTextureClass", 4, 4),
        PoolSizeRec::new("AlphaTerrainTextureClass", 4, 4),
        PoolSizeRec::new("TerrainTextureClass", 4, 4),
        PoolSizeRec::new("MeshClass", 14000, 2000),
        PoolSizeRec::new("HTreeClass", 2048, 512),
        PoolSizeRec::new("HLodClass", 2048, 512),
        PoolSizeRec::new("MeshModelClass", 8192, 32),
        PoolSizeRec::new("ShareBufferClass", 32768, 1024),
        PoolSizeRec::new("AABTreeClass", 300, 128),
        PoolSizeRec::new("MotionChannelClass", 16384, 32),
        PoolSizeRec::new("BitChannelClass", 84, 32),
        PoolSizeRec::new("TimeCodedMotionChannelClass", 116, 32),
        PoolSizeRec::new("AdaptiveDeltaMotionChannelClass", 32, 32),
        PoolSizeRec::new("TimeCodedBitChannelClass", 32, 32),
        PoolSizeRec::new("UVBufferClass", 8192, 32),
        PoolSizeRec::new("TexBufferClass", 384, 128),
        PoolSizeRec::new("MatBufferClass", 256, 128),
        PoolSizeRec::new("MatrixMapperClass", 32, 32),
        PoolSizeRec::new("ScaleTextureMapperClass", 32, 32),
        PoolSizeRec::new("LinearOffsetTextureMapperClass", 96, 32),
        PoolSizeRec::new("GridTextureMapperClass", 32, 32),
        PoolSizeRec::new("RotateTextureMapperClass", 32, 32),
        PoolSizeRec::new("SineLinearOffsetTextureMapperClass", 32, 32),
        PoolSizeRec::new("StepLinearOffsetTextureMapperClass", 32, 32),
        PoolSizeRec::new("ZigZagLinearOffsetTextureMapperClass", 32, 32),
        PoolSizeRec::new("ClassicEnvironmentMapperClass", 32, 32),
        PoolSizeRec::new("EnvironmentMapperClass", 256, 32),
        PoolSizeRec::new("EdgeMapperClass", 32, 32),
        PoolSizeRec::new("WSClassicEnvironmentMapperClass", 32, 32),
        PoolSizeRec::new("WSEnvironmentMapperClass", 32, 32),
        PoolSizeRec::new("GridClassicEnvironmentMapperClass", 32, 32),
        PoolSizeRec::new("GridEnvironmentMapperClass", 32, 32),
        PoolSizeRec::new("ScreenMapperClass", 32, 32),
        PoolSizeRec::new("RandomTextureMapperClass", 32, 32),
        PoolSizeRec::new("BumpEnvTextureMapperClass", 32, 32),
        PoolSizeRec::new("MeshLoadContextClass", 4, 4),
        PoolSizeRec::new("MaterialInfoClass", 8192, 32),
        PoolSizeRec::new("MeshMatDescClass", 8192, 32),
        PoolSizeRec::new("TextureLoadTaskClass", 256, 32),
        PoolSizeRec::new("SortingNodeStruct", 288, 32),
        PoolSizeRec::new("ProxyArrayClass", 32, 32),
        PoolSizeRec::new("Line3DClass", 8, 8),
        PoolSizeRec::new("Render2DClass", 64, 32),
        PoolSizeRec::new("SurfaceClass", 128, 32),
        PoolSizeRec::new("FontCharsClassCharDataStruct", 1024, 32),
        PoolSizeRec::new("FontCharsBuffer", 16, 4),
        PoolSizeRec::new("FVFInfoClass", 152, 64),
        PoolSizeRec::new("TerrainTracksRenderObjClass", 128, 32),
        PoolSizeRec::new("DynamicIBAccessClass", 32, 32),
        PoolSizeRec::new("DX8IndexBufferClass", 128, 32),
        PoolSizeRec::new("SortingIndexBufferClass", 32, 32),
        PoolSizeRec::new("DX8VertexBufferClass", 128, 32),
        PoolSizeRec::new("SortingVertexBufferClass", 32, 32),
        PoolSizeRec::new("DynD3DMATERIAL8", 8192, 32),
        PoolSizeRec::new("DynamicMatrix3D", 512, 32),
        PoolSizeRec::new("MeshGeometryClass", 32, 32),
        PoolSizeRec::new("DynamicMeshModel", 32, 32),
        PoolSizeRec::new("GapFillerClass", 32, 32),
        PoolSizeRec::new("FontCharsClass", 64, 32),
        PoolSizeRec::new("ThumbnailManagerClass", 32, 32),
        PoolSizeRec::new("SmudgeSet", 32, 32),
        PoolSizeRec::new("Smudge", 128, 32),
    ]
}

/// Memory pool manager
///
/// This manages the configuration and initialization of memory pools.
pub struct MemoryPoolManager {
    /// Pool configurations indexed by name
    pool_configs: HashMap<String, PoolSizeRec>,
    /// DMA pool configurations
    dma_configs: Vec<PoolInitRec>,
    /// Whether the manager has been initialized
    initialized: bool,
}

impl MemoryPoolManager {
    /// Create a new memory pool manager
    pub fn new() -> Self {
        Self {
            pool_configs: HashMap::new(),
            dma_configs: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize with default pool configurations
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }

        // Load default DMA configurations
        self.dma_configs = get_default_dma_params();

        // Load default pool size configurations
        for pool_size in get_default_pool_sizes() {
            self.pool_configs
                .entry(pool_size.name.clone())
                .or_insert(pool_size);
        }

        // Try to load overrides from configuration file
        self.load_config_overrides();

        self.initialized = true;
    }

    /// Load configuration overrides from MemoryPools.ini
    ///
    /// This corresponds to the userMemoryManagerInitPools function in C++.
    fn load_config_overrides(&mut self) {
        if let Some(path) = memory_pools_ini_path() {
            if let Ok(config_content) = std::fs::read_to_string(path) {
                self.parse_config_file(&config_content);
            }
        }
    }

    fn parse_config_file(&mut self, config_content: &str) {
        for line in config_content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with(';') {
                continue;
            }

            let mut parts = trimmed.split_whitespace();
            let Some(pool_name) = parts.next() else {
                continue;
            };
            let Some(initial) = parts.next().and_then(|value| value.parse::<usize>().ok()) else {
                continue;
            };
            let Some(overflow) = parts.next().and_then(|value| value.parse::<usize>().ok()) else {
                continue;
            };

            if let Some(config) = self
                .pool_configs
                .values_mut()
                .find(|config| config.name.eq_ignore_ascii_case(pool_name))
            {
                config.initial = Self::round_up_mem_bound(initial);
                config.overflow = Self::round_up_mem_bound(overflow);
            }
        }
    }

    /// Get pool size configuration for a named pool
    pub fn get_pool_config(&self, pool_name: &str) -> Option<&PoolSizeRec> {
        self.pool_configs.get(pool_name)
    }

    /// Adjust pool size for a named pool
    ///
    /// This corresponds to userMemoryAdjustPoolSize in the C++ code.
    pub fn adjust_pool_size(&mut self, pool_name: &str, initial: usize, overflow: usize) {
        if let Some(config) = self.pool_configs.get_mut(pool_name) {
            config.initial = initial;
            config.overflow = overflow;
        } else {
            // Create new configuration if it doesn't exist
            self.pool_configs.insert(
                pool_name.to_string(),
                PoolSizeRec::new(pool_name, initial, overflow),
            );
        }
    }

    /// Get DMA pool configurations
    pub fn get_dma_configs(&self) -> &[PoolInitRec] {
        &self.dma_configs
    }

    /// Get all pool configurations
    pub fn get_all_pool_configs(&self) -> &HashMap<String, PoolSizeRec> {
        &self.pool_configs
    }

    /// Round up memory boundary for alignment
    ///
    /// This corresponds to roundUpMemBound in the C++ code.
    pub fn round_up_mem_bound(size: usize) -> usize {
        const MEM_BOUND_ALIGNMENT: usize = 4;

        if size < MEM_BOUND_ALIGNMENT {
            MEM_BOUND_ALIGNMENT
        } else {
            (size + (MEM_BOUND_ALIGNMENT - 1)) & !(MEM_BOUND_ALIGNMENT - 1)
        }
    }
}

fn memory_pools_ini_path() -> Option<PathBuf> {
    let relative = Path::new("Data").join("INI").join("MemoryPools.ini");
    if relative.is_file() {
        return Some(relative);
    }

    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(&relative)))
        .filter(|path| path.is_file())
}

impl Default for MemoryPoolManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global memory pool manager instance
lazy_static::lazy_static! {
    pub static ref MEMORY_POOL_MANAGER: Arc<Mutex<MemoryPoolManager>> =
        Arc::new(Mutex::new(MemoryPoolManager::new()));
}

/// Initialize memory pools
///
/// This corresponds to the userMemoryManagerInitPools function in C++.
pub fn init_memory_pools() {
    let mut manager = MEMORY_POOL_MANAGER.lock().unwrap();
    manager.init();
}

/// Get memory pool manager instance
pub fn get_memory_pool_manager() -> Arc<Mutex<MemoryPoolManager>> {
    MEMORY_POOL_MANAGER.clone()
}

/// User memory manager DMA parameters callback
///
/// This corresponds to userMemoryManagerGetDmaParms in the C++ code.
pub fn get_user_memory_dma_params() -> (usize, Vec<PoolInitRec>) {
    let dma_params = get_default_dma_params();
    let num_sub_pools = dma_params.len();
    (num_sub_pools, dma_params)
}

/// Adjust pool size for a specific pool
///
/// This corresponds to userMemoryAdjustPoolSize in the C++ code.
pub fn adjust_pool_size(
    pool_name: &str,
    initial_allocation_count: &mut usize,
    overflow_allocation_count: &mut usize,
) {
    if *initial_allocation_count > 0 {
        return; // Already configured
    }

    let mut manager = MEMORY_POOL_MANAGER.lock().unwrap();
    if !manager.initialized {
        manager.init();
    }
    if let Some(config) = manager.get_pool_config(pool_name) {
        *initial_allocation_count = config.initial;
        *overflow_allocation_count = config.overflow;
    } else {
        eprintln!(
            "Initial size for pool {} not found -- you should add it to memory pool configuration",
            pool_name
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_init_rec() {
        let rec = PoolInitRec::new("TestPool", 64, 100, 50);
        assert_eq!(rec.name, "TestPool");
        assert_eq!(rec.alloc_size, 64);
        assert_eq!(rec.initial_count, 100);
        assert_eq!(rec.overflow_count, 50);
    }

    #[test]
    fn test_pool_size_rec() {
        let rec = PoolSizeRec::new("TestPool", 200, 100);
        assert_eq!(rec.name, "TestPool");
        assert_eq!(rec.initial, 200);
        assert_eq!(rec.overflow, 100);
    }

    #[test]
    fn test_default_dma_params() {
        let dma_params = get_default_dma_params();
        assert_eq!(dma_params.len(), 7);

        // Verify first DMA pool
        assert_eq!(dma_params[0].name, "dmaPool_16");
        assert_eq!(dma_params[0].alloc_size, 16);
        assert_eq!(dma_params[0].initial_count, 130000);
        assert_eq!(dma_params[0].overflow_count, 10000);
    }

    #[test]
    fn test_default_pool_sizes() {
        let pool_sizes = get_default_pool_sizes();
        assert!(!pool_sizes.is_empty());
        assert_eq!(pool_sizes.len(), 617);

        // Check for some expected pools
        let pool_names: Vec<&str> = pool_sizes.iter().map(|p| p.name.as_str()).collect();
        assert!(pool_names.contains(&"ObjectPool"));
        assert!(pool_names.contains(&"ParticlePool"));
        assert!(pool_names.contains(&"PathNodePool"));
        assert!(pool_names.contains(&"W3DDebrisDraw"));
        assert!(pool_names.contains(&"TerrainTracksRenderObjClass"));
        assert!(pool_names.contains(&"DynamicAudioEventRTS"));
    }

    #[test]
    fn test_memory_pool_manager() {
        let mut manager = MemoryPoolManager::new();
        assert!(!manager.initialized);

        manager.init();
        assert!(manager.initialized);
        assert!(!manager.get_all_pool_configs().is_empty());

        // Test getting specific pool config
        let object_pool = manager.get_pool_config("ObjectPool");
        assert!(object_pool.is_some());

        let config = object_pool.unwrap();
        assert_eq!(config.name, "ObjectPool");
    }

    #[test]
    fn test_duplicate_pool_names_keep_first_cxx_match() {
        let mut manager = MemoryPoolManager::new();
        manager.init();

        let weapon_bonus = manager.get_pool_config("WeaponBonusUpgrade").unwrap();
        assert_eq!(weapon_bonus.initial, 512);
        assert_eq!(weapon_bonus.overflow, 128);

        let box_prototype = manager.get_pool_config("BoxPrototypeClass").unwrap();
        assert_eq!(box_prototype.initial, 128);
        assert_eq!(box_prototype.overflow, 128);
    }

    #[test]
    fn test_memory_pool_ini_overrides_round_and_match_case_insensitive() {
        let mut manager = MemoryPoolManager::new();
        for pool_size in get_default_pool_sizes() {
            manager
                .pool_configs
                .entry(pool_size.name.clone())
                .or_insert(pool_size);
        }

        manager.parse_config_file(
            "; ignored comment\n\
             objectpool 5 9\n\
             W3DDEBRISDRAW 130 131\n\
             MissingPool 1 1\n\
             malformed\n",
        );

        let object_pool = manager.get_pool_config("ObjectPool").unwrap();
        assert_eq!(object_pool.initial, 8);
        assert_eq!(object_pool.overflow, 12);

        let debris_draw = manager.get_pool_config("W3DDebrisDraw").unwrap();
        assert_eq!(debris_draw.initial, 132);
        assert_eq!(debris_draw.overflow, 132);
    }

    #[test]
    fn test_adjust_pool_size() {
        let mut manager = MemoryPoolManager::new();
        manager.init();

        // Adjust existing pool
        manager.adjust_pool_size("ObjectPool", 2000, 500);
        let config = manager.get_pool_config("ObjectPool").unwrap();
        assert_eq!(config.initial, 2000);
        assert_eq!(config.overflow, 500);

        // Create new pool configuration
        manager.adjust_pool_size("NewPool", 100, 25);
        let new_config = manager.get_pool_config("NewPool").unwrap();
        assert_eq!(new_config.initial, 100);
        assert_eq!(new_config.overflow, 25);
    }

    #[test]
    fn test_round_up_mem_bound() {
        assert_eq!(MemoryPoolManager::round_up_mem_bound(1), 4);
        assert_eq!(MemoryPoolManager::round_up_mem_bound(4), 4);
        assert_eq!(MemoryPoolManager::round_up_mem_bound(5), 8);
        assert_eq!(MemoryPoolManager::round_up_mem_bound(8), 8);
        assert_eq!(MemoryPoolManager::round_up_mem_bound(9), 12);
    }

    #[test]
    fn test_user_memory_dma_params() {
        let (num_pools, dma_params) = get_user_memory_dma_params();
        assert_eq!(num_pools, dma_params.len());
        assert_eq!(num_pools, 7);
    }

    #[test]
    fn test_adjust_pool_size_function() {
        // Test with already configured pool
        let mut initial = 100usize;
        let mut overflow = 50usize;

        adjust_pool_size("TestPool", &mut initial, &mut overflow);
        assert_eq!(initial, 100); // Should remain unchanged
        assert_eq!(overflow, 50);

        // Test with unconfigured pool
        initial = 0;
        overflow = 0;

        init_memory_pools(); // Ensure manager is initialized
        adjust_pool_size("ObjectPool", &mut initial, &mut overflow);
        assert_ne!(initial, 0); // Should be set to default value
        assert_ne!(overflow, 0);
    }
}
