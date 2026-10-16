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
        weapon_controls: &[W3dWeaponVisualControl],
    ) -> Option<Vec<(i32, Mat4)>> {
        let (_, hierarchy) = self.rigid_hlod_context()?;
        let mut capture_controls = Vec::new();
        if primary_turret.primary_fields_valid && !primary_turret.has_unsupported_alternate_turret()
        {
            if let Some((bone_index, transform)) = primary_turret
                .yaw_bone
                .as_deref()
                .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
                .and_then(|bone_index| {
                    Self::primary_turret_angle_radians(
                        turret_angle_degrees,
                        primary_turret.yaw_art_angle_radians(),
                    )
                    .map(|angle| (bone_index, Mat4::from_rotation_z(angle)))
                })
            {
                capture_controls.push((i32::try_from(bone_index).ok()?, transform));
            }
            if let Some((bone_index, transform)) = primary_turret
                .pitch_bone
                .as_deref()
                .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
                .and_then(|bone_index| {
                    Self::primary_turret_angle_radians(
                        turret_pitch_degrees,
                        primary_turret.pitch_art_angle_radians(),
                    )
                    .map(|angle| (bone_index, Mat4::from_rotation_y(-angle)))
                })
            {
                // `handleClientTurretPositioning` controls yaw before pitch.
                // Preserve that order, including a malformed same-pivot
                // source where pitch intentionally replaces yaw.
                capture_controls.push((i32::try_from(bone_index).ok()?, transform));
            }
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

    pub(crate) fn has_source_named_camera_bone(
        &self,
        bone_name: &str,
    ) -> Result<bool, BonePoseUnavailable> {
        Ok(self.source_named_camera_bone_index(bone_name)?.is_some())
    }

    fn source_named_camera_bone_index(
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
    /// `Ok(None)` is C++ Get_Bone_Index's zero sentinel, including a valid
    /// non-animatable mesh. It is NOT an unavailable render object. The caller
    /// must commit SOURCE identity for this result, independently of the
    /// installed object's world transform.
    ///
    /// `Ok(Some)` stays source-local W3D basis. Compose the receiving installed
    /// render-world transform with `S * local` for a CAMERA, rather than the
    /// usual mesh `S * local * S`: camera-local axes are not mesh-local axes.
    pub(crate) fn source_named_bone_transform(
        &self,
        bone_name: &str,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        capture_controls: &[(i32, Mat4)],
    ) -> Result<Option<Mat4>, BonePoseUnavailable> {
        use BonePoseUnavailable::*;
        let Some(index) = self.source_named_camera_bone_index(bone_name)? else {
            return Ok(None);
        };
        let globals = self.source_controlled_hierarchy_transforms(
            animation_binding,
            animation_frame,
            capture_controls,
        )?;
        let source = globals.get(index).ok_or(InvalidHierarchy)?;
        let transform = Mat4::from_cols_array(source);
        if !Self::capture_control_transform_is_affine(transform) {
            return Err(InvalidPose);
        }
        Ok(Some(transform))
    }
    pub(crate) fn source_controlled_hierarchy_valid(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        capture_controls: &[(i32, Mat4)],
    ) -> Result<(), BonePoseUnavailable> {
        // MeshClass has no Animatable3DObj hierarchy to validate. This does
        // not turn a parsed unrelated HTree/clip into its live render object.
        if self.hlods.is_empty()
            && self.hmodels.is_empty()
            && self.collections.is_empty()
            && self.aggregates.is_empty()
            && self.dist_lods.is_empty()
            && !self.hlod_parse_failed
        {
            return if animation_binding.is_none() && capture_controls.is_empty() {
                Ok(())
            } else {
                Err(BonePoseUnavailable::UnsupportedRenderObject)
            };
        }
        self.source_controlled_hierarchy_transforms(
            animation_binding,
            animation_frame,
            capture_controls,
        )
        .map(|_| ())
    }

    fn source_controlled_hierarchy_transforms(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        capture_controls: &[(i32, Mat4)],
    ) -> Result<Vec<[f32; 16]>, BonePoseUnavailable> {
        use BonePoseUnavailable::*;
        let hierarchy = self
            .hierarchy_for_sampled_hlod_or_legacy()
            .ok_or(InvalidHierarchy)?;
        let locals = self
            .local_transforms_for_animation_binding(animation_binding, animation_frame)
            .ok_or(IncompatibleAnimation)?;
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
