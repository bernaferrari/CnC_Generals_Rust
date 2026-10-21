//! One movable CPU Draw world, separate from the renderer's device services.
//! Constructor state, playback and pending restore records have one owner.
//! A failed map candidate can drop this value without changing the live world.
use super::*;

#[derive(Default)]
pub(crate) struct NativeDrawWorld {
    /// Runtime identity and current state for each original Draw module.
    pub(super) drawable_visual_states: HashMap<(u32, u32), ObjectVisualState>,
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
            live_draw_playback: &mut self.live_draw_playback,
            drawable_visual_states: &mut self.drawable_visual_states,
            failed_model_resolutions,
            pending_client_drawable_restore: &mut self.pending_client_drawable_restore,
            pending_client_drawable_imports: &mut self.pending_client_drawable_imports,
            animation_sync_ms,
        }
    }
}
