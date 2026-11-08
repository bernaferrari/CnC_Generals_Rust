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
/// HAnim path.  Keep the existing compressed-channel implementation isolated:
/// that format takes the generic path and must not silently inherit raw-frame
/// behavior merely because both records share [`W3dAnimation`].
pub(super) fn sample_animation_local_transforms(
    hierarchy: &W3dHierarchy,
    anim: &W3dAnimation,
    frame: f32,
) -> Option<Vec<[f32; 16]>> {
    if anim.source_is_compressed {
        return sample_compressed_animation_local_transforms(hierarchy, anim, frame);
    }

    if !frame.is_finite() {
        return None;
    }
    let raw_frame = anim.raw_frame_index(frame)?;
    let mut local_transforms: Vec<[f32; 16]> =
        hierarchy.pivots.iter().map(mat4_from_pivot).collect();
    let mut motion_channels: Vec<[Option<&W3dAnimChannel>; 4]> =
        vec![[None; 4]; hierarchy.pivots.len()];

    // `HRawAnimClass::add_channel` keeps one X/Y/Z/Q pointer per pivot and
    // replaces an earlier pointer for the same channel kind.  Preserve that
    // exact final-source-record authority rather than sequentially composing
    // duplicate W3D chunks.
    for channel in &anim.channels {
        let Some(slot) = raw_motion_channel_slot(channel.flags) else {
            continue;
        };
        let pivot_index = usize::from(channel.pivot);
        if pivot_index < motion_channels.len() {
            motion_channels[pivot_index][slot] = Some(channel);
        }
    }

    // C++ sets pivot zero to the external RenderObj root and begins the raw
    // node-motion walk at one. Source pivot-zero channels are intentionally
    // not sampled, including malformed ones.
    for pivot_index in 1..local_transforms.len() {
        let channels = motion_channels[pivot_index];
        let translation = [
            raw_scalar_channel_value(channels[0], raw_frame)?,
            raw_scalar_channel_value(channels[1], raw_frame)?,
            raw_scalar_channel_value(channels[2], raw_frame)?,
        ];
        let rotation = raw_quaternion_channel_value(channels[3], raw_frame)?;

        // `HTreeClass::Anim_Update(HRawAnimClass*)` first obtains
        // parent * BaseTransform, then Matrix3D::Translate and postMul(q).
        // Associativity permits the local equivalent here: Base * T * Q;
        // the shared HTree evaluator supplies the parent afterward.
        let with_translation = mat4_mul(
            &local_transforms[pivot_index],
            &mat4_from_translation(translation),
        );
        local_transforms[pivot_index] =
            mat4_mul(&with_translation, &mat4_from_quaternion(rotation));
    }

    Some(local_transforms)
}

/// The compact source `ANIM_CHANNEL_*` kinds that the Generals raw HAnim path
/// installs in one `NodeMotionStruct`. Other raw motion kinds are retained by
/// the parser but are not consumed by the specialized game update path.
pub(super) fn raw_motion_channel_slot(flags: u16) -> Option<usize> {
    match flags {
        0 => Some(0),
        1 => Some(1),
        2 => Some(2),
        6 => Some(3),
        _ => None,
    }
}

/// `MotionChannelClass::Get_Vector` returns scalar zero outside an authored
/// range. A malformed scalar record is not source-usable, so fail the pose
/// rather than treating a truncated payload as a real zero channel.
pub(super) fn raw_scalar_channel_value(
    channel: Option<&W3dAnimChannel>,
    frame: i32,
) -> Option<f32> {
    let Some(channel) = channel else {
        return Some(0.0);
    };
    if channel.vector_len != 1 || channel.last_frame < channel.first_frame {
        return None;
    }
    let first = i32::from(channel.first_frame);
    let last = i32::from(channel.last_frame);
    if frame < first || frame > last {
        return Some(0.0);
    }
    let index = usize::try_from(frame - first).ok()?;
    channel.data.get(index).copied()
}

/// `MotionChannelClass::Get_Vector_As_Quat` returns the identity quaternion
/// outside an authored range and does not normalize authored raw values.
pub(super) fn raw_quaternion_channel_value(
    channel: Option<&W3dAnimChannel>,
    frame: i32,
) -> Option<[f32; 4]> {
    let Some(channel) = channel else {
        return Some([0.0, 0.0, 0.0, 1.0]);
    };
    if channel.vector_len != 4 || channel.last_frame < channel.first_frame {
        return None;
    }
    let first = i32::from(channel.first_frame);
    let last = i32::from(channel.last_frame);
    if frame < first || frame > last {
        return Some([0.0, 0.0, 0.0, 1.0]);
    }
    let index = usize::try_from(frame - first).ok()?.checked_mul(4)?;
    Some([
        *channel.data.get(index)?,
        *channel.data.get(index + 1)?,
        *channel.data.get(index + 2)?,
        *channel.data.get(index + 3)?,
    ])
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
/// allocating temporary channel vectors. The returned pose is caller-owned.
pub(super) fn sample_compressed_animation_local_transforms(
    hierarchy: &W3dHierarchy,
    anim: &W3dAnimation,
    frame: f32,
) -> Option<Vec<[f32; 16]>> {
    let compressed = anim.compressed_anim.as_ref()?;
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
