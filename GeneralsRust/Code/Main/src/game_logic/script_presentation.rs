//! Ephemeral loans for one synchronous script/callback walk, never world state.
use crate::assets::LiveDrawPlaybackIdentity;
#[derive(Clone)]
pub(crate) struct NativeSlaveSource {
    pub(crate) identity: LiveDrawPlaybackIdentity,
    pub(crate) template_name: String,
    pub(crate) receiving_draw_models: std::sync::Arc<crate::game_logic::thing::ReceivingDrawModels>,
    pub(crate) condition_bits: u128,
    pub(crate) mesh_scale: f32,
    pub(crate) float_yaw: f32,
    pub(crate) float_pitch: f32,
    pub(crate) under_construction: bool,
    pub(crate) construction_percent: f32,
    pub(crate) max_height_above_position: f32,
}

use crate::presentation_frame::camera::build::SlaveCameraSample;
pub(crate) trait NativeViewEffects {
    fn local_player_id(&self) -> u32;
    fn sample_slave(&mut self, source: &NativeSlaveSource, bone: &str) -> SlaveCameraSample;
}
#[derive(Default)]
pub(crate) struct ScriptPresentationEffects<'a> {
    pub(crate) ui: Option<&'a mut dyn super::ScriptUiEffects>,
    pub(crate) view: Option<&'a mut dyn NativeViewEffects>,
}
impl ScriptPresentationEffects<'_> {
    pub(crate) fn reborrow(&mut self) -> ScriptPresentationEffects<'_> {
        ScriptPresentationEffects {
            ui: self
                .ui
                .as_mut()
                .map(|ui| &mut **ui as &mut dyn super::ScriptUiEffects),
            view: self
                .view
                .as_mut()
                .map(|view| &mut **view as &mut dyn NativeViewEffects),
        }
    }
}
