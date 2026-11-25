//! Persist AutoDeposit schedule + capture bonus (C++ AutoDepositUpdate).
//!
//! C++ `AutoDepositUpdate::xfer` v2 (`AutoDepositUpdate.cpp:235-256`) writes
//! `m_depositOnFrame`, `m_awardInitialCaptureBonus`, and `m_initialized`.
//! Load must not re-award InitialCaptureBonus. Live host keeps those clocks on
//! `HostOilDerrickRegistry` / `HostBlackMarketRegistry`, which
//! `GameLogic::reset` clears and snapshot never rebound — a captured derrick
//! re-armed the 12s clock and could pay $1000 again; Black Markets restarted
//! the 2s cash clock.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload so
//! older decoders ignore the extra bytes. No WorldSnapshot version bump.

use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::{GameLogic, HostBlackMarketRegistry, HostOilDerrickRegistry};
use crate::save_load::SaveLoadResult;
use serde::{Deserialize, Serialize};

const ADPS_MAGIC: &[u8; 4] = b"ADPS";
const ADPS_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AutoDepositPersistPayload {
    oil_derricks: HostOilDerrickRegistry,
    black_markets: HostBlackMarketRegistry,
}

impl AutoDepositPersistPayload {
    fn is_empty(&self) -> bool {
        self.oil_derricks.next_deposit_keys().is_empty()
            && self.oil_derricks.deposits() == 0
            && self.oil_derricks.capture_bonuses() == 0
            && self.black_markets.next_deposit_keys().is_empty()
            && self.black_markets.deposits() == 0
    }
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.is_empty() {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(ADPS_MAGIC);
    append_u32(bytes, ADPS_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn apply(
    records: &LifecycleRecords<'_>,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep pre-load deposit clocks.
    game_logic.oil_derricks.clear();
    game_logic.black_markets.clear();
    let Some(record) = records.get(ADPS_MAGIC) else {
        return Ok(());
    };
    let payload: AutoDepositPersistPayload = record.decode(ADPS_VERSION)?;
    game_logic.oil_derricks = payload.oil_derricks;
    game_logic.black_markets = payload.black_markets;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> AutoDepositPersistPayload {
    AutoDepositPersistPayload {
        oil_derricks: game_logic.oil_derricks.clone(),
        black_markets: game_logic.black_markets.clone(),
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
    fn snapshot_round_trips_autodeposit_schedule_and_capture_bonus() {
        let mut source = GameLogic::new();
        let derrick = ObjectId(11);
        let market = ObjectId(22);
        source.oil_derricks.set_next_deposit(derrick, 3570);
        assert_eq!(source.oil_derricks.try_capture_bonus(derrick, 1000), 1000);
        source.oil_derricks.note_non_neutral_gain(derrick, 1);
        source.black_markets.set_next_deposit(market, 48);

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(ADPS_MAGIC)
                .is_some(),
            "ADPS suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.oil_derricks.set_next_deposit(ObjectId(99), 1);
        restored.black_markets.set_next_deposit(ObjectId(98), 1);
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        assert_eq!(restored.oil_derricks.peek_next_deposit(derrick), Some(3570));
        assert!(
            restored.oil_derricks.has_capture_bonus_awarded(derrick),
            "m_awardInitialCaptureBonus must stay spent"
        );
        assert_eq!(
            restored.oil_derricks.try_capture_bonus(derrick, 1000),
            0,
            "load must not re-award InitialCaptureBonus"
        );
        assert_eq!(restored.black_markets.peek_next_deposit(market), Some(48));
        assert_eq!(restored.oil_derricks.peek_next_deposit(ObjectId(99)), None);
        assert_eq!(restored.black_markets.peek_next_deposit(ObjectId(98)), None);
    }

    #[test]
    fn absent_suffix_clears_stale_autodeposit_schedules() {
        let mut logic = GameLogic::new();
        logic.oil_derricks.set_next_deposit(ObjectId(3), 12);
        logic.black_markets.set_next_deposit(ObjectId(4), 6);
        apply(&LifecycleRecords::parse(&[]).unwrap(), &mut logic).expect("apply");
        assert!(logic.oil_derricks.peek_next_deposit(ObjectId(3)).is_none());
        assert!(logic.black_markets.peek_next_deposit(ObjectId(4)).is_none());
    }
}
