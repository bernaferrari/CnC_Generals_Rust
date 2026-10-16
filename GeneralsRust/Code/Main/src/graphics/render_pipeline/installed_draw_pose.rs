//! Persistent native RenderObj data on the existing ObjectVisualState owner.
//! Camera lookup never installs current snapshots as a substitute for missing
//! constructor/condition/transform admission. Those missing producers are
//! returned explicitly until the corresponding native lifecycle is connected.
use super::*;

/// Unified renderer-local state for a single `(ObjectId, Draw-module)`.
/// Animation may be absent while recoil remains meaningful: a selected Draw
/// state with no HAnim is bind pose, not an instruction to lose a real gun
/// recoil event.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ObjectVisualState {
    pub native_source_template: String,
    pub native_source_supported: bool,
    pub native_condition_supported: bool,
    pub native_condition_bits: Option<u128>,
    pub native_pending_condition: Option<crate::assets::AuthoredDrawModel>,
    pub native_selected_condition: Option<crate::assets::AuthoredDrawModel>,
    pub native_drawable_transform: Mat4,
    pub native_transform_revision: Option<u64>,
    pub native_host_identity: Option<crate::assets::LiveDrawPlaybackIdentity>,
    /// Actual Client association allocated after the constructor observer.
    #[cfg(feature = "game_client")]
    pub native_client_binding: Option<game_client::core::PresentationDirectDrawableBindingKey>,

    pub installed_draw_pose: installed_draw_pose::InstalledDrawObject,
    pub identity: Option<FrozenVisualDrawIdentity>,
    pub animation: Option<ObjectAnimationState>,
    pub animation_paused: bool,
    pub animation_resume_mode: Option<crate::assets::AuthoredDrawAnimationMode>,
    /// A restored `None` animation is an explicit saved bind-pose choice.
    /// Keep it distinct from an ordinary cache miss: the latter may become
    /// available after normal prewarm, while a loaded snapshot must never
    /// invent a timeline that was absent when it was saved.
    pub force_bind_pose: bool,
    pub last_seen_weapon_discharge_sequence: u64,
    pub recoil_slots: [Vec<ObjectWeaponRecoilState>; 3],
    pub loco_acceleration_pitch_rate: f32,
    pub loco_acceleration_roll_rate: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) enum InstalledDrawObject {
    #[default]
    AwaitingAdmission,
    NoRenderObject,
    Ready(InstalledDrawPose),
}

#[derive(Debug, Clone)]
pub(super) struct InstalledDrawPose {
    pub(super) model: Arc<crate::assets::W3DModel>,
    pub(super) world: Mat4,
    pub(super) animation_binding: Option<crate::assets::W3dAnimationBinding>,
    pub(super) controls: Vec<(i32, Mat4)>,
    pub(super) turret_names: (Option<String>, Option<String>),
    pub(super) supported_controls: bool,
    pub(super) hierarchy_valid: bool,
    pub(super) sampled_frame: f32,
}

impl PartialEq for InstalledDrawPose {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.model, &other.model)
            && self.world == other.world
            && self.controls == other.controls
            && self.turret_names == other.turret_names
            && self.supported_controls == other.supported_controls
            && self.hierarchy_valid == other.hierarchy_valid
            && self.sampled_frame == other.sampled_frame
            && self.animation_binding.as_ref().map(|b| b.state_key())
                == other.animation_binding.as_ref().map(|b| b.state_key())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeCameraPoseUnavailable {
    UnadmittedDrawableLifecycle,
    NoRenderObject,
    UnsupportedSourceControls,
    Bone(crate::assets::models::BonePoseUnavailable),
}

impl InstalledDrawPose {
    pub(super) fn created(
        model: Arc<crate::assets::W3DModel>,
        world: Mat4,
        draw_model: &crate::assets::AuthoredDrawModel,
        animation_binding: Option<crate::assets::W3dAnimationBinding>,
    ) -> Self {
        Self {
            model,
            world,
            animation_binding,
            controls: Vec::new(),
            turret_names: (
                draw_model.primary_turret.yaw_bone.clone(),
                draw_model.primary_turret.pitch_bone.clone(),
            ),
            supported_controls: draw_model.primary_turret.primary_fields_valid
                && !draw_model.primary_turret.has_unsupported_alternate_turret(),
            hierarchy_valid: false,
            sampled_frame: 0.,
        }
    }
    pub(super) fn can_reuse_for(
        &self,
        model: &crate::assets::W3DModel,
        selected: &crate::assets::AuthoredDrawModel,
    ) -> bool {
        std::ptr::eq(self.model.as_ref(), model)
            && self.supported_controls
            && !selected.primary_turret.has_unsupported_alternate_turret()
            && self.turret_names.0 == selected.primary_turret.yaw_bone
            && self.turret_names.1 == selected.primary_turret.pitch_bone
    }
    pub(super) fn install_transform(&mut self, world: Mat4) {
        self.world = world;
        // Source Set_Transform invalidates even for equal matrices.
        self.hierarchy_valid = false;
    }
    fn install_controls(&mut self, controls: Vec<(i32, Mat4)>) {
        // Capture_Bone state persists until the render object is recreated.
        // Later Control_Bone replaces an earlier control on the same pivot.
        for (index, transform) in controls {
            if let Some(control) = self.controls.iter_mut().find(|control| control.0 == index) {
                control.1 = transform;
            } else {
                self.controls.push((index, transform));
            }
            self.hierarchy_valid = false;
        }
    }
}

