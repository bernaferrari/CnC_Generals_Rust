// Command & Conquer Generals Zero Hour™
// Copyright 2025 Electronic Arts Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! # WWShade - Rust Graphics Shader Library
//!
//! This is a Rust port of the WWShade library originally used in Command & Conquer Generals Zero Hour.
//! The library provides a modern graphics abstraction layer for shader management, materials, and rendering.

pub mod api_demo;
pub mod bump_mapping;
pub mod class_ids;
pub mod cubemap;
pub mod def;
pub mod embedded_shaders;
pub mod error;
pub mod gloss_mask;
pub mod hardware;
pub mod interface;
pub mod legacy;
pub mod loader;
pub mod manager;
pub mod mesh;
pub mod renderer;
pub mod simple; // API compatibility demonstration

// Modern WGPU backend
pub mod modern_shaders;
pub mod wgpu_backend;
// pub mod wgpu_renderer; // Temporarily disabled
pub mod compatibility; // API-compatible layer

pub mod shd6bumpdiff;
pub mod shd6bumpdiff_constants;
pub mod shd6bumpspec;
pub mod shd6bumpspec_constants;
pub mod shd7bumpdiff;
pub mod shd7bumpdiff_constants;
pub mod shd7bumpspec;
pub mod shd7bumpspec_constants;
pub mod shd8bumpdiff;
pub mod shd8bumpdiff_constants;
pub mod shd8bumpspec;
pub mod shd8bumpspec_constants;
pub mod shdbumpdiff;
pub mod shdbumpspec;
pub mod shdclassids;
pub mod shdcubemap;
pub mod shddef;
pub mod shddeffactory;
pub mod shddefmanager;
pub mod shddump;
pub mod shdforcelinks;
pub mod shdglossmask;
pub mod shdhw_constants;
pub mod shdhwshader;
pub mod shdinterface;
pub mod shdlegacyw3d;
pub mod shdlib;
pub mod shdloader;
pub mod shdmesh;
pub mod shdrenderer;
pub mod shdsimple;
pub mod shdsubmesh;
pub use error::{ShdError, ShdResult};

// Re-export commonly used types
pub use class_ids::*;
pub use def::ShdDefClass;
pub use embedded_shaders::{DirectXVersion, EmbeddedShaders, ShaderKey, ShaderSource, ShaderType};
pub use interface::{RenderInfo, ShdInterface, MAX_PASSES};
pub use mesh::ShdMesh;

// Re-export the API demo to show compatibility
pub use api_demo::{WWShadeApiCompatibility, YourExistingGameEngine};

// Version information
pub const WWSHADE_VERSION: &str = env!("CARGO_PKG_VERSION");
