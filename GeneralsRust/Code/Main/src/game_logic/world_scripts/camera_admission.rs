//! Admission of exact requests from the driving engine's own completed freeze.
//! The outer engine supplies its own last_presentation_frame synchronously;
//! a raw PresentationFrame is not a transferable session-identity token.
use super::super::*;

/// Exact frozen requests acknowledged at their receiving presentation boundary.
#[derive(Default)]
pub(crate) struct CameraRequestAdmission {
    pub focus: bool,
    pub reset: bool,
    pub zoom: bool,
    pub pitch: bool,
    pub rotate: bool,
    pub look: bool,
    pub slave_enable: bool,
    pub slave_disable: bool,
    pub screen_shakes: usize,
    pub shakers: usize,
}

impl GameLogic {
    pub(crate) fn acknowledge_frozen_camera_requests(
        &mut self,
        frame: &crate::presentation_frame::PresentationFrame,
    ) -> CameraRequestAdmission {
        let mut admitted = CameraRequestAdmission::default();
        if frame.camera_focus.is_some()
            && self.pending_camera_focus.map(|p| p.to_array()) == frame.camera_focus
        {
            admitted.focus = true;
            self.pending_camera_focus = None;
        }
        if frame
            .camera_zoom
            .map(|r| (r, frame.camera_zoom_ease))
            .is_some()
            && self.pending_camera_zoom.as_ref().map(|r| {
                (
                    (r.zoom, r.duration_seconds),
                    (r.ease_in_seconds, r.ease_out_seconds),
                )
            }) == frame.camera_zoom.map(|r| (r, frame.camera_zoom_ease))
        {
            admitted.zoom = true;
            self.pending_camera_zoom = None;
        }
        if frame
            .camera_pitch
            .map(|r| (r, frame.camera_pitch_ease))
            .is_some()
            && self.pending_camera_pitch.as_ref().map(|r| {
                (
                    (r.pitch, r.duration_seconds),
                    (r.ease_in_seconds, r.ease_out_seconds),
                )
            }) == frame.camera_pitch.map(|r| (r, frame.camera_pitch_ease))
        {
            admitted.pitch = true;
            self.pending_camera_pitch = None;
        }
        if frame
            .camera_rotate
            .map(|r| (r, frame.camera_rotate_ease))
            .is_some()
            && self.pending_camera_rotate.as_ref().map(|r| {
                (
                    (r.rotations, r.duration_seconds),
                    (r.ease_in_seconds, r.ease_out_seconds),
                )
            }) == frame.camera_rotate.map(|r| (r, frame.camera_rotate_ease))
        {
            admitted.rotate = true;
            self.pending_camera_rotate = None;
        }
        if frame
            .camera_look_toward
            .map(|p| {
                (
                    p,
                    frame.camera_look_toward_duration,
                    frame.camera_look_toward_ease.0,
                    frame.camera_look_toward_ease.1,
                    frame.camera_look_toward_reverse_rotation,
                )
            })
            .is_some()
            && self.pending_camera_look_toward.as_ref().map(|r| {
                (
                    r.position.to_array(),
                    r.duration_seconds,
                    r.ease_in_seconds,
                    r.ease_out_seconds,
                    r.reverse_rotation,
                )
            }) == frame.camera_look_toward.map(|p| {
                (
                    p,
                    frame.camera_look_toward_duration,
                    frame.camera_look_toward_ease.0,
                    frame.camera_look_toward_ease.1,
                    frame.camera_look_toward_reverse_rotation,
                )
            })
        {
            admitted.look = true;
            self.pending_camera_look_toward = None;
        }
        if frame
            .camera_slave_enable
            .as_ref()
            .map(|(a, b)| (a, b))
            .is_some()
            && self
                .pending_camera_slave_mode_enable
                .as_ref()
                .map(|r| (&r.thing_template_name, &r.bone_name))
                == frame.camera_slave_enable.as_ref().map(|(a, b)| (a, b))
        {
            admitted.slave_enable = true;
            self.pending_camera_slave_mode_enable = None;
        }
        if frame.camera_zoom_reset
            && self.pending_camera_zoom_reset
            && self.pending_camera_zoom_reset_duration == frame.camera_zoom_reset_duration
            && (
                self.pending_camera_zoom_reset_ease_in,
                self.pending_camera_zoom_reset_ease_out,
            ) == frame.camera_zoom_reset_ease
        {
            admitted.reset = true;
            self.take_camera_zoom_reset();
        }
        if frame.camera_slave_disable && self.pending_camera_slave_mode_disable {
            admitted.slave_disable = true;
            self.pending_camera_slave_mode_disable = false;
        }
        admitted.screen_shakes = self
            .pending_screen_shakes
            .iter()
            .zip(&frame.screen_shakes)
            .take_while(|(a, b)| a.intensity == **b)
            .count();
        self.pending_screen_shakes.drain(..admitted.screen_shakes);
        admitted.shakers = self
            .pending_camera_add_shakers
            .iter()
            .zip(&frame.camera_shakers)
            .take_while(|(a, b)| {
                (
                    a.position.to_array(),
                    a.amplitude,
                    a.duration_seconds,
                    a.radius,
                ) == **b
            })
            .count();
        self.pending_camera_add_shakers.drain(..admitted.shakers);
        admitted
    }
}

#[cfg(test)]
mod camera_request_admission_tests {
    use super::*;
    use crate::presentation_frame::PresentationFrame;
    #[test]
    fn frozen_camera_requests_apply_once_and_do_not_consume_newer_values() {
        let mut world = GameLogic::new();
        world.queue_pending_camera_zoom(0.8, 2.0);
        world.queue_pending_camera_pitch(1.0, 3.0);
        world.queue_pending_screen_shake(3);
        let frame = PresentationFrame::build_from_logic(&mut world, 0);
        let first = world.acknowledge_frozen_camera_requests(&frame);
        assert!(first.zoom && first.pitch);
        assert_eq!(first.screen_shakes, 1);
        let repeat = world.acknowledge_frozen_camera_requests(&frame);
        assert!(!repeat.zoom && !repeat.pitch);
        assert_eq!(repeat.screen_shakes, 0);
        world.queue_pending_camera_zoom(1.2, 5.0);
        assert!(!world.acknowledge_frozen_camera_requests(&frame).zoom);
        assert_eq!(world.peek_pending_camera_zoom().unwrap().zoom, 1.2);
    }
    #[test]
    fn frozen_camera_queue_ack_retains_new_tail_and_same_id_peer() {
        let mut world = GameLogic::new();
        let mut peer = GameLogic::new();
        world.queue_pending_screen_shake(1);
        peer.queue_pending_screen_shake(1);
        let frame = PresentationFrame::build_from_logic(&mut world, 0);
        world.queue_pending_screen_shake(2);
        assert_eq!(
            world
                .acknowledge_frozen_camera_requests(&frame)
                .screen_shakes,
            1
        );
        assert_eq!(world.peek_pending_screen_shakes().len(), 1);
        assert_eq!(world.peek_pending_screen_shakes()[0].intensity, 2);
        assert_eq!(peer.peek_pending_screen_shakes()[0].intensity, 1);
        assert_eq!(peer.peek_pending_screen_shakes().len(), 1);
    }
}
