//! Persist warehouse crippling heal clocks (C++ SupplyWarehouseCripplingBehavior).
//!
//! C++ `SupplyWarehouseCripplingBehavior::xfer` v1
//! (`SupplyWarehouseCripplingBehavior.cpp:174-193`) writes
//! `m_healingSupressedUntilFrame` + `m_nextHealingFrame`. Leftover
//! `supply_warehouse_crippling_behavior.rs` already matches that table. Live
//! host stores the same clocks in process-global `WAREHOUSE_CRIPPLING_STATES`
//! (plus `last_health`, the live `onDamage` stand-in). Those were session-only
//! — a mid-suppression save re-armed the full SelfHealSupression window after
//! load because the first observation below max looked like a fresh hit.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload so
//! older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore always clears the process-global map first so a load cannot leak
//! the previous session's heal cadence.

use crate::game_logic::host_supply_gather::{
    WarehouseCripplingState, restore_live_warehouse_crippling_states,
    snapshot_live_warehouse_crippling_states,
};
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const WHCR_MAGIC: &[u8; 4] = b"WHCR";
const WHCR_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WarehouseCripplingPersistPayload {
    states: Vec<WarehouseCripplingPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WarehouseCripplingPersist {
    object_id: u32,
    last_health: f32,
    healing_suppressed_until_frame: u32,
    next_healing_frame: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, _game_logic: &GameLogic) {
    let payload = capture();
    if payload.states.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(WHCR_MAGIC);
    append_u32(bytes, WHCR_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], _game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Always drop the previous session first (C++ module state is per-object).
    restore_live_warehouse_crippling_states(Vec::new());
    let Some(suffix) = find_whcr_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != WHCR_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown WHCR suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "WHCR payload truncated".to_string(),
        ));
    }
    let payload: WarehouseCripplingPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("WHCR payload decode: {err}")))?;
    apply_payload(payload);
    Ok(())
}

fn capture() -> WarehouseCripplingPersistPayload {
    WarehouseCripplingPersistPayload {
        states: snapshot_live_warehouse_crippling_states()
            .into_iter()
            .map(|(id, state)| WarehouseCripplingPersist {
                object_id: id.0,
                last_health: state.last_health,
                healing_suppressed_until_frame: state.healing_suppressed_until_frame,
                next_healing_frame: state.next_healing_frame,
            })
            .collect(),
    }
}

fn apply_payload(payload: WarehouseCripplingPersistPayload) {
    let restored = payload
        .states
        .into_iter()
        .map(|entry| {
            (
                ObjectId(entry.object_id),
                WarehouseCripplingState {
                    last_health: entry.last_health,
                    healing_suppressed_until_frame: entry.healing_suppressed_until_frame,
                    next_healing_frame: entry.next_healing_frame,
                },
            )
        })
        .collect();
    restore_live_warehouse_crippling_states(restored);
}

fn find_whcr_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == WHCR_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("WHCR u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_supply_gather::{
        reset_live_warehouse_host_state, warehouse_crippling_heal_amount,
    };

    #[test]
    fn snapshot_round_trips_mid_suppression_heal_cadence() {
        reset_live_warehouse_host_state();
        let warehouse = ObjectId(7);
        // Damage at frame 10 → suppress until 100. Save mid-window at frame 50.
        restore_live_warehouse_crippling_states(vec![(
            warehouse,
            WarehouseCripplingState {
                last_health: 200.0,
                healing_suppressed_until_frame: 100,
                next_healing_frame: 100,
            },
        )]);

        let builder = super::super::SnapshotBuilder::new();
        let source = GameLogic::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_whcr_suffix(&snapshot.lifecycle_tail).is_some(),
            "WHCR suffix must be appended to lifecycle tail"
        );

        restore_live_warehouse_crippling_states(vec![(
            ObjectId(99),
            WarehouseCripplingState {
                last_health: 1.0,
                healing_suppressed_until_frame: 1,
                next_healing_frame: 1,
            },
        )]);
        let mut restored = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let states = snapshot_live_warehouse_crippling_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].0, warehouse);
        let mut state = states[0].1;
        assert!((state.last_health - 200.0).abs() < 0.01);
        assert_eq!(state.healing_suppressed_until_frame, 100);
        assert_eq!(state.next_healing_frame, 100);

        let mid = warehouse_crippling_heal_amount(
            50,
            200.0,
            1000.0,
            state.last_health,
            &mut state.healing_suppressed_until_frame,
            &mut state.next_healing_frame,
        );
        assert_eq!(mid, 0.0);
        assert_eq!(
            state.healing_suppressed_until_frame, 100,
            "mid-suppression load must not restart SelfHealSupression"
        );
        let heal = warehouse_crippling_heal_amount(
            100,
            200.0,
            1000.0,
            state.last_health,
            &mut state.healing_suppressed_until_frame,
            &mut state.next_healing_frame,
        );
        assert!((heal - 5.0).abs() < 0.01);
        reset_live_warehouse_host_state();
    }

    #[test]
    fn absent_suffix_clears_stale_warehouse_crippling() {
        restore_live_warehouse_crippling_states(vec![(
            ObjectId(3),
            WarehouseCripplingState {
                last_health: 200.0,
                healing_suppressed_until_frame: 90,
                next_healing_frame: 90,
            },
        )]);
        let mut logic = GameLogic::new();
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert!(snapshot_live_warehouse_crippling_states().is_empty());
    }
}
