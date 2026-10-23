//! Versioned Main-owned A10 delivery registry continuation.
//! Actor and projectile bincode layouts remain unchanged; durable ObjectId
//! bindings are stored only in this lifecycle-tail capsule.
use crate::game_logic::host_a10_strike_flight::{
    A10DeliveryId, HostA10StrikeDelivery, HostA10StrikeFlightRegistry,
};
use crate::game_logic::special_power_strikes::{
    HostSpecialPowerStrike, HostStrikePhase, HostSuperweaponKind,
};
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const MAGIC: &[u8; 4] = b"A10F";
const VERSION: u32 = 2;
const MAX_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Payload {
    registry: WireRegistry,
    lighting: crate::game_logic::lighting_rules::LightingContinuationV1,
}

#[derive(Serialize, Deserialize)]
struct WireRegistry {
    transports_spawned: u32,
    missiles_scheduled: u32,
    missiles_dropped: u32,
    impacts: u32,
    next_delivery_id: u32,
    deliveries: Vec<(A10DeliveryId, WireDelivery)>,
    transport_bindings: Vec<(ObjectId, A10DeliveryId)>,
    live_transport_bindings: Vec<ObjectId>,
    missile_bindings: Vec<(ObjectId, A10DeliveryId)>,
}

#[derive(Serialize, Deserialize)]
struct WireDelivery {
    source_object: ObjectId,
    strike_id: Option<u32>,
    tier: crate::game_logic::special_power_strikes::A10StrikeScienceTier,
    transport_ids: Vec<ObjectId>,
    active_transports: Vec<ObjectId>,
    active_missiles: Vec<ObjectId>,
    payload_attempts_consumed: u32,
    missiles_scheduled: u32,
    impacts: u32,
    launch_complete: bool,
    completed: bool,
}

impl WireRegistry {
    fn from_registry(registry: &HostA10StrikeFlightRegistry, world: &GameLogic) -> Self {
        Self {
            transports_spawned: registry.transports_spawned,
            missiles_scheduled: registry.missiles_scheduled,
            missiles_dropped: registry.missiles_dropped,
            impacts: registry.impacts,
            next_delivery_id: registry.next_delivery_id,
            deliveries: registry
                .deliveries
                .iter()
                .map(|(id, row)| {
                    (
                        *id,
                        WireDelivery {
                            source_object: row.source_object,
                            strike_id: row.strike_id,
                            tier: row.tier,
                            transport_ids: row.transport_ids.iter().copied().collect(),
                            active_transports: row.active_transports.iter().copied().collect(),
                            active_missiles: row.active_missiles.iter().copied().collect(),
                            payload_attempts_consumed: row.payload_attempts_consumed,
                            missiles_scheduled: row.missiles_scheduled,
                            impacts: row.impacts,
                            launch_complete: row.launch_complete,
                            completed: row.completed,
                        },
                    )
                })
                .collect(),
            transport_bindings: registry
                .transport_bindings
                .iter()
                .map(|(id, delivery)| (*id, *delivery))
                .collect(),
            live_transport_bindings: registry
                .transport_bindings
                .keys()
                .filter_map(|id| {
                    let actor = world.objects.get(id)?;
                    registry
                        .transport_lifetime_matches_current_actor(
                            *id,
                            &actor.visual_draw_identity_token,
                        )
                        .then_some(*id)
                })
                .collect(),
            missile_bindings: registry
                .missile_bindings
                .iter()
                .map(|(id, delivery)| (*id, *delivery))
                .collect(),
        }
    }
}

