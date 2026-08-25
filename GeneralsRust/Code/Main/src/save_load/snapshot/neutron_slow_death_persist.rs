//! Persist leftover NeutronMissileSlowDeath blast sequence.
//!
//! C++ `NeutronMissileSlowDeathBehavior::xfer` v1 writes SlowDeath base,
//! `m_activationFrame`, `MAX_NEUTRON_BLASTS` size byte, `m_completedBlasts[]`,
//! `m_completedScorchBlasts[]`, and `m_scorchPlaced`. Leftover
//! `neutron_missile_slow_death_update.rs` already matches that table. Live
//! runs the sequence as `HostNeutronMissileSlowDeathData` in
//! `special_power_strikes` — those fields were live-only, so a mid-detonation
//! save re-fired every blast/scorch wave.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes completed-blast clocks only; it never calls `begin`.

use crate::game_logic::GameLogic;
use crate::game_logic::host_neutron_missile_slow_death::HostNeutronMissileSlowDeathData;
use crate::game_logic::special_power_strikes::HostNeutronSlowDeathMeta;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const NMSD_MAGIC: &[u8; 4] = b"NMSD";
const NMSD_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct NeutronSlowDeathPersistPayload {
    next_id: u32,
    spawned_total: u32,
    fields: Vec<HostNeutronMissileSlowDeathData>,
    metas: Vec<HostNeutronSlowDeathMeta>,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.fields.is_empty() && payload.spawned_total == 0 {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(NMSD_MAGIC);
    append_u32(bytes, NMSD_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    game_logic
        .special_power_strikes_mut()
        .restore_neutron_slow_death_persist(0, 0, Vec::new(), Vec::new());
    let Some(suffix) = find_nmsd_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != NMSD_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown NMSD suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "NMSD payload truncated".to_string(),
        ));
    }
    let payload: NeutronSlowDeathPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("NMSD payload decode: {err}")))?;
    game_logic
        .special_power_strikes_mut()
        .restore_neutron_slow_death_persist(
            payload.next_id,
            payload.spawned_total,
            payload.fields,
            payload.metas,
        );
    Ok(())
}

fn capture(game_logic: &GameLogic) -> NeutronSlowDeathPersistPayload {
    let reg = game_logic.special_power_strikes();
    NeutronSlowDeathPersistPayload {
        next_id: reg.neutron_slow_death_next_id(),
        spawned_total: reg.neutron_slow_death_spawned_total(),
        fields: reg.neutron_slow_death_fields().to_vec(),
        metas: reg.neutron_slow_death_meta().to_vec(),
    }
}

fn find_nmsd_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == NMSD_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("NMSD u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{ObjectId, Team};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_neutron_completed_blasts() {
        let mut source = GameLogic::new();
        source
            .special_power_strikes_mut()
            .spawn_neutron_slow_death_field(
                ObjectId(4),
                Team::USA,
                Vec3::new(10.0, 0.0, 8.0),
                20,
                1,
            );
        {
            let fields = source
                .special_power_strikes_mut()
                .neutron_slow_death_fields_mut_for_tick();
            let mut fields = fields;
            fields[0].completed_blasts[0] = true;
            fields[0].completed_scorch[0] = true;
            fields[0].scorch_placed = true;
            fields[0].radiation_ocl_spawned = true;
            let metas = source
                .special_power_strikes()
                .neutron_slow_death_meta()
                .to_vec();
            source
                .special_power_strikes_mut()
                .restore_neutron_slow_death_fields(fields, metas);
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_nmsd_suffix(&snapshot.lifecycle_tail).is_some(),
            "NMSD suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        {
            let fields = restored.special_power_strikes().neutron_slow_death_fields();
            assert_eq!(fields.len(), 1);
            assert!(fields[0].completed_blasts[0]);
            assert!(fields[0].completed_scorch[0]);
            assert!(fields[0].scorch_placed);
            assert!(fields[0].radiation_ocl_spawned);
            assert_eq!(fields[0].activation_frame, 20);
            assert!(!fields[0].done);
        }
        assert_eq!(
            restored
                .special_power_strikes()
                .neutron_slow_death_next_id(),
            source.special_power_strikes().neutron_slow_death_next_id()
        );
    }

    #[test]
    fn absent_suffix_clears_stale_neutron() {
        let mut logic = GameLogic::new();
        logic
            .special_power_strikes_mut()
            .spawn_neutron_slow_death_field(ObjectId(4), Team::USA, Vec3::ZERO, 0, 1);
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert_eq!(
            logic
                .special_power_strikes()
                .neutron_slow_death_field_count(),
            0
        );
    }
}
