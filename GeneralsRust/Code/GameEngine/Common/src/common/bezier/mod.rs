//! Cubic Bezier segment and forward-difference iterator.
//!
//! C++ counterparts:
//! - `GeneralsMD/Code/GameEngine/Source/Common/Bezier/BezierSegment.cpp`
//! - `GeneralsMD/Code/GameEngine/Source/Common/Bezier/BezFwdIterator.cpp`
//! - headers under `GeneralsMD/Code/GameEngine/Include/Common/`
//!
//! Intentional Rust deviations:
//! - `BezFwdIterator` also implements `Iterator` / `ExactSizeIterator` for idiomatic
//!   consumption; C++ callers use `start` / `done` / `getCurrent` / `next`.
//! - `evaluate_at_t` returns `Coord3D` directly (C++ wrote through a nullable out-param).
//! - Local `Matrix4x4` / `Vector4` helpers replace D3DX types; the basis matrix values
//!   and multiply match `D3DXVec4Transform` for this symmetric matrix.

pub mod bez_fwd_iterator;
pub mod bezier_segment;

pub use bez_fwd_iterator::BezFwdIterator;
pub use bezier_segment::{BezierSegment, Bool, Coord3D, Int, Matrix4x4, Real, VecCoord3D, Vector4};
