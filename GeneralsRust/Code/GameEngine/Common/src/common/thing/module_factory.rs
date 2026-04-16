////////////////////////////////////////////////////////////////////////////////
//																			//
//  (c) 2001-2003 Electronic Arts Inc.										//
//																			//
////////////////////////////////////////////////////////////////////////////////

//! Module factory for creating modules for objects and drawables
//! Contains a list of module templates, when we request a new module,
//! we look for that template and create it

use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::system::Snapshotable;
use crate::common::thing::thing_template::{
    ModuleDescriptor as TemplateModuleDescriptor,
    ModuleDescriptorSet as TemplateModuleDescriptorSet,
};
use crate::common::{
    ini::INI,
    rts::{AsciiString, NameKeyType},
    system::{SubsystemInterface, Xfer},
    thing::module::{Module, ModuleData, ModuleInterfaceType, ModuleType, Thing},
};
use once_cell::sync::Lazy;
use std::{
    any::Any,
    collections::{hash_map::Entry, HashMap},
    mem,
    sync::{Arc, Mutex},
};

const fn mask_or(a: ModuleInterfaceType, b: ModuleInterfaceType) -> ModuleInterfaceType {
    ModuleInterfaceType(a.0 | b.0)
}

const DIE_DAMAGE_MASK: ModuleInterfaceType =
    mask_or(ModuleInterfaceType::DIE, ModuleInterfaceType::DAMAGE);
const UPDATE_DIE_DAMAGE_MASK: ModuleInterfaceType =
    mask_or(ModuleInterfaceType::UPDATE, DIE_DAMAGE_MASK);
const UPDATE_DAMAGE_MASK: ModuleInterfaceType =
    mask_or(ModuleInterfaceType::UPDATE, ModuleInterfaceType::DAMAGE);
const UPDATE_DIE_MASK: ModuleInterfaceType =
    mask_or(ModuleInterfaceType::UPDATE, ModuleInterfaceType::DIE);
const UPDATE_COLLIDE_MASK: ModuleInterfaceType =
    mask_or(ModuleInterfaceType::UPDATE, ModuleInterfaceType::COLLIDE);
const UPDATE_DIE_UPGRADE_MASK: ModuleInterfaceType =
    mask_or(UPDATE_DIE_MASK, ModuleInterfaceType::UPGRADE);
const UPDATE_DIE_DAMAGE_COLLIDE_MASK: ModuleInterfaceType =
    mask_or(UPDATE_DIE_DAMAGE_MASK, ModuleInterfaceType::COLLIDE);
const OPEN_CONTAIN_MASK: ModuleInterfaceType = mask_or(
    ModuleInterfaceType::UPDATE,
    mask_or(
        ModuleInterfaceType::CONTAIN,
        mask_or(ModuleInterfaceType::COLLIDE, DIE_DAMAGE_MASK),
    ),
);
const OPEN_CONTAIN_CREATE_MASK: ModuleInterfaceType =
    mask_or(OPEN_CONTAIN_MASK, ModuleInterfaceType::CREATE);

