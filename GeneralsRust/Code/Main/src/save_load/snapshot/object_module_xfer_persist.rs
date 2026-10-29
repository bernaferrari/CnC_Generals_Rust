//! Persist leftover-matching per-object module xfer residuals.
//!
//! Leftover Snapshotable already matches C++:
//! - `PhysicsBehaviorUpdate::xfer` v2 rates/flags (`physics_update.rs`)
//! - `BattleBusSlowDeathBehavior::xfer` first-death / undeath
//! - `SlowDeathBehavior` / jet / heli sink-midpoint-destruction
//! - `RadarUpdate::xfer` `m_extendDoneFrame` / `m_extendComplete` / `m_radarActive`
//!
//! Live snapshot recreate builds a fresh `Object` and never copied those
//! fields, so a stunned tumble, bus second-life, death collapse, or radar
//! dish snapped back to idle on load.
//!
//! Persist a Rust-only tagged suffix after the contain/producer payload and
//! before the terminal owner-footer chain.
//! OXFR v10 adds queued/active Particle Cannon manual-drive settings. OXFR v9
//! adds a typed terminal locator for persisted names. OXFR v8 carries
//! Object-completion provenance. OXFR v7 adds per-module CostModifierUpgrade
//! execution state. OXFR v6 adds
//! per-module StatusBitsUpgrade execution state. OXFR v5 adds per-module
//! ObjectCreationUpgrade execution state. OXFR v4 adds per-module RadarUpgrade executed state and preserves Object-owned retained death status and KeepObject runtime
//! separately from the existing functional onDie latch. C++ Object::xfer
//! persists object/private status and ActiveBody::xfer persists BODY_RUBBLE;
//! KeepObjectDie::xfer adds no custom retained fields. These are Rust runtime
//! records, not new C++ wire fields. Versions 1–3 are rejected on restore.
//! The enclosing WorldSnapshot advances to v38, retains v37 Object completion
//! provenance and v36's GOTS continuation contract.

