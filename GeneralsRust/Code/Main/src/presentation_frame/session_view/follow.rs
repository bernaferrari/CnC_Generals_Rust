//! W3DView's ordinary object follow: before scalar motion, independently of pause.
//! Frozen identity/facts enter once; this session owns position, basis and snap.
use super::*;
use crate::game_logic::ObjectId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FollowTargetFacts {
    pub(crate) id: ObjectId,
    pub(crate) position: Vec3,
    /// Present only for source selected airborne locomotor above terrain/water.
    pub(crate) airborne_orientation: Option<f32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FollowAdmission {
    Inactive,
    AwaitingFrame,
    TargetGone,
    Followed,
}
fn normalize(angle: &mut f32) {
    if *angle < -10. * std::f32::consts::PI {
        *angle = 0.;
    }
    if *angle > 10. * std::f32::consts::PI {
        *angle = 0.;
    }
    while *angle < -std::f32::consts::PI {
        *angle += 2. * std::f32::consts::PI;
    }
    while *angle > std::f32::consts::PI {
        *angle -= 2. * std::f32::consts::PI;
    }
}
impl SessionView {
    /// Raw setCameraLock resets Follow style/distance without LookAt.
    /// It never clears the source snapImmediate latch.
    pub(crate) fn set_camera_follow(&mut self, id: Option<ObjectId>) {
        self.scripted.camera_follow_target = id;
        self.scripted.camera_tether_play = None;
    }
    pub(crate) fn set_camera_tether(&mut self, id: ObjectId, snap: bool, play: f32) {
        self.scripted.camera_follow_target = Some(id);
        self.scripted.camera_tether_play = Some(play);
        if snap {
            self.camera.camera_follow_snap_immediate = true;
        }
    }
    /// Parent View admission cancels the path, follows the exact object, then
    /// admits independent scalar axes. No airborne LookAt projection occurs.
    pub(crate) fn admit_camera_follow(
        &mut self,
        observed_id: Option<ObjectId>,
        facts: Option<FollowTargetFacts>,
        partition_cell_size: f32,
        record_previous: bool,
    ) -> Result<FollowAdmission, &'static str> {
        let Some(id) = self.scripted.camera_follow_target else {
            self.camera.camera_follow_factor = -1.;
            return Ok(FollowAdmission::Inactive);
        };
        // A new lock must not consume a stale peer/prior-target freeze.
        if observed_id != Some(id) {
            return Ok(FollowAdmission::AwaitingFrame);
        }
        let Some(facts) = facts else {
            self.scripted.motion.active = false;
            self.scripted.path_arrived = false;
            self.set_camera_follow(None);
            self.camera.camera_follow_factor = -1.;
            return Ok(FollowAdmission::TargetGone);
        };
        if facts.id != id {
            return Err("follow facts do not match receiving lock identity");
        }
        if !facts.position.is_finite()
            || facts
                .airborne_orientation
                .is_some_and(|angle| !angle.is_finite())
        {
            return Err("invalid frozen follow target or partition cell");
        }
        // Prepare the same source arithmetic without touching the live owner.
        // Invalid/cold inputs must not cancel a path or consume a pending snap.
        let mut factor = self.camera.camera_follow_factor;
        if factor < 0. {
            factor = 0.05;
        } else {
            factor += 0.05;
            if factor > 1. {
                factor = 1.;
            }
        }
        let snap = self.camera.camera_follow_snap_immediate;
        let current = self.target;
        let mut next = current;
        if snap {
            next.x = facts.position.x;
            next.z = facts.position.z;
        } else {
            let dx = facts.position.x - current.x;
            let dz = facts.position.z - current.z;
            if let Some(play) = self.scripted.camera_tether_play {
                let threshold = partition_cell_size * partition_cell_size;
                if !threshold.is_finite() || threshold <= 0. {
                    return Err("invalid tether partition threshold");
                }
                let distance = dx * dx + dz * dz;
                if distance >= threshold {
                    let ratio = 1. - threshold / distance;
                    next.x += dx * ratio * 0.5;
                    next.z += dz * ratio * 0.5;
                } else {
                    let ratio = 0.01 * play;
                    next.x += dx * ratio;
                    next.z += dz * ratio;
                }
            } else {
                next.x += dx * factor;
                next.z += dz * factor;
            }
        }
        let mut angle = self.camera.camera_yaw_radians;
        if self.scripted.camera_tether_play.is_none() {
            if let Some(orientation) = facts.airborne_orientation {
                let mut ideal = (f64::from(orientation) - std::f64::consts::FRAC_PI_2) as f32;
                normalize(&mut ideal);
                let mut old = angle;
                normalize(&mut old);
                let mut diff = ideal - old;
                normalize(&mut diff);
                angle = if snap { ideal } else { angle + diff * 0.1 };
                normalize(&mut angle);
            }
        }
        if !next.is_finite() || !factor.is_finite() || !angle.is_finite() {
            return Err("nonfinite camera follow result");
        }
        self.scripted.motion.active = false;
        self.scripted.path_arrived = false;
        self.camera.camera_follow_factor = factor;
        #[cfg(feature = "game_client")]
        if record_previous {
            self.camera.native_filter_previous_target = current;
        }
        #[cfg(not(feature = "game_client"))]
        let _ = record_previous;
        self.target = next;
        self.camera.camera_yaw_radians = angle;
        self.camera.camera_follow_snap_immediate = false;
        self.camera
            .native_camera
            .set_follow_ground(facts.position.y);
        Ok(FollowAdmission::Followed)
    }
}

#[cfg(test)]
#[path = "follow_tests.rs"]
mod tests;
