//! Active Body Module - Objects with health that can die and be affected by damage
//!
//! This module provides the main implementation for active bodies that have health,
//! can take damage, heal, and manage various body states in a thread-safe manner.

use super::body_module::{
    ArmorSetType, BodyDamageType, BodyError, BodyModule, BodyModuleData, BodyModuleInterface,
    BodyResult, DamageInfo, DamageInfoInput, DamageType, MaxHealthChangeType, ObjectId,
    VeterancyLevel,
};
use crate::ai::CommandSourceType;
use crate::common::types::ThingTemplate;
use crate::common::{
    AsciiString, DefaultThingTemplate, KindOf, ObjectStatusTypes, PlayerMaskType, INVALID_ID,
};
use crate::damage::{is_subdual_damage, DamageInfoOutput, DeathType};
use crate::helpers::{game_client_random_value, TheParticleSystemManager, TheThingFactory};
use crate::object::armor::{ensure_default_templates_loaded, Armor, ArmorTemplate, TheArmorStore};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::system::game_logic::current_frame;
use game_engine::common::bit_flags::{create_armor_set_flags, ArmorSetBitFlags, ArmorSetFlags};
use game_engine::common::game_common::convert_duration_from_msecs_to_frames;
use game_engine::common::global_data;
use game_engine::common::ini::ini_damage_fx::{
    get_damage_fx_store, get_damage_fx_store_mut, init_global_damage_fx_store, DamageFX,
    DamageType as IniDamageType, Object as DamageFxObjectTrait,
};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use std::sync::{Arc, RwLock};

/// Yellow damage threshold percentage (when fear sounds play)
#[allow(dead_code)]
const YELLOW_DAMAGE_PERCENT: f32 = 0.25;

/// Configuration data specific to active bodies
#[derive(Debug, Clone)]
pub struct ActiveBodyModuleData {
    /// Base body module data
    pub base: BodyModuleData,
    /// Maximum health this body can have
    pub max_health: f32,
    /// Initial health value
    pub initial_health: f32,
    /// Maximum subdual damage that can accumulate
    pub subdual_damage_cap: f32,
    /// How often subdual damage heals (in frames)
    pub subdual_damage_heal_rate: u32,
    /// How much subdual damage heals each time
    pub subdual_damage_heal_amount: f32,
    /// Default armor template name to apply when the body initializes
    pub default_armor_template: Option<AsciiString>,
}

impl Default for ActiveBodyModuleData {
    fn default() -> Self {
        Self {
            base: BodyModuleData::default(),
            max_health: 0.0,
            initial_health: 0.0,
            subdual_damage_cap: 0.0,
            subdual_damage_heal_rate: 0,
            subdual_damage_heal_amount: 0.0,
            default_armor_template: None,
        }
    }
}

fn parse_max_health(
    _ini: &mut INI,
    data: &mut ActiveBodyModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.max_health = INI::parse_real(token)?;
    Ok(())
}

fn parse_initial_health(
    _ini: &mut INI,
    data: &mut ActiveBodyModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.initial_health = INI::parse_real(token)?;
    Ok(())
}

fn parse_subdual_damage_cap(
    _ini: &mut INI,
    data: &mut ActiveBodyModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.subdual_damage_cap = INI::parse_real(token)?;
    Ok(())
}

fn parse_subdual_damage_heal_rate(
    _ini: &mut INI,
    data: &mut ActiveBodyModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.subdual_damage_heal_rate = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_subdual_damage_heal_amount(
    _ini: &mut INI,
    data: &mut ActiveBodyModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.subdual_damage_heal_amount = INI::parse_real(token)?;
    Ok(())
}

const ACTIVE_BODY_FIELDS: &[FieldParse<ActiveBodyModuleData>] = &[
    FieldParse {
        token: "MaxHealth",
        parse: parse_max_health,
    },
    FieldParse {
        token: "InitialHealth",
        parse: parse_initial_health,
    },
    FieldParse {
        token: "SubdualDamageCap",
        parse: parse_subdual_damage_cap,
    },
    FieldParse {
        token: "SubdualDamageHealRate",
        parse: parse_subdual_damage_heal_rate,
    },
    FieldParse {
        token: "SubdualDamageHealAmount",
        parse: parse_subdual_damage_heal_amount,
    },
];

impl ActiveBodyModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, ACTIVE_BODY_FIELDS)
    }
}

crate::impl_legacy_module_data_via_base!(ActiveBodyModuleData, base);

impl Snapshotable for ActiveBodyModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)?;
        xfer.xfer_real(&mut self.max_health)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.initial_health)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.subdual_damage_cap)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.subdual_damage_heal_rate)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.subdual_damage_heal_amount)
            .map_err(|e| e.to_string())?;

        let mut has_default = self.default_armor_template.is_some();
        xfer.xfer_bool(&mut has_default)
            .map_err(|e| e.to_string())?;
        let mut name = self
            .default_armor_template
            .clone()
            .unwrap_or_else(AsciiString::new)
            .to_string();
        xfer.xfer_ascii_string(&mut name)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.default_armor_template = if has_default && !name.is_empty() {
                Some(AsciiString::from(name.as_str()))
            } else {
                None
            };
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

fn body_damage_type_to_u32(value: BodyDamageType) -> u32 {
    match value {
        BodyDamageType::Pristine => 0,
        BodyDamageType::Damaged => 1,
        BodyDamageType::ReallyDamaged => 2,
        BodyDamageType::Rubble => 3,
    }
}

fn body_damage_type_from_u32(value: u32) -> BodyDamageType {
    match value {
        1 => BodyDamageType::Damaged,
        2 => BodyDamageType::ReallyDamaged,
        3 => BodyDamageType::Rubble,
        _ => BodyDamageType::Pristine,
    }
}

fn armor_set_flags_to_u32(flags: &ArmorSetBitFlags) -> u32 {
    let mut bits = 0u32;
    for index in 0..ArmorSetFlags::BIT_NAMES.len() {
        if flags.test(index) {
            bits |= 1u32 << index;
        }
    }
    bits
}

fn armor_set_flags_from_u32(bits: u32) -> ArmorSetBitFlags {
    let mut flags = create_armor_set_flags();
    for index in 0..ArmorSetFlags::BIT_NAMES.len() {
        flags.set(index, (bits & (1u32 << index)) != 0);
    }
    flags
}

fn xfer_damage_info_input(xfer: &mut dyn Xfer, input: &mut DamageInfoInput) -> Result<(), String> {
    const CURRENT_VERSION: XferVersion = 3;
    let mut version = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)
        .map_err(|e| e.to_string())?;

    xfer.xfer_unsigned_int(&mut input.source_id)
        .map_err(|e| e.to_string())?;

    let mut player_mask_bits = input.source_player_mask.bits();
    xfer.xfer_unsigned_int(&mut player_mask_bits)
        .map_err(|e| e.to_string())?;
    input.source_player_mask = PlayerMaskType::from_bits_truncate(player_mask_bits);

    let mut damage_type = input.damage_type as u32;
    xfer.xfer_unsigned_int(&mut damage_type)
        .map_err(|e| e.to_string())?;
    input.damage_type = DamageType::from_u32(damage_type);

    if version >= 2 {
        let mut damage_fx_override = input.damage_fx_override as u32;
        xfer.xfer_unsigned_int(&mut damage_fx_override)
            .map_err(|e| e.to_string())?;
        input.damage_fx_override = DamageType::from_u32(damage_fx_override);
    }

    let mut death_type = input.death_type as u32;
    xfer.xfer_unsigned_int(&mut death_type)
        .map_err(|e| e.to_string())?;
    input.death_type = DeathType::from_u32(death_type);

    xfer.xfer_real(&mut input.amount)
        .map_err(|e| e.to_string())?;

    if CURRENT_VERSION >= 2 {
        xfer.xfer_bool(&mut input.kill).map_err(|e| e.to_string())?;
    }

    let mut status_type = input.damage_status_type as u32;
    xfer.xfer_unsigned_int(&mut status_type)
        .map_err(|e| e.to_string())?;
    input.damage_status_type = ObjectStatusTypes::from_u32(status_type);

    xfer.xfer_real(&mut input.shock_wave_vector.x)
        .map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut input.shock_wave_vector.y)
        .map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut input.shock_wave_vector.z)
        .map_err(|e| e.to_string())?;

    xfer.xfer_real(&mut input.shock_wave_amount)
        .map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut input.shock_wave_radius)
        .map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut input.shock_wave_taper_off)
        .map_err(|e| e.to_string())?;

    if version >= 3 {
        let mut template_name = input
            .source_template
            .as_ref()
            .map(|template| template.get_name().as_str().to_string())
            .unwrap_or_default();
        xfer.xfer_ascii_string(&mut template_name)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            input.source_template = TheThingFactory::find_template(&template_name);
        }
    }

    Ok(())
}

fn xfer_damage_info_output(
    xfer: &mut dyn Xfer,
    output: &mut DamageInfoOutput,
) -> Result<(), String> {
    const CURRENT_VERSION: XferVersion = 2;
    let mut version = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)
        .map_err(|e| e.to_string())?;

    xfer.xfer_real(&mut output.actual_damage_dealt)
        .map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut output.actual_damage_clipped)
        .map_err(|e| e.to_string())?;
    xfer.xfer_bool(&mut output.no_effect)
        .map_err(|e| e.to_string())?;

    if version >= 2 {
        xfer.xfer_bool(&mut output.killed_target)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut output.experience_awarded)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn xfer_damage_info(xfer: &mut dyn Xfer, info: &mut DamageInfo) -> Result<(), String> {
    const CURRENT_VERSION: XferVersion = 1;
    let mut version = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)
        .map_err(|e| e.to_string())?;

    xfer_damage_info_input(xfer, &mut info.input)?;
    xfer_damage_info_output(xfer, &mut info.output)?;
    info.sync_from_input();
    Ok(())
}