use crate::game_logic::host_battle_bus::HostBattleBusBodyData;
use crate::game_logic::host_helicopter_slow_death::HostHelicopterSlowDeathData;
use crate::game_logic::host_jet_slow_death::HostJetSlowDeathData;
use crate::game_logic::host_slow_death::HostSlowDeathData;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const OXFR_MAGIC: &[u8; 4] = b"OXFR";
const OXFR_FOOTER: &[u8; 4] = b"OXFE";
const OXFR_VERSION: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectModuleXferPersistPayload {
    stun: Vec<StunPersist>,
    battle_bus: Vec<BattleBusPersist>,
    slow_death: Vec<SlowDeathPersist>,
    radar: Vec<RadarPersist>,
    death_start: Vec<DeathStartPersist>,
    retained_death: Vec<RetainedDeathStatePersist>,
    object_creation_upgrade: Vec<ObjectCreationUpgradePersist>,
    status_bits_executed_present: bool,
    cost_modifier_executed_present: bool,
    completed_object_upgrades: Vec<ObjectUpgradeCompletionPersist>,
    completion_snapshot_present: bool,
    particle_drive_strikes: Vec<ParticleDriveStrikePersist>,
    particle_drive_beams: Vec<ParticleDriveBeamPersist>,
    particle_drive_snapshot_present: bool,
    #[serde(skip)]
    wire_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParticleDriveSettingsPersist {
    manual_driving_speed: f32,
    manual_fast_driving_speed: f32,
    double_click_to_fast_drive_delay: u32,
}

impl From<crate::game_logic::special_power_strikes::ParticleManualDriveSettings>
    for ParticleDriveSettingsPersist
{
    fn from(value: crate::game_logic::special_power_strikes::ParticleManualDriveSettings) -> Self {
        Self {
            manual_driving_speed: value.manual_driving_speed,
            manual_fast_driving_speed: value.manual_fast_driving_speed,
            double_click_to_fast_drive_delay: value.double_click_to_fast_drive_delay,
        }
    }
}

impl From<ParticleDriveSettingsPersist>
    for crate::game_logic::special_power_strikes::ParticleManualDriveSettings
{
    fn from(value: ParticleDriveSettingsPersist) -> Self {
        Self {
            manual_driving_speed: value.manual_driving_speed,
            manual_fast_driving_speed: value.manual_fast_driving_speed,
            double_click_to_fast_drive_delay: value.double_click_to_fast_drive_delay,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParticleDriveStrikePersist {
    strike_id: u32,
    settings: ParticleDriveSettingsPersist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParticleDriveBeamPersist {
    beam_id: u32,
    settings: ParticleDriveSettingsPersist,
}

/// OXFR v9 payload layout. Keep the old positional body stable.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectModuleXferPersistPayloadV9 {
    stun: Vec<StunPersist>,
    battle_bus: Vec<BattleBusPersist>,
    slow_death: Vec<SlowDeathPersist>,
    radar: Vec<RadarPersist>,
    death_start: Vec<DeathStartPersist>,
    retained_death: Vec<RetainedDeathStatePersist>,
    object_creation_upgrade: Vec<ObjectCreationUpgradePersist>,
    status_bits_executed_present: bool,
    cost_modifier_executed_present: bool,
    completed_object_upgrades: Vec<ObjectUpgradeCompletionPersist>,
    completion_snapshot_present: bool,
}

impl From<ObjectModuleXferPersistPayloadV9> for ObjectModuleXferPersistPayload {
    fn from(value: ObjectModuleXferPersistPayloadV9) -> Self {
        Self {
            stun: value.stun,
            battle_bus: value.battle_bus,
            slow_death: value.slow_death,
            radar: value.radar,
            death_start: value.death_start,
            retained_death: value.retained_death,
            object_creation_upgrade: value.object_creation_upgrade,
            status_bits_executed_present: value.status_bits_executed_present,
            cost_modifier_executed_present: value.cost_modifier_executed_present,
            completed_object_upgrades: value.completed_object_upgrades,
            completion_snapshot_present: value.completion_snapshot_present,
            particle_drive_strikes: Vec::new(),
            particle_drive_beams: Vec::new(),
            particle_drive_snapshot_present: false,
            wire_version: 9,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectUpgradeCompletionPersist {
    object_id: u32,
    names: Vec<String>,
}

/// OXFR v7 positional layout. Keep the old payload stable for saved streams.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectModuleXferPersistPayloadV7 {
    stun: Vec<StunPersist>,
    battle_bus: Vec<BattleBusPersist>,
    slow_death: Vec<SlowDeathPersist>,
    radar: Vec<RadarPersist>,
    death_start: Vec<DeathStartPersist>,
    retained_death: Vec<RetainedDeathStatePersist>,
    object_creation_upgrade: Vec<ObjectCreationUpgradePersist>,
    status_bits_executed_present: bool,
    cost_modifier_executed_present: bool,
}

impl From<ObjectModuleXferPersistPayloadV7> for ObjectModuleXferPersistPayload {
    fn from(value: ObjectModuleXferPersistPayloadV7) -> Self {
        Self {
            stun: value.stun,
            battle_bus: value.battle_bus,
            slow_death: value.slow_death,
            radar: value.radar,
            death_start: value.death_start,
            retained_death: value.retained_death,
            object_creation_upgrade: value.object_creation_upgrade,
            status_bits_executed_present: value.status_bits_executed_present,
            cost_modifier_executed_present: value.cost_modifier_executed_present,
            completed_object_upgrades: Vec::new(),
            completion_snapshot_present: false,
            particle_drive_strikes: Vec::new(),
            particle_drive_beams: Vec::new(),
            particle_drive_snapshot_present: false,
            wire_version: 7,
        }
    }
}

/// OXFR v4 payload layout. Keep its field order stable for existing saves.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectModuleXferPersistPayloadV4 {
    stun: Vec<StunPersist>,
    battle_bus: Vec<BattleBusPersist>,
    slow_death: Vec<SlowDeathPersist>,
    radar: Vec<RadarPersist>,
    death_start: Vec<DeathStartPersist>,
    retained_death: Vec<RetainedDeathStatePersist>,
}

/// OXFR v5 payload layout. Keep its field order stable for existing saves.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectModuleXferPersistPayloadV5 {
    stun: Vec<StunPersist>,
    battle_bus: Vec<BattleBusPersist>,
    slow_death: Vec<SlowDeathPersist>,
    radar: Vec<RadarPersist>,
    death_start: Vec<DeathStartPersist>,
    retained_death: Vec<RetainedDeathStatePersist>,
    object_creation_upgrade: Vec<ObjectCreationUpgradePersistV5>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectCreationUpgradePersistV5 {
    object_id: u32,
    executed: Vec<bool>,
}

/// OXFR v6 payload layout; its positional field order is a saved wire contract.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectModuleXferPersistPayloadV6 {
    stun: Vec<StunPersist>,
    battle_bus: Vec<BattleBusPersist>,
    slow_death: Vec<SlowDeathPersist>,
    radar: Vec<RadarPersist>,
    death_start: Vec<DeathStartPersist>,
    retained_death: Vec<RetainedDeathStatePersist>,
    object_creation_upgrade: Vec<ObjectCreationUpgradePersistV6>,
    status_bits_executed_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectCreationUpgradePersistV6 {
    object_id: u32,
    executed: Vec<bool>,
    status_bits_executed: Vec<bool>,
}

impl From<ObjectModuleXferPersistPayloadV6> for ObjectModuleXferPersistPayload {
    fn from(value: ObjectModuleXferPersistPayloadV6) -> Self {
        Self {
            stun: value.stun,
            battle_bus: value.battle_bus,
            slow_death: value.slow_death,
            radar: value.radar,
            death_start: value.death_start,
            retained_death: value.retained_death,
            object_creation_upgrade: value
                .object_creation_upgrade
                .into_iter()
                .map(|entry| ObjectCreationUpgradePersist {
                    object_id: entry.object_id,
                    executed: entry.executed,
                    status_bits_executed: entry.status_bits_executed,
                    cost_modifier_executed: Vec::new(),
                })
                .collect(),
            status_bits_executed_present: value.status_bits_executed_present,
            cost_modifier_executed_present: false,
            completed_object_upgrades: Vec::new(),
            completion_snapshot_present: false,
            particle_drive_strikes: Vec::new(),
            particle_drive_beams: Vec::new(),
            particle_drive_snapshot_present: false,
            wire_version: 6,
        }
    }
}

impl From<ObjectModuleXferPersistPayloadV4> for ObjectModuleXferPersistPayload {
    fn from(value: ObjectModuleXferPersistPayloadV4) -> Self {
        Self {
            stun: value.stun,
            battle_bus: value.battle_bus,
            slow_death: value.slow_death,
            radar: value.radar,
            death_start: value.death_start,
            retained_death: value.retained_death,
            object_creation_upgrade: Vec::new(),
            status_bits_executed_present: false,
            cost_modifier_executed_present: false,
            completed_object_upgrades: Vec::new(),
            completion_snapshot_present: false,
            particle_drive_strikes: Vec::new(),
            particle_drive_beams: Vec::new(),
            particle_drive_snapshot_present: false,
            wire_version: 4,
        }
    }
}

impl From<ObjectModuleXferPersistPayloadV5> for ObjectModuleXferPersistPayload {
    fn from(value: ObjectModuleXferPersistPayloadV5) -> Self {
        Self {
            stun: value.stun,
            battle_bus: value.battle_bus,
            slow_death: value.slow_death,
            radar: value.radar,
            death_start: value.death_start,
            retained_death: value.retained_death,
            object_creation_upgrade: value
                .object_creation_upgrade
                .into_iter()
                .map(|entry| ObjectCreationUpgradePersist {
                    object_id: entry.object_id,
                    executed: entry.executed,
                    status_bits_executed: Vec::new(),
                    cost_modifier_executed: Vec::new(),
                })
                .collect(),
            status_bits_executed_present: false,
            cost_modifier_executed_present: false,
            completed_object_upgrades: Vec::new(),
            completion_snapshot_present: false,
            particle_drive_strikes: Vec::new(),
            particle_drive_beams: Vec::new(),
            particle_drive_snapshot_present: false,
            wire_version: 5,
        }
    }
}

/// Rust's functional onDie reentry latch, independent of health/death modules.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeathStartPersist {
    object_id: u32,
    on_die_started: bool,
}

/// Exact owned status/module state; zero HP or the onDie latch cannot infer it.
/// KeepObjectDie itself has no corresponding custom C++ Xfer fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RetainedDeathStatePersist {
    object_id: u32,
    effectively_dead: bool,
    keep_as_rubble: bool,
    keep_object_die: Option<KeepObjectDiePersist>,
}

/// Typed Rust persistence boundary for the existing private KeepObject runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeepObjectDiePersist {
    is_rubble: bool,
    rubble_frame: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StunPersist {
    object_id: u32,
    shock_stun_frames: u32,
    shock_yaw_rate: f32,
    shock_pitch_rate: f32,
    shock_roll_rate: f32,
    shock_allow_bounce: bool,
    shock_was_airborne: bool,
    shock_grounded_once: bool,
    shock_up_z: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BattleBusPersist {
    object_id: u32,
    armor_set_second_life: bool,
    battle_bus_body: Option<HostBattleBusBodyData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlowDeathPersist {
    object_id: u32,
    slow_death: Option<HostSlowDeathData>,
    jet_slow_death: Option<HostJetSlowDeathData>,
    helicopter_slow_death: Option<HostHelicopterSlowDeathData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadarPersist {
    executed: Vec<bool>,
    contributors: Vec<Vec<u32>>,
    object_id: u32,
    radar_extend_done_frame: u32,
    radar_extend_complete: bool,
    radar_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectCreationUpgradePersist {
    object_id: u32,
    executed: Vec<bool>,
    status_bits_executed: Vec<bool>,
    cost_modifier_executed: Vec<bool>,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) -> SaveLoadResult<()> {
    let payload = capture(game_logic);
    append_payload_record(bytes, &payload)
}

fn decode_payload(bytes: &[u8]) -> SaveLoadResult<Option<ObjectModuleXferPersistPayload>> {
    let Some((start, end)) = completion_record_range(bytes)? else {
        return Ok(None);
    };
    let suffix = &bytes[start + OXFR_MAGIC.len()..end];
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if !(4..=OXFR_VERSION).contains(&version) {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown OXFR suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "OXFR payload truncated".to_string(),
        ));
    }
    let mut payload: ObjectModuleXferPersistPayload = if version == 4 {
        let old: ObjectModuleXferPersistPayloadV4 =
            bincode_legacy::deserialize(&rest[..payload_len]).map_err(|err| {
                SaveLoadError::Corrupted(format!("OXFR v4 payload decode: {err}"))
            })?;
        old.into()
    } else if version == 5 {
        let old: ObjectModuleXferPersistPayloadV5 =
            bincode_legacy::deserialize(&rest[..payload_len]).map_err(|err| {
                SaveLoadError::Corrupted(format!("OXFR v5 payload decode: {err}"))
            })?;
        old.into()
    } else if version == 6 {
        let old: ObjectModuleXferPersistPayloadV6 =
            bincode_legacy::deserialize(&rest[..payload_len]).map_err(|err| {
                SaveLoadError::Corrupted(format!("OXFR v6 payload decode: {err}"))
            })?;
        old.into()
    } else if version == 7 {
        let old: ObjectModuleXferPersistPayloadV7 =
            bincode_legacy::deserialize(&rest[..payload_len]).map_err(|err| {
                SaveLoadError::Corrupted(format!("OXFR v7 payload decode: {err}"))
            })?;
        old.into()
    } else if version == 8 || version == 9 {
        let old: ObjectModuleXferPersistPayloadV9 =
            bincode_legacy::deserialize(&rest[..payload_len]).map_err(|err| {
                SaveLoadError::Corrupted(format!("OXFR v{version} payload decode: {err}"))
            })?;
        old.into()
    } else {
        bincode_legacy::deserialize(&rest[..payload_len])
            .map_err(|err| SaveLoadError::Corrupted(format!("OXFR payload decode: {err}")))?
    };
    payload.wire_version = version;
    Ok(Some(payload))
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(payload) = decode_payload(bytes)? else {
        if game_logic
            .host_objects()
            .registered()
            .any(|(_, object)| !object.applied_upgrades.is_empty())
        {
            return Err(SaveLoadError::IncompatibleSave(
                "legacy save has ambiguous Object/player upgrade completion provenance".into(),
            ));
        }
        return Ok(());
    };
    if !payload.completion_snapshot_present
        && game_logic
            .host_objects()
            .registered()
            .any(|(_, object)| !object.applied_upgrades.is_empty())
    {
        return Err(SaveLoadError::IncompatibleSave(
            "legacy save has ambiguous Object/player upgrade completion provenance".into(),
        ));
    }
    apply_payload(game_logic, payload)?;
    Ok(())
}

/// Validate provenance before SnapshotBuilder mutates the receiving world.
pub fn validate_completion_provenance_before_restore(
    bytes: &[u8],
    version: u32,
    objects: &std::collections::HashMap<ObjectId, super::object::ObjectSnapshot>,
    special_powers: &super::special_powers::SpecialPowerStrikeRegistrySnapshot,
    game_logic: &GameLogic,
) -> SaveLoadResult<()> {
    let payload = decode_payload(bytes)?;
    let completion_state = payload
        .as_ref()
        .is_some_and(|payload| payload.completion_snapshot_present);
    if version >= super::types::WORLD_SNAPSHOT_OBJECT_COMPLETION_VERSION && !completion_state {
        return Err(SaveLoadError::Corrupted(
            "current snapshot is missing OXFR object-completion continuation".into(),
        ));
    }
    if version >= super::types::WORLD_SNAPSHOT_PARTICLE_DRIVE_SETTINGS_VERSION
        && !payload
            .as_ref()
            .is_some_and(|payload| payload.wire_version >= 10 && payload.particle_drive_snapshot_present)
    {
        return Err(SaveLoadError::Corrupted(
            "version-38 snapshot is missing OXFR v10 Particle Cannon settings".into(),
        ));
    }
    if let Some(payload) = payload
        .as_ref()
        .filter(|payload| payload.completion_snapshot_present)
    {
        if payload.particle_drive_snapshot_present {
            let expected_strikes: std::collections::HashSet<_> = special_powers
                .strikes
                .iter()
                .filter(|strike| {
                    strike.kind
                        == crate::game_logic::special_power_strikes::HostSuperweaponKind::ParticleCannon
                })
                .map(|strike| strike.id)
                .collect();
            let actual_strikes: std::collections::HashSet<_> = payload
                .particle_drive_strikes
                .iter()
                .map(|row| row.strike_id)
                .collect();
            let expected_beams: std::collections::HashSet<_> = special_powers
                .beam_fields
                .iter()
                .map(|beam| beam.id)
                .collect();
            let actual_beams: std::collections::HashSet<_> = payload
                .particle_drive_beams
                .iter()
                .map(|row| row.beam_id)
                .collect();
            let valid_settings = payload
                .particle_drive_strikes
                .iter()
                .map(|row| &row.settings)
                .chain(payload.particle_drive_beams.iter().map(|row| &row.settings))
                .all(|settings| {
                    settings.manual_driving_speed.is_finite()
                        && settings.manual_fast_driving_speed.is_finite()
                });
            if actual_strikes.len() != payload.particle_drive_strikes.len()
                || actual_beams.len() != payload.particle_drive_beams.len()
                || actual_strikes != expected_strikes
                || actual_beams != expected_beams
                || !valid_settings
            {
                return Err(SaveLoadError::Corrupted(
                    "invalid or incomplete OXFR Particle Cannon settings rows".into(),
                ));
            }
        }
        let mut object_ids = std::collections::HashSet::new();
        for row in &payload.completed_object_upgrades {
            let id = ObjectId(row.object_id);
            if !object_ids.insert(row.object_id)
                || !objects.contains_key(&id)
                || row.names.is_empty()
            {
                return Err(SaveLoadError::Corrupted(
                    "invalid or duplicate Object upgrade completion owner".into(),
                ));
            }
            let mut names = std::collections::HashSet::new();
            for name in &row.names {
                if name.trim().is_empty()
                    || !names.insert(name.to_ascii_lowercase())
                    || game_logic.upgrade_template(name).is_none()
                {
                    return Err(SaveLoadError::Corrupted(
                        "invalid Object upgrade completion name".into(),
                    ));
                }
            }
        }
        return Ok(());
    }
    let ambiguous = objects.values().any(|object| {
        matches!(
            object.modules.get("Upgrade"),
            Some(super::object::ModuleSnapshot::Upgrade(upgrades))
                if !upgrades.active_upgrades.is_empty()
        )
    });
    if ambiguous {
        return Err(SaveLoadError::IncompatibleSave(
            "legacy save has ambiguous Object/player upgrade completion provenance".into(),
        ));
    }
    Ok(())
}

/// WorldSnapshot::default is a valid current empty save value.
pub fn append_empty_completion_capsule(bytes: &mut Vec<u8>) -> SaveLoadResult<()> {
    let payload = ObjectModuleXferPersistPayload {
        completion_snapshot_present: true,
        particle_drive_snapshot_present: true,
        ..Default::default()
    };
    append_payload_record(bytes, &payload)
}

pub fn upgrade_completion_capsule_for_save(
    bytes: &mut Vec<u8>,
    version: u32,
    objects: &std::collections::HashMap<ObjectId, super::object::ObjectSnapshot>,
    special_powers: &super::special_powers::SpecialPowerStrikeRegistrySnapshot,
) -> SaveLoadResult<()> {
    let mut payload = decode_payload(bytes)?;
    if let Some(current) = payload
        .as_mut()
        .filter(|payload| payload.completion_snapshot_present)
    {
        if current.particle_drive_snapshot_present {
            return Ok(());
        }
        if version >= super::types::WORLD_SNAPSHOT_PARTICLE_DRIVE_SETTINGS_VERSION {
            return Err(SaveLoadError::Corrupted(
                "version-38 snapshot is missing OXFR v10 Particle Cannon settings".into(),
            ));
        }
        // v37 carried the same positional strike/beam data without the
        // receiving Cannon module settings. Preserve prior Main behavior when
        // such an older snapshot is rewritten into the v38 contract.
        let upgraded = payload_with_default_particle_settings(current.clone(), special_powers);
        append_payload_record(bytes, &upgraded)?;
        return Ok(());
    }
    if version >= super::types::WORLD_SNAPSHOT_OBJECT_COMPLETION_VERSION {
        return Err(SaveLoadError::Corrupted(
            "current snapshot is missing OXFR object-completion continuation".into(),
        ));
    }
    let ambiguous = objects.values().any(|object| {
        matches!(
            object.modules.get("Upgrade"),
            Some(super::object::ModuleSnapshot::Upgrade(upgrades))
                if !upgrades.active_upgrades.is_empty()
        )
    });
    if ambiguous {
        return Err(SaveLoadError::IncompatibleSave(
            "legacy save has ambiguous Object/player upgrade completion provenance".into(),
        ));
    }
    let mut payload = payload.take().unwrap_or_default();
    payload.completed_object_upgrades.clear();
    payload.completion_snapshot_present = true;
    payload = payload_with_default_particle_settings(payload, special_powers);
    append_payload_record(bytes, &payload)
}

fn payload_with_default_particle_settings(
    mut payload: ObjectModuleXferPersistPayload,
    special_powers: &super::special_powers::SpecialPowerStrikeRegistrySnapshot,
) -> ObjectModuleXferPersistPayload {
    payload.particle_drive_strikes = special_powers
        .strikes
        .iter()
        .filter(|strike| {
            strike.kind
                == crate::game_logic::special_power_strikes::HostSuperweaponKind::ParticleCannon
        })
        .map(|strike| ParticleDriveStrikePersist {
            strike_id: strike.id,
            settings: crate::game_logic::special_power_strikes::ParticleManualDriveSettings::default().into(),
        })
        .collect();
    payload.particle_drive_beams = special_powers
        .beam_fields
        .iter()
        .map(|beam| ParticleDriveBeamPersist {
            beam_id: beam.id,
            settings: crate::game_logic::special_power_strikes::ParticleManualDriveSettings::default().into(),
        })
        .collect();
    payload.particle_drive_strikes.sort_by_key(|row| row.strike_id);
    payload.particle_drive_beams.sort_by_key(|row| row.beam_id);
    payload.particle_drive_snapshot_present = true;
    payload.wire_version = OXFR_VERSION;
    payload
}

fn append_payload_record(
    bytes: &mut Vec<u8>,
    payload: &ObjectModuleXferPersistPayload,
) -> SaveLoadResult<()> {
    let encoded = bincode_legacy::serialize(payload)
        .map_err(|error| SaveLoadError::Corrupted(format!("encoding OXFR payload: {error}")))?;
    let mut record = Vec::with_capacity(OXFR_MAGIC.len() + 16 + encoded.len());
    record.extend_from_slice(OXFR_MAGIC);
    append_u32(&mut record, OXFR_VERSION);
    append_u32(&mut record, encoded.len() as u32);
    record.extend_from_slice(&encoded);
    append_u32(&mut record, encoded.len() as u32);
    record.extend_from_slice(OXFR_FOOTER);

    // OXFR sits at the typed AIQF prefix boundary. Its v9 end locator keeps
    // `OXFR` inside an authored upgrade name from looking like a record.
    let boundary = completion_record_boundary(bytes)?;
    if let Some((start, end)) = completion_record_range(bytes)? {
        if end == boundary {
            bytes.splice(start..end, record);
        } else {
            // v4-v8 records were emitted earlier in the legacy extension
            // prefix. Replace that record and place v9 at the typed boundary.
            bytes.drain(start..end);
            let insertion = completion_record_boundary(bytes)?;
            bytes.splice(insertion..insertion, record);
        }
    } else {
        bytes.splice(boundary..boundary, record);
    }
    Ok(())
}

fn completion_record_boundary(bytes: &[u8]) -> SaveLoadResult<usize> {
    super::named_unit_history_persist::suffix_start(bytes)
}

fn completion_record_range(bytes: &[u8]) -> SaveLoadResult<Option<(usize, usize)>> {
    Ok(
        super::hotkey_squad_persist::find_lifecycle_record(bytes, OXFR_MAGIC)?
            .map(|range| (range.start, range.end)),
    )
}

/// Detached observations of the exact saved lifecycle flags, independent of HP.
#[derive(Default)]
pub(super) struct SavedDeathState {
    pub started: bool,
    pub effectively_dead: bool,
}

pub(super) fn saved_death_states(
    bytes: &[u8],
) -> SaveLoadResult<std::collections::HashMap<ObjectId, SavedDeathState>> {
    let Some(payload) = decode_payload(bytes)? else {
        return Ok(Default::default());
    };
    let mut states = std::collections::HashMap::<ObjectId, SavedDeathState>::new();
    let mut started_ids = std::collections::HashSet::new();
    for entry in payload.death_start {
        let id = ObjectId(entry.object_id);
        if !started_ids.insert(id) {
            return Err(SaveLoadError::Corrupted(
                "duplicate OXFR death-start owner".into(),
            ));
        }
        states.entry(id).or_default().started = entry.on_die_started;
    }
    let mut retained_ids = std::collections::HashSet::new();
    for entry in payload.retained_death {
        let id = ObjectId(entry.object_id);
        if !retained_ids.insert(id) {
            return Err(SaveLoadError::Corrupted(
                "duplicate OXFR retained-death owner".into(),
            ));
        }
        states.entry(id).or_default().effectively_dead = entry.effectively_dead;
    }
    Ok(states)
}

/// Inspect saved reentry latches before detached native receipt validation.
pub(super) fn saved_death_started_ids(
    bytes: &[u8],
) -> SaveLoadResult<std::collections::HashSet<ObjectId>> {
    Ok(saved_death_states(bytes)?
        .into_iter()
        .filter(|(_, state)| state.started)
        .map(|(id, _)| id)
        .collect())
}

fn capture(game_logic: &GameLogic) -> ObjectModuleXferPersistPayload {
    let mut ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
    ids.sort();

    let mut stun = Vec::new();
    let mut battle_bus = Vec::new();
    let mut slow_death = Vec::new();
    let mut radar = Vec::new();
    let mut death_start = Vec::with_capacity(ids.len());
    let mut retained_death = Vec::with_capacity(ids.len());
    let mut object_creation_upgrade = Vec::new();
    let mut completed_object_upgrades = Vec::new();

    for id in ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if !object.completed_object_upgrades.is_empty() {
            let mut names: Vec<_> = object.completed_object_upgrades.iter().cloned().collect();
            names.sort();
            completed_object_upgrades.push(ObjectUpgradeCompletionPersist {
                object_id: id.0,
                names,
            });
        }
        // Exact state for every admitted object, including false and KeepObject
        // husks. A death phase or dead HP is not evidence that onDie ran.
        death_start.push(DeathStartPersist {
            object_id: id.0,
            on_die_started: object.status.on_die_started,
        });
        // Preserve both true and false rows, independently from HP and callback state.
        retained_death.push(RetainedDeathStatePersist {
            object_id: id.0,
            effectively_dead: object.status.effectively_dead,
            keep_as_rubble: object.status.keep_as_rubble,
            keep_object_die: object
                .keep_object_die
                .as_ref()
                .map(|data| KeepObjectDiePersist {
                    is_rubble: data.is_rubble,
                    rubble_frame: data.rubble_frame,
                }),
        });

        if object.shock_stun_frames > 0
            || object.shock_yaw_rate != 0.0
            || object.shock_pitch_rate != 0.0
            || object.shock_roll_rate != 0.0
            || object.shock_allow_bounce
            || object.shock_was_airborne
            || object.shock_grounded_once
        {
            stun.push(StunPersist {
                object_id: id.0,
                shock_stun_frames: object.shock_stun_frames,
                shock_yaw_rate: object.shock_yaw_rate,
                shock_pitch_rate: object.shock_pitch_rate,
                shock_roll_rate: object.shock_roll_rate,
                shock_allow_bounce: object.shock_allow_bounce,
                shock_was_airborne: object.shock_was_airborne,
                shock_grounded_once: object.shock_grounded_once,
                shock_up_z: object.shock_up_z,
            });
        }

        if object.armor_set_second_life || object.battle_bus_body.is_some() {
            battle_bus.push(BattleBusPersist {
                object_id: id.0,
                armor_set_second_life: object.armor_set_second_life,
                battle_bus_body: object.battle_bus_body.clone(),
            });
        }

        if object.slow_death.is_some()
            || object.jet_slow_death.is_some()
            || object.helicopter_slow_death.is_some()
        {
            slow_death.push(SlowDeathPersist {
                object_id: id.0,
                slow_death: object.slow_death.clone(),
                jet_slow_death: object.jet_slow_death.clone(),
                helicopter_slow_death: object.helicopter_slow_death.clone(),
            });
        }

        if !object.radar_upgrade_executed.is_empty()
            || object.radar_active
            || object.radar_extend_complete
            || object.radar_extend_done_frame != 0
        {
            radar.push(RadarPersist {
                executed: object.radar_upgrade_executed.clone(),
                contributors: object.radar_upgrade_contributors.clone(),
                object_id: id.0,
                radar_extend_done_frame: object.radar_extend_done_frame,
                radar_extend_complete: object.radar_extend_complete,
                radar_active: object.radar_active,
            });
        }

        if !object.object_creation_upgrade_executed.is_empty()
            || !object.status_bits_upgrade_executed.is_empty()
            || !object.cost_modifier_upgrade_executed.is_empty()
        {
            object_creation_upgrade.push(ObjectCreationUpgradePersist {
                object_id: id.0,
                executed: object.object_creation_upgrade_executed.clone(),
                status_bits_executed: object.status_bits_upgrade_executed.clone(),
                cost_modifier_executed: object.cost_modifier_upgrade_executed.clone(),
            });
        }
    }

    let mut particle_drive_strikes: Vec<_> = game_logic
        .special_power_strikes
        .strikes
        .values()
        .filter(|strike| {
            strike.kind == crate::game_logic::special_power_strikes::HostSuperweaponKind::ParticleCannon
        })
        .map(|strike| ParticleDriveStrikePersist {
            strike_id: strike.id,
            settings: strike.manual_drive_settings.into(),
        })
        .collect();
    particle_drive_strikes.sort_by_key(|row| row.strike_id);
    let mut particle_drive_beams: Vec<_> = game_logic
        .special_power_strikes
        .beam_fields
        .iter()
        .map(|field| ParticleDriveBeamPersist {
            beam_id: field.id,
            settings: field.manual_drive_settings.into(),
        })
        .collect();
    particle_drive_beams.sort_by_key(|row| row.beam_id);

    ObjectModuleXferPersistPayload {
        stun,
        battle_bus,
        slow_death,
        radar,
        death_start,
        retained_death,
        object_creation_upgrade,
        status_bits_executed_present: true,
        cost_modifier_executed_present: true,
        completed_object_upgrades,
        completion_snapshot_present: true,
        particle_drive_strikes,
        particle_drive_beams,
        particle_drive_snapshot_present: true,
        wire_version: OXFR_VERSION,
    }
}

fn apply_payload(
    game_logic: &mut GameLogic,
    payload: ObjectModuleXferPersistPayload,
) -> SaveLoadResult<()> {
    validate_payload_before_apply(game_logic, &payload)?;

    if payload.particle_drive_snapshot_present {
        for row in &payload.particle_drive_strikes {
            let strike = game_logic
                .special_power_strikes
                .strikes
                .get_mut(&row.strike_id)
                .expect("ParticleCannon strike was preflighted");
            strike.manual_drive_settings = row.settings.clone().into();
        }
        for row in &payload.particle_drive_beams {
            let field = game_logic
                .special_power_strikes
                .beam_fields
                .iter_mut()
                .find(|field| field.id == row.beam_id)
                .expect("Particle beam was preflighted");
            field.manual_drive_settings = row.settings.clone().into();
        }
    }

    if payload.completion_snapshot_present {
        for (_, object) in game_logic.host_objects_mut().registered_mut() {
            object.completed_object_upgrades.clear();
        }
        for row in &payload.completed_object_upgrades {
            let object = game_logic
                .host_object_mut(ObjectId(row.object_id))
                .expect("completion owner was preflighted");
            object
                .completed_object_upgrades
                .extend(row.names.iter().cloned());
        }
    }

    for entry in payload.stun {
        if let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) {
            object.shock_stun_frames = entry.shock_stun_frames;
            object.shock_yaw_rate = entry.shock_yaw_rate;
            object.shock_pitch_rate = entry.shock_pitch_rate;
            object.shock_roll_rate = entry.shock_roll_rate;
            object.shock_allow_bounce = entry.shock_allow_bounce;
            object.shock_was_airborne = entry.shock_was_airborne;
            object.shock_grounded_once = entry.shock_grounded_once;
            object.shock_up_z = entry.shock_up_z;
            apply_stun_bits(object);
        }
    }
    for entry in payload.battle_bus {
        if let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) {
            object.armor_set_second_life = entry.armor_set_second_life;
            object.battle_bus_body = entry.battle_bus_body;
            apply_second_life_bits(object);
        }
    }
    for entry in payload.slow_death {
        if let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) {
            object.slow_death = entry.slow_death;
            object.jet_slow_death = entry.jet_slow_death;
            object.helicopter_slow_death = entry.helicopter_slow_death;
        }
    }
    for entry in payload.radar {
        if let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) {
            object.radar_upgrade_executed = entry.executed;
            object.radar_upgrade_contributors = entry.contributors;
            object.radar_extend_done_frame = entry.radar_extend_done_frame;
            object.radar_extend_complete = entry.radar_extend_complete;
            object.radar_active = entry.radar_active;
            apply_radar_bits(object);
        }
    }
    for entry in payload.object_creation_upgrade {
        let object_id = ObjectId(entry.object_id);
        // Older versions lacked the newer module-family latch vectors. Rebuild
        // those defaults from this receiving world's immutable admitted INI.
        let default_status_count = if payload.status_bits_executed_present {
            0
        } else {
            let object = game_logic
                .host_object(object_id)
                .expect("preflight validated object");
            let template = game_logic
                .receiving_module_content
                .as_ref()
                .and_then(|content| content.admitted_template(&object.template_name))
                .expect("preflight validated receiving template");
            template
                .behavior_modules()
                .iter()
                .filter(|module| {
                    module
                        .name
                        .as_str()
                        .eq_ignore_ascii_case("StatusBitsUpgrade")
                })
                .count()
        };
        let default_cost_modifier_count = if payload.cost_modifier_executed_present {
            0
        } else {
            let object = game_logic
                .host_object(object_id)
                .expect("preflight validated object");
            let template = game_logic
                .receiving_module_content
                .as_ref()
                .and_then(|content| content.admitted_template(&object.template_name))
                .expect("preflight validated receiving template");
            template
                .behavior_modules()
                .iter()
                .filter(|module| {
                    module
                        .name
                        .as_str()
                        .eq_ignore_ascii_case("CostModifierUpgrade")
                })
                .count()
        };
        if let Some(object) = game_logic.host_object_mut(object_id) {
            object.object_creation_upgrade_executed = entry.executed;
            if payload.status_bits_executed_present {
                object.status_bits_upgrade_executed = entry.status_bits_executed;
            } else {
                object.status_bits_upgrade_executed = vec![false; default_status_count];
            }
            if payload.cost_modifier_executed_present {
                object.cost_modifier_upgrade_executed = entry.cost_modifier_executed;
            } else {
                object.cost_modifier_upgrade_executed = vec![false; default_cost_modifier_count];
            }
        }
    }
    for entry in payload.death_start {
        if let Some(object) = game_logic
            .host_objects_mut()
            .get_mut(&ObjectId(entry.object_id))
        {
            object.status.on_die_started = entry.on_die_started;
        }
    }
    for entry in payload.retained_death {
        if let Some(object) = game_logic
            .host_objects_mut()
            .get_mut(&ObjectId(entry.object_id))
        {
            object.status.effectively_dead = entry.effectively_dead;
            object.status.keep_as_rubble = entry.keep_as_rubble;
            match entry.keep_object_die {
                Some(saved) => {
                    let data = object.keep_object_die.get_or_insert_with(Default::default);
                    data.is_rubble = saved.is_rubble;
                    data.rubble_frame = saved.rubble_frame;
                }
                None => object.keep_object_die = None,
            }
        }
    }
    Ok(())
}

