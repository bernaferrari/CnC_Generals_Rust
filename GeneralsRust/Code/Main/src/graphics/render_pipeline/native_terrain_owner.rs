//! Explicit native map presentation receiver; construction never publishes it.
use super::*;
impl RenderPipeline {
    #[cfg(feature = "game_client")]
    pub(crate) fn borrow_native_installed_poses_and_terrain(
        &mut self,
    ) -> (
        native_pose_loan::NativeInstalledPoseLoan<'_>,
        Option<&mut game_client::terrain::terrain_visual::TerrainVisualImpl>,
    ) {
        let poses = native_pose_loan::NativeInstalledPoseLoan {
            drawable_visual_states: &mut self.drawable_visual_states,
            failed_model_resolutions: &mut self.failed_model_resolutions,
            pending_client_drawable_restore: &mut self.pending_client_drawable_restore,
            pending_client_drawable_imports: &mut self.pending_client_drawable_imports,
            animation_sync_ms: self.animation_sync_ms,
        };
        (poses, self.native_terrain.as_mut())
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn native_terrain(
        &self,
    ) -> Option<&game_client::terrain::terrain_visual::TerrainVisualImpl> {
        self.native_terrain.as_ref()
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn native_terrain_mut(
        &mut self,
    ) -> Option<&mut game_client::terrain::terrain_visual::TerrainVisualImpl> {
        self.native_terrain.as_mut()
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn clear_native_terrain(&mut self) {
        self.native_terrain = None;
        self.radar_terrain_colors = None;
        self.heightmap_world_size = None;
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "native_terrain_owner_tests.rs"]
mod tests;
