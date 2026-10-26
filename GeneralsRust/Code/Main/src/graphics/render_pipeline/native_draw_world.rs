//! One movable CPU Draw world, separate from the renderer's device services.
//! Constructor state, playback and pending restore records have one owner.
//! A failed map candidate can drop this value without changing the live world.
use super::*;

/// Device-adjacent services borrowed independently of either live or candidate
/// world. The animation clock remains the driving renderer's one clock.
pub(crate) struct NativeDrawServices<'a> {
    failed_model_resolutions: &'a mut pipeline_collect::FailedModelResolutions,
    animation_sync_ms: u32,
}

impl RenderPipeline {
    pub(crate) fn borrow_native_draw_services(&mut self) -> NativeDrawServices<'_> {
        NativeDrawServices {
            failed_model_resolutions: &mut self.failed_model_resolutions,
            animation_sync_ms: self.animation_sync_ms,
        }
    }
}

/// The driving native surface only, with no world/model ownership loan.
pub(crate) struct NativeLoadSurface<'a> {
    forward: &'a mut ForwardPass,
}
impl NativeLoadSurface<'_> {
    pub(crate) fn present(
        &mut self,
        ui: &mut crate::graphics::ui_render_pass::NativeUiOverlay<'_>,
    ) -> Result<(), String> {
        if !self.forward.is_ready() {
            return Err("native loading surface is unavailable".into());
        }
        self.forward
            .renderer
            .begin_frame()
            .map_err(|error| error.to_string())?;
        let mut overlay = |frame: &mut ww3d_engine::RenderFrame| ui(frame, None);
        self.forward
            .renderer
            .end_frame_with_scene_policy(true, Some(&mut overlay))
            .map_err(|error| error.to_string())
    }
}
impl RenderPipeline {
    pub(crate) fn borrow_native_load_surface(&mut self) -> NativeLoadSurface<'_> {
        NativeLoadSurface {
            forward: &mut self.forward_pass,
        }
    }
    pub(crate) fn borrow_native_draw_services_and_load_surface(
        &mut self,
    ) -> (NativeDrawServices<'_>, NativeLoadSurface<'_>) {
        (
            NativeDrawServices {
                failed_model_resolutions: &mut self.failed_model_resolutions,
                animation_sync_ms: self.animation_sync_ms,
            },
            NativeLoadSurface {
                forward: &mut self.forward_pass,
            },
        )
    }
}
impl<'a> NativeDrawServices<'a> {
    pub(crate) fn bind_world(
        self,
        world: &'a mut NativeDrawWorld,
    ) -> native_pose_loan::NativeInstalledPoseLoan<'a> {
        world.borrow_installed_poses(self.failed_model_resolutions, self.animation_sync_ms)
    }
}

/// Actual per-object constructor lifetime, including authored NoDraw actors.
/// Module-level identities remain consistency checks while their broader move
/// is pending; lifecycle decisions use this canonical constructor record.
#[derive(Debug, Clone)]
pub(super) struct NativeDrawableLifetime {
    pub(super) identity: crate::assets::LiveDrawPlaybackIdentity,
    pub(super) auxiliary: Option<Arc<crate::game_logic::thing::ReceivingDrawModels>>,
    #[cfg(feature = "game_client")]
    pub(super) binding: Option<game_client::core::PresentationDirectDrawableBindingKey>,
}
#[derive(Default)]
pub(crate) struct NativeDrawWorld {
    pub(super) native_drawable_lifetimes: HashMap<NativeDrawOwnerId, NativeDrawableLifetime>,
    /// Runtime identity and current state for each original Draw module.
    pub(super) drawable_visual_states: HashMap<(NativeDrawOwnerId, u32), ObjectVisualState>,
    pub(super) live_draw_playback:
        HashMap<crate::assets::LiveDrawPlaybackKey, crate::assets::LiveDrawPlayback>,
    /// Validated payload awaiting actual receiving topology, consumed once.
    pub(super) pending_client_drawable_restore:
        Option<crate::save_load::snapshot::ClientDrawableWorldSnapshot>,
    pub(super) pending_client_drawable_imports:
        HashMap<(u32, u32), crate::save_load::snapshot::ClientDrawableStateSnapshot>,
}

impl NativeDrawWorld {
    pub(crate) fn borrow_installed_poses<'a>(
        &'a mut self,
        failed_model_resolutions: &'a mut pipeline_collect::FailedModelResolutions,
        animation_sync_ms: u32,
    ) -> native_pose_loan::NativeInstalledPoseLoan<'a> {
        native_pose_loan::NativeInstalledPoseLoan {
            native_drawable_lifetimes: &mut self.native_drawable_lifetimes,
            live_draw_playback: &mut self.live_draw_playback,
            drawable_visual_states: &mut self.drawable_visual_states,
            failed_model_resolutions,
            pending_client_drawable_restore: &mut self.pending_client_drawable_restore,
            pending_client_drawable_imports: &mut self.pending_client_drawable_imports,
            animation_sync_ms,
        }
    }
}

impl RenderPipeline {
    /// Commit already constructed CPU Draw state after retiring the old Client.
    /// Device services and caches retain their application lifetime. Never copy
    /// bindings or replay Creating: candidate identity is already admitted.
    pub(crate) fn install_native_draw_world(&mut self, world: NativeDrawWorld) {
        self.invalidate_world_visual_state();
        self.native_draw_world = world;
    }
}
