//! CPP PhysicsUpdate.cpp403–417: reset first, then synchronously set wake.
//! The installed descriptor releases its module before the actual scheduler
//! records that module's wake; this is a bounded continuation, never frame-end work.
use super::*;

pub(in crate::object) struct PhysicsResetWake {
    object_id: ObjectID,
    registrations: Vec<crate::modules::UpdateModulePtr>,
    wake: UnsignedInt,
}
impl PhysicsResetWake {
    pub(in crate::object) fn apply(self) {
        for module in self.registrations {
            if let Err(error) =
                TheGameLogic::register_update_module(self.object_id, module, self.wake)
            {
                log::error!(
                    "Physics reset wake registration failed for {}: {error}",
                    self.object_id
                );
            }
        }
    }
}
impl PhysicsBehaviorHandle {
    pub(super) fn prepare_dynamic_physics_reset(&mut self) -> Option<PhysicsResetWake> {
        self.state.accel = Coord3D::ZERO;
        self.state.prev_accel = Coord3D::ZERO;
        self.state.vel = Coord3D::ZERO;
        self.state.vel_mag = 0.0;
        self.state.turning = 0;
        self.state.yaw_rate = 0.0;
        self.state.roll_rate = 0.0;
        self.state.pitch_rate = 0.0;
        // PhysicsUpdate.cpp413 clears the flag without erasing stored angles.
        self.state.set_flag(FLAG_HAS_PITCHROLLYAW, false);
        if let Some(object) = self.object_arc() {
            if let Ok(guard) = object.read() {
                let now = TheGameLogic::get_frame();
                let zero_vel =
                    self.state.vel.x == 0.0 && self.state.vel.y == 0.0 && self.state.vel.z == 0.0;
                let zero_accel = self.state.accel.x == 0.0
                    && self.state.accel.y == 0.0
                    && self.state.accel.z == 0.0;
                let asleep = zero_vel
                    && zero_accel
                    && !self.state.has_flag(FLAG_HAS_PITCHROLLYAW)
                    && self.state.motive_force_expires <= now
                    && guard.get_layer() == crate::common::PathfindLayerEnum::Ground
                    && !guard.is_above_terrain()
                    && self.state.current_overlap == crate::common::INVALID_ID
                    && self.state.previous_overlap == crate::common::INVALID_ID
                    && self.state.has_flag(FLAG_UPDATE_EVER_RUN);
                let wake = if asleep {
                    UpdateSleepTime::Forever.to_u32()
                } else {
                    now.saturating_add(1)
                };
                return Some(PhysicsResetWake {
                    object_id: guard.get_id(),
                    registrations: guard.named_update_registrations("PhysicsBehavior"),
                    wake,
                });
            }
        }
        None
    }
}
impl PhysicsBehaviorUpdate {
    pub(in crate::object) fn reset_dynamic_physics_borrowed(&mut self) -> Option<PhysicsResetWake> {
        self.physics_handle.prepare_dynamic_physics_reset()
    }
}
