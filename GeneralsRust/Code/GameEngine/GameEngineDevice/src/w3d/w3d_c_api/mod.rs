//! # W3D C++ API Compatibility Layer
//!
//! This module provides 100% compatibility with the original Westwood 3D C++ API
//! while using the modern Rust/wgpu backend underneath. All function signatures
//! match the original W3D API exactly.
//!
//! Split from the former `w3d_c_api.rs` god-file. Public names stay identical so
//! the C ABI / parity surface is unchanged. Live module via
//! `#[path = "w3d_c_api/mod.rs"]`.

mod constants;
mod decl;
mod device;
mod draw;
mod leftover;
mod lighting;
mod materials;
mod math;
mod render_state;
mod streams;
mod textures;
mod transforms;
mod types;

pub use device::{
    W3D_CreateDevice, W3D_Init, W3DDevice_BeginScene, W3DDevice_Clear, W3DDevice_Create,
    W3DDevice_Destroy, W3DDevice_EndScene, W3DDevice_GetDeviceCaps, W3DDevice_GetViewport,
    W3DDevice_Present, W3DDevice_SetViewport,
};
pub use draw::{
    W3DDevice_DrawIndexedPrimitive, W3DDevice_DrawIndexedPrimitiveLegacy,
    W3DDevice_DrawIndexedPrimitiveUP, W3DDevice_DrawPrimitive, W3DDevice_DrawPrimitiveUP,
};
pub use lighting::{
    W3DDevice_GetLight, W3DDevice_GetLightEnable, W3DDevice_LightEnable, W3DDevice_SetLight,
    W3DDevice_SetLightEnable,
};
pub use materials::{W3DDevice_GetMaterial, W3DDevice_SetMaterial};
pub use math::{W3D_MATRIX, W3D_VECTOR};
pub use render_state::{
    W3DDevice_ClearVertexDeclaration, W3DDevice_DefineVertexDeclaration, W3DDevice_GetFVF,
    W3DDevice_GetPixelShader, W3DDevice_GetRenderState, W3DDevice_GetVertexDeclaration,
    W3DDevice_GetVertexShader, W3DDevice_SetFVF, W3DDevice_SetPixelShader,
    W3DDevice_SetRenderState, W3DDevice_SetVertexDeclaration, W3DDevice_SetVertexShader,
};
pub use streams::{
    W3DDevice_GetIndices, W3DDevice_GetStreamSource, W3DDevice_GetStreamSourceEx,
    W3DDevice_SetIndices, W3DDevice_SetStreamSource, W3DDevice_SetStreamSourceEx,
    W3DDevice_SetStreamSourceUP,
};
pub use textures::{
    W3DDevice_GetTexture, W3DDevice_GetTextureStageState, W3DDevice_LoadTexture,
    W3DDevice_SetTexture, W3DDevice_SetTextureStageState,
};
pub use transforms::{W3DDevice_GetTransform, W3DDevice_SetTransform};
pub use types::{
    W3D_DEVICE, W3D_ERROR_CODE, W3D_MATERIAL, W3D_MESH, W3D_PRIMITIVE_TYPE, W3D_RENDER_STATE,
    W3D_TEXTURE, W3D_TRANSFORM_STATE, W3D_VERTEX, W3D_VERTEX_ELEMENT, W3D_VIEWPORT, W3DDeviceC,
    W3DMaterialC, W3DMeshC, W3DTextureC,
};

#[cfg(test)]
mod tests;

/// Concatenated live sources for residual `include_str!` scans.
pub const W3D_C_API_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("math.rs"),
    include_str!("types.rs"),
    include_str!("constants.rs"),
    include_str!("leftover.rs"),
    include_str!("device.rs"),
    include_str!("render_state.rs"),
    include_str!("streams.rs"),
    include_str!("draw.rs"),
    include_str!("textures.rs"),
    include_str!("materials.rs"),
    include_str!("lighting.rs"),
    include_str!("transforms.rs"),
    include_str!("decl.rs"),
);