const BUILTIN_BEHAVIOR_DESCRIPTORS: &[(&str, ModuleInterfaceType)] = &[
    ("InactiveBody", ModuleInterfaceType::BODY),
    ("ActiveBody", ModuleInterfaceType::BODY),
    ("StructureBody", ModuleInterfaceType::BODY),
    ("HighlanderBody", ModuleInterfaceType::BODY),
    ("ImmortalBody", ModuleInterfaceType::BODY),
    ("HiveStructureBody", ModuleInterfaceType::BODY),
    ("UndeadBody", ModuleInterfaceType::BODY),
    ("StatusBitsUpgrade", ModuleInterfaceType::UPGRADE),
    ("PassengersFireUpgrade", ModuleInterfaceType::UPGRADE),
    ("SubObjectsUpgrade", ModuleInterfaceType::UPGRADE),
    ("StealthUpdate", ModuleInterfaceType::UPDATE),
    ("FireWeaponCollide", ModuleInterfaceType::COLLIDE),
    (
        "AutoHealBehavior",
        mask_or(ModuleInterfaceType::UPDATE, ModuleInterfaceType::UPGRADE),
    ),
    (
        "SlowDeathBehavior",
        mask_or(ModuleInterfaceType::UPDATE, ModuleInterfaceType::DIE),
    ),
    ("InstantDeathBehavior", ModuleInterfaceType::DIE),
    (
        "FXListDie",
        mask_or(ModuleInterfaceType::DIE, ModuleInterfaceType::UPGRADE),
    ),
    ("UpgradeDie", ModuleInterfaceType::DIE),
    ("DestroyDie", ModuleInterfaceType::DIE),
    ("KeepObjectDie", ModuleInterfaceType::DIE),
    ("CreateObjectDie", ModuleInterfaceType::DIE),
    ("CreateCrateDie", ModuleInterfaceType::DIE),
    ("CrushDie", ModuleInterfaceType::DIE),
    ("EjectPilotDie", ModuleInterfaceType::DIE),
    ("RebuildHoleExposeDie", ModuleInterfaceType::DIE),
    ("SpecialPowerCompletionDie", ModuleInterfaceType::DIE),
    ("DamDie", ModuleInterfaceType::DIE),
    ("BridgeBehavior", UPDATE_DIE_DAMAGE_MASK),
    ("BridgeScaffoldBehavior", ModuleInterfaceType::UPDATE),
    ("BridgeTowerBehavior", DIE_DAMAGE_MASK),
    ("OverchargeBehavior", UPDATE_DAMAGE_MASK),
    (
        "FireWeaponWhenDamagedBehavior",
        mask_or(UPDATE_DAMAGE_MASK, ModuleInterfaceType::UPGRADE),
    ),
    ("TransitionDamageFX", ModuleInterfaceType::DAMAGE),
    (
        "FireWeaponWhenDeadBehavior",
        mask_or(ModuleInterfaceType::UPGRADE, ModuleInterfaceType::DIE),
    ),
    ("BunkerBusterBehavior", UPDATE_DIE_MASK),
    ("GenerateMinefieldBehavior", UPDATE_DIE_UPGRADE_MASK),
    ("ParkingPlaceBehavior", UPDATE_DIE_MASK),
    ("FlightDeckBehavior", UPDATE_DIE_MASK),
    ("PoisonedBehavior", UPDATE_DAMAGE_MASK),
    ("RebuildHoleBehavior", UPDATE_DIE_MASK),
    ("SupplyWarehouseCripplingBehavior", UPDATE_DAMAGE_MASK),
    ("TechBuildingBehavior", UPDATE_DIE_MASK),
    ("MinefieldBehavior", UPDATE_DIE_DAMAGE_COLLIDE_MASK),
    ("GrantStealthBehavior", ModuleInterfaceType::UPDATE),
    ("NeutronBlastBehavior", UPDATE_DIE_MASK),
    (
        "CountermeasuresBehavior",
        mask_or(ModuleInterfaceType::UPDATE, ModuleInterfaceType::UPGRADE),
    ),
    ("DumbProjectileBehavior", ModuleInterfaceType::UPDATE),
    ("PhysicsBehavior", UPDATE_COLLIDE_MASK),
    ("SpawnBehavior", UPDATE_DIE_DAMAGE_MASK),
    ("HelicopterSlowDeathBehavior", UPDATE_DIE_MASK),
    ("NeutronMissileSlowDeathBehavior", UPDATE_DIE_MASK),
    ("JetSlowDeathBehavior", UPDATE_DIE_MASK),
    ("BattleBusSlowDeathBehavior", UPDATE_DIE_MASK),
    ("PropagandaTowerBehavior", UPDATE_DIE_MASK),
    ("RailroadBehavior", UPDATE_COLLIDE_MASK),
    ("OpenContain", OPEN_CONTAIN_MASK),
    ("TransportContain", OPEN_CONTAIN_MASK),
    ("TunnelContain", OPEN_CONTAIN_MASK),
    ("OverlordContain", OPEN_CONTAIN_MASK),
    ("HelixContain", OPEN_CONTAIN_MASK),
    ("ParachuteContain", OPEN_CONTAIN_MASK),
    ("MobNexusContain", OPEN_CONTAIN_MASK),
    ("RailedTransportContain", OPEN_CONTAIN_MASK),
    ("RiderChangeContain", OPEN_CONTAIN_MASK),
    ("InternetHackContain", OPEN_CONTAIN_MASK),
    ("GarrisonContain", OPEN_CONTAIN_MASK),
    ("HealContain", OPEN_CONTAIN_MASK),
    ("PrisonBehavior", OPEN_CONTAIN_MASK),
    ("PropagandaCenterBehavior", OPEN_CONTAIN_MASK),
    ("POWTruckBehavior", OPEN_CONTAIN_MASK),
    ("CaveContain", OPEN_CONTAIN_CREATE_MASK),
    ("DestroyDie", ModuleInterfaceType::DIE),
    ("FXListDie", ModuleInterfaceType::DIE),
    ("CrushDie", ModuleInterfaceType::DIE),
    ("DamDie", ModuleInterfaceType::DIE),
    ("CreateCrateDie", ModuleInterfaceType::DIE),
    ("CreateObjectDie", ModuleInterfaceType::DIE),
    ("EjectPilotDie", ModuleInterfaceType::DIE),
    ("SpecialPowerCompletionDie", ModuleInterfaceType::DIE),
    ("RebuildHoleExposeDie", ModuleInterfaceType::DIE),
    ("UpgradeDie", ModuleInterfaceType::DIE),
    ("KeepObjectDie", ModuleInterfaceType::DIE),
    ("AssistedTargetingUpdate", ModuleInterfaceType::UPDATE),
    ("AutoFindHealingUpdate", ModuleInterfaceType::UPDATE),
    ("BaseRegenerateUpdate", UPDATE_DAMAGE_MASK),
    ("StealthDetectorUpdate", ModuleInterfaceType::UPDATE),
    ("DeletionUpdate", ModuleInterfaceType::UPDATE),
    ("SmartBombTargetHomingUpdate", ModuleInterfaceType::UPDATE),
    (
        "DynamicShroudClearingRangeUpdate",
        ModuleInterfaceType::UPDATE,
    ),
    ("LeafletDropBehavior", UPDATE_DIE_MASK),
    ("DeployStyleAIUpdate", ModuleInterfaceType::UPDATE),
    ("AssaultTransportAIUpdate", ModuleInterfaceType::UPDATE),
    ("HordeUpdate", ModuleInterfaceType::UPDATE),
    ("ToppleUpdate", UPDATE_COLLIDE_MASK),
    ("EnemyNearUpdate", ModuleInterfaceType::UPDATE),
    ("LifetimeUpdate", ModuleInterfaceType::UPDATE),
    ("RadiusDecalUpdate", ModuleInterfaceType::UPDATE),
    ("EMPUpdate", ModuleInterfaceType::UPDATE),
    ("AutoDepositUpdate", ModuleInterfaceType::UPDATE),
    ("WeaponBonusUpdate", ModuleInterfaceType::UPDATE),
    ("MissileAIUpdate", ModuleInterfaceType::UPDATE),
    ("NeutronMissileUpdate", UPDATE_DIE_MASK),
    ("FireSpreadUpdate", ModuleInterfaceType::UPDATE),
    ("FireWeaponUpdate", ModuleInterfaceType::UPDATE),
    ("FlammableUpdate", UPDATE_DAMAGE_MASK),
    ("FloatUpdate", ModuleInterfaceType::UPDATE),
    ("TensileFormationUpdate", ModuleInterfaceType::UPDATE),
    ("HeightDieUpdate", ModuleInterfaceType::UPDATE),
    ("ChinookAIUpdate", ModuleInterfaceType::UPDATE),
    ("JetAIUpdate", ModuleInterfaceType::UPDATE),
    ("AIUpdateInterface", ModuleInterfaceType::UPDATE),
    ("SupplyTruckAIUpdate", ModuleInterfaceType::UPDATE),
    ("DeliverPayloadAIUpdate", ModuleInterfaceType::UPDATE),
    ("HackInternetAIUpdate", ModuleInterfaceType::UPDATE),
    ("DynamicGeometryInfoUpdate", ModuleInterfaceType::UPDATE),
    (
        "FirestormDynamicGeometryInfoUpdate",
        ModuleInterfaceType::UPDATE,
    ),
    ("LaserUpdate", ModuleInterfaceType::CLIENT_UPDATE),
    ("PointDefenseLaserUpdate", ModuleInterfaceType::UPDATE),
    ("CleanupHazardUpdate", ModuleInterfaceType::UPDATE),
    ("CommandButtonHuntUpdate", ModuleInterfaceType::UPDATE),
    ("PilotFindVehicleUpdate", ModuleInterfaceType::UPDATE),
    ("DemoTrapUpdate", ModuleInterfaceType::UPDATE),
    ("ParticleUplinkCannonUpdate", ModuleInterfaceType::UPDATE),
    ("SpectreGunshipUpdate", ModuleInterfaceType::UPDATE),
    (
        "SpectreGunshipDeploymentUpdate",
        ModuleInterfaceType::UPDATE,
    ),
    ("BaikonurLaunchPower", ModuleInterfaceType::UPDATE),
    ("BattlePlanUpdate", ModuleInterfaceType::UPDATE),
    ("ProjectileStreamUpdate", ModuleInterfaceType::UPDATE),
    ("QueueProductionExitUpdate", ModuleInterfaceType::UPDATE),
    ("RepairDockUpdate", ModuleInterfaceType::UPDATE),
    ("PrisonDockUpdate", ModuleInterfaceType::UPDATE),
    ("RailedTransportDockUpdate", ModuleInterfaceType::UPDATE),
    ("DefaultProductionExitUpdate", ModuleInterfaceType::UPDATE),
    (
        "SpawnPointProductionExitUpdate",
        ModuleInterfaceType::UPDATE,
    ),
    (
        "SpyVisionUpdate",
        mask_or(ModuleInterfaceType::UPDATE, ModuleInterfaceType::UPGRADE),
    ),
    ("SlavedUpdate", ModuleInterfaceType::UPDATE),
    ("MobMemberSlavedUpdate", ModuleInterfaceType::UPDATE),
    ("OCLUpdate", ModuleInterfaceType::UPDATE),
    ("SpecialAbilityUpdate", ModuleInterfaceType::UPDATE),
    ("MissileLauncherBuildingUpdate", ModuleInterfaceType::UPDATE),
    (
        "SupplyCenterProductionExitUpdate",
        ModuleInterfaceType::UPDATE,
    ),
    ("SupplyCenterDockUpdate", ModuleInterfaceType::UPDATE),
    ("SupplyWarehouseDockUpdate", ModuleInterfaceType::UPDATE),
    ("DozerAIUpdate", ModuleInterfaceType::UPDATE),
    ("POWTruckAIUpdate", ModuleInterfaceType::UPDATE),
    ("RailedTransportAIUpdate", ModuleInterfaceType::UPDATE),
    ("ProductionUpdate", UPDATE_DIE_MASK),
    ("ProneUpdate", ModuleInterfaceType::UPDATE),
    ("StickyBombUpdate", ModuleInterfaceType::UPDATE),
    (
        "FireOCLAfterWeaponCooldownUpdate",
        ModuleInterfaceType::UPDATE,
    ),
    ("HijackerUpdate", ModuleInterfaceType::UPDATE),
    ("StructureToppleUpdate", UPDATE_DIE_MASK),
    ("StructureCollapseUpdate", UPDATE_DIE_MASK),
    ("BoneFXUpdate", ModuleInterfaceType::UPDATE),
    ("RadarUpdate", ModuleInterfaceType::UPDATE),
    ("AnimationSteeringUpdate", ModuleInterfaceType::UPDATE),
    ("TransportAIUpdate", ModuleInterfaceType::UPDATE),
    ("WanderAIUpdate", ModuleInterfaceType::UPDATE),
    ("WaveGuideUpdate", ModuleInterfaceType::UPDATE),
    ("WorkerAIUpdate", ModuleInterfaceType::UPDATE),
    ("PowerPlantUpdate", ModuleInterfaceType::UPDATE),
    ("CheckpointUpdate", ModuleInterfaceType::UPDATE),
    ("CostModifierUpgrade", ModuleInterfaceType::UPGRADE),
    ("ActiveShroudUpgrade", ModuleInterfaceType::UPGRADE),
    ("ArmorUpgrade", ModuleInterfaceType::UPGRADE),
    ("CommandSetUpgrade", ModuleInterfaceType::UPGRADE),
    ("GrantScienceUpgrade", ModuleInterfaceType::UPGRADE),
    ("PassengersFireUpgrade", ModuleInterfaceType::UPGRADE),
    ("SubObjectsUpgrade", ModuleInterfaceType::UPGRADE),
    ("StealthUpgrade", ModuleInterfaceType::UPGRADE),
    ("RadarUpgrade", ModuleInterfaceType::UPGRADE),
    ("PowerPlantUpgrade", ModuleInterfaceType::UPGRADE),
    ("LocomotorSetUpgrade", ModuleInterfaceType::UPGRADE),
    ("ObjectCreationUpgrade", ModuleInterfaceType::UPGRADE),
    ("ReplaceObjectUpgrade", ModuleInterfaceType::UPGRADE),
    ("ModelConditionUpgrade", ModuleInterfaceType::UPGRADE),
    ("UnpauseSpecialPowerUpgrade", ModuleInterfaceType::UPGRADE),
    ("WeaponBonusUpgrade", ModuleInterfaceType::UPGRADE),
    ("WeaponSetUpgrade", ModuleInterfaceType::UPGRADE),
    ("ExperienceScalarUpgrade", ModuleInterfaceType::UPGRADE),
    ("MaxHealthUpgrade", ModuleInterfaceType::UPGRADE),
    ("LockWeaponCreate", ModuleInterfaceType::CREATE),
    ("PreorderCreate", ModuleInterfaceType::CREATE),
    ("SupplyCenterCreate", ModuleInterfaceType::CREATE),
    ("SupplyWarehouseCreate", ModuleInterfaceType::CREATE),
    ("SpecialPowerCreate", ModuleInterfaceType::CREATE),
    ("GrantUpgradeCreate", ModuleInterfaceType::CREATE),
    ("VeterancyGainCreate", ModuleInterfaceType::CREATE),
    ("BoneFXDamage", ModuleInterfaceType::DAMAGE),
    ("TransitionDamageFX", ModuleInterfaceType::DAMAGE),
    ("FireWeaponCollide", ModuleInterfaceType::COLLIDE),
    ("SquishCollide", ModuleInterfaceType::COLLIDE),
    ("HealCrateCollide", ModuleInterfaceType::COLLIDE),
    ("MoneyCrateCollide", ModuleInterfaceType::COLLIDE),
    ("ShroudCrateCollide", ModuleInterfaceType::COLLIDE),
    ("UnitCrateCollide", ModuleInterfaceType::COLLIDE),
    ("VeterancyCrateCollide", ModuleInterfaceType::COLLIDE),
    ("ConvertToCarBombCrateCollide", ModuleInterfaceType::COLLIDE),
    (
        "ConvertToHijackedVehicleCrateCollide",
        ModuleInterfaceType::COLLIDE,
    ),
    (
        "SabotageCommandCenterCrateCollide",
        ModuleInterfaceType::COLLIDE,
    ),
    (
        "SabotageFakeBuildingCrateCollide",
        ModuleInterfaceType::COLLIDE,
    ),
    (
        "SabotageInternetCenterCrateCollide",
        ModuleInterfaceType::COLLIDE,
    ),
    (
        "SabotageMilitaryFactoryCrateCollide",
        ModuleInterfaceType::COLLIDE,
    ),
    (
        "SabotagePowerPlantCrateCollide",
        ModuleInterfaceType::COLLIDE,
    ),
    (
        "SabotageSuperweaponCrateCollide",
        ModuleInterfaceType::COLLIDE,
    ),
    (
        "SabotageSupplyCenterCrateCollide",
        ModuleInterfaceType::COLLIDE,
    ),
    (
        "SabotageSupplyDropzoneCrateCollide",
        ModuleInterfaceType::COLLIDE,
    ),
    ("SalvageCrateCollide", ModuleInterfaceType::COLLIDE),
];

