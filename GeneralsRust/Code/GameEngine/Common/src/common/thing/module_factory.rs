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
use std::{
    any::Any,
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, Mutex},
};

mod behavior_interfaces;
pub use behavior_interfaces::builtin_behavior_has_ai_update_interface;

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
        mask_or(
            mask_or(ModuleInterfaceType::UPDATE, ModuleInterfaceType::UPGRADE),
            ModuleInterfaceType::DAMAGE,
        ),
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
    ("DefectorSpecialPower", ModuleInterfaceType::UPDATE),
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

// Names are case-sensitive, as in C++ makeDecoratedNameKey. The type index
// separates behavior, draw and client-update without borrowing a name-key namespace.
type ModuleMap<T> = [HashMap<String, T>; ModuleType::NUM_MODULE_TYPES];

fn module_maps<T>() -> ModuleMap<T> {
    std::array::from_fn(|_| HashMap::new())
}

#[derive(Clone)]
pub struct ModuleFactory {
    // Present only for an explicitly admitted receiving content owner.
    // This plain value moves with the loader; it is never installed globally.
    runtime_name_keys: Option<crate::common::name_key_generator::NameKeyNamespace>,
    receiving_sciences: Arc<crate::common::rts::science::ScienceNameFacts>,
    module_template_map: ModuleMap<ModuleTemplate>,
    module_overrides: ModuleMap<ModuleOverride>,
    module_data_list: Vec<Arc<dyn ModuleData>>,
    fallible_data_constructors: ModuleMap<FallibleModuleDataProc>,
    descriptor_catalog: ModuleMap<TemplateModuleDescriptor>,
    descriptor_order: [Vec<String>; ModuleType::NUM_MODULE_TYPES],
}

/// Function pointer type for creating new modules
pub type NewModuleProc =
    fn(thing: Arc<dyn Thing>, module_data: Arc<dyn ModuleData>) -> Box<dyn Module>;

/// Function pointer type for creating new module data
pub type NewModuleDataProc = fn(ini: Option<&mut INI>) -> Box<dyn ModuleData>;
/// Receiving-content admission must preserve the constructor's parse result.
pub type FallibleModuleDataProc = fn(ini: Option<&mut INI>) -> Result<Box<dyn ModuleData>, String>;

// Stub module creation removed; missing modules must be implemented.

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

/// Compatibility registration on the retained factory owner. Local rules owners
/// use `ModuleFactory::register_override`; no thread-local startup table exists.
pub fn register_module_override(
    name: &str,
    module_type: ModuleType,
    create_proc: NewModuleProc,
    create_data_proc: NewModuleDataProc,
) -> Result<(), String> {
    if name.is_empty() {
        return Err("module override name cannot be empty".to_owned());
    }
    let mut guard = get_module_factory().map_err(|_| "Failed to lock module factory")?;
    guard
        .get_or_insert_with(ModuleFactory::new)
        .register_override(name, module_type, create_proc, create_data_proc)
}