impl WireRegistry {
    fn into_registry(
        self,
    ) -> SaveLoadResult<(
        HostA10StrikeFlightRegistry,
        std::collections::BTreeSet<ObjectId>,
    )> {
        let mut deliveries = std::collections::BTreeMap::new();
        for (id, row) in self.deliveries {
            let transport_ids =
                unique_set(row.transport_ids, "duplicate transport ID in delivery")?;
            let active_transports =
                unique_set(row.active_transports, "duplicate active transport ID")?;
            let active_missiles = unique_set(row.active_missiles, "duplicate active missile ID")?;
            if deliveries
                .insert(
                    id,
                    HostA10StrikeDelivery {
                        source_object: row.source_object,
                        strike_id: row.strike_id,
                        tier: row.tier,
                        transport_ids,
                        active_transports,
                        active_missiles,
                        payload_attempts_consumed: row.payload_attempts_consumed,
                        missiles_scheduled: row.missiles_scheduled,
                        impacts: row.impacts,
                        launch_complete: row.launch_complete,
                        completed: row.completed,
                    },
                )
                .is_some()
            {
                return Err(corrupted("duplicate A10 delivery ID"));
            }
        }
        let transport_bindings =
            unique_map(self.transport_bindings, "duplicate A10 transport binding")?;
        let live_transport_bindings = unique_set(
            self.live_transport_bindings,
            "duplicate live A10 transport identity",
        )?;
        if live_transport_bindings
            .iter()
            .any(|id| !transport_bindings.contains_key(id))
        {
            return Err(corrupted(
                "live A10 identity has no historical transport binding",
            ));
        }
        let missile_bindings = unique_map(self.missile_bindings, "duplicate A10 missile binding")?;
        let registry = HostA10StrikeFlightRegistry {
            transports_spawned: self.transports_spawned,
            missiles_scheduled: self.missiles_scheduled,
            missiles_dropped: self.missiles_dropped,
            impacts: self.impacts,
            next_delivery_id: self.next_delivery_id,
            deliveries,
            transport_bindings,
            missile_bindings,
            transport_lifetimes: std::collections::BTreeMap::new(),
        };
        Ok((registry, live_transport_bindings))
    }
}

fn unique_set(
    values: Vec<ObjectId>,
    message: &str,
) -> SaveLoadResult<std::collections::BTreeSet<ObjectId>> {
    let mut set = std::collections::BTreeSet::new();
    for value in values {
        if !set.insert(value) {
            return Err(corrupted(message));
        }
    }
    Ok(set)
}
fn unique_map<K: Ord, V>(
    values: Vec<(K, V)>,
    message: &str,
) -> SaveLoadResult<std::collections::BTreeMap<K, V>> {
    let mut map = std::collections::BTreeMap::new();
    for (key, value) in values {
        if map.insert(key, value).is_some() {
            return Err(corrupted(message));
        }
    }
    Ok(map)
}

pub(super) struct Decoded {
    pub registry: HostA10StrikeFlightRegistry,
    pub live_transport_bindings: std::collections::BTreeSet<ObjectId>,
    pub live_strike_ids: HashSet<u32>,
    pub lighting: Option<crate::game_logic::lighting_rules::LightingContentRules>,
}

pub(super) fn append_to_lifecycle_tail(
    bytes: &mut Vec<u8>,
    world: &GameLogic,
) -> SaveLoadResult<()> {
    let registry = &world.a10_strike_flight_reg;
    if registry.deliveries.values().any(|row| !row.launch_complete) {
        return Err(corrupted(
            "cannot snapshot during A10 delivery launch callbacks",
        ));
    }
    let payload = bincode_legacy::serialize(&Payload {
        registry: WireRegistry::from_registry(registry, world),
        lighting: world.capture_lighting_continuation(),
    })
    .map_err(|error| corrupted(format!("A10 registry encode: {error}")))?;
    let len = u32::try_from(payload.len()).map_err(|_| corrupted("A10 registry too large"))?;
    bytes.extend_from_slice(&payload);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(MAGIC);
    Ok(())
}

