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
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.

use crate::game_logic::host_battle_bus::HostBattleBusBodyData;
use crate::game_logic::host_helicopter_slow_death::HostHelicopterSlowDeathData;
use crate::game_logic::host_jet_slow_death::HostJetSlowDeathData;
use crate::game_logic::host_slow_death::HostSlowDeathData;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const OXFR_MAGIC: &[u8; 4] = b"OXFR";
const OXFR_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectModuleXferPersistPayload {
    stun: Vec<StunPersist>,
    battle_bus: Vec<BattleBusPersist>,
    slow_death: Vec<SlowDeathPersist>,
    radar: Vec<RadarPersist>,
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
    object_id: u32,
    radar_extend_done_frame: u32,
    radar_extend_complete: bool,
    radar_active: bool,
}

impl ObjectModuleXferPersistPayload {
    fn is_empty(&self) -> bool {
        self.stun.is_empty()
            && self.battle_bus.is_empty()
            && self.slow_death.is_empty()
            && self.radar.is_empty()
    }
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(OXFR_MAGIC);
    append_u32(bytes, OXFR_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_oxfr_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != OXFR_VERSION {
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
    let payload: ObjectModuleXferPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("OXFR payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> ObjectModuleXferPersistPayload {
    let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    ids.sort();

    let mut stun = Vec::new();
    let mut battle_bus = Vec::new();
    let mut slow_death = Vec::new();
    let mut radar = Vec::new();

    for id in ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };

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

        if object.radar_active
            || object.radar_extend_complete
            || object.radar_extend_done_frame != 0
        {
            radar.push(RadarPersist {
                object_id: id.0,
                radar_extend_done_frame: object.radar_extend_done_frame,
                radar_extend_complete: object.radar_extend_complete,
                radar_active: object.radar_active,
            });
        }
    }

    ObjectModuleXferPersistPayload {
        stun,
        battle_bus,
        slow_death,
        radar,
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: ObjectModuleXferPersistPayload) {
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
            object.radar_extend_done_frame = entry.radar_extend_done_frame;
            object.radar_extend_complete = entry.radar_extend_complete;
            object.radar_active = entry.radar_active;
            apply_radar_bits(object);
        }
    }
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
    object.record_host_model_condition();
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
    object.record_host_model_condition();
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
    object.record_host_model_condition();
}

fn find_oxfr_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == OXFR_MAGIC)
        .map(|idx| &bytes[idx + 4..])
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