impl RenderPipeline {
    /// Real render-object creation (constructor or setModelState recreation).
    /// Transform is retained old render-world, or actual Drawable world when
    /// no render object existed. Caller must install the actual binding clock
    /// in the same ObjectVisualState at this source phase.
    pub(crate) fn admit_native_render_object_creation(
        &mut self,
        id: crate::game_logic::ObjectId,
        module_index: u32,
        model: Arc<crate::assets::W3DModel>,
        draw_model: &crate::assets::AuthoredDrawModel,
        world: Mat4,
        binding: Option<crate::assets::W3dAnimationBinding>,
    ) {
        let state = self
            .drawable_visual_states
            .entry((id.0, module_index))
            .or_default();
        state.installed_draw_pose = InstalledDrawObject::Ready(InstalledDrawPose::created(
            model, world, draw_model, binding,
        ));
    }
    /// Explicit selected Model=None admission. Not the same as not having
    /// reached the constructor/condition-state producer yet.
    pub(crate) fn admit_native_no_render_object(
        &mut self,
        id: crate::game_logic::ObjectId,
        module_index: u32,
    ) {
        self.drawable_visual_states
            .entry((id.0, module_index))
            .or_default()
            .installed_draw_pose = InstalledDrawObject::NoRenderObject;
    }
    /// Actual reactToTransformChange, with source adjustTransformMtx already
    /// applied by the driving owner. Does not change installed animation or
    /// manufacture a render object for an unadmitted/Model=None module.
    pub(crate) fn admit_native_render_transform(
        &mut self,
        id: crate::game_logic::ObjectId,
        module_index: u32,
        world: Mat4,
    ) {
        if let Some(state) = self.drawable_visual_states.get_mut(&(id.0, module_index)) {
            if let InstalledDrawObject::Ready(pose) = &mut state.installed_draw_pose {
                pose.install_transform(world);
            }
        }
    }
    /// Real later Draw producer. This is wired before frustum rejection in
    /// collect, where the actual model, binding, world, and controls exist.
    pub(super) fn install_native_draw_pose(
        &mut self,
        id: crate::game_logic::ObjectId,
        module_index: u32,
        model: Arc<crate::assets::W3DModel>,
        draw_model: &crate::assets::AuthoredDrawModel,
        world: Mat4,
        binding: Option<crate::assets::W3dAnimationBinding>,
        controls: Option<Vec<(i32, Mat4)>>,
        controls_requested: bool,
    ) {
        let state = self
            .drawable_visual_states
            .entry((id.0, module_index))
            .or_default();
        let needs_recreation = match &state.installed_draw_pose {
            InstalledDrawObject::Ready(pose) => {
                !Arc::ptr_eq(&pose.model, &model)
                    || pose.turret_names.0 != draw_model.primary_turret.yaw_bone
                    || pose.turret_names.1 != draw_model.primary_turret.pitch_bone
            }
            _ => true,
        };
        if needs_recreation {
            state.installed_draw_pose = InstalledDrawObject::Ready(InstalledDrawPose::created(
                model,
                world,
                draw_model,
                binding.clone(),
            ));
        }
        let InstalledDrawObject::Ready(pose) = &mut state.installed_draw_pose else {
            unreachable!()
        };
        if pose.animation_binding.as_ref().map(|b| b.state_key())
            != binding.as_ref().map(|b| b.state_key())
        {
            pose.hierarchy_valid = false;
        }
        pose.animation_binding = binding;
        pose.install_transform(world);
        // No HTree/non-controlled plain mesh is valid. A malformed/unsupported
        // turret input remains explicitly unavailable for camera acceptance.
        pose.supported_controls = draw_model.primary_turret.primary_fields_valid
            && !draw_model.primary_turret.has_unsupported_alternate_turret();
        // `None` may mean a non-HTree MeshClass, where no bone exists. For a
        // real named HTree, callers must not silently drop unsupported controls.
        if controls_requested && controls.is_none() {
            pose.supported_controls = false;
        }
        if let Some(controls) = controls {
            pose.install_controls(controls);
        }
    }
    /// Current installed object, exact bone, lazy hierarchy validation.
    /// No global client/View, previous RenderItem, animation-zero, or current
    /// UnitRenderInput fallback. First-interface identity is supplied by the
    /// receiving source declaration selector, independently of visible models.
    pub(crate) fn native_installed_camera_pose(
        &mut self,
        id: crate::game_logic::ObjectId,
        module_index: u32,
        bone_name: &str,
    ) -> Result<Mat4, NativeCameraPoseUnavailable> {
        self.borrow_native_installed_poses()
            .installed_camera_pose(id, module_index, bone_name)
    }
}

