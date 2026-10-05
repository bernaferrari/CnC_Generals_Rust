//! Active Body Module - Objects with health that can die and be affected by damage
//!
//! This module provides the main implementation for active bodies that have health,
//! can take damage, heal, and manage various body states in a thread-safe manner.

use super::body_module::{
    ArmorSetType, BodyDamageType, BodyError, BodyModule, BodyModuleData, BodyModuleInterface,
    BodyResult, DamageInfo, DamageInfoInput, DamageType, MaxHealthChangeType, ObjectId,
    VeterancyLevel,
};
use crate::ai::{CommandSourceType, the_ai};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::types::ThingTemplate;
use crate::common::{
    AsciiString, DefaultThingTemplate, FROM_BOUNDING_SPHERE_2D, INVALID_ID, KindOf,
    ObjectStatusTypes, PlayerMaskType, Relationship,
};
use crate::damage::{DamageInfoOutput, DeathType, is_subdual_damage};
use crate::helpers::{
    TheAudio, TheParticleSystemManager, ThePartitionManager, TheThingFactory,
    game_client_random_value, get_game_logic_random_value,
};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::Object;
use crate::object::armor::{Armor, ArmorTemplate, TheArmorStore, ensure_default_templates_loaded};
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::{PlayerType, player_list};
use crate::system::game_logic::current_frame;
use game_engine::common::bit_flags::{ArmorSetBitFlags, ArmorSetFlags, create_armor_set_flags};
use game_engine::common::game_common::convert_duration_from_msecs_to_frames;
use game_engine::common::global_data;
use game_engine::common::ini::ini_damage_fx::{
    DamageFX, DamageType as IniDamageType, Object as DamageFxObjectTrait, get_damage_fx_store,
    get_damage_fx_store_mut, init_global_damage_fx_store,
};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use std::borrow::Cow;
use std::sync::{Arc, RwLock};

/// Wave 291: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

/// Yellow damage threshold percentage (when fear sounds play)
const YELLOW_DAMAGE_PERCENT: f32 = 0.25;

fn record_neutral_vehicle_sniped() {
    let Ok(list) = player_list().read() else {
        return;
    };
    let Some(neutral) = list.get_neutral_player() else {
        return;
    };
    if let Ok(mut player) = neutral.write() {
        player.get_academy_stats_mut().record_vehicle_sniped();
    }
}

fn record_cleared_garrison_for_object(victim: &Object) {
    victim.with_controlling_player_mut(|guard| {
        guard
            .get_academy_stats_mut()
            .record_cleared_garrisoned_building();
    });
}

fn play_object_template_sound(owner: &Object, mut event: crate::common::audio::AudioEventRts) {
    if event.get_event_name().is_empty() {
        return;
    }
    event.set_object_id(owner.get_id());
    if let Some(audio) = TheAudio::get() {
        audio.add_audio_event(&event);
    }
}

fn play_voice_fear(owner: &Object) {
    let mut event = owner.get_template().get_voice_fear();
    if event.get_event_name().is_empty() {
        return;
    }
    let pos = *owner.get_position();
    event.set_position(&(pos.x, pos.y, pos.z));
    if let Some(index) = owner.with_controlling_player(|guard| guard.get_player_index()) {
        event.set_player_index(index as u32);
    }
    if let Some(audio) = TheAudio::get() {
        audio.add_audio_event(&event);
    }
}

fn should_retaliate_against_aggressor(obj: &Object, damager: &Object) -> bool {
    if damager.is_airborne_target() {
        return false;
    }
    if damager.relationship_to(obj) != Relationship::Enemies {
        return false;
    }
    let max_dist = the_ai()
        .read()
        .ok()
        .map(|ai| ai.get_ai_data().max_retaliate_distance)
        .unwrap_or(210.0);
    let dist_sqr = ThePartitionManager::get_distance_squared(obj, damager, FROM_BOUNDING_SPHERE_2D);
    if dist_sqr > max_dist * max_dist {
        return false;
    }
    if obj.with_controlling_player(|g| g.get_player_type()) != Some(PlayerType::Human) {
        return false;
    }
    if obj.is_kind_of(KindOf::Drone) {
        return false;
    }
    true
}

fn should_retaliate(obj: &Object) -> bool {
    if obj.is_kind_of(KindOf::CannotRetaliate) || obj.is_kind_of(KindOf::Immobile) {
        return false;
    }
    if obj.is_kind_of(KindOf::Drone) {
        return false;
    }
    let Some(ai) = obj.get_ai() else {
        return false;
    };
    if !ai.is_idle() {
        return false;
    }
    if obj.test_status(ObjectStatusTypes::Stealthed)
        && !obj.test_status(ObjectStatusTypes::Detected)
    {
        return false;
    }
    if obj.test_status(ObjectStatusTypes::IsUsingAbility) {
        return false;
    }
    true
}

pub(crate) fn retaliate_nearby_friends(victim: &Object, damager: &Object) {
    let eligible = victim.with_controlling_player(|player_guard| {
        player_guard.is_logical_retaliation_mode_enabled()
            && player_guard.get_player_type() == PlayerType::Human
    });
    if eligible != Some(true) {
        return;
    }
    if !should_retaliate_against_aggressor(victim, damager) {
        return;
    }
    let friends_radius = the_ai()
        .read()
        .ok()
        .map(|ai| ai.get_ai_data().retaliate_friends_radius)
        .unwrap_or(120.0)
        + victim.get_geometry_info().get_bounding_circle_radius();
    let Some(partition) = ThePartitionManager::get() else {
        return;
    };
    let damager_id = damager.get_id();
    let candidates = partition.get_objects_in_range(victim.get_position(), friends_radius);
    for friend_id in candidates {
        if friend_id == victim.get_id() || friend_id == damager_id {
            continue;
        }
        let _ = OBJECT_REGISTRY.with_object(friend_id, |them| {
            if them.is_off_map() {
                return;
            }
            if them.relationship_to(victim) != Relationship::Allies {
                return;
            }
            if !should_retaliate(them) {
                return;
            }
            if them.is_kind_of(KindOf::Immobile) {
                return;
            }
            let Some(ai) = them.get_ai() else {
                return;
            };
            let can_attack = matches!(
                them.get_able_to_attack_specific_object(
                    AbleToAttackType::NewTarget,
                    damager,
                    CommandSourceType::FromAi,
                ),
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
            );
            if can_attack {
                ai.ai_guard_retaliate(
                    damager_id,
                    them.get_position(),
                    i32::MAX,
                    CommandSourceType::FromAi,
                );
            }
        });
    }
}

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
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ ActiveBodyModuleData inherits ModuleData's empty snapshot hooks.
        // Runtime health and damage belong to ActiveBody::xfer below.
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
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

    // C++ PlayerMaskType is UnsignedShort when MAX_PLAYER_COUNT is 16.
    let mut player_mask_bits = input.source_player_mask.bits() as u16;
    xfer.xfer_unsigned_short(&mut player_mask_bits)
        .map_err(|e| e.to_string())?;
    input.source_player_mask = PlayerMaskType::from_bits_truncate(player_mask_bits as u32);

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
    const CURRENT_VERSION: XferVersion = 1;
    let mut version = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)
        .map_err(|e| e.to_string())?;

    xfer.xfer_real(&mut output.actual_damage_dealt)
        .map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut output.actual_damage_clipped)
        .map_err(|e| e.to_string())?;
    xfer.xfer_bool(&mut output.no_effect)
        .map_err(|e| e.to_string())?;

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
    // Wave 291: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }

    if object_id == INVALID_ID {
        return None;
    }

    OBJECT_REGISTRY.with_object(object_id, |guard| {
        DamageFxObjectSnapshot::from_object(guard)
    })
}

/// Mutable state owned by an active body.
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
    module_data: ActiveBodyModuleData,
    /// Mutable simulation state; writes require exclusive access to this body.
    state: ActiveBodyState,
    /// Current armor. C++ `m_curArmor` is a plain member of the body the
    /// caller already owns; the outer body-module mutex is the shared handle.
    armor: Armor,
    /// Name of the currently applied armor template (if any)
    armor_template_name: Option<AsciiString>,
    /// The exact owner's immutable definition, bound during module installation.
    engine_template: Option<Arc<dyn ThingTemplate>>,
    /// Owning object ID (legacy handle lookup)
    owner_id: ObjectId,
    /// Whether to treat damage-state thresholds as structure semantics.
    ///
    /// Some tests construct bodies without an owning Object registered; this flag allows
    /// StructureBody to request structure-style damage-state transitions without relying on
    /// registry lookups.
    treat_as_structure: bool,
    /// C++ ImmortalBody::internalChangeHealth floor (ImmortalBody.cpp:34).
    /// 0 = ActiveBody (can die). 1 = ImmortalBody (never below 1 HP).
    min_health_floor: f32,
}