/// Check every fallible Radar/OCU restore condition before the first row in
/// this extension mutates a live object. A save made against a different or
/// missing authored module catalog is incompatible; silently dropping its
/// module latch could replay the OCL on the next upgrade update.
fn validate_payload_before_apply(
    game_logic: &GameLogic,
    payload: &ObjectModuleXferPersistPayload,
) -> SaveLoadResult<()> {
    let corrupted = |message: &str| SaveLoadError::Corrupted(message.to_owned());
    if payload.particle_drive_snapshot_present {
        let expected_strikes: std::collections::HashSet<u32> = game_logic
            .special_power_strikes
            .strikes
            .values()
            .filter(|strike| {
                strike.kind
                    == crate::game_logic::special_power_strikes::HostSuperweaponKind::ParticleCannon
            })
            .map(|strike| strike.id)
            .collect();
        let mut actual_strikes = std::collections::HashSet::new();
        for row in &payload.particle_drive_strikes {
            if !actual_strikes.insert(row.strike_id)
                || !expected_strikes.contains(&row.strike_id)
                || !row.settings.manual_driving_speed.is_finite()
                || !row.settings.manual_fast_driving_speed.is_finite()
            {
                return Err(corrupted("invalid or duplicate ParticleCannon strike settings in OXFR"));
            }
        }
        let expected_beams: std::collections::HashSet<u32> = game_logic
            .special_power_strikes
            .beam_fields
            .iter()
            .map(|field| field.id)
            .collect();
        let mut actual_beams = std::collections::HashSet::new();
        for row in &payload.particle_drive_beams {
            if !actual_beams.insert(row.beam_id)
                || !expected_beams.contains(&row.beam_id)
                || !row.settings.manual_driving_speed.is_finite()
                || !row.settings.manual_fast_driving_speed.is_finite()
            {
                return Err(corrupted("invalid or duplicate Particle beam settings in OXFR"));
            }
        }
        if actual_strikes != expected_strikes || actual_beams != expected_beams {
            return Err(corrupted("ParticleCannon drive settings are incomplete in OXFR"));
        }
    }
    if payload.completion_snapshot_present {
        let mut object_ids = std::collections::HashSet::new();
        for row in &payload.completed_object_upgrades {
            if !object_ids.insert(row.object_id) || row.names.is_empty() {
                return Err(corrupted(
                    "invalid or duplicate Object upgrade completion owner in OXFR",
                ));
            }
            let object_id = ObjectId(row.object_id);
            if game_logic.host_object(object_id).is_none() {
                return Err(corrupted(
                    "Object upgrade completion owner is absent during OXFR restore",
                ));
            }
            let mut names = std::collections::HashSet::new();
            for name in &row.names {
                if name.trim().is_empty()
                    || !names.insert(name.to_ascii_lowercase())
                    || game_logic.upgrade_template(name).is_none()
                {
                    return Err(corrupted("invalid Object upgrade completion name in OXFR"));
                }
            }
        }
    }
    let mut radar_ids = std::collections::HashSet::new();
    for entry in &payload.radar {
        if !radar_ids.insert(entry.object_id) {
            return Err(corrupted("duplicate RadarUpgrade owner in OXFR payload"));
        }
        let Some(object) = game_logic.host_object(ObjectId(entry.object_id)) else {
            // Preserve the existing Radar restore behavior for a row whose
            // Object was not recreated; only validate rows that can be applied.
            continue;
        };
        if !entry.executed.is_empty()
            && object
                .thing()
                .template
                .radar_upgrade_rules
                .as_ref()
                .is_none_or(|rules| rules.len() != entry.executed.len())
        {
            return Err(corrupted(
                "RadarUpgrade executed-state count disagrees with authored modules",
            ));
        }
        if entry.contributors.len() != entry.executed.len()
            || entry
                .contributors
                .iter()
                .enumerate()
                .any(|(index, owner)| !owner.is_empty() && !entry.executed[index])
        {
            return Err(corrupted(
                "RadarUpgrade contribution state disagrees with executed modules",
            ));
        }
    }

    let mut object_creation_ids = std::collections::HashSet::new();
    for entry in &payload.object_creation_upgrade {
        if !object_creation_ids.insert(entry.object_id) {
            return Err(corrupted(
                "duplicate ObjectCreationUpgrade owner in OXFR payload",
            ));
        }
        let object_id = ObjectId(entry.object_id);
        let object = game_logic.host_object(object_id).ok_or_else(|| {
            corrupted("ObjectCreationUpgrade owner is absent during OXFR restore")
        })?;
        let content = game_logic
            .receiving_module_content
            .as_ref()
            .ok_or_else(|| corrupted("ObjectCreationUpgrade receiving module catalog is absent"))?;
        let template = content
            .admitted_template(&object.template_name)
            .ok_or_else(|| corrupted("ObjectCreationUpgrade receiving template is absent"))?;
        use game_engine::common::thing::module::ModuleData;
        let mut authored_count = 0usize;
        for module in template.behavior_modules().iter().filter(|module| {
            module
                .name
                .as_str()
                .eq_ignore_ascii_case("ObjectCreationUpgrade")
        }) {
            if !module.data.as_any().is::<
                gamelogic::upgrade::modules::object_creation::ObjectCreationUpgradeModuleData,
            >() {
                return Err(corrupted(
                    "ObjectCreationUpgrade receiving module has an incompatible data type",
                ));
            }
            authored_count += 1;
        }
        if authored_count != entry.executed.len() {
            return Err(corrupted(
                "ObjectCreationUpgrade executed-state count disagrees with authored modules",
            ));
        }
        let mut status_count = 0usize;
        for module in template.behavior_modules().iter().filter(|module| {
            module
                .name
                .as_str()
                .eq_ignore_ascii_case("StatusBitsUpgrade")
        }) {
            if !module
                .data
                .as_any()
                .is::<gamelogic::object::upgrade::status_bits_upgrade::StatusBitsUpgradeModuleData>(
            ) {
                return Err(corrupted(
                    "StatusBitsUpgrade receiving module has an incompatible data type",
                ));
            }
            status_count += 1;
        }
        if payload.status_bits_executed_present && status_count != entry.status_bits_executed.len()
        {
            return Err(corrupted(
                "StatusBitsUpgrade executed-state count disagrees with authored modules",
            ));
        }
        let mut cost_modifier_count = 0usize;
        for module in template.behavior_modules().iter().filter(|module| {
            module
                .name
                .as_str()
                .eq_ignore_ascii_case("CostModifierUpgrade")
        }) {
            if !module.data.as_any().is::<
                gamelogic::object::upgrade::cost_modifier_upgrade::CostModifierUpgradeModuleData,
            >() {
                return Err(corrupted("CostModifierUpgrade receiving module has an incompatible data type"));
            }
            cost_modifier_count += 1;
        }
        if payload.cost_modifier_executed_present
            && cost_modifier_count != entry.cost_modifier_executed.len()
        {
            return Err(corrupted(
                "CostModifierUpgrade executed-state count disagrees with authored modules",
            ));
        }
    }
    Ok(())
}