impl ObjectVisualState {
    pub(super) fn retained_installed_binding(&self) -> Option<crate::assets::W3dAnimationBinding> {
        if self.force_bind_pose || self.identity.as_ref()?.animation.is_some() {
            return None;
        }
        let InstalledDrawObject::Ready(pose) = &self.installed_draw_pose else {
            return None;
        };
        let binding = pose.animation_binding.as_ref()?;
        let animation = self.animation.as_ref()?;
        (animation.animation_binding_key.as_ref() == Some(&binding.state_key())
            && pose.model.animation_binding_is_compatible(binding))
        .then(|| binding.clone())
    }
    pub(super) fn advance_retained_motion_at_draw(
        &mut self,
        sync_ms: u32,
        should_animate: bool,
    ) -> Option<(crate::assets::W3dAnimationBinding, f32)> {
        let binding = self.retained_installed_binding()?;
        self.install_animation_pause(!should_animate, sync_ms);
        let animation = self.animation.as_ref()?;
        // doDraw Peek/controls install does not progress the HAnim clock.
        Some((binding, animation.current_frame))
    }
    /// Source parent Render entry, after scene eligibility and before any
    /// animation-dependent child topology is frozen. Render progresses once;
    /// it invalidates HTree, whose Update then progresses again at same Sync.
    /// This is deliberately distinct from Get_Bone_Transform's validity gate.
    pub(super) fn admit_parent_render_hierarchy(
        &mut self,
        sync_ms: u32,
    ) -> Result<f32, crate::assets::models::BonePoseUnavailable> {
        use crate::assets::models::BonePoseUnavailable;
        let InstalledDrawObject::Ready(pose) = &mut self.installed_draw_pose else {
            return Err(BonePoseUnavailable::UnsupportedRenderObject);
        };
        // MeshClass has no Animatable3DObj HTree progression even if its file
        // also contains an unrelated hierarchy/animation.
        if pose.model.hlods.is_empty() {
            return Ok(self
                .animation
                .as_ref()
                .map_or(0., |animation| animation.current_frame));
        }
        if let Some(animation) = self.animation.as_mut() {
            if !matches!(
                animation.playback_mode,
                crate::assets::AuthoredDrawAnimationMode::Manual
            ) {
                animation.admit_sync(sync_ms);
                pose.hierarchy_valid = false;
            }
        }
        if !pose.hierarchy_valid {
            if let Some(animation) = self.animation.as_mut() {
                if !matches!(
                    animation.playback_mode,
                    crate::assets::AuthoredDrawAnimationMode::Manual
                ) {
                    animation.admit_sync(sync_ms);
                }
                pose.sampled_frame = animation.current_frame;
            } else {
                pose.sampled_frame = 0.;
            }
            pose.model.source_controlled_hierarchy_valid(
                pose.animation_binding.as_ref(),
                pose.sampled_frame,
                &pose.controls,
            )?;
            pose.hierarchy_valid = true;
        }
        Ok(pose.sampled_frame)
    }
    pub(super) fn installed_camera_pose(
        &mut self,
        sync_ms: u32,
        bone_name: &str,
    ) -> Result<Mat4, NativeCameraPoseUnavailable> {
        use NativeCameraPoseUnavailable::*;
        let pose = match &mut self.installed_draw_pose {
            InstalledDrawObject::AwaitingAdmission => return Err(UnadmittedDrawableLifecycle),
            InstalledDrawObject::NoRenderObject => return Err(NoRenderObject),
            InstalledDrawObject::Ready(pose) => pose,
        };
        if !pose.supported_controls {
            return Err(UnsupportedSourceControls);
        }
        let swap = Mat4::from_cols_array(&[
            1., 0., 0., 0., 0., 0., 1., 0., 0., 1., 0., 0., 0., 0., 0., 1.,
        ]);
        // Original Get_Bone_Index is before Get_Bone_Transform; unresolved names
        // write FULL source identity without advancing even an invalid tree.
        if !pose
            .model
            .has_source_named_camera_bone(bone_name)
            .map_err(Bone)?
        {
            return Ok(swap);
        }
        if !pose.hierarchy_valid {
            if let Some(animation) = self.animation.as_mut() {
                if !matches!(
                    animation.playback_mode,
                    crate::assets::AuthoredDrawAnimationMode::Manual
                ) {
                    animation.admit_sync(sync_ms);
                }
                pose.sampled_frame = animation.current_frame;
            } else {
                pose.sampled_frame = 0.;
            }
        }
        let source = pose
            .model
            .source_named_bone_transform(
                bone_name,
                pose.animation_binding.as_ref(),
                pose.sampled_frame,
                &pose.controls,
            )
            .map_err(Bone)?
            .ok_or(Bone(
                crate::assets::models::BonePoseUnavailable::InvalidHierarchy,
            ))?;
        pose.hierarchy_valid = true;
        Ok(pose.world * swap * source)
    }
}

#[cfg(test)]
#[path = "installed_draw_pose_tests.rs"]
mod tests;
