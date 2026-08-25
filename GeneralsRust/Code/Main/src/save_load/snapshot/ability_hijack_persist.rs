//! Persist mid-channel SpecialAbilityUpdate + hijacker-in-vehicle linkage.
//!
//! C++ `SpecialAbilityUpdate::xfer` (`SpecialAbilityUpdate.cpp:1987-2048`) writes
//! `m_active` / `m_prepFrames` / `m_animFrames` / `m_targetID` / packing state so
//! a Ranger still raising the flag or Burton still planting C4 continues after
//! load. C++ `HijackerUpdate::xfer` (`HijackerUpdate.cpp:196-221`) writes
//! `m_targetID` / `m_ejectPos` / `m_update` / `m_isInVehicle` /
//! `m_wasTargetAirborne` so the hidden hijacker stays bound to the stolen
//! vehicle.
//!
//! Live `ObjectSnapshot` only stores the HackerDisable channel. Capture
//! (`capture_channel`), C4 unpack (`charge_plant_unpack_remaining_seconds` +
//! leftover plant channel / pending plant), and hijacker ride fields are
//! reconstructed as defaults on restore — the channel aborts or the rider
//! unbinds.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! (and after SPCD / BPPL / SUBD / HSQD / BTRY / CBPD) so older decoders
//! ignore the extra bytes. No world snapshot version bump.

