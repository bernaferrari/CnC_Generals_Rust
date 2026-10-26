//! Completed receiving-camera facts for audio; observation never advances HAG/FX.
use crate::game_logic::GameLogic;
use game_engine::common::audio::{audio_event_rts::Coord3D, game_audio::AudioViewObservation};
use glam::{Mat4, Vec3};

#[derive(Clone, Copy)]
pub(super) struct CompletedAudioView {
    pub(super) listener: AudioViewObservation,
    view: Mat4,
    projection: Mat4,
}
impl CompletedAudioView {
    pub(super) fn capture(world: &GameLogic) -> Self {
        let session_view = &world.session_view;
        let camera = &session_view.camera;
        let z_up = |v: Vec3| Coord3D {
            x: v.x,
            y: v.z,
            z: v.y,
        };
        Self {
            listener: AudioViewObservation {
                ground_position: z_up(session_view.target),
                // C++ View::getAngle returns m_angle, independent of shake,
                // FX pitch and the map's fixed camera-offset yaw.
                heading: camera.camera_yaw_radians,
                camera_position: z_up(camera.camera_position),
                ground_height: world.raw_terrain_ground_height_at(session_view.target),
            },
            view: camera.view_matrix,
            projection: camera.projection_matrix,
        }
    }

    /// MilesAudioManager::isOnScreen → W3DView::worldToScreenTriReturn →
    /// CameraClass::Project (camera.cpp:330): near, far, then inclusive XY.
    pub(super) fn is_on_screen(&self, position: &Coord3D) -> bool {
        let world = Vec3::new(position.x, position.z, position.y);
        let camera = self.view * world.extend(1.0);
        if !camera.is_finite()
            || camera.z > -crate::presentation_frame::camera::projection::DEFAULT_VIEW_NEAR_CLIP
        {
            return false;
        }
        let clip = self.projection * camera;
        if !clip.is_finite() || clip.w <= 0.0 {
            return false;
        }
        let ndc = clip.truncate() / clip.w;
        ndc.is_finite()
            && ndc.z <= 1.0
            && (-1.0..=1.0).contains(&ndc.x)
            && (-1.0..=1.0).contains(&ndc.y)
    }
}

#[cfg(test)]
#[path = "audio_view_tests.rs"]
mod tests;
