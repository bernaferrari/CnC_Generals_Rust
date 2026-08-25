//! Persist live Paradrop / Leaflet / SneakAttack mid-flight registries.
//!
//! C++ `DeliverPayloadAIUpdate::xfer` v5 writes approach / drop-delay / decal
//! so a cargo plane still dumps remaining payloads after load. Leftover
//! `DeliverPayloadAIUpdate` already matches that table. Live host queues the
//! same mid-flight work on `host_paradrops` / `host_leaflet_drops` /
//! `host_sneak_attacks`, which `GameLogic` construct clears and snapshot
//! never rebound — save mid-approach cancelled the drop.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.

use crate::game_logic::GameLogic;
use crate::game_logic::host_leaflet_drop::HostLeafletDropMission;
use crate::game_logic::host_paradrop::HostParadropMission;
use crate::game_logic::host_sneak_attack::{HostSneakAttackMission, PendingSneakShockwave};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const DPLS_MAGIC: &[u8; 4] = b"DPLS";
const DPLS_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DeliverPayloadPersistPayload {
    paradrop_next_id: u32,
    paradrop_missions: Vec<HostParadropMission>,
    paradrop_transports_spawned: u32,
    paradrop_parachutes_dropped: u32,
    leaflet_next_id: u32,
    leaflet_missions: Vec<HostLeafletDropMission>,
    leaflet_activation_count: u32,
    leaflet_disable_count: u32,
    leaflet_transports_spawned: u32,
    leaflet_containers_dropped: u32,
    sneak_next_id: u32,
    sneak_missions: Vec<HostSneakAttackMission>,
    sneak_activation_count: u32,
    sneak_tunnel_spawn_count: u32,
    sneak_shockwave_hit_count: u32,
    sneak_pending_shockwaves: Vec<PendingSneakShockwave>,
    sneak_multi_pulse_applies: u32,
    sneak_tunnel_starts_spawned: u32,
}

