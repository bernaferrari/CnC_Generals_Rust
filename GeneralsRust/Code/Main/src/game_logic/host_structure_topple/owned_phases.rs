//! CPP StructureToppleUpdate.cpp194–296. Each returned effect phase is
//! executed immediately by the driving GameLogic after releasing its Object.
use super::*;

impl HostStructureToppleData {
    pub(super) fn prepare_owned_update(
        &mut self,
        frame: u32,
    ) -> (bool, Vec<StructureToppleFxEvent>) {
        let due = self.prepare_owned_waiting_fx(frame);
        self.finish_owned_waiting(frame, due);
        self.prepare_owned_angle_fx()
    }
    pub(crate) fn prepare_owned_waiting_fx(&mut self, frame: u32) -> bool {
        if self.state != HostStructureToppleState::WaitingForStart {
            return false;
        }
        self.queue_owned_burst_fx(frame)
    }
    pub(crate) fn finish_owned_waiting(&mut self, frame: u32, burst_due: bool) {
        if self.state != HostStructureToppleState::WaitingForStart {
            return;
        }
        if burst_due {
            self.finish_owned_burst(frame);
        }
        if frame >= self.topple_start_frame {
            self.state = HostStructureToppleState::Toppling;
        }
    }
    pub(crate) fn prepare_owned_angle_fx(&mut self) -> (bool, Vec<StructureToppleFxEvent>) {
        let toppling = self.state == HostStructureToppleState::Toppling;
        if toppling {
            // Original 0.001 constructor angle supplies the initial nudge;
            // there is no velocity kick or minimum acceleration.
            self.topple_velocity += STRUCTURE_TOPPLE_ACCEL_FACTOR
                * self.accumulated_angle.sin()
                * (1.0 - self.structural_integrity);
            if self.structural_integrity > 0.0 {
                self.structural_integrity *= self.structural_decay;
                if self.structural_integrity < 0.0 {
                    self.structural_integrity = 0.0;
                }
            }
            self.queue_angle_crossings(
                self.accumulated_angle,
                self.accumulated_angle + self.topple_velocity,
            );
            self.last_polled_angle = self.accumulated_angle + self.topple_velocity;
        }
        (toppling, self.take_pending_fx())
    }
    pub(crate) fn advance_owned_angle(&mut self) {
        self.accumulated_angle += self.topple_velocity;
    }
    /// Runs only after the first crushing callback. Final crushing and Done FX
    /// occur before this update's local transform rotation.
    pub(crate) fn finish_owned_angle(&mut self, frame: u32) -> bool {
        if self.accumulated_angle < std::f32::consts::FRAC_PI_2 {
            return false;
        }
        self.topple_velocity -= self.accumulated_angle - std::f32::consts::FRAC_PI_2;
        self.accumulated_angle = std::f32::consts::FRAC_PI_2;
        self.state = HostStructureToppleState::WaitingForDone;
        self.done_frame = frame;
        if !self.done_fx_played {
            self.queue_obj_fx(&self.topple_done_fx.clone());
            self.done_fx_played = true;
        }
        true
    }
    pub(crate) fn queue_owned_burst_fx(&mut self, frame: u32) -> bool {
        if frame < self.next_burst_frame {
            return false;
        }
        self.queue_pos_fx(
            &self.topple_delay_fx.clone(),
            self.delay_burst_x,
            self.delay_burst_y,
            self.delay_burst_z,
        );
        true
    }
    pub(crate) fn finish_owned_burst(&mut self, frame: u32) {
        self.next_burst_frame = frame.wrapping_add(self.burst_delay_frames(frame));
    }
    pub(crate) fn queue_owned_toppling_burst(&mut self, frame: u32) {
        if self.queue_owned_burst_fx(frame) {
            self.finish_owned_burst(frame);
        }
    }
    pub(crate) fn owned_completion_is_due(&self, frame: u32) -> bool {
        self.state == HostStructureToppleState::WaitingForDone && self.done_frame <= frame
    }
    pub(crate) fn complete_owned_update(&mut self, frame: u32) -> bool {
        if self.state == HostStructureToppleState::WaitingForDone && self.done_frame <= frame {
            self.state = HostStructureToppleState::Done;
            return true;
        }
        false
    }
}