/// Compatibility descriptor admission on the same retained factory. Registration
/// before subsystem initialization needs no staging or later cross-owner import.
pub fn register_descriptor_set_global(descriptor_set: &TemplateModuleDescriptorSet) {
    if let Ok(mut guard) = get_module_factory() {
        guard
            .get_or_insert_with(ModuleFactory::new)
            .register_descriptor_set(descriptor_set);
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
        module_type: ModuleType,
        descriptor: &TemplateModuleDescriptor,
    ) {
        match self.descriptor_catalog[module_type as usize].entry(descriptor.name.to_string()) {
            Entry::Vacant(slot) => {
                self.descriptor_order[module_type as usize].push(slot.key().clone());
                slot.insert(descriptor.clone());
            }
            Entry::Occupied(mut slot) => {
                let entry = slot.get_mut();
                entry.interface_mask.0 |= descriptor.interface_mask.0;
                entry.inheritable |= descriptor.inheritable;
                entry.overrideable_by_like_kind |= descriptor.overrideable_by_like_kind;
                entry.copied_from_default &= descriptor.copied_from_default;
            }
        }
    }

    fn register_template_descriptors(
        &mut self,
        module_type: ModuleType,
        descriptors: &[TemplateModuleDescriptor],
    ) {
        for descriptor in descriptors {
            self.record_descriptor(module_type, descriptor);

            let interface_bits = descriptor.interface_mask;
            if let Some(existing) =
                self.module_template_map[module_type as usize].get_mut(descriptor.name.as_str())
            {
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
            let descriptor = TemplateModuleDescriptor {
                name: ascii_name.clone(),
                module_tag: AsciiString::new(),
                interface_mask: *mask,
                inheritable: false,
                overrideable_by_like_kind: false,
                copied_from_default: false,
            };

            self.record_descriptor(ModuleType::Behavior, &descriptor);

            if let Some(existing) =
                self.module_template_map[ModuleType::Behavior as usize].get_mut(*name)
            {
                if mask.0 & !existing.which_interfaces.0 != 0 {
                    existing.which_interfaces.0 |= mask.0;
                }
                continue;
            }

            self.add_module_internal(None, None, ModuleType::Behavior, &ascii_name, *mask);
        }
    }

    /// Construct an independent catalog. Startup registration is explicit;
    /// construction neither publishes a catalog nor allocates ambient name keys.
    pub fn new() -> Self {
        Self::new_with_name_keys(None)
    }

    /// Inert receiving content owner. Metadata discovery does not allocate keys;
    /// the actual original registration phase supplies its ordered class names.
    pub fn new_receiving(namespace: crate::common::name_key_generator::NameKeyNamespace) -> Self {
        Self::new_with_name_keys(Some(namespace))
    }

    pub fn new_receiving_with_sciences(
        namespace: crate::common::name_key_generator::NameKeyNamespace,
        sciences: Arc<crate::common::rts::science::ScienceNameFacts>,
    ) -> Self {
        let mut factory = Self::new_receiving(namespace);
        factory.receiving_sciences = sciences;
        factory
    }
    pub fn receiving_science_names(&self) -> &crate::common::rts::science::ScienceNameFacts {
        &self.receiving_sciences
    }

    fn new_with_name_keys(
        runtime_name_keys: Option<crate::common::name_key_generator::NameKeyNamespace>,
    ) -> Self {
        let mut factory = Self {
            runtime_name_keys,
            receiving_sciences: Arc::new(crate::common::rts::science::ScienceNameFacts::default()),
            module_template_map: module_maps(),
            module_overrides: module_maps(),
            module_data_list: Vec::new(),
            fallible_data_constructors: module_maps(),
            descriptor_catalog: module_maps(),
            descriptor_order: std::array::from_fn(|_| Vec::new()),
        };
        factory.seed_builtin_descriptors();
        factory
    }

    /// C++ ModuleFactory::makeDecoratedNameKey, called by actual registration/find.
    /// Descriptor advertisements deliberately do not enter this phase.
    pub fn admit_module_registration_key(&mut self, name: &str, module_type: ModuleType) {
        if let Some(keys) = self.runtime_name_keys.as_mut() {
            keys.name_to_key(&format!("{}{}", module_type as u8, name));
        }
    }

    #[cfg(test)]
    pub(crate) fn retained_module_data_count(&self) -> usize {
        self.module_data_list.len()
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

        if template.create_proc.is_none() {
            return Err(format!(
                "Missing module implementation for '{}' ({:?})",
                name, module_type
            ));
        }
        let module = template.create_module(thing, module_data);
        module.validate_initialization().map_err(|error| {
            format!("Module '{name}' ({module_type:?}) initialization failed: {error}")
        })?;
        Ok(module)
    }

    /// Fork immutable constructor registrations into a new explicit content owner.
    /// Existing runtime ModuleData is not copied into the new owner's retention list.
    pub fn receiving_catalog(
        &self,
        namespace: crate::common::name_key_generator::NameKeyNamespace,
    ) -> Self {
        let mut catalog = self.clone();
        catalog.module_data_list.clear();
        catalog.runtime_name_keys = Some(namespace);
        catalog
    }

    pub fn receiving_name_keys_mut(
        &mut self,
    ) -> Option<&mut crate::common::name_key_generator::NameKeyNamespace> {
        self.runtime_name_keys.as_mut()
    }

    pub(crate) fn validation_catalog(&self) -> Self {
        let mut catalog = self.clone();
        catalog.runtime_name_keys = None;
        catalog.module_data_list.clear();
        catalog
    }

    /// Install validation on this catalog only; no ambient registration.
    pub fn register_fallible_data_constructor(
        &mut self,
        name: &str,
        module_type: ModuleType,
        constructor: FallibleModuleDataProc,
    ) {
        self.fallible_data_constructors[module_type as usize].insert(name.to_owned(), constructor);
    }

    /// Strict admission never substitutes captured/default data for a failed
    /// or unavailable typed parser. Legacy infallible constructors need an
    /// explicit fallible adapter before they can certify receiving content.
    pub(crate) fn new_module_data_from_ini_strict(
        &mut self,
        ini: Option<&mut INI>,
        name: &str,
        module_type: ModuleType,
        module_tag: &str,
    ) -> Result<Arc<dyn ModuleData>, String> {
        self.admit_module_registration_key(name, module_type);
        let constructor = self.fallible_data_constructors[module_type as usize]
            .get(name)
            .copied()
            .ok_or_else(|| {
                format!("Module '{name}' ({module_type:?}) has no fallible data constructor")
            })?;
        // Move the one receiving namespace through the actual typed parser.
        // Detached validation receives a private throw-away namespace and remains
        // non-runnable; neither path selects the thread's compatibility registry.
        let receiving = self.runtime_name_keys.is_some();
        let keys = self.runtime_name_keys.take().unwrap_or_default();
        let (keys, result) = match ini {
            Some(ini) => ini.with_receiving_science_names(self.receiving_sciences.clone(), |ini| {
                ini.with_name_key_namespace(keys, |ini| constructor(Some(ini)))
            }),
            None => (keys, constructor(None)),
        };
        if receiving {
            self.runtime_name_keys = Some(keys);
        }
        let mut data = result?;
        // Validation preserves exact tag strings in ModuleInfo. Numeric tag
        // identity is admitted only by the receiving runtime's key owner;
        // detached validation must never allocate from the ambient registry.
        let _ = module_tag;
        let tag_key = self
            .runtime_name_keys
            .as_mut()
            .map_or(crate::common::name_key_generator::NAMEKEY_INVALID, |keys| {
                keys.name_to_key(module_tag)
            });
        data.set_module_tag_name_key(tag_key);
        let data: Arc<dyn ModuleData> = Arc::from(data);
        self.module_data_list.push(data.clone());
        Ok(data)
    }

    /// Create module data from INI
    pub fn new_module_data_from_ini(
        &mut self,
        ini: Option<&mut INI>,
        name: &str,
        module_type: ModuleType,
        module_tag: &str,
    ) -> Result<Arc<dyn ModuleData>, String> {
        if self.runtime_name_keys.is_some() {
            return self.new_module_data_from_ini_strict(ini, name, module_type, module_tag);
        }
        if name.is_empty() {
            return Err("Module name cannot be empty".to_string());
        }

        let mut module_data = if let Some(template) = self.find_module_template(name, module_type) {
            template.create_module_data(ini)
        } else if let Some(constructors) = self.module_overrides[module_type as usize].get(name) {
            // Authored module data is parsed before its descriptor is recorded.
            // Draw/ClientUpdate have no builtin descriptor seed. Use the actual
            // constructors already owned by this factory, without adding a
            // template/descriptor or changing their registration order.
            (constructors.create_data_proc)(ini)
        } else {
            return Err(format!("Module template '{}' not found", name));
        };

        // Set the module tag name key
        let module_tag_key = self.string_to_name_key(module_tag);
        module_data.set_module_tag_name_key(module_tag_key);

        let arc_data: Arc<dyn ModuleData> = Arc::from(module_data);
        self.module_data_list.push(arc_data.clone());

        Ok(arc_data)
    }

    /// True when a registered `create_data_proc` exists (GameLogic override or builtin).
    pub fn has_module_data_proc(&self, name: &str, module_type: ModuleType) -> bool {
        self.find_module_template(name, module_type)
            .and_then(|template| template.create_data_proc)
            .is_some()
            || self.module_overrides[module_type as usize].contains_key(name)
    }

    /// Fail-closed module-data create: `None` when the template or data_proc is missing.
    pub fn try_new_module_data_from_ini(
        &mut self,
        ini: Option<&mut INI>,
        name: &str,
        module_type: ModuleType,
        module_tag: &str,
    ) -> Option<Arc<dyn ModuleData>> {
        if !self.has_module_data_proc(name, module_type) {
            return None;
        }
        self.new_module_data_from_ini(ini, name, module_type, module_tag)
            .ok()
    }

    /// Query immutable builtin masks without constructing a module catalog.
    /// Names and module types match an independent factory's builtin seed.
    pub(crate) fn find_builtin_module_interface_mask(
        name: &str,
        module_type: ModuleType,
    ) -> ModuleInterfaceType {
        if name.is_empty() || module_type != ModuleType::Behavior {
            return ModuleInterfaceType::NONE;
        }

        BUILTIN_BEHAVIOR_DESCRIPTORS
            .iter()
            .filter(|(builtin_name, _)| *builtin_name == name)
            .fold(ModuleInterfaceType::NONE, |mask, (_, interfaces)| {
                mask_or(mask, *interfaces)
            })
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
        self.descriptor_catalog[module_type as usize].get(name)
    }

    /// Returns all registered descriptors for the requested module type.
    pub fn descriptors_for_type(&self, module_type: ModuleType) -> Vec<&TemplateModuleDescriptor> {
        self.descriptor_order[module_type as usize]
            .iter()
            .filter_map(|name| self.descriptor_catalog[module_type as usize].get(name))
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
            .filter_map(|name| self.module_template_map[module_type as usize].get(name))
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
        let (proc, data_proc) = self.module_overrides[module_type as usize]
            .get(name)
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
        self.module_template_map[module_type as usize].insert(name.to_owned(), template);
    }

    /// Register constructors on this factory, rebinding an existing template in place.
    /// Subsequent template registration keeps this override, including after `init`.
    pub fn register_override(
        &mut self,
        name: &str,
        module_type: ModuleType,
        create_proc: NewModuleProc,
        create_data_proc: NewModuleDataProc,
    ) -> Result<(), String> {
        if name.is_empty() {
            return Err("module override name cannot be empty".to_owned());
        }
        self.module_overrides[module_type as usize].insert(
            name.to_owned(),
            ModuleOverride {
                create_proc,
                create_data_proc,
            },
        );
        if let Some(template) = self.module_template_map[module_type as usize].get_mut(name) {
            template.create_proc = Some(create_proc);
            template.create_data_proc = Some(create_data_proc);
        }
        Ok(())
    }

    /// Admit a built-in constructor only when this owner has no explicit choice.
    /// Repeated compatibility startup preserves overrides installed by its caller.
    pub fn register_override_if_absent(
        &mut self,
        name: &str,
        module_type: ModuleType,
        create_proc: NewModuleProc,
        create_data_proc: NewModuleDataProc,
    ) -> Result<(), String> {
        if self.module_overrides[module_type as usize].contains_key(name) {
            return Ok(());
        }
        self.register_override(name, module_type, create_proc, create_data_proc)
    }

    /// Find a module template by name and type
    fn find_module_template(&self, name: &str, module_type: ModuleType) -> Option<&ModuleTemplate> {
        self.module_template_map[module_type as usize].get(name)
    }

    /// Convert string to name key (simplified implementation)
    fn string_to_name_key(&mut self, s: &str) -> NameKeyType {
        match self.runtime_name_keys.as_mut() {
            Some(keys) => keys.name_to_key(s),
            None => NameKeyGenerator::name_to_key(s),
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
        // C++ ModuleFactory.cpp:681-687 — every registered ModuleData, not a unique-Arc subset.
        for module_data in &self.module_data_list {
            module_data.crc(xfer)?;
        }
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ ModuleFactory.cpp:690-702
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("ModuleFactory::xfer version failed: {}", e))?;

        // Module data is shared with templates via Arc. C++ still xfers every
        // entry; save/load is single-threaded, so exclusive mutation is valid.
        for module_data in &self.module_data_list {
            // TODO(unsafe-review): `Arc::as_ptr` + `&mut` aliases module data
            // that live modules and templates also own via `Arc` clones; the
            // exclusivity of this `&mut` comes only from the single-threaded
            // save/load convention, not from anything the type system can
            // check (under stacked borrows, later reads through another
            // `Arc` clone would be UB). Prefer `Arc::make_mut` or a `&dyn`
            // read-only xfer path.
            let data = unsafe { &mut *(Arc::as_ptr(module_data) as *mut dyn ModuleData) };
            data.xfer(xfer)?;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ ModuleFactory.cpp:705-707 is empty: no ModuleData callbacks.
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

/// Initialize the retained compatibility factory in place. Like C++ init(),
/// this registers on the current owner; it does not replace its catalog/data.
pub fn init_module_factory() -> Result<(), String> {
    let mut guard = get_module_factory().map_err(|_| "Failed to lock module factory")?;
    guard
        .get_or_insert_with(ModuleFactory::new)
        .init()
        .map_err(|e| format!("Failed to initialize module factory: {:?}", e))
}

/// Compatibility for old startup callers. Overrides now rebind templates at
/// registration, so there is no deferred table to import at this boundary.
pub fn apply_module_overrides_to_existing_templates() -> Result<(), String> {
    let _guard = get_module_factory().map_err(|_| "Failed to lock module factory")?;
    Ok(())
}

/// Shutdown the global module factory
pub fn shutdown_module_factory() {
    if let Ok(mut factory_guard) = get_module_factory() {
        *factory_guard = None;
    }
}

#[cfg(test)]
#[path = "builtin_module_mask_query_tests.rs"]
mod builtin_module_mask_query_tests;

#[cfg(test)]
#[path = "module_factory_ownership_tests.rs"]
mod ownership_tests;

#[cfg(test)]
#[path = "module_mask_validation_tests.rs"]
mod mask_validation_tests;

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
        fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
            let mut version: u8 = 0;
            xfer.xfer_version(&mut version, 1)
                .map_err(|e| e.to_string())?;
            Ok(())
        }

        fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
            let mut version: u8 = 0;
            xfer.xfer_version(&mut version, 1)
                .map_err(|e| e.to_string())?;
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
        fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
            let mut version: u8 = 0;
            xfer.xfer_version(&mut version, 1)
                .map_err(|e| e.to_string())?;
            Ok(())
        }

        fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
            let mut version: u8 = 0;
            xfer.xfer_version(&mut version, 1)
                .map_err(|e| e.to_string())?;
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
        let mut factory = ModuleFactory::new();
        factory
            .register_override(
                "StubModule",
                ModuleType::Behavior,
                override_new_module,
                override_new_module_data,
            )
            .expect("failed to register override");

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
        let mut factory = ModuleFactory::new();

        let mut set = TemplateModuleDescriptorSet::default();
        set.behavior.push(make_descriptor(
            "FactoryOnlyBehavior",
            "TagBehavior",
            ModuleInterfaceType::BODY,
        ));
        set.draw.push(make_descriptor(
            "W3DModelDraw",
            "TagDraw",
            ModuleInterfaceType::DRAW,
        ));

        factory.register_descriptor_set(&set);

        let behavior_name = AsciiString::from("FactoryOnlyBehavior");
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
        let mut factory = ModuleFactory::new();

        let mut set = TemplateModuleDescriptorSet::default();
        set.behavior.push(make_descriptor(
            "FactoryMergeBehavior",
            "PrimaryTag",
            ModuleInterfaceType::CONTAIN,
        ));
        set.behavior.push(make_descriptor(
            "FactoryMergeBehavior",
            "SecondaryTag",
            ModuleInterfaceType::UPDATE,
        ));

        factory.register_descriptor_set(&set);

        let name = AsciiString::from("FactoryMergeBehavior");
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
    fn descriptor_registration_survives_repeated_owner_initialization() {
        let mut factory = ModuleFactory::new();
        let mut set = TemplateModuleDescriptorSet::default();
        set.behavior.push(make_descriptor(
            "AutoHealBehavior",
            "TagBehavior",
            ModuleInterfaceType::BODY,
        ));
        factory.register_descriptor_set(&set);
        factory.init().unwrap();
        factory.init().unwrap();
        assert!(
            factory
                .descriptor_for(ModuleType::Behavior, "AutoHealBehavior")
                .is_some()
        );
    }

    #[test]
    fn descriptors_preserve_registration_order() {
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
    fn owned_descriptors_merge_interface_flags() {
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

        let mut factory = ModuleFactory::new();
        factory.register_descriptor_set(&set);

        let name = AsciiString::from("MergedBehavior");
        let descriptor = factory
            .descriptor_for(ModuleType::Behavior, &name)
            .expect("descriptor should exist");
        assert_eq!(
            descriptor.interface_mask,
            ModuleInterfaceType::BODY | ModuleInterfaceType::UPDATE
        );

        let matching = factory
            .descriptors_for_type(ModuleType::Behavior)
            .into_iter()
            .filter(|descriptor| descriptor.name.as_str() == "MergedBehavior")
            .count();
        assert_eq!(
            matching, 1,
            "descriptor should not be duplicated among builtins"
        );
    }
}
