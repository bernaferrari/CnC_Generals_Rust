//! One native UI draw's radar input and receiving receipt sink.
//! Camera facts are immutable; the UI handle is the existing application service.
use super::radar_view::RadarCamera;
use game_client::{
    core::subsystems::InGameUiHandle,
    gui::{
        w3d_gadget_draw::{RadarDrawContext, RadarViewCorners},
        window_manager::WindowManager,
    },
    helpers::InGameUiHooks,
};
use game_engine::common::system::radar::{Coord3D, RadarPresentation, RadarSystem};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct NativeRadarFrame {
    camera: RadarCamera,
    radar: Option<Arc<RadarPresentation>>,
    ui: Option<InGameUiHandle>,
}
impl NativeRadarFrame {
    pub(crate) fn capture(
        camera: RadarCamera,
        owner: &RadarSystem,
        ui: Option<InGameUiHandle>,
    ) -> Self {
        let radar = ui
            .as_ref()
            .and_then(|ui| ui.radar_presentation())
            .filter(|radar| owner.owns_presentation(radar));
        Self { camera, radar, ui }
    }
    pub(crate) fn with_matrices(&self, view: glam::Mat4, projection: glam::Mat4) -> Self {
        Self {
            camera: self.camera.with_matrices(view, projection),
            ..self.clone()
        }
    }
    pub(crate) fn project_hint(&self, position: glam::Vec3) -> Option<glam::Vec2> {
        self.camera.project_world(position)
    }
    pub(crate) fn draw_windows(
        &self,
        windows: &mut WindowManager,
        draw: &mut game_client::gui::window_draw_context::WindowDrawContext<'_>,
    ) {
        let z_up = |v: glam::Vec3| Coord3D::new(v.x, v.z, v.y);
        let view_box = self.camera.corners().map(|corners| RadarViewCorners {
            origin_world: z_up(corners[0]),
            corners: corners.map(z_up),
        });
        let admit = |radar: &RadarPresentation| {
            if let Some(ui) = self.ui.as_ref() {
                for _ in 0..ui.admit_radar_draw(radar) {
                    game_engine::common::system::radar::play_radar_event_chirp();
                }
            }
        };
        windows.draw_all_with_radar(
            &RadarDrawContext {
                radar: self.radar.as_deref(),
                view_box,
                admit: &admit,
            },
            draw,
        );
    }
}

#[cfg(test)]
#[path = "ui_radar_tests.rs"]
mod tests;