fn record_body_particle_system(state: &mut ActiveBodyState, particle_system_id: u32) {
    let node = BodyParticleSystem {
        particle_system_id,
        next: state.particle_systems.take(),
    };
    state.particle_systems = Some(Box::new(node));
}

/// Body particle system for managing visual effects
#[derive(Debug)]
struct BodyParticleSystem {
    particle_system_id: u32,
    next: Option<Box<BodyParticleSystem>>,
}

#[derive(Debug, Clone)]
struct DamageFxObjectSnapshot {
    id: u32,
    name: String,
    veterancy_level: usize,
}

impl DamageFxObjectSnapshot {
    fn from_object(object: &Object) -> Self {
        Self {
            id: object.get_id(),
            name: object.get_name().as_str().to_string(),
            veterancy_level: object.get_veterancy_level() as usize,
        }
    }
}

impl DamageFxObjectTrait for DamageFxObjectSnapshot {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_veterancy_level(&self) -> usize {
        self.veterancy_level
    }
}

fn to_ini_damage_type(damage_type: DamageType) -> IniDamageType {
    match damage_type {
        DamageType::Explosion => IniDamageType::Explosion,
        DamageType::Crush => IniDamageType::Crush,
        DamageType::ArmorPiercing => IniDamageType::ArmorPiercing,
        DamageType::SmallArms => IniDamageType::SmallArms,
        DamageType::Gattling => IniDamageType::Gattling,
        DamageType::Radiation => IniDamageType::Radiation,
        DamageType::Flame => IniDamageType::Flame,
        DamageType::Laser => IniDamageType::Laser,
        DamageType::Sniper => IniDamageType::Sniper,
        DamageType::Poison => IniDamageType::Poison,
        DamageType::Healing => IniDamageType::Healing,
        DamageType::Unresistable => IniDamageType::Unresistable,
        DamageType::Water => IniDamageType::Water,
        DamageType::Deploy => IniDamageType::Deploy,
        DamageType::Surrender => IniDamageType::Surrender,
        DamageType::Hack => IniDamageType::Hack,
        DamageType::KillPilot => IniDamageType::KillPilot,
        DamageType::Penalty => IniDamageType::Penalty,
        DamageType::Falling => IniDamageType::Falling,
        DamageType::Melee => IniDamageType::Melee,
        DamageType::Disarm => IniDamageType::Disarm,
        DamageType::HazardCleanup => IniDamageType::HazardCleanup,
        DamageType::ParticleBeam => IniDamageType::ParticleBeam,
        DamageType::Toppling => IniDamageType::Toppling,
        DamageType::InfantryMissile => IniDamageType::InfantryMissile,
        DamageType::AuroraBomb => IniDamageType::AuroraBomb,
        DamageType::LandMine => IniDamageType::LandMine,
        DamageType::JetMissiles => IniDamageType::JetMissiles,
        DamageType::StealthJetMissiles => IniDamageType::StealthJetMissiles,
        DamageType::MolotovCocktail => IniDamageType::MolotovCocktail,
        DamageType::ComancheVulcan => IniDamageType::ComancheVulcan,
        DamageType::SubdualMissile => IniDamageType::SubdualMissile,
        DamageType::SubdualVehicle => IniDamageType::SubdualVehicle,
        DamageType::SubdualBuilding => IniDamageType::SubdualBuilding,
        DamageType::SubdualUnresistable => IniDamageType::SubdualUnresistable,
        DamageType::Microwave => IniDamageType::Microwave,
        DamageType::KillGarrisoned => IniDamageType::KillGarrisoned,
        DamageType::Status => IniDamageType::Status,
        DamageType::DamageNumTypes => IniDamageType::Explosion,
    }
}

fn snapshot_object_for_damage_fx(object_id: ObjectId) -> Option<DamageFxObjectSnapshot> {
    if object_id == INVALID_ID {
        return None;
    }

    let object = OBJECT_REGISTRY.get_object(object_id)?;
    let guard = object.read().ok()?;
    Some(DamageFxObjectSnapshot::from_object(&guard))
}

/// Thread-safe state for active body
#[derive(Debug)]
struct ActiveBodyState {
    /// Current health of the object
    current_health: f32,
    /// Previous health value before current change
    previous_health: f32,
    /// Maximum health this object can have
    max_health: f32,
    /// Starting health for this object
    initial_health: f32,
    /// Current subdual damage (starts at 0, goes up)
    current_subdual_damage: f32,
    /// Current damage state
    current_damage_state: BodyDamageType,
    /// Next time damage FX can be played
    next_damage_fx_time: u32,
    /// Last damage FX type played
    last_damage_fx_done: DamageType,
    /// Store last damage info received
    last_damage_info: Option<DamageInfo>,
    /// Frame of last damage dealt
    last_damage_timestamp: u32,
    /// Frame of last healing dealt
    last_healing_timestamp: u32,
    /// Front crushed state
    front_crushed: bool,
    /// Back crushed state
    back_crushed: bool,
    /// Whether last damage attacker info was cleared
    last_damage_cleared: bool,
    /// Is this object indestructible?
    indestructible: bool,
    /// Current armor set flags
    armor_set_flags: ArmorSetBitFlags,
    /// Cached mask from the last successful armor resolution
    resolved_armor_flags: ArmorSetBitFlags,
    /// Indicates the armor cache must be rebuilt before use
    armor_flags_dirty: bool,
    /// Cached damage FX name resolved from the active armor set
    current_damage_fx_name: Option<AsciiString>,
    /// Particle systems attached to this body
    particle_systems: Option<Box<BodyParticleSystem>>,
}

impl Default for ActiveBodyState {
    fn default() -> Self {
        Self {
            current_health: 0.0,
            previous_health: 0.0,
            max_health: 0.0,
            initial_health: 0.0,
            current_subdual_damage: 0.0,
            current_damage_state: BodyDamageType::Pristine,
            next_damage_fx_time: 0,
            last_damage_fx_done: DamageType::Unresistable,
            last_damage_info: None,
            last_damage_timestamp: u32::MAX, // So we don't think we just got damaged on first frame
            last_healing_timestamp: u32::MAX, // So we don't think we just got healed on first frame
            front_crushed: false,
            back_crushed: false,
            last_damage_cleared: false,
            indestructible: false,
            armor_set_flags: create_armor_set_flags(),
            resolved_armor_flags: create_armor_set_flags(),
            armor_flags_dirty: true,
            current_damage_fx_name: None,
            particle_systems: None,
        }
    }
}

/// Active body implementation
pub struct ActiveBody {
    /// Base body module
    base: BodyModule,
    /// Module-specific configuration
    module_data: Arc<ActiveBodyModuleData>,
    /// Thread-safe mutable state
    state: Arc<RwLock<ActiveBodyState>>,
    /// Damage scalar for defensive bonuses/penalties
    damage_scalar: Arc<RwLock<f32>>,
    /// Current armor applied to this body
    armor: Arc<RwLock<Armor>>,
    /// Name of the currently applied armor template (if any)
    armor_template_name: Arc<RwLock<Option<AsciiString>>>,
    /// Engine thing template backing this body (for armor lookups)
    engine_template: Arc<RwLock<Option<Arc<DefaultThingTemplate>>>>,
    /// Owning object ID (legacy handle lookup)
    owner_id: ObjectId,
    /// Whether to treat damage-state thresholds as structure semantics.
    ///
    /// Some tests construct bodies without an owning Object registered; this flag allows
    /// StructureBody to request structure-style damage-state transitions without relying on
    /// registry lookups.
    treat_as_structure: bool,
}

impl ActiveBody {
    /// Create a new active body

    /// Create a new active body with a known owner ID.
    pub fn new_with_owner(module_data: ActiveBodyModuleData, owner_id: ObjectId) -> Self {
        ensure_default_templates_loaded();
        let module_data = Arc::new(module_data);
        let base = BodyModule::new(module_data.base.clone());
        let state = {
            let mut initial_state = ActiveBodyState::default();
            initial_state.current_health = module_data.initial_health;
            initial_state.previous_health = module_data.initial_health;
            initial_state.max_health = module_data.max_health;
            initial_state.initial_health = module_data.initial_health;
            Arc::new(RwLock::new(initial_state))
        };

        let mut body = Self {
            base,
            module_data: Arc::clone(&module_data),
            state,
            damage_scalar: Arc::new(RwLock::new(1.0)),
            armor: Arc::new(RwLock::new(Armor::default())),
            armor_template_name: Arc::new(RwLock::new(None)),
            engine_template: Arc::new(RwLock::new(None)),
            owner_id,
            treat_as_structure: false,
        };

        // Set correct initial damage state
        body.set_correct_damage_state().unwrap_or_default();

        if let Some(default_name) = body.module_data.default_armor_template.clone() {
            if let Err(err) = body.set_armor_by_name(default_name.clone()) {
                log::warn!(
                    "Failed to apply default armor template {}: {}",
                    default_name.as_str(),
                    err
                );
            }
        }

        body
    }

    /// Create a new active body without an owner handle (legacy/tests).
    /// Owner-dependent features (pilot kill, garrison slay, status effects)
    /// will be no-ops when the ID is `INVALID_ID`.
    pub fn new(module_data: ActiveBodyModuleData) -> Self {
        Self::new_with_owner(module_data, INVALID_ID)
    }