#[derive(Clone, Copy)]
struct ModuleOverride {
    create_proc: NewModuleProc,
    create_data_proc: NewModuleDataProc,
}

static MODULE_OVERRIDES: Lazy<Mutex<HashMap<NameKeyType, ModuleOverride>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
struct RegisteredDescriptor {
    module_type: ModuleType,
    descriptor: TemplateModuleDescriptor,
}

#[derive(Clone)]
struct PendingDescriptor {
    key: NameKeyType,
    module_type: ModuleType,
    descriptor: TemplateModuleDescriptor,
}

static PENDING_DESCRIPTORS: Lazy<Mutex<Vec<PendingDescriptor>>> =
    Lazy::new(|| Mutex::new(Vec::new()));

pub struct ModuleFactory {
    module_template_map: HashMap<NameKeyType, ModuleTemplate>,
    module_data_list: Vec<Arc<dyn ModuleData>>,
    descriptor_catalog: HashMap<NameKeyType, RegisteredDescriptor>,
    descriptor_order: [Vec<NameKeyType>; ModuleType::NUM_MODULE_TYPES],
}

#[derive(Clone)]
struct DescriptorRecord {
    name: AsciiString,
    interface_mask: ModuleInterfaceType,
    inheritable: bool,
    overrideable_by_like_kind: bool,
    copied_from_default: bool,
}

