//! Host A10 Thunderbolt DeliverPayload residual (jet + missiles).
//!
//! C++: `SUPERWEAPON_A10ThunderboltMissileStrike1/2/3` DeliverPayload
//! Transport=`AmericaJetA10Thunderbolt`, Payload=`A10ThunderboltMissile`,
//! FormationSize 1/2/3, FormationSpacing 35, DropDelay 500ms,
//! VisibleItemsDroppedPerInterval 2, VisibleNumBones 6.
//!
//! Residual playability slice:
//! - Spawn jet transport residual toward target
//! - Schedule missile drops along formation line with DropDelay stagger
//! - Missiles dive and apply A10ThunderboltMissileWeapon residual
//!
//! Fail-closed: not full AmericaJetA10 pathfinder / preferred-height locomotor.

use crate::game_logic::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Weak};

use crate::game_logic::host_deliver_payload::is_close_enough_to_target_squared_residual;
use crate::game_logic::special_power_strikes::{
    A10_DELIVERY_DISTANCE, A10_DIVE_END_DISTANCE, A10_DIVE_START_DISTANCE,
    A10_FORMATIONION_SPACING, A10_MISSILE_PRIMARY_DAMAGE, A10_MISSILE_PRIMARY_RADIUS,
    A10_PAYLOAD_TEMPLATE, A10_PRE_OPEN_DISTANCE, A10_TRANSPORT, A10StrikeScienceTier,
};

/// C++ DeliverPayloadAIUpdate `DIVESTATE_PREDIVE`.
pub const A10_DIVE_PREDIVE: u8 = 0;
/// C++ DeliverPayloadAIUpdate `DIVESTATE_DIVING`.
pub const A10_DIVE_DIVING: u8 = 1;
/// C++ DeliverPayloadAIUpdate `DIVESTATE_POSTDIVE`.
pub const A10_DIVE_POSTDIVE: u8 = 2;
/// Host residual preferred-height cruise (precise-Z off).
pub const A10_CRUISE_HEIGHT: f32 = 140.0;
/// C++ `ThingTemplate::getPerUnitSound("StartDive")`.
pub const A10_START_DIVE_SOUND: &str = "StartDive";
/// C++ A10ThunderboltVulcan DelayBetweenShots 60ms → 2 frames @ 30 FPS.
pub const A10_VULCAN_DELAY_FRAMES: u32 = 2;

/// One tick of C++ DeliverPayloadAIUpdate dive + StrafingWeaponSlot (cpp:155-220).
#[derive(Debug, Clone, Copy)]
pub struct A10DiveTick {
    pub new_y: f32,
    pub start_dive: bool,
    pub should_strafe: bool,
    pub strafe_point: Vec3,
}

/// Live Y-up residual of leftover `DeliverPayloadAIUpdate::update_dive_logic`.
/// C++ `velocity.z` (up) is `vel.y` here. PREDIVE uses 2D start 500; DIVING
/// ends on 3D 300; vulcan fires while diving and `vel.y < 5`.
pub fn tick_a10_dive(
    dive_state: &mut u8,
    pos: Vec3,
    target: Vec3,
    vel: Vec3,
    speed: f32,
) -> A10DiveTick {
    let mut out = A10DiveTick {
        new_y: pos.y,
        start_dive: false,
        should_strafe: false,
        strafe_point: Vec3::new(target.x, 0.0, target.z),
    };
    if *dive_state == A10_DIVE_POSTDIVE {
        if pos.y < A10_CRUISE_HEIGHT {
            out.new_y = (pos.y + speed).min(A10_CRUISE_HEIGHT);
        } else {
            out.new_y = pos.y.max(A10_CRUISE_HEIGHT);
        }
        return out;
    }
    let dx = pos.x - target.x;
    let dz = pos.z - target.z;
    let dy = pos.y - target.y;
    let dist2_sq = dx * dx + dz * dz;
    if *dive_state == A10_DIVE_PREDIVE {
        out.new_y = pos.y.max(A10_CRUISE_HEIGHT);
        if dist2_sq <= A10_DIVE_START_DISTANCE * A10_DIVE_START_DISTANCE {
            *dive_state = A10_DIVE_DIVING;
            out.start_dive = true;
        }
        return out;
    }
    let dist3_sq = dist2_sq + dy * dy;
    if dist3_sq <= A10_DIVE_END_DISTANCE * A10_DIVE_END_DISTANCE {
        *dive_state = A10_DIVE_POSTDIVE;
    } else {
        let dist3 = dist3_sq.sqrt();
        if dist3 > 1e-4 {
            let step = speed.min(dist3);
            out.new_y = pos.y + (target.y - pos.y) / dist3 * step;
        }
    }
    if vel.y < 5.0 {
        let current = dist3_sq.sqrt();
        let denom = (A10_DIVE_START_DISTANCE - A10_DIVE_END_DISTANCE).max(0.001);
        let dive_ratio = (A10_DIVE_START_DISTANCE - current) / denom;
        let mut v = Vec3::new(vel.x, 0.0, vel.z);
        let len = v.length();
        if len > 1e-6 {
            v /= len;
        }
        v *= dive_ratio * 100.0;
        let backwards = v * 0.33;
        let mut strafe = target - backwards + v;
        strafe.y = 0.0;
        out.strafe_point = strafe;
        out.should_strafe = true;
    }
    out
}

