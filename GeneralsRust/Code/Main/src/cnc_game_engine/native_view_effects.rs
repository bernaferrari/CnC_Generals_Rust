//! Lexical access to the actual installed Draw owner during a view build.
use super::*;
use crate::game_logic::{NativeSlaveSource, NativeViewEffects};
use crate::graphics::render_pipeline::native_pose_loan::NativeInstalledPoseLoan;
use crate::presentation_frame::camera::build::SlaveCameraSample;

#[cfg(feature = "game_client")]
pub(super) struct NativeRendererViewEffects<'a> {
    poses: NativeInstalledPoseLoan<'a>,
    graphics: &'a mut GraphicsSystem,
    presence: game_client::core::game_client::NativeDrawablePresence<'a>,
    host_epoch: u64,
    local_player_id: u32,
}
#[cfg(feature = "game_client")]
impl<'a> NativeRendererViewEffects<'a> {
    pub(super) fn new(
        poses: NativeInstalledPoseLoan<'a>,
        graphics: &'a mut GraphicsSystem,
        presence: game_client::core::game_client::NativeDrawablePresence<'a>,
        host_epoch: u64,
        local_player_id: u32,
    ) -> Self {
        Self {
            poses,
            graphics,
            presence,
            host_epoch,
            local_player_id,
        }
    }
}
#[cfg(feature = "game_client")]
impl NativeViewEffects for NativeRendererViewEffects<'_> {
    fn local_player_id(&self) -> u32 {
        self.local_player_id
    }
    fn sample_slave(&mut self, source: &NativeSlaveSource, bone: &str) -> SlaveCameraSample {
        sample_installed_slave(
            &mut self.poses,
            self.graphics,
            source,
            bone,
            self.presence
                .binding_key(self.host_epoch, source.identity.object_id()),
        )
    }
}
/// Same actual source/dirty/restore/query body for script, display and filter.
/// The PostRender caller supplies an immutable association fact only after
/// proving no binding producer can execute during that render operation.
#[cfg(feature = "game_client")]
pub(super) fn sample_installed_slave(
    poses: &mut NativeInstalledPoseLoan<'_>,
    graphics: &mut GraphicsSystem,
    source: &NativeSlaveSource,
    bone: &str,
    binding_key: Option<game_client::core::PresentationDirectDrawableBindingKey>,
) -> SlaveCameraSample {
    let Some(binding_key) = binding_key else {
        return SlaveCameraSample::MissingDrawable;
    };
    match source
        .receiving_draw_models
        .first_object_draw(source.condition_bits)
    {
        Ok(None) => return SlaveCameraSample::NoInterface,
        Err(()) => return SlaveCameraSample::Unavailable("unsupported current Draw condition"),
        Ok(Some(_)) => {}
    }
    match poses.sample_current_source(source, binding_key, bone, graphics) {
        Ok(pose) => SlaveCameraSample::Pose(pose),
        Err(reason) => {
            log::trace!("Receiving native camera pose unavailable: {reason:?}");
            SlaveCameraSample::Unavailable("unavailable installed receiving Draw pose")
        }
    }
}
impl CnCGameEngine {
    /// A real source setCameraTransform, including every immediate slave query.
    /// Pure inspection/redraw helpers deliberately do not call this operation.
    pub(super) fn set_native_camera_transform(&mut self) -> Result<(), String> {
        self.with_native_camera_effects(|world, effects| {
            world.commit_native_camera_with_effects(effects)
        })
    }
    pub(super) fn restore_short_mmb_camera_defaults(&mut self) -> Result<(), String> {
        self.with_native_camera_effects(|world, effects| world.admit_short_mmb_defaults(effects))
    }
    pub(super) fn with_native_camera_effects(
        &mut self,
        body: impl FnOnce(
            &mut crate::game_logic::GameLogic,
            &mut crate::game_logic::ScriptPresentationEffects<'_>,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        #[cfg(feature = "game_client")]
        {
            let (_, _, presence) = self.game_client.split_frame_snow_and_drawable_presence();
            let mut view = NativeRendererViewEffects::new(
                self.render_pipeline.borrow_native_installed_poses(),
                &mut self.graphics_system,
                presence,
                self.host_direct_visual_world_epoch,
                self.current_player_id,
            );
            body(
                &mut self.game_logic,
                &mut crate::game_logic::ScriptPresentationEffects {
                    ui: None,
                    view: Some(&mut view),
                },
            )
        }
        #[cfg(not(feature = "game_client"))]
        body(
            &mut self.game_logic,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "native_view_effects_tests.rs"]
mod tests;