/// Template hash size constant
const TEMPLATE_HASH_SIZE: usize = 4096;

/// Function pointer type for creating new modules
pub type NewModuleProc =
    fn(thing: Arc<dyn Thing>, module_data: Arc<dyn ModuleData>) -> Box<dyn Module>;

/// Function pointer type for creating new module data
pub type NewModuleDataProc = fn(ini: Option<&mut INI>) -> Box<dyn ModuleData>;

// Stub module creation removed; missing modules must be implemented.

fn enqueue_pending_descriptor(module_type: ModuleType, descriptor: &TemplateModuleDescriptor) {
    let key = decorated_name_key(descriptor.name.as_str(), module_type);

    if let Ok(mut pending) = PENDING_DESCRIPTORS.lock() {
        if let Some(existing) = pending.iter_mut().find(|entry| entry.key == key) {
            let combined_mask = existing.descriptor.interface_mask.0 | descriptor.interface_mask.0;
            existing.descriptor.interface_mask = ModuleInterfaceType(combined_mask);
            existing.descriptor.inheritable |= descriptor.inheritable;
            existing.descriptor.overrideable_by_like_kind |= descriptor.overrideable_by_like_kind;
            existing.descriptor.copied_from_default &= descriptor.copied_from_default;
        } else {
            pending.push(PendingDescriptor {
                key,
                module_type,
                descriptor: descriptor.clone(),
            });
        }
    }
}

fn cache_descriptor_set(descriptor_set: &TemplateModuleDescriptorSet) {
    for descriptor in &descriptor_set.behavior {
        enqueue_pending_descriptor(ModuleType::Behavior, descriptor);
    }
    for descriptor in &descriptor_set.draw {
        enqueue_pending_descriptor(ModuleType::Draw, descriptor);
    }
    for descriptor in &descriptor_set.client_update {
        enqueue_pending_descriptor(ModuleType::ClientUpdate, descriptor);
    }
}

/// Module template containing creation functions and interface info
#[derive(Clone)]
pub struct ModuleTemplate {
    module_name: String,
    module_type: ModuleType,
    pub create_proc: Option<NewModuleProc>,
    pub create_data_proc: Option<NewModuleDataProc>,
    pub which_interfaces: ModuleInterfaceType,
}

impl ModuleTemplate {
    pub fn new(
        module_name: String,
        module_type: ModuleType,
        create_proc: Option<NewModuleProc>,
        create_data_proc: Option<NewModuleDataProc>,
        which_interfaces: ModuleInterfaceType,
    ) -> Self {
        Self {
            module_name,
            module_type,
            create_proc,
            create_data_proc,
            which_interfaces,
        }
    }

    fn create_module(
        &self,
        thing: Arc<dyn Thing>,
        module_data: Arc<dyn ModuleData>,
    ) -> Box<dyn Module> {
        if let Some(proc) = self.create_proc {
            proc(thing, module_data)
        } else {
            panic!(
                "Missing module implementation for '{}' ({:?})",
                self.module_name, self.module_type
            );
        }
    }

    fn create_module_data(&self, ini: Option<&mut INI>) -> Box<dyn ModuleData> {
        if let Some(proc) = self.create_data_proc {
            proc(ini)
        } else {
            let _ = ini;
            panic!(
                "Missing module data implementation for '{}' ({:?})",
                self.module_name, self.module_type
            );
        }
    }

    fn is_stub(&self) -> bool {
        self.create_proc.is_none()
    }

    fn module_name(&self) -> &str {
        &self.module_name
    }
}

fn decorated_name_key(name: &str, module_type: ModuleType) -> NameKeyType {
    let decorated = format!("{}{}", module_type as u8, name);
    NameKeyGenerator::name_to_key(&decorated)
}

pub fn register_module_override(
    name: &str,
    module_type: ModuleType,
    create_proc: NewModuleProc,
    create_data_proc: NewModuleDataProc,
) -> Result<(), String> {
    if name.is_empty() {
        return Err("module override name cannot be empty".to_string());
    }

    let key = decorated_name_key(name, module_type);
    let mut registry = MODULE_OVERRIDES
        .lock()
        .map_err(|_| "module override registry poisoned".to_string())?;
    registry.insert(
        key,
        ModuleOverride {
            create_proc,
            create_data_proc,
        },
    );
    Ok(())
}

