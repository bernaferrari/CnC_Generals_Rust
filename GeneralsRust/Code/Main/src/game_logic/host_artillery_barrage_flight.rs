//! Per-carrier continuation for the artillery DeliverPayload order.

use game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL;
use gamelogic::common::{Coord3D, Region3D};
use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::host_deliver_payload::{
    RESIDUAL_MAP_EXTENT_MAX_X, RESIDUAL_MAP_EXTENT_MAX_Z, RESIDUAL_MAP_EXTENT_MIN_X,
    RESIDUAL_MAP_EXTENT_MIN_Z, head_off_map_exit_point_residual,
    is_close_enough_to_target_squared_residual, is_off_map_residual,
};
use crate::game_logic::special_power_strikes::{
    ARTILLERY_BARRAGE_DAMAGE, ARTILLERY_BARRAGE_DELIVERY_DISTANCE,
    ARTILLERY_BARRAGE_PRE_OPEN_DISTANCE, ARTILLERY_BARRAGE_RADIUS, ARTILLERY_BARRAGE_SHELL_OBJECT,
    ARTILLERY_BARRAGE_TRANSPORT, ArtilleryBarrageScienceTier, artillery_barrage_points_for_tier,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VisibleArtilleryPayload {
    pub object: String,
    pub weapon: String,
    pub bone: String,
    pub subobject: String,
    pub inherit_velocity: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum ArtilleryPayload {
    Visible(VisibleArtilleryPayload),
    TemplateOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum ArtilleryDeliveryPhase {
    Approach,
    Delivering { frames_left: u32 },
    Reapproach { destination: Vec3 },
    RecoveringOffMap { reentry_frame: u32 },
    HeadOffMap { destination: Vec3 },
}

/// The visible-payload delivery state is owned and saved by its carrier.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthoredArtilleryDelivery {
    phase: ArtilleryDeliveryPhase,
    door_delay: u32,
    drop_delay: u32,
    attempts_left: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtilleryDeliveryEvent {
    Continue,
    OpenDoor,
    Release,
    CloseDoor,
    BeginRecovery { close_door: bool },
    Reenter,
    Depart,
}

/// Original TerrainLogic queries for the active and border-inclusive extents.
#[derive(Clone, Copy)]
pub(crate) struct ArtilleryMapExtents {
    pub playable: Region3D,
    pub with_border: Region3D,
}

impl ArtilleryMapExtents {
    fn is_off_map(self, position: Vec3) -> bool {
        !self
            .with_border
            .is_in_region_no_z(&Coord3D::new(position.x, position.z, position.y))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ArtilleryFlightFrame {
    pub frame: u32,
    pub position: Vec3,
    pub heading: f32,
    /// Receiving locomotor limits in distance and radians per second.
    pub maximum_speed: f32,
    pub maximum_turn_rate: f32,
    pub arrival_distance: f32,
    pub map: Option<ArtilleryMapExtents>,
}

impl ArtilleryFlightFrame {
    fn is_off_map(self) -> bool {
        self.map.is_some_and(|map| map.is_off_map(self.position))
    }

    fn turn_radius(self) -> f32 {
        if self.maximum_turn_rate > 0.0 {
            let speed = self.maximum_speed * SECONDS_PER_LOGICFRAME_REAL;
            let turn = self.maximum_turn_rate * SECONDS_PER_LOGICFRAME_REAL;
            speed / turn
        } else {
            999_999.0
        }
    }

    fn reentry_frame(self) -> u32 {
        // C++ calcMinTurnRadius supplies the travel time in logic frames.
        let speed = self.maximum_speed * SECONDS_PER_LOGICFRAME_REAL;
        let delay = if speed > 0.0 {
            (self.turn_radius() / speed).ceil() as u32
        } else {
            0
        };
        self.frame.wrapping_add(delay)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostArtilleryBarrageFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub tier: ArtilleryBarrageScienceTier,
    pub(crate) aim: Vec3,
    /// The one visible payload is consumed on its release attempt.
    pub(crate) payload: Option<ArtilleryPayload>,
    pub(crate) strike_id: Option<u32>,
    pub(crate) delivery_distance: f32,
    pub(crate) pre_open_distance: f32,
    pub(crate) speed_per_frame: f32,
    pub(crate) self_destruct_on_departure: bool,
    /// Previous 2D dist² for C++ isCloseEnoughToTarget inbound tracking
    /// (DeliverPayloadAIUpdate.cpp:99,356-357; starts at 0 like m_previousDistanceSqr).
    #[serde(default)]
    pub prev_dist_sq: f32,
    /// Map extent for C++ `isOffMap` / HeadOffMap HUGE_DIST (world_min/max).
    #[serde(default)]
    pub map_min: Vec3,
    #[serde(default)]
    pub map_max: Vec3,
    /// C++ setDisabledUntil(DISABLED_DEFAULT, now + GameLogicRandomValue(0, DelayDeliveryMax)).
    #[serde(default)]
    pub delay_until_frame: u32,
    authored: Option<AuthoredArtilleryDelivery>,
    wing_physics: bool,
}

impl HostArtilleryBarrageFlightData {
    pub fn start(launch: Vec3, target: Vec3, tier: ArtilleryBarrageScienceTier) -> Self {
        Self {
            target,
            launch,
            tier,
            aim: target,
            payload: Some(ArtilleryPayload::TemplateOnly),
            strike_id: None,
            delivery_distance: ARTILLERY_BARRAGE_DELIVERY_DISTANCE,
            pre_open_distance: ARTILLERY_BARRAGE_PRE_OPEN_DISTANCE,
            speed_per_frame: 14.0,
            self_destruct_on_departure: false,
            prev_dist_sq: 0.0,
            map_min: Vec3::ZERO,
            map_max: Vec3::ZERO,
            delay_until_frame: 0,
            authored: None,
            wing_physics: false,
        }
    }

    /// C++ DISABLED_DEFAULT hold until DelayDeliveryMax elapses.
    pub fn is_hold_for_delay(&self, frame: u32) -> bool {
        self.delay_until_frame > frame
    }

    pub(crate) fn start_authored_delivery(
        &mut self,
        door_delay: u32,
        drop_delay: u32,
        max_attempts: i32,
    ) {
        self.authored = Some(AuthoredArtilleryDelivery {
            phase: ArtilleryDeliveryPhase::Approach,
            door_delay,
            drop_delay,
            attempts_left: max_attempts.max(0),
        });
    }

    pub(crate) fn is_authored_delivery(&self) -> bool {
        self.authored.is_some()
    }

    pub(crate) fn enable_wing_physics(&mut self) {
        self.wing_physics = true;
    }

    pub(crate) fn uses_wing_physics(&self) -> bool {
        self.wing_physics
    }

    pub fn map_extent_ok(&self) -> bool {
        self.map_max.x > self.map_min.x && self.map_max.z > self.map_min.z
    }

    /// C++ DeliveringState `isCloseEnoughToTarget` residual (live band):
    /// authored DeliveryDistance 250, PreOpenDistance 0; inbound expands the
    /// allowed band to the sum.
    pub fn in_delivery_band(&mut self, pos: Vec3) -> bool {
        let dx = self.aim.x - pos.x;
        let dz = self.aim.z - pos.z;
        let current = dx * dx + dz * dz;
        let previous = self.prev_dist_sq;
        self.prev_dist_sq = current;
        is_close_enough_to_target_squared_residual(
            current,
            previous,
            self.delivery_distance,
            self.pre_open_distance,
        )
    }

    /// DeliverPayloadAIUpdate.cpp:581–703, 893–985, 1115–1148.
    /// State entry runs on the transition frame. Its first update runs on the
    /// next frame, and a nonzero countdown consumes that whole update.
    pub(crate) fn advance_delivery(
        &mut self,
        flight: ArtilleryFlightFrame,
    ) -> ArtilleryDeliveryEvent {
        use ArtilleryDeliveryEvent as Event;
        use ArtilleryDeliveryPhase as Phase;

        let pos = flight.position;
        let Some(phase) = self.authored.as_ref().map(|state| state.phase) else {
            return if self.payload.is_none() && self.self_destruct_on_departure {
                Event::Depart
            } else if self.payload.is_some() && self.in_delivery_band(pos) {
                Event::Release
            } else {
                Event::Continue
            };
        };
        match phase {
            Phase::Approach if self.in_delivery_band(pos) => {
                let state = self.authored.as_mut().expect("authored delivery");
                state.phase = Phase::Delivering {
                    frames_left: state.door_delay,
                };
                Event::OpenDoor
            }
            Phase::Delivering { frames_left } if frames_left > 0 => {
                self.authored.as_mut().expect("authored delivery").phase = Phase::Delivering {
                    frames_left: frames_left - 1,
                };
                Event::Continue
            }
            Phase::Delivering { .. } => {
                let in_band = self.in_delivery_band(pos);
                let state = self.authored.as_mut().expect("authored delivery");
                if in_band && self.payload.is_some() {
                    state.phase = Phase::Delivering {
                        frames_left: state.drop_delay,
                    };
                    return Event::Release;
                }
                let (sin, cos) = flight.heading.sin_cos();
                if !in_band && state.attempts_left > 0 {
                    state.attempts_left -= 1;
                    if flight.is_off_map() {
                        // ConsiderNewApproach's condition runs on entry too.
                        state.phase = Phase::RecoveringOffMap {
                            reentry_frame: flight.reentry_frame(),
                        };
                        return Event::BeginRecovery { close_door: true };
                    }
                    state.phase = Phase::Reapproach {
                        destination: pos + Vec3::new(cos, 0.0, sin) * (2.2 * flight.turn_radius()),
                    };
                    return Event::CloseDoor;
                }
                let (min_x, min_z, max_x, max_z) = self.map_edges(flight.map);
                let destination =
                    head_off_map_exit_point_residual(pos, cos, sin, min_x, min_z, max_x, max_z);
                self.authored.as_mut().expect("authored delivery").phase =
                    Phase::HeadOffMap { destination };
                Event::Depart
            }
            Phase::Reapproach { destination } => {
                // This carrier's movement stops at its waypoint. Enter the
                // next Approach without updating it a second time this frame.
                let offset = destination - pos;
                let arrival_distance = if self.wing_physics {
                    flight.arrival_distance
                } else {
                    1.0
                };
                if offset.x * offset.x + offset.z * offset.z < arrival_distance * arrival_distance {
                    self.authored.as_mut().expect("authored delivery").phase = Phase::Approach;
                } else if flight.is_off_map() {
                    self.authored.as_mut().expect("authored delivery").phase =
                        Phase::RecoveringOffMap {
                            reentry_frame: flight.reentry_frame(),
                        };
                    return Event::BeginRecovery { close_door: false };
                }
                Event::Continue
            }
            Phase::RecoveringOffMap { reentry_frame } if flight.frame >= reentry_frame => {
                self.authored.as_mut().expect("authored delivery").phase = Phase::Approach;
                Event::Reenter
            }
            Phase::Approach | Phase::RecoveringOffMap { .. } | Phase::HeadOffMap { .. } => {
                Event::Continue
            }
        }
    }

    pub(crate) fn may_release_payload(&self) -> bool {
        self.payload.is_some()
            && !self.authored.as_ref().is_some_and(|state| {
                matches!(state.phase, ArtilleryDeliveryPhase::HeadOffMap { .. })
            })
    }

    pub(crate) fn is_recovering_off_map(&self) -> bool {
        self.authored.as_ref().is_some_and(|state| {
            matches!(state.phase, ArtilleryDeliveryPhase::RecoveringOffMap { .. })
        })
    }

    fn map_edges(&self, map: Option<ArtilleryMapExtents>) -> (f32, f32, f32, f32) {
        if let Some(ArtilleryMapExtents { playable, .. }) = map {
            (playable.lo.x, playable.lo.y, playable.hi.x, playable.hi.y)
        } else if self.map_extent_ok() {
            (
                self.map_min.x,
                self.map_min.z,
                self.map_max.x,
                self.map_max.z,
            )
        } else {
            (
                RESIDUAL_MAP_EXTENT_MIN_X,
                RESIDUAL_MAP_EXTENT_MIN_Z,
                RESIDUAL_MAP_EXTENT_MAX_X,
                RESIDUAL_MAP_EXTENT_MAX_Z,
            )
        }
    }

    /// Current move order and whether HeadOffMap can enter CleanUp.
    pub(crate) fn course(&self, pos: Vec3, map: Option<ArtilleryMapExtents>) -> (Vec3, bool) {
        let (min_x, min_z, max_x, max_z) = self.map_edges(map);
        let (dest, departing) = match self.authored.as_ref().map(|state| state.phase) {
            Some(ArtilleryDeliveryPhase::Reapproach { destination }) => (destination, false),
            Some(ArtilleryDeliveryPhase::HeadOffMap { destination }) => (destination, true),
            None if self.payload.is_none() => {
                let direction = self.target - self.launch;
                (
                    head_off_map_exit_point_residual(
                        pos,
                        direction.x,
                        direction.z,
                        min_x,
                        min_z,
                        max_x,
                        max_z,
                    ),
                    true,
                )
            }
            _ => (self.target, false),
        };
        let at_exit = departing
            && map.map_or_else(
                || is_off_map_residual(pos, min_x, min_z, max_x, max_z),
                |extent| extent.is_off_map(pos),
            );
        (dest, at_exit)
    }

    /// Movement for deliveries whose template has no receiving wing physics.
    pub(crate) fn planar_step(&self, pos: Vec3, destination: Vec3) -> Vec3 {
        let offset = (destination - pos).with_y(0.0);
        let distance = offset.length();
        if distance >= 1.0 {
            pos + offset * (self.speed_per_frame.min(distance) / distance)
        } else {
            pos
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostArtilleryBarrageFlightRegistry {
    pub transports_spawned: u32,
    pub shells_dropped: u32,
    pub impacts: u32,
}

impl HostArtilleryBarrageFlightRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_transport(&mut self) {
        self.transports_spawned = self.transports_spawned.saturating_add(1);
    }

    pub fn record_drop(&mut self) {
        self.shells_dropped = self.shells_dropped.saturating_add(1);
    }

    pub fn record_impact(&mut self) {
        self.impacts = self.impacts.saturating_add(1);
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.transports_spawned > 0 || self.impacts > 0
    }
}

pub fn honesty_artillery_barrage_flight_residual_ok() -> bool {
    ARTILLERY_BARRAGE_TRANSPORT == "ChinaArtilleryCannon"
        && ARTILLERY_BARRAGE_SHELL_OBJECT == "ChinaArtilleryBarrageShell"
        && (ARTILLERY_BARRAGE_DAMAGE - 105.0).abs() < 0.1
        && (ARTILLERY_BARRAGE_RADIUS - 50.0).abs() < 0.1
        && artillery_barrage_points_for_tier(Vec3::ZERO, ArtilleryBarrageScienceTier::Level1).len()
            == 12
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_band_is_authored_250_not_residual_80() {
        // C++ DeliveringState isCloseEnoughToTarget uses the authored
        // DeliveryDistance 250 (SUPERWEAPON_ArtilleryBarrage1), not the
        // mis-ported 80; exactly 250 is outside the strict band.
        let mut data = HostArtilleryBarrageFlightData::start(
            Vec3::new(0.0, 130.0, 0.0),
            Vec3::new(500.0, 0.0, 0.0),
            ArtilleryBarrageScienceTier::Level1,
        );
        assert!(
            data.in_delivery_band(Vec3::new(400.0, 130.0, 0.0)),
            "100wu inside the authored 250 DeliveryDistance band must deliver"
        );
        assert!(
            data.in_delivery_band(Vec3::new(251.0, 130.0, 0.0)),
            "249wu inside the authored 250 DeliveryDistance band must deliver"
        );
        assert!(
            !data.in_delivery_band(Vec3::new(500.0 - 251.0, 130.0, 0.0)),
            "251wu outside the authored 250 DeliveryDistance band must not deliver"
        );
    }
    #[test]
    fn a_missed_delivery_reopens_once_before_leaving_with_its_payload() {
        use ArtilleryDeliveryEvent as Event;

        let mut flight = HostArtilleryBarrageFlightData::start(
            Vec3::ZERO,
            Vec3::ZERO,
            ArtilleryBarrageScienceTier::Level1,
        );
        flight.delivery_distance = 100.0;
        flight.pre_open_distance = 0.0;
        flight.start_authored_delivery(0, 0, 1);
        let in_range = Vec3::new(40.0, 0.0, 0.0);
        let outside = Vec3::new(101.0, 0.0, 0.0);
        let retry_point = outside + Vec3::X * 22.0;
        let frame_at = |position| ArtilleryFlightFrame {
            frame: 0,
            position,
            heading: 0.0,
            maximum_speed: 10.0,
            maximum_turn_rate: 1.0,
            arrival_distance: 2.0,
            map: None,
        };
        assert_eq!(flight.advance_delivery(frame_at(in_range)), Event::OpenDoor);
        assert_eq!(flight.advance_delivery(frame_at(outside)), Event::CloseDoor);
        assert!(flight.may_release_payload());
        let (goal, _) = flight.course(outside, None);
        let next = flight.planar_step(outside, goal);
        assert!(
            next.x > outside.x,
            "retry moves forward to make room to turn"
        );
        assert_eq!(
            flight.advance_delivery(frame_at(retry_point)),
            Event::Continue
        );
        assert_eq!(flight.advance_delivery(frame_at(in_range)), Event::OpenDoor);
        assert_eq!(flight.advance_delivery(frame_at(outside)), Event::Depart);
        assert!(!flight.may_release_payload());
    }
}
