//! Persist OCLUpdate supply-drop timer (C++ OCLUpdate).
//!
//! C++ `OCLUpdate::xfer` v1 (`OCLUpdate.cpp:261-285`) writes
//! `m_nextCreationFrame`, `m_timerStartedFrame`, `m_isFactionNeutral`, and
//! `m_currentPlayerColor`. America Supply Drop Zone crate drops keep the
//! remaining Min/MaxDelay (120s) after load. Live host stores the per-zone
//! next-drop frame on `HostSupplyDropZoneRegistry`, which `GameLogic::reset`
//! clears and snapshot never rebound — save at 1:59 of the 2:00 cycle
//! restarted a full 3600-frame wait and the ControlBar OCL timer snapped back.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload so
//! older decoders ignore the extra bytes. No WorldSnapshot version bump.

use crate::game_logic::GameLogic;
use crate::game_logic::host_supply_drop_zone::HostSupplyDropZoneRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const OCLT_MAGIC: &[u8; 4] = b"OCLT";
const OCLT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SupplyDropPersistPayload {
    registry: HostSupplyDropZoneRegistry,
}

impl SupplyDropPersistPayload {
    fn is_empty(&self) -> bool {
        self.registry.next_drop_keys().is_empty()
            && self.registry.drops() == 0
            && self.registry.flights_started() == 0
            && self.registry.cash_total() == 0
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
    bytes.extend_from_slice(OCLT_MAGIC);
    append_u32(bytes, OCLT_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep a pre-load drop clock.
    game_logic.supply_drop_zones.clear();
    let Some(suffix) = find_oclt_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != OCLT_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown OCLT suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "OCLT payload truncated".to_string(),
        ));
    }
    let payload: SupplyDropPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("OCLT payload decode: {err}")))?;
    game_logic.supply_drop_zones = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> SupplyDropPersistPayload {
    SupplyDropPersistPayload {
        registry: game_logic.supply_drop_zones.clone(),
    }
}

fn find_oclt_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == OCLT_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("OCLT u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;

    #[test]
    fn snapshot_round_trips_supply_drop_timer() {
        let mut source = GameLogic::new();
        let zone = ObjectId(7);
        // 1:59 of a 2:00 cycle: 30 frames remaining of 3600.
        source.supply_drop_zones.set_next_drop(zone, 3630);
        source.supply_drop_zones.flights_started = 2;

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_oclt_suffix(&snapshot.lifecycle_tail).is_some(),
            "OCLT suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.supply_drop_zones.set_next_drop(ObjectId(99), 1);
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        assert_eq!(restored.supply_drop_zones.peek_next_drop(zone), Some(3630));
        assert_eq!(
            restored
                .supply_drop_zones
                .remaining_ocl_timer_seconds(zone, 3600),
            1,
            "ControlBar OCL timer must keep remaining seconds"
        );
        assert_eq!(restored.supply_drop_zones.flights_started(), 2);
        assert!(
            restored
                .supply_drop_zones
                .peek_next_drop(ObjectId(99))
                .is_none()
        );
    }

    #[test]
    fn absent_suffix_clears_stale_supply_drop_timer() {
        let mut logic = GameLogic::new();
        logic.supply_drop_zones.set_next_drop(ObjectId(3), 100);
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert!(
            logic
                .supply_drop_zones
                .peek_next_drop(ObjectId(3))
                .is_none()
        );
    }
}