impl DeliverPayloadPersistPayload {
    fn is_empty(&self) -> bool {
        self.paradrop_missions.is_empty()
            && self.paradrop_transports_spawned == 0
            && self.paradrop_parachutes_dropped == 0
            && self.leaflet_missions.is_empty()
            && self.leaflet_activation_count == 0
            && self.leaflet_disable_count == 0
            && self.leaflet_transports_spawned == 0
            && self.leaflet_containers_dropped == 0
            && self.sneak_missions.is_empty()
            && self.sneak_activation_count == 0
            && self.sneak_tunnel_spawn_count == 0
            && self.sneak_shockwave_hit_count == 0
            && self.sneak_pending_shockwaves.is_empty()
            && self.sneak_multi_pulse_applies == 0
            && self.sneak_tunnel_starts_spawned == 0
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
    bytes.extend_from_slice(DPLS_MAGIC);
    append_u32(bytes, DPLS_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep pre-load mid-flight drops.
    game_logic.host_paradrops.clear();
    game_logic.host_leaflet_drops.clear();
    game_logic.host_sneak_attacks.clear();
    let Some(suffix) = find_dpls_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != DPLS_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown DPLS suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "DPLS payload truncated".to_string(),
        ));
    }
    let payload: DeliverPayloadPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("DPLS payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> DeliverPayloadPersistPayload {
    let paradrop = &game_logic.host_paradrops;
    let leaflet = &game_logic.host_leaflet_drops;
    let sneak = &game_logic.host_sneak_attacks;
    DeliverPayloadPersistPayload {
        paradrop_next_id: paradrop.next_id(),
        paradrop_missions: paradrop.missions_snapshot(),
        paradrop_transports_spawned: paradrop.transports_spawned,
        paradrop_parachutes_dropped: paradrop.parachutes_dropped,
        leaflet_next_id: leaflet.next_id(),
        leaflet_missions: leaflet.missions_snapshot(),
        leaflet_activation_count: leaflet.activation_count,
        leaflet_disable_count: leaflet.disable_count,
        leaflet_transports_spawned: leaflet.transports_spawned,
        leaflet_containers_dropped: leaflet.containers_dropped,
        sneak_next_id: sneak.next_id(),
        sneak_missions: sneak.missions_snapshot(),
        sneak_activation_count: sneak.activation_count,
        sneak_tunnel_spawn_count: sneak.tunnel_spawn_count,
        sneak_shockwave_hit_count: sneak.shockwave_hit_count,
        sneak_pending_shockwaves: sneak.pending_shockwaves.clone(),
        sneak_multi_pulse_applies: sneak.multi_pulse_applies,
        sneak_tunnel_starts_spawned: sneak.tunnel_starts_spawned,
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: DeliverPayloadPersistPayload) {
    game_logic
        .host_paradrops
        .restore_from_snapshot(payload.paradrop_next_id, payload.paradrop_missions);
    game_logic.host_paradrops.transports_spawned = payload.paradrop_transports_spawned;
    game_logic.host_paradrops.parachutes_dropped = payload.paradrop_parachutes_dropped;

    game_logic
        .host_leaflet_drops
        .restore_from_snapshot(payload.leaflet_next_id, payload.leaflet_missions);
    game_logic.host_leaflet_drops.activation_count = payload.leaflet_activation_count;
    game_logic.host_leaflet_drops.disable_count = payload.leaflet_disable_count;
    game_logic.host_leaflet_drops.transports_spawned = payload.leaflet_transports_spawned;
    game_logic.host_leaflet_drops.containers_dropped = payload.leaflet_containers_dropped;

    game_logic
        .host_sneak_attacks
        .restore_from_snapshot(payload.sneak_next_id, payload.sneak_missions);
    game_logic.host_sneak_attacks.activation_count = payload.sneak_activation_count;
    game_logic.host_sneak_attacks.tunnel_spawn_count = payload.sneak_tunnel_spawn_count;
    game_logic.host_sneak_attacks.shockwave_hit_count = payload.sneak_shockwave_hit_count;
    game_logic.host_sneak_attacks.pending_shockwaves = payload.sneak_pending_shockwaves;
    game_logic.host_sneak_attacks.multi_pulse_applies = payload.sneak_multi_pulse_applies;
    game_logic.host_sneak_attacks.tunnel_starts_spawned = payload.sneak_tunnel_starts_spawned;
}

fn find_dpls_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == DPLS_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("DPLS u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_leaflet_drop::HostLeafletDropKind;
    use crate::game_logic::host_paradrop::{
        HostParadropKind, HostParadropPhase, PARADROP_RESIDUAL_TEMPLATE,
    };
    use crate::game_logic::host_sneak_attack::{
        GLA_SNEAK_TUNNEL_TEMPLATE, HostSneakAttackKind, HostSneakAttackPhase,
    };
    use crate::game_logic::{GameLogic, ObjectId, Team};
    use glam::Vec3;

    #[test]
    fn absent_suffix_clears_pre_load_missions() {
        let mut logic = GameLogic::new();
        logic.host_paradrops.queue(
            HostParadropKind::AmericaParadrop,
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            0,
            PARADROP_RESIDUAL_TEMPLATE,
        );
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert_eq!(logic.host_paradrops.pending_count(), 0);
        assert_eq!(logic.host_leaflet_drops.pending_count(), 0);
        assert_eq!(logic.host_sneak_attacks.pending_count(), 0);
    }

    #[test]
    fn snapshot_round_trips_mid_flight_paradrop_leaflet_sneak() {
        let mut source = GameLogic::new();
        let drop_id = source.host_paradrops.queue(
            HostParadropKind::AmericaParadrop,
            ObjectId(9),
            Team::USA,
            Vec3::new(10.0, 0.0, 20.0),
            10,
            PARADROP_RESIDUAL_TEMPLATE,
        );
        source.host_paradrops.transports_spawned = 1;
        let leaflet_id = source.host_leaflet_drops.queue(
            HostLeafletDropKind::UsaLeafletDrop,
            ObjectId(8),
            Team::USA,
            Vec3::new(30.0, 0.0, 40.0),
            20,
        );
        source.host_leaflet_drops.transports_spawned = 1;
        let sneak_id = source.host_sneak_attacks.queue(
            HostSneakAttackKind::GLASneakAttack,
            ObjectId(7),
            Team::GLA,
            Vec3::new(50.0, 0.0, 60.0),
            30,
            GLA_SNEAK_TUNNEL_TEMPLATE,
        );
        source
            .host_sneak_attacks
            .pending_shockwaves
            .push(PendingSneakShockwave {
                mission_id: sneak_id,
                source_object: ObjectId(7),
                source_team: Team::GLA,
                source_owner_player_id: None,
                target_position: Vec3::new(50.0, 0.0, 60.0),
                apply_frame: 31,
                damage: 10.0,
                radius: 35.0,
                weapon_name: "SneakAttackShockwaveWeaponSmall".to_string(),
                pulse_index: 0,
            });

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_dpls_suffix(&snapshot.lifecycle_tail).is_some(),
            "DPLS suffix must be appended to lifecycle tail"
        );

        let mut loaded = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut loaded)
            .expect("restore");

        assert_eq!(loaded.host_paradrops.pending_count(), 1);
        let drop = loaded.host_paradrops.get(drop_id).expect("paradrop");
        assert_eq!(drop.phase, HostParadropPhase::Queued);
        assert_eq!(
            drop.drop_frame,
            source.host_paradrops.get(drop_id).unwrap().drop_frame
        );
        assert_eq!(loaded.host_paradrops.transports_spawned, 1);

        assert_eq!(loaded.host_leaflet_drops.pending_count(), 1);
        let leaflet = loaded.host_leaflet_drops.get(leaflet_id).expect("leaflet");
        assert_eq!(
            leaflet.impact_frame,
            source
                .host_leaflet_drops
                .get(leaflet_id)
                .unwrap()
                .impact_frame
        );

        assert_eq!(loaded.host_sneak_attacks.pending_count(), 1);
        let sneak = loaded.host_sneak_attacks.get(sneak_id).expect("sneak");
        assert_eq!(sneak.phase, HostSneakAttackPhase::Queued);
        assert_eq!(
            sneak.spawn_frame,
            source.host_sneak_attacks.get(sneak_id).unwrap().spawn_frame
        );
        assert_eq!(loaded.host_sneak_attacks.pending_shockwaves.len(), 1);
    }
}
