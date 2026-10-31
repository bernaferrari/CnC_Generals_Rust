//! Explicit native map presentation receiver; construction never publishes it.
use super::*;
impl RenderPipeline {
    #[cfg(feature = "game_client")]
    pub(crate) fn bind_native_prop_epoch(&mut self, epoch: u64) {
        self.native_draw_world.pending_native_props.bind_epoch(epoch);
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn receive_native_prop(
        &mut self,
        host_epoch: u64,
        request: gamelogic::object::draw::w3d_prop_draw::TerrainPropAddRequest,
    ) {
        if !self.native_draw_world.pending_native_props.push(host_epoch, request) {
            warn!("rejected terrain prop from a different draw-world epoch");
            return;
        }
        self.flush_native_props();
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn flush_native_props(&mut self) {
        let Some(terrain) = self.native_terrain.as_mut() else {
            return;
        };
        self.native_draw_world.pending_native_props.drain_into(terrain);
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn borrow_native_installed_poses_and_terrain(
        &mut self,
    ) -> (
        native_pose_loan::NativeInstalledPoseLoan<'_>,
        Option<&mut game_client::terrain::terrain_visual::TerrainVisualImpl>,
    ) {
        let poses = self
            .native_draw_world
            .borrow_installed_poses(&mut self.failed_model_resolutions, self.animation_sync_ms);
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
        self.native_draw_world.pending_native_props.clear();
        self.radar_terrain_colors = None;
        self.heightmap_world_size = None;
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "native_terrain_owner_tests.rs"]
mod tests;
