//! Shared imports for the W3D models split. Visibility is parent-only.
#![allow(unused_imports)]

pub(super) use crate::assets::archive::ArchiveFileSystem;
pub(super) use crate::assets::ini_parser::{
    AuthoredDrawPrimaryTurret, AuthoredDrawSubobjectVisibility, AuthoredDrawWeaponBoneBindings,
    AuthoredDrawWeaponBoneSlot,
};
pub(super) use anyhow::{Result, anyhow};
pub(super) use crc32fast::Hasher;
pub(super) use glam::{Mat4, Vec3};
pub(super) use log::{debug, info, warn};
pub(super) use std::collections::HashMap;
pub(super) use std::sync::Arc;
pub(super) use ww3d_assets::prototypes::{MaterialPassInfo, VertexMapperConfig};
pub(super) use ww3d_core::w3d_format::{
    W3dMeshHeader3Struct, W3dRGBAStruct, W3dShaderStruct, W3dVertInfStruct,
    W3dVertexMaterialStruct, w3d_string_from_bytes,
};
pub(super) use ww3d_renderer_3d::rendering::mesh_system::MeshModelClass;