/// Registers a descriptor set with the global factory if available, otherwise caches it
/// until initialization completes.
pub fn register_descriptor_set_global(descriptor_set: &TemplateModuleDescriptorSet) {
    let should_cache = {
        if let Ok(mut guard) = MODULE_FACTORY.lock() {
            if let Some(factory) = guard.as_mut() {
                factory.register_descriptor_set(descriptor_set);
                false
            } else {
                true
            }
        } else {
            true
        }
    };

    if should_cache {
        cache_descriptor_set(descriptor_set);
    }
}

#[cfg(test)]
fn clear_module_overrides_for_test() {
    if let Ok(mut registry) = MODULE_OVERRIDES.lock() {
        registry.clear();
    }
}

#[cfg(test)]
pub(crate) fn clear_pending_descriptors_for_test() {
    if let Ok(mut pending) = PENDING_DESCRIPTORS.lock() {
        pending.clear();
    }
}

impl ModuleFactory {
    /// Registers the descriptors advertised by a `ThingTemplate`.
    ///
    /// Each descriptor seeds the factory's metadata catalog and ensures the corresponding
    /// module template exists so objects can request instances at runtime.  The record is
    /// deduplicated by module type and name, allowing multiple templates to contribute
    /// interface flags for the same module without maintaining bespoke static tables.
    pub fn register_descriptor_set(&mut self, descriptor_set: &TemplateModuleDescriptorSet) {
        self.register_template_descriptors(ModuleType::Behavior, &descriptor_set.behavior);
        self.register_template_descriptors(ModuleType::Draw, &descriptor_set.draw);
        self.register_template_descriptors(ModuleType::ClientUpdate, &descriptor_set.client_update);
    }

    fn record_descriptor(
        &mut self,
        key: NameKeyType,
        module_type: ModuleType,
        descriptor: &TemplateModuleDescriptor,
    ) {
        match self.descriptor_catalog.entry(key) {
            Entry::Vacant(slot) => {
                slot.insert(RegisteredDescriptor {
                    module_type,
                    descriptor: descriptor.clone(),
                });
                self.descriptor_order[module_type as usize].push(key);
            }
            Entry::Occupied(mut slot) => {
                let entry = slot.get_mut();
                debug_assert_eq!(
                    entry.module_type as u8, module_type as u8,
                    "module type mismatch while recording descriptor '{}'",
                    descriptor.name
                );

                let combined_mask = entry.descriptor.interface_mask.0 | descriptor.interface_mask.0;
                entry.descriptor.interface_mask = ModuleInterfaceType(combined_mask);
                entry.descriptor.inheritable |= descriptor.inheritable;
                entry.descriptor.overrideable_by_like_kind |= descriptor.overrideable_by_like_kind;
                entry.descriptor.copied_from_default &= descriptor.copied_from_default;
            }
        }
    }

    fn register_template_descriptors(
        &mut self,
        module_type: ModuleType,
        descriptors: &[TemplateModuleDescriptor],
    ) {
        for descriptor in descriptors {
            let key = self.make_decorated_name_key(&descriptor.name, module_type);
            self.record_descriptor(key, module_type, descriptor);

            let interface_bits = descriptor.interface_mask;
            if let Some(existing) = self.module_template_map.get_mut(&key) {
                if interface_bits.0 & !existing.which_interfaces.0 != 0 {
                    existing.which_interfaces.0 |= interface_bits.0;
                }
                continue;
            }

            self.add_module_internal(None, None, module_type, &descriptor.name, interface_bits);
        }
    }

    fn seed_builtin_descriptors(&mut self) {
        for (name, mask) in BUILTIN_BEHAVIOR_DESCRIPTORS {
            let ascii_name = AsciiString::from(*name);
            let key = self.make_decorated_name_key(&ascii_name, ModuleType::Behavior);

            let descriptor = TemplateModuleDescriptor {
                name: ascii_name.clone(),
                module_tag: AsciiString::new(),
                interface_mask: *mask,
                inheritable: false,
                overrideable_by_like_kind: false,
                copied_from_default: false,
            };

            self.record_descriptor(key, ModuleType::Behavior, &descriptor);

            if let Some(existing) = self.module_template_map.get_mut(&key) {
                if mask.0 & !existing.which_interfaces.0 != 0 {
                    existing.which_interfaces.0 |= mask.0;
                }
                continue;
            }

            self.add_module_internal(None, None, ModuleType::Behavior, &ascii_name, *mask);
        }
    }

    pub fn new() -> Self {
        let mut factory = Self {
            module_template_map: HashMap::with_capacity(TEMPLATE_HASH_SIZE),
            module_data_list: Vec::new(),
            descriptor_catalog: HashMap::new(),
            descriptor_order: std::array::from_fn(|_| Vec::new()),
        };
        factory.seed_builtin_descriptors();
        factory.absorb_pending_descriptors();
        factory
    }

    /// Allocate a new module given the name and data
    pub fn new_module(
        &self,
        thing: Arc<dyn Thing>,
        name: &str,
        module_data: Arc<dyn ModuleData>,
        module_type: ModuleType,
    ) -> Result<Box<dyn Module>, String> {
        if name.is_empty() {
            return Err("attempting to create module with empty name".to_string());
        }

        let template = self
            .find_module_template(name, module_type)
            .ok_or_else(|| format!("Module template '{}' not found", name))?;

        let module = template.create_module(thing, module_data);

        Ok(module)
    }

    /// Create module data from INI
    pub fn new_module_data_from_ini(
        &mut self,
        ini: Option<&mut INI>,
        name: &str,
        module_type: ModuleType,
        module_tag: &str,
    ) -> Result<Arc<dyn ModuleData>, String> {
        if name.is_empty() {
            return Err("Module name cannot be empty".to_string());
        }

        let template = self
            .find_module_template(name, module_type)
            .ok_or_else(|| format!("Module template '{}' not found", name))?;

        let mut module_data = template.create_module_data(ini);

        // Set the module tag name key
        let module_tag_key = self.string_to_name_key(module_tag);
        module_data.set_module_tag_name_key(module_tag_key);

        let arc_data: Arc<dyn ModuleData> = Arc::from(module_data);
        self.module_data_list.push(arc_data.clone());

        Ok(arc_data)
    }

    /// Find the interface mask for a module
    pub fn find_module_interface_mask(
        &self,
        name: &str,
        module_type: ModuleType,
    ) -> ModuleInterfaceType {
        if name.is_empty() {
            return ModuleInterfaceType::NONE;
        }

        self.find_module_template(name, module_type)
            .map(|template| template.which_interfaces)
            .unwrap_or(ModuleInterfaceType::NONE)
    }

