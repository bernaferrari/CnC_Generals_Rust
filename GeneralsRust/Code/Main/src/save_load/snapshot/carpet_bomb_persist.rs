//! Persist live CarpetBomb `pending_drops` (C++ DeliveringState).
//!
//! C++ `DeliveringState::xfer` (`DeliverPayloadAIUpdate.cpp:630-638`) writes
//! `m_dropDelayLeft` / `m_didOpen` so remaining payloads keep exiting after
//! load. Leftover `DeliverPayloadAIUpdate` has those fields but no instance
//! Snapshotable xfer. Live host schedules the remaining line on
//! `HostCarpetBombFlightRegistry.pending_drops`, which was live-only — save
//! mid-run wiped the queue and cancelled the rest of the stick.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! (and after SPCD / BPPL / SUBD / HSQD / BTRY) so older decoders ignore the
//! extra bytes. No world snapshot version bump.

use crate::game_logic::GameLogic;
use crate::game_logic::host_carpet_bomb_flight::HostCarpetBombFlightRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const CBPD_MAGIC: &[u8; 4] = b"CBPD";
const CBPD_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CarpetBombPersistPayload {
    registry: HostCarpetBombFlightRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.pending_drops.is_empty()
        && payload.registry.transports_spawned == 0
        && payload.registry.bombs_scheduled == 0
        && payload.registry.bombs_dropped == 0
        && payload.registry.impacts == 0
    {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(CBPD_MAGIC);
    append_u32(bytes, CBPD_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep pre-load remaining bombs.
    game_logic.carpet_bomb_flight_reg.clear();
    let Some(suffix) = find_cbpd_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != CBPD_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown CBPD suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "CBPD payload truncated".to_string(),
        ));
    }
    let payload: CarpetBombPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("CBPD payload decode: {err}")))?;
    game_logic.carpet_bomb_flight_reg = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> CarpetBombPersistPayload {
    CarpetBombPersistPayload {
        registry: game_logic.carpet_bomb_flight_reg.clone(),
    }
}

fn find_cbpd_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == CBPD_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("CBPD u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_carpet_bomb_flight::PendingCarpetBombDrop;
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_carpet_pending_drops() {
        let mut source = GameLogic::new();
        source.carpet_bomb_flight_reg.transports_spawned = 1;
        source.carpet_bomb_flight_reg.bombs_scheduled = 15;
        source.carpet_bomb_flight_reg.bombs_dropped = 4;
        source.carpet_bomb_flight_reg.impacts = 3;
        source.carpet_bomb_flight_reg.pending_drops = vec![
            PendingCarpetBombDrop {
                drop_frame: 80,
                target: Vec3::new(120.0, 0.0, 40.0),
                source_id: 7,
                bomb_index: 4,
                transport_id: 11,
            },
            PendingCarpetBombDrop {
                drop_frame: 89,
                target: Vec3::new(145.0, 0.0, 40.0),
                source_id: 7,
                bomb_index: 5,
                transport_id: 11,
            },
        ];

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_cbpd_suffix(&snapshot.lifecycle_tail).is_some(),
            "CBPD suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        // Stale queue must be replaced, not merged.
        restored
            .carpet_bomb_flight_reg
            .pending_drops
            .push(PendingCarpetBombDrop {
                drop_frame: 1,
                target: Vec3::ZERO,
                source_id: 0,
                bomb_index: 0,
                transport_id: 99,
            });
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let reg = &restored.carpet_bomb_flight_reg;
        assert_eq!(reg.transports_spawned, 1);
        assert_eq!(reg.bombs_scheduled, 15);
        assert_eq!(reg.bombs_dropped, 4);
        assert_eq!(reg.impacts, 3);
        assert_eq!(reg.pending_drops.len(), 2);
        assert_eq!(reg.pending_drops[0].drop_frame, 80);
        assert_eq!(reg.pending_drops[0].bomb_index, 4);
        assert_eq!(reg.pending_drops[0].transport_id, 11);
        assert_eq!(reg.pending_drops[1].drop_frame, 89);
        assert_eq!(reg.pending_drops[1].transport_id, 11);
        assert!((reg.pending_drops[0].target.x - 120.0).abs() < 0.01);
    }

    #[test]
    fn absent_suffix_clears_stale_pending_drops() {
        let mut logic = GameLogic::new();
        logic
            .carpet_bomb_flight_reg
            .pending_drops
            .push(PendingCarpetBombDrop {
                drop_frame: 10,
                target: Vec3::ZERO,
                source_id: 1,
                bomb_index: 0,
                transport_id: 2,
            });
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert!(logic.carpet_bomb_flight_reg.pending_drops.is_empty());
    }
}