fn apply_stun_bits(object: &mut crate::game_logic::Object) {
    use crate::game_logic::host_enum_table_residual::{
        stunned_flailing_model_bit, stunned_model_bit,
    };
    let stunned = stunned_model_bit();
    let flailing = stunned_flailing_model_bit();
    object.model_condition_bits &= !(1u128 << stunned);
    object.model_condition_bits &= !(1u128 << flailing);
    if object.shock_stun_frames > 0 {
        if object.shock_grounded_once {
            object.model_condition_bits |= 1u128 << stunned;
        } else {
            object.model_condition_bits |= 1u128 << flailing;
        }
    }
}

#[cfg(test)]
mod object_creation_upgrade_state_tests {
    use super::*;
    use crate::game_logic::Team;
    use glam::Vec3;

    fn world_with_creation_modules() -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world
            .admit_object_ini_sources([(
                "saved-creation-upgrade.ini".into(),
                "Object SavedCreationUpgradeHost\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = ObjectCreationUpgrade FirstCreation\n  TriggeredBy = Upgrade_Veterancy_VETERAN\n  UpgradeObject = None\n End\n Behavior = StatusBitsUpgrade SavedStatus\n  TriggeredBy = Upgrade_Veterancy_VETERAN\n  StatusToSet = BOOBY_TRAPPED\n End\n Behavior = ObjectCreationUpgrade SecondCreation\n  TriggeredBy = Upgrade_Veterancy_ELITE\n  UpgradeObject = None\n End\nEnd\n".into(),
            )])
            .expect("receiving OCU catalog should admit");
        let id = world
            .create_object("SavedCreationUpgradeHost", Team::USA, Vec3::ZERO)
            .expect("catalog host should create");
        (world, id)
    }

    #[test]
    fn executed_module_latches_survive_the_lifecycle_suffix_restore() {
        let (mut source, id) = world_with_creation_modules();
        source
            .host_object_mut(id)
            .unwrap()
            .object_creation_upgrade_executed = vec![true, false];
        source
            .host_object_mut(id)
            .unwrap()
            .status_bits_upgrade_executed = vec![true];

        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        let decoded = decode_payload(&bytes).unwrap().unwrap();
        assert_eq!(decoded.object_creation_upgrade.len(), 1);
        assert_eq!(decoded.object_creation_upgrade[0].executed, [true, false]);
        assert_eq!(
            decoded.object_creation_upgrade[0].status_bits_executed,
            [true]
        );

        let (mut restored, restored_id) = world_with_creation_modules();
        assert_eq!(restored_id, id);
        apply_from_lifecycle_tail(&bytes, &mut restored).unwrap();
        assert_eq!(
            restored
                .host_object(id)
                .unwrap()
                .object_creation_upgrade_executed,
            [true, false]
        );
        assert_eq!(
            restored
                .host_object(id)
                .unwrap()
                .status_bits_upgrade_executed,
            [true]
        );
    }

    #[test]
    fn invalid_later_ocu_row_is_rejected_before_earlier_stun_mutation() {
        let mut world = GameLogic::new();
        world.templates.insert(
            "OXFRPreflightOwner".into(),
            crate::game_logic::ThingTemplate::new("OXFRPreflightOwner"),
        );
        let id = world
            .create_object("OXFRPreflightOwner", Team::USA, Vec3::ZERO)
            .unwrap();
        world.host_object_mut(id).unwrap().shock_stun_frames = 3;

        let mut payload = ObjectModuleXferPersistPayload::default();
        payload.stun.push(StunPersist {
            object_id: id.0,
            shock_stun_frames: 99,
            shock_yaw_rate: 1.0,
            shock_pitch_rate: 0.0,
            shock_roll_rate: 0.0,
            shock_allow_bounce: false,
            shock_was_airborne: false,
            shock_grounded_once: false,
            shock_up_z: 1.0,
        });
        payload
            .object_creation_upgrade
            .push(ObjectCreationUpgradePersist {
                object_id: id.0 + 100,
                executed: vec![true],
                status_bits_executed: Vec::new(),
                cost_modifier_executed: Vec::new(),
            });

        assert!(apply_payload(&mut world, payload).is_err());
        assert_eq!(world.host_object(id).unwrap().shock_stun_frames, 3);
    }

    #[test]
    fn duplicate_ocu_owner_rows_are_rejected_before_mutation() {
        let (mut world, id) = world_with_creation_modules();
        world.host_object_mut(id).unwrap().shock_stun_frames = 3;

        let mut payload = ObjectModuleXferPersistPayload::default();
        payload.stun.push(StunPersist {
            object_id: id.0,
            shock_stun_frames: 99,
            shock_yaw_rate: 1.0,
            shock_pitch_rate: 0.0,
            shock_roll_rate: 0.0,
            shock_allow_bounce: false,
            shock_was_airborne: false,
            shock_grounded_once: false,
            shock_up_z: 1.0,
        });
        payload.object_creation_upgrade = vec![
            ObjectCreationUpgradePersist {
                object_id: id.0,
                executed: vec![true, false],
                status_bits_executed: Vec::new(),
                cost_modifier_executed: Vec::new(),
            },
            ObjectCreationUpgradePersist {
                object_id: id.0,
                executed: vec![false, true],
                status_bits_executed: Vec::new(),
                cost_modifier_executed: Vec::new(),
            },
        ];

        assert!(apply_payload(&mut world, payload).is_err());
        assert_eq!(world.host_object(id).unwrap().shock_stun_frames, 3);
    }

    #[test]
    fn oxfr_v4_payloads_still_decode_without_creation_upgrade_rows() {
        let old = ObjectModuleXferPersistPayloadV4::default();
        let encoded = bincode_legacy::serialize(&old).unwrap();
        let mut bytes = OXFR_MAGIC.to_vec();
        append_u32(&mut bytes, 4);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        let decoded = decode_payload(&bytes).unwrap().unwrap();
        assert!(decoded.object_creation_upgrade.is_empty());
    }
}