    /// Look up the registered descriptor metadata for a module.
    pub fn descriptor_for(
        &self,
        module_type: ModuleType,
        name: &str,
    ) -> Option<&TemplateModuleDescriptor> {
        let key = self.make_decorated_name_key(name, module_type);
        self.descriptor_catalog
            .get(&key)
            .map(|entry| &entry.descriptor)
    }

    /// Returns all registered descriptors for the requested module type.
    pub fn descriptors_for_type(&self, module_type: ModuleType) -> Vec<&TemplateModuleDescriptor> {
        self.descriptor_order[module_type as usize]
            .iter()
            .filter_map(|key| self.descriptor_catalog.get(key))
            .map(|entry| &entry.descriptor)
            .collect()
    }

    /// Returns descriptors sorted by the legacy registration order.
    pub fn descriptors_in_registration_order(
        &self,
        module_type: ModuleType,
    ) -> Vec<&TemplateModuleDescriptor> {
        self.descriptors_for_type(module_type)
    }

    /// Returns the names of modules that still lack implementations.
    pub fn stubbed_module_names(&self, module_type: ModuleType) -> Vec<&str> {
        self.descriptor_order[module_type as usize]
            .iter()
            .filter_map(|key| self.module_template_map.get(key))
            .filter(|template| template.is_stub())
            .map(|template| template.module_name())
            .collect()
    }

    /// Add a module template to the factory
    pub fn add_module_internal(
        &mut self,
        proc: Option<NewModuleProc>,
        data_proc: Option<NewModuleDataProc>,
        module_type: ModuleType,
        name: &str,
        which_interfaces: ModuleInterfaceType,
    ) {
        let name_key = self.make_decorated_name_key(name, module_type);
        let (proc, data_proc) = MODULE_OVERRIDES
            .lock()
            .map(|registry| registry.get(&name_key).copied())
            .unwrap_or(None)
            .map(|override_entry| {
                (
                    Some(override_entry.create_proc),
                    Some(override_entry.create_data_proc),
                )
            })
            .unwrap_or((proc, data_proc));

        let template = ModuleTemplate::new(
            name.to_string(),
            module_type,
            proc,
            data_proc,
            which_interfaces,
        );
        self.module_template_map.insert(name_key, template);
    }

    /// Find a module template by name and type
    fn find_module_template(&self, name: &str, module_type: ModuleType) -> Option<&ModuleTemplate> {
        let name_key = self.make_decorated_name_key(name, module_type);
        self.module_template_map.get(&name_key)
    }

    /// Make a decorated name key for module template lookup
    fn make_decorated_name_key(&self, name: &str, module_type: ModuleType) -> NameKeyType {
        decorated_name_key(name, module_type)
    }

    /// Convert string to name key (simplified implementation)
    fn string_to_name_key(&self, s: &str) -> NameKeyType {
        NameKeyGenerator::name_to_key(s)
    }

    fn absorb_pending_descriptors(&mut self) {
        let pending_entries = if let Ok(mut pending) = PENDING_DESCRIPTORS.lock() {
            mem::take(&mut *pending)
        } else {
            Vec::new()
        };

        for PendingDescriptor {
            key: _,
            module_type,
            descriptor,
        } in pending_entries
        {
            let descriptor_array = [descriptor];
            self.register_template_descriptors(module_type, &descriptor_array);
        }
    }
}

impl SubsystemInterface for ModuleFactory {
    fn name(&self) -> &str {
        "ModuleFactory"
    }

    fn init(&mut self) -> crate::common::system::subsystem_interface::SubsystemResult<()> {
        // Initialize all module types
        self.init_behavior_modules();
        self.init_draw_modules();
        self.init_update_modules();
        self.init_upgrade_modules();
        self.init_other_modules();

        Ok(())
    }

    fn reset(&mut self) -> crate::common::system::subsystem_interface::SubsystemResult<()> {
        // Module factory doesn't reset during app lifetime
        Ok(())
    }

    fn update(
        &mut self,
        _delta_time: std::time::Duration,
    ) -> crate::common::system::subsystem_interface::SubsystemResult<()> {
        // Module factory doesn't need regular updates
        Ok(())
    }

    fn shutdown(&mut self) -> crate::common::system::subsystem_interface::SubsystemResult<()> {
        // Clear the factory state
        Ok(())
    }

    fn state(&self) -> crate::common::system::subsystem_interface::SubsystemState {
        crate::common::system::subsystem_interface::SubsystemState::Running
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }
}

impl ModuleFactory {
    /// Initialize behavior modules
    fn init_behavior_modules(&mut self) {
        // Legacy descriptors are now seeded at runtime via object descriptor registration.
    }

    /// Initialize draw modules
    fn init_draw_modules(&mut self) {
        // handled via descriptor registration
    }

    /// Initialize update modules
    fn init_update_modules(&mut self) {
        // handled via descriptor registration
    }

    /// Initialize upgrade modules
    fn init_upgrade_modules(&mut self) {
        // handled via descriptor registration
    }

    /// Initialize other module types
    fn init_other_modules(&mut self) {
        // handled via descriptor registration
    }
}

impl Snapshotable for ModuleFactory {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        for module_data in &self.module_data_list {
            module_data.crc(xfer)?;
        }
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // Module factory serialization
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Post-load processing if needed
        Ok(())
    }
}

/// Global module factory singleton
static MODULE_FACTORY: Mutex<Option<ModuleFactory>> = Mutex::new(None);

/// Get the global module factory instance
pub fn get_module_factory() -> Result<
    std::sync::MutexGuard<'static, Option<ModuleFactory>>,
    std::sync::PoisonError<std::sync::MutexGuard<'static, Option<ModuleFactory>>>,
> {
    MODULE_FACTORY.lock()
}

/// Initialize the global module factory
pub fn init_module_factory() -> Result<(), String> {
    let mut factory_guard = get_module_factory().map_err(|_| "Failed to lock module factory")?;
    let mut factory = ModuleFactory::new();
    factory
        .init()
        .map_err(|e| format!("Failed to initialize module factory: {:?}", e))?;
    *factory_guard = Some(factory);
    Ok(())
}

