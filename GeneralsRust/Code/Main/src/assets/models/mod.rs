////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// W3D model loading system for real C&C 3D assets

mod prelude;
pub(self) use prelude::*;

mod w3d_anim;
mod w3d_format;
mod w3d_loader;
mod w3d_loader_parse;
mod w3d_mesh;
mod w3d_mesh_build;
mod w3d_model;

pub use w3d_format::{
    W3dPivot, W3dHierarchy, W3dHlodSubObject, W3dHlodLod, W3dHlodAttachmentArray,
    W3dHlodAggregatePose, W3dHlodPrototypeBindPose, W3dHlod, W3dHmodelNodeKind, W3dHmodelNode,
    W3dHmodelSnapPoint, W3dHmodel, W3dHmodelNodePose, W3dHmodelSkinNodeBinding,
    W3dWeaponBarrelBinding, W3dWeaponBarrelTopology, W3dWeaponVisualControl, W3dAnimChannel,
    W3dRawVisibilityChannel, W3dAnimation, W3dAnimationBinding, W3dAnimationBindingKey,
};
pub(crate) use w3d_format::split_w3d_draw_animation_identity;
pub use w3d_mesh::{
    W3DVertex, W3DMaterial, TextureStageMapping, UVSource, TextureBlendMode, BlendMode,
    TextureAddressMode, TextureFilter, W3DMesh, W3DModel,
};
pub use w3d_loader::{
    W3DLoader, w3d_archive_path_variants, w3d_companion_animation_filename,
    w3d_companion_animation_archive_path_variants,
};
pub use w3d_mesh_build::get_common_cnc_units;

#[cfg(test)]
mod tests;
