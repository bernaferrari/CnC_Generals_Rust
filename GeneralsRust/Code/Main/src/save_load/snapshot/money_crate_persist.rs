//! Persist live money / salvage crate expire + pickup registry.
//!
//! C++ `DeletionUpdate::xfer` (`DeletionUpdate.cpp:101-114`) writes v1
//! UpdateModule base plus `m_dieFrame`. Crate objects carry DeletionUpdate
//! and CrateCollide; leftover `DeletionUpdate::xfer` already writes
//! `delete_frame`. Live crates are host objects plus
//! `GameLogic.host_money_crates` (expires_frame + collide kind). Spawn
//! registers then `arm_default_deletion`; pickup iterates `ids()`. The
//! registry was live-only — a mid-crate save left the drawable but empty
//! registry, so units could not pick it up and it never expired.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.

use crate::game_logic::GameLogic;
use crate::game_logic::host_money_crate::HostMoneyCrateRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const MCRT_MAGIC: &[u8; 4] = b"MCRT";
const MCRT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MoneyCratePersistPayload {
    registry: HostMoneyCrateRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.crate_count() == 0 && payload.registry.pickups == 0 {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(MCRT_MAGIC);
    append_u32(bytes, MCRT_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep pre-load crate ids.
    game_logic.host_money_crates.clear();
    let Some(suffix) = find_mcrt_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != MCRT_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown MCRT suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "MCRT payload truncated".to_string(),
        ));
    }
    let payload: MoneyCratePersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("MCRT payload decode: {err}")))?;
    game_logic.host_money_crates = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> MoneyCratePersistPayload {
    MoneyCratePersistPayload {
        registry: game_logic.host_money_crates.clone(),
    }
}

fn find_mcrt_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == MCRT_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("MCRT u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;
    use crate::game_logic::host_money_crate::MONEY_CRATE_DELETION_MIN_FRAMES;

    #[test]
    fn snapshot_round_trips_money_crate_expire_and_pickup() {
        let mut source = GameLogic::new();
        let money_id = ObjectId(42);
        let salvage_id = ObjectId(43);
        source
            .host_money_crates
            .register_supply_drop_crate(money_id);
        source
            .host_money_crates
            .arm_default_deletion(money_id, 10, 0);
        source
            .host_money_crates
            .register_salvage_crate(salvage_id, 50);
        source
            .host_money_crates
            .arm_default_deletion(salvage_id, 20, 0);
        source.host_money_crates.pickups = 2;

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_mcrt_suffix(&snapshot.lifecycle_tail).is_some(),
            "MCRT suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored
            .host_money_crates
            .register_supply_drop_crate(ObjectId(99));
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let reg = restored.host_money_crates();
        assert_eq!(reg.crate_count(), 2);
        assert_eq!(reg.pickups, 2);
        let mut ids = reg.ids();
        ids.sort_by_key(|id| id.0);
        assert_eq!(ids, vec![money_id, salvage_id]);
        let money = reg.get(money_id).expect("money crate");
        assert_eq!(money.expires_frame, 10 + MONEY_CRATE_DELETION_MIN_FRAMES);
        assert!(!money.is_salvage);
        let salvage = reg.get(salvage_id).expect("salvage crate");
        assert!(salvage.is_salvage);
        assert_eq!(salvage.money_provided, 50);
        assert!(salvage.expires_frame > 20);
        assert!(reg.contains(money_id));
        assert!(
            reg.expired_ids(10 + MONEY_CRATE_DELETION_MIN_FRAMES)
                .contains(&money_id)
        );
    }

    #[test]
    fn absent_suffix_clears_stale_crates() {
        let mut logic = GameLogic::new();
        logic
            .host_money_crates
            .register_supply_drop_crate(ObjectId(7));
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert_eq!(logic.host_money_crates.crate_count(), 0);
    }
}
