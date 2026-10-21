//! Borrowed constructor/setter facts, shared by live callbacks and display.
//! This view owns no state and never captures a presentation frame.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct NativeDrawSource<'a> {
    pub identity: Option<&'a crate::assets::LiveDrawPlaybackIdentity>,
    pub receiving: Option<&'a crate::game_logic::thing::ReceivingDrawModels>,
    pub template: &'a str,
    pub mesh_scale: f32,
    pub float_yaw: f32,
    pub float_pitch: f32,
    pub under_construction: bool,
    pub construction_percent: f32,
    pub max_height_above_position: f32,
}

impl<'a> NativeDrawSource<'a> {
    pub(super) fn display(source: &'a crate::presentation_frame::UnitRenderInput) -> Self {
        Self {
            identity: source.draw_playback_identity.as_ref(),
            receiving: source.receiving_draw_models.as_deref(),
            template: &source.template_name,
            mesh_scale: source.mesh_scale,
            float_yaw: source.float_yaw,
            float_pitch: source.float_pitch,
            under_construction: source.under_construction,
            construction_percent: source.construction_percent,
            max_height_above_position: source.max_height_above_position,
        }
    }

    pub(super) fn current(source: &'a crate::game_logic::NativeSlaveSource) -> Self {
        Self {
            identity: Some(&source.identity),
            receiving: Some(&source.receiving_draw_models),
            template: &source.template_name,
            mesh_scale: source.mesh_scale,
            float_yaw: source.float_yaw,
            float_pitch: source.float_pitch,
            under_construction: source.under_construction,
            construction_percent: source.construction_percent,
            max_height_above_position: source.max_height_above_position,
        }
    }

    pub(super) fn install_transform(self, state: &mut ObjectVisualState) {
        if let Some(identity) = self.identity {
            state.receive_native_transform_observations(
                identity,
                self.mesh_scale,
                self.float_yaw,
                self.float_pitch,
                self.under_construction,
                self.construction_percent,
                self.max_height_above_position,
            );
        }
    }
}
