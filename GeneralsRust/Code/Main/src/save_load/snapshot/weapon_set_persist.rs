//! Persist C++ `Object::m_curWeaponSetFlags` / `WeaponSet::xfer` onto live host.
//!
//! C++ `Object::xfer` v4 (`Object.cpp:4393-4396`) writes `m_curWeaponSetFlags`
//! before the weapon set so load can rebind
//! `WEAPONSET_CRATEUPGRADE_ONE/TWO`, `VETERAN/ELITE/HERO`, and
//! `PLAYER_UPGRADE`. `WeaponSet::xfer` (`WeaponSet.cpp:190-215`) rewrites the
//! same flags with the template name. Live host already has
//! `weapon_crate_upgrade` / `armor_crate_upgrade` and
//! `weapon_set_veteran/elite/hero`, but `save_load/` had zero coverage —
//! restore cloned `Experience.level` so rank numbers survived while the
//! weapon-set flags stayed at template defaults until the next rank change.
//! A salvaged Technical lost the crate gun; an elite tank dropped to rookie
//! weapons.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No world snapshot version bump.

use crate::game_logic::host_enum_table_residual::{
    weaponset_crateupgrade_one_model_bit, weaponset_crateupgrade_two_model_bit,
    weaponset_elite_model_bit, weaponset_hero_model_bit, weaponset_veteran_model_bit,
};
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const WSFL_MAGIC: &[u8; 4] = b"WSFL";
const WSFL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WeaponSetPersistPayload {
    objects: Vec<ObjectWeaponSetPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectWeaponSetPersist {
    object_id: u32,
    weapon_crate_upgrade: u8,
    armor_crate_upgrade: u8,
    weapon_set_veteran: bool,
    weapon_set_elite: bool,
    weapon_set_hero: bool,
    weapon_bonus_veteran: bool,
    weapon_bonus_elite: bool,
    weapon_bonus_hero: bool,
    weapon_set_player_upgrade: bool,
    weapon_bonus_player_upgrade: bool,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(WSFL_MAGIC);
    append_u32(bytes, WSFL_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_wsfl_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
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
    let payload: WeaponSetPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("WSFL payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> WeaponSetPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if object.weapon_crate_upgrade == 0
            && object.armor_crate_upgrade == 0
            && !object.weapon_set_veteran
            && !object.weapon_set_elite
            && !object.weapon_set_hero
            && !object.weapon_bonus_veteran
            && !object.weapon_bonus_elite
            && !object.weapon_bonus_hero
            && !object.weapon_set_player_upgrade
            && !object.weapon_bonus_player_upgrade
        {
            continue;
        }
        objects.push(ObjectWeaponSetPersist {
            object_id: id.0,
            weapon_crate_upgrade: object.weapon_crate_upgrade,
            armor_crate_upgrade: object.armor_crate_upgrade,
            weapon_set_veteran: object.weapon_set_veteran,
            weapon_set_elite: object.weapon_set_elite,
            weapon_set_hero: object.weapon_set_hero,
            weapon_bonus_veteran: object.weapon_bonus_veteran,
            weapon_bonus_elite: object.weapon_bonus_elite,
            weapon_bonus_hero: object.weapon_bonus_hero,
            weapon_set_player_upgrade: object.weapon_set_player_upgrade,
            weapon_bonus_player_upgrade: object.weapon_bonus_player_upgrade,
        });
    }
    WeaponSetPersistPayload { objects }
}

fn apply_payload(game_logic: &mut GameLogic, payload: WeaponSetPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.weapon_crate_upgrade = entry.weapon_crate_upgrade;
        object.armor_crate_upgrade = entry.armor_crate_upgrade;
        object.weapon_set_veteran = entry.weapon_set_veteran;
        object.weapon_set_elite = entry.weapon_set_elite;
        object.weapon_set_hero = entry.weapon_set_hero;
        object.weapon_bonus_veteran = entry.weapon_bonus_veteran;
        object.weapon_bonus_elite = entry.weapon_bonus_elite;
        object.weapon_bonus_hero = entry.weapon_bonus_hero;
        object.weapon_set_player_upgrade = entry.weapon_set_player_upgrade;
        object.weapon_bonus_player_upgrade = entry.weapon_bonus_player_upgrade;

        object.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        object.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match entry.weapon_crate_upgrade {
            0 => {}
            1 => {
                object
                    .applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            _ => {
                object
                    .applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
        }
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

        stamp_crate_weaponset_model_bits(object);
        stamp_veterancy_weaponset_model_bits(object);
        object.validate_armor_and_damage_fx();
        object.record_host_weapon_set();
        object.record_host_ai_request();
    }
}

fn stamp_crate_weaponset_model_bits(object: &mut crate::game_logic::Object) {
    let one = weaponset_crateupgrade_one_model_bit();
    let two = weaponset_crateupgrade_two_model_bit();
    object.model_condition_bits &= !(1u128 << one);
    object.model_condition_bits &= !(1u128 << two);
    if object.weapon_crate_upgrade >= 2 {
        object.model_condition_bits |= 1u128 << two;
    } else if object.weapon_crate_upgrade == 1 {
        object.model_condition_bits |= 1u128 << one;
    }
    object.record_host_model_condition();
}

fn stamp_veterancy_weaponset_model_bits(object: &mut crate::game_logic::Object) {
    let vet_b = weaponset_veteran_model_bit();
    let elite_b = weaponset_elite_model_bit();
    let hero_b = weaponset_hero_model_bit();
    object.model_condition_bits &= !(1u128 << vet_b);
    object.model_condition_bits &= !(1u128 << elite_b);
    object.model_condition_bits &= !(1u128 << hero_b);
    if object.weapon_set_hero {
        object.model_condition_bits |= 1u128 << hero_b;
    } else if object.weapon_set_elite {
        object.model_condition_bits |= 1u128 << elite_b;
    } else if object.weapon_set_veteran {
        object.model_condition_bits |= 1u128 << vet_b;
    }
    object.record_host_model_condition();
}

fn find_wsfl_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == WSFL_MAGIC)
        .map(|idx| &bytes[idx + 4..])
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
        assert!(find_wsfl_suffix(b"no-magic-here").is_none());
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
            tech.apply_salvage_weapon_upgrade();
            tech.apply_salvage_armor_upgrade();
            tech.experience.level = VeterancyLevel::Elite;
            tech.weapon_set_elite = true;
            tech.weapon_bonus_elite = true;
            tech.weapon_set_player_upgrade = true;
            tech.weapon_bonus_player_upgrade = true;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_wsfl_suffix(&snapshot.lifecycle_tail).is_some(),
            "WSFL suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.host_object(tech_id).expect("restored technical");
        assert_eq!(
            loaded.weapon_crate_upgrade, 1,
            "WEAPONSET_CRATEUPGRADE_ONE must survive load"
        );
        assert_eq!(
            loaded.armor_crate_upgrade, 1,
            "ARMORSET_CRATEUPGRADE_ONE must survive load"
        );
        assert!(
            loaded
                .applied_upgrades
                .contains("WEAPONSET_CRATEUPGRADE_ONE"),
            "crate upgrade tag must be re-applied"
        );
        assert!(loaded.weapon_set_elite, "WEAPONSET_ELITE must survive load");
        assert!(
            loaded.weapon_bonus_elite,
            "WEAPONBONUSCONDITION_ELITE must survive load"
        );
        assert!(!loaded.weapon_set_veteran);
        assert!(!loaded.weapon_set_hero);
        assert!(loaded.weapon_set_player_upgrade);
        assert!(loaded.weapon_bonus_player_upgrade);
        assert_eq!(loaded.experience.level, VeterancyLevel::Elite);
    }
}
