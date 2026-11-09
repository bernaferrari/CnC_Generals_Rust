//! Mechanical split from `assets/models.rs`. No behavior change.
#![allow(dead_code, unused_imports)]
use super::prelude::*;
use super::w3d_format::*;
use super::w3d_loader::*;
use super::w3d_loader_parse::*;
use super::w3d_mesh::*;
use super::w3d_mesh_build::*;
use super::w3d_model::*;
use super::*;

/// Build a column-major 4x4 matrix from a pivot's translation + quaternion rotation.
/// Same logic as W3DLoader::mat4_from_tr_quat but operates on W3dPivot directly.
pub(super) fn mat4_from_pivot(pivot: &W3dPivot) -> [f32; 16] {
    mat4_from_translation_and_quaternion(pivot.translation, pivot.rotation)
}

pub(super) fn mat4_from_translation_and_quaternion(
    translation: [f32; 3],
    rotation: [f32; 4],
) -> [f32; 16] {
    let x = rotation[0];
    let y = rotation[1];
    let z = rotation[2];
    let w = rotation[3];
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;
    let m00 = 1.0 - 2.0 * (yy + zz);
    let m01 = 2.0 * (xy - wz);
    let m02 = 2.0 * (xz + wy);
    let m10 = 2.0 * (xy + wz);
    let m11 = 1.0 - 2.0 * (xx + zz);
    let m12 = 2.0 * (yz - wx);
    let m20 = 2.0 * (xz - wy);
    let m21 = 2.0 * (yz + wx);
    let m22 = 1.0 - 2.0 * (xx + yy);
    let tx = translation[0];
    let ty = translation[1];
    let tz = translation[2];
    [
        m00, m10, m20, 0.0, m01, m11, m21, 0.0, m02, m12, m22, 0.0, tx, ty, tz, 1.0,
    ]
}

/// Build source-space HTree local transforms for the selected W3D animation.
///
/// Generals runs ordinary raw W3D animations through its specialized
/// `HTreeClass::Anim_Update(HRawAnimClass*, ...)`, not the generic interpolated
/// HAnim path, while compressed animations take the generic path. Both sample
/// the canonical `ww3d-animation` channels in place; the returned pose is
/// caller-owned.
pub(super) fn sample_animation_local_transforms(
    hierarchy: &W3dHierarchy,
    anim: &W3dAnimation,
    frame: f32,
) -> Option<Vec<[f32; 16]>> {
    match &anim.motion {
        W3dAnimationMotion::Raw(raw) => sample_raw_local_transforms(hierarchy, raw, frame),
        W3dAnimationMotion::Compressed(compressed) => {
            sample_compressed_local_transforms(hierarchy, anim, compressed, frame)
        }
    }
}

fn sample_raw_local_transforms(
    hierarchy: &W3dHierarchy,
    raw: &ww3d_animation::HRawAnimClass,
    frame: f32,
) -> Option<Vec<[f32; 16]>> {
    let raw_frame = raw_frame_index(raw, frame)?;
    let mut locals: Vec<[f32; 16]> = hierarchy.pivots.iter().map(mat4_from_pivot).collect();
    // C++ sets pivot zero to the external RenderObj root and begins the raw
    // node-motion walk at one; source pivot-zero channels are not sampled.
    for (pivot, local) in locals.iter_mut().enumerate().skip(1) {
        let pose = raw.raw_pivot_pose(pivot, raw_frame);
        // `HTreeClass::Anim_Update(HRawAnimClass*)` first obtains
        // parent * BaseTransform, then Matrix3D::Translate and, only when a
        // Q channel exists, postMul(q). Associativity permits the local
        // equivalent Base * T * Q; the shared HTree evaluator supplies the
        // parent afterward.
        *local = mat4_mul(local, &mat4_from_translation(pose.translation.to_array()));
        if let Some(rotation) = pose.rotation {
            *local = mat4_mul(local, &mat4_from_quaternion(rotation.to_array()));
        }
    }
    Some(locals)
}

pub(super) fn mat4_from_translation(translation: [f32; 3]) -> [f32; 16] {
    [
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        translation[0],
        translation[1],
        translation[2],
        1.0,
    ]
}

pub(super) fn mat4_from_quaternion(rotation: [f32; 4]) -> [f32; 16] {
    mat4_from_translation_and_quaternion([0.0; 3], rotation)
}

