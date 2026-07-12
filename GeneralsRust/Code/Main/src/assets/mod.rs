////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// Asset loading system - Rust implementation based on C++ Win32BIGFile system
// Includes WW3D asset manager and INI parsing for object definitions

pub mod archive;
pub mod audio;
pub mod big_file;
pub mod ini_parser;
pub mod ini_template_loader;
pub mod local_file_system;
pub mod manager;
pub mod mesh_asset_resolve;
pub mod models;
pub mod sound_effects;
pub mod textures;
pub mod ww3d_asset_manager;

pub use archive::*;
pub use audio::*;
pub use big_file::*;
pub use ini_parser::*;
pub use local_file_system::LocalFileSystem;
pub use manager::*;
pub use mesh_asset_resolve::{
    create_placeholder_mesh_model, model_key_from_presentation, model_key_from_template,
    remap_model_key_alias, resolve_mesh_for_model_key, resolve_mesh_for_presentation,
    resolve_mesh_for_template, MeshResolveHonesty, MeshResolveResult, PLACEHOLDER_MODEL_KEY,
};
pub use models::*;
pub use sound_effects::*;
pub use textures::*;
pub use ww3d_asset_manager::*;