/// Retail DropDelay residual (ms) for A10 payload sets.
pub const A10_DROP_DELAY_MS: u32 = 500;
/// DropDelay frames @ 30 FPS.
pub const A10_DROP_DELAY_FRAMES: u32 = 15;
/// Retail VisibleItemsDroppedPerInterval residual.
pub const A10_ITEMS_PER_DROP: u32 = 2;
/// Retail VisibleNumBones residual (max missiles per jet).
pub const A10_NUM_BONES: u32 = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostA10StrikeFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    #[serde(default)]
    pub exit: Vec3,
    pub tier: A10StrikeScienceTier,
    pub transport_alive: bool,
    #[serde(default)]
    pub passed_target: bool,
    #[serde(default)]
    pub last_vulcan_frame: u32,
    /// C++ DeliverPayloadAIUpdate `m_diveState`.
    #[serde(default)]
    pub dive_state: u8,
    /// C++ DeliverPayload visible bones still to drop (VisibleNumBones 6).
    #[serde(default)]
    pub missiles_remaining: u32,
    /// Next frame a visible-payload pair may drop (DropDelay 15).
    #[serde(default)]
    pub next_drop_frame: u32,
    /// Previous 2D dist² for inbound DeliverPayload::isCloseEnoughToTarget.
    #[serde(default = "host_a10_prev_dist_sq_default")]
    pub prev_dist_sq: f32,
    /// Map extent for C++ `isOffMap` / HeadOffMap HUGE_DIST (world_min/max).
    #[serde(default)]
    pub map_min: Vec3,
    #[serde(default)]
    pub map_max: Vec3,
}

fn host_a10_prev_dist_sq_default() -> f32 {
    f32::MAX
}

impl HostA10StrikeFlightData {
    pub fn start(launch: Vec3, target: Vec3, tier: A10StrikeScienceTier) -> Self {
        Self::start_with_exit(launch, target, Vec3::ZERO, tier)
    }

    pub fn start_with_exit(
        launch: Vec3,
        target: Vec3,
        exit: Vec3,
        tier: A10StrikeScienceTier,
    ) -> Self {
        Self {
            target,
            launch,
            exit,
            tier,
            transport_alive: true,
            passed_target: false,
            last_vulcan_frame: 0,
            dive_state: A10_DIVE_PREDIVE,
            missiles_remaining: A10_NUM_BONES,
            next_drop_frame: 0,
            prev_dist_sq: f32::MAX,
            map_min: Vec3::ZERO,
            map_max: Vec3::ZERO,
        }
    }

    fn map_extent_ok(&self) -> bool {
        self.map_max.x > self.map_min.x && self.map_max.z > self.map_min.z
    }

