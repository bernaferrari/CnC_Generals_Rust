//! Persist native-indexed weapon conditions and RiderChange continuation.
//! WSFL v3 stores the complete bitset; live weapon instances are separate.
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const WSFL_MAGIC: &[u8; 4] = b"WSFL";
const WSFL_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WeaponSetPersistPayload {
    objects: Vec<ObjectWeaponSetPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectWeaponSetPersist {
    object_id: u32,
    weapon_condition_flags: u32,
    weapon_model_condition_bits: u128,
    armor_crate_upgrade: u8,
    weapon_bonus_veteran: bool,
    weapon_bonus_elite: bool,
    weapon_bonus_hero: bool,
    weapon_bonus_player_upgrade: bool,
    rider_change_weapon_set: Option<String>,
    rider_change_active_slot: Option<u8>,
    rider_change_model_condition_mask: u128,
    rider_change_object_status_mask: u64,
    rider_change_locomotor_set: Option<String>,
    rider_change_locomotor_name: Option<String>,
    combat_cycle_rider: u8,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(WSFL_MAGIC);
    append_u32(bytes, WSFL_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(payload) = decode_payload(bytes)? else {
        return Ok(());
    };
    apply_payload(game_logic, payload);
    Ok(())
}

pub(super) fn validate_lifecycle_tail(bytes: &[u8]) -> SaveLoadResult<()> {
    let _ = decode_payload(bytes)?;
    Ok(())
}

fn decode_payload(bytes: &[u8]) -> SaveLoadResult<Option<WeaponSetPersistPayload>> {
    let Some(record) = find_wsfl_record(bytes)? else {
        return Ok(None);
    };
    if record.len() < 12 {
        return Err(SaveLoadError::Corrupted(
            "WSFL header truncated".to_string(),
        ));
    }
    let mut rest = &record[4..];
    let version = take_u32(&mut rest)?;
    if version != WSFL_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown WSFL suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "WSFL payload truncated".to_string(),
        ));
    }
    let payload: WeaponSetPersistPayload = bincode_legacy::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("WSFL payload decode: {err}")))?;
    Ok(Some(payload))
}

fn capture(game_logic: &GameLogic) -> WeaponSetPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        objects.push(ObjectWeaponSetPersist {
            object_id: id.0,
            weapon_condition_flags: object.weapon_condition_flags(),
            weapon_model_condition_bits: object.model_condition_bits,
            armor_crate_upgrade: object.armor_crate_upgrade,
            weapon_bonus_veteran: object.weapon_bonus_veteran,
            weapon_bonus_elite: object.weapon_bonus_elite,
            weapon_bonus_hero: object.weapon_bonus_hero,
            weapon_bonus_player_upgrade: object.weapon_bonus_player_upgrade,
            rider_change_weapon_set: object.rider_change_weapon_set.clone(),
            rider_change_active_slot: object.rider_change_active_slot,
            rider_change_model_condition_mask: object.rider_change_model_condition_mask,
            rider_change_object_status_mask: object.rider_change_object_status_mask,
            rider_change_locomotor_set: object.rider_change_locomotor_set.clone(),
            rider_change_locomotor_name: object.rider_change_locomotor_name.clone(),
            combat_cycle_rider: object.combat_cycle_rider,
        });
    }
    WeaponSetPersistPayload { objects }
}

fn apply_payload(game_logic: &mut GameLogic, payload: WeaponSetPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.restore_weapon_condition_flags(
            entry.weapon_condition_flags,
            entry.weapon_model_condition_bits,
            entry.rider_change_model_condition_mask,
        );
        object.armor_crate_upgrade = entry.armor_crate_upgrade;
        object.weapon_bonus_veteran = entry.weapon_bonus_veteran;
        object.weapon_bonus_elite = entry.weapon_bonus_elite;
        object.weapon_bonus_hero = entry.weapon_bonus_hero;
        object.weapon_bonus_player_upgrade = entry.weapon_bonus_player_upgrade;
        object.rider_change_weapon_set = entry.rider_change_weapon_set;
        object.rider_change_active_slot = entry.rider_change_active_slot;
        object.rider_change_model_condition_mask = entry.rider_change_model_condition_mask;
        object.rider_change_object_status_mask = entry.rider_change_object_status_mask;
        object.rider_change_locomotor_set = entry.rider_change_locomotor_set;
        object.rider_change_locomotor_name = entry.rider_change_locomotor_name;
        object.combat_cycle_rider = entry.combat_cycle_rider;

        object.applied_upgrades.remove("ARMORSET_CRATEUPGRADE_ONE");
        object.applied_upgrades.remove("ARMORSET_CRATEUPGRADE_TWO");
        match entry.armor_crate_upgrade {
            0 => {}
            1 => {
                object
                    .applied_upgrades
                    .insert("ARMORSET_CRATEUPGRADE_ONE".to_string());
            }
            _ => {
                object
                    .applied_upgrades
                    .insert("ARMORSET_CRATEUPGRADE_TWO".to_string());
            }
        }

        object.validate_armor_and_damage_fx();
    }
}


