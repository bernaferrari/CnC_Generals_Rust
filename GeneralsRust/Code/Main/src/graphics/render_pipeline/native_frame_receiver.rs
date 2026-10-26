//! Receiving native owners lent only at their original render phases.
use super::*;
pub(crate) trait NativeFrameRenderReceiver {
    fn complete_physics_visual(
        &mut self,
        input: &mut crate::presentation_frame::UnitRenderInput,
        frame: &crate::presentation_frame::PresentationFrame,
    );
    fn direct_scene_decisions(
        &mut self,
        candidates: &[FrozenDirectDrawableSceneCandidate],
    ) -> Vec<FrozenDirectDrawableSceneDecision>;
    fn view_filter(&self) -> &game_client::display::view_filter::ViewFilterState;
    fn view_filter_mut(&mut self) -> &mut game_client::display::view_filter::ViewFilterState;
    fn complete_filter_camera(
        &mut self,
        point: Option<Vec3>,
        poses: &mut native_pose_loan::NativeInstalledPoseLoan<'_>,
        graphics: &mut GraphicsSystem,
    ) -> std::result::Result<FilterCameraMatrices, String>;
    fn complete_tree_cpu_draw(
        &mut self,
        terrain: &mut game_client::terrain::terrain_visual::TerrainVisualImpl,
        poses: native_pose_loan::NativeInstalledPoseLoan<'_>,
        graphics: &mut GraphicsSystem,
        motion: game_client::terrain::terrain_visual::TerrainVisualMotion,
    ) -> std::result::Result<Option<game_client::terrain::TreeCpuDrawReceipt>, String>;
}
