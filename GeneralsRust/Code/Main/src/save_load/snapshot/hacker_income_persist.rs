//! Persist HackInternet unpack/hack/pack + pending command.
//!
//! C++ `HackInternetAIUpdate::xfer` v1 (`HackInternetAIUpdate.cpp:187-201`)
//! writes the AIUpdateInterface state machine (UNPACKING / HACK_INTERNET /
//! PACKING + remaining frames) then `m_hasPendingCommand` +
//! `m_pendingCommand`. A Hacker mid-unpack or mid-hack cash cycle continues
//! after load; a pending move stored during pack is not dropped. Live
//! `HostHackerIncomeRegistry` was live-only — `GameLogic::reset` cleared it
//! and snapshot never rebound, so a field-hacking Hacker stopped (or
//! restarted unpack) and the cash interval reset (field 60f / IC 54f).
//!
//! Append a tagged HKIN v2 suffix after the historical lifecycle payload.
//! The outer Rust capability 32 gates the changed idle continuation schema;
//! active v1 suffixes are rejected rather than inferred or converted.

use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::GameLogic;
use crate::game_logic::host_hacker_income::HostHackerIncomeRegistry;
use crate::save_load::SaveLoadResult;
use serde::{Deserialize, Serialize};

const HKIN_MAGIC: &[u8; 4] = b"HKIN";
const HKIN_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HackerIncomePersistPayload {
    registry: HostHackerIncomeRegistry,
}

impl HackerIncomePersistPayload {
    fn is_empty(&self) -> bool {
        self.registry.deposits() == 0
            && self.registry.cash_total() == 0
            && self.registry.tracked_pack_ids().is_empty()
            && self.registry.next_deposit_keys().is_empty()
            && self.registry.field_starts == 0
            && self.registry.internet_center_auto_starts == 0
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
    bytes.extend_from_slice(HKIN_MAGIC);
    append_u32(bytes, HKIN_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn apply(
    records: &LifecycleRecords<'_>,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    // A missing suffix means no continuation. For present data, validate the
    // complete envelope before replacing the receiving registry.
    let Some(record) = records.get(HKIN_MAGIC) else {
        game_logic.hacker_income.clear();
        return Ok(());
    };
    let payload: HackerIncomePersistPayload = record.decode(HKIN_VERSION)?;
    game_logic.hacker_income = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> HackerIncomePersistPayload {
    HackerIncomePersistPayload {
        registry: game_logic.hacker_income.clone(),
    }
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;
    use crate::game_logic::host_hacker_income::{
        HACKER_CASH_INTERVAL_FRAMES, HackerInternetPhase, PendingHackerCommand,
    };
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_hacker_unpack_hack_and_pending_pack() {
        let mut source = GameLogic::new();
        let unpacking = ObjectId(4);
        let packing = ObjectId(5);
        source
            .hacker_income
            .start_hacking(unpacking, 100, 40, HACKER_CASH_INTERVAL_FRAMES);
        source
            .hacker_income
            .start_hacking(packing, 0, 0, HACKER_CASH_INTERVAL_FRAMES);
        assert!(source.hacker_income.finish_unpack_if_due(packing, 0));
        assert!(source.hacker_income.request_pack(
            packing,
            200,
            30,
            PendingHackerCommand::MoveTo(Vec3::new(8.0, 0.0, 4.0)),
        ));

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(HKIN_MAGIC)
                .is_some(),
            "HKIN suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored
            .hacker_income
            .start_hacking(ObjectId(99), 1, 10, 60);
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        assert_eq!(
            restored.hacker_income.pack_phase(unpacking),
            Some(HackerInternetPhase::Unpacking)
        );
        assert_eq!(restored.hacker_income.peek_pack_until(unpacking), Some(140));
        assert_eq!(
            restored.hacker_income.peek_next_deposit(unpacking),
            Some(201)
        );
        assert!(
            !restored.hacker_income.finish_unpack_if_due(unpacking, 139),
            "mid-unpack must continue"
        );
        assert!(restored.hacker_income.finish_unpack_if_due(unpacking, 140));

        assert_eq!(
            restored.hacker_income.pack_phase(packing),
            Some(HackerInternetPhase::Packing)
        );
        assert_eq!(
            restored.hacker_income.peek_pending_command(packing),
            Some(PendingHackerCommand::MoveTo(Vec3::new(8.0, 0.0, 4.0)))
        );
        assert!(
            restored
                .hacker_income
                .take_finished_pack(packing, 229)
                .is_none()
        );
        assert_eq!(
            restored.hacker_income.take_finished_pack(packing, 230),
            Some(PendingHackerCommand::MoveTo(Vec3::new(8.0, 0.0, 4.0)))
        );
        assert!(!restored.hacker_income.is_hacking(ObjectId(99)));
    }

    #[test]
    fn absent_suffix_clears_stale_hacker_income() {
        let mut logic = GameLogic::new();
        logic.hacker_income.start_hacking(ObjectId(3), 0, 10, 60);
        apply(&LifecycleRecords::parse(&[]).unwrap(), &mut logic).expect("apply");
        assert!(!logic.hacker_income.is_hacking(ObjectId(3)));
        assert!(logic.hacker_income.pack_phase(ObjectId(3)).is_none());
    }
}

#[cfg(test)]
mod derived_ai_idle_native_tests {
    use super::*;
    #[test]
    fn native_v2_round_trips_ai_idle_pack_and_rejects_v1_without_mutation() {
        use crate::game_logic::ObjectId;
        use crate::game_logic::host_hacker_income::PendingHackerCommand;
        let id = ObjectId(42);
        let mut source = GameLogic::new();
        source.hacker_income.start_hacking(id, 0, 0, 100);
        assert!(source.hacker_income.finish_unpack_if_due(id, 0));
        assert!(
            source
                .hacker_income
                .request_pack(id, 10, 30, PendingHackerCommand::AiIdle)
        );
        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        let mut restored = GameLogic::new();
        restored.hacker_income.start_hacking(id, 0, 0, 100);
        assert!(restored.hacker_income.finish_unpack_if_due(id, 0));
        assert!(
            restored
                .hacker_income
                .request_pack(id, 1, 30, PendingHackerCommand::Stop)
        );
        apply(&LifecycleRecords::parse(&bytes).unwrap(), &mut restored).unwrap();
        assert_eq!(
            restored.hacker_income.peek_pending_command(id),
            Some(PendingHackerCommand::AiIdle)
        );
        assert_eq!(restored.hacker_income.peek_pack_until(id), Some(40));
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(
            LifecycleRecords::parse(&bytes)
                .and_then(|records| apply(&records, &mut restored))
                .is_err()
        );
        assert_eq!(
            restored.hacker_income.peek_pending_command(id),
            Some(PendingHackerCommand::AiIdle)
        );
        assert_eq!(restored.hacker_income.peek_pack_until(id), Some(40));
    }
}
