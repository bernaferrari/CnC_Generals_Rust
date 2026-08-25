//! W3D C API matrices and vectors.
//!
//! Split from `w3d_c_api.rs`. Public names stay identical for C ABI / parity.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::ffi::{CStr, CString, c_char, c_void};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::RwLock;

/// W3D matrix structure matching original W3D API
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3D_MATRIX {
    pub m: [[f32; 4]; 4],
}

impl From<Mat4> for W3D_MATRIX {
    fn from(mat: Mat4) -> Self {
        Self {
            m: mat.to_cols_array_2d(),
        }
    }
}

impl From<W3D_MATRIX> for Mat4 {
    fn from(mat: W3D_MATRIX) -> Self {
        Mat4::from_cols_array_2d(&mat.m)
    }
}

/// W3D Vector structure matching original W3D API
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3D_VECTOR {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Vec3> for W3D_VECTOR {
    fn from(vec: Vec3) -> Self {
        Self {
            x: vec.x,
            y: vec.y,
            z: vec.z,
        }
    }
}

impl From<W3D_VECTOR> for Vec3 {
    fn from(vec: W3D_VECTOR) -> Self {
        Self::new(vec.x, vec.y, vec.z)
    }
}