/// Decode and cross-check before restore retires any receiving objects.
pub(super) fn decode_and_validate(
    bytes: &[u8],
    objects: &HashMap<ObjectId, super::ObjectSnapshot>,
    strikes: &[HostSpecialPowerStrike],
    lifecycle: &super::lifecycle_tail::LifecycleTail,
    receiving_world: &GameLogic,
) -> SaveLoadResult<Decoded> {
    let payload = decode_suffix(bytes)?;
    let active_saved: Vec<_> = strikes
        .iter()
        .filter(|strike| {
            strike.kind == HostSuperweaponKind::A10Strike
                && strike.phase == HostStrikePhase::Queued
                && strike.live_a10_delivery
        })
        .collect();
    let Some(payload) = payload else {
        if !active_saved.is_empty() || snapshot_has_a10_actor(lifecycle)? {
            return Err(corrupted(
                "saved A10 actor or live strike lacks its registry capsule",
            ));
        }
        return Ok(Decoded {
            registry: HostA10StrikeFlightRegistry::new(),
            live_transport_bindings: std::collections::BTreeSet::new(),
            live_strike_ids: HashSet::new(),
            lighting: None,
        });
    };
    let lighting = receiving_world
        .validate_lighting_continuation(&payload.lighting)
        .map_err(|error| corrupted(format!("invalid lighting continuation: {error}")))?;
    let (registry, live_transport_bindings) = payload.registry.into_registry()?;
    validate_registry(
        &registry,
        &live_transport_bindings,
        objects,
        strikes,
        lifecycle,
    )?;
    let live_strike_ids = registry
        .deliveries
        .iter()
        .filter_map(|(_, row)| {
            row.strike_id.filter(|strike_id| {
                !row.completed
                    && strikes.iter().any(|strike| {
                        strike.id == *strike_id
                            && strike.kind == HostSuperweaponKind::A10Strike
                            && strike.phase == HostStrikePhase::Queued
                    })
            })
        })
        .collect::<HashSet<_>>();
    for strike in active_saved {
        if !live_strike_ids.contains(&strike.id) {
            return Err(corrupted(format!(
                "queued live A10 strike {} has no validated delivery",
                strike.id
            )));
        }
    }
    Ok(Decoded {
        registry,
        live_transport_bindings,
        live_strike_ids,
        lighting: Some(lighting),
    })
}

/// Prefix boundary before the optional A10F continuation. This lets earlier
/// owner frames (such as AIQF) locate themselves without scanning through A10F.
pub(super) fn suffix_start(bytes: &[u8]) -> SaveLoadResult<usize> {
    let end = super::body_rules_persist::suffix_start(bytes)?;
    let prefix = bytes
        .get(..end)
        .ok_or_else(|| corrupted("A10 footer boundary invalid"))?;
    if !prefix.ends_with(MAGIC) {
        return Ok(end);
    }
    // Validate the complete typed frame before exposing its boundary to an
    // earlier owner reader. This is read-only and performs no restore mutation.
    decode_suffix(bytes)?;
    if prefix.len() < 12 {
        return Err(corrupted("A10 capsule footer truncated"));
    }
    let footer = prefix.len() - 12;
    let version = u32::from_le_bytes(prefix[footer..footer + 4].try_into().unwrap());
    if version != VERSION {
        return Err(corrupted(format!(
            "unsupported A10 capsule version {version}"
        )));
    }
    let len = u32::from_le_bytes(prefix[footer + 4..footer + 8].try_into().unwrap()) as usize;
    if len > MAX_PAYLOAD_BYTES {
        return Err(corrupted("A10 capsule exceeds size limit"));
    }
    let start = footer
        .checked_sub(len)
        .ok_or_else(|| corrupted("A10 capsule length underflow"))?;
    if prefix[..start].ends_with(MAGIC) {
        return Err(corrupted("duplicate A10 registry capsule"));
    }
    Ok(start)
}