impl ActiveBody {
    /// Create a new active body

    /// Create a new active body with a known owner ID.
    pub fn new_with_owner(module_data: ActiveBodyModuleData, owner_id: ObjectId) -> Self {
        ensure_default_templates_loaded();
        let base = BodyModule::new(module_data.base.clone());
        let mut state = ActiveBodyState::default();
        state.current_health = module_data.initial_health;
        state.previous_health = module_data.initial_health;
        state.max_health = module_data.max_health;
        state.initial_health = module_data.initial_health;

        let mut body = Self {
            base,
            module_data,
            state,
            armor: Armor::default(),
            armor_template_name: None,
            engine_template: None,
            owner_id,
            treat_as_structure: false,
            min_health_floor: 0.0,
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
    pub fn set_engine_template(&mut self, template: Arc<DefaultThingTemplate>) {
        self.engine_template = Some(template);
        {
            let state = &mut self.state;
            state.armor_flags_dirty = true;
        }
    }

    /// C++ ActiveBody.cpp:158-163 resolves the constructing Object's authored
    /// ArmorSet before the next module constructor or onObjectCreated callback.
    /// Installation lends the exact definition; no numeric owner lookup occurs.
    pub(crate) fn bind_object_template(
        &mut self,
        template: Arc<dyn ThingTemplate>,
    ) -> BodyResult<()> {
        self.engine_template = Some(template);
        self.state.armor_flags_dirty = true;
        self.validate_armor_and_damage_fx()?;
        self.set_correct_damage_state()
    }

    /// Clear the cached engine template handle (used during deletion).
    pub fn clear_engine_template(&mut self) {
        self.engine_template = None;
        {
            let state = &mut self.state;
            state.armor_flags_dirty = true;
        }
    }

    fn is_structure_for_damage_state(&self) -> bool {
        if self.treat_as_structure {
            return true;
        }

        self.engine_template
            .as_ref()
            .map(|template| template.is_kind_of(crate::common::KindOf::Structure))
            .unwrap_or(false)
    }

    pub fn set_treat_as_structure(&mut self, value: bool) {
        self.treat_as_structure = value;
        let _ = self.set_correct_damage_state();
    }

    /// Arm the C++ ImmortalBody 1-HP floor so `attempt_damage` / `set_max_health`
    /// hit `internal_change_health` with the same clamp as the virtual override.
    pub fn set_min_health_floor(&mut self, floor: f32) {
        self.min_health_floor = floor.max(0.0);
    }

    pub fn min_health_floor(&self) -> f32 {
        self.min_health_floor
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
        // C++ tests the IEEE ratio directly, including zero maximum health.
        // In particular, 0/0 produces NaN and falls through to RUBBLE.
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
        let thresholds = global_data::read_safe().ok().map(|global| {
            (
                global.unit_damaged_thresh,
                global.unit_really_damaged_thresh,
            )
        });
        {
            let state = &mut self.state;
            // C++ calcDamageState returns BODY_PRISTINE before the divide when
            // TheGlobalData is null. Rubble pose is applied by the Object that
            // already holds this body, after the guard drops.
            let new_state = if let Some((damaged_thresh, really_damaged_thresh)) = thresholds {
                Self::calc_damage_state(
                    state.current_health,
                    state.max_health,
                    is_structure,
                    damaged_thresh,
                    really_damaged_thresh,
                )
            } else {
                BodyDamageType::Pristine
            };
            state.current_damage_state = new_state;
        };

        Ok(())
    }

    /// Validate armor and damage FX against the active template.
    ///
    /// `&mut self` because C++ `m_curArmor` is a plain member. `estimate_damage`
    /// is `&self` (trait / C++ const) and resolves a temporary armor instead.
    fn validate_armor_and_damage_fx(&mut self) -> BodyResult<()> {
        if !self.state.armor_flags_dirty && self.armor.template().is_some() {
            return Ok(());
        }

        // Retain the exact selected flags for cache publication, but only on
        // the dirty path that actually resolves a new armor choice.
        let flags = self.state.armor_set_flags.clone();
        let (desired_armor_name, damage_fx_name) = Self::resolve_armor_choice(
            self.engine_template.as_ref(),
            &flags,
            self.module_data.default_armor_template.as_ref(),
        );

        self.apply_named_armor(desired_armor_name)?;

        if let Some(fx_name) = &damage_fx_name {
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
            let state = &mut self.state;
            state.resolved_armor_flags = flags;
            state.armor_flags_dirty = false;
            state.current_damage_fx_name = damage_fx_name;
        }

        Ok(())
    }

    /// Armor name and damage-FX name C++ `validateArmorAndDamageFX` would install.
    fn resolve_armor_choice(
        engine_template: Option<&Arc<dyn ThingTemplate>>,
        flags: &ArmorSetBitFlags,
        default_name: Option<&AsciiString>,
    ) -> (Option<AsciiString>, Option<AsciiString>) {
        let mut desired_armor_name: Option<AsciiString> = None;
        let mut damage_fx_name: Option<AsciiString> = None;

        if let Some(template) = engine_template {
            if let Some(set) = template.find_armor_template_set(flags) {
                // C++ ActiveBody.cpp:249-256. A set with no armor template
                // clears m_curArmor. Do not substitute the module default.
                desired_armor_name = set
                    .armor_template_name()
                    .map(|s| AsciiString::from(s.as_str()));
                damage_fx_name = set.damage_fx_name().map(|s| AsciiString::from(s.as_str()));
                return (desired_armor_name, damage_fx_name);
            }
        }

        if desired_armor_name.is_none() {
            desired_armor_name = default_name.cloned();
        }

        (desired_armor_name, damage_fx_name)
    }

    /// Borrow the current armor when valid, or resolve the same temporary armor
    /// `validate_armor_and_damage_fx` would store without mutating the cache.
    /// Used by the immutable `estimate_damage` path.
    fn armor_resolved_for_read(&self) -> BodyResult<Cow<'_, Armor>> {
        if !self.state.armor_flags_dirty && self.armor.template().is_some() {
            return Ok(Cow::Borrowed(&self.armor));
        }
        let (name, _) = Self::resolve_armor_choice(
            self.engine_template.as_ref(),
            &self.state.armor_set_flags,
            self.module_data.default_armor_template.as_ref(),
        );
        match name {
            Some(armor_name) => {
                let template = TheArmorStore::find_template(&armor_name)
                    .ok_or(BodyError::ArmorTemplateNotFound(armor_name))?;
                Ok(Cow::Owned(Armor::from_template(template)))
            }
            None => Ok(Cow::Owned(Armor::default())),
        }
    }

    /// Replace the current armor with the template referenced by name.
    pub fn set_armor_by_name(&mut self, name: AsciiString) -> BodyResult<()> {
        self.apply_named_armor(Some(name))?;
        {
            let state = &mut self.state;
            state.resolved_armor_flags = state.armor_set_flags.clone();
            state.armor_flags_dirty = false;
            state.current_damage_fx_name = None;
        }
        Ok(())
    }

    /// Replace the current armor with an explicit template reference.
    pub fn set_armor_template(&mut self, template: Arc<ArmorTemplate>) -> BodyResult<()> {
        self.apply_armor_template(template, None)?;
        {
            let state = &mut self.state;
            state.resolved_armor_flags = state.armor_set_flags.clone();
            state.armor_flags_dirty = false;
            state.current_damage_fx_name = None;
        }
        self.armor_template_name = None;
        Ok(())
    }

    fn apply_armor_template(
        &mut self,
        template: Arc<ArmorTemplate>,
        name: Option<AsciiString>,
    ) -> BodyResult<()> {
        self.armor = Armor::from_template(template);
        if let Some(name) = name {
            self.armor_template_name = Some(name);
        }
        Ok(())
    }

    fn apply_named_armor(&mut self, name: Option<AsciiString>) -> BodyResult<()> {
        if let Some(armor_name) = &name {
            let template = TheArmorStore::find_template(armor_name)
                .ok_or_else(|| BodyError::ArmorTemplateNotFound(armor_name.clone()))?;
            self.apply_armor_template(template, Some(armor_name.clone()))
        } else {
            self.armor.clear();
            self.armor_template_name = None;
            Ok(())
        }
    }

    fn adjust_damage_by_armor(&self, damage_type: DamageType, amount: f32) -> f32 {
        // C++ ArmorTemplate::adjustDamage multiplies, then clamps < 0 to 0.
        // Returning early kept healing amounts negative and skipped that clamp.
        // Poison used to skip armor and return `amount` unchanged (a fake
        // unarmored result). The armor value is owned, so this is the real one.
        self.armor.adjust_damage(damage_type, amount)
    }

    /// Retrieve the name of the currently applied armor template, if any.
    pub fn current_armor_template_name(&self) -> Option<AsciiString> {
        self.armor_template_name.clone()
    }

    /// Retrieve the active damage FX name, if any.
    pub fn current_damage_fx_name(&self) -> Option<AsciiString> {
        self.state.current_damage_fx_name.clone()
    }

    /// Perform damage FX using the resolved template, respecting throttling.
    pub(crate) fn do_damage_fx(&mut self, damage_info: &DamageInfo) -> BodyResult<()> {
        let dealt = damage_info.output.actual_damage_dealt;
        // C++ ActiveBody.cpp:297-316 doDamageFX records last type + throttle
        // even when actualDamageDealt is 0. Do not drop bookkeeping first.

        // C++ ActiveBody::doDamageFX applies visual override when requested.
        let mut damage_type_to_use = damage_info.input.damage_type;
        if damage_info.input.damage_fx_override != DamageType::Unresistable {
            damage_type_to_use = damage_info.input.damage_fx_override;
        }

        let current_time = current_frame();

        let (fx_name, next_allowed, last_damage_fx_done) = {
            let state = &self.state;
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
                    fx.get_damage_fx_throttle_time(damage_type, source_obj)
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

        {
            let state = &mut self.state;
            state.last_damage_fx_done = damage_type_to_use;
            state.next_damage_fx_time = current_time.saturating_add(throttle);
        }

        if let Some(store) = get_damage_fx_store() {
            if let Some(fx) = store.find_damage_fx(fx_name.as_str()) {
                fx.do_damage_fx(damage_type, dealt, source_obj, victim_obj);
            }
        }

        Ok(())
    }

    /// Resolve an owning object handle if still alive.
    fn get_owner(&self) -> Option<Arc<RwLock<Object>>> {
        // Wave 291: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        OBJECT_REGISTRY.get_object(self.owner_id)
    }

    /// C++ ActiveBody::setIndestructible mirrors the flag onto KINDOF_BRIDGE towers.
    fn mirror_indestructible_to_bridge_towers(&mut self, indestructible: bool) {
        use crate::object::behavior::behavior_module::BridgeTowerType;
        let Some(owner) = self.get_owner() else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        if !owner_guard.is_kind_of(KindOf::Bridge) {
            return;
        }
        let mut tower_ids = Vec::new();
        for mut behavior in owner_guard.get_behavior_modules() {
            let Ok(mut guard) = behavior.access() else {
                continue;
            };
            if let Some(interface) = guard.get_bridge_behavior_interface() {
                for tower_type in [
                    BridgeTowerType::North,
                    BridgeTowerType::South,
                    BridgeTowerType::East,
                    BridgeTowerType::West,
                ] {
                    let id = interface.get_tower_id(tower_type);
                    if id != INVALID_ID {
                        tower_ids.push(id);
                    }
                }
                break;
            }
        }
        drop(owner_guard);
        for id in tower_ids {
            let _ = OBJECT_REGISTRY.with_object(id, |tower| {
                if let Some(body) = tower.get_body() {
                    if let Ok(mut body_guard) = body.lock() {
                        let _ = body_guard.set_indestructible(indestructible);
                    }
                }
            });
        }
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

        for mut behavior in behaviors {
            if let Ok(mut behavior_guard) = behavior.access() {
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
        // Wave 291: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        OBJECT_REGISTRY.get_object(self.owner_id)
    }

    /// Internal method to add subdual damage
    fn internal_add_subdual_damage(&mut self, delta: f32) -> BodyResult<()> {
        {
            let state = &mut self.state;
            state.current_subdual_damage += delta;
            state.current_subdual_damage = state
                .current_subdual_damage
                .min(self.module_data.subdual_damage_cap);
            Ok(())
        }
    }

    /// Delete all particle systems
    fn delete_all_particle_systems(&mut self) -> BodyResult<()> {
        if let Some(ps_manager) = TheParticleSystemManager::get() {
            {
                let state = &self.state;
                let mut cursor = state.particle_systems.as_ref();
                while let Some(system) = cursor {
                    ps_manager.destroy_particle_system(system.particle_system_id);
                    cursor = system.next.as_ref();
                }
            }
        }

        {
            let state = &mut self.state;
            state.particle_systems = None;
            Ok(())
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
        // C++ ActiveBody.cpp:963-969. MAX_BONES is 16. maxSystems only limits
        // how many systems are spawned, not how many bones are queried.
        const MAX_BONES: usize = 16;
        let bone_positions = owner_guard.get_multi_logical_bone_position(bone_base_name, MAX_BONES);
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

        {
            let state = &mut self.state;
            for system_id in spawned_ids {
                record_body_particle_system(state, system_id);
            }
            Ok(())
        }
    }

    /// Check if this body can be subdued
    pub fn can_be_subdued(&self) -> bool {
        self.module_data.subdual_damage_cap > 0.0
    }

    /// Check if this body is currently subdued
    pub fn is_subdued(&self) -> bool {
        {
            let state = &self.state;
            state.max_health <= state.current_subdual_damage
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
                    for mut behavior in obj.get_behavior_modules() {
                        if let Ok(mut behavior_guard) = behavior.access() {
                            if let Some(projectile) =
                                behavior_guard.get_projectile_update_interface()
                            {
                                projectile.projectile_now_jammed();
                                break;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl ActiveBody {
    fn attempt_damage_with_owner_context(
        &mut self,
        damage_info: &mut DamageInfo,
        context: Option<&super::body_module::BodyDamageContext>,
    ) -> BodyResult<()> {
        // C++ ActiveBody::attemptDamage always applies local HP damage and records
        // lastDamageInfo (including death type). Owner/registry lookups already
        // no-op when the dual-world registry is empty; do not skip the hull path.
        // OCL diesOnBadLand / WaveGuide use DAMAGE_WATER + DEATH_FLOODED via this
        // path instead of Object::kill().
        damage_info.sync_from_input();

        self.validate_armor_and_damage_fx()?;

        // C++ ActiveBody.cpp:329 returns before clearing output.
        {
            let state = &self.state;
            if state.indestructible {
                return Ok(());
            }
        }

        // Initialize output values
        damage_info.output.actual_damage_dealt = 0.0;
        damage_info.output.actual_damage_clipped = 0.0;

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
        } else {
            let state = &self.state;
            if state.current_health <= 0.0 {
                return Ok(());
            }
        }

        if damage_info.input.source_id != INVALID_ID {
            let mut skip_lookup = false;
            if let Some(owner) = self.get_owner() {
                match owner.try_read() {
                    Ok(guard) if guard.get_id() == damage_info.input.source_id => {
                        damage_info.input.source_template = Some(guard.get_template().clone());
                        skip_lookup = true;
                    }
                    Ok(_) => {}
                    Err(std::sync::TryLockError::WouldBlock) => {
                        skip_lookup = true;
                    }
                    Err(std::sync::TryLockError::Poisoned(_)) => {
                        skip_lookup = true;
                    }
                }
            }
            if !skip_lookup {
                if let Some(template) = OBJECT_REGISTRY
                    .with_object(damage_info.input.source_id, |damager_guard| {
                        damager_guard.get_template().clone()
                    })
                {
                    damage_info.input.source_template = Some(template);
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
                // C++ ActiveBody.cpp:365-418 DAMAGE_KILLPILOT.
                // Vehicles only. RiderChangeContain (combat bike) splits on
                // AI::isMoving: a moving bike is scored+killed outright; a
                // stationary bike evacuates then kills the rider so the bike
                // scuttles. Ordinary vehicles become DISABLED_UNMANNED and
                // transfer to the neutral team. Hull HP is never applied.
                if let Some(owner) = self.get_owner() {
                    let owner_id = owner
                        .read()
                        .ok()
                        .map(|obj| obj.get_id())
                        .unwrap_or(INVALID_ID);
                    let source_id = damage_info.input.source_id;
                    let mut kill_vehicle = false;
                    let mut kill_rider: Option<ObjectId> = None;
                    let mut evacuate = false;
                    if let Ok(mut obj) = owner.write() {
                        if obj.is_kind_of(crate::common::KindOf::Vehicle) {
                            let rider_change = obj
                                .get_contain()
                                .and_then(|contain| {
                                    contain.lock().ok().map(|g| g.is_rider_change_contain())
                                })
                                .unwrap_or(false);
                            if rider_change {
                                if obj.is_moving() {
                                    kill_vehicle = true;
                                } else {
                                    kill_rider = obj.get_contain().and_then(|contain| {
                                        contain.lock().ok().and_then(|g| {
                                            g.get_contained_objects().first().copied()
                                        })
                                    });
                                    evacuate = true;
                                }
                            } else {
                                obj.set_disabled_unmanned();
                                obj.deselect_all();
                                obj.ai_idle();
                                obj.set_team_to_neutral();
                            }
                            record_neutral_vehicle_sniped();
                        }
                    }
                    if kill_vehicle {
                        if source_id != INVALID_ID {
                            let _ = OBJECT_REGISTRY.with_object_mut(source_id, |damager| {
                                let _ = OBJECT_REGISTRY.with_object(owner_id, |victim| {
                                    damager.score_the_kill(victim);
                                });
                            });
                        }
                        let _ = OBJECT_REGISTRY.with_object_mut(owner_id, |vehicle| {
                            vehicle.kill(None, None);
                        });
                    } else {
                        if evacuate {
                            if let Ok(obj) = owner.read() {
                                if let Some(ai) = obj.get_ai() {
                                    if let Ok(mut ai_guard) = ai.lock() {
                                        let mut params = crate::ai::AiCommandParams::new(
                                            crate::ai::AiCommandType::EvacuateInstantly,
                                            CommandSourceType::FromAi,
                                        );
                                        params.int_value = 1;
                                        let _ = ai_guard.execute_command(&params);
                                    }
                                }
                            }
                        }
                        if let Some(rider_id) = kill_rider {
                            if source_id != INVALID_ID {
                                let _ = OBJECT_REGISTRY.with_object_mut(source_id, |damager| {
                                    let _ = OBJECT_REGISTRY.with_object(rider_id, |rider| {
                                        damager.score_the_kill(rider);
                                    });
                                });
                            }
                            let _ = OBJECT_REGISTRY.with_object_mut(rider_id, |rider| {
                                rider.kill(None, None);
                            });
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
                                    let ids: Vec<ObjectId> =
                                        cont.get_contained_objects().into_owned();
                                    let mut kills_made = 0;
                                    for id in ids {
                                        if kills_made >= kills_to_make {
                                            break;
                                        }
                                        let source_id = damage_info.input.source_id;
                                        if OBJECT_REGISTRY
                                            .with_object_mut(id, |victim| {
                                                if victim.is_effectively_dead() {
                                                    return false;
                                                }
                                                let _ = OBJECT_REGISTRY.with_object_mut(
                                                    source_id,
                                                    |dam| {
                                                        dam.score_the_kill(victim);
                                                    },
                                                );
                                                record_cleared_garrison_for_object(victim);
                                                victim.kill(None, None);
                                                true
                                            })
                                            .unwrap_or(false)
                                        {
                                            kills_made += 1;
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
                let _ = self.on_subdual_change(now_subdued);
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

            let (previous_health, current_health, max_health) = {
                let state = &self.state;
                damage_info.output.actual_damage_dealt = amount;
                damage_info.output.actual_damage_clipped =
                    state.previous_health - state.current_health;
                (
                    state.previous_health,
                    state.current_health,
                    state.max_health,
                )
            };

            // Store damage info
            let frame_now = current_frame();
            let mut should_overwrite_last_damage = true;
            let mut existing_source_id = INVALID_ID;
            {
                let state = &self.state;
                let is_same_or_next_frame = state.last_damage_timestamp == frame_now
                    || state.last_damage_timestamp == frame_now.wrapping_sub(1);
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
                let src2_is_preferred = if damage_info.input.source_id == self.owner_id {
                    context
                        .map(|context| context.owner_is_preferred_source)
                        .or_else(|| {
                            OBJECT_REGISTRY.with_object(damage_info.input.source_id, |guard| {
                                guard.is_kind_of(crate::common::KindOf::Vehicle)
                                    || guard.is_kind_of(crate::common::KindOf::Infantry)
                                    || guard.is_faction_structure()
                            })
                        })
                } else {
                    OBJECT_REGISTRY.with_object(damage_info.input.source_id, |guard| {
                        guard.is_kind_of(crate::common::KindOf::Vehicle)
                            || guard.is_kind_of(crate::common::KindOf::Infantry)
                            || guard.is_faction_structure()
                    })
                };
                let src1_exists = OBJECT_REGISTRY.contains(existing_source_id);

                if let Some(src2_is_preferred) = src2_is_preferred {
                    if !src1_exists || src2_is_preferred {
                        should_overwrite_last_damage = true;
                    }
                }
            }

            if should_overwrite_last_damage {
                // Keep compatibility fields (death_type / damage_type) in lockstep
                // with input so last death type (e.g. DEATH_FLOODED) is readable
                // from either DamageInfo.death_type or input.death_type.
                damage_info.sync_from_input();
                {
                    let state = &mut self.state;
                    state.last_damage_info = Some(damage_info.clone());
                    state.last_damage_cleared = false;
                    state.last_damage_timestamp = frame_now;
                }
            }

            // C++ ActiveBody.cpp:574-583 — victim player remembers who attacked.
            let last_source_id = self
                .state
                .last_damage_info
                .as_ref()
                .map(|info| info.input.source_id)
                .unwrap_or(INVALID_ID);
            if last_source_id != INVALID_ID {
                if let Some(owner) = self.get_owner() {
                    // try_read: the owner's write guard may be held by this
                    // thread while the body mutates (self-sourced damage).
                    if let Ok(owner_guard) = owner.try_read() {
                        // `.flatten()`: with_object wraps the scoped player
                        // lookup's own Option; keep only the source's index.
                        let src_index = owner_guard.with_controlling_player(|_| {
                            OBJECT_REGISTRY
                                .with_object(last_source_id, |src| {
                                    src.with_controlling_player(|g| g.get_player_index())
                                })
                                .flatten()
                        });
                        if let Some(Some(src_index)) = src_index {
                            owner_guard.with_controlling_player_mut(|player| {
                                player.set_attacked_by(src_index)
                            });
                        }
                    }
                }
            }

            if current_health < previous_health {
                self.notify_damage_modules_on_damage(damage_info);
            }

            // Handle damage state change
            let new_state = self.get_damage_state();
            if new_state != old_state {
                self.notify_damage_modules_on_state_change(damage_info, old_state, new_state);
                if let Some(owner) = self.get_owner() {
                    if let Ok(owner_guard) = owner.try_read() {
                        match new_state {
                            BodyDamageType::Damaged => {
                                play_object_template_sound(
                                    &owner_guard,
                                    owner_guard.get_template().get_sound_on_damaged(),
                                );
                            }
                            BodyDamageType::ReallyDamaged => {
                                play_object_template_sound(
                                    &owner_guard,
                                    owner_guard.get_template().get_sound_on_really_damaged(),
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }

            // C++: 25% chance to shout VoiceFear when health crosses yellow.
            if max_health > 0.0
                && (previous_health / max_health) > YELLOW_DAMAGE_PERCENT
                && (current_health / max_health) < YELLOW_DAMAGE_PERCENT
                && current_health > 0.0
                && get_game_logic_random_value(0, 99) < 25
            {
                if let Some(owner) = self.get_owner() {
                    if let Ok(owner_guard) = owner.try_read() {
                        play_voice_fear(&owner_guard);
                    }
                }
            }

            // Check if we died. Object::attempt_damage_with_return also calls
            // handle_death after dropping the body. That caller already holds
            // the object write lock, so try_write fails and death runs once
            // there. A direct body caller does not hold that lock; C++
            // ActiveBody.cpp:641-649 still scores and calls onDie here.
            if current_health <= 0.0 && previous_health > 0.0 {
                if damage_info.input.source_id != INVALID_ID && context.is_none() {
                    let source_id = damage_info.input.source_id;
                    let owner_id = self.owner_id;
                    if source_id == owner_id {
                        if let Some(owner) = self.get_owner() {
                            if let Ok(mut owner_guard) = owner.try_write() {
                                owner_guard.score_self_kill();
                            }
                        }
                    } else {
                        let _ = OBJECT_REGISTRY.with_object_mut(source_id, |damager_guard| {
                            let _ = OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
                                damager_guard.score_the_kill(owner_guard);
                            });
                        });
                    }
                }
                if let Some(owner) = self.get_owner() {
                    if let Ok(mut owner_guard) = owner.try_write() {
                        owner_guard.handle_death(Some(damage_info));
                        drop(owner_guard);
                        let _ = self.do_damage_fx(damage_info);
                        return Ok(());
                    }
                }
                // The object entry holds the write lock and runs death, then FX.
                return Ok(());
            }
        }

        let _ = self.do_damage_fx(damage_info);

        // Repulsor and friend retaliation run on the Object after this guard
        // drops. try_write/try_read fail while attempt_damage holds the object.

        Ok(())
    }
}

impl BodyModuleInterface for ActiveBody {
    fn do_damage_fx_after_death(&mut self, damage_info: &DamageInfo) {
        let _ = self.do_damage_fx(damage_info);
    }

    fn attempt_damage(&mut self, damage_info: &mut DamageInfo) -> BodyResult<()> {
        self.attempt_damage_with_owner_context(damage_info, None)
    }

    fn attempt_damage_with_context(
        &mut self,
        damage_info: &mut DamageInfo,
        context: &super::body_module::BodyDamageContext,
    ) -> BodyResult<()> {
        self.attempt_damage_with_owner_context(damage_info, Some(context))
    }

    fn attempt_healing(&mut self, healing_info: &mut DamageInfo) -> BodyResult<()> {
        self.validate_armor_and_damage_fx()?;

        if healing_info.input.damage_type != DamageType::Healing {
            return self.attempt_damage(healing_info);
        }

        // C++ ActiveBody.cpp:792-795 returns before clearing output.
        if let Some(owner) = self.get_owner() {
            if let Ok(owner_guard) = owner.read() {
                let is_bridge = owner_guard.is_kind_of(KindOf::Bridge)
                    || owner_guard.is_kind_of(KindOf::BridgeTower);
                if owner_guard.is_effectively_dead() && !is_bridge {
                    return Ok(());
                }
            }
        } else {
            let state = &self.state;
            if state.current_health <= 0.0 {
                return Ok(());
            }
        }

        healing_info.output.actual_damage_dealt = 0.0;
        healing_info.output.actual_damage_clipped = 0.0;

        let amount =
            self.adjust_damage_by_armor(healing_info.input.damage_type, healing_info.input.amount);

        if amount > 0.0 {
            let old_state = self.get_damage_state();

            // Do the healing
            self.internal_change_health(amount)?;

            let (previous_health, current_health) = {
                let state = &self.state;
                healing_info.output.actual_damage_dealt = amount;
                healing_info.output.actual_damage_clipped =
                    state.previous_health - state.current_health;
                (state.previous_health, state.current_health)
            };

            let frame_now = current_frame();

            {
                let state = &mut self.state;
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

        // Do damage FX. A failed FX call must not undo or retry the heal.
        let _ = self.do_damage_fx(healing_info);

        Ok(())
    }

    fn estimate_damage(&self, damage_info: &DamageInfoInput) -> BodyResult<f32> {
        // C++ estimateDamage is const and calls validateArmorAndDamageFX, which
        // mutates mutable armor fields. That cache write needs `&mut self` here.
        // Resolve the same armor into a local value so this `&self` path does
        // not mutate the cached armor.
        let armor = self.armor_resolved_for_read()?;

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
                // C++ ActiveBody.cpp:281-287 — pathfinder vs stinger site.
                if let Some(owner) = self.get_owner() {
                    if let Ok(owner_guard) = owner.read() {
                        if owner_guard.is_kind_of(KindOf::Structure)
                            && owner_guard.test_status(ObjectStatusTypes::UnderConstruction)
                        {
                            return Ok(0.0);
                        }
                    }
                }
            }
            _ => {}
        }

        // C++ parity: estimate damage after armor adjustments only.
        let amount = armor.adjust_damage(damage_info.damage_type, damage_info.amount);

        Ok(amount)
    }

    fn get_health(&self) -> f32 {
        self.state.current_health
    }

    fn get_max_health(&self) -> f32 {
        self.state.max_health
    }

    fn get_initial_health(&self) -> f32 {
        self.state.initial_health
    }

    fn get_previous_health(&self) -> f32 {
        self.state.previous_health
    }

    fn get_subdual_damage_heal_rate(&self) -> u32 {
        self.module_data.subdual_damage_heal_rate
    }

    fn get_subdual_damage_heal_amount(&self) -> f32 {
        self.module_data.subdual_damage_heal_amount
    }

    fn has_any_subdual_damage(&self) -> bool {
        self.state.current_subdual_damage > 0.0
    }

    fn get_current_subdual_damage_amount(&self) -> f32 {
        self.state.current_subdual_damage
    }

    fn get_damage_state(&self) -> BodyDamageType {
        self.state.current_damage_state
    }

    fn set_damage_state(&mut self, new_state: BodyDamageType) -> BodyResult<()> {
        if global_data::read_safe().is_err() {
            return self.set_correct_damage_state();
        }
        let (damaged_thresh, really_damaged_thresh) = match global_data::read_safe() {
            Ok(global) => (
                global.unit_damaged_thresh,
                global.unit_really_damaged_thresh,
            ),
            Err(_) => return self.set_correct_damage_state(),
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

        // Promotion audio is played by Object::on_veterancy_level_changed.
        // owner.read() here deadlocks score_the_kill's write lock.
        let _ = provide_feedback;

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
        let multiplier = if old_bonus == 0.0 {
            1.0
        } else {
            new_bonus / old_bonus
        };

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
        {
            let state = &mut self.state;
            let index = armor_type as usize;
            if !state.armor_set_flags.test(index) {
                state.armor_set_flags.set(index, true);
                state.armor_flags_dirty = true;
            }
            Ok(())
        }
    }

    fn clear_armor_set_flag(&mut self, armor_type: ArmorSetType) -> BodyResult<()> {
        {
            let state = &mut self.state;
            let index = armor_type as usize;
            if state.armor_set_flags.test(index) {
                state.armor_set_flags.set(index, false);
                state.armor_flags_dirty = true;
            }
            Ok(())
        }
    }

    fn test_armor_set_flag(&self, armor_type: ArmorSetType) -> bool {
        {
            let state = &self.state;
            state.armor_set_flags.test(armor_type as usize)
        }
    }

    fn get_last_damage_info(&self) -> Option<DamageInfo> {
        self.state.last_damage_info.clone()
    }

    fn get_last_damage_timestamp(&self) -> u32 {
        self.state.last_damage_timestamp
    }

    fn get_last_healing_timestamp(&self) -> u32 {
        self.state.last_healing_timestamp
    }

    fn get_clearable_last_attacker(&self) -> ObjectId {
        {
            let state = &self.state;
            if state.last_damage_cleared {
                INVALID_ID
            } else {
                state
                    .last_damage_info
                    .as_ref()
                    .map(|info| info.source_id)
                    .unwrap_or(INVALID_ID)
            }
        }
    }

    fn clear_last_attacker(&mut self) {
        {
            let state = &mut self.state;
            state.last_damage_cleared = true;
        }
    }

    fn get_front_crushed(&self) -> bool {
        self.state.front_crushed
    }

    fn get_back_crushed(&self) -> bool {
        self.state.back_crushed
    }

    fn set_initial_health(&mut self, initial_percent: i32) -> BodyResult<()> {
        let factor = initial_percent as f32 / 100.0;
        let initial_health = self.get_initial_health();
        let new_health = factor * initial_health;
        let current_health = self.get_health();

        self.internal_change_health(new_health - current_health)
    }

    fn set_initial_health_for_borrowed_owner(
        &mut self,
        initial_percent: i32,
        is_structure: bool,
    ) -> BodyResult<Option<super::body_module::OwnerHealthTransition>> {
        // C++ setInitialHealth uses the current initialHealth, including a
        // preceding map max-health assignment. Preserve the derived floor
        // and previous-health bookkeeping through the canonical kernel.
        let factor = initial_percent as f32 / 100.0;
        let new_health = factor * self.get_initial_health();
        let delta = new_health - self.get_health();
        self.change_health_for_borrowed_owner(delta, is_structure)
            .map(Some)
    }

    fn begin_owner_max_health_change(
        &mut self,
        max_health: f32,
        change_type: MaxHealthChangeType,
    ) -> BodyResult<super::body_module::OwnerMaxHealthChange> {
        let prev_max_health = self.get_max_health();
        let current_health = self.get_health();
        self.state.max_health = max_health;
        self.state.initial_health = max_health;
        let first_delta = match change_type {
            MaxHealthChangeType::PreserveRatio => {
                let ratio = if prev_max_health > 0.0 {
                    current_health / prev_max_health
                } else {
                    1.0
                };
                Some(max_health * ratio - current_health)
            }
            MaxHealthChangeType::AddCurrentHealthToo => Some(max_health - prev_max_health),
            MaxHealthChangeType::SameCurrentHealth => None,
            MaxHealthChangeType::FullyHeal => Some(max_health - current_health),
        };
        Ok(super::body_module::OwnerMaxHealthChange::Active { first_delta })
    }

    fn set_max_health(
        &mut self,
        max_health: f32,
        change_type: MaxHealthChangeType,
    ) -> BodyResult<()> {
        if let super::body_module::OwnerMaxHealthChange::Active {
            first_delta: Some(delta),
        } = self.begin_owner_max_health_change(max_health, change_type)?
        {
            self.internal_change_health(delta)?;
        }
        let now = self.get_health();
        if now > max_health {
            self.internal_change_health(max_health - now)?;
        }
        Ok(())
    }

    fn owner_particle_head(&self) -> Option<u32> {
        self.state
            .particle_systems
            .as_ref()
            .map(|node| node.particle_system_id)
    }
    fn remove_owner_particle_head(&mut self) {
        if let Some(node) = self.state.particle_systems.take() {
            self.state.particle_systems = node.next;
        }
    }
    fn record_owner_particle(&mut self, id: u32) {
        record_body_particle_system(&mut self.state, id);
    }

    fn set_front_crushed(&mut self, crushed: bool) -> BodyResult<()> {
        {
            let state = &mut self.state;
            state.front_crushed = crushed;
            Ok(())
        }
    }

    fn set_back_crushed(&mut self, crushed: bool) -> BodyResult<()> {
        {
            let state = &mut self.state;
            state.back_crushed = crushed;
            Ok(())
        }
    }

    fn apply_damage_scalar(&mut self, scalar: f32) -> BodyResult<()> {
        BodyModuleInterface::apply_damage_scalar(&mut self.base, scalar)
    }

    fn get_damage_scalar(&self) -> f32 {
        BodyModuleInterface::get_damage_scalar(&self.base)
    }

    fn change_health_for_borrowed_owner(
        &mut self,
        delta: f32,
        is_structure: bool,
    ) -> BodyResult<super::body_module::OwnerHealthTransition> {
        let mut changed_state = false;
        let mut effectively_dead = false;
        let floor = self.min_health_floor;
        // C++ ImmortalBody.cpp:34 — clamp delta before ActiveBody so we
        // never die and then un-die. attempt_damage calls this method
        // directly (no Rust virtual), so the floor lives here.
        let current_health = self.get_health();
        let delta = if floor > 0.0 {
            delta.max(-current_health + floor)
        } else {
            delta
        };
        let thresholds = global_data::read_safe().ok().map(|global| {
            (
                global.unit_damaged_thresh,
                global.unit_really_damaged_thresh,
            )
        });
        {
            let state = &mut self.state;
            state.previous_health = state.current_health;

            state.current_health += delta;

            let high = state.max_health.max(floor);
            state.current_health = state.current_health.clamp(floor.min(high), high);

            let old_state = state.current_damage_state;
            state.current_damage_state =
                if let Some((damaged_thresh, really_damaged_thresh)) = thresholds {
                    Self::calc_damage_state(
                        state.current_health,
                        state.max_health,
                        is_structure,
                        damaged_thresh,
                        really_damaged_thresh,
                    )
                } else {
                    BodyDamageType::Pristine
                };

            if state.current_damage_state != old_state {
                changed_state = true;
            }

            effectively_dead = state.current_health <= 0.0;
        }

        Ok(super::body_module::OwnerHealthTransition {
            damage_state: self.get_damage_state(),
            changed_state,
            effectively_dead,
        })
    }

    fn internal_change_health(&mut self, delta: f32) -> BodyResult<()> {
        let is_structure = self.is_structure_for_damage_state();
        let transition = self.change_health_for_borrowed_owner(delta, is_structure)?;
        if transition.changed_state {
            // C++ ActiveBody.cpp:1219 — skip damaged-art while building.
            let under_construction = match self.get_owner() {
                // try_read, matching attempt_damage/evaluate_visual_condition:
                // the owner's write guard may be held by this very thread while
                // the body mutates (self-sourced ticks like fall splat or
                // FlammableUpdate aflame damage applied through the owner's
                // lock); a blocking read() there self-deadlocks.
                Some(owner) => owner
                    .try_read()
                    .map(|guard| guard.test_status(ObjectStatusTypes::UnderConstruction))
                    .unwrap_or(false),
                None => false,
            };
            if !under_construction {
                let _ = self.evaluate_visual_condition();
            }
        }

        // Only clear the bit here. Setting it before handle_death makes that
        // function return and skip on_die. try_write fails when the caller
        // already holds the object; those paths sync after death handling.
        if !transition.effectively_dead {
            if let Some(owner) = self.get_owner() {
                if let Ok(mut owner_guard) = owner.try_write() {
                    if owner_guard.is_effectively_dead() {
                        owner_guard.set_effectively_dead(false);
                    }
                }
            }
        }

        Ok(())
    }

    fn set_indestructible(&mut self, indestructible: bool) -> BodyResult<()> {
        {
            let state = &mut self.state;
            state.indestructible = indestructible;
        }
        // C++ ActiveBody.cpp:1350-1384 — bridges mirror to towers.
        self.mirror_indestructible_to_bridge_towers(indestructible);
        Ok(())
    }

    fn is_indestructible(&self) -> bool {
        self.state.indestructible
    }

    fn evaluate_visual_condition(&mut self) -> BodyResult<()> {
        // C++ ActiveBody::evaluateVisualCondition:
        //   Drawable* draw = getObject()->getDrawable();
        //   if (draw) draw->reactToBodyDamageStateChange(m_curDamageState);
        //   updateBodyParticleSystems();
        let damage_state = self.get_damage_state();
        if let Some(owner) = self.get_owner() {
            if let Ok(owner_guard) = owner.try_read() {
                if let Some(drawable) = owner_guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        draw_guard.react_to_body_damage_state_change_with_owner(
                            damage_state,
                            &owner_guard,
                        );
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
        for group in super::owner_health::body_particle_groups(aflame) {
            self.create_particle_systems(&group.prefix, &group.system, group.count)?;
        }

        Ok(())
    }

    fn snapshot_xfer(
        &mut self,
        xfer: &mut dyn game_engine::common::system::Xfer,
    ) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
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

        let state = &mut self.state;

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
        let mut cursor = state.particle_systems.as_ref();
        while let Some(system) = cursor {
            particle_ids.push(system.particle_system_id);
            cursor = system.next.as_ref();
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
        } else if state.particle_systems.is_some() {
            return Err(
                "ActiveBody::xfer - m_particleSystems should be empty, but is not".to_string(),
            );
        } else {
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

        // C++ BitFlags::xfer: version, then count and ASCII names on save/load.
        const ARMOR_SET_NAMES: [&str; 8] = [
            "VETERAN",
            "ELITE",
            "HERO",
            "PLAYER_UPGRADE",
            "WEAK_VERSUS_BASEDEFENSES",
            "SECOND_LIFE",
            "CRATE_UPGRADE_ONE",
            "CRATE_UPGRADE_TWO",
        ];
        let mut armor_version: XferVersion = 1;
        xfer.xfer_version(&mut armor_version, 1)
            .map_err(|e| e.to_string())?;
        let mut armor_bits = armor_set_flags_to_u32(&state.armor_set_flags);
        if xfer.is_reading() {
            let mut count = 0i32;
            xfer.xfer_int(&mut count).map_err(|e| e.to_string())?;
            armor_bits = 0;
            for _ in 0..count {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| e.to_string())?;
                if let Some(index) = ARMOR_SET_NAMES
                    .iter()
                    .position(|bit| bit.eq_ignore_ascii_case(&name))
                {
                    armor_bits |= 1 << index;
                } else {
                    return Err(format!("ActiveBody armor set flag unknown: {name}"));
                }
            }
            state.armor_set_flags = armor_set_flags_from_u32(armor_bits);
            state.resolved_armor_flags = create_armor_set_flags();
            state.armor_flags_dirty = true;
            state.current_damage_fx_name = None;
        } else if xfer.is_writing() {
            let mut count = armor_bits.count_ones() as i32;
            xfer.xfer_int(&mut count).map_err(|e| e.to_string())?;
            for (index, bit_name) in ARMOR_SET_NAMES.iter().enumerate() {
                if armor_bits & (1 << index) == 0 {
                    continue;
                }
                let mut name = (*bit_name).to_string();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            // C++ CRC is xferUser(this, sizeof(this)): pointer-sized, not names.
            // Hash the flag bits so a CRC does not emit the save strings.
            xfer.xfer_unsigned_int(&mut armor_bits)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()?;
        {
            let state = &mut self.state;
            state.armor_flags_dirty = true;
        }
        Ok(())
    }
}

#[cfg(test)]
mod owned_state_xfer_tests {
    use super::*;
    use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;

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
    fn body_definitions_and_state_are_independent_for_equal_owner_ids() {
        let _lock = crate::test_sync::lock();
        let mut authored = ActiveBodyModuleData::default();
        authored.max_health = 100.0;
        authored.initial_health = 100.0;
        let mut first = ActiveBody::new_with_owner(authored.clone(), 91_900);
        authored.max_health = 250.0;
        authored.initial_health = 200.0;
        let second = ActiveBody::new_with_owner(authored.clone(), 91_900);

        first.internal_change_health(-35.0).unwrap();
        assert_eq!(first.get_health(), 65.0);
        assert_eq!(first.get_max_health(), 100.0);
        assert_eq!(second.get_health(), 200.0);
        assert_eq!(second.get_max_health(), 250.0);
        assert_eq!(authored.initial_health, 200.0);
        assert_eq!(first.module_data.initial_health, 100.0);
        assert_eq!(second.module_data.initial_health, 200.0);

        let mut bytes = Vec::new();
        first
            .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap();
        let mut restored = ActiveBody::new_with_owner(authored, 91_900);
        restored
            .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
            .unwrap();
        assert_eq!(restored.get_health(), first.get_health());
        assert_eq!(restored.get_max_health(), first.get_max_health());
        assert_eq!(
            second.get_health(),
            200.0,
            "restore does not write another body"
        );
    }

    #[test]
    fn active_body_xfer_roundtrips_owned_state_and_particle_ids() {
        let mut saved_body = create_test_active_body();
        saved_body.state.current_health = 61.0;
        saved_body.state.previous_health = 72.0;
        saved_body.state.max_health = 120.0;
        saved_body.state.initial_health = 110.0;
        saved_body.state.current_subdual_damage = 17.0;
        saved_body.state.current_damage_state = BodyDamageType::Damaged;
        saved_body.state.next_damage_fx_time = 234;
        saved_body.state.last_damage_fx_done = DamageType::Flame;
        saved_body.state.last_damage_info = Some(DamageInfo::new());
        saved_body.state.last_damage_timestamp = 123;
        saved_body.state.last_healing_timestamp = 101;
        saved_body.state.front_crushed = true;
        saved_body.state.back_crushed = true;
        saved_body.state.last_damage_cleared = true;
        saved_body.state.indestructible = true;
        saved_body
            .set_armor_set_flag(ArmorSetType::Veteran)
            .expect("armor flag");
        saved_body.state.particle_systems = Some(Box::new(BodyParticleSystem {
            particle_system_id: 0x1234,
            next: Some(Box::new(BodyParticleSystem {
                particle_system_id: 0x5678,
                next: None,
            })),
        }));

        let mut backing = Cursor::new(Vec::new());
        let mut save_xfer = XferSave::new(&mut backing, 1);
        Snapshotable::xfer(&mut saved_body, &mut save_xfer).expect("save ActiveBody");
        drop(save_xfer);
        let bytes = backing.into_inner();
        // C++ ActiveBody -> BodyModule -> BehaviorModule -> ObjectModule ->
        // Module version chain, then damage scalar and the five hull values.
        let mut expected_prefix = vec![1_u8; 5];
        for value in [1.0_f32, 61.0, 17.0, 72.0, 120.0, 110.0] {
            expected_prefix.extend_from_slice(&value.to_le_bytes());
        }
        expected_prefix
            .extend_from_slice(&body_damage_type_to_u32(BodyDamageType::Damaged).to_le_bytes());
        expected_prefix.extend_from_slice(&234_u32.to_le_bytes());
        expected_prefix.extend_from_slice(&(DamageType::Flame as u32).to_le_bytes());
        assert!(bytes.starts_with(&expected_prefix), "C++ body field order");

        let mut restored_body = create_test_active_body();
        let mut load_xfer = XferLoad::new(Cursor::new(bytes), 1);
        Snapshotable::xfer(&mut restored_body, &mut load_xfer).expect("load ActiveBody");

        assert_eq!(restored_body.get_health(), 61.0);
        assert_eq!(restored_body.get_previous_health(), 72.0);
        assert_eq!(restored_body.get_max_health(), 120.0);
        assert_eq!(restored_body.get_initial_health(), 110.0);
        assert_eq!(restored_body.get_current_subdual_damage_amount(), 17.0);
        assert_eq!(restored_body.get_damage_state(), BodyDamageType::Damaged);
        assert_eq!(restored_body.state.next_damage_fx_time, 234);
        assert_eq!(restored_body.state.last_damage_fx_done, DamageType::Flame);
        assert_eq!(restored_body.get_last_damage_timestamp(), 123);
        assert_eq!(restored_body.get_last_healing_timestamp(), 101);
        assert!(restored_body.get_front_crushed());
        assert!(restored_body.get_back_crushed());
        assert_eq!(restored_body.get_clearable_last_attacker(), INVALID_ID);
        assert!(restored_body.is_indestructible());
        assert!(restored_body.test_armor_set_flag(ArmorSetType::Veteran));
        let particles = restored_body
            .state
            .particle_systems
            .as_ref()
            .expect("restored particle systems");
        assert_eq!(particles.particle_system_id, 0x5678);
        assert_eq!(
            particles.next.as_ref().map(|next| next.particle_system_id),
            Some(0x1234)
        );
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
        assert!(
            body.set_max_health(200.0, MaxHealthChangeType::PreserveRatio)
                .is_ok()
        );
        assert_eq!(body.get_max_health(), 200.0);
        assert_eq!(body.get_health(), 100.0); // Should be 50% of 200

        // Test full heal
        assert!(
            body.set_max_health(150.0, MaxHealthChangeType::FullyHeal)
                .is_ok()
        );
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
    fn attempt_damage_water_death_flooded_records_last_death_type() {
        use crate::damage::HUGE_DAMAGE_AMOUNT;

        let mut body = create_test_active_body();
        assert_eq!(body.get_health(), 100.0);

        let mut info = DamageInfo::with_simple(
            HUGE_DAMAGE_AMOUNT,
            INVALID_ID,
            DamageType::Water,
            DeathType::Flooded,
        );
        body.attempt_damage(&mut info)
            .expect("water+flooded damage must apply");

        assert!(body.get_health() <= 0.0);
        let last = body
            .get_last_damage_info()
            .expect("last damage info must record the killing blow");
        assert_eq!(last.input.damage_type, DamageType::Water);
        assert_eq!(last.input.death_type, DeathType::Flooded);
        assert_eq!(last.death_type, DeathType::Flooded);
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

#[cfg(test)]
mod death_flooded_tests {
    use super::*;
    use crate::damage::HUGE_DAMAGE_AMOUNT;

    #[test]
    fn attempt_damage_water_death_flooded_records_last_death_type() {
        // ActiveBody.cpp:547 compares against unsigned frame - 1. At frame zero,
        // the initial FFFFFFFF timestamp takes the attacker-preference branch.
        // Environmental damage has no attacker, so record this blow at logic frame one.
        let _frame = crate::system::game_logic::enter_update_frame(1);
        let mut module_data = ActiveBodyModuleData::default();
        module_data.max_health = 10.0;
        module_data.initial_health = 10.0;
        let mut body = ActiveBody::new(module_data);

        assert_eq!(body.get_health(), 10.0);
        assert!(body.get_last_damage_info().is_none());

        let mut info = DamageInfo::with_simple(
            HUGE_DAMAGE_AMOUNT,
            INVALID_ID,
            DamageType::Water,
            DeathType::Flooded,
        );
        body.attempt_damage(&mut info)
            .expect("water+flooded damage must apply even without a dual-world registry");

        assert!(body.get_health() <= 0.0);
        let last = body
            .get_last_damage_info()
            .expect("last damage info must record the killing blow");
        assert_eq!(last.input.damage_type, DamageType::Water);
        assert_eq!(last.input.death_type, DeathType::Flooded);
        assert_eq!(last.damage_type, DamageType::Water);
        assert_eq!(last.death_type, DeathType::Flooded);
        assert_eq!(body.get_last_damage_timestamp(), 1);
    }

    #[derive(Debug)]
    struct TestContain {
        ids: Vec<ObjectId>,
        rider_change: bool,
    }

    impl crate::modules::ContainModuleInterface for TestContain {
        fn can_contain(&self, _object_id: ObjectId) -> bool {
            true
        }
        fn contain_object(&mut self, object_id: ObjectId) -> Result<(), String> {
            self.ids.push(object_id);
            Ok(())
        }
        fn release_object(&mut self, object_id: ObjectId) -> Result<(), String> {
            self.ids.retain(|id| *id != object_id);
            Ok(())
        }
        fn get_contained_objects(&self) -> std::borrow::Cow<'_, [ObjectId]> {
            std::borrow::Cow::Borrowed(&self.ids)
        }
        fn get_contained_count(&self) -> usize {
            self.ids.len()
        }
        fn get_max_capacity(&self) -> usize {
            8
        }
        fn is_rider_change_contain(&self) -> bool {
            self.rider_change
        }
    }

    #[derive(Debug)]
    struct TestAi {
        moving: bool,
    }

    impl crate::modules::AIUpdateInterface for TestAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn is_moving(&self) -> bool {
            self.moving
        }
        fn is_idle(&self) -> bool {
            !self.moving
        }
        fn set_movement_target(&mut self, _target: &crate::common::Coord3D) -> Result<(), String> {
            Ok(())
        }
    }

    fn vehicle_with_contain(
        id: ObjectId,
        rider_change: bool,
        moving: bool,
        occupants: Vec<ObjectId>,
    ) -> std::sync::Arc<std::sync::RwLock<Object>> {
        let mut template = crate::common::DefaultThingTemplate::new(format!("Veh{id}"));
        template.add_kind_of(crate::common::KindOf::Vehicle);
        let mut obj = Object::new_test_from_template(id, 100.0, std::sync::Arc::new(template));
        obj.set_contain(Some(std::sync::Arc::new(std::sync::Mutex::new(
            TestContain {
                ids: occupants,
                rider_change,
            },
        ))));
        obj.set_ai_update_interface(Some(std::sync::Arc::new(std::sync::Mutex::new(TestAi {
            moving,
        }))));
        let arc = std::sync::Arc::new(std::sync::RwLock::new(obj));
        OBJECT_REGISTRY.register_object(id, &arc);
        arc
    }

    #[test]
    fn kill_pilot_unmans_ordinary_vehicle_without_hull_damage() {
        // C++ ActiveBody.cpp:399-410 ordinary vehicle DAMAGE_KILLPILOT path.
        OBJECT_REGISTRY.clear();
        let tank = vehicle_with_contain(501, false, false, vec![502]);
        let rider = std::sync::Arc::new(std::sync::RwLock::new(Object::new_test(502, 50.0)));
        OBJECT_REGISTRY.register_object(502, &rider);

        let mut body = ActiveBody::new_with_owner(ActiveBodyModuleData::default(), 501);
        let mut info =
            DamageInfo::with_simple(1.0, INVALID_ID, DamageType::KillPilot, DeathType::Normal);
        body.attempt_damage(&mut info).expect("killpilot");

        let tank_g = tank.read().unwrap();
        assert!(
            (tank_g.get_health() - 100.0).abs() < 1e-3,
            "KillPilot must not apply hull HP, got {}",
            tank_g.get_health()
        );
        assert!(tank_g.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned));
        assert!(!tank_g.is_effectively_dead());
        drop(tank_g);
        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn kill_pilot_destroys_moving_rider_change_bike() {
        // C++ ActiveBody.cpp:380-385: moving RiderChangeContain bike is killed.
        OBJECT_REGISTRY.clear();
        let bike = vehicle_with_contain(601, true, true, vec![602]);
        let rider = std::sync::Arc::new(std::sync::RwLock::new(Object::new_test(602, 50.0)));
        OBJECT_REGISTRY.register_object(602, &rider);

        let mut body = ActiveBody::new_with_owner(ActiveBodyModuleData::default(), 601);
        let mut info =
            DamageInfo::with_simple(1.0, INVALID_ID, DamageType::KillPilot, DeathType::Normal);
        body.attempt_damage(&mut info).expect("killpilot");

        let bike_g = bike.read().unwrap();
        assert!(
            bike_g.is_effectively_dead() || bike_g.get_health() <= 0.0,
            "moving combat bike must be destroyed (C++ obj->kill)"
        );
        assert!(!bike_g.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned));
        drop(bike_g);
        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn kill_pilot_kills_rider_on_stationary_bike() {
        // C++ ActiveBody.cpp:387-396: stationary bike evacuates + kills rider.
        OBJECT_REGISTRY.clear();
        let bike = vehicle_with_contain(701, true, false, vec![702]);
        let rider = std::sync::Arc::new(std::sync::RwLock::new(Object::new_test(702, 50.0)));
        OBJECT_REGISTRY.register_object(702, &rider);

        let mut body = ActiveBody::new_with_owner(ActiveBodyModuleData::default(), 701);
        let mut info =
            DamageInfo::with_simple(1.0, INVALID_ID, DamageType::KillPilot, DeathType::Normal);
        body.attempt_damage(&mut info).expect("killpilot");

        let bike_g = bike.read().unwrap();
        assert!(
            (bike_g.get_health() - 100.0).abs() < 1e-3,
            "stationary bike hull must survive KillPilot"
        );
        assert!(
            !bike_g.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned),
            "stationary bike is scuttled via rider evacuate, not UNMANNED"
        );
        drop(bike_g);
        let rider_g = rider.read().unwrap();
        assert!(
            rider_g.is_effectively_dead() || rider_g.get_health() <= 0.0,
            "stationary bike rider must be killed (C++ rider->kill)"
        );
        drop(rider_g);
        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn set_max_health_routes_through_internal_change_health() {
        // C++ ActiveBody::setMaxHealth (ActiveBody.cpp:873-922) uses
        // internalChangeHealth. Pre-fix wrote current_health directly,
        // skipping evaluateVisualCondition (UndeadBody second-life art)
        // and the 0-HP low cap.
        let mut module_data = ActiveBodyModuleData::default();
        module_data.max_health = 100.0;
        module_data.initial_health = 100.0;
        let mut body = ActiveBody::new(module_data);
        assert!(body.internal_change_health(-50.0).is_ok());
        assert_eq!(body.get_health(), 50.0);

        assert!(
            body.set_max_health(10.0, MaxHealthChangeType::AddCurrentHealthToo)
                .is_ok()
        );
        assert_eq!(body.get_max_health(), 10.0);
        assert_eq!(
            body.get_health(),
            0.0,
            "ADD_CURRENT_HEALTH_TOO must go through internalChangeHealth low cap"
        );

        let mut module_data = ActiveBodyModuleData::default();
        module_data.max_health = 100.0;
        module_data.initial_health = 100.0;
        let mut body = ActiveBody::new(module_data);
        assert!(body.internal_change_health(-90.0).is_ok());
        assert_eq!(body.get_damage_state(), BodyDamageType::ReallyDamaged);
        assert!(
            body.set_max_health(25.0, MaxHealthChangeType::FullyHeal)
                .is_ok()
        );
        assert_eq!(body.get_health(), 25.0);
        assert_eq!(body.get_previous_health(), 10.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);
    }
}

#[cfg(test)]
mod damage_state_edge_tests {
    use super::{ActiveBody, BodyDamageType};

    #[test]
    fn nonpositive_maximum_uses_original_ieee_ratio_branches() {
        // ActiveBody.cpp:83-112 has no nonpositive-maximum special case.
        for (health, maximum, expected) in [
            (0.0, 0.0, BodyDamageType::Rubble),
            (1.0, 0.0, BodyDamageType::Pristine),
            (-1.0, 0.0, BodyDamageType::Rubble),
            (1.0, -1.0, BodyDamageType::Rubble),
            (-0.25, -1.0, BodyDamageType::Damaged),
            (-0.05, -1.0, BodyDamageType::ReallyDamaged),
            (f32::NAN, 100.0, BodyDamageType::Rubble),
        ] {
            assert_eq!(
                ActiveBody::calc_damage_state(health, maximum, false, 0.5, 0.1),
                expected,
                "health={health}, maximum={maximum}"
            );
        }
    }
}

#[cfg(test)]
impl ActiveBody {
    pub(crate) fn is_bound_to_template_for_test(&self, template: &Arc<dyn ThingTemplate>) -> bool {
        self.engine_template
            .as_ref()
            .map(|bound| {
                std::ptr::eq(
                    Arc::as_ptr(bound) as *const (),
                    Arc::as_ptr(template) as *const (),
                )
            })
            .unwrap_or(false)
    }
}
