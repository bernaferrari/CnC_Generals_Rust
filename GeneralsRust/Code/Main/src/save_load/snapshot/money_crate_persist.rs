//! Persist the money/salvage crate pickup registry. Object lifecycle Xfer
//! carries authored DeletionUpdate state independently (DeletionUpdate.cpp:101–114).
//!
//! This versioned suffix follows the contain/producer payload in the world
//! snapshot lifecycle tail.

use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::host_money_crate::HostMoneyCrateRegistry;
use crate::game_logic::GameLogic;
use crate::save_load::SaveLoadResult;
use serde::{Deserialize, Serialize};

const MCRT_MAGIC: &[u8; 4] = b"MCRT";
const MCRT_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MoneyCratePersistPayload {
    registry: HostMoneyCrateRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.crate_count() == 0 && payload.registry.pickups == 0 {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(MCRT_MAGIC);
    append_u32(bytes, MCRT_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn apply(
    records: &LifecycleRecords<'_>,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep pre-load crate ids.
    game_logic.host_money_crates.clear();
    let Some(record) = records.get(MCRT_MAGIC) else {
        return Ok(());
    };
    let payload: MoneyCratePersistPayload = record.decode(MCRT_VERSION)?;
    game_logic.host_money_crates = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> MoneyCratePersistPayload {
    MoneyCratePersistPayload {
        registry: game_logic.host_money_crates.clone(),
    }
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;

    #[test]
    fn snapshot_round_trips_money_crate_pickup() {
        let mut source = GameLogic::new();
        let money_id = ObjectId(42);
        let salvage_id = ObjectId(43);
        source
            .host_money_crates
            .register_supply_drop_crate(money_id);
        source
            .host_money_crates
            .register_salvage_crate(salvage_id, 50);
        source.host_money_crates.pickups = 2;

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(MCRT_MAGIC)
                .is_some(),
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
        assert!(money.building_pickup);
        assert!(!money.is_salvage);
        let salvage = reg.get(salvage_id).expect("salvage crate");
        assert!(salvage.is_salvage);
        assert_eq!(salvage.money_provided, 50);
        assert!(reg.contains(money_id));
    }

    #[test]
    fn absent_suffix_clears_stale_crates() {
        let mut logic = GameLogic::new();
        logic
            .host_money_crates
            .register_supply_drop_crate(ObjectId(7));
        apply(&LifecycleRecords::parse(&[]).unwrap(), &mut logic).expect("apply");
        assert_eq!(logic.host_money_crates.crate_count(), 0);
    }
}