    /// C++ HeadOffMapState: after the target, dest is HUGE_DIST ahead.
    /// Returns (new_pos, vel, off_map / CLEAN_UP).
    pub fn tick_transport(&mut self, pos: Vec3) -> (Vec3, Vec3, bool) {
        use crate::game_logic::host_deliver_payload::{
            head_off_map_exit_point_residual, is_off_map_residual,
        };
        let hx = self.target.x - self.launch.x;
        let hz = self.target.z - self.launch.z;
        if !self.passed_target {
            let dx = self.target.x - pos.x;
            let dz = self.target.z - pos.z;
            if dx * dx + dz * dz <= 25.0
                || (pos.x - self.target.x) * hx + (pos.z - self.target.z) * hz > 0.0
            {
                self.passed_target = true;
            }
        }
        let dest = if self.passed_target {
            if self.map_extent_ok() {
                head_off_map_exit_point_residual(
                    pos,
                    hx,
                    hz,
                    self.map_min.x,
                    self.map_min.z,
                    self.map_max.x,
                    self.map_max.z,
                )
            } else if self.exit.length_squared() > 1.0 {
                self.exit
            } else {
                self.target
            }
        } else {
            self.target
        };
        let dx = dest.x - pos.x;
        let dz = dest.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let speed = 22.0_f32;
        let mut new_pos = pos;
        new_pos.y = new_pos.y.max(140.0);
        let vel = if dist >= 1.0 {
            let step = speed.min(dist);
            new_pos.x += dx / dist * step;
            new_pos.z += dz / dist * step;
            new_pos - pos
        } else {
            Vec3::ZERO
        };
        let at_exit = self.passed_target
            && if self.map_extent_ok() {
                is_off_map_residual(
                    new_pos,
                    self.map_min.x,
                    self.map_min.z,
                    self.map_max.x,
                    self.map_max.z,
                )
            } else {
                self.exit.length_squared() > 1.0
                    && (new_pos.x - self.exit.x).abs() < 8.0
                    && (new_pos.z - self.exit.z).abs() < 8.0
            };
        (new_pos, vel, at_exit)
    }

    /// C++ `DeliverPayloadAIUpdate::isCloseEnoughToTarget` (DeliveryDistance 450,
    /// PreOpenDistance 0): inbound expands the allowed band to the sum.
    pub fn is_close_enough_to_target(&mut self, pos: Vec3) -> bool {
        let dx = pos.x - self.target.x;
        let dz = pos.z - self.target.z;
        let current = dx * dx + dz * dz;
        let previous = self.prev_dist_sq;
        self.prev_dist_sq = current;
        is_close_enough_to_target_squared_residual(
            current,
            previous,
            A10_DELIVERY_DISTANCE,
            A10_PRE_OPEN_DISTANCE,
        )
    }

    /// C++ DeliveringState first decrements a positive delay and returns.
    /// A delivery at N with a 15-frame delay is next attempted at N+16.
    /// The initial six-item/zero-deadline state is the only unscheduled state;
    /// a later wrapped zero is a real absolute deadline.
    pub(crate) fn visible_payload_attempt_due(&self, now: u32) -> bool {
        self.missiles_remaining != 0
            && ((self.missiles_remaining == A10_NUM_BONES && self.next_drop_frame == 0)
                || now.wrapping_sub(self.next_drop_frame) < (1_u32 << 31))
    }

    /// Admit an interval without advancing delivered-item observations across
    /// synchronous constructors. Call complete only after each attempt returns.
    pub(crate) fn begin_visible_payload_interval(&mut self, now: u32) -> u32 {
        if !self.visible_payload_attempt_due(now) {
            return 0;
        }
        self.next_drop_frame = now.wrapping_add(A10_DROP_DELAY_FRAMES + 1);
        self.missiles_remaining.min(A10_ITEMS_PER_DROP)
    }

    pub(crate) fn complete_visible_payload_attempt(&mut self) {
        self.missiles_remaining = self.missiles_remaining.saturating_sub(1);
    }

