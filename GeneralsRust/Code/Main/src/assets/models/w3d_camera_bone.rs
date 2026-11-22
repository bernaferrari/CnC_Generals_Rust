//! Exact named HTree observation for a receiving native RenderObj.
//! Proposed external patch; not integrated or accepted yet.
use super::w3d_anim::compute_htree_global_transforms_from_locals_with_capture_controls;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BonePoseUnavailable {
    UnsupportedRenderObject,
    InvalidHierarchy,
    IncompatibleAnimation,
    InvalidControl,
    InvalidPose,
}

impl W3DModel {
    /// Ordered source controls installed by native doDrawModule, shared by
    /// render and named-camera sampling. None is unsupported/malformed input.
    pub(crate) fn native_draw_capture_controls(
        &self,
        primary_turret: &AuthoredDrawPrimaryTurret,
        turret_angle_degrees: f32,
        turret_pitch_degrees: f32,
        alt_turret_degrees: (f32, f32),
        weapon_controls: &[W3dWeaponVisualControl],
    ) -> Option<Vec<(i32, Mat4)>> {
        let (_, hierarchy) = self.rigid_hlod_context()?;
        let mut capture_controls = Vec::new();
        for (bone_index, transform) in Self::turret_capture_controls(
            hierarchy,
            primary_turret,
            (turret_angle_degrees, turret_pitch_degrees),
            alt_turret_degrees,
        ) {
            capture_controls.push((i32::try_from(bone_index).ok()?, transform));
        }

        for control in weapon_controls {
            let Some(pivot_index) = control
                .recoil_pivot_index
                .and_then(|index| usize::try_from(index).ok())
                .filter(|index| *index != 0 && *index < hierarchy.pivots.len())
            else {
                continue;
            };
            if !control.recoil_shift.is_finite() || control.recoil_shift < 0.0 {
                return None;
            }
            // `handleClientRecoil` runs after turret positioning. Its later
            // slot/barrel control replaces an earlier control on the same
            // source pivot, exactly as `HTreeClass::Control_Bone` does.
            capture_controls.push((
                i32::try_from(pivot_index).ok()?,
                Mat4::from_translation(Vec3::new(-control.recoil_shift, 0.0, 0.0)),
            ));
        }

        Some(capture_controls)
    }

    /// `Ok(None)` is the original Get_Bone_Index zero sentinel, including for
    /// a valid non-animatable mesh. The caller can still commit source identity.
    pub(crate) fn source_named_bone_index(
        &self,
        bone_name: &str,
    ) -> Result<Option<usize>, BonePoseUnavailable> {
        use BonePoseUnavailable::*;
        // These prototypes have separate child/tree selection rules. A camera
        // must report unavailable until their exact query is implemented; it
        // may not pick the first parsed hierarchy or a rendered mesh palette.
        if self.hlod_parse_failed
            || self.hlods.len() > 1
            || !self.hmodels.is_empty()
            || !self.collections.is_empty()
            || !self.aggregates.is_empty()
            || !self.dist_lods.is_empty()
        {
            return Err(UnsupportedRenderObject);
        }
        // A standalone MeshClass inherits RenderObjClass::Get_Bone_Index=0,
        // even if its W3D file also happened to contain an unrelated HTree.
        if self.hlods.is_empty() {
            return Ok(None);
        }
        let Some(hierarchy) = self.hierarchy_for_sampled_hlod_or_legacy() else {
            return if self.hlods.is_empty() && self.hierarchy.is_none() {
                // MeshClass inherits RenderObjClass zero-bone lookup.
                Ok(None)
            } else {
                Err(InvalidHierarchy)
            };
        };
        let Some(index) = hierarchy
            .pivots
            .iter()
            .position(|pivot| pivot.name.eq_ignore_ascii_case(bone_name))
            .filter(|index| *index != 0)
        else {
            return Ok(None);
        };
        Ok(Some(index))
    }
    /// Source-local W3D basis. Camera axes compose with `S * local`.
    pub(crate) fn checked_source_bone_transform(
        globals: &[[f32; 16]],
        index: usize,
    ) -> Result<Mat4, BonePoseUnavailable> {
        use BonePoseUnavailable::*;
        let source = globals.get(index).ok_or(InvalidHierarchy)?;
        let transform = Mat4::from_cols_array(source);
        if !Self::capture_control_transform_is_affine(transform) {
            return Err(InvalidPose);
        }
        Ok(transform)
    }

    pub(crate) fn source_controlled_hierarchy_transforms(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        asset_scale: f32,
        capture_controls: &[(i32, Mat4)],
    ) -> Result<Vec<[f32; 16]>, BonePoseUnavailable> {
        use BonePoseUnavailable::*;
        let hierarchy = self
            .hierarchy_for_sampled_hlod_or_legacy()
            .ok_or(InvalidHierarchy)?;
        let mut locals = self
            .local_transforms_for_animation_binding(animation_binding, animation_frame)
            .ok_or(IncompatibleAnimation)?;
        // HTreeClass::Scale scales the base pivot and animation translations.
        // Apply that baked asset transform before live Control_Bone offsets.
        if asset_scale != 1.0 {
            for local in &mut locals {
                for translation in &mut local[12..15] {
                    *translation *= asset_scale;
                }
            }
        }
        let mut controls = vec![None; hierarchy.pivots.len()];
        for (raw_index, transform) in capture_controls {
            let index = usize::try_from(*raw_index)
                .ok()
                .filter(|index| *index != 0 && *index < controls.len())
                .ok_or(InvalidControl)?;
            if !Self::capture_control_transform_is_affine(*transform) {
                return Err(InvalidControl);
            }
            // Ordered Control_Bone replaces an earlier control on same pivot.
            controls[index] = Some(transform.to_cols_array());
        }
        let globals = compute_htree_global_transforms_from_locals_with_capture_controls(
            hierarchy, &locals, &controls,
        )
        .ok_or(InvalidPose)?;
        Ok(globals)
    }
}