    /// Provide the engine ThingTemplate backing this body for parity lookups.
    pub fn set_engine_template(&self, template: Arc<DefaultThingTemplate>) {
        if let Ok(mut slot) = self.engine_template.write() {
            *slot = Some(template);
        }
        if let Ok(mut state) = self.state.write() {
            state.armor_flags_dirty = true;
        }
    }

    /// Clear the cached engine template handle (used during deletion).
    pub fn clear_engine_template(&self) {
        if let Ok(mut slot) = self.engine_template.write() {
            *slot = None;
        }
        if let Ok(mut state) = self.state.write() {
            state.armor_flags_dirty = true;
        }
    }

    fn is_structure_for_damage_state(&self) -> bool {
        if self.treat_as_structure {
            return true;
        }

        self.engine_template
            .read()
            .ok()
            .and_then(|slot| slot.clone())
            .map(|template| template.is_kind_of(crate::common::KindOf::Structure))
            .unwrap_or(false)
    }

    pub fn set_treat_as_structure(&mut self, value: bool) {
        self.treat_as_structure = value;
        let _ = self.set_correct_damage_state();
    }

    /// Calculate damage state based on health ratio and global thresholds.
    ///
    /// C++ ActiveBody::calcDamageState uses the same threshold flow for units and
    /// structures; structure-specific behavior is handled later (for rubble side-effects).
    fn calc_damage_state(
        health: f32,
        max_health: f32,
        _is_structure: bool,
        damaged_thresh: f32,
        really_damaged_thresh: f32,
    ) -> BodyDamageType {
        if max_health <= 0.0 {
            return BodyDamageType::Pristine;
        }

        let ratio = health / max_health;

        if ratio > damaged_thresh {
            BodyDamageType::Pristine
        } else if ratio > really_damaged_thresh {
            BodyDamageType::Damaged
        } else if ratio > 0.0 {
            BodyDamageType::ReallyDamaged
        } else {
            BodyDamageType::Rubble
        }
    }