    /// Value-level adapter retained for existing non-callback consumers.
    pub fn take_visible_payload_drops(&mut self, now: u32) -> u32 {
        let attempts = self.begin_visible_payload_interval(now);
        for _ in 0..attempts {
            self.complete_visible_payload_attempt();
        }
        attempts
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct A10DeliveryId(pub u32);

/// Durable ownership row for one actual A10 OCL delivery. Actor IDs are held
/// only by the registry capsule; actor/projectile bincode stays unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HostA10StrikeDelivery {
    pub(crate) source_object: ObjectId,
    pub(crate) strike_id: Option<u32>,
    pub(crate) tier: A10StrikeScienceTier,
    pub(crate) transport_ids: BTreeSet<ObjectId>,
    /// Transports which can still attempt to release payload.
    pub(crate) active_transports: BTreeSet<ObjectId>,
    /// Missiles successfully born and not yet impacted/retired.
    pub(crate) active_missiles: BTreeSet<ObjectId>,
    pub(crate) payload_attempts_consumed: u32,
    pub(crate) missiles_scheduled: u32,
    pub(crate) impacts: u32,
    pub(crate) launch_complete: bool,
    pub(crate) completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostA10StrikeFlightRegistry {
    pub transports_spawned: u32,
    pub missiles_scheduled: u32,
    pub missiles_dropped: u32,
    pub impacts: u32,
    #[serde(default = "default_next_delivery_id")]
    pub(crate) next_delivery_id: u32,
    #[serde(default)]
    pub(crate) deliveries: BTreeMap<A10DeliveryId, HostA10StrikeDelivery>,
    /// Historical ID bindings survive actor retirement for missile lineage.
    #[serde(default)]
    pub(crate) transport_bindings: BTreeMap<ObjectId, A10DeliveryId>,
    #[serde(default)]
    pub(crate) missile_bindings: BTreeMap<ObjectId, A10DeliveryId>,
    /// Runtime-only identity witnesses prevent a reused ObjectId from inheriting old delivery ownership.
    #[serde(skip)]
    pub(crate) transport_lifetimes: BTreeMap<ObjectId, Weak<()>>,
}

fn default_next_delivery_id() -> u32 {
    1
}

impl Default for HostA10StrikeFlightRegistry {
    fn default() -> Self {
        Self {
            transports_spawned: 0,
            missiles_scheduled: 0,
            missiles_dropped: 0,
            impacts: 0,
            next_delivery_id: 1,
            deliveries: BTreeMap::new(),
            transport_bindings: BTreeMap::new(),
            missile_bindings: BTreeMap::new(),
            transport_lifetimes: BTreeMap::new(),
        }
    }
}

impl HostA10StrikeFlightRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
        self.next_delivery_id = 1;
    }

    /// Begin one queued or standalone delivery. IDs are never reused; exhaustion
    /// fails closed instead of wrapping onto historical bindings.
    pub(crate) fn begin_delivery(
        &mut self,
        source_object: ObjectId,
        tier: A10StrikeScienceTier,
        strike_id: Option<u32>,
    ) -> Option<A10DeliveryId> {
        let id = self.next_delivery_id.max(1);
        let next = id.checked_add(1)?;
        let delivery_id = A10DeliveryId(id);
        if self.deliveries.contains_key(&delivery_id) {
            return None;
        }
        self.next_delivery_id = next;
        self.deliveries.insert(
            delivery_id,
            HostA10StrikeDelivery {
                source_object,
                strike_id,
                tier,
                transport_ids: BTreeSet::new(),
                active_transports: BTreeSet::new(),
                active_missiles: BTreeSet::new(),
                payload_attempts_consumed: 0,
                missiles_scheduled: 0,
                impacts: 0,
                launch_complete: false,
                completed: false,
            },
        );
        Some(delivery_id)
    }

    pub(crate) fn bind_transport(
        &mut self,
        delivery_id: A10DeliveryId,
        transport_id: ObjectId,
    ) -> bool {
        if self.transport_bindings.contains_key(&transport_id) {
            return false;
        }
        let Some(delivery) = self.deliveries.get_mut(&delivery_id) else {
            return false;
        };
        delivery.transport_ids.insert(transport_id);
        delivery.active_transports.insert(transport_id);
        self.transport_bindings.insert(transport_id, delivery_id);
        self.transports_spawned = self.transports_spawned.saturating_add(1);
        true
    }

    /// Bind the actual actor lifetime immediately after its delivery ID is admitted.
    pub(crate) fn bind_transport_lifetime(
        &mut self,
        delivery_id: A10DeliveryId,
        transport_id: ObjectId,
        identity: &Arc<()>,
    ) -> bool {
        if self.transport_bindings.get(&transport_id) != Some(&delivery_id)
            || self.transport_lifetimes.contains_key(&transport_id)
        {
            return false;
        }
        self.transport_lifetimes
            .insert(transport_id, Arc::downgrade(identity));
        true
    }

    pub(crate) fn transport_lifetime_matches(
        &self,
        delivery_id: A10DeliveryId,
        transport_id: ObjectId,
        identity: &Arc<()>,
    ) -> bool {
        self.transport_bindings.get(&transport_id) == Some(&delivery_id)
            && self
                .transport_lifetimes
                .get(&transport_id)
                .is_some_and(|saved| saved.ptr_eq(&Arc::downgrade(identity)))
    }

