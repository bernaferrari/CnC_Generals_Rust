//! Persist owned construction admission and status continuation flags.
//! C++ Object::xfer preserves status independently of BuildAssistant sellList.
//! Restoring this framed suffix never calls Player or CreateModules.
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
const MAGIC: &[u8; 4] = b"CPAD";

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, world: &GameLogic) {
    let mut rows: Vec<(u32, u32)> = world
        .host_objects()
        .values()
        .filter_map(|object| {
            let flags = u32::from(object.construction_power_admitted)
                | (u32::from(object.status.sold) << 1)
                | (u32::from(object.status.reconstructing) << 2);
            (flags != 0).then_some((object.id.0, flags))
        })
        .collect();
    rows.sort_unstable_by_key(|row| row.0);
    bytes.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    for (id, flags) in &rows {
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&flags.to_le_bytes());
    }
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&(4 + rows.len() as u32 * 8).to_le_bytes());
    bytes.extend_from_slice(MAGIC);
}

/// Locate the outer, EOF-framed admission suffix without interpreting earlier
/// markers. Inner owner suffixes end exactly at this boundary.
pub(super) fn suffix_start(bytes: &[u8]) -> SaveLoadResult<usize> {
    super::combat_particle_attachment_persist::suffix_start(bytes)
}

pub(super) fn own_suffix_start(bytes: &[u8]) -> SaveLoadResult<usize> {
    let bytes = super::player_team_chunks::lifecycle_prefix(bytes)?;
    if !bytes.ends_with(MAGIC) {
        return Ok(bytes.len());
    }
    if bytes.len() < 16 {
        return Err(SaveLoadError::Corrupted("invalid CPAD footer".into()));
    }
    let footer = bytes.len() - 12;
    let length = u32::from_le_bytes(bytes[footer + 4..footer + 8].try_into().unwrap()) as usize;
    footer
        .checked_sub(length)
        .ok_or_else(|| SaveLoadError::Corrupted("invalid CPAD length".into()))
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], world: &mut GameLogic) -> SaveLoadResult<()> {
    let bytes = super::player_team_chunks::lifecycle_prefix(bytes)?;
    if !bytes.ends_with(MAGIC) {
        // Historical saves do not prove admission. Never infer it from
        // transient factory percent or invoke completion while loading.
        for object in world.host_objects_mut().values_mut() {
            object.construction_power_admitted = false;
        }
        return Ok(());
    }
    let invalid = || SaveLoadError::Corrupted("invalid CPAD construction admission footer".into());
    if bytes.len() < 16 {
        return Err(invalid());
    }
    let footer = bytes.len() - 12;
    let version = u32::from_le_bytes(bytes[footer..footer + 4].try_into().unwrap());
    let length = u32::from_le_bytes(bytes[footer + 4..footer + 8].try_into().unwrap()) as usize;
    let start = footer.checked_sub(length).ok_or_else(invalid)?;
    if !matches!(version, 1 | 2) || length < 4 {
        return Err(invalid());
    }
    let tail = &bytes[start..footer];
    let read = |offset| u32::from_le_bytes(tail[offset..offset + 4].try_into().unwrap());
    let count = read(0) as usize;
    let stride = if version == 1 { 4 } else { 8 };
    if length
        != count
            .checked_mul(stride)
            .and_then(|value| value.checked_add(4))
            .ok_or_else(invalid)?
    {
        return Err(invalid());
    }
    // Validate the complete capsule before touching any receiving Object.
    let mut rows = std::collections::BTreeMap::new();
    for index in 0..count {
        let offset = 4 + index * stride;
        let id = ObjectId(read(offset));
        let flags = if version == 1 { 1 } else { read(offset + 4) };
        if flags & !7 != 0 || world.host_object(id).is_none() || rows.insert(id, flags).is_some() {
            return Err(invalid());
        }
    }
    for object in world.host_objects_mut().values_mut() {
        let flags = rows.get(&object.id).copied().unwrap_or(0);
        object.construction_power_admitted = flags & 1 != 0;
        // Version one contained only power IDs. It never overwrites status
        // fields, preserving its exact historical interpretation.
        if version == 2 {
            object.status.sold = flags & 2 != 0;
            object.status.reconstructing = flags & 4 != 0;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;
    #[test]
    fn construction_power_admission_restores_without_player_callbacks() {
        let mut source = GameLogic::new();
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let mut template = ThingTemplate::new("InstantPower");
        template.add_kind_of(KindOf::Structure).set_health(500.0);
        source.templates.insert("InstantPower".into(), template);
        let id = source
            .create_object_for_player("InstantPower", 0, Vec3::ZERO)
            .unwrap();
        source
            .host_object_mut(id)
            .unwrap()
            .construction_power_admitted = true;
        // An unrelated payload can contain the exact marker without
        // becoming a bogus header or masking the real framed footer.
        let mut bytes = b"unrelatedCPADpayload".to_vec();
        append_to_lifecycle_tail(&mut bytes, &source);
        source
            .host_object_mut(id)
            .unwrap()
            .construction_power_admitted = false;
        apply_from_lifecycle_tail(&bytes, &mut source).unwrap();
        assert!(source.host_object(id).unwrap().construction_power_admitted);
        assert_eq!(source.script_object_count_changed_frame, 0);
        let mut bad = bytes.clone();
        let version = bad.len() - 12;
        bad[version..version + 4].copy_from_slice(&3_u32.to_le_bytes());
        assert!(apply_from_lifecycle_tail(&bad, &mut source).is_err());
        assert!(source.host_object(id).unwrap().construction_power_admitted);
        let mut oversized = bytes.clone();
        let length = oversized.len() - 8;
        oversized[length..length + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(apply_from_lifecycle_tail(&oversized, &mut source).is_err());
        assert!(source.host_object(id).unwrap().construction_power_admitted);
        let mut absent = bytes.clone();
        let object_id_offset = absent.len() - 20;
        absent[object_id_offset..object_id_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(apply_from_lifecycle_tail(&absent, &mut source).is_err());
        assert!(source.host_object(id).unwrap().construction_power_admitted);
    }
}

#[cfg(test)]
#[path = "construction_status_persist_tests.rs"]
mod status_tests;
