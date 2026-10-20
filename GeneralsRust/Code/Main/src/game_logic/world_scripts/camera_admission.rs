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
    pub bw: bool,
    pub motion_blur: usize,
}

impl GameLogic {
    pub(crate) fn acknowledge_frozen_camera_requests(
        &mut self,
        frame: &crate::presentation_frame::PresentationFrame,
    ) -> CameraRequestAdmission {
        let mut admitted = CameraRequestAdmission::default();
        if frame.camera_bw_mode.is_some()
            && self
                .pending_camera_bw_mode
                .as_ref()
                .map(|r| (r.enabled, r.frames))
                == frame.camera_bw_mode
        {
            admitted.bw = true;
            self.pending_camera_bw_mode = None;
        }
        admitted.motion_blur = self
            .pending_camera_motion_blur
            .iter()
            .zip(&frame.camera_motion_blur)
            .take_while(|(a, b)| a == b)
            .count();
        self.pending_camera_motion_blur
            .drain(..admitted.motion_blur);
        if frame.camera_focus.is_some()
            && self.pending_camera_focus.map(|p| p.to_array()) == frame.camera_focus
            && self
                .session_view
                .pending_focus_previous_target
                .map(|p| p.to_array())
                == frame.camera_focus_previous_target
        {
            admitted.focus = true;
            self.pending_camera_focus = None;
            self.session_view.pending_focus_previous_target = None;
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
    #[test]
    fn frozen_filter_admission_preserves_new_values_tail_and_peer() {
        use crate::game_logic::mission_scripts::CameraMotionBlurRequest;
        let mut world = GameLogic::new();
        let mut peer = GameLogic::new();
        world.queue_pending_camera_bw_mode(true, 9);
        world
            .pending_camera_motion_blur
            .push(CameraMotionBlurRequest::Follow { amount: 17 });
        peer.pending_camera_motion_blur
            .push(CameraMotionBlurRequest::EndFollow);
        let frame = PresentationFrame::build_from_logic(&mut world, 0);
        world.queue_pending_camera_bw_mode(false, 4);
        world
            .pending_camera_motion_blur
            .push(CameraMotionBlurRequest::EndFollow);
        let admitted = world.acknowledge_frozen_camera_requests(&frame);
        assert!(!admitted.bw);
        assert_eq!(admitted.motion_blur, 1);
        assert_eq!(
            world
                .peek_pending_camera_bw_mode()
                .map(|r| (r.enabled, r.frames)),
            Some((false, 4))
        );
        assert_eq!(
            world.peek_pending_camera_motion_blur(),
            &[CameraMotionBlurRequest::EndFollow]
        );
        assert_eq!(
            peer.peek_pending_camera_motion_blur(),
            &[CameraMotionBlurRequest::EndFollow]
        );
        assert_eq!(
            world.acknowledge_frozen_camera_requests(&frame).motion_blur,
            0
        );
        let fresh = PresentationFrame::build_from_logic(&mut world, 0);
        let admitted = world.acknowledge_frozen_camera_requests(&fresh);
        assert!(admitted.bw);
        assert_eq!(admitted.motion_blur, 1);
        assert!(world.peek_pending_camera_bw_mode().is_none());
        assert!(world.peek_pending_camera_motion_blur().is_empty());
        assert_eq!(peer.peek_pending_camera_motion_blur().len(), 1);
    }
    #[test]
    fn waypoint_focus_origin_survives_completion_and_equal_position_overwrite_is_distinct() {
        let mut world = GameLogic::new();
        // LookAt is a real source terrain/bounds operation, not a raw queue
        // setter. Use a receiving map and an interior destination so the
        // same-position overwrite witness does not accidentally test clipping.
        let mut map = gamelogic::system::map_loader::MapData::new();
        map.width = 256;
        map.height = 256;
        map.heightmap = vec![0; 256 * 256];
        map.boundaries = vec![gamelogic::common::ICoord2D::new(256, 256)];
        assert!(
            world
                .world_services
                .terrain()
                .write()
                .unwrap()
                .load_map_geometry(map)
                .is_empty()
        );
        world.session_view.target = Vec3::new(2., 0., 3.);
        let destination = Vec3::new(1000., 0., 1000.);
        world.request_path_camera_focus(destination);
        let frame = PresentationFrame::build_from_logic(&mut world, 0);
        assert_eq!(frame.camera_focus_previous_target, Some([2., 0., 3.]));
        // A direct LookAt at the same position must not replay an old path delta.
        world.request_camera_focus(destination);
        assert!(!world.acknowledge_frozen_camera_requests(&frame).focus);
        assert_eq!(world.peek_pending_camera_focus(), Some(destination));
        let direct = PresentationFrame::build_from_logic(&mut world, 0);
        assert!(direct.camera_focus_previous_target.is_none());
        assert!(world.acknowledge_frozen_camera_requests(&direct).focus);
        world.request_path_camera_focus(destination);
        let path = PresentationFrame::build_from_logic(&mut world, 0);
        assert!(world.acknowledge_frozen_camera_requests(&path).focus);
        assert!(world.session_view.pending_focus_previous_target.is_none());
    }
}