    pub(crate) fn transport_lifetime_matches_current_actor(
        &self,
        transport_id: ObjectId,
        identity: &Arc<()>,
    ) -> bool {
        self.transport_lifetimes
            .get(&transport_id)
            .is_some_and(|saved| saved.ptr_eq(&Arc::downgrade(identity)))
    }

    /// Reattach a serialized live-original marker to the reconstructed actor.
    pub(crate) fn restore_transport_lifetime(
        &mut self,
        transport_id: ObjectId,
        identity: &Arc<()>,
    ) -> bool {
        if !self.transport_bindings.contains_key(&transport_id)
            || self.transport_lifetimes.contains_key(&transport_id)
        {
            return false;
        }
        self.transport_lifetimes
            .insert(transport_id, Arc::downgrade(identity));
        true
    }

    pub(crate) fn bind_missile(
        &mut self,
        delivery_id: A10DeliveryId,
        transport_id: ObjectId,
        missile_id: ObjectId,
    ) -> bool {
        if self.missile_bindings.contains_key(&missile_id) {
            return false;
        }
        let Some(delivery) = self.deliveries.get_mut(&delivery_id) else {
            return false;
        };
        if !delivery.transport_ids.contains(&transport_id)
            || self.transport_bindings.get(&transport_id) != Some(&delivery_id)
        {
            return false;
        }
        delivery.active_missiles.insert(missile_id);
        delivery.missiles_scheduled = delivery.missiles_scheduled.saturating_add(1);
        self.missile_bindings.insert(missile_id, delivery_id);
        self.missiles_scheduled = self.missiles_scheduled.saturating_add(1);
        self.missiles_dropped = self.missiles_dropped.saturating_add(1);
        true
    }

    pub(crate) fn delivery_for_transport(&self, id: ObjectId) -> Option<A10DeliveryId> {
        self.transport_bindings.get(&id).copied()
    }
    pub(crate) fn delivery_for_missile(&self, id: ObjectId) -> Option<A10DeliveryId> {
        self.missile_bindings.get(&id).copied()
    }
    pub(crate) fn delivery_strike_id(&self, id: A10DeliveryId) -> Option<u32> {
        self.deliveries.get(&id).and_then(|row| row.strike_id)
    }
    pub(crate) fn delivery_transport_ids(&self, id: A10DeliveryId) -> Vec<ObjectId> {
        self.deliveries
            .get(&id)
            .map(|row| row.transport_ids.iter().copied().collect())
            .unwrap_or_default()
    }
    pub(crate) fn delivery_missile_ids(&self, id: A10DeliveryId) -> Vec<ObjectId> {
        self.missile_bindings
            .iter()
            .filter_map(|(object, bound)| (*bound == id).then_some(*object))
            .collect()
    }
    pub(crate) fn finish_delivery_launch(&mut self, id: A10DeliveryId) -> bool {
        let Some(row) = self.deliveries.get_mut(&id) else {
            return false;
        };
        if row.launch_complete {
            return false;
        }
        row.launch_complete = true;
        true
    }
    pub(crate) fn delivery_is_final(&self, id: A10DeliveryId) -> bool {
        self.deliveries.get(&id).is_some_and(|row| {
            row.launch_complete
                && row.active_transports.is_empty()
                && row.active_missiles.is_empty()
        })
    }
    pub(crate) fn delivery_is_completed(&self, id: A10DeliveryId) -> bool {
        self.deliveries.get(&id).is_some_and(|row| row.completed)
    }
    pub(crate) fn finalizable_deliveries(&self) -> Vec<A10DeliveryId> {
        self.deliveries
            .iter()
            .filter_map(|(id, row)| {
                (row.launch_complete
                    && !row.completed
                    && row.active_transports.is_empty()
                    && row.active_missiles.is_empty())
                .then_some(*id)
            })
            .collect()
    }

    pub(crate) fn delivery_ids(&self) -> Vec<A10DeliveryId> {
        self.deliveries.keys().copied().collect()
    }