fn find_wsfl_record(bytes: &[u8]) -> SaveLoadResult<Option<&[u8]>> {
    let Some(range) = super::hotkey_squad_persist::find_lifecycle_record(bytes, WSFL_MAGIC)? else {
        return Ok(None);
    };
    Ok(Some(&bytes[range]))
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("WSFL u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team, ThingTemplate, VeterancyLevel};
    use glam::Vec3;

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_wsfl_record(b"no-magic-here").unwrap().is_none());
        apply_from_lifecycle_tail(b"no-magic-here", &mut GameLogic::new()).expect("apply");
    }

    #[test]
    fn snapshot_round_trips_crate_and_veterancy_weapon_set_flags() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "GLATechnical".to_string(),
            ThingTemplate::new("GLATechnical"),
        );
        source.add_player(Player::new(0, Team::GLA, "GLA", true));
        let tech_id = source
            .create_object("GLATechnical", Team::GLA, Vec3::new(12.0, 0.0, 8.0))
            .expect("technical");
        {
            let tech = source.host_object_mut(tech_id).expect("technical");
            tech.apply_salvage_weapon_upgrade(0);
            tech.set_weapon_condition_at_frame(5, true, 0);
            tech.apply_salvage_armor_upgrade();
            tech.experience.level = VeterancyLevel::Elite;
            tech.set_weapon_condition_at_frame(1, true, 0);
            tech.weapon_bonus_elite = true;
            tech.set_weapon_condition_at_frame(3, true, 0);
            tech.weapon_bonus_player_upgrade = true;
            tech.set_weapon_condition_at_frame(7, true, 0);
            tech.set_weapon_condition_at_frame(6, true, 0);
            tech.set_weapon_condition_at_frame(8, true, 0);
            tech.set_rider_weapon_condition_at_frame(Some("WEAPON_RIDER2"), 0);
            let rider1 = crate::game_logic::host_enum_table_residual::model_condition_bit_name_index("RIDER1").unwrap();
            tech.rider_change_model_condition_mask = 1u128 << rider1;
            tech.model_condition_bits |= 1u128 << rider1;
            tech.rider_change_active_slot = Some(2);
            tech.combat_cycle_rider = 2;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_wsfl_record(&snapshot.lifecycle_tail)
                .expect("WSFL framing")
                .is_some(),
            "WSFL suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.add_player(Player::new(0, Team::GLA, "GLA", true));
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.host_object(tech_id).expect("restored technical");
        assert_eq!(loaded.weapon_crate_upgrade(), 2);
        assert!(loaded.test_weapon_condition(4));
        assert!(loaded.test_weapon_condition(5));
        assert_eq!(
            loaded.armor_crate_upgrade, 1,
            "ARMORSET_CRATEUPGRADE_ONE must survive load"
        );
        assert!(loaded.test_weapon_condition(1), "WEAPONSET_ELITE must survive load");
        assert!(
            loaded.weapon_bonus_elite,
            "WEAPONBONUSCONDITION_ELITE must survive load"
        );
        assert!(!loaded.test_weapon_condition(0));
        assert!(!loaded.test_weapon_condition(2));
        assert!(loaded.test_weapon_condition(3));
        assert!(loaded.weapon_bonus_player_upgrade);
        assert!(loaded.test_weapon_condition(7));
        assert!(loaded.test_weapon_condition(6));
        assert!(loaded.test_weapon_condition(8));
        assert!(loaded.test_weapon_condition(10));
        assert_ne!(loaded.model_condition_bits & loaded.rider_change_model_condition_mask, 0);
        assert_eq!(loaded.rider_change_weapon_set.as_deref(), Some("WEAPON_RIDER2"));
        assert_eq!(loaded.rider_change_active_slot, Some(2));
        assert_eq!(loaded.combat_cycle_rider, 2);
        assert_eq!(loaded.experience.level, VeterancyLevel::Elite);
    }
}
