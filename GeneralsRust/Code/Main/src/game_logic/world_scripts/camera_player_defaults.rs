//! Source player setters, sharing the actual receiving native build operation.
use super::super::*;
use crate::game_logic::{NativeSlaveSource, ScriptPresentationEffects};
use crate::presentation_frame::camera::build::SlaveCameraSample;
impl GameLogic {
    pub(super) fn with_current_camera_sampler<R>(
        &mut self,
        effects: &mut ScriptPresentationEffects<'_>,
        body: impl FnOnce(
            &mut Self,
            &mut dyn FnMut(
                &crate::game_logic::mission_scripts::CameraSlaveModeRequest,
            ) -> SlaveCameraSample,
        ) -> Result<R, String>,
    ) -> Result<R, String> {
        let source: Option<NativeSlaveSource> =
            match self.session_view.camera.camera_slave_mode.as_ref() {
                None => None,
                Some(mode) => match self.owned_named_script_object(&mode.thing_template_name, None)
                {
                    None => None,
                    Some(id) => Some(
                        self.current_native_slave_source(
                            id,
                            effects
                                .view
                                .as_ref()
                                .ok_or("native slave source requires receiving view effects")?
                                .local_player_id(),
                        )
                        .map_err(str::to_owned)?,
                    ),
                },
            };
        body(self, &mut |mode| match source.as_ref() {
            None => SlaveCameraSample::MissingObject,
            Some(source) => effects.view.as_mut().map_or(
                SlaveCameraSample::Unavailable("missing receiving native view effects"),
                |view| view.sample_slave(source, &mode.bone_name),
            ),
        })
    }
    pub(crate) fn commit_native_camera_with_effects(
        &mut self,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        self.with_current_camera_sampler(effects, |world, sample| {
            let bounds = {
                let terrain = world
                    .world_services
                    .terrain()
                    .read()
                    .map_err(|_| "receiving camera bounds terrain poisoned")?;
                Self::camera_bounds_for_terrain(&terrain)
            };
            world.session_view.set_camera_transform(bounds, sample)
        })
    }
    pub(super) fn camera_bounds_for_terrain(
        terrain: &gamelogic::terrain::TerrainLogic,
    ) -> (Vec3, Vec3) {
        // calcCameraConstraints459 reads current TerrainLogic::getExtent,
        // including the active boundary, rather than cached host/grid bounds.
        let extent = terrain.get_extent();
        (
            Vec3::new(extent.lo.x, extent.lo.z, extent.lo.y),
            Vec3::new(extent.hi.x, extent.hi.z, extent.hi.y),
        )
    }
    // setDefaultView stores this floor once; both reset and zoomDefault use it.
    pub(crate) fn receiving_camera_max_hag(&self) -> f32 {
        self.session_view.camera.settings.maximum()
    }
    fn mmb_angle_pitch_defaults(
        &mut self,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        // W3DView1877: no cancellation or cache invalidation at this setter.
        self.session_view.camera.camera_yaw_radians = 0.;
        self.session_view.camera.camera_pitch_radians = self.script_default_camera_pitch();
        self.session_view.camera.camera_fx_pitch = 1.;
        self.commit_native_camera_with_effects(effects)
    }
    fn mmb_zoom_default(
        &mut self,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        // Sample AFTER the first build: a slave may have changed raw m_pos.
        let target = self.session_view.target;
        let max_hag = self.receiving_camera_max_hag();
        let terrain_owner = self.world_services.terrain();
        let terrain = terrain_owner
            .read()
            .map_err(|_| "receiving zoom-default terrain poisoned")?;
        let height = crate::presentation_frame::camera::height::height_around(target, |x, z| {
            Some(terrain.get_ground_height(x, z, None))
        })
        .ok_or("invalid receiving zoom-default terrain")?;
        drop(terrain);
        self.session_view.camera.camera_zoom = self
            .session_view
            .camera
            .native_camera
            .set_zoom_to_default(height, max_hag);
        // Source1952–1975 cancels motion, not follow/tether/slave identity.
        self.cancel_scripted_camera_from_player_set();
        self.session_view.camera.constraints.invalidate();
        self.commit_native_camera_with_effects(effects)
    }
    pub(crate) fn admit_short_mmb_defaults(
        &mut self,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        self.mmb_angle_pitch_defaults(effects)?;
        self.mmb_zoom_default(effects)
    }
}
#[cfg(test)]
#[path = "camera_player_defaults_tests.rs"]
mod tests;