fn decode_suffix(bytes: &[u8]) -> SaveLoadResult<Option<Payload>> {
    // A10F precedes the optional Body-rules frame and APRO; its boundary comes from the next
    // framed footer instead of scanning arbitrary actor/template payloads.
    let end = super::body_rules_persist::suffix_start(bytes)?;
    let prefix = bytes
        .get(..end)
        .ok_or_else(|| corrupted("A10 footer boundary invalid"))?;
    if !prefix.ends_with(MAGIC) {
        return Ok(None);
    }
    if prefix.len() < 12 {
        return Err(corrupted("A10 capsule footer truncated"));
    }
    let footer = prefix.len() - 12;
    let version = u32::from_le_bytes(prefix[footer..footer + 4].try_into().unwrap());
    if version != VERSION {
        return Err(corrupted(format!(
            "unsupported A10 capsule version {version}"
        )));
    }
    let len = u32::from_le_bytes(prefix[footer + 4..footer + 8].try_into().unwrap()) as usize;
    if len > MAX_PAYLOAD_BYTES {
        return Err(corrupted("A10 capsule exceeds size limit"));
    }
    let start = footer
        .checked_sub(len)
        .ok_or_else(|| corrupted("A10 capsule length underflow"))?;
    if prefix[..start].ends_with(MAGIC) {
        return Err(corrupted("duplicate A10 registry capsule"));
    }
    let (payload, consumed): (Payload, usize) =
        bincode::serde::decode_from_slice(&prefix[start..footer], bincode::config::legacy())
            .map_err(|error| corrupted(format!("A10 capsule decode: {error}")))?;
    if consumed != len {
        return Err(corrupted("A10 capsule payload has trailing bytes"));
    }
    Ok(Some(payload))
}

