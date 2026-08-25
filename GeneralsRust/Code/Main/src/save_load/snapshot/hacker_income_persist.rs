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
//! Append a tagged suffix after the historical v9 contain/producer payload so
//! older decoders ignore the extra bytes. No WorldSnapshot version bump.

use crate::game_logic::GameLogic;
use crate::game_logic::host_hacker_income::HostHackerIncomeRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const HKIN_MAGIC: &[u8; 4] = b"HKIN";
const HKIN_VERSION: u32 = 1;

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
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(HKIN_MAGIC);
    append_u32(bytes, HKIN_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep pre-load hack cycles.
    game_logic.hacker_income.clear();
    let Some(suffix) = find_hkin_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != HKIN_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown HKIN suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "HKIN payload truncated".to_string(),
        ));
    }
    let payload: HackerIncomePersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("HKIN payload decode: {err}")))?;
    game_logic.hacker_income = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> HackerIncomePersistPayload {
    HackerIncomePersistPayload {
        registry: game_logic.hacker_income.clone(),
    }
}

fn find_hkin_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == HKIN_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("HKIN u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
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
            find_hkin_suffix(&snapshot.lifecycle_tail).is_some(),
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
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert!(!logic.hacker_income.is_hacking(ObjectId(3)));
        assert!(logic.hacker_income.pack_phase(ObjectId(3)).is_none());
    }
}