#[cfg(test)]
mod cost_modifier_v6_compat_tests {
    use super::*;

    #[test]
    fn v6_positional_payload_decodes_without_cost_modifier_latches() {
        let old = ObjectModuleXferPersistPayloadV6 {
            object_creation_upgrade: vec![ObjectCreationUpgradePersistV6 {
                object_id: 41,
                executed: vec![true, false],
                status_bits_executed: vec![false],
            }],
            status_bits_executed_present: true,
            ..Default::default()
        };
        let payload_bytes = bincode_legacy::serialize(&old).unwrap();
        let mut bytes = OXFR_MAGIC.to_vec();
        append_u32(&mut bytes, 6);
        append_u32(&mut bytes, payload_bytes.len() as u32);
        bytes.extend_from_slice(&payload_bytes);

        let decoded = decode_payload(&bytes).unwrap().unwrap();
        assert!(decoded.status_bits_executed_present);
        assert!(!decoded.cost_modifier_executed_present);
        assert_eq!(decoded.object_creation_upgrade[0].executed, [true, false]);
        assert_eq!(
            decoded.object_creation_upgrade[0].cost_modifier_executed,
            []
        );
    }
}

fn apply_second_life_bits(object: &mut crate::game_logic::Object) {
    use crate::game_logic::host_enum_table_residual::second_life_model_bit;
    let bit = second_life_model_bit();
    if object.armor_set_second_life
        || object
            .battle_bus_body
            .as_ref()
            .is_some_and(|body| body.is_second_life)
    {
        object.model_condition_bits |= 1u128 << bit;
    } else {
        object.model_condition_bits &= !(1u128 << bit);
    }
}