    /// Remove an actor from the work set while keeping its historical binding.
    pub(crate) fn note_transport_payload_exhausted(
        &mut self,
        id: A10DeliveryId,
        actor: ObjectId,
    ) -> bool {
        if self.transport_bindings.get(&actor) != Some(&id) {
            return false;
        }
        self.deliveries
            .get_mut(&id)
            .is_some_and(|row| row.active_transports.remove(&actor))
    }
    pub(crate) fn note_transport_exit(&mut self, id: A10DeliveryId, actor: ObjectId) -> bool {
        self.note_transport_payload_exhausted(id, actor)
    }
    pub(crate) fn complete_visible_payload_attempt(
        &mut self,
        id: A10DeliveryId,
        actor: ObjectId,
    ) -> bool {
        if self.transport_bindings.get(&actor) != Some(&id) {
            return false;
        }
        let Some(row) = self.deliveries.get_mut(&id) else {
            return false;
        };
        row.payload_attempts_consumed = row.payload_attempts_consumed.saturating_add(1);
        true
    }
    pub(crate) fn note_missile_impact(&mut self, id: A10DeliveryId, actor: ObjectId) -> bool {
        if self.missile_bindings.get(&actor) != Some(&id) {
            return false;
        }
        let Some(row) = self.deliveries.get_mut(&id) else {
            return false;
        };
        if !row.active_missiles.remove(&actor) {
            return false;
        }
        row.impacts = row.impacts.saturating_add(1);
        self.impacts = self.impacts.saturating_add(1);
        true
    }
    pub(crate) fn note_missile_retired_without_impact(
        &mut self,
        id: A10DeliveryId,
        actor: ObjectId,
    ) -> bool {
        if self.missile_bindings.get(&actor) != Some(&id) {
            return false;
        }
        self.deliveries
            .get_mut(&id)
            .is_some_and(|row| row.active_missiles.remove(&actor))
    }
    /// Newly-completed result preserves the distinction between standalone and
    /// strike-bound deliveries; the outer None means not a new completion.
    pub(crate) fn complete_delivery_if_final(&mut self, id: A10DeliveryId) -> Option<Option<u32>> {
        if !self.delivery_is_final(id) {
            return None;
        }
        let row = self.deliveries.get_mut(&id)?;
        if row.completed {
            return None;
        }
        row.completed = true;
        Some(row.strike_id)
    }

    pub fn record_transport(&mut self) {
        self.transports_spawned = self.transports_spawned.saturating_add(1);
    }

    pub fn record_drop(&mut self) {
        self.missiles_dropped = self.missiles_dropped.saturating_add(1);
    }

    pub fn record_impact(&mut self) {
        self.impacts = self.impacts.saturating_add(1);
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.transports_spawned > 0 || self.missiles_scheduled > 0 || self.impacts > 0
    }
}