    /// Set the correct damage state based on current health
    fn set_correct_damage_state(&mut self) -> BodyResult<()> {
        let is_structure = self.is_structure_for_damage_state();
        let (damaged_thresh, really_damaged_thresh) = match global_data::read_safe() {
            Ok(global) => (
                global.unit_damaged_thresh,
                global.unit_really_damaged_thresh,
            ),
            Err(_) => (0.5, 0.25),
        };
        let should_apply_structure_rubble_effects = if let Ok(mut state) = self.state.write() {
            let new_state = Self::calc_damage_state(
                state.current_health,
                state.max_health,
                is_structure,
                damaged_thresh,
                really_damaged_thresh,
            );
            state.current_damage_state = new_state;
            new_state == BodyDamageType::Rubble && is_structure
        } else {
            return Err(BodyError::OperationNotSupported);
        };

        // Handle special case for structures becoming rubble.
        //
        // C++ ActiveBody::setCorrectDamageState does all three:
        // 1) set rubble geometry height
        // 2) refresh pathfind map entry (remove/add)
        // 3) force NO_COLLISIONS status
        if should_apply_structure_rubble_effects {
            // Avoid deadlocks: this can run while the owner object lock is already held.
            if let Some(owner) = self.get_owner() {
                let mut object_id = INVALID_ID;
                if let Ok(mut obj) = owner.try_write() {
                    object_id = obj.get_id();
                    let rubble_height = obj
                        .get_template()
                        .structure_rubble_height()
                        .unwrap_or_else(|| {
                            global_data::read_safe()
                                .map(|g| g.default_structure_rubble_height as u8)
                                .unwrap_or(0)
                        });
                    obj.set_geometry_info_z(rubble_height as f32);
                    obj.set_status(crate::common::ObjectStatusMaskType::NO_COLLISIONS, true);
                }

                if object_id != INVALID_ID {
                    if let Ok(ai_guard) = crate::ai::THE_AI.read() {
                        if let Some(pathfinder) = ai_guard.pathfinder() {
                            if let Ok(mut pf_guard) = pathfinder.write() {
                                pf_guard.remove_object_from_map(object_id, &[]);
                                pf_guard.add_object_to_map(object_id, &[], false);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate armor and damage FX against the active template.
    fn validate_armor_and_damage_fx(&self) -> BodyResult<()> {
        let engine_template = self
            .engine_template
            .read()
            .map_err(|_| BodyError::ArmorValidationFailed)?
            .clone();

        let (flags, dirty) = {
            let state = self
                .state
                .read()
                .map_err(|_| BodyError::ArmorValidationFailed)?;
            (state.armor_set_flags.clone(), state.armor_flags_dirty)
        };

        let needs_template = {
            let guard = self
                .armor
                .read()
                .map_err(|_| BodyError::ArmorValidationFailed)?;
            guard.template().is_none()
        };

        if !dirty && !needs_template {
            return Ok(());
        }

        let mut desired_armor_name: Option<AsciiString> = None;
        let mut damage_fx_name: Option<AsciiString> = None;

        if let Some(template) = engine_template {
            if let Some(set) = template.find_armor_template_set(&flags) {
                desired_armor_name = set
                    .armor_template_name()
                    .map(|s| AsciiString::from(s.as_str()));
                damage_fx_name = set.damage_fx_name().map(|s| AsciiString::from(s.as_str()));
            }
        }

        if desired_armor_name.is_none() {
            desired_armor_name = self.module_data.default_armor_template.clone();
        }

        self.apply_named_armor(desired_armor_name.clone())?;

        if let Some(ref fx_name) = damage_fx_name {
            let missing = get_damage_fx_store()
                .map(|store| store.find_damage_fx(fx_name.as_str()).is_none())
                .unwrap_or(true);
            if missing {
                log::trace!(
                    "Missing damage FX '{}' referenced by armor template",
                    fx_name.as_str()
                );
            }
        }

        {
            let mut state = self
                .state
                .write()
                .map_err(|_| BodyError::ArmorValidationFailed)?;
            state.resolved_armor_flags = flags.clone();
            state.armor_flags_dirty = false;
            state.current_damage_fx_name = damage_fx_name;
        }

        Ok(())
    }

    /// Replace the current armor with the template referenced by name.
    pub fn set_armor_by_name(&mut self, name: AsciiString) -> BodyResult<()> {
        self.apply_named_armor(Some(name))?;
        if let Ok(mut state) = self.state.write() {
            state.resolved_armor_flags = state.armor_set_flags.clone();
            state.armor_flags_dirty = false;
            state.current_damage_fx_name = None;
        }
        Ok(())
    }

    /// Replace the current armor with an explicit template reference.
    pub fn set_armor_template(&mut self, template: Arc<ArmorTemplate>) -> BodyResult<()> {
        self.apply_armor_template(template, None)?;
        if let Ok(mut state) = self.state.write() {
            state.resolved_armor_flags = state.armor_set_flags.clone();
            state.armor_flags_dirty = false;
            state.current_damage_fx_name = None;
        }
        if let Ok(mut stored) = self.armor_template_name.write() {
            *stored = None;
        }
        Ok(())
    }

    fn apply_armor_template(
        &self,
        template: Arc<ArmorTemplate>,
        name: Option<AsciiString>,
    ) -> BodyResult<()> {
        {
            let mut armor = self
                .armor
                .write()
                .map_err(|_| BodyError::ArmorValidationFailed)?;
            *armor = Armor::from_template(template);
        }
        if let Some(name) = name {
            if let Ok(mut stored) = self.armor_template_name.write() {
                *stored = Some(name);
            }
        }
        Ok(())
    }

    fn apply_named_armor(&self, name: Option<AsciiString>) -> BodyResult<()> {
        if let Some(ref armor_name) = name {
            let template = TheArmorStore::find_template(armor_name)
                .ok_or_else(|| BodyError::ArmorTemplateNotFound(armor_name.clone()))?;
            self.apply_armor_template(template, Some(armor_name.clone()))
        } else {
            {
                let mut armor = self
                    .armor
                    .write()
                    .map_err(|_| BodyError::ArmorValidationFailed)?;
                armor.clear();
            }
            if let Ok(mut stored) = self.armor_template_name.write() {
                *stored = None;
            }
            Ok(())
        }
    }

    fn adjust_damage_by_armor(&self, damage_type: DamageType, amount: f32) -> f32 {
        if amount <= 0.0 {
            return amount;
        }
        match self.armor.read() {
            Ok(armor) => armor.adjust_damage(damage_type, amount),
            Err(_) => amount,
        }
    }

    /// Retrieve the name of the currently applied armor template, if any.
    pub fn current_armor_template_name(&self) -> Option<AsciiString> {
        self.armor_template_name
            .read()
            .ok()
            .and_then(|name| name.clone())
    }

    /// Retrieve the active damage FX name, if any.
    pub fn current_damage_fx_name(&self) -> Option<AsciiString> {
        self.state
            .read()
            .ok()
            .and_then(|state| state.current_damage_fx_name.clone())
    }

    /// Perform damage FX using the resolved template, respecting throttling.
    fn do_damage_fx(&mut self, damage_info: &DamageInfo) -> BodyResult<()> {
        let dealt = damage_info.output.actual_damage_dealt;
        if dealt <= 0.0 {
            return Ok(());
        }

        // C++ ActiveBody::doDamageFX applies visual override when requested.
        let mut damage_type_to_use = damage_info.input.damage_type;
        if damage_info.input.damage_fx_override != DamageType::Unresistable {
            damage_type_to_use = damage_info.input.damage_fx_override;
        }

        let current_time = current_frame();

        let (fx_name, next_allowed, last_damage_fx_done) = {
            let state = self
                .state
                .read()
                .map_err(|_| BodyError::ArmorValidationFailed)?;
            (
                state.current_damage_fx_name.clone(),
                state.next_damage_fx_time,
                state.last_damage_fx_done,
            )
        };

        let fx_name = match fx_name {
            Some(name) => name,
            None => return Ok(()),
        };

        // C++ throttle only suppresses repeated effects of the same damage type.
        if damage_type_to_use == last_damage_fx_done && current_time < next_allowed {
            return Ok(());
        }

        let source_snapshot = snapshot_object_for_damage_fx(damage_info.input.source_id);
        let victim_snapshot = self.get_owner().and_then(|owner| {
            owner
                .read()
                .ok()
                .map(|guard| DamageFxObjectSnapshot::from_object(&guard))
        });
        let source_obj = source_snapshot
            .as_ref()
            .map(|obj| obj as &dyn DamageFxObjectTrait);
        let victim_obj = victim_snapshot
            .as_ref()
            .map(|obj| obj as &dyn DamageFxObjectTrait);
        let damage_type = to_ini_damage_type(damage_type_to_use);

        let throttle = {
            if let Some(store) = get_damage_fx_store() {
                if let Some(fx) = store.find_damage_fx(fx_name.as_str()) {
                    let throttle = fx.get_damage_fx_throttle_time(damage_type, source_obj);
                    fx.do_damage_fx(damage_type, dealt, source_obj, victim_obj);
                    throttle
                } else {
                    log::trace!(
                        "Missing damage FX '{}' referenced by armor template",
                        fx_name.as_str()
                    );
                    0
                }
            } else {
                log::trace!(
                    "Damage FX store is not initialized while looking up '{}'",
                    fx_name.as_str()
                );
                0
            }
        };

        let mut state = self
            .state
            .write()
            .map_err(|_| BodyError::ArmorValidationFailed)?;
        state.last_damage_fx_done = damage_type_to_use;
        state.next_damage_fx_time = current_time.saturating_add(throttle);

        Ok(())
    }

    /// Resolve an owning object handle if still alive.
    fn get_owner(&self) -> Option<Arc<RwLock<Object>>> {
        OBJECT_REGISTRY.get_object(self.owner_id)
    }

    fn for_each_damage_module_mut(
        &self,
        mut f: impl FnMut(&mut dyn crate::modules::DamageModuleInterface),
    ) {
        let Some(owner) = self.get_owner() else {
            return;
        };
        let behaviors = match owner.try_read() {
            Ok(owner_guard) => owner_guard.get_behavior_modules(),
            Err(_) => return,
        };

        for behavior in behaviors {
            if let Ok(mut behavior_guard) = behavior.lock() {
                if let Some(damage_module) = behavior_guard.get_damage() {
                    f(damage_module);
                }
            }
        }
    }

    fn with_contain_module_mut(
        &self,
        mut f: impl FnMut(&mut dyn crate::modules::ContainModuleInterface),
    ) {
        let Some(owner) = self.get_owner() else {
            return;
        };
        let contain = match owner.try_read() {
            Ok(owner_guard) => owner_guard.get_contain(),
            Err(_) => return,
        };
        if let Some(contain) = contain {
            if let Ok(mut contain_guard) = contain.lock() {
                f(&mut *contain_guard);
            }
        }
    }

    fn notify_damage_modules_on_damage(&self, damage_info: &mut DamageInfo) {
        self.for_each_damage_module_mut(|damage_module| {
            if let Err(err) = damage_module.on_damage(damage_info) {
                log::trace!("ActiveBody damage callback failed: {err}");
            }
        });
    }

    fn notify_damage_modules_on_healing(&self, damage_info: &mut DamageInfo) {
        self.for_each_damage_module_mut(|damage_module| {
            if let Err(err) = damage_module.on_healing(damage_info) {
                log::trace!("ActiveBody healing callback failed: {err}");
            }
        });
    }

    fn notify_damage_modules_on_state_change(
        &self,
        damage_info: &DamageInfo,
        old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) {
        self.for_each_damage_module_mut(|damage_module| {
            if let Err(err) =
                damage_module.on_body_damage_state_change(damage_info, old_state, new_state)
            {
                log::trace!("ActiveBody body state callback failed: {err}");
            }
        });
        self.with_contain_module_mut(|contain_module| {
            if let Err(err) =
                contain_module.on_body_damage_state_change(damage_info, old_state, new_state)
            {
                log::trace!("ActiveBody contain body state callback failed: {err}");
            }
        });
    }

    /// Resolve an owning object handle for external callers.
    pub fn owner_handle(&self) -> Option<Arc<RwLock<Object>>> {
        OBJECT_REGISTRY.get_object(self.owner_id)
    }

    /// Internal method to add subdual damage
    fn internal_add_subdual_damage(&mut self, delta: f32) -> BodyResult<()> {
        if let Ok(mut state) = self.state.write() {
            state.current_subdual_damage += delta;
            state.current_subdual_damage = state
                .current_subdual_damage
                .min(self.module_data.subdual_damage_cap);
            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }

    /// Delete all particle systems
    fn delete_all_particle_systems(&mut self) -> BodyResult<()> {
        if let Some(ps_manager) = TheParticleSystemManager::get() {
            if let Ok(state) = self.state.read() {
                let mut cursor = state.particle_systems.as_ref();
                while let Some(system) = cursor {
                    ps_manager.destroy_particle_system(system.particle_system_id);
                    cursor = system.next.as_ref();
                }
            } else {
                return Err(BodyError::OperationNotSupported);
            }
        }

        if let Ok(mut state) = self.state.write() {
            state.particle_systems = None;
            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }

    /// Create particle systems for visual effects
    fn create_particle_systems(
        &mut self,
        bone_base_name: &str,
        system_template: &str,
        max_systems: i32,
    ) -> BodyResult<()> {
        if system_template.is_empty() || max_systems <= 0 {
            return Ok(());
        }

        let Some(owner) = self.get_owner() else {
            return Ok(());
        };
        let Ok(owner_guard) = owner.read() else {
            return Ok(());
        };
        let bone_positions =
            owner_guard.get_multi_logical_bone_position(bone_base_name, max_systems as usize);
        drop(owner_guard);

        let num_bones = bone_positions.len();
        if num_bones == 0 {
            return Ok(());
        }
        let target_count = usize::min(max_systems as usize, num_bones);

        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return Ok(());
        };

        let mut used_bone_indices = vec![false; num_bones];
        let mut spawned_ids = Vec::with_capacity(target_count);

        for i in 0..target_count {
            let slot_hi = (target_count - i - 1) as i32;
            let pick = game_client_random_value(0, slot_hi) as usize;

            let mut selected_index = None;
            let mut free_count = 0usize;
            for (idx, used) in used_bone_indices.iter().enumerate() {
                if *used {
                    continue;
                }
                if free_count == pick {
                    selected_index = Some(idx);
                    break;
                }
                free_count += 1;
            }

            let Some(bone_index) = selected_index else {
                continue;
            };
            used_bone_indices[bone_index] = true;

            let Some(system_id) = ps_manager.create_particle_system(Some(system_template)) else {
                continue;
            };

            ps_manager.set_particle_system_position(system_id, &bone_positions[bone_index]);
            ps_manager.attach_particle_system_to_object(system_id, self.owner_id);

            spawned_ids.push(system_id);
        }

        if let Ok(mut state) = self.state.write() {
            for system_id in spawned_ids {
                record_body_particle_system(&mut state, system_id);
            }
            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }

    /// Check if this body can be subdued
    pub fn can_be_subdued(&self) -> bool {
        self.module_data.subdual_damage_cap > 0.0
    }

    /// Check if this body is currently subdued
    pub fn is_subdued(&self) -> bool {
        if let Ok(state) = self.state.read() {
            state.max_health <= state.current_subdual_damage
        } else {
            false
        }
    }

    /// Handle subdual state change
    pub fn on_subdual_change(&mut self, is_now_subdued: bool) -> BodyResult<()> {
        if let Some(owner) = self.get_owner() {
            if let Ok(mut obj) = owner.write() {
                if !obj.is_kind_of(crate::common::KindOf::Projectile) {
                    if is_now_subdued {
                        obj.set_disabled(crate::common::DisabledType::DisabledSubdued);
                        if let Some(contain) = obj.get_contain() {
                            if let Ok(mut contain_guard) = contain.lock() {
                                let _ = contain_guard
                                    .order_all_passengers_to_idle(CommandSourceType::FromAi);
                            }
                        }
                    } else {
                        obj.clear_disabled(crate::common::DisabledType::DisabledSubdued);
                        if obj.is_kind_of(crate::common::KindOf::FSInternetCenter) {
                            if let Some(contain) = obj.get_contain() {
                                if let Ok(mut contain_guard) = contain.lock() {
                                    let _ = contain_guard.order_all_passengers_to_hack_internet(
                                        CommandSourceType::FromAi,
                                    );
                                }
                            }
                        }
                    }
                } else if is_now_subdued {
                    for behavior in obj.get_behavior_modules() {
                        if let Ok(mut behavior_guard) = behavior.lock() {
                            if let Some(projectile) =
                                behavior_guard.get_projectile_update_interface()
                            {
                                projectile.projectile_now_jammed();
                            }
                        }
                    }
                }
            }
        }
        if let Ok(mut state) = self.state.write() {
            state.last_damage_cleared = false;
        }
        Ok(())
    }
}

impl BodyModuleInterface for ActiveBody {
    fn attempt_damage(&mut self, damage_info: &mut DamageInfo) -> BodyResult<()> {
        self.validate_armor_and_damage_fx()?;

        // Initialize output values
        damage_info.output.actual_damage_dealt = 0.0;
        damage_info.output.actual_damage_clipped = 0.0;

        // Check if indestructible
        {
            let state = self
                .state
                .read()
                .map_err(|_| BodyError::OperationNotSupported)?;
            if state.indestructible {
                return Ok(());
            }
        }

        if let Some(owner) = self.get_owner() {
            match owner.try_read() {
                Ok(owner_guard) => {
                    if owner_guard.is_effectively_dead() {
                        return Ok(());
                    }
                }
                Err(std::sync::TryLockError::WouldBlock) => {}
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err(BodyError::OperationNotSupported);
                }
            }
        } else if let Ok(state) = self.state.read() {
            if state.current_health <= 0.0 {
                return Ok(());
            }
        }

        // Store source template if damager exists
        if damage_info.input.source_id != INVALID_ID {
            if let Some(damager) = OBJECT_REGISTRY.get_object(damage_info.input.source_id) {
                if let Ok(damager_guard) = damager.read() {
                    damage_info.input.source_template = Some(damager_guard.get_template().clone());
                }
            }
        }

        let mut already_handled = false;
        let mut allow_modifier = true;
        let mut amount =
            self.adjust_damage_by_armor(damage_info.input.damage_type, damage_info.input.amount);

        // Handle special damage types
        match damage_info.input.damage_type {
            DamageType::Healing => {
                if !damage_info.input.kill {
                    return self.attempt_healing(damage_info);
                }
                return Ok(());
            }
            DamageType::KillPilot => {
                // Parity: if vehicle, kill or eject one contained occupant; ignore hull damage.
                if let Some(owner) = self.get_owner() {
                    if let Ok(mut obj) = owner.write() {
                        if obj.is_kind_of(crate::common::KindOf::Vehicle) {
                            if let Some(contain) = obj.get_contain() {
                                if let Ok(mut cont) = contain.lock() {
                                    if let Some(&victim_id) = cont.get_contained_objects().first() {
                                        if let Some(victim) = OBJECT_REGISTRY.get_object(victim_id)
                                        {
                                            if let Ok(mut v) = victim.write() {
                                                if let Some(damager) = OBJECT_REGISTRY
                                                    .get_object(damage_info.input.source_id)
                                                {
                                                    if let Ok(mut dam) = damager.write() {
                                                        dam.score_the_kill(&v);
                                                    }
                                                }
                                                v.kill(None, None);
                                            }
                                        }
                                        let _ = cont.release_object(victim_id);
                                    }
                                }
                            }
                            // Mark unmanned and neutralize like C++ path.
                            obj.set_disabled_unmanned();
                            obj.deselect_all();
                            obj.ai_idle();
                            obj.set_team_to_neutral();
                        }
                    }
                }
                already_handled = true;
                allow_modifier = false;
            }
            DamageType::KillGarrisoned => {
                // C++ parity: only garrisonable, non-immune containers are affected.
                if let Some(owner) = self.get_owner() {
                    if let Ok(obj) = owner.write() {
                        if let Some(contain) = obj.get_contain() {
                            if let Ok(mut cont) = contain.lock() {
                                if cont.get_contained_count() > 0
                                    && cont.is_garrisonable()
                                    && !cont.is_immune_to_clear_building_attacks()
                                {
                                    let kills_to_make = damage_info.input.amount.floor() as i32;
                                    let ids: Vec<ObjectId> = cont.get_contained_objects().to_vec();
                                    let mut kills_made = 0;
                                    for id in ids {
                                        if kills_made >= kills_to_make {
                                            break;
                                        }
                                        if let Some(v) = OBJECT_REGISTRY.get_object(id) {
                                            if let Ok(mut victim) = v.write() {
                                                if !victim.is_effectively_dead() {
                                                    if let Some(damager) = OBJECT_REGISTRY
                                                        .get_object(damage_info.input.source_id)
                                                    {
                                                        if let Ok(mut dam) = damager.write() {
                                                            dam.score_the_kill(&victim);
                                                        }
                                                    }
                                                    victim.kill(None, None);
                                                    kills_made += 1;
                                                    let _ = cont.release_object(id);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                already_handled = true;
                allow_modifier = false;
            }
            DamageType::Status => {
                // Apply status effect duration.
                if let Some(owner) = self.get_owner() {
                    if let Ok(mut obj) = owner.write() {
                        let duration_frames = convert_duration_from_msecs_to_frames(amount).ceil();
                        obj.do_status_damage(damage_info.input.damage_status_type, duration_frames);
                    }
                }
                already_handled = true;
                allow_modifier = false;
            }
            _ => {}
        }

        // Handle subdual damage
        if is_subdual_damage(damage_info.input.damage_type) {
            if !self.can_be_subdued() {
                return Ok(());
            }

            let was_subdued = self.is_subdued();
            self.internal_add_subdual_damage(amount)?;
            let now_subdued = self.is_subdued();
            already_handled = true;
            allow_modifier = false;

            if was_subdued != now_subdued {
                self.on_subdual_change(now_subdued)?;
            }

            if let Some(owner) = self.get_owner() {
                if let Ok(mut obj) = owner.write() {
                    obj.notify_subdual_damage(amount);
                }
            }
        }

        // Apply damage scalar if allowed
        if allow_modifier && damage_info.input.damage_type != DamageType::Unresistable {
            let scalar = self.get_damage_scalar();
            amount *= scalar;
        }

        // Apply damage if amount is positive or kill is requested
        if amount > 0.0 || damage_info.input.kill {
            let old_state = self.get_damage_state();

            // If kill is requested, damage all remaining health
            if damage_info.input.kill {
                amount = self.get_health();
            }

            // Do the actual damage
            if !already_handled {
                self.internal_change_health(-amount)?;
            }

            let (previous_health, current_health) = {
                let state = self
                    .state
                    .read()
                    .map_err(|_| BodyError::OperationNotSupported)?;
                damage_info.output.actual_damage_dealt = amount;
                damage_info.output.actual_damage_clipped =
                    state.previous_health - state.current_health;
                (state.previous_health, state.current_health)
            };

            // Store damage info
            let frame_now = current_frame();
            let mut should_overwrite_last_damage = true;
            let mut existing_source_id = INVALID_ID;
            if let Ok(state) = self.state.read() {
                let is_same_or_next_frame = state.last_damage_timestamp == frame_now
                    || state.last_damage_timestamp == frame_now.saturating_sub(1);
                if is_same_or_next_frame {
                    should_overwrite_last_damage = false;
                    existing_source_id = state
                        .last_damage_info
                        .as_ref()
                        .map(|info| info.input.source_id)
                        .unwrap_or(INVALID_ID);
                }
            }

            if !should_overwrite_last_damage {
                let src2_is_preferred = OBJECT_REGISTRY
                    .get_object(damage_info.input.source_id)
                    .and_then(|obj| {
                        obj.read().ok().map(|guard| {
                            guard.is_kind_of(crate::common::KindOf::Vehicle)
                                || guard.is_kind_of(crate::common::KindOf::Infantry)
                                || guard.is_kind_of(crate::common::KindOf::Structure)
                        })
                    });
                let src1_exists = OBJECT_REGISTRY
                    .get_object(existing_source_id)
                    .and_then(|obj| obj.read().ok().map(|_| ()))
                    .is_some();

                if let Some(src2_is_preferred) = src2_is_preferred {
                    if !src1_exists || src2_is_preferred {
                        should_overwrite_last_damage = true;
                    }
                }
            }

            if should_overwrite_last_damage {
                if let Ok(mut state) = self.state.write() {
                    state.last_damage_info = Some(damage_info.clone());
                    state.last_damage_cleared = false;
                    state.last_damage_timestamp = frame_now;
                }
            }

            if current_health < previous_health {
                self.notify_damage_modules_on_damage(damage_info);
            }

            // Handle damage state change
            let new_state = self.get_damage_state();
            if new_state != old_state {
                self.notify_damage_modules_on_state_change(damage_info, old_state, new_state);
            }

            // Check if we died
            if current_health <= 0.0 && previous_health > 0.0 {
                // C++ parity: credit the killer on health-crossing inside ActiveBody.
                if damage_info.input.source_id != INVALID_ID {
                    if let (Some(damager), Some(owner)) = (
                        OBJECT_REGISTRY.get_object(damage_info.input.source_id),
                        self.get_owner(),
                    ) {
                        if let (Ok(mut damager_guard), Ok(owner_guard)) =
                            (damager.write(), owner.read())
                        {
                            damager_guard.score_the_kill(&owner_guard);
                        }
                    }
                }

                // Object has died - death will be handled by the Object after this returns
                // The ActiveBody just tracks that death occurred
                log::debug!("ActiveBody: Health reached 0, death should be processed by Object");
            }
        }

        // Do damage FX
        self.do_damage_fx(damage_info)?;

        Ok(())
    }

    fn attempt_healing(&mut self, healing_info: &mut DamageInfo) -> BodyResult<()> {
        self.validate_armor_and_damage_fx()?;

        // Initialize output values
        healing_info.output.actual_damage_dealt = 0.0;
        healing_info.output.actual_damage_clipped = 0.0;

        if healing_info.input.damage_type != DamageType::Healing {
            return self.attempt_damage(healing_info);
        }

        // C++ parity: allow bridge/bridge-tower healing even when effectively dead.
        if let Some(owner) = self.get_owner() {
            if let Ok(owner_guard) = owner.read() {
                let is_bridge = owner_guard.is_kind_of(KindOf::Bridge)
                    || owner_guard.is_kind_of(KindOf::BridgeTower);
                if owner_guard.is_effectively_dead() && !is_bridge {
                    return Ok(());
                }
            }
        } else if let Ok(state) = self.state.read() {
            if state.current_health <= 0.0 {
                return Ok(());
            }
        }

        let amount =
            self.adjust_damage_by_armor(healing_info.input.damage_type, healing_info.input.amount);

        if amount > 0.0 {
            let old_state = self.get_damage_state();

            // Do the healing
            self.internal_change_health(amount)?;

            let (previous_health, current_health) = {
                let state = self
                    .state
                    .read()
                    .map_err(|_| BodyError::OperationNotSupported)?;
                healing_info.output.actual_damage_dealt = amount;
                healing_info.output.actual_damage_clipped =
                    state.previous_health - state.current_health;
                (state.previous_health, state.current_health)
            };

            let frame_now = current_frame();

            if let Ok(mut state) = self.state.write() {
                state.last_damage_info = Some(healing_info.clone());
                state.last_damage_cleared = false;
                state.last_damage_timestamp = frame_now;
                state.last_healing_timestamp = frame_now;
            }

            if current_health > previous_health {
                self.notify_damage_modules_on_healing(healing_info);
            }

            // Handle damage state change
            let new_state = self.get_damage_state();
            if new_state != old_state {
                self.notify_damage_modules_on_state_change(healing_info, old_state, new_state);
            }
        }

        // Do damage FX
        self.do_damage_fx(healing_info)?;

        Ok(())
    }

    fn estimate_damage(&self, damage_info: &DamageInfoInput) -> BodyResult<f32> {
        self.validate_armor_and_damage_fx()?;

        // Handle subdual damage
        if is_subdual_damage(damage_info.damage_type) && !self.can_be_subdued() {
            return Ok(0.0);
        }

        // Handle special damage types
        match damage_info.damage_type {
            DamageType::KillGarrisoned => {
                if let Some(owner) = self.get_owner() {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(contain) = owner_guard.get_contain() {
                            if let Ok(contain_guard) = contain.lock() {
                                if contain_guard.get_contained_count() > 0
                                    && contain_guard.is_garrisonable()
                                    && !contain_guard.is_immune_to_clear_building_attacks()
                                {
                                    return Ok(1.0);
                                }
                            }
                        }
                    }
                }
                return Ok(0.0);
            }
            DamageType::Sniper => {
                // Sniper damage is a normal damage type for units; special casing for
                // under-construction structures is handled at a higher level.
            }
            _ => {}
        }

        // C++ parity: estimate damage after armor adjustments only.
        let amount = self.adjust_damage_by_armor(damage_info.damage_type, damage_info.amount);

        Ok(amount)
    }

    fn get_health(&self) -> f32 {
        self.state
            .read()
            .map(|state| state.current_health)
            .unwrap_or(0.0)
    }

    fn get_max_health(&self) -> f32 {
        self.state
            .read()
            .map(|state| state.max_health)
            .unwrap_or(0.0)
    }

    fn get_initial_health(&self) -> f32 {
        self.state
            .read()
            .map(|state| state.initial_health)
            .unwrap_or(0.0)
    }

    fn get_previous_health(&self) -> f32 {
        self.state
            .read()
            .map(|state| state.previous_health)
            .unwrap_or(0.0)
    }

    fn get_subdual_damage_heal_rate(&self) -> u32 {
        self.module_data.subdual_damage_heal_rate
    }

    fn get_subdual_damage_heal_amount(&self) -> f32 {
        self.module_data.subdual_damage_heal_amount
    }

    fn has_any_subdual_damage(&self) -> bool {
        self.state
            .read()
            .map(|state| state.current_subdual_damage > 0.0)
            .unwrap_or(false)
    }

    fn get_current_subdual_damage_amount(&self) -> f32 {
        self.state
            .read()
            .map(|state| state.current_subdual_damage)
            .unwrap_or(0.0)
    }

    fn get_damage_state(&self) -> BodyDamageType {
        self.state
            .read()
            .map(|state| state.current_damage_state)
            .unwrap_or(BodyDamageType::Pristine)
    }

    fn set_damage_state(&mut self, new_state: BodyDamageType) -> BodyResult<()> {
        let (damaged_thresh, really_damaged_thresh) = match global_data::read_safe() {
            Ok(global) => (
                global.unit_damaged_thresh,
                global.unit_really_damaged_thresh,
            ),
            Err(_) => (0.5, 0.25),
        };

        // Calculate the health ratio for the desired state
        let ratio = match new_state {
            BodyDamageType::Pristine => 1.0,
            BodyDamageType::Damaged => damaged_thresh,
            BodyDamageType::ReallyDamaged => really_damaged_thresh,
            BodyDamageType::Rubble => 0.0,
        };

        let max_health = self.get_max_health();
        let desired_health = (max_health * ratio - 1.0).max(0.0); // -1 because < not <= in calc
        let current_health = self.get_health();
        let delta = desired_health - current_health;

        self.internal_change_health(delta)?;
        self.set_correct_damage_state()?;

        Ok(())
    }

    fn set_aflame(&mut self, _setting: bool) -> BodyResult<()> {
        // This would set/clear the aflame object status
        // and update particle systems accordingly
        self.update_body_particle_systems()
    }

    fn on_veterancy_level_changed(
        &mut self,
        old_level: VeterancyLevel,
        new_level: VeterancyLevel,
        provide_feedback: bool,
    ) -> BodyResult<()> {
        if old_level == new_level {
            return Ok(());
        }

        // Handle promotion (increase in level)
        if old_level < new_level {
            if provide_feedback {
                // Play appropriate promotion sound
            }

            // Mark UI dirty if object is selected
        }

        let (old_bonus, new_bonus) = if let Some(data) = game_engine::common::ini::get_global_data()
        {
            let guard = data.read();
            let old = guard
                .health_bonus
                .get(old_level as usize)
                .copied()
                .unwrap_or(1.0);
            let new = guard
                .health_bonus
                .get(new_level as usize)
                .copied()
                .unwrap_or(1.0);
            (old, new)
        } else {
            (1.0, 1.0)
        };
        let multiplier = new_bonus / old_bonus;

        // Change max health preserving ratio
        let new_max_health = self.get_max_health() * multiplier;
        self.set_max_health(new_max_health, MaxHealthChangeType::PreserveRatio)?;

        // Set appropriate armor flags based on level
        match new_level {
            VeterancyLevel::Regular => {
                self.clear_armor_set_flag(ArmorSetType::Veteran)?;
                self.clear_armor_set_flag(ArmorSetType::Elite)?;
                self.clear_armor_set_flag(ArmorSetType::Hero)?;
            }
            VeterancyLevel::Veteran => {
                self.set_armor_set_flag(ArmorSetType::Veteran)?;
                self.clear_armor_set_flag(ArmorSetType::Elite)?;
                self.clear_armor_set_flag(ArmorSetType::Hero)?;
            }
            VeterancyLevel::Elite => {
                self.clear_armor_set_flag(ArmorSetType::Veteran)?;
                self.set_armor_set_flag(ArmorSetType::Elite)?;
                self.clear_armor_set_flag(ArmorSetType::Hero)?;
            }
            VeterancyLevel::Heroic => {
                self.clear_armor_set_flag(ArmorSetType::Veteran)?;
                self.clear_armor_set_flag(ArmorSetType::Elite)?;
                self.set_armor_set_flag(ArmorSetType::Hero)?;
            }
        }

        Ok(())
    }

    fn set_armor_set_flag(&mut self, armor_type: ArmorSetType) -> BodyResult<()> {
        if let Ok(mut state) = self.state.write() {
            let index = armor_type as usize;
            if !state.armor_set_flags.test(index) {
                state.armor_set_flags.set(index, true);
                state.armor_flags_dirty = true;
            }
            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }

    fn clear_armor_set_flag(&mut self, armor_type: ArmorSetType) -> BodyResult<()> {
        if let Ok(mut state) = self.state.write() {
            let index = armor_type as usize;
            if state.armor_set_flags.test(index) {
                state.armor_set_flags.set(index, false);
                state.armor_flags_dirty = true;
            }
            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }

    fn test_armor_set_flag(&self, armor_type: ArmorSetType) -> bool {
        if let Ok(state) = self.state.read() {
            state.armor_set_flags.test(armor_type as usize)
        } else {
            false
        }
    }

    fn get_last_damage_info(&self) -> Option<DamageInfo> {
        self.state
            .read()
            .ok()
            .and_then(|state| state.last_damage_info.clone())
    }

    fn get_last_damage_timestamp(&self) -> u32 {
        self.state
            .read()
            .map(|state| state.last_damage_timestamp)
            .unwrap_or(0)
    }

    fn get_last_healing_timestamp(&self) -> u32 {
        self.state
            .read()
            .map(|state| state.last_healing_timestamp)
            .unwrap_or(0)
    }

    fn get_clearable_last_attacker(&self) -> ObjectId {
        if let Ok(state) = self.state.read() {
            if state.last_damage_cleared {
                INVALID_ID
            } else {
                state
                    .last_damage_info
                    .as_ref()
                    .map(|info| info.source_id)
                    .unwrap_or(INVALID_ID)
            }
        } else {
            INVALID_ID
        }
    }

    fn clear_last_attacker(&mut self) {
        if let Ok(mut state) = self.state.write() {
            state.last_damage_cleared = true;
        }
    }

    fn get_front_crushed(&self) -> bool {
        self.state
            .read()
            .map(|state| state.front_crushed)
            .unwrap_or(false)
    }

    fn get_back_crushed(&self) -> bool {
        self.state
            .read()
            .map(|state| state.back_crushed)
            .unwrap_or(false)
    }

    fn set_initial_health(&mut self, initial_percent: i32) -> BodyResult<()> {
        let factor = initial_percent as f32 / 100.0;
        let initial_health = self.get_initial_health();
        let new_health = factor * initial_health;
        let current_health = self.get_health();

        self.internal_change_health(new_health - current_health)
    }

    fn set_max_health(
        &mut self,
        max_health: f32,
        change_type: MaxHealthChangeType,
    ) -> BodyResult<()> {
        let prev_max_health = self.get_max_health();
        let current_health = self.get_health();

        // Update max and initial health
        if let Ok(mut state) = self.state.write() {
            state.max_health = max_health;
            state.initial_health = max_health;

            // Handle different change types
            match change_type {
                MaxHealthChangeType::PreserveRatio => {
                    // Preserve health ratio
                    let ratio = current_health / prev_max_health;
                    let new_health = max_health * ratio;
                    state.previous_health = state.current_health;
                    state.current_health = new_health;
                }
                MaxHealthChangeType::AddCurrentHealthToo => {
                    // Add the same amount to current health
                    let delta = max_health - prev_max_health;
                    state.previous_health = state.current_health;
                    state.current_health += delta;
                }
                MaxHealthChangeType::SameCurrentHealth => {
                    // Keep current health the same
                }
                MaxHealthChangeType::FullyHeal => {
                    // Set current to new max
                    state.previous_health = state.current_health;
                    state.current_health = max_health;
                }
            }

            // Clamp current health to new max
            if state.current_health > max_health {
                state.current_health = max_health;
            }
        }

        // Update damage state
        self.set_correct_damage_state()
    }

    fn set_front_crushed(&mut self, crushed: bool) -> BodyResult<()> {
        if let Ok(mut state) = self.state.write() {
            state.front_crushed = crushed;
            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }

    fn set_back_crushed(&mut self, crushed: bool) -> BodyResult<()> {
        if let Ok(mut state) = self.state.write() {
            state.back_crushed = crushed;
            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }

    fn apply_damage_scalar(&mut self, scalar: f32) -> BodyResult<()> {
        if let Ok(mut damage_scalar) = self.damage_scalar.write() {
            *damage_scalar *= scalar;
            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }

    fn get_damage_scalar(&self) -> f32 {
        self.damage_scalar
            .read()
            .map(|scalar| *scalar)
            .unwrap_or(1.0)
    }

    fn internal_change_health(&mut self, delta: f32) -> BodyResult<()> {
        let mut changed_state = false;
        let is_structure = self.is_structure_for_damage_state();
        let (damaged_thresh, really_damaged_thresh) = match global_data::read_safe() {
            Ok(global) => (
                global.unit_damaged_thresh,
                global.unit_really_damaged_thresh,
            ),
            Err(_) => (0.5, 0.25),
        };
        if let Ok(mut state) = self.state.write() {
            // Save current as previous
            state.previous_health = state.current_health;

            // Apply delta
            state.current_health += delta;

            // Clamp to valid range
            state.current_health = state.current_health.clamp(0.0, state.max_health);

            // Update damage state
            let old_state = state.current_damage_state;
            state.current_damage_state = Self::calc_damage_state(
                state.current_health,
                state.max_health,
                is_structure,
                damaged_thresh,
                really_damaged_thresh,
            );

            // Handle state change
            if state.current_damage_state != old_state {
                changed_state = true;
            }
        } else {
            return Err(BodyError::OperationNotSupported);
        }

        if changed_state {
            self.evaluate_visual_condition()?;
        }

        Ok(())
    }

    fn set_indestructible(&mut self, indestructible: bool) -> BodyResult<()> {
        if let Ok(mut state) = self.state.write() {
            state.indestructible = indestructible;

            // For bridges, would mirror this state on towers
            // This would involve looking up tower objects and setting their indestructible flag

            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }

    fn is_indestructible(&self) -> bool {
        self.state
            .read()
            .map(|state| state.indestructible)
            .unwrap_or(false)
    }

    fn evaluate_visual_condition(&mut self) -> BodyResult<()> {
        if let Some(owner) = self.get_owner() {
            if let Ok(owner_guard) = owner.try_read() {
                if let Some(drawable) = owner_guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        let max_health = self.get_max_health().max(f32::EPSILON);
                        let health_pct = self.get_health() / max_health;
                        draw_guard.update_damage_state_for_health(health_pct);
                    }
                }
            }
        }

        self.update_body_particle_systems()
    }

    fn update_body_particle_systems(&mut self) -> BodyResult<()> {
        self.delete_all_particle_systems()?;

        let aflame = if let Some(owner) = self.get_owner() {
            if let Ok(guard) = owner.try_read() {
                guard.test_status(ObjectStatusTypes::Aflame)
            } else {
                false
            }
        } else {
            false
        };
        let count_modifier = if aflame { 2i32 } else { 1i32 };

        let global = match global_data::read_safe() {
            Ok(data) => data,
            Err(_) => return Ok(()),
        };

        let fire_small_system = if aflame {
            global.auto_fire_particle_medium_system.clone()
        } else {
            global.auto_fire_particle_small_system.clone()
        };
        let fire_medium_system = if aflame {
            global.auto_fire_particle_large_system.clone()
        } else {
            global.auto_fire_particle_medium_system.clone()
        };
        let fire_large_system = global.auto_fire_particle_large_system.clone();
        let smoke_small_system = if aflame {
            global.auto_fire_particle_small_system.clone()
        } else {
            global.auto_smoke_particle_small_system.clone()
        };
        let smoke_medium_system = if aflame {
            global.auto_fire_particle_small_system.clone()
        } else {
            global.auto_smoke_particle_medium_system.clone()
        };
        let smoke_large_system = if aflame {
            global.auto_fire_particle_small_system.clone()
        } else {
            global.auto_smoke_particle_large_system.clone()
        };

        let fire_small_prefix = global.auto_fire_particle_small_prefix.clone();
        let fire_medium_prefix = global.auto_fire_particle_medium_prefix.clone();
        let fire_large_prefix = global.auto_fire_particle_large_prefix.clone();
        let smoke_small_prefix = global.auto_smoke_particle_small_prefix.clone();
        let smoke_medium_prefix = global.auto_smoke_particle_medium_prefix.clone();
        let smoke_large_prefix = global.auto_smoke_particle_large_prefix.clone();
        let aflame_prefix = global.auto_aflame_particle_prefix.clone();
        let aflame_system = global.auto_aflame_particle_system.clone();
        let fire_small_max = global.auto_fire_particle_small_max;
        let fire_medium_max = global.auto_fire_particle_medium_max;
        let fire_large_max = global.auto_fire_particle_large_max;
        let smoke_small_max = global.auto_smoke_particle_small_max;
        let smoke_medium_max = global.auto_smoke_particle_medium_max;
        let smoke_large_max = global.auto_smoke_particle_large_max;
        let aflame_max = global.auto_aflame_particle_max;
        drop(global);

        self.create_particle_systems(
            fire_small_prefix.as_str(),
            fire_small_system.as_str(),
            fire_small_max.saturating_mul(count_modifier),
        )?;
        self.create_particle_systems(
            fire_medium_prefix.as_str(),
            fire_medium_system.as_str(),
            fire_medium_max.saturating_mul(count_modifier),
        )?;
        self.create_particle_systems(
            fire_large_prefix.as_str(),
            fire_large_system.as_str(),
            fire_large_max.saturating_mul(count_modifier),
        )?;
        self.create_particle_systems(
            smoke_small_prefix.as_str(),
            smoke_small_system.as_str(),
            smoke_small_max.saturating_mul(count_modifier),
        )?;
        self.create_particle_systems(
            smoke_medium_prefix.as_str(),
            smoke_medium_system.as_str(),
            smoke_medium_max.saturating_mul(count_modifier),
        )?;
        self.create_particle_systems(
            smoke_large_prefix.as_str(),
            smoke_large_system.as_str(),
            smoke_large_max.saturating_mul(count_modifier),
        )?;

        if aflame {
            self.create_particle_systems(
                aflame_prefix.as_str(),
                aflame_system.as_str(),
                aflame_max.saturating_mul(count_modifier),
            )?;
        }

        Ok(())
    }
}

impl Snapshotable for ActiveBody {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        self.base.xfer(xfer)?;

        let mut state = self
            .state
            .write()
            .map_err(|_| "ActiveBody state lock poisoned".to_string())?;

        xfer.xfer_real(&mut state.current_health)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut state.current_subdual_damage)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut state.previous_health)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut state.max_health)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut state.initial_health)
            .map_err(|e| e.to_string())?;

        let mut damage_state = body_damage_type_to_u32(state.current_damage_state);
        xfer.xfer_unsigned_int(&mut damage_state)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            state.current_damage_state = body_damage_type_from_u32(damage_state);
        }

        xfer.xfer_unsigned_int(&mut state.next_damage_fx_time)
            .map_err(|e| e.to_string())?;

        let mut last_damage_fx_done = state.last_damage_fx_done as u32;
        xfer.xfer_unsigned_int(&mut last_damage_fx_done)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            state.last_damage_fx_done = DamageType::from_u32(last_damage_fx_done);
        }

        let mut last_damage = state.last_damage_info.clone().unwrap_or_default();
        xfer_damage_info(xfer, &mut last_damage)?;
        state.last_damage_info = Some(last_damage);

        xfer.xfer_unsigned_int(&mut state.last_damage_timestamp)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() && state.last_damage_timestamp == u32::MAX {
            state.last_damage_info = None;
        }
        xfer.xfer_unsigned_int(&mut state.last_healing_timestamp)
            .map_err(|e| e.to_string())?;

        xfer.xfer_bool(&mut state.front_crushed)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut state.back_crushed)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut state.last_damage_cleared)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut state.indestructible)
            .map_err(|e| e.to_string())?;

        let mut particle_ids: Vec<u32> = Vec::new();
        if xfer.is_writing() {
            let mut cursor = state.particle_systems.as_ref();
            while let Some(system) = cursor {
                particle_ids.push(system.particle_system_id);
                cursor = system.next.as_ref();
            }
        }

        let mut particle_count = particle_ids.len().min(u16::MAX as usize) as u16;
        xfer.xfer_unsigned_short(&mut particle_count)
            .map_err(|e| e.to_string())?;

        if xfer.is_writing() {
            for id in particle_ids.into_iter().take(particle_count as usize) {
                let mut value = id;
                xfer.xfer_unsigned_int(&mut value)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            state.particle_systems = None;
            for _ in 0..particle_count {
                let mut value = 0u32;
                xfer.xfer_unsigned_int(&mut value)
                    .map_err(|e| e.to_string())?;
                let entry = BodyParticleSystem {
                    particle_system_id: value,
                    next: state.particle_systems.take(),
                };
                state.particle_systems = Some(Box::new(entry));
            }
        }

        let mut armor_bits = armor_set_flags_to_u32(&state.armor_set_flags);
        xfer.xfer_unsigned_int(&mut armor_bits)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            state.armor_set_flags = armor_set_flags_from_u32(armor_bits);
            state.resolved_armor_flags = create_armor_set_flags();
            state.armor_flags_dirty = true;
            state.current_damage_fx_name = None;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()?;
        if let Ok(mut state) = self.state.write() {
            state.armor_flags_dirty = true;
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "engine_tests"))]
mod tests {
    use super::*;
    use game_engine::common::bit_flags::ArmorSetFlags as ArmorSetBits;
    use game_engine::thing::thing_template::{
        ArmorTemplateSet as EngineArmorTemplateSet, ThingTemplate as EngineThingTemplateDataExt,
    };
    use std::sync::Arc;

    fn create_test_active_body() -> ActiveBody {
        let mut module_data = ActiveBodyModuleData::default();
        module_data.max_health = 100.0;
        module_data.initial_health = 100.0;
        module_data.subdual_damage_cap = 50.0;
        module_data.subdual_damage_heal_rate = 30;
        module_data.subdual_damage_heal_amount = 10.0;

        ActiveBody::new(module_data)
    }

    #[test]
    fn test_active_body_creation() {
        let body = create_test_active_body();

        assert_eq!(body.get_health(), 100.0);
        assert_eq!(body.get_max_health(), 100.0);
        assert_eq!(body.get_initial_health(), 100.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);
        assert!(!body.is_indestructible());
        assert!(body.can_be_subdued());
        assert!(!body.is_subdued());
    }

    #[test]
    fn test_damage_state_calculation() {
        assert_eq!(
            ActiveBody::calc_damage_state(100.0, 100.0),
            BodyDamageType::Pristine
        );
        assert_eq!(
            ActiveBody::calc_damage_state(50.0, 100.0),
            BodyDamageType::Damaged
        );
        assert_eq!(
            ActiveBody::calc_damage_state(20.0, 100.0),
            BodyDamageType::ReallyDamaged
        );
        assert_eq!(
            ActiveBody::calc_damage_state(0.0, 100.0),
            BodyDamageType::Rubble
        );
    }

    #[test]
    fn test_health_changes() {
        let mut body = create_test_active_body();

        // Test damage
        assert!(body.internal_change_health(-25.0).is_ok());
        assert_eq!(body.get_health(), 75.0);
        assert_eq!(body.get_previous_health(), 100.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);

        // Test more damage
        assert!(body.internal_change_health(-30.0).is_ok());
        assert_eq!(body.get_health(), 45.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Damaged);

        // Test healing
        assert!(body.internal_change_health(20.0).is_ok());
        assert_eq!(body.get_health(), 65.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Damaged);
    }

    #[test]
    fn test_max_health_changes() {
        let mut body = create_test_active_body();

        // Damage to 50%
        assert!(body.internal_change_health(-50.0).is_ok());
        assert_eq!(body.get_health(), 50.0);

        // Increase max health preserving ratio
        assert!(body
            .set_max_health(200.0, MaxHealthChangeType::PreserveRatio)
            .is_ok());
        assert_eq!(body.get_max_health(), 200.0);
        assert_eq!(body.get_health(), 100.0); // Should be 50% of 200

        // Test full heal
        assert!(body
            .set_max_health(150.0, MaxHealthChangeType::FullyHeal)
            .is_ok());
        assert_eq!(body.get_max_health(), 150.0);
        assert_eq!(body.get_health(), 150.0);
    }

    #[test]
    fn test_armor_set_flags() {
        let mut body = create_test_active_body();

        // Test setting and testing flags
        assert!(!body.test_armor_set_flag(ArmorSetType::Veteran));
        assert!(body.set_armor_set_flag(ArmorSetType::Veteran).is_ok());
        assert!(body.test_armor_set_flag(ArmorSetType::Veteran));

        // Test clearing flags
        assert!(body.clear_armor_set_flag(ArmorSetType::Veteran).is_ok());
        assert!(!body.test_armor_set_flag(ArmorSetType::Veteran));
    }

    #[test]
    fn test_armor_adjustment() {
        TheArmorStore::reset();

        let mut template = ArmorTemplate::new();
        template.set_coefficient(DamageType::SmallArms, 0.5);
        let armor_name = AsciiString::from("TestArmor");
        TheArmorStore::register_template(&armor_name, template);

        let mut module_data = ActiveBodyModuleData::default();
        module_data.max_health = 100.0;
        module_data.initial_health = 100.0;
        module_data.default_armor_template = Some(armor_name.clone());

        let mut body = ActiveBody::new(module_data);
        let mut info = DamageInfo {
            input: DamageInfoInput {
                damage_type: DamageType::SmallArms,
                amount: 20.0,
                ..Default::default()
            },
            ..Default::default()
        };

        body.attempt_damage(&mut info)
            .expect("damage application failed");
        assert!(info.output.actual_damage_dealt < 20.0);
        assert_eq!(body.current_armor_template_name(), Some(armor_name.clone()));

        TheArmorStore::reset();
    }

    #[test]
    fn test_damage_scalar() {
        let mut body = create_test_active_body();

        assert_eq!(body.get_damage_scalar(), 1.0);

        assert!(body.apply_damage_scalar(1.5).is_ok());
        assert_eq!(body.get_damage_scalar(), 1.5);

        assert!(body.apply_damage_scalar(2.0).is_ok());
        assert_eq!(body.get_damage_scalar(), 3.0);
    }

    #[test]
    fn test_indestructible() {
        let mut body = create_test_active_body();

        assert!(!body.is_indestructible());

        assert!(body.set_indestructible(true).is_ok());
        assert!(body.is_indestructible());

        assert!(body.set_indestructible(false).is_ok());
        assert!(!body.is_indestructible());
    }
    #[test]
    fn resolves_armor_template_from_template_flags() {
        TheArmorStore::reset();
        init_global_damage_fx_store();
        if let Some(mut store) = get_damage_fx_store_mut() {
            store.reset();
        }

        let base_name = AsciiString::from("BaseArmor");
        let hero_name = AsciiString::from("HeroArmor");
        let hero_fx_name = AsciiString::from("HeroFX");

        let mut base_template = ArmorTemplate::new();
        base_template.set_default(1.0);
        TheArmorStore::register_template(&base_name, base_template);

        let mut hero_template = ArmorTemplate::new();
        hero_template.set_default(0.5);
        TheArmorStore::register_template(&hero_name, hero_template);

        if let Some(mut store) = get_damage_fx_store_mut() {
            store.add_damage_fx(hero_fx_name.as_str().to_string(), DamageFX::new());
        }

        let mut engine_template = EngineThingTemplateDataExt::new();
        let mut base_set = EngineArmorTemplateSet::new();
        base_set.set_armor_template_name(Some(base_name.clone()));
        engine_template.add_armor_template_set(base_set);

        let mut hero_set = EngineArmorTemplateSet::new();
        hero_set.types_mut().set(ArmorSetBits::HERO, true);
        hero_set.set_armor_template_name(Some(hero_name.clone()));
        hero_set.set_damage_fx_name(Some(hero_fx_name.clone()));
        engine_template.add_armor_template_set(hero_set);

        let template = Arc::new(engine_template);
        let mut body = create_test_active_body();
        body.set_engine_template(template);

        body.validate_armor_and_damage_fx()
            .expect("initial armor validate");
        assert_eq!(body.current_armor_template_name(), Some(base_name.clone()));
        assert!(body.current_damage_fx_name().is_none());

        body.set_armor_set_flag(ArmorSetType::Hero)
            .expect("set hero flag");
        body.validate_armor_and_damage_fx()
            .expect("hero armor validate");
        assert_eq!(body.current_armor_template_name(), Some(hero_name.clone()));
        assert_eq!(body.current_damage_fx_name(), Some(hero_fx_name.clone()));

        body.clear_armor_set_flag(ArmorSetType::Hero)
            .expect("clear hero flag");
        body.validate_armor_and_damage_fx()
            .expect("fallback armor validate");
        assert_eq!(body.current_armor_template_name(), Some(base_name.clone()));
        assert!(body.current_damage_fx_name().is_none());

        TheArmorStore::reset();
        if let Some(mut store) = get_damage_fx_store_mut() {
            store.reset();
        }
    }
}