use crate::game_logic::host_hero_abilities::{LeftoverSaChannel, LeftoverSaKind, LeftoverSaPhase};
use crate::game_logic::{CaptureChannelState, GameLogic, ObjectId, PendingSpecialAbility};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const SABL_MAGIC: &[u8; 4] = b"SABL";
const SABL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AbilityHijackPersistPayload {
    abilities: Vec<ObjectAbilityPersist>,
    hijackers: Vec<ObjectHijackPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectAbilityPersist {
    object_id: u32,
    capture: Option<CaptureChannelState>,
    charge_plant_unpack_remaining_seconds: Option<f32>,
    leftover: Option<LeftoverSaChannel>,
    pending_plant_kind: Option<u8>,
    pending_plant_target_id: u32,
    using_ability: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectHijackPersist {
    object_id: u32,
    hijack_vehicle_id: u32,
    hijacker_in_vehicle: bool,
    hijacker_update_active: bool,
    hijacker_was_airborne: bool,
    hijacker_eject_pos: Option<[f32; 3]>,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.abilities.is_empty() && payload.hijackers.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(SABL_MAGIC);
    append_u32(bytes, SABL_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_sabl_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != SABL_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown SABL suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "SABL payload truncated".to_string(),
        ));
    }
    let payload: AbilityHijackPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("SABL payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> AbilityHijackPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut abilities = Vec::new();
    let mut hijackers = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        let leftover = game_logic
            .hero_abilities()
            .leftover_channel(id)
            .copied()
            .filter(|channel| {
                matches!(
                    channel.kind,
                    LeftoverSaKind::PlantTimed | LeftoverSaKind::PlantRemote
                )
            });
        let (pending_plant_kind, pending_plant_target_id) =
            match game_logic.pending_special_ability(id) {
                Some(PendingSpecialAbility::PlantTimedDemoCharge { target_id }) => {
                    (Some(0), target_id.0)
                }
                Some(PendingSpecialAbility::PlantRemoteDemoCharge { target_id }) => {
                    (Some(1), target_id.0)
                }
                _ => (None, 0),
            };
        let mut charge_plant = object.charge_plant_unpack_remaining_seconds;
        if charge_plant.is_none() {
            if let Some(channel) = leftover {
                if channel.phase == LeftoverSaPhase::Unpacking {
                    charge_plant = Some(channel.remaining_seconds);
                }
            }
        }
        if object.capture_channel.is_some()
            || charge_plant.is_some()
            || leftover.is_some()
            || pending_plant_kind.is_some()
        {
            abilities.push(ObjectAbilityPersist {
                object_id: id.0,
                capture: object.capture_channel,
                charge_plant_unpack_remaining_seconds: charge_plant,
                leftover,
                pending_plant_kind,
                pending_plant_target_id,
                using_ability: object.status.using_ability,
            });
        }
        if object.hijack_vehicle_id.is_some()
            || object.hijacker_in_vehicle
            || object.hijacker_update_active
            || object.hijacker_was_airborne
            || object.hijacker_eject_pos.is_some()
        {
            hijackers.push(ObjectHijackPersist {
                object_id: id.0,
                hijack_vehicle_id: object.hijack_vehicle_id.map(|vid| vid.0).unwrap_or(0),
                hijacker_in_vehicle: object.hijacker_in_vehicle,
                hijacker_update_active: object.hijacker_update_active,
                hijacker_was_airborne: object.hijacker_was_airborne,
                hijacker_eject_pos: object.hijacker_eject_pos.map(|p| [p.x, p.y, p.z]),
            });
        }
    }
    AbilityHijackPersistPayload {
        abilities,
        hijackers,
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: AbilityHijackPersistPayload) {
    for entry in payload.abilities {
        let object_id = ObjectId(entry.object_id);
        if let Some(object) = game_logic.host_object_mut(object_id) {
            if let Some(channel) = entry.capture {
                if channel.remaining_seconds.is_finite() && channel.remaining_seconds >= 0.0 {
                    object.capture_channel = Some(channel);
                }
            }
            if let Some(remaining) = entry.charge_plant_unpack_remaining_seconds {
                if remaining.is_finite() && remaining >= 0.0 {
                    object.charge_plant_unpack_remaining_seconds = Some(remaining);
                }
            }
        }
        if let Some(kind) = entry.pending_plant_kind {
            let target_id = ObjectId(entry.pending_plant_target_id);
            let ability = match kind {
                0 => Some(PendingSpecialAbility::PlantTimedDemoCharge { target_id }),
                1 => Some(PendingSpecialAbility::PlantRemoteDemoCharge { target_id }),
                _ => None,
            };
            if let Some(ability) = ability {
                game_logic.restore_pending_special_ability(object_id, ability);
            }
        }
        if let Some(channel) = entry.leftover {
            if channel.remaining_seconds.is_finite() && channel.remaining_seconds >= 0.0 {
                game_logic
                    .hero_abilities_mut()
                    .set_leftover_channel(object_id, channel);
                if channel.phase == LeftoverSaPhase::Unpacking {
                    if let Some(object) = game_logic.host_object_mut(object_id) {
                        if object.charge_plant_unpack_remaining_seconds.is_none() {
                            object.charge_plant_unpack_remaining_seconds =
                                Some(channel.remaining_seconds);
                        }
                    }
                }
            }
        }
        if let Some(object) = game_logic.host_object_mut(object_id) {
            object.set_status_using_ability(entry.using_ability);
        }
    }

    for entry in payload.hijackers {
        let object_id = ObjectId(entry.object_id);
        let vehicle_id =
            (entry.hijack_vehicle_id != 0).then_some(ObjectId(entry.hijack_vehicle_id));
        let vehicle_alive = vehicle_id.is_some_and(|vid| {
            game_logic
                .host_object(vid)
                .is_some_and(|vehicle| vehicle.is_alive())
        });
        let Some(object) = game_logic.host_object_mut(object_id) else {
            continue;
        };
        if entry.hijacker_in_vehicle {
            if let Some(vehicle_id) = vehicle_id.filter(|_| vehicle_alive) {
                object.begin_hijacker_in_vehicle(vehicle_id);
                object.hijacker_update_active = entry.hijacker_update_active;
                object.hijacker_was_airborne = entry.hijacker_was_airborne;
                object.hijacker_eject_pos = entry
                    .hijacker_eject_pos
                    .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            }
            continue;
        }
        object.hijack_vehicle_id = vehicle_id;
        object.hijacker_in_vehicle = false;
        object.hijacker_update_active = entry.hijacker_update_active;
        object.hijacker_was_airborne = entry.hijacker_was_airborne;
        object.hijacker_eject_pos = entry
            .hijacker_eject_pos
            .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
    }
}

fn find_sabl_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == SABL_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("SABL u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_hero_abilities::{
        LeftoverSaChannel, LeftoverSaKind, LeftoverSaPhase,
    };
    use crate::game_logic::{
        AIState, CaptureChannelPhase, CaptureChannelState, Player, Team, ThingTemplate,
    };
    use glam::Vec3;

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_sabl_suffix(b"no-magic-here").is_none());
    }

    #[test]
    fn snapshot_round_trips_capture_channel_and_c4_unpack() {
        let mut source = GameLogic::new();
        let mut ranger = ThingTemplate::new("USARanger");
        ranger
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .add_kind_of(crate::game_logic::KindOf::Selectable);
        source.templates.insert("USARanger".to_string(), ranger);
        let mut burton = ThingTemplate::new("AmericaColonelBurton");
        burton
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .add_kind_of(crate::game_logic::KindOf::Selectable);
        source
            .templates
            .insert("AmericaColonelBurton".to_string(), burton);
        let mut barracks = ThingTemplate::new("AmericaBarracks");
        barracks.add_kind_of(crate::game_logic::KindOf::Structure);
        source
            .templates
            .insert("AmericaBarracks".to_string(), barracks);
        source.add_player(Player::new(0, Team::USA, "USA", true));

        let ranger_id = source
            .create_object("USARanger", Team::USA, Vec3::new(10.0, 0.0, 8.0))
            .expect("ranger");
        let burton_id = source
            .create_object("AmericaColonelBurton", Team::USA, Vec3::new(14.0, 0.0, 8.0))
            .expect("burton");
        let barracks_id = source
            .create_object("AmericaBarracks", Team::USA, Vec3::new(40.0, 0.0, 8.0))
            .expect("barracks");

        {
            let ranger = source.host_object_mut(ranger_id).expect("ranger");
            ranger.capture_channel = Some(CaptureChannelState {
                phase: CaptureChannelPhase::Preparing,
                remaining_seconds: 1.25,
            });
            ranger.set_ai_state(AIState::Capturing);
            ranger.set_status_using_ability(true);
            ranger.target = Some(barracks_id);
        }
        {
            let burton = source.host_object_mut(burton_id).expect("burton");
            burton.charge_plant_unpack_remaining_seconds = Some(2.2);
            burton.set_ai_state(AIState::SpecialAbility);
            burton.set_status_using_ability(true);
            burton.target = Some(barracks_id);
        }
        source.queue_pending_special_ability(
            burton_id,
            PendingSpecialAbility::PlantTimedDemoCharge {
                target_id: barracks_id,
            },
        );
        source.hero_abilities_mut().set_leftover_channel(
            burton_id,
            LeftoverSaChannel::new(
                LeftoverSaKind::PlantTimed,
                barracks_id,
                LeftoverSaPhase::Unpacking,
                2_200,
            ),
        );

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_sabl_suffix(&snapshot.lifecycle_tail).is_some(),
            "SABL suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded_ranger = restored.host_object(ranger_id).expect("restored ranger");
        assert_eq!(
            loaded_ranger.capture_channel,
            Some(CaptureChannelState {
                phase: CaptureChannelPhase::Preparing,
                remaining_seconds: 1.25,
            }),
            "capture_channel must survive load"
        );
        assert!(
            loaded_ranger.status.using_ability,
            "IS_USING_ABILITY must survive with the capture channel"
        );
        assert_eq!(loaded_ranger.ai_state, AIState::Capturing);

        let loaded_burton = restored.host_object(burton_id).expect("restored burton");
        assert_eq!(
            loaded_burton.charge_plant_unpack_remaining_seconds,
            Some(2.2),
            "m_animFrames charge-plant unpack must survive load"
        );
        let leftover = restored
            .hero_abilities()
            .leftover_channel(burton_id)
            .copied()
            .expect("leftover plant channel");
        assert_eq!(leftover.kind, LeftoverSaKind::PlantTimed);
        assert_eq!(leftover.phase, LeftoverSaPhase::Unpacking);
        assert!((leftover.remaining_seconds - 2.2).abs() < 1e-3);
        assert_eq!(leftover.target_id, barracks_id);
        assert_eq!(
            restored.pending_special_ability(burton_id),
            Some(PendingSpecialAbility::PlantTimedDemoCharge {
                target_id: barracks_id,
            }),
            "pending plant must remain so SpecialAbility tick continues"
        );
    }

    #[test]
    fn snapshot_round_trips_hijacker_in_vehicle_linkage() {
        let mut source = GameLogic::new();
        let mut hijacker = ThingTemplate::new("GLAHijacker");
        hijacker
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .add_kind_of(crate::game_logic::KindOf::Selectable);
        source.templates.insert("GLAHijacker".to_string(), hijacker);
        let mut tank = ThingTemplate::new("AmericaTankCrusader");
        tank.add_kind_of(crate::game_logic::KindOf::Vehicle)
            .add_kind_of(crate::game_logic::KindOf::Selectable);
        source
            .templates
            .insert("AmericaTankCrusader".to_string(), tank);
        source.add_player(Player::new(0, Team::GLA, "GLA", true));

        let hid = source
            .create_object("GLAHijacker", Team::GLA, Vec3::new(4.0, 0.0, 6.0))
            .expect("hijacker");
        let vid = source
            .create_object("AmericaTankCrusader", Team::GLA, Vec3::new(8.0, 0.0, 6.0))
            .expect("tank");
        source.host_object_mut(vid).expect("tank").apply_hijacked();
        source
            .host_object_mut(hid)
            .expect("hijacker")
            .begin_hijacker_in_vehicle(vid);
        {
            let rider = source.host_object_mut(hid).expect("hijacker");
            rider.hijacker_was_airborne = true;
            rider.hijacker_eject_pos = Some(Vec3::new(8.0, 4.0, 6.0));
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_sabl_suffix(&snapshot.lifecycle_tail).is_some(),
            "SABL suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(hid).expect("restored hijacker");
        assert_eq!(loaded.hijack_vehicle_id, Some(vid));
        assert!(loaded.hijacker_in_vehicle);
        assert!(loaded.hijacker_update_active);
        assert!(loaded.hijacker_was_airborne);
        assert_eq!(loaded.hijacker_eject_pos, Some(Vec3::new(8.0, 4.0, 6.0)));
        assert!(
            loaded.status.masked && loaded.drawable_hidden,
            "hidden rider residual must be reapplied after load"
        );
        let tank = restored.host_object(vid).expect("restored tank");
        assert!(tank.status.hijacked);
    }
}