pub fn honesty_a10_strike_flight_residual_ok() -> bool {
    A10_TRANSPORT == "AmericaJetA10Thunderbolt"
        && A10_PAYLOAD_TEMPLATE == "A10ThunderboltMissile"
        && (A10_MISSILE_PRIMARY_DAMAGE - 200.0).abs() < 0.1
        && (A10_MISSILE_PRIMARY_RADIUS - 50.0).abs() < 0.1
        && A10_DROP_DELAY_FRAMES == 15
        && A10_ITEMS_PER_DROP == 2
        && A10_NUM_BONES == 6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_honesty_keeps_cpp_payload_constants() {
        assert!(honesty_a10_strike_flight_residual_ok());
    }

    #[test]
    fn close_enough_matches_cpp_delivery_distance() {
        // C++ DeliverPayloadAIUpdate.cpp:348-368 — 2D DeliveryDistance 450.
        let mut data = HostA10StrikeFlightData::start(
            Vec3::new(0.0, 160.0, 0.0),
            Vec3::new(1000.0, 0.0, 0.0),
            A10StrikeScienceTier::Level1,
        );
        assert!(
            !data.is_close_enough_to_target(Vec3::new(0.0, 160.0, 0.0)),
            "1000wu from target must not deliver"
        );
        assert!(
            data.is_close_enough_to_target(Vec3::new(600.0, 160.0, 0.0)),
            "400wu inbound must deliver"
        );
        // C++ strict boundary: exactly DeliveryDistance is outside the band.
        assert!(
            !data.is_close_enough_to_target(Vec3::new(550.0, 160.0, 0.0)),
            "exactly 450wu is outside the strict C++ band"
        );
        assert_eq!(data.take_visible_payload_drops(0), 2);
        assert_eq!(data.take_visible_payload_drops(1), 0);
        assert_eq!(data.take_visible_payload_drops(A10_DROP_DELAY_FRAMES), 0);
        assert_eq!(
            data.take_visible_payload_drops(A10_DROP_DELAY_FRAMES + 1),
            2
        );
    }

    #[test]
    fn head_off_map_flies_past_target_then_destroys() {
        // C++ HeadOffMapState + CleanUpState: fly HUGE_DIST, destroy when isOffMap.
        let mut data = HostA10StrikeFlightData::start_with_exit(
            Vec3::new(0.0, 160.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            Vec3::ZERO,
            A10StrikeScienceTier::Level1,
        );
        data.map_min = Vec3::new(0.0, 0.0, 0.0);
        data.map_max = Vec3::new(200.0, 0.0, 200.0);
        let mut pos = Vec3::new(98.0, 160.0, 0.0);
        let mut left = false;
        let mut destroyed = false;
        for _ in 0..80 {
            let (next, vel, at_exit) = data.tick_transport(pos);
            assert!(
                vel.x > 0.0 || at_exit,
                "must keep flying past the target, pos={next:?}"
            );
            pos = next;
            if pos.x > 100.0 {
                left = true;
            }
            if at_exit {
                destroyed = true;
                break;
            }
        }
        assert!(left, "jet must leave the target, pos={pos:?}");
        assert!(
            destroyed,
            "C++ isOffMap must destroy the transport, pos={pos:?}"
        );
        assert!(pos.x > 200.0 || pos.x < 0.0 || pos.z < 0.0 || pos.z > 200.0);
    }

    #[test]
    fn per_delivery_bindings_survive_actor_retirement_and_wire_roundtrip() {
        let mut registry = HostA10StrikeFlightRegistry::new();
        let delivery = registry
            .begin_delivery(ObjectId(10), A10StrikeScienceTier::Level1, Some(44))
            .unwrap();
        assert!(registry.bind_transport(delivery, ObjectId(20)));
        assert!(registry.finish_delivery_launch(delivery));
        assert!(registry.bind_missile(delivery, ObjectId(20), ObjectId(30)));
        assert!(registry.note_transport_exit(delivery, ObjectId(20)));
        assert_eq!(registry.delivery_for_missile(ObjectId(30)), Some(delivery));
        assert_eq!(registry.delivery_strike_id(delivery), Some(44));
        assert!(!registry.delivery_is_final(delivery));

        let bytes = bincode_legacy::serialize(&registry).unwrap();
        let mut restored: HostA10StrikeFlightRegistry =
            bincode_legacy::deserialize(&bytes).unwrap();
        assert_eq!(
            restored.delivery_for_transport(ObjectId(20)),
            Some(delivery)
        );
        assert_eq!(restored.delivery_for_missile(ObjectId(30)), Some(delivery));
        assert!(restored.note_missile_impact(delivery, ObjectId(30)));
        assert!(!restored.note_missile_impact(delivery, ObjectId(30)));
        assert_eq!(
            restored.complete_delivery_if_final(delivery),
            Some(Some(44))
        );
        assert_eq!(restored.complete_delivery_if_final(delivery), None);
    }

    #[test]
    fn deliveries_do_not_share_completion_or_strike_identity() {
        let mut registry = HostA10StrikeFlightRegistry::new();
        let first = registry
            .begin_delivery(ObjectId(1), A10StrikeScienceTier::Level1, Some(70))
            .unwrap();
        let second = registry
            .begin_delivery(ObjectId(2), A10StrikeScienceTier::Level2, Some(71))
            .unwrap();
        assert!(registry.bind_transport(first, ObjectId(11)));
        assert!(registry.bind_transport(second, ObjectId(12)));
        assert!(registry.finish_delivery_launch(first));
        assert!(registry.finish_delivery_launch(second));
        assert!(registry.bind_missile(first, ObjectId(11), ObjectId(21)));
        assert!(registry.bind_missile(second, ObjectId(12), ObjectId(22)));
        assert!(registry.note_transport_exit(first, ObjectId(11)));
        assert!(registry.note_transport_exit(second, ObjectId(12)));
        assert!(registry.note_missile_impact(first, ObjectId(21)));
        assert!(registry.delivery_is_final(first));
        assert!(!registry.delivery_is_final(second));
        assert_eq!(registry.delivery_strike_id(first), Some(70));
        assert_eq!(registry.delivery_strike_id(second), Some(71));
    }
}