fn apply_radar_bits(object: &mut crate::game_logic::Object) {
    use crate::game_logic::host_enum_table_residual::{
        radar_extending_model_bit, radar_upgraded_model_bit,
    };
    let extending = radar_extending_model_bit();
    let upgraded = radar_upgraded_model_bit();
    object.model_condition_bits &= !(1u128 << extending);
    object.model_condition_bits &= !(1u128 << upgraded);
    if object.radar_extend_complete {
        object.model_condition_bits |= 1u128 << upgraded;
    } else if object.radar_extend_done_frame != 0 {
        object.model_condition_bits |= 1u128 << extending;
    }
    object.refresh_model_condition_bits();
}

fn find_oxfr_suffix(bytes: &[u8]) -> Option<&[u8]> {
    let (start, end) = completion_record_range(bytes).ok().flatten()?;
    Some(&bytes[start + OXFR_MAGIC.len()..end])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("OXFR u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_battle_bus::HostBattleBusBodyData;
    use crate::game_logic::host_enum_table_residual::{
        radar_extending_model_bit, radar_upgraded_model_bit, second_life_model_bit,
        stunned_flailing_model_bit,
    };
    use crate::game_logic::host_slow_death::{HostSlowDeathData, HostSlowDeathPhase};
    use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_oxfr_suffix(b"no-magic-here").is_none());
        let mut logic = GameLogic::new();
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
    }

    #[test]
    fn snapshot_round_trips_stun_bus_death_radar() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "GLAVehicleBattleBus".to_string(),
            ThingTemplate::new("GLAVehicleBattleBus"),
        );
        source.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        source.templates.insert(
            "AmericaRadarVan".to_string(),
            ThingTemplate::new("AmericaRadarVan"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        source.add_player(Player::new(1, Team::GLA, "GLA", false));

        let stunned = source
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("stunned");
        let bus = source
            .create_object("GLAVehicleBattleBus", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
            .expect("bus");
        let dying = source
            .create_object(
                "AmericaInfantryRanger",
                Team::USA,
                Vec3::new(40.0, 0.0, 0.0),
            )
            .expect("dying");
        let dish = source
            .create_object("AmericaRadarVan", Team::USA, Vec3::new(60.0, 0.0, 0.0))
            .expect("dish");

        {
            let object = source.host_object_mut(stunned).expect("stunned");
            object.shock_stun_frames = 24;
            object.shock_yaw_rate = 0.4;
            object.shock_pitch_rate = 0.2;
            object.shock_roll_rate = 0.1;
            object.shock_allow_bounce = true;
            object.shock_was_airborne = true;
            object.shock_grounded_once = false;
        }
        {
            let object = source.host_object_mut(bus).expect("bus");
            object.armor_set_second_life = true;
            let mut body = HostBattleBusBodyData::new();
            body.begin_first_life_undeath(12);
            object.battle_bus_body = Some(body);
        }
        {
            let object = source.host_object_mut(dying).expect("dying");
            let mut death = HostSlowDeathData::default();
            death.phase = HostSlowDeathPhase::Sinking;
            death.begin_frame = 5;
            death.sink_at_frame = 10;
            death.destroy_at_frame = 40;
            death.sink_offset = -2.0;
            object.slow_death = Some(death);
        }
        {
            let object = source.host_object_mut(dish).expect("dish");
            object.extend_radar(90);
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_oxfr_suffix(&snapshot.lifecycle_tail).is_some(),
            "OXFR suffix must be appended to lifecycle tail"
        );

        let mut loaded = GameLogic::new();
        loaded.templates = source.templates.clone();
        loaded.add_player(Player::new(0, Team::USA, "USA", true));
        loaded.add_player(Player::new(1, Team::GLA, "GLA", false));
        builder
            .restore_from_snapshot(&snapshot, &mut loaded)
            .expect("restore");

        let loaded_stun = loaded.host_object(stunned).expect("stunned");
        assert_eq!(loaded_stun.shock_stun_frames, 24);
        assert!((loaded_stun.shock_yaw_rate - 0.4).abs() < 1e-5);
        assert!((loaded_stun.shock_pitch_rate - 0.2).abs() < 1e-5);
        assert!((loaded_stun.shock_roll_rate - 0.1).abs() < 1e-5);
        assert!(loaded_stun.shock_allow_bounce);
        assert!(loaded_stun.shock_was_airborne);
        assert_ne!(
            loaded_stun.model_condition_bits & (1u128 << stunned_flailing_model_bit()),
            0
        );

        let loaded_bus = loaded.host_object(bus).expect("bus");
        assert!(loaded_bus.armor_set_second_life);
        let body = loaded_bus.battle_bus_body.as_ref().expect("body");
        assert!(body.is_second_life);
        assert!(body.is_in_first_death);
        assert_eq!(body.ground_check_frame, 22);
        assert_ne!(
            loaded_bus.model_condition_bits & (1u128 << second_life_model_bit()),
            0
        );

        let loaded_death = loaded.host_object(dying).expect("dying");
        let death = loaded_death.slow_death.as_ref().expect("slow death");
        assert_eq!(death.phase, HostSlowDeathPhase::Sinking);
        assert_eq!(death.destroy_at_frame, 40);
        assert!((death.sink_offset + 2.0).abs() < 1e-5);

        let loaded_dish = loaded.host_object(dish).expect("dish");
        assert!(loaded_dish.radar_active);
        assert!(!loaded_dish.radar_extend_complete);
        assert_eq!(loaded_dish.radar_extend_done_frame, 90);
        assert_ne!(
            loaded_dish.model_condition_bits & (1u128 << radar_extending_model_bit()),
            0
        );
        assert_eq!(
            loaded_dish.model_condition_bits & (1u128 << radar_upgraded_model_bit()),
            0
        );
    }
}

#[cfg(test)]
mod death_start_latch_tests;

#[cfg(test)]
mod retained_keepobject_tests;

#[cfg(test)]
mod retained_schema_tests;
