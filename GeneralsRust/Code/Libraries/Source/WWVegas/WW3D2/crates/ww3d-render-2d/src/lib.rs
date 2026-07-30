#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(clippy::cargo)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(deprecated)]
#![allow(nonstandard_style)]
#![allow(unconditional_recursion)]
#![allow(mismatched_lifetime_syntaxes)]
#![allow(unexpected_cfgs)]
#![allow(private_interfaces)]
#![allow(missing_docs)]

//! WW3D 2D Renderer
//!
//! This crate provides specialized rendering for all 2D elements
//! including UI, text, and overlays with complete feature parity
//! to the C++ WW3D2 rendering system.

pub mod bitmap_renderer;
pub mod font_system;
pub mod text_draw;
pub mod text_renderer;
pub mod ui_renderer;

pub use bitmap_renderer::*;
pub use font_system::*;
pub use text_draw::*;
pub use text_renderer::*;
pub use ui_renderer::*;