/// Apply registered override constructors to any templates already present in the global factory.
///
/// This supports late override registration (e.g. game-logic override installation after
/// module-factory initialization) by rebinding constructor pointers in place.
pub fn apply_module_overrides_to_existing_templates() -> Result<(), String> {
    let overrides = MODULE_OVERRIDES
        .lock()
        .map_err(|_| "module override registry poisoned".to_string())?;

    let mut factory_guard =
        get_module_factory().map_err(|_| "Failed to lock module factory".to_string())?;
    let Some(factory) = factory_guard.as_mut() else {
        return Ok(());
    };

    for (key, template) in &mut factory.module_template_map {
        if let Some(override_entry) = overrides.get(key) {
            template.create_proc = Some(override_entry.create_proc);
            template.create_data_proc = Some(override_entry.create_data_proc);
        }
    }

    Ok(())
}

/// Shutdown the global module factory
pub fn shutdown_module_factory() {
    if let Ok(mut factory_guard) = get_module_factory() {
        *factory_guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::system::Xfer;
    use crate::common::thing::module::{
        Module, ModuleData, ModuleInterfaceType, ModuleType, Thing,
    };
    use std::any::Any;
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Arc;

    #[test]
    fn interface_masks_match_cpp_signatures() {
        fn descriptor_for_any<'a>(
            factory: &'a ModuleFactory,
            name: &AsciiString,
        ) -> Option<&'a TemplateModuleDescriptor> {
            const TYPES: [ModuleType; 3] = [
                ModuleType::Behavior,
                ModuleType::Draw,
                ModuleType::ClientUpdate,
            ];
            TYPES
                .iter()
                .find_map(|ty| factory.descriptor_for(*ty, name))
        }

        fn assert_mask(factory: &ModuleFactory, name: &str, expected: ModuleInterfaceType) {
            let ascii = AsciiString::from(name);
            let descriptor = descriptor_for_any(factory, &ascii)
                .unwrap_or_else(|| panic!("descriptor for {} missing", name));
            assert_eq!(
                descriptor.interface_mask, expected,
                "interface mask for {} differs",
                name
            );
        }

        let factory = ModuleFactory::new();

        assert_mask(
            &factory,
            "GenerateMinefieldBehavior",
            ModuleInterfaceType::UPDATE | ModuleInterfaceType::DIE | ModuleInterfaceType::UPGRADE,
        );
        assert_mask(
            &factory,
            "NeutronMissileUpdate",
            ModuleInterfaceType::UPDATE | ModuleInterfaceType::DIE,
        );
        assert_mask(
            &factory,
            "SpyVisionUpdate",
            ModuleInterfaceType::UPDATE | ModuleInterfaceType::UPGRADE,
        );
        assert_mask(
            &factory,
            "ProductionUpdate",
            ModuleInterfaceType::UPDATE | ModuleInterfaceType::DIE,
        );
        assert_mask(&factory, "LockWeaponCreate", ModuleInterfaceType::CREATE);
        assert_mask(&factory, "BoneFXDamage", ModuleInterfaceType::DAMAGE);
        assert_mask(&factory, "FireWeaponCollide", ModuleInterfaceType::COLLIDE);
        assert_mask(&factory, "LaserUpdate", ModuleInterfaceType::CLIENT_UPDATE);
        assert_mask(
            &factory,
            "SpecialAbilityUpdate",
            ModuleInterfaceType::UPDATE,
        );
        assert_mask(
            &factory,
            "MissileLauncherBuildingUpdate",
            ModuleInterfaceType::UPDATE,
        );

        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mask_path = manifest_dir.join("../../../../tools/module_interface_masks.json");
        if mask_path.exists() {
            let json = std::fs::read_to_string(&mask_path)
                .expect("failed to read module_interface_masks.json");
            let mask_map: HashMap<String, u32> =
                serde_json::from_str(&json).expect("invalid module_interface_masks.json");

            for (name, cpp_mask) in mask_map {
                let ascii = AsciiString::from(name.as_str());
                let descriptor = descriptor_for_any(&factory, &ascii)
                    .unwrap_or_else(|| panic!("descriptor for {} missing", name));
                assert_eq!(
                    descriptor.interface_mask.0, cpp_mask,
                    "interface mask for {} diverges from C++",
                    name
                );
            }
        }
    }

    #[derive(Debug)]
    struct StubThing;

    impl Thing for StubThing {
        // Uses default `Thing` methods.
    }

    #[derive(Debug)]
    struct StubModuleData;

    impl ModuleData for StubModuleData {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn set_module_tag_name_key(&mut self, _key: NameKeyType) {}
        fn get_module_tag_name_key(&self) -> NameKeyType {
            0
        }
    }

    impl Snapshotable for StubModuleData {
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
    struct TestStubModule;
    static TEST_STUB_MODULE_DATA: StubModuleData = StubModuleData;

    impl Snapshotable for TestStubModule {
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

    impl Module for TestStubModule {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn get_module_name_key(&self) -> NameKeyType {
            0
        }

        fn get_module_tag_name_key(&self) -> NameKeyType {
            0
        }

        fn get_module_data(&self) -> &dyn ModuleData {
            &TEST_STUB_MODULE_DATA
        }
    }

    fn test_new_module(
        _thing: Arc<dyn Thing>,
        _module_data: Arc<dyn ModuleData>,
    ) -> Box<dyn Module> {
        Box::new(TestStubModule)
    }

    fn test_new_module_data(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
        Box::new(StubModuleData)
    }

    fn override_new_module(
        _thing: Arc<dyn Thing>,
        _module_data: Arc<dyn ModuleData>,
    ) -> Box<dyn Module> {
        Box::new(TestStubModule)
    }

    fn override_new_module_data(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
        Box::new(StubModuleData)
    }

    fn make_descriptor(
        name: &str,
        tag: &str,
        interface_mask: ModuleInterfaceType,
    ) -> TemplateModuleDescriptor {
        TemplateModuleDescriptor {
            name: AsciiString::from(name),
            module_tag: AsciiString::from(tag),
            interface_mask,
            inheritable: false,
            overrideable_by_like_kind: false,
            copied_from_default: false,
        }
    }

    #[test]
    fn register_override_swaps_constructors() {
        clear_pending_descriptors_for_test();
        clear_module_overrides_for_test();

        register_module_override(
            "StubModule",
            ModuleType::Behavior,
            override_new_module,
            override_new_module_data,
        )
        .expect("failed to register override");

        let mut factory = ModuleFactory::new();
        let name = AsciiString::from("StubModule");
        factory.add_module_internal(
            Some(test_new_module),
            Some(test_new_module_data),
            ModuleType::Behavior,
            &name,
            ModuleInterfaceType::NONE,
        );

        let template = factory
            .find_module_template(&name, ModuleType::Behavior)
            .expect("template should exist");

        assert!(
            matches!(template.create_proc, Some(proc) if proc as usize == override_new_module as usize),
            "override should swap constructor"
        );
    }

    #[test]
    fn builtin_descriptors_seeded_on_construction() {
        clear_pending_descriptors_for_test();
        clear_module_overrides_for_test();

        let factory = ModuleFactory::new();

        let status_bits = AsciiString::from("StatusBitsUpgrade");
        let descriptor = factory
            .descriptor_for(ModuleType::Behavior, &status_bits)
            .expect("builtin descriptor should be registered");
        assert_eq!(descriptor.interface_mask, ModuleInterfaceType::UPGRADE);

        let template = factory
            .find_module_template(&status_bits, ModuleType::Behavior)
            .expect("builtin template should be registered");
        assert_eq!(template.which_interfaces, ModuleInterfaceType::UPGRADE);
    }

    #[test]
    fn descriptor_registration_populates_catalog() {
        clear_pending_descriptors_for_test();
        let mut factory = ModuleFactory::new();

        let mut set = TemplateModuleDescriptorSet::default();
        set.behavior.push(make_descriptor(
            "AutoHealBehavior",
            "TagBehavior",
            ModuleInterfaceType::BODY,
        ));
        set.draw.push(make_descriptor(
            "W3DModelDraw",
            "TagDraw",
            ModuleInterfaceType::DRAW,
        ));

        factory.register_descriptor_set(&set);

        let behavior_name = AsciiString::from("AutoHealBehavior");
        let behavior_descriptor = factory
            .descriptor_for(ModuleType::Behavior, &behavior_name)
            .expect("behavior descriptor registered");
        assert_eq!(
            behavior_descriptor.interface_mask,
            ModuleInterfaceType::BODY
        );

        let template = factory
            .find_module_template(&behavior_name, ModuleType::Behavior)
            .expect("behavior template registered");
        assert_eq!(template.which_interfaces, ModuleInterfaceType::BODY);

        assert_eq!(
            factory.descriptors_for_type(ModuleType::Draw).len(),
            1,
            "draw descriptor should be recorded"
        );
    }

    #[test]
    fn descriptor_registration_merges_interface_masks() {
        clear_pending_descriptors_for_test();
        let mut factory = ModuleFactory::new();

        let mut set = TemplateModuleDescriptorSet::default();
        set.behavior.push(make_descriptor(
            "TunnelContain",
            "PrimaryTag",
            ModuleInterfaceType::CONTAIN,
        ));
        set.behavior.push(make_descriptor(
            "TunnelContain",
            "SecondaryTag",
            ModuleInterfaceType::UPDATE,
        ));

        factory.register_descriptor_set(&set);

        let name = AsciiString::from("TunnelContain");
        let descriptor = factory
            .descriptor_for(ModuleType::Behavior, &name)
            .expect("descriptor should be registered");
        let expected_mask = (ModuleInterfaceType::CONTAIN | ModuleInterfaceType::UPDATE).0;
        assert_eq!(descriptor.interface_mask.0, expected_mask);

        let template = factory
            .find_module_template(&name, ModuleType::Behavior)
            .expect("template should be registered");
        assert_eq!(template.which_interfaces.0, expected_mask);
    }

    #[test]
    fn global_registration_caches_until_factory_ready() {
        clear_pending_descriptors_for_test();
        let mut guard = get_module_factory().expect("module factory mutex poisoned");
        let previous = guard.take();
        drop(guard);

        let mut set = TemplateModuleDescriptorSet::default();
        set.behavior.push(make_descriptor(
            "AutoHealBehavior",
            "TagBehavior",
            ModuleInterfaceType::BODY,
        ));

        register_descriptor_set_global(&set);

        let factory = ModuleFactory::new();
        let behavior_name = AsciiString::from("AutoHealBehavior");
        assert!(
            factory
                .descriptor_for(ModuleType::Behavior, &behavior_name)
                .is_some(),
            "descriptor should surface after factory construction"
        );

        let mut guard = get_module_factory().expect("module factory mutex poisoned");
        *guard = previous;
        drop(guard);
        clear_pending_descriptors_for_test();
    }

    #[test]
    fn descriptors_preserve_registration_order() {
        clear_pending_descriptors_for_test();
        clear_module_overrides_for_test();
        let mut factory = ModuleFactory::new();

        let mut set = TemplateModuleDescriptorSet::default();
        set.behavior.push(make_descriptor(
            "FirstBehavior",
            "TagFirst",
            ModuleInterfaceType::BODY,
        ));
        set.behavior.push(make_descriptor(
            "SecondBehavior",
            "TagSecond",
            ModuleInterfaceType::CONTAIN,
        ));

        factory.register_descriptor_set(&set);

        let ordered = factory.descriptors_in_registration_order(ModuleType::Behavior);
        let names: Vec<&str> = ordered
            .into_iter()
            .map(|descriptor| descriptor.name.as_str())
            .collect();

        assert!(names.len() >= 2, "expected at least two descriptors");

        let tail: Vec<&str> = names
            .iter()
            .rev()
            .take(2)
            .map(|name| *name)
            .collect::<Vec<&str>>()
            .into_iter()
            .rev()
            .collect();

        assert_eq!(tail, vec!["FirstBehavior", "SecondBehavior"]);
    }

    // Stub template ordering tests removed: missing modules now hard-fail instead of stubbing.

    #[test]
    fn pending_descriptors_merge_interface_flags() {
        clear_pending_descriptors_for_test();

        let mut set = TemplateModuleDescriptorSet::default();
        set.behavior.push(make_descriptor(
            "MergedBehavior",
            "TagPrimary",
            ModuleInterfaceType::BODY,
        ));
        set.behavior.push(make_descriptor(
            "MergedBehavior",
            "TagSecondary",
            ModuleInterfaceType::UPDATE,
        ));

        register_descriptor_set_global(&set);

        let factory = ModuleFactory::new();

        let name = AsciiString::from("MergedBehavior");
        let descriptor = factory
            .descriptor_for(ModuleType::Behavior, &name)
            .expect("descriptor should exist");
        assert_eq!(
            descriptor.interface_mask,
            ModuleInterfaceType::BODY | ModuleInterfaceType::UPDATE
        );

        let ordered = factory.descriptors_for_type(ModuleType::Behavior);
        assert_eq!(ordered.len(), 1, "descriptor should not be duplicated");

        clear_pending_descriptors_for_test();
    }
}