fn validate_registry(
    registry: &HostA10StrikeFlightRegistry,
    live_transport_bindings: &std::collections::BTreeSet<ObjectId>,
    objects: &HashMap<ObjectId, super::ObjectSnapshot>,
    strikes: &[HostSpecialPowerStrike],
    lifecycle: &super::lifecycle_tail::LifecycleTail,
) -> SaveLoadResult<()> {
    if registry.next_delivery_id == 0 {
        return Err(corrupted("A10 delivery allocator is zero"));
    }
    let max_id = registry.deliveries.keys().map(|id| id.0).max().unwrap_or(0);
    if registry.next_delivery_id <= max_id {
        return Err(corrupted("A10 delivery allocator behind saved IDs"));
    }
    let envelopes: HashMap<u32, &gamelogic::world::entities::EntityLifecycleEnvelope> = lifecycle
        .envelopes
        .iter()
        .map(|e| (e.entity_id, e))
        .collect();
    let producers: HashMap<u32, u32> = lifecycle
        .producer_links
        .iter()
        .map(|link| (link.entity_id, link.producer_id))
        .collect();
    if envelopes.len() != lifecycle.envelopes.len()
        || producers.len() != lifecycle.producer_links.len()
    {
        return Err(corrupted(
            "duplicate lifecycle identity while validating A10 bindings",
        ));
    }
    let mut strike_owners = HashSet::new();
    for envelope in &lifecycle.envelopes {
        let has_transport = envelope
            .module_states
            .iter()
            .any(|module| module.tag == "A10StrikeTransport");
        let has_missile = envelope.module_states.iter().find(|module| module.tag == "ProjectileFlightResiduals")
            .map(|module| bincode_legacy::deserialize::<crate::game_logic::object::entity_lifecycle_projectiles::ProjectileFlightResiduals>(&module.payload)
                .map(|residual| residual.a10_strike_missile)
                .map_err(|error| corrupted(format!("projectile lifecycle decode: {error}"))))
            .transpose()?.unwrap_or(false);
        if has_transport
            && !registry
                .transport_bindings
                .contains_key(&ObjectId(envelope.entity_id))
        {
            return Err(corrupted("saved A10 transport has no delivery binding"));
        }
        if has_missile
            && !registry
                .missile_bindings
                .contains_key(&ObjectId(envelope.entity_id))
        {
            return Err(corrupted("saved A10 missile has no delivery binding"));
        }
    }
    for actor in live_transport_bindings {
        if !objects.contains_key(actor) || !registry.transport_bindings.contains_key(actor) {
            return Err(corrupted(
                "live A10 actor identity is absent from saved object records",
            ));
        }
    }
    for (id, row) in &registry.deliveries {
        if id.0 == 0
            || !row.launch_complete
            || row.impacts > row.missiles_scheduled
            || row.active_missiles.len() as u32 + row.impacts > row.missiles_scheduled
            || row.missiles_scheduled > (row.transport_ids.len() as u32).saturating_mul(6)
            || row.payload_attempts_consumed
                > (row.transport_ids.len() as u32)
                    .saturating_mul(crate::game_logic::host_a10_strike_flight::A10_NUM_BONES)
            || row
                .active_transports
                .iter()
                .any(|actor| !row.transport_ids.contains(actor))
            || row
                .active_missiles
                .iter()
                .any(|actor| !registry.missile_bindings.contains_key(actor))
            || (row.completed
                && (!row.launch_complete
                    || !row.active_transports.is_empty()
                    || !row.active_missiles.is_empty()))
        {
            return Err(corrupted(
                "invalid A10 delivery counters or active actor sets",
            ));
        }
        if let Some(strike_id) = row.strike_id {
            if !strike_owners.insert(strike_id) {
                return Err(corrupted("two A10 deliveries claim one strike"));
            }
            let strike = strikes
                .iter()
                .find(|strike| strike.id == strike_id)
                .ok_or_else(|| corrupted("A10 delivery references missing strike"))?;
            if strike.kind != HostSuperweaponKind::A10Strike
                || strike.source_object != row.source_object
                || strike.a10_tier != row.tier
            {
                return Err(corrupted("A10 delivery/strike identity mismatch"));
            }
        }
        for transport in &row.transport_ids {
            if registry.transport_bindings.get(transport) != Some(id) {
                return Err(corrupted("A10 transport binding missing or mismatched"));
            }
            if let Some(object) = objects.get(transport) {
                let envelope = envelopes.get(&transport.0);
                let is_a10_transport = envelope.is_some_and(|envelope| {
                    envelope
                        .module_states
                        .iter()
                        .any(|module| module.tag == "A10StrikeTransport")
                });
                if !live_transport_bindings.contains(transport) {
                    if is_a10_transport {
                        return Err(corrupted(
                            "present A10 transport has a different saved actor lifetime",
                        ));
                    }
                    continue;
                }
                if producers.get(&transport.0) != Some(&row.source_object.0) {
                    return Err(corrupted(
                        "A10 transport producer does not match delivery source",
                    ));
                }
                let envelope = envelopes
                    .get(&transport.0)
                    .ok_or_else(|| corrupted("A10 transport lifecycle envelope missing"))?;
                let module = envelope
                    .module_states
                    .iter()
                    .find(|module| module.tag == "A10StrikeTransport")
                    .ok_or_else(|| corrupted("A10 transport residual missing"))?;
                let residual: crate::game_logic::host_a10_strike_flight::HostA10StrikeFlightData =
                    bincode_legacy::deserialize(&module.payload)
                        .map_err(|e| corrupted(format!("A10 transport residual decode: {e}")))?;
                let active = row.active_transports.contains(transport);
                if !active
                    && !object.status.destroyed
                    && residual.transport_alive
                    && residual.missiles_remaining > 0
                {
                    return Err(corrupted(
                        "A10 transport with remaining payload is absent from active set",
                    ));
                }
            }
        }
        for actor in &row.active_missiles {
            if registry.missile_bindings.get(actor) != Some(id) {
                return Err(corrupted("active A10 missile binding mismatch"));
            }
        }
    }
    for (actor, id) in &registry.transport_bindings {
        if !registry
            .deliveries
            .get(id)
            .is_some_and(|row| row.transport_ids.contains(actor))
        {
            return Err(corrupted("orphan A10 transport binding"));
        }
    }
    for (actor, id) in &registry.missile_bindings {
        if !registry.deliveries.contains_key(id) {
            return Err(corrupted("unknown A10 missile delivery binding"));
        }
        // Present historical missiles must still carry the projectile-flight flag.
        if objects.contains_key(actor) {
            let envelope = envelopes
                .get(&actor.0)
                .ok_or_else(|| corrupted("bound A10 missile envelope missing"))?;
            let module = envelope
                .module_states
                .iter()
                .find(|module| module.tag == "ProjectileFlightResiduals")
                .ok_or_else(|| corrupted("bound A10 missile residual missing"))?;
            let residual: crate::game_logic::object::entity_lifecycle_projectiles::ProjectileFlightResiduals = bincode_legacy::deserialize(&module.payload).map_err(|e| corrupted(format!("A10 missile residual decode: {e}")))?;
            if !residual.a10_strike_missile {
                return Err(corrupted(
                    "A10 missile binding points at another projectile role",
                ));
            }
            let delivery = registry
                .deliveries
                .get(id)
                .ok_or_else(|| corrupted("unknown A10 missile delivery binding"))?;
            if delivery.active_missiles.contains(actor) && !producers.contains_key(&actor.0) {
                return Err(corrupted("active A10 missile producer link missing"));
            }
            if let Some(producer) = producers.get(&actor.0) {
                if registry.transport_bindings.get(&ObjectId(*producer)) != Some(id) {
                    return Err(corrupted(
                        "A10 missile producer belongs to another delivery",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn snapshot_has_a10_actor(
    lifecycle: &super::lifecycle_tail::LifecycleTail,
) -> SaveLoadResult<bool> {
    for envelope in &lifecycle.envelopes {
        if envelope
            .module_states
            .iter()
            .any(|module| module.tag == "A10StrikeTransport")
        {
            return Ok(true);
        }
        if let Some(module) = envelope
            .module_states
            .iter()
            .find(|module| module.tag == "ProjectileFlightResiduals")
        {
            let residual: crate::game_logic::object::entity_lifecycle_projectiles::ProjectileFlightResiduals =
                bincode_legacy::deserialize(&module.payload).map_err(|error| corrupted(format!("projectile lifecycle decode: {error}")))?;
            if residual.a10_strike_missile {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn corrupted(message: impl Into<String>) -> SaveLoadError {
    SaveLoadError::Corrupted(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_a10_strike_flight::A10DeliveryId;
    use crate::game_logic::{Player, Team};
    use crate::save_load::snapshot::SnapshotBuilder;

    pub(super) fn owned_a10_world() -> GameLogic {
        let text = format!(
            "Object FlightSource\nKindOf = STRUCTURE\nBody = ActiveBody Body\nMaxHealth = 5000\nInitialHealth = 5000\nEnd\nEnd\nObject {}\nKindOf = VEHICLE AIRCRAFT\nBody = ActiveBody Body\nMaxHealth = 600\nInitialHealth = 600\nEnd\nEnd\nObject {}\nKindOf = PROJECTILE\nBody = ActiveBody Body\nMaxHealth = 40\nInitialHealth = 40\nEnd\nEnd\n",
            crate::game_logic::special_power_strikes::A10_TRANSPORT,
            crate::game_logic::special_power_strikes::A10_PAYLOAD_TEMPLATE,
        );
        let mut world = GameLogic::new();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "a10_snapshot.ini".into(),
                    text,
                )])
                .unwrap(),
            )
            .unwrap();
        let mut owner = Player::new(1, Team::USA, "Snapshot Owner", true);
        owner.alliance_team = 1;
        world.add_player(owner);
        world
    }

    pub(super) fn rewrite_capsule(tail: &mut Vec<u8>, edit: impl FnOnce(&mut Payload)) {
        let end = super::super::body_rules_persist::suffix_start(tail).unwrap();
        let footer = end - 12;
        assert_eq!(&tail[end - 4..end], MAGIC);
        let len = u32::from_le_bytes(tail[footer + 4..footer + 8].try_into().unwrap()) as usize;
        let start = footer - len;
        let mut payload: Payload = bincode_legacy::deserialize(&tail[start..footer]).unwrap();
        edit(&mut payload);
        let encoded = bincode_legacy::serialize(&payload).unwrap();
        let mut replacement = tail[..start].to_vec();
        replacement.extend_from_slice(&encoded);
        replacement.extend_from_slice(&VERSION.to_le_bytes());
        replacement.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        replacement.extend_from_slice(MAGIC);
        replacement.extend_from_slice(&tail[end..]);
        *tail = replacement;
    }

    #[test]
    fn registry_capsule_roundtrips_before_apro_footer() {
        let mut source = GameLogic::new();
        let id = source
            .a10_strike_flight_reg
            .begin_delivery(
                ObjectId(999),
                crate::game_logic::special_power_strikes::A10StrikeScienceTier::Level2,
                None,
            )
            .unwrap();
        source.a10_strike_flight_reg.finish_delivery_launch(id);
        source.a10_strike_flight_reg.complete_delivery_if_final(id);
        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source).unwrap();
        super::super::player_academy_persist::append_to_lifecycle_tail(&mut bytes, &source);
        super::super::combat_particle_attachment_persist::append(&mut bytes, &source).unwrap();
        super::super::construction_completion_persist::append_to_lifecycle_tail(
            &mut bytes, &source,
        );
        let decoded = decode_and_validate(
            &bytes,
            &HashMap::new(),
            &[],
            &super::super::lifecycle_tail::LifecycleTail::default(),
            &source,
        )
        .unwrap();
        assert_eq!(decoded.registry.deliveries.len(), 1);
        assert!(decoded.registry.delivery_is_completed(A10DeliveryId(1)));
        assert!(decoded.live_strike_ids.is_empty());
        assert!(decoded.lighting.is_some());
    }

    #[test]
    fn duplicate_wire_bindings_are_rejected_before_registry_install() {
        let decoded = WireRegistry {
            transports_spawned: 0,
            missiles_scheduled: 0,
            missiles_dropped: 0,
            impacts: 0,
            next_delivery_id: 2,
            deliveries: vec![(
                A10DeliveryId(1),
                WireDelivery {
                    source_object: ObjectId(9),
                    strike_id: None,
                    tier: crate::game_logic::special_power_strikes::A10StrikeScienceTier::Level1,
                    transport_ids: vec![ObjectId(10)],
                    active_transports: Vec::new(),
                    active_missiles: Vec::new(),
                    payload_attempts_consumed: 0,
                    missiles_scheduled: 0,
                    impacts: 0,
                    launch_complete: true,
                    completed: false,
                },
            )],
            transport_bindings: vec![
                (ObjectId(10), A10DeliveryId(1)),
                (ObjectId(10), A10DeliveryId(1)),
            ],
            live_transport_bindings: Vec::new(),
            missile_bindings: Vec::new(),
        }
        .into_registry();
        assert!(decoded.is_err());
    }

    #[test]
    fn actual_queued_a10_world_roundtrips_delivery_identity_and_live_ownership() {
        let mut source = owned_a10_world();
        let source_id = source
            .create_object_for_player("FlightSource", 1, glam::Vec3::ZERO)
            .expect("admitted command source");
        let strike_id = source
            .queue_special_power_strike(
                &crate::command_system::SpecialPowerType::Airstrike,
                source_id,
                glam::Vec3::new(400.0, 0.0, 500.0),
            )
            .expect("queued A10 strike");
        let delivery_id = source
            .a10_strike_flight_reg
            .deliveries
            .iter()
            .find_map(|(id, row)| (row.strike_id == Some(strike_id)).then_some(*id))
            .expect("queued strike has actual delivery");
        let transport_ids = source
            .a10_strike_flight_reg
            .delivery_transport_ids(delivery_id);
        assert!(!transport_ids.is_empty(), "queue must birth its transport");

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).unwrap();
        let encoded = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = super::super::decode_bincode_world_snapshot(&encoded).unwrap();
        let mut restored = owned_a10_world();
        builder
            .restore_from_snapshot(&decoded, &mut restored)
            .unwrap();

        assert_eq!(
            restored
                .a10_strike_flight_reg
                .delivery_strike_id(delivery_id),
            Some(strike_id)
        );
        assert_eq!(
            restored
                .a10_strike_flight_reg
                .delivery_transport_ids(delivery_id),
            transport_ids
        );
        assert!(
            restored
                .special_power_strikes
                .get(strike_id)
                .expect("restored queued strike")
                .live_a10_delivery
        );
        for transport_id in transport_ids {
            assert_eq!(
                restored
                    .a10_strike_flight_reg
                    .delivery_for_transport(transport_id),
                Some(delivery_id)
            );
            assert!(
                restored.objects[&transport_id]
                    .a10_strike_transport
                    .is_some()
            );
        }
    }

    #[test]
    fn active_a10_without_capsule_is_rejected_before_receiver_mutation() {
        let mut receiver = owned_a10_world();
        let source_id = receiver
            .create_object_for_player("FlightSource", 1, glam::Vec3::ZERO)
            .unwrap();
        let strike_id = receiver
            .queue_special_power_strike(
                &crate::command_system::SpecialPowerType::Airstrike,
                source_id,
                glam::Vec3::new(400.0, 0.0, 500.0),
            )
            .unwrap();
        let before_ids: Vec<_> = receiver.objects.registered_keys().copied().collect();
        let before_delivery_count = receiver.a10_strike_flight_reg.deliveries.len();
        let builder = SnapshotBuilder::new();
        let mut snapshot = builder.create_world_snapshot(&receiver).unwrap();

        let end = super::super::body_rules_persist::suffix_start(&snapshot.lifecycle_tail).unwrap();
        let footer = end - 12;
        assert_eq!(&snapshot.lifecycle_tail[end - 4..end], MAGIC);
        let payload_len = u32::from_le_bytes(
            snapshot.lifecycle_tail[footer + 4..footer + 8]
                .try_into()
                .unwrap(),
        ) as usize;
        let start = footer - payload_len;
        snapshot.lifecycle_tail.drain(start..end);

        assert!(
            builder
                .restore_from_snapshot(&snapshot, &mut receiver)
                .is_err()
        );
        assert_eq!(
            receiver
                .objects
                .registered_keys()
                .copied()
                .collect::<Vec<_>>(),
            before_ids
        );
        assert_eq!(
            receiver.a10_strike_flight_reg.deliveries.len(),
            before_delivery_count
        );
        assert!(
            receiver
                .special_power_strikes
                .get(strike_id)
                .expect("original strike remains")
                .live_a10_delivery
        );
    }

    #[test]
    fn malformed_a10_counter_and_strike_claim_reject_before_receiver_mutation() {
        let mut source = owned_a10_world();
        let source_id = source
            .create_object_for_player("FlightSource", 1, glam::Vec3::ZERO)
            .unwrap();
        source
            .queue_special_power_strike(
                &crate::command_system::SpecialPowerType::Airstrike,
                source_id,
                glam::Vec3::new(400.0, 0.0, 500.0),
            )
            .unwrap();
        let builder = SnapshotBuilder::new();
        for corruption in 0..3 {
            let mut snapshot = builder.create_world_snapshot(&source).unwrap();
            rewrite_capsule(&mut snapshot.lifecycle_tail, |payload| {
                if corruption == 1 {
                    payload.registry.deliveries[0].1.strike_id = Some(u32::MAX);
                } else if corruption == 2 {
                    payload.registry.deliveries[0].1.transport_ids.clear();
                } else {
                    payload.registry.next_delivery_id = 0;
                }
            });
            let mut receiver = owned_a10_world();
            assert!(
                builder
                    .restore_from_snapshot(&snapshot, &mut receiver)
                    .is_err()
            );
            assert!(receiver.objects.registered_is_empty());
            assert!(receiver.a10_strike_flight_reg.deliveries.is_empty());
            assert_eq!(receiver.special_power_strikes.strike_count(), 0);
        }
    }
}

#[cfg(test)]
#[path = "a10_flight_persist_tests.rs"]
mod followup_tests;
