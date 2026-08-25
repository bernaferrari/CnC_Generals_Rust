//! Draw Modules - Visual representation of game objects
//!
//! Port of C++ DrawModule hierarchy from:
//! - /GeneralsMD/Code/GameEngine/Include/Common/DrawModule.h
//! - /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/
//!
//! Draw modules handle rendering, animation, and visual effects for game objects.
//! They are the client-side representation of objects in the game world.

pub mod client_visual;
pub mod draw_module;
pub mod overlord_rider;
pub mod w3d_debris_draw;
pub mod w3d_default_draw;
pub mod w3d_dependency_model_draw;
pub mod w3d_laser_draw;
#[path = "w3d_model_draw_impl/mod.rs"]
pub mod w3d_model_draw;
pub mod w3d_overlord_aircraft_draw;
pub mod w3d_overlord_tank_draw;
pub mod w3d_overlord_truck_draw;
pub mod w3d_police_car_draw;
pub mod w3d_projectile_draw;
pub mod w3d_projectile_stream_draw;
pub mod w3d_prop_draw;
pub mod w3d_rope_draw;
pub mod w3d_science_model_draw;
pub mod w3d_supply_draw;
pub mod w3d_tank_draw;
pub mod w3d_tank_truck_draw;
pub mod w3d_tracer_draw;
pub mod w3d_tree_draw;
pub mod w3d_truck_draw;

pub use client_visual::{
    TerrainDecalClient, TerrainDecalDesc, TerrainTrackClient, leftover_hidden_status_deselects,
    object_should_animate, object_should_animate_flags, preload_draw_asset,
    register_preload_asset_hook, register_terrain_decal_client, register_terrain_track_client,
    register_texture_aspect_hook, terrain_decal_client, terrain_decal_texture_name,
    terrain_track_client, texture_aspect_ratio,
};
pub use draw_module::{
    DebrisDrawInterface, DrawModule, DrawModuleData, LaserDrawInterface, ObjectDrawInterface,
    RopeDrawInterface, ShadowType, TerrainDecalType, TracerDrawInterface,
};
pub use w3d_debris_draw::{W3DDebrisDraw, W3DDebrisDrawModuleData};
pub use w3d_default_draw::{W3DDefaultDraw, W3DDefaultDrawModuleData};
pub use w3d_dependency_model_draw::{W3DDependencyModelDraw, W3DDependencyModelDrawModuleData};
pub use w3d_laser_draw::{W3DLaserDraw, W3DLaserDrawModuleData};
pub use w3d_model_draw::{
    W3DModelDraw, W3DModelDrawModuleData, lookup_current_client_bone_pose,
    lookup_pristine_bone_pose, lookup_pristine_bone_translation,
    register_pristine_bone_lookup_hook,
};
pub use w3d_overlord_aircraft_draw::{W3DOverlordAircraftDraw, W3DOverlordAircraftDrawModuleData};
pub use w3d_overlord_tank_draw::{W3DOverlordTankDraw, W3DOverlordTankDrawModuleData};
pub use w3d_overlord_truck_draw::{W3DOverlordTruckDraw, W3DOverlordTruckDrawModuleData};
pub use w3d_police_car_draw::{
    W3DPoliceCarDraw, W3DPoliceCarDrawModuleData, prune_live_host_police_car_light,
    tick_live_host_police_car_light,
};
pub use w3d_projectile_draw::{W3DProjectileDraw, W3DProjectileDrawModuleData};
pub use w3d_projectile_stream_draw::{W3DProjectileStreamDraw, W3DProjectileStreamDrawModuleData};
pub use w3d_prop_draw::{W3DPropDraw, W3DPropDrawModuleData};
pub use w3d_rope_draw::{W3DRopeDraw, W3DRopeDrawModuleData};
pub use w3d_science_model_draw::{
    W3DScienceModelDraw, W3DScienceModelDrawModuleData, leftover_science_model_data,
    leftover_science_model_should_hide, tick_live_host_science_model_hide,
};
pub use w3d_supply_draw::{W3DSupplyDraw, W3DSupplyDrawModuleData};
pub use w3d_tank_draw::{
    W3DTankDraw, W3DTankDrawModuleData, prune_live_host_tread_debris, tick_live_host_tread_debris,
};
pub use w3d_tank_truck_draw::{W3DTankTruckDraw, W3DTankTruckDrawModuleData};
pub use w3d_tracer_draw::{W3DTracerDraw, W3DTracerDrawModuleData};
pub use w3d_tree_draw::{W3DTreeDraw, W3DTreeDrawModuleData};
pub use w3d_truck_draw::{
    TruckDrawLivePhysics, W3DTruckDraw, W3DTruckDrawModuleData, prune_live_host_truck_dust,
    tick_live_host_truck_dust,
};
