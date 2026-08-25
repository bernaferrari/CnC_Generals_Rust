////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// W3D model loading system for real C&C 3D assets

mod prelude;
pub(self) use prelude::*;

mod hlod_bounding_box;
mod hlod_dazzle_child;
mod hlod_emitter_child;
mod w3d_anim;
mod w3d_collection_aggregate;
mod w3d_dazzle_loader;
mod w3d_emitter_loader;
mod w3d_format;
mod w3d_loader;
mod w3d_loader_parse;
mod w3d_mesh;
mod w3d_mesh_build;
mod w3d_model;
mod w3d_primitive_protos;
mod w3d_proto_registry;

pub use hlod_bounding_box::{
    hlod_bounding_box_child_index, hlod_bounding_box_proto, should_skip_obbox_child,
};
pub use hlod_dazzle_child::hlod_child_is_dazzle;
pub use hlod_emitter_child::hlod_child_is_emitter;
pub use w3d_collection_aggregate::{
    W3dAggregateProto, W3dAggregateSubobject, W3dCollectionProto, W3dDistLodEntry, W3dDistLodProto,
};
pub use w3d_dazzle_loader::W3dDazzleProto;
pub use w3d_emitter_loader::W3dEmitterProto;
pub(crate) use w3d_format::split_w3d_draw_animation_identity;
pub use w3d_format::{
    W3dAnimChannel, W3dAnimation, W3dAnimationBinding, W3dAnimationBindingKey, W3dHierarchy,
    W3dHlod, W3dHlodAggregatePose, W3dHlodAttachmentArray, W3dHlodLod, W3dHlodPrototypeBindPose,
    W3dHlodSubObject, W3dHmodel, W3dHmodelNode, W3dHmodelNodeKind, W3dHmodelNodePose,
    W3dHmodelSkinNodeBinding, W3dHmodelSnapPoint, W3dPivot, W3dRawVisibilityChannel,
    W3dWeaponBarrelBinding, W3dWeaponBarrelTopology, W3dWeaponVisualControl,
};
pub use w3d_loader::{
    W3DLoader, w3d_archive_path_variants, w3d_companion_animation_archive_path_variants,
    w3d_companion_animation_filename,
};
pub use w3d_mesh::{
    BlendMode, TextureAddressMode, TextureBlendMode, TextureFilter, TextureStageMapping, UVSource,
    W3DMaterial, W3DMesh, W3DModel, W3DVertex,
};
pub use w3d_mesh_build::get_common_cnc_units;
pub use w3d_primitive_protos::{W3dBoxProto, W3dNullProto, W3dRingProto, W3dSphereProto};
pub use w3d_proto_registry::{W3dExtraPrototypeKind, extra_prototypes};

#[cfg(test)]
mod tests;