/// Sample the canonical compressed channels without cloning assets or
/// allocating temporary channel vectors.
fn sample_compressed_local_transforms(
    hierarchy: &W3dHierarchy,
    anim: &W3dAnimation,
    compressed: &ww3d_animation::HCompressedAnimClass,
    frame: f32,
) -> Option<Vec<[f32; 16]>> {
    if !frame.is_finite() || frame < 0.0 || anim.num_frames == 0
        || !anim.hierarchy_name.eq_ignore_ascii_case(&hierarchy.name)
        || compressed.get_num_pivots() > hierarchy.pivots.len() {
        return None;
    }
    let frame = frame.min((anim.num_frames - 1) as f32);
    let mut locals: Vec<_> = hierarchy.pivots.iter().map(mat4_from_pivot).collect();
    for (pivot, local) in locals.iter_mut().enumerate().skip(1) {
        // Generic HTree applies BaseTransform * animated T/Q, just as the
        // original HCompressedAnimClass supplies zero/identity missing tracks.
        *local = mat4_mul(local, &compressed.get_transform(pivot, frame).to_cols_array());
    }
    Some(locals)
}

/// Multiply two source W3D affine transforms in C++ `Matrix3D::Multiply`
/// order: `a * b`.
///
/// Source `Matrix3D` stores three logical rows and transforms column vectors;
/// Main retains that same transform in glam's column-major array layout.  Do
/// not use the old row/column-swapped loop here: it evaluated `b * a`, which
/// happens to pass translation-only fixtures but makes a child translation
/// ignore its rotated parent.  Keeping the three-by-four arithmetic explicit
/// also matches C++'s affine multiplication rather than accidentally granting
/// source HTree controls projective semantics.
pub(super) fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut r = [0.0; 16];
    for column in 0..3 {
        for row in 0..3 {
            r[column * 4 + row] = a[row] * b[column * 4]
                + a[4 + row] * b[column * 4 + 1]
                + a[8 + row] * b[column * 4 + 2];
        }
    }
    for row in 0..3 {
        r[12 + row] = a[row] * b[12] + a[4 + row] * b[13] + a[8 + row] * b[14] + a[12 + row];
    }
    r[15] = 1.0;
    r
}

/// C++ `HTreeClass::{Base,Anim}_Update` source-space globals for a model
/// whose object/world transform is deliberately supplied by its caller.
///
/// `HTreeClass` overwrites pivot zero with that external object root and forces
/// it visible; it does not apply pivot-zero W3D bind/animation data. Main's
/// aggregate pose API leaves the object transform outside so it can compose
/// the parent RenderItem world matrix exactly once, therefore pivot zero is
/// the identity here. All non-root pivots retain the ordinary ordered
/// parent-local/capture update semantics.
pub(super) fn compute_htree_global_transforms_from_locals_with_capture_controls(
    hierarchy: &W3dHierarchy,
    locals: &[[f32; 16]],
    capture_controls: &[Option<[f32; 16]>],
) -> Option<Vec<[f32; 16]>> {
    if locals.len() != hierarchy.pivots.len()
        || capture_controls.len() != hierarchy.pivots.len()
        || hierarchy.pivots.is_empty()
        || hierarchy.pivots[0].parent_idx != u32::MAX
    {
        return None;
    }

    let mut globals: Vec<[f32; 16]> = vec![[0.0; 16]; hierarchy.pivots.len()];
    globals[0] = Mat4::IDENTITY.to_cols_array();
    for (pivot_index, pivot) in hierarchy.pivots.iter().enumerate().skip(1) {
        let parent_index = usize::try_from(pivot.parent_idx).ok()?;
        // Source HTree stores parent pivots before children. Main has no
        // recursive malformed-order evaluator, so do not fabricate a parent.
        if parent_index >= pivot_index {
            return None;
        }
        let mut global = mat4_mul(&globals[parent_index], &locals[pivot_index]);
        if let Some(control) = capture_controls[pivot_index] {
            global = mat4_mul(&global, &control);
        }
        globals[pivot_index] = global;
    }
    Some(globals)
}

/// As [`compute_htree_global_transforms_from_locals_with_capture_controls`],
/// without C++ `Capture_Bone` controls.  Every runtime HTree caller goes
/// through this wrapper so pivot zero always remains the external object root
/// rather than leaking W3D bind or HAnim-local data into child transforms.
pub(super) fn compute_htree_global_transforms_from_locals(
    hierarchy: &W3dHierarchy,
    locals: &[[f32; 16]],
) -> Option<Vec<[f32; 16]>> {
    compute_htree_global_transforms_from_locals_with_capture_controls(
        hierarchy,
        locals,
        &vec![None; hierarchy.pivots.len()],
    )
}

/// Compute the HTree bind-pose globals from source W3D pivot data.
///
/// Both static rigid HLOD children and animation sampling use the same hierarchy
/// convention.  Keeping this outside `W3DLoader` prevents a render-time HLOD
/// binding from accidentally depending on loader-only state.
pub(super) fn compute_bind_pose_global_transforms(
    hierarchy: &W3dHierarchy,
) -> Option<Vec<[f32; 16]>> {
    let locals: Vec<[f32; 16]> = hierarchy.pivots.iter().map(mat4_from_pivot).collect();
    compute_htree_global_transforms_from_locals(hierarchy, &locals)
}
