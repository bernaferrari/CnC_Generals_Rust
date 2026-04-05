//! Scene submission trait: bridges GameLogic draw modules to GameEngineDevice rendering.
//!
//! GameLogic draw modules (laser, tracer, rope) compute line geometry but cannot
//! depend on GameEngineDevice directly. This trait lives in Common so both sides
//! can reference it: GameLogic submits geometry, GameEngineDevice implements the trait.

use super::geometry::Coord3D;

#[derive(Debug, Clone)]
pub struct SceneLineDesc {
    pub start: Coord3D,
    pub end: Coord3D,
    pub width: f32,
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub opacity: f32,
    pub texture_name: Option<String>,
    pub tile_factor: f32,
    pub visible: bool,
}

pub type SceneLineId = u64;

/// Uses `&self` because implementations wrap interior-mutable scene state
/// (e.g. `Arc<RwLock<W3DScene>>`).
pub trait SceneSubmission: Send + Sync {
    fn submit_line(&self, drawable_id: u32, desc: &SceneLineDesc) -> Option<SceneLineId>;
    fn update_line(&self, id: SceneLineId, desc: &SceneLineDesc);
    fn remove_line(&self, id: SceneLineId);
}
