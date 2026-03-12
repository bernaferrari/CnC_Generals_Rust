//! Draw Modules - Visual representation of game objects
//!
//! Port of C++ DrawModule hierarchy from:
//! - /GeneralsMD/Code/GameEngine/Include/Common/DrawModule.h
//! - /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/
//!
//! Draw modules handle rendering, animation, and visual effects for game objects.
//! They are the client-side representation of objects in the game world.

pub mod draw_module;
pub mod w3d_debris_draw;
pub mod w3d_laser_draw;
pub mod w3d_model_draw;
pub mod w3d_overlord_tank_draw;
pub mod w3d_projectile_draw;
pub mod w3d_projectile_stream_draw;
pub mod w3d_rope_draw;
pub mod w3d_tank_draw;
pub mod w3d_tracer_draw;
pub mod w3d_tree_draw;

pub use draw_module::{
    DebrisDrawInterface, DrawModule, DrawModuleData, LaserDrawInterface, ObjectDrawInterface,
    RopeDrawInterface, ShadowType, TerrainDecalType, TracerDrawInterface,
};
pub use w3d_debris_draw::{W3DDebrisDraw, W3DDebrisDrawModuleData};
pub use w3d_laser_draw::{W3DLaserDraw, W3DLaserDrawModuleData};
pub use w3d_model_draw::{W3DModelDraw, W3DModelDrawModuleData};
pub use w3d_overlord_tank_draw::{W3DOverlordTankDraw, W3DOverlordTankDrawModuleData};
pub use w3d_projectile_draw::{W3DProjectileDraw, W3DProjectileDrawModuleData};
pub use w3d_projectile_stream_draw::{W3DProjectileStreamDraw, W3DProjectileStreamDrawModuleData};
pub use w3d_rope_draw::{W3DRopeDraw, W3DRopeDrawModuleData};
pub use w3d_tank_draw::{W3DTankDraw, W3DTankDrawModuleData};
pub use w3d_tracer_draw::{W3DTracerDraw, W3DTracerDrawModuleData};
pub use w3d_tree_draw::{W3DTreeDraw, W3DTreeDrawModuleData};
