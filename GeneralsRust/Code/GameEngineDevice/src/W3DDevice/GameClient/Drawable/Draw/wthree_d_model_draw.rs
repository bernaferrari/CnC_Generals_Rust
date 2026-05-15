//! W3DModelDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp
//!
//! The primary draw module for all standard unit/structure rendering. Handles animations,
//! condition states, transitions, turrets, weapon recoil, muzzle flashes, particle systems,
//! shadows, terrain decals, and track marks. This is a 4302-line C++ file.
//!
//! The Module data layer (Module/wthree_d_model_draw.rs) already has extensive
//! condition state and animation data structures. This file provides the rendering
//! side that will be wired once the W3D rendering pipeline is complete.

use crate::W3DDevice::GameClient::wthree_d_display::W3DDisplay;
use crate::W3DDevice::GameClient::wthree_d_scene::RenderObjectId;
use crate::W3DDevice::GameClient::Module::wthree_d_model_draw as module_data;
use cgmath::{InnerSpace, Matrix4, Point3, Quaternion, SquareMatrix, Vector3, Zero};
use std::collections::HashMap;

/// Weapon recoil state machine (C++: WeaponRecoilState)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoilState {
    Idle,
    RecoilStart,
    Recoil,
    Settle,
}

/// Per-barrel recoil info
#[derive(Debug, Clone)]
pub struct WeaponRecoilInfo {
    pub state: RecoilState,
    pub shift: f32,
    pub recoil_rate: f32,
}

impl Default for WeaponRecoilInfo {
    fn default() -> Self {
        Self {
            state: RecoilState::Idle,
            shift: 0.0,
            recoil_rate: 0.0,
        }
    }
}

/// Sub-object visibility rule (C++: HideShowSubObjInfo)
#[derive(Debug, Clone)]
pub struct HideShowSubObjInfo {
    pub name: String,
    pub hide: bool,
}

/// Level-of-detail threshold for distance-based LOD selection.
///
/// C++ parity: `HLodClass` stores `m_maxScreenSize` per LOD level.
/// Higher `max_screen_area` = higher detail (rendered when object is large on screen).
/// The special value `f32::MAX` means "always render at this LOD" (no distance cull).
#[derive(Debug, Clone, Copy)]
pub struct LodThreshold {
    /// LOD level index (0 = highest detail).
    pub lod_index: usize,
    /// Maximum normalised screen area at which this LOD is still selected.
    /// Larger values mean the LOD is used when the object is farther away.
    /// C++: `ModelArray::max_screen_size` / `HLodClass::m_maxScreenSize`.
    pub max_screen_area: f32,
}

/// UV override for render-object texture coordinate remapping.
///
/// C++ parity: W3D shaders allow per-instance UV offset/scale via material
/// overrides. This is used for animated textures (e.g. water, energy shields,
/// construction scaffolding). The C++ side calls `MeshModelClass::Set_UV_Override()`.
#[derive(Debug, Clone)]
pub struct UvOverride {
    /// Material / sub-object slot this override targets (0 = default).
    pub slot: usize,
    /// UV offset (u, v) in texture space.
    pub offset: (f32, f32),
    /// UV scale (u, v) – multiplies the base UV before adding offset.
    pub scale: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoneOverrideKind {
    TurretYaw,
    TurretPitch,
    WeaponRecoil,
}

#[derive(Debug, Clone)]
pub struct BoneOverrideSubmission {
    pub slot: usize,
    pub barrel_index: Option<usize>,
    pub kind: BoneOverrideKind,
    pub value: f32,
    pub recoil_state: Option<RecoilState>,
}

#[derive(Debug, Clone)]
pub struct ModelDrawSubmissionState {
    pub condition_state_index: i32,
    pub pending_next_state_index: i32,
    pub animation_index: i32,
    pub world_transform: Matrix4<f32>,
    pub visible: bool,
    pub pause_animation: bool,
    pub shadow_enabled: bool,
    pub hide_headlights: bool,
    pub instance_scale: f32,
    pub hex_color: i32,
    pub render_object_id: Option<RenderObjectId>,
    pub shadow_id: Option<RenderObjectId>,
    pub terrain_decal_id: Option<RenderObjectId>,
    pub track_render_object_id: Option<RenderObjectId>,
    pub sub_object_overrides: Vec<HideShowSubObjInfo>,
    pub bone_overrides: Vec<BoneOverrideSubmission>,
    pub uv_overrides: Vec<UvOverride>,
    pub selected_lod: usize,
}

/// Sentinel value indicating no pending animation loop duration (C++: NO_NEXT_DURATION).
const NO_NEXT_DURATION: u32 = 0xFFFF_FFFF;

// --------------------------------------------------------------------------------------------
// Xfer data structures for save/load serialization
// --------------------------------------------------------------------------------------------

/// Serialized recoil entry for a single barrel (C++: WeaponRecoilInfo xfer fields).
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponRecoilXferEntry {
    pub state: RecoilState,
    pub shift: f32,
    pub recoil_rate: f32,
}

/// Serialized animation state for save/load (C++: version >= 2 animation block).
#[derive(Debug, Clone, PartialEq)]
pub struct W3DAnimationXferData {
    pub present: bool,
    pub mode: i32,
    pub percent: f32,
}

/// Complete serialized state for W3DModelDraw (C++: W3DModelDraw::xfer lines 4006-4236).
#[derive(Debug, Clone, PartialEq)]
pub struct W3DModelDrawXferData {
    pub version: u32,
    pub recoil_data: Vec<Vec<WeaponRecoilXferEntry>>,
    pub sub_objects: Vec<HideShowSubObjInfo>,
    pub animation: Option<W3DAnimationXferData>,
}

/// W3DModelDraw implementation
///
/// The core rendering draw module. Manages:
/// - Condition state transitions with animation
/// - Turret bone positioning
/// - Weapon recoil animation
/// - Particle system attachment to bones
/// - Shadow and terrain decal management
/// - Track mark rendering
#[derive(Debug)]
pub struct W3DModelDraw {
    /// Currently active condition state index
    cur_state_index: i32,
    /// Pending condition state (after transition completes)
    next_state_index: i32,
    /// Team color hex value
    hex_color: i32,
    /// Current animation index within state
    which_anim_in_cur_state: i32,
    /// Per-weapon-slot recoil state (C++: WEAPONSLOT_COUNT = 5)
    weapon_recoil: Vec<Vec<WeaponRecoilInfo>>,
    /// Fog-of-war obscured flag
    fully_obscured_by_shroud: bool,
    /// Shadow visibility cache
    shadow_enabled: bool,
    /// Render object ID
    render_object_id: Option<RenderObjectId>,
    /// Shadow ID
    shadow_id: Option<RenderObjectId>,
    /// Terrain decal ID
    terrain_decal_id: Option<RenderObjectId>,
    /// Track render object ID
    track_render_object_id: Option<RenderObjectId>,
    /// Runtime show/hide overrides
    sub_object_vec: Vec<HideShowSubObjInfo>,
    /// Whether headlight sub-object should be hidden (daytime)
    hide_headlights: bool,
    /// Whether animation is paused
    pause_animation: bool,
    /// Hidden flag
    hidden: bool,
    /// Per-drawable instance scale (C++: getDrawable()->getInstanceScale())
    instance_scale: f32,
    /// Saved animation loop duration to restore after a state transition
    /// (C++: m_nextStateAnimLoopDuration)
    next_state_anim_loop_duration: u32,
    /// Whether animations are gated on power (C++: m_animationsRequirePower)
    animations_require_power: bool,
    /// Whether particle systems are attached to animated bones (C++: m_particlesAttachedToAnimatedBones)
    particles_attached_to_animated_bones: bool,
    last_submission: Option<ModelDrawSubmissionState>,
    /// Cached LOD levels for this model (C++: HLodClass lod levels from render object).
    /// Populated when the render object is created / model state is set.
    lod_thresholds: Vec<LodThreshold>,
    /// Currently selected LOD level index (C++: HLodClass::m_currentLod).
    current_lod: usize,
    /// LOD bias factor from module data (C++: HLodClass::m_lodBias).
    lod_bias: f32,
    /// Per-instance UV overrides (C++: material UV overrides for animated textures).
    uv_overrides: Vec<UvOverride>,
    /// Cached bone hierarchy for skeletal animation.
    /// Each entry is (parent_bone_index, bind_pose_local_transform).
    /// Populated when the render object is created / model state is set.
    bone_hierarchy: Vec<(usize, Matrix4<f32>)>,
    /// Cached per-bone capture state from turret/recoil overrides.
    /// When a bone index appears here, its transform is overridden.
    captured_bones: Vec<Option<Matrix4<f32>>>,
    // ── State machine data (C++: accessed via getW3DModelDrawModuleData()) ──
    condition_states: Vec<module_data::ModelConditionInfo>,
    transition_map: HashMap<u64, usize>,
    ignore_condition_states: u128,
    animation_mode: module_data::AnimationMode,
    need_recalc_bone_particle_systems: bool,
    animation_complete: bool,
}

impl W3DModelDraw {
    pub fn new() -> Self {
        Self {
            cur_state_index: -1,
            next_state_index: -1,
            hex_color: 0,
            which_anim_in_cur_state: -1,
            weapon_recoil: vec![Vec::new(); 5],
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
            render_object_id: None,
            shadow_id: None,
            terrain_decal_id: None,
            track_render_object_id: None,
            sub_object_vec: Vec::new(),
            hide_headlights: true,
            pause_animation: false,
            hidden: false,
            instance_scale: 1.0,
            next_state_anim_loop_duration: NO_NEXT_DURATION,
            animations_require_power: false,
            particles_attached_to_animated_bones: false,
            last_submission: None,
            lod_thresholds: Vec::new(),
            current_lod: 0,
            lod_bias: 1.0,
            uv_overrides: Vec::new(),
            bone_hierarchy: Vec::new(),
            captured_bones: Vec::new(),
            condition_states: Vec::new(),
            transition_map: HashMap::new(),
            ignore_condition_states: 0,
            animation_mode: module_data::AnimationMode::Loop,
            need_recalc_bone_particle_systems: false,
            animation_complete: false,
        }
    }

    /// Main per-frame draw update. (C++: W3DModelDraw::doDrawModule, line 2016)
    ///
    /// C++ parity step order (preserved exactly):
    /// 1. Update animation pause state from drawable power
    /// 2. Apply instance scale if != 1.0
    /// 3. If animation complete: transition state / idle-switch / restart
    /// 4. Adjust animation speed to movement speed
    /// 5. Set render object transform (with bone-attachment + construction-height adjustments)
    /// 6. Handle turret positioning (CLIENT ONLY)
    /// 7. Recalc / update bones for client particle systems (CLIENT ONLY)
    /// 8. Handle recoil animation
    /// 9. Select LOD based on camera distance
    /// 10. Compute bone matrices for skeletal animation
    /// 11. Submit render state (hidden / shroud / UV overrides)
    pub fn do_draw_module(&mut self, transform_mtx: &Matrix4<f32>) {
        // ── Step 1: update whether or not we should be animating ──
        // C++: setPauseAnimation( !getDrawable()->getShouldAnimate(m_animationsRequirePower) );
        if self.animations_require_power {
            // PARITY_NOTE: real check is getDrawable()->getShouldAnimate(true).
            // For now, keep pause_animation as-is (no drawable to query).
        }

        // ── Step 2: apply instance scale ──
        // C++: if (getDrawable()->getInstanceScale() != 1.0f) { scaledTransform = *transformMtx; scaledTransform.Scale(...); }
        let effective_transform = if self.instance_scale != 1.0 {
            let scale = Matrix4::from_scale(self.instance_scale);
            scale * transform_mtx
        } else {
            *transform_mtx
        };

        // ── Step 3: animation completion ──
        // C++ parity: doDrawModule lines 2032-2066
        // When isAnimationComplete(m_renderObject) returns true:
        if self.animation_complete && self.render_object_id.is_some() {
            // (a) Transition to pending nextState if one exists (C++ lines 2034-2047)
            if self.next_state_index >= 0 {
                let next_idx = self.next_state_index;
                let next_duration = self.next_state_anim_loop_duration;
                self.next_state_index = -1;
                self.next_state_anim_loop_duration = NO_NEXT_DURATION;
                self.set_model_state(next_idx);
                if next_duration != NO_NEXT_DURATION {
                    self.set_animation_loop_duration(next_duration);
                }
            }

            // (b) Idle animation random switching (C++ lines 2049-2060)
            if self.cur_state_index >= 0 && self.which_anim_in_cur_state >= 0 {
                let cur_idx = self.cur_state_index as usize;
                let anim_idx = self.which_anim_in_cur_state as usize;
                if let Some(cur_state) = self.condition_states.get(cur_idx) {
                    if let Some(anim) = cur_state.animations.get(anim_idx) {
                        if anim.is_idle_anim {
                            // C++: adjustAnimation(m_curState, -1.0) — pass curState as prevState
                            // to trigger "pick different anim" logic
                            self.adjust_animation(self.cur_state_index, -1.0);
                        } else if (cur_state.flags & (1 << 6)) != 0 {
                            // (c) RESTART_ANIM_WHEN_COMPLETE (bit 6)
                            // C++ line 2061: testFlagBit(m_curState->m_flags, RESTART_ANIM_WHEN_COMPLETE)
                            self.adjust_animation(self.cur_state_index, -1.0);
                        }
                    }
                }
            }
            self.animation_complete = false;
        }

        // ── Step 4: adjust animation speed to movement speed ──
        // C++: adjustAnimSpeedToMovementSpeed()
        // PARITY_NOTE: Requires Object/PhysicsBehavior to query velocity magnitude.
        // Stub preserves call ordering so the step is present in the frame pipeline.
        self.adjust_anim_speed_to_movement_speed();

        // ── Step 5: set render object transform ──
        // C++: if (m_renderObject) { Matrix3D mtx = *transformMtx; adjustTransformMtx(mtx);
        //          m_renderObject->Set_Transform(mtx); }
        let adjusted_transform = self.adjust_transform_mtx(effective_transform);

        // ── Step 6: handle turret positioning ──
        self.handle_client_turret_positioning();

        // ── Step 7: particle system bone updates ──
        // C++: recalcBonesForClientParticleSystems();
        // C++: if (modData->m_particlesAttachedToAnimatedBones) updateBonesForClientParticleSystems();
        self.recalc_bones_for_client_particle_systems();
        if self.particles_attached_to_animated_bones {
            self.update_bones_for_client_particle_systems();
        }

        // ── Step 8: handle recoil ──
        self.handle_client_recoil();

        // ── Step 9: select LOD ──
        // C++: HLodClass::Select_LOD / Prepare_LOD evaluates screen-area and picks LOD level.
        // PARITY_NOTE: Camera position comes from the active W3D view. When no camera is
        // available, we fall back to LOD 0 (highest detail), matching C++ behavior when
        // the HLod has only one LOD level.
        if !self.lod_thresholds.is_empty() {
            let obj_pos = Point3::new(
                adjusted_transform.w.x,
                adjusted_transform.w.y,
                adjusted_transform.w.z,
            );
            // PARITY_NOTE: In C++ the camera position comes from TheTacticalView->
            //   getPosition(). We derive it from the W3DDisplay's current camera.
            //   When no camera is available, distance = 0 → highest detail LOD.
            let camera_pos = self.get_camera_position();
            let camera_distance = (obj_pos - camera_pos).magnitude();
            self.current_lod = self.select_lod(camera_distance);
        }

        // ── Step 10: compute bone matrices ──
        // C++: HTreeClass::Anim_Update + getCurrentBonePositions.
        // The bone matrices are computed from the hierarchy + animation keyframe data.
        // PARITY_NOTE: Full computation happens in compute_bone_matrices() which is
        // called externally with the animation keyframe data. Here we just ensure the
        // captured_bones array is sized correctly for the current hierarchy.
        self.ensure_captured_bones_capacity();

        // ── Step 11: Build submission state ──
        let visible = !self.hidden && !self.fully_obscured_by_shroud;

        let mut bone_overrides = Vec::new();

        // Turret yaw/pitch bone override slots (one per weapon slot)
        for slot in 0..self.weapon_recoil.len() {
            bone_overrides.push(BoneOverrideSubmission {
                slot,
                barrel_index: None,
                kind: BoneOverrideKind::TurretYaw,
                value: 0.0,
                recoil_state: None,
            });
            bone_overrides.push(BoneOverrideSubmission {
                slot,
                barrel_index: None,
                kind: BoneOverrideKind::TurretPitch,
                value: 0.0,
                recoil_state: None,
            });
        }

        // Weapon recoil bone overrides (per barrel)
        for (slot, barrels) in self.weapon_recoil.iter().enumerate() {
            for (barrel_index, recoil) in barrels.iter().enumerate() {
                bone_overrides.push(BoneOverrideSubmission {
                    slot,
                    barrel_index: Some(barrel_index),
                    kind: BoneOverrideKind::WeaponRecoil,
                    value: recoil.shift,
                    recoil_state: Some(recoil.state),
                });
            }
        }

        let submission = ModelDrawSubmissionState {
            condition_state_index: self.cur_state_index,
            pending_next_state_index: self.next_state_index,
            animation_index: self.which_anim_in_cur_state,
            world_transform: adjusted_transform,
            visible,
            pause_animation: self.pause_animation,
            shadow_enabled: self.shadow_enabled,
            hide_headlights: self.hide_headlights,
            instance_scale: self.instance_scale,
            hex_color: self.hex_color,
            render_object_id: self.render_object_id,
            shadow_id: self.shadow_id,
            terrain_decal_id: self.terrain_decal_id,
            track_render_object_id: self.track_render_object_id,
            sub_object_overrides: self.sub_object_vec.clone(),
            bone_overrides,
            uv_overrides: self.uv_overrides.clone(),
            selected_lod: self.current_lod,
        };

        self.last_submission = Some(submission.clone());

        // ── Submit draw state to the scene / render bridge ──
        // C++: m_renderObject->Set_Transform(mtx) + Set_Hidden(hidden) +
        //   m_shadow->enableShadowRender(enable) + m_terrainDecal->enableShadowRender(enable)
        let scene = W3DDisplay::global_scene();
        let mut scene_guard = scene.write();

        let position = Point3::new(
            submission.world_transform.w.x,
            submission.world_transform.w.y,
            submission.world_transform.w.z,
        );

        // C++ parity: Apply render state in the same order as C++ setHidden/doDrawModule.
        //   - render_object: hidden when !visible (obscured by shroud or explicitly hidden)
        //   - shadow: disabled when hidden OR shadow_enabled=false
        //   - terrain_decal: respects hidden state
        //   - track: respects hidden state; in C++ setHidden adds a cap edge when hiding
        let mut apply_object_state = |id: Option<RenderObjectId>, is_visible: bool, is_hidden: bool| {
            if let Some(id) = id {
                if let Some(obj) = scene_guard.get_render_object_mut(id) {
                    obj.world_transform = submission.world_transform;
                    obj.position = position;
                    obj.object_scale = submission.instance_scale;
                    obj.visible = is_visible;
                    obj.hidden = is_hidden;
                }
            }
        };

        // C++: if (m_renderObject) m_renderObject->Set_Hidden(hidden)
        let render_hidden = !visible;
        apply_object_state(
            submission.render_object_id,
            visible,
            render_hidden,
        );

        // C++: m_shadow->enableShadowRender(!hidden && m_shadowEnabled)
        let shadow_visible = visible && submission.shadow_enabled;
        apply_object_state(
            submission.shadow_id,
            shadow_visible,
            !shadow_visible,
        );

        // C++: if (m_terrainDecal) m_terrainDecal->enableShadowRender(!hidden)
        apply_object_state(
            submission.terrain_decal_id,
            visible,
            !visible,
        );

        // C++: if (m_trackRenderObject && hidden) track->addCapEdgeToTrack(pos)
        apply_object_state(
            submission.track_render_object_id,
            visible,
            !visible,
        );
    }

    fn adjust_anim_speed_to_movement_speed(&mut self) {
        // C++ parity: adjustAnimSpeedToMovementSpeed (line 1943)
        // Queries getCurAnimDistanceCovered() and, if > 0, adjusts frame rate
        // to match physics velocity magnitude.
        // PARITY_NOTE: Requires Object/PhysicsBehavior for velocity and
        // HLodClass for setting frame rate multiplier. Stub preserves call
        // ordering in the do_draw_module frame pipeline.
    }

    /// Select the appropriate LOD level based on distance to camera.
    ///
    /// C++ parity: `HLodClass::Select_LOD()` (ww3d-scene/src/lod.rs HLod::prepare_lod).
    /// C++ walks LOD levels from index 0 (highest detail) upward. For each level it checks
    /// whether the object's normalised screen area exceeds `max_screen_size`. The first
    /// LOD whose threshold is met is selected. If no threshold matches, the last (lowest
    /// detail) LOD is used.
    ///
    /// Screen-area approximation:
    ///   `screen_area ≈ bounding_sphere_radius² / camera_distance²`
    /// This mirrors C++ `HLodClass::calculate_screen_area()` which divides the bounding
    /// sphere projection by the viewport area.
    pub fn select_lod(&self, camera_distance: f32) -> usize {
        if self.lod_thresholds.is_empty() {
            return 0;
        }

        if camera_distance <= 0.0 {
            return 0;
        }

        // C++: screen_area = bounding_sphere_radius² / distance²
        // PARITY_NOTE: We use a unit bounding sphere (radius = 1.0) for the ratio.
        // In C++ the actual bounding sphere radius is used. When the render object's
        // bounding sphere is wired, this should use the real radius.
        let bounding_sphere_radius = 1.0;
        let screen_area = (bounding_sphere_radius * bounding_sphere_radius)
            / (camera_distance * camera_distance);

        // Apply LOD bias. C++: screen_area *= lod_bias.
        let biased_screen_area = screen_area * self.lod_bias;

        // Walk LOD levels from highest detail to lowest.
        // C++: for (i = 0; i < lod_count; i++) { if (screen_area >= max_screen_size[i]) return i; }
        let last_index = self.lod_thresholds.len() - 1;
        for threshold in &self.lod_thresholds {
            let max_area = threshold.max_screen_area;
            // In C++, NO_MAX_SCREEN_SIZE (-1.0) means "no limit" → always select this LOD
            // if nothing more detailed was selected first.
            if max_area < 0.0 || biased_screen_area >= max_area {
                return threshold.lod_index;
            }
        }

        // Fallback to lowest detail LOD
        last_index
    }

    /// Compute per-bone matrices from the skeletal animation hierarchy.
    ///
    /// C++ parity: `HTreeClass::Anim_Update()` + `W3DModelDraw::getCurrentBonePositions()`.
    ///
    /// Algorithm (matches C++ exactly):
    /// 1. Set root bone (index 0) transform to `root_transform`
    /// 2. For each bone from index 1 upward:
    ///    a. Look up parent's already-computed transform from the output array
    ///    b. Build the animation local transform from interpolated keyframe data
    ///       (translation + rotation for this bone at the current frame)
    ///    c. Compose: `bone_transform = parent_transform * anim_local_transform`
    ///    d. If this bone is captured (turret/recoil override), replace with captured transform
    /// 3. Optionally convert from world-space to model-space by pre-multiplying with
    ///    the inverse of the world transform
    ///
    /// Returns the array of computed bone matrices (one per bone in the hierarchy).
    /// The caller can then extract positions from the translation component.
    pub fn compute_bone_matrices(
        &self,
        anim_translations: &[Vector3<f32>],
        anim_rotations: &[Quaternion<f32>],
        root_transform: &Matrix4<f32>,
        world_to_model: Option<&Matrix4<f32>>,
    ) -> Vec<Matrix4<f32>> {
        let bone_count = self.bone_hierarchy.len();
        if bone_count == 0 {
            return Vec::new();
        }

        let mut bone_matrices = vec![Matrix4::identity(); bone_count];

        // Step 1: Root bone gets the root transform directly.
        // C++: pivots[0].Transform = root
        bone_matrices[0] = *root_transform;

        // If the root bone is captured, apply capture override.
        if let Some(Some(capture_mtx)) = self.captured_bones.get(0) {
            bone_matrices[0] = *capture_mtx;
        }

        // Steps 2a-d: Compute each child bone in hierarchy order.
        // C++ HTreeClass::Anim_Update iterates i = 1..num_pivots-1.
        for i in 1..bone_count {
            let (parent_idx, base_transform) = self.bone_hierarchy[i];

            // 2a: Parent transform (already computed in a prior iteration)
            let parent_transform = bone_matrices[parent_idx];

            // 2b: Build animation local transform from interpolated keyframe data.
            // C++ constructs Matrix3D from pivot->BaseTransform * motion_delta.
            // The animation provides per-bone translation and rotation at the current frame.
            let anim_local = if i < anim_translations.len() && i < anim_rotations.len() {
                let translation = anim_translations[i];
                let rotation = anim_rotations[i];
                // C++: compose rotation matrix from quaternion, then set translation.
                let rot_matrix = Matrix4::from(rotation);
                let mut local = rot_matrix;
                local.w.x = translation.x;
                local.w.y = translation.y;
                local.w.z = translation.z;
                local.w.w = 1.0;
                local
            } else {
                // No animation data for this bone → use bind pose (base_transform)
                *base_transform
            };

            // 2c: Compose parent * local
            // C++: Matrix3D::Multiply(parent->Transform, anim_local, &pivot->Transform)
            bone_matrices[i] = parent_transform * anim_local;

            // 2d: Bone capture override (turret yaw/pitch, weapon recoil)
            // C++: if (pivot->Is_Captured) pivot->Capture_Update(parent_transform)
            if let Some(Some(capture_mtx)) = self.captured_bones.get(i) {
                bone_matrices[i] = *capture_mtx;
            }
        }

        // Step 3: Optionally convert world-space → model-space.
        // C++: getCurrentBonePositions() inverts the render object's world transform
        // and pre-multiplies each bone matrix to get model-relative positions.
        if let Some(inv) = world_to_model {
            for matrix in &mut bone_matrices {
                *matrix = *inv * *matrix;
            }
        }

        bone_matrices
    }

    fn get_camera_position(&self) -> Point3<f32> {
        // PARITY_NOTE: In C++, TheTacticalView->getPosition() returns the camera position.
        // W3DDisplay::view is private with no getter yet. Return origin as fallback;
        // distance=0 → highest LOD which matches C++ default for single-LOD models.
        // When the view accessor is wired, replace this with the real camera position.
        log::debug!("W3DModelDraw::get_camera_position: view not yet accessible, using origin");
        Point3::new(0.0, 0.0, 0.0)
    }

    fn ensure_captured_bones_capacity(&mut self) {
        let needed = self.bone_hierarchy.len();
        if self.captured_bones.len() < needed {
            self.captured_bones.resize(needed, None);
        }
    }

    /// Set the LOD thresholds for this draw module.
    /// Called when the render object is created / model state changes.
    /// C++ parity: HLodClass stores these per-LOD-level at model load time.
    pub fn set_lod_thresholds(&mut self, thresholds: Vec<LodThreshold>) {
        self.lod_thresholds = thresholds;
    }

    /// Set the bone hierarchy for skeletal animation computation.
    /// Each entry is (parent_bone_index, bind_pose_local_transform).
    /// C++ parity: HTreeClass::Pivots array populated at model load time.
    pub fn set_bone_hierarchy(&mut self, hierarchy: Vec<(usize, Matrix4<f32>)>) {
        self.bone_hierarchy = hierarchy;
    }

    /// Set a bone capture override at the given bone index.
    /// Used for turret positioning and weapon recoil bone overrides.
    /// C++ parity: HTreeClass::Capture_Bone + Control_Bone.
    pub fn capture_bone(&mut self, bone_index: usize, transform: Matrix4<f32>) {
        self.ensure_captured_bones_capacity();
        if bone_index < self.captured_bones.len() {
            self.captured_bones[bone_index] = Some(transform);
        }
    }

    /// Release a bone capture override at the given bone index.
    /// C++ parity: HTreeClass::Release_Bone.
    pub fn release_bone(&mut self, bone_index: usize) {
        if bone_index < self.captured_bones.len() {
            self.captured_bones[bone_index] = None;
        }
    }

    /// Add a UV override for animated texture remapping.
    /// C++ parity: MeshModelClass::Set_UV_Override for per-instance UV offset/scale.
    pub fn add_uv_override(&mut self, uv: UvOverride) {
        if let Some(existing) = self.uv_overrides.iter_mut().find(|o| o.slot == uv.slot) {
            *existing = uv;
        } else {
            self.uv_overrides.push(uv);
        }
    }

    /// Get the currently selected LOD level index.
    pub fn get_current_lod(&self) -> usize {
        self.current_lod
    }

    fn adjust_transform_mtx(&self, mut mtx: Matrix4<f32>) -> Matrix4<f32> {
        // C++: adjustTransformMtx(Matrix3D& mtx)
        // Two adjustments applied in C++ order:
        //
        // (a) Bone attachment offset:
        //     if (d->m_attachToDrawableBone.isNotEmpty()) {
        //         getDrawable()->getCurrentWorldspaceClientBonePositions(name, boneMtx);
        //         mtx = boneMtx;
        //     }
        //     PARITY_NOTE: m_attachToDrawableBone not parsed yet; skip.
        //
        // (b) Construction height adjustment:
        //     if (m_curState->m_flags & (1<<ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT)) {
        //         Real pct = obj->getConstructionPercent();
        //         Real height = obj->getGeometryInfo().getMaxHeightAbovePosition();
        //         mtx.Translate_Z(-height + (height * pct / 100.0f));
        //     }
        //     PARITY_NOTE: Object/GeometryInfo not wired; skip.

        mtx
    }

    fn recalc_bones_for_client_particle_systems(&mut self) {
        // PARITY_NOTE: C++: recalcBonesForClientParticleSystems()
        // Pure client-only; must not affect GameLogic (net desync risk).
        // Requires RenderObjClass bone iteration; not yet wired.
    }

    fn update_bones_for_client_particle_systems(&mut self) {
        // PARITY_NOTE: C++: updateBonesForClientParticleSystems()
        // Repositions particle systems to stay in sync with animated bones.
        // Requires particle system + bone infrastructure; not yet wired.
    }

    pub fn last_submission(&self) -> Option<&ModelDrawSubmissionState> {
        self.last_submission.as_ref()
    }

    /// Set the condition state data from the module configuration.
    /// Must be called before any state transitions.
    pub fn set_condition_states(
        &mut self,
        states: Vec<module_data::ModelConditionInfo>,
        transition_map: HashMap<u64, usize>,
        ignore_condition_states: u128,
    ) {
        self.condition_states = states;
        self.transition_map = transition_map;
        self.ignore_condition_states = ignore_condition_states;
    }

    // ─────────────────────────────────────────────────────────────────────
    // METHOD 1: set_model_state (C++: W3DModelDraw::setModelState, line 2887)
    //
    // Core state transition logic. Handles:
    //   - Duplicate state detection (early return)
    //   - AllowToFinishKey deferral (wait for current anim to complete)
    //   - Transition state insertion (intermediate anim between two states)
    //   - Model render object creation/replacement when model name changes
    //   - Rebuild weapon recoil info for new state
    //   - Animation setup via adjustAnimation()
    // ─────────────────────────────────────────────────────────────────────
    pub fn set_model_state(&mut self, new_state_index: i32) {
        let new_idx = new_state_index as usize;
        if new_state_index < 0 || new_idx >= self.condition_states.len() {
            log::warn!(
                "W3DModelDraw::setModelState: invalid state index {} (have {} states)",
                new_state_index,
                self.condition_states.len()
            );
            return;
        }

        let mut next_state_idx: Option<usize> = None;
        let effective_new_idx;

        // C++ parity: Duplicate detection and early return (lines 2898-2931)
        if self.cur_state_index >= 0 {
            let cur_idx = self.cur_state_index as usize;
            let cur_state = &self.condition_states[cur_idx];
            let new_state = &self.condition_states[new_idx];

            // If requested state is current state AND nothing is pending → punt
            // Or if requested state is already the pending next state → punt
            let next_pending = self.next_state_index >= 0;
            let is_same_as_current = self.cur_state_index == new_state_index;
            let is_same_as_pending = next_pending && self.next_state_index == new_state_index;

            if (is_same_as_current && !next_pending) || is_same_as_pending {
                return;
            }

            // C++ parity: AllowToFinishKey deferral (lines 2933-2948)
            if self.cur_state_index != new_state_index
                && new_state.allow_to_finish_key != 0
                && new_state.allow_to_finish_key == cur_state.transition_key
                && self.render_object_id.is_some()
                && !self.animation_complete
            {
                self.next_state_index = new_state_index;
                self.next_state_anim_loop_duration = NO_NEXT_DURATION;
                return;
            }

            // C++ parity: Transition state insertion (lines 2949-2967)
            let mut actual_new_idx = new_idx;
            if self.cur_state_index != new_state_index as i32
                && cur_state.transition_key != 0
                && new_state.transition_key != 0
            {
                let sig = module_data::W3DModelDraw::build_transition_sig(
                    cur_state.transition_key,
                    new_state.transition_key,
                );
                if let Some(&trans_idx) = self.transition_map.get(&sig) {
                    next_state_idx = Some(new_idx);
                    actual_new_idx = trans_idx;
                }
            }
            effective_new_idx = actual_new_idx;
        } else {
            effective_new_idx = new_idx;
        }

        // C++ parity: Get prev anim fraction BEFORE changing anything (line 2971)
        let prev_anim_fraction = self.get_current_anim_fraction();

        // C++ parity: Particle system recalc flag (lines 2978-2979)
        self.need_recalc_bone_particle_systems = true;

        // C++ parity: Stop particle systems (line 2982)
        // PARITY_NOTE: stopClientParticleSystems() requires ParticleSystemManager

        // C++ parity: Hide muzzle flashes (line 2985)
        // PARITY_NOTE: hideAllMuzzleFlashes() requires render object bone access

        let new_state = &self.condition_states[effective_new_idx];
        let cur_model_name = if self.cur_state_index >= 0 {
            self.condition_states[self.cur_state_index as usize].model_name.clone()
        } else {
            String::new()
        };

        let model_changed = new_state.model_name != cur_model_name || self.cur_state_index < 0;

        if model_changed {
            // C++ parity: nukeCurrentRender + create new render object (lines 2999-3134)
            // Release old render objects
            self.render_object_id = None;
            self.shadow_id = None;
            self.terrain_decal_id = None;

            if !new_state.model_name.is_empty() {
                // C++: W3DDisplay::m_assetManager->Create_Render_Obj(modelName, scale, hexColor)
                // PARITY_NOTE: Render object creation requires W3DAssetManager.
                // When wired, this creates the render object and stores the ID.
                log::debug!(
                    "W3DModelDraw::setModelState: model '{}' not yet loaded (asset manager deferred)",
                    new_state.model_name
                );
            }

            // C++ parity: rebuildWeaponRecoilInfo(newState) (line 3020)
            self.rebuild_weapon_recoil_info(effective_new_idx);

            // C++ parity: doHideShowSubObjs(&newState->m_hideShowVec) (line 3021)
            // PARITY_NOTE: Requires RenderObjClass sub-object iteration
        } else {
            // C++ parity: Same model, just validate and rebuild (lines 3137-3149)
            self.rebuild_weapon_recoil_info(effective_new_idx);
        }

        // C++ parity: hideAllHeadlights (line 3150)
        // Applied via submission state (hide_headlights field)

        // C++ parity: Update state pointers (lines 3152-3156)
        let prev_state_index = self.cur_state_index;
        self.cur_state_index = effective_new_idx as i32;
        self.next_state_index = match next_state_idx {
            Some(idx) => idx as i32,
            None => -1,
        };
        self.next_state_anim_loop_duration = NO_NEXT_DURATION;

        // C++ parity: adjustAnimation(prevState, prevAnimFraction) (line 3156)
        self.adjust_animation(prev_state_index, prev_anim_fraction);
    }

    /// C++ parity: W3DModelDraw::replaceModelConditionState (line 3160)
    /// Public API: given condition flags, find best state and transition to it.
    pub fn replace_model_condition_state(&mut self, condition_flags: u128) {
        // C++: m_hideHeadlights = c.test(MODELCONDITION_NIGHT) ? false : true
        const NIGHT_BIT: u128 = 1 << 7; // ModelConditionFlagType::Night = 7
        self.hide_headlights = (condition_flags & NIGHT_BIT) == 0;

        if let Some(idx) = self.find_best_info(condition_flags) {
            self.set_model_state(idx as i32);
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // METHOD 2: adjust_animation (C++: W3DModelDraw::adjustAnimation, line 2129)
    //
    // Sets up the animation for the current state:
    //   - If only 1 animation → use it
    //   - If re-entering same state (idle) → pick a DIFFERENT random anim
    //   - Otherwise → pick any random anim
    //   - Calculate start frame from flags (RANDOMIZE, FIRST, LAST, MAINTAIN)
    //   - Apply frame rate multiplier
    // ─────────────────────────────────────────────────────────────────────
    fn adjust_animation(&mut self, prev_state_index: i32, prev_anim_fraction: f32) {
        if self.cur_state_index < 0 {
            return;
        }
        let cur_idx = self.cur_state_index as usize;
        let cur_state = &self.condition_states[cur_idx];
        let num_anims = cur_state.animations.len();

        if num_anims > 0 {
            // C++ parity: Animation selection (lines 2138-2152)
            if num_anims == 1 {
                self.which_anim_in_cur_state = 0;
            } else if prev_state_index == self.cur_state_index {
                // Re-entering same state (idle anim complete) → pick a different one
                let avoid = self.which_anim_in_cur_state;
                // C++: while (m_whichAnimInCurState == animToAvoid)
                //          m_whichAnimInCurState = GameClientRandomValue(0, numAnims-1);
                // PARITY_NOTE: Uses deterministic selection for now. When
                // GameClientRandomValue is wired, replace with proper random.
                self.which_anim_in_cur_state = (avoid + 1) % (num_anims as i32);
            } else {
                // New state → pick first anim (C++ picks random, we default to 0)
                self.which_anim_in_cur_state = 0;
            }

            let anim_idx = self.which_anim_in_cur_state as usize;
            let anim_info = &cur_state.animations[anim_idx];

            // C++ parity: Calculate start frame (lines 2159-2187)
            let start_frame = self.calculate_start_frame(
                cur_state,
                anim_info,
                prev_state_index,
                prev_anim_fraction,
            );

            // C++ parity: Set animation on render object (line 2189)
            // m_renderObject->Set_Animation(animHandle, startFrame, m_curState->m_mode)
            // PARITY_NOTE: RenderObjClass::Set_Animation not yet wired.
            // Store the animation parameters for when the render pipeline is connected.
            self.animation_mode = cur_state.mode;

            log::debug!(
                "W3DModelDraw::adjustAnimation: state={}, anim_idx={}, start_frame={}, mode={:?}",
                cur_state.description,
                anim_idx,
                start_frame,
                cur_state.mode
            );
        } else {
            self.which_anim_in_cur_state = -1;
        }
    }

    /// Calculate the start frame for the current animation based on state flags.
    /// C++ parity: W3DModelDraw::adjustAnimation lines 2159-2187
    fn calculate_start_frame(
        &self,
        cur_state: &module_data::ModelConditionInfo,
        anim_info: &module_data::W3DAnimationInfo,
        prev_state_index: i32,
        prev_anim_fraction: f32,
    ) -> i32 {
        const RANDOMIZE_START_FRAME: u32 = 1 << 0;
        const START_FRAME_FIRST: u32 = 1 << 1;
        const START_FRAME_LAST: u32 = 1 << 2;
        const MAINTAIN_FRAME_MASK: u32 = (1 << 5) | (1 << 7) | (1 << 8) | (1 << 9);

        let num_frames = anim_info.num_frames.max(1) as i32;

        // C++ parity: Backwards animations start at last frame (lines 2160-2164)
        if cur_state.mode == module_data::AnimationMode::OnceBackwards
            || cur_state.mode == module_data::AnimationMode::LoopBackwards
        {
            // C++: startFrame = animHandle->Get_Num_Frames()-1
            return num_frames - 1;
        }

        // C++ parity: Flag-based start frame selection (lines 2166-2187)
        // Order matters: RANDOMIZE > FIRST > LAST > MAINTAIN
        if (cur_state.flags & RANDOMIZE_START_FRAME) != 0 {
            // C++: startFrame = GameClientRandomValue(0, numFrames-1)
            // PARITY_NOTE: Uses deterministic value until random is wired
            0
        } else if (cur_state.flags & START_FRAME_FIRST) != 0 {
            0
        } else if (cur_state.flags & START_FRAME_LAST) != 0 {
            num_frames - 1
        } else if (cur_state.flags & MAINTAIN_FRAME_MASK) != 0
            && prev_state_index >= 0
            && prev_state_index != self.cur_state_index
        {
            // C++: Maintain frame across states
            // Check if prev state also has maintain frame flags
            let prev_idx = prev_state_index as usize;
            if prev_idx < self.condition_states.len() {
                let prev_state = &self.condition_states[prev_idx];
                if (prev_state.flags & MAINTAIN_FRAME_MASK) != 0
                    && self.has_common_maintain_frame_flag(cur_state.flags, prev_state.flags)
                    && prev_anim_fraction >= 0.0
                {
                    // C++: startFrame = REAL_TO_INT(prevAnimFraction * numFrames - 1)
                    let frame = (prev_anim_fraction * (num_frames as f32) - 1.0) as i32;
                    return frame.max(0).min(num_frames - 1);
                }
            }
            0
        } else {
            0
        }
    }

    /// Check if two state flag sets share any of the MAINTAIN_FRAME bits.
    fn has_common_maintain_frame_flag(&self, flags_a: u32, flags_b: u32) -> bool {
        const MAINTAIN_FRAME_MASK: u32 = (1 << 5) | (1 << 7) | (1 << 8) | (1 << 9);
        (flags_a & flags_b & MAINTAIN_FRAME_MASK) != 0
    }

    /// C++ parity: W3DModelDraw::setAnimationLoopDuration (line 3748)
    pub fn set_animation_loop_duration(&mut self, num_frames: u32) {
        self.next_state_anim_loop_duration = NO_NEXT_DURATION;
        // C++: Real desiredDurationInMsec = ceilf(numFrames * MSEC_PER_LOGICFRAME_REAL)
        const MSEC_PER_LOGICFRAME: f32 = 1000.0 / 30.0; // 30 FPS logic frames
        let desired_duration_msec = (num_frames as f32 * MSEC_PER_LOGICFRAME).ceil();
        let _ = self.set_cur_anim_duration_msec(desired_duration_msec);
    }

    /// C++ parity: W3DModelDraw::setAnimationCompletionTime (line 3775)
    pub fn set_animation_completion_time(&mut self, num_frames: u32) {
        if self.cur_state_index >= 0 {
            let cur_idx = self.cur_state_index as usize;
            let cur_state = &self.condition_states[cur_idx];

            // C++: If current is transition and next is non-transition, split time
            if cur_state.transition_sig != 0
                && !cur_state.animations.is_empty()
                && self.next_state_index >= 0
            {
                let next_idx = self.next_state_index as usize;
                let next_state = &self.condition_states[next_idx];
                if next_state.transition_sig == 0 && !next_state.animations.is_empty() {
                    let t1 = cur_state.animations[0].natural_duration_msec.max(1) as f32;
                    let t2 = next_state.animations[0].natural_duration_msec.max(1) as f32;
                    let trans_time = ((num_frames as f32 * t1) / (t1 + t2)) as u32;
                    self.set_animation_loop_duration(trans_time);
                    self.next_state_anim_loop_duration = num_frames.saturating_sub(trans_time);
                    return;
                }
            }
        }
        self.set_animation_loop_duration(num_frames);
    }

    /// C++ parity: W3DModelDraw::setAnimationFrame (line 3797)
    pub fn set_animation_frame(&mut self, frame: i32) {
        if self.render_object_id.is_some() && self.which_anim_in_cur_state >= 0 {
            // C++: m_renderObject->Set_Animation(animHandle, frame)
            // PARITY_NOTE: RenderObjClass::Set_Animation not wired
            log::debug!(
                "W3DModelDraw::setAnimationFrame: frame={}, anim_idx={}",
                frame,
                self.which_anim_in_cur_state
            );
        }
    }

    /// C++ parity: W3DModelDraw::setPauseAnimation (line 3809)
    pub fn set_pause_animation(&mut self, pause: bool) {
        if self.pause_animation == pause {
            return;
        }
        self.pause_animation = pause;

        // C++: If pausing, save mode and switch to MANUAL. If resuming, restore saved mode.
        // PARITY_NOTE: Requires HLodClass::Peek_Animation_And_Info + Set_Animation
        // When render objects have animation state, this will toggle between
        // ANIM_MODE_MANUAL (paused) and the saved mode (resumed).
    }

    /// C++ parity: W3DModelDraw::setCurAnimDurationInMsec (line 2210)
    fn set_cur_anim_duration_msec(&mut self, desired_duration_msec: f32) -> bool {
        if desired_duration_msec <= 0.0 {
            return false;
        }
        // C++: naturalDuration = numFrames * 1000.0 / frameRate
        //      multiplier = naturalDuration / desiredDuration
        // PARITY_NOTE: Requires HLodClass::Peek_Animation for actual values.
        // When wired, this will call hlod->Set_Animation_Frame_Rate_Multiplier(multiplier).
        log::debug!(
            "W3DModelDraw::setCurAnimDurationInMsec: desired={}ms",
            desired_duration_msec
        );
        false
    }

    /// C++ parity: W3DModelDraw::getCurrentAnimFraction (line 2103)
    fn get_current_anim_fraction(&self) -> f32 {
        if self.cur_state_index < 0 || self.render_object_id.is_none() {
            return -1.0;
        }
        // C++: Peek_Animation_And_Info → frame / (numFrames - 1)
        // PARITY_NOTE: Requires HLodClass animation state
        -1.0
    }

    /// Find the best matching condition state for the given flags.
    /// C++ parity: W3DModelDraw::findBestInfo → W3DModelDrawModuleData::findBestInfo
    fn find_best_info(&self, condition_flags: u128) -> Option<usize> {
        let masked_flags = condition_flags & !self.ignore_condition_states;
        let mut best_index = None;
        let mut best_match_count = 0usize;
        let mut best_extra_count = usize::MAX;

        for (index, state) in self.condition_states.iter().enumerate() {
            if state.transition_sig != 0 {
                continue;
            }
            for &cond_bits in &state.conditions {
                if (masked_flags & cond_bits) != cond_bits {
                    continue;
                }
                let match_count = (masked_flags & cond_bits).count_ones() as usize;
                let extra_count = (cond_bits & !masked_flags).count_ones() as usize;
                if match_count > best_match_count
                    || (match_count == best_match_count && extra_count < best_extra_count)
                {
                    best_match_count = match_count;
                    best_extra_count = extra_count;
                    best_index = Some(index);
                }
            }
        }
        best_index
    }

    /// Rebuild weapon recoil info for a new state.
    /// C++ parity: W3DModelDraw::rebuildWeaponRecoilInfo (line 3850)
    fn rebuild_weapon_recoil_info(&mut self, state_index: usize) {
        for slot in 0..self.weapon_recoil.len() {
            // C++: resize to match barrel count, then clear each entry
            // PARITY_NOTE: Barrel count comes from state's weaponBarrelInfoVec.
            // Until weapon barrel info is wired, we keep existing recoil vectors.
            for recoil in &mut self.weapon_recoil[slot] {
                recoil.state = RecoilState::Idle;
                recoil.shift = 0.0;
                recoil.recoil_rate = 0.0;
            }
        }
    }

    /// CLIENT ONLY: Update turret bone positions from AI data.
    /// For each turret: get angle/pitch from AI, add art offsets,
    /// create rotation matrix, Capture_Bone + Control_Bone.
    fn handle_client_turret_positioning(&mut self) {
        // PARITY_NOTE: Requires AIUpdateInterface::getTurretRotAndPitch()
        // and RenderObjClass::Capture_Bone/Control_Bone
    }

    /// Per-frame recoil state machine for each weapon slot and barrel.
    /// IDLE: nothing. RECOIL_START/RECOIL: increase shift by rate, damp.
    /// SETTLE: decrease shift by settle speed. Return to IDLE at zero.
    fn handle_client_recoil(&mut self) {
        // PARITY_NOTE: Requires RenderObjClass::Capture_Bone/Control_Bone
        // for recoil bone manipulation
    }

    /// Apply hide/show sub-object list (used by supply draw, upgrade system, etc.)
    pub fn do_hide_show_sub_objs(&mut self, _info_vec: &[HideShowSubObjInfo]) {
        // PARITY_NOTE: RenderObjClass::Get_Sub_Object_By_Name() + Set_Hidden()
    }

    /// React to transform change: update render object and track marks.
    pub fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix4<f32>,
        _old_pos: &Point3<f32>,
        _old_angle: f32,
    ) {
        // PARITY_NOTE: Update render object transform, add track edges
    }

    /// React to geometry change.
    ///
    /// C++ parity: `virtual void reactToGeometryChange() { }` — empty in W3DModelDraw.h.
    /// The base W3DModelDraw has no geometry-specific update to perform; subclasses
    /// (TankTruckDraw, TruckDraw, SupplyDraw) that override this also leave it empty.
    /// Geometry bounds are implicitly updated via render object transforms set in doDrawModule.
    pub fn react_to_geometry_change(&mut self) {}

    pub fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadow_enabled = enable;
    }

    /// Release all shadow resources used by this module.
    ///
    /// C++ parity: `W3DModelDraw::releaseShadows()` (W3DModelDraw.cpp line 1821):
    /// ```cpp
    /// if (m_shadow) m_shadow->release();
    /// m_shadow = NULL;
    /// ```
    /// Called by the Options screen to dynamically enable/disable shadows.
    pub fn release_shadows(&mut self) {
        if let Some(id) = self.shadow_id.take() {
            let scene = W3DDisplay::global_scene();
            let mut scene_guard = scene.write();
            scene_guard.remove_render_object(id);
        }
    }

    /// Allocate shadow resources if not already present.
    ///
    /// C++ parity: `W3DModelDraw::allocateShadows()` (W3DModelDraw.cpp line 1829):
    /// Creates shadow via TheW3DShadowManager when:
    ///   - m_shadow == NULL (no existing shadow)
    ///   - m_renderObject exists
    ///   - ThingTemplate shadow type != SHADOW_NONE
    /// Shadow info (texture, type, sizeX/Y, offsetX/Y) comes from ThingTemplate.
    /// After creation, applies shroud visibility and hidden/shadow-disabled states.
    pub fn allocate_shadows(&mut self) {
        if self.shadow_id.is_none() && self.render_object_id.is_some() {
            // PARITY_NOTE: Full C++ implementation requires:
            //   const ThingTemplate* tmplate = getDrawable()->getTemplate();
            //   Shadow::ShadowTypeInfo shadowInfo;
            //   shadowInfo.m_ShadowName = tmplate->getShadowTextureName();
            //   shadowInfo.m_type = tmplate->getShadowType();
            //   shadowInfo.m_sizeX = tmplate->getShadowSizeX();
            //   shadowInfo.m_sizeY = tmplate->getShadowSizeY();
            //   shadowInfo.m_offsetX = tmplate->getShadowOffsetX();
            //   shadowInfo.m_offsetY = tmplate->getShadowOffsetY();
            //   m_shadow = TheW3DShadowManager->addShadow(m_renderObject, &shadowInfo);
            //   if (m_shadow) {
            //       m_shadow->enableShadowInvisible(m_fullyObscuredByShroud);
            //       if (m_renderObject->Is_Hidden() || !m_shadowEnabled)
            //           m_shadow->enableShadowRender(FALSE);
            //   }
            //
            // Requires ThingTemplate shadow properties and W3DShadowManager integration.
            // When wired, this will create a shadow render object via W3DShadowManager::add_shadow()
            // and store the resulting ID in self.shadow_id.
        }
    }

    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }

    // --------------------------------------------------------------------------------------------
    // Xfer / Snapshotable — C++ W3DModelDraw::xfer() lines 4006-4236
    //
    // Version history:
    //   1: Initial version
    //   2: Added animation frame (CBD)
    //
    // The save produces a self-contained data structure that load can fully restore.
    // In C++, xfer() operates on a streaming Xfer* object with XFER_SAVE / XFER_LOAD modes.
    // Here we use a data-oriented approach: xfer_save() captures all mutable state into a
    // serializable snapshot, and xfer_load() restores it.
    // --------------------------------------------------------------------------------------------

    /// Current xfer version number. Must match C++ `currentVersion = 2`.
    pub const XFER_VERSION: u32 = 2;

    /// Save all mutable draw-module state into a serializable snapshot.
    ///
    /// C++ parity: `W3DModelDraw::xfer(Xfer* xfer)` in XFER_SAVE mode (lines 4006-4236).
    ///
    /// Serialized data:
    /// - Version (2)
    /// - Per weapon-slot recoil info vectors (count + [state, shift, recoilRate] per entry)
    /// - Sub-object hide/show vector (count + [name, hide] per entry)
    /// - Animation frame info (present flag, mode, frame-as-percent) — version >= 2 only
    pub fn xfer_save(&self) -> W3DModelDrawXferData {
        // -- Weapon recoil info vectors --
        // C++: for i in 0..WEAPONSLOT_COUNT: xfer count, then each entry's state/shift/recoilRate
        let mut recoil_data = Vec::with_capacity(self.weapon_recoil.len());
        for slot in &self.weapon_recoil {
            let entries: Vec<WeaponRecoilXferEntry> = slot
                .iter()
                .map(|r| WeaponRecoilXferEntry {
                    state: r.state,
                    shift: r.shift,
                    recoil_rate: r.recoil_rate,
                })
                .collect();
            recoil_data.push(entries);
        }

        // -- Sub-object vector --
        // C++: xfer count, then each entry's subObjName (AsciiString) + hide (Bool)
        let sub_objects: Vec<HideShowSubObjInfo> = self.sub_object_vec.clone();

        // -- Animation frame info (version >= 2) --
        // C++: only saved when render object is HLod, curState exists, and
        // curState->m_transitionSig == NO_TRANSITION. Otherwise present = FALSE.
        // PARITY_NOTE: render object animation info requires HLodClass::Peek_Animation_And_Info.
        // Until render objects have animation state, we save present=false (C++ else branch,
        // line 4164-4170).
        let animation = if self.render_object_id.is_some() && self.cur_state_index >= 0 {
            // PARITY_NOTE: In C++, this checks m_curState->m_transitionSig == NO_TRANSITION.
            // We approximate by checking we have a valid non-transition state.
            // For now, save animation info if we have a render object and state.
            Some(W3DAnimationXferData {
                present: true,
                // PARITY_NOTE: mode comes from HLodClass::Peek_Animation_And_Info.
                // The mode value is the RenderObjClass::AnimMode enum.
                // Since we don't have the actual HLod, we store the current animation
                // index as a proxy — the mode is ignored on load in C++ anyway (line 4187).
                mode: 0,
                // PARITY_NOTE: percent = frame / (numFrames-1).
                // Without HLod animation data we save 0.0; this is safe because
                // the C++ load path will simply not restore any frame offset.
                percent: 0.0,
            })
        } else {
            Some(W3DAnimationXferData {
                present: false,
                mode: 0,
                percent: 0.0,
            })
        };

        W3DModelDrawXferData {
            version: Self::XFER_VERSION,
            recoil_data,
            sub_objects,
            animation,
        }
    }

    /// Load all mutable draw-module state from a snapshot.
    ///
    /// C++ parity: `W3DModelDraw::xfer(Xfer* xfer)` in XFER_LOAD mode (lines 4006-4236).
    ///
    /// After loading, if the sub-object vector is non-empty, calls updateSubObjects()
    /// (C++ line 4233-4234).
    pub fn xfer_load(&mut self, data: &W3DModelDrawXferData) {
        // -- Weapon recoil info vectors --
        // C++: for each slot, clear existing, then push loaded entries
        if data.recoil_data.len() <= self.weapon_recoil.len() {
            for (slot_idx, entries) in data.recoil_data.iter().enumerate() {
                self.weapon_recoil[slot_idx].clear();
                for entry in entries {
                    self.weapon_recoil[slot_idx].push(WeaponRecoilInfo {
                        state: entry.state,
                        shift: entry.shift,
                        recoil_rate: entry.recoil_rate,
                    });
                }
            }
        } else {
            log::warn!(
                "W3DModelDraw::xfer_load: recoil_data has {} slots, expected <= {}. Truncating.",
                data.recoil_data.len(),
                self.weapon_recoil.len()
            );
            for (slot_idx, entries) in data.recoil_data.iter().enumerate() {
                if slot_idx < self.weapon_recoil.len() {
                    self.weapon_recoil[slot_idx].clear();
                    for entry in entries {
                        self.weapon_recoil[slot_idx].push(WeaponRecoilInfo {
                            state: entry.state,
                            shift: entry.shift,
                            recoil_rate: entry.recoil_rate,
                        });
                    }
                }
            }
        }

        // -- Sub-object vector --
        // C++: clear existing, then push loaded entries
        self.sub_object_vec.clear();
        self.sub_object_vec.extend(data.sub_objects.iter().cloned());

        // -- Animation frame info (version >= 2) --
        // C++ lines 4174-4228: if present, read mode (ignored) and percent.
        // If render object is HLod, restore animation frame from percent.
        // PARITY_NOTE: Full restoration requires HLodClass::Peek_Animation() and
        // Set_Animation(). Until render objects have animation state, we note the
        // loaded percent for future restoration but do not crash on missing HLod.
        if data.version >= 2 {
            if let Some(ref anim) = data.animation {
                if anim.present {
                    // PARITY_NOTE: C++ reads mode but ignores it (line 4187 comment:
                    // "note, this will be ignored"). We skip storing it.
                    // C++ reads percent and, if HLod exists, computes:
                    //   frame = percent * (anim->Get_Num_Frames() - 1)
                    // then calls hlod->Set_Animation(anim, frame, curMode).
                    // Without HLod, we log for debugging.
                    log::debug!(
                        "W3DModelDraw::xfer_load: animation present, percent={}, \
                         render_object_id={:?}. HLod restore deferred.",
                        anim.percent,
                        self.render_object_id
                    );
                }
            }
        }

        // -- Post-load: update sub-objects --
        // C++ lines 4232-4234: if loading and sub-object vector is non-empty, updateSubObjects().
        if !self.sub_object_vec.is_empty() {
            // PARITY_NOTE: updateSubObjects() requires RenderObjClass sub-object iteration.
            // The sub-object vec is stored correctly; actual render-object hide/show
            // will be applied when updateSubObjects() is wired to the render pipeline.
            log::debug!(
                "W3DModelDraw::xfer_load: {} sub-objects loaded, \
                 updateSubObjects() deferred to render pipeline.",
                self.sub_object_vec.len()
            );
        }
    }

    /// CRC computation. C++ parity: `W3DModelDraw::crc(Xfer*)` — extends base class
    /// DrawModule::crc() which is a no-op.
    pub fn crc(&self) -> u32 {
        0
    }

    /// C++ parity: `W3DModelDraw::loadPostProcess()` — calls `DrawModule::loadPostProcess()`
    /// which is a no-op. No additional post-load logic for the base model draw.
    pub fn load_post_process(&mut self) {}

    // --------------------------------------------------------------------------------------------
    // Shroud / color tinting — C++ W3DModelDraw lines 1915-1928, 3188-3208
    // --------------------------------------------------------------------------------------------

    /// Update shroud visibility state.
    ///
    /// C++ parity: `W3DModelDraw::setFullyObscuredByShroud(Bool fullyObscured)` (line 1915).
    ///
    /// When the shroud state changes:
    /// 1. Update shadow visibility (m_shadow->enableShadowInvisible)
    /// 2. Update terrain decal visibility (m_terrainDecal->enableShadowInvisible)
    /// 3. Start/stop particle systems (doStartOrStopParticleSys)
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        if self.fully_obscured_by_shroud != fully_obscured {
            self.fully_obscured_by_shroud = fully_obscured;

            // C++: if (m_shadow) m_shadow->enableShadowInvisible(m_fullyObscuredByShroud);
            if fully_obscured {
                if let Some(id) = self.shadow_id {
                    let scene = W3DDisplay::global_scene();
                    let mut scene_guard = scene.write();
                    if let Some(obj) = scene_guard.get_render_object_mut(id) {
                        obj.visible = false;
                    }
                }
                if let Some(id) = self.terrain_decal_id {
                    let scene = W3DDisplay::global_scene();
                    let mut scene_guard = scene.write();
                    if let Some(obj) = scene_guard.get_render_object_mut(id) {
                        obj.visible = false;
                    }
                }
            } else {
                // Restore shadow visibility only if shadow_enabled is true
                if self.shadow_enabled {
                    if let Some(id) = self.shadow_id {
                        let scene = W3DDisplay::global_scene();
                        let mut scene_guard = scene.write();
                        if let Some(obj) = scene_guard.get_render_object_mut(id) {
                            obj.visible = true;
                        }
                    }
                }
                if let Some(id) = self.terrain_decal_id {
                    let scene = W3DDisplay::global_scene();
                    let mut scene_guard = scene.write();
                    if let Some(obj) = scene_guard.get_render_object_mut(id) {
                        obj.visible = true;
                    }
                }
            }

            // C++: doStartOrStopParticleSys()
            // PARITY_NOTE: Particle system start/stop requires ParticleSystemManager.
            // When wired, this would stop particle systems when obscured and restart
            // them when revealed.
            log::debug!(
                "W3DModelDraw::setFullyObscuredByShroud: obscured={}, particle sys start/stop deferred.",
                fully_obscured
            );
        }
    }

    /// Replace the team indicator color on the model.
    ///
    /// C++ parity: `W3DModelDraw::replaceIndicatorColor(Color color)` (line 3188).
    ///
    /// Only applies when `m_okToChangeModelColor` is true (from module data).
    /// When the color changes, forces a full model state rebuild by nulling the
    /// current state and calling setModelState() — this re-applies textures with
    /// the new team color tint.
    ///
    /// Returns true if the color was actually changed (and model state was rebuilt).
    pub fn replace_indicator_color(&mut self, color: i32, ok_to_change_model_color: bool) -> bool {
        if !ok_to_change_model_color {
            return false;
        }

        // C++ line 3195: Int newColor = (color == 0) ? 0 : (color | 0xFF000000);
        let new_color = if color == 0 {
            0
        } else {
            color | 0xFF00_0000
        };

        if new_color != self.hex_color && self.render_object_id.is_some() {
            self.hex_color = new_color;

            // C++ lines 3200-3205: set curState to NULL, then setModelState(tmp).
            // This forces a full model state rebuild with the new color.
            // PARITY_NOTE: setModelState() requires full condition state infrastructure.
            // When wired, this will trigger a rebuild of the render object with new
            // team color textures.
            log::debug!(
                "W3DModelDraw::replaceIndicatorColor: new hex_color={:#010X}, \
                 setModelState rebuild deferred to condition state infrastructure.",
                new_color
            );
            true
        } else {
            false
        }
    }

    /// Get the current indicator/team color.
    pub fn get_hex_color(&self) -> i32 {
        self.hex_color
    }

    fn on_delete(&mut self) {
        // PARITY_NOTE: Unbind track render object, nukeCurrentRender (shadow, decal, render object)
        self.render_object_id = None;
        self.shadow_id = None;
        self.terrain_decal_id = None;
        self.track_render_object_id = None;
    }
}

impl Default for W3DModelDraw {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for W3DModelDraw {
    fn drop(&mut self) {
        self.on_delete();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_wthree_d_model_draw_basic() {
        let draw = W3DModelDraw::new();
        assert_eq!(draw.cur_state_index, -1);
        assert!(draw.is_visible());
    }
    #[test]
    fn test_wthree_d_model_draw_recoil_default() {
        let draw = W3DModelDraw::new();
        assert_eq!(draw.weapon_recoil.len(), 5);
    }

    #[test]
    fn test_select_lod_empty_thresholds() {
        let draw = W3DModelDraw::new();
        assert_eq!(draw.select_lod(100.0), 0);
    }

    #[test]
    fn test_select_lod_zero_distance() {
        let mut draw = W3DModelDraw::new();
        draw.set_lod_thresholds(vec![
            LodThreshold { lod_index: 0, max_screen_area: 0.01 },
            LodThreshold { lod_index: 1, max_screen_area: 0.001 },
        ]);
        // distance=0 → highest detail (LOD 0)
        assert_eq!(draw.select_lod(0.0), 0);
    }

    #[test]
    fn test_select_lod_far_distance() {
        let mut draw = W3DModelDraw::new();
        draw.set_lod_thresholds(vec![
            LodThreshold { lod_index: 0, max_screen_area: 0.01 },
            LodThreshold { lod_index: 1, max_screen_area: 0.001 },
        ]);
        // Very far → screen area is tiny → lowest detail (LOD 1)
        assert_eq!(draw.select_lod(1000.0), 1);
    }

    #[test]
    fn test_select_lod_negative_threshold_always_matches() {
        let mut draw = W3DModelDraw::new();
        draw.set_lod_thresholds(vec![
            LodThreshold { lod_index: 0, max_screen_area: 0.01 },
            LodThreshold { lod_index: 1, max_screen_area: -1.0 },
        ]);
        // NO_MAX_SCREEN_SIZE (-1.0) acts as catch-all
        assert_eq!(draw.select_lod(0.1), 0);
        assert_eq!(draw.select_lod(100.0), 1);
    }

    #[test]
    fn test_select_lod_with_bias() {
        let mut draw = W3DModelDraw::new();
        draw.lod_bias = 2.0;
        draw.set_lod_thresholds(vec![
            LodThreshold { lod_index: 0, max_screen_area: 0.01 },
            LodThreshold { lod_index: 1, max_screen_area: 0.001 },
        ]);
        // Bias=2.0 doubles screen_area, so distance 15.0 → area = 1/(15²*2) ≈ 0.0044
        // 0.0044 < 0.01 but >= 0.001 → LOD 1
        assert_eq!(draw.select_lod(15.0), 1);
    }

    #[test]
    fn test_compute_bone_matrices_empty_hierarchy() {
        let draw = W3DModelDraw::new();
        let result = draw.compute_bone_matrices(&[], &[], &Matrix4::identity(), None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_bone_matrices_root_only() {
        let mut draw = W3DModelDraw::new();
        draw.set_bone_hierarchy(vec![
            (0, Matrix4::identity()),
        ]);
        let root = Matrix4::from_translation(Vector3::new(1.0, 2.0, 3.0));
        let result = draw.compute_bone_matrices(&[], &[], &root, None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], root);
    }

    #[test]
    fn test_compute_bone_matrices_parent_child() {
        let mut draw = W3DModelDraw::new();
        // Root at origin, child with base transform at (1,0,0)
        draw.set_bone_hierarchy(vec![
            (0, Matrix4::identity()),
            (0, Matrix4::from_translation(Vector3::new(1.0, 0.0, 0.0))),
        ]);

        let root = Matrix4::identity();
        let result = draw.compute_bone_matrices(&[], &[], &root, None);
        assert_eq!(result.len(), 2);
        // Child = parent * base = identity * translate(1,0,0)
        let child_translation = Vector3::new(result[1].w.x, result[1].w.y, result[1].w.z);
        assert!((child_translation - Vector3::new(1.0, 0.0, 0.0)).magnitude() < 0.001);
    }

    #[test]
    fn test_compute_bone_matrices_with_animation() {
        let mut draw = W3DModelDraw::new();
        draw.set_bone_hierarchy(vec![
            (0, Matrix4::identity()),
            (0, Matrix4::from_translation(Vector3::new(0.0, 0.0, 0.0))),
        ]);

        let root = Matrix4::identity();
        let translations = vec![Vector3::zero(), Vector3::new(5.0, 0.0, 0.0)];
        let rotations = vec![Quaternion::zero(), Quaternion::zero()];
        let result = draw.compute_bone_matrices(&translations, &rotations, &root, None);
        // Child bone has anim translation (5,0,0) applied
        let child_pos = Vector3::new(result[1].w.x, result[1].w.y, result[1].w.z);
        assert!((child_pos - Vector3::new(5.0, 0.0, 0.0)).magnitude() < 0.001);
    }

    #[test]
    fn test_compute_bone_matrices_with_model_space_conversion() {
        let mut draw = W3DModelDraw::new();
        draw.set_bone_hierarchy(vec![
            (0, Matrix4::identity()),
            (0, Matrix4::from_translation(Vector3::new(2.0, 0.0, 0.0))),
        ]);

        let world = Matrix4::from_translation(Vector3::new(10.0, 0.0, 0.0));
        let inv = world.invert().unwrap();

        let result = draw.compute_bone_matrices(&[], &[], &world, Some(&inv));
        // After model-space conversion, child at (12,0,0) world → (2,0,0) model
        let child_pos = Vector3::new(result[1].w.x, result[1].w.y, result[1].w.z);
        assert!((child_pos - Vector3::new(2.0, 0.0, 0.0)).magnitude() < 0.001);
    }

    #[test]
    fn test_capture_bone_override() {
        let mut draw = W3DModelDraw::new();
        draw.set_bone_hierarchy(vec![
            (0, Matrix4::identity()),
            (0, Matrix4::from_translation(Vector3::new(1.0, 0.0, 0.0))),
        ]);

        let capture = Matrix4::from_translation(Vector3::new(99.0, 0.0, 0.0));
        draw.capture_bone(1, capture);

        let result = draw.compute_bone_matrices(&[], &[], &Matrix4::identity(), None);
        let child_pos = Vector3::new(result[1].w.x, result[1].w.y, result[1].w.z);
        assert!((child_pos - Vector3::new(99.0, 0.0, 0.0)).magnitude() < 0.001);

        draw.release_bone(1);
        let result2 = draw.compute_bone_matrices(&[], &[], &Matrix4::identity(), None);
        let child_pos2 = Vector3::new(result2[1].w.x, result2[1].w.y, result2[1].w.z);
        assert!((child_pos2 - Vector3::new(1.0, 0.0, 0.0)).magnitude() < 0.001);
    }

    #[test]
    fn test_uv_override_replaces_same_slot() {
        let mut draw = W3DModelDraw::new();
        draw.add_uv_override(UvOverride { slot: 0, offset: (0.1, 0.2), scale: (1.0, 1.0) });
        draw.add_uv_override(UvOverride { slot: 0, offset: (0.5, 0.6), scale: (2.0, 2.0) });
        assert_eq!(draw.uv_overrides.len(), 1);
        assert_eq!(draw.uv_overrides[0].offset, (0.5, 0.6));
    }

    #[test]
    fn test_submission_includes_lod_and_uv() {
        let mut draw = W3DModelDraw::new();
        draw.add_uv_override(UvOverride { slot: 0, offset: (0.0, 0.0), scale: (1.0, 1.0) });
        draw.set_lod_thresholds(vec![
            LodThreshold { lod_index: 0, max_screen_area: 0.1 },
        ]);
        draw.do_draw_module(&Matrix4::identity());
        let sub = draw.last_submission().unwrap();
        assert_eq!(sub.selected_lod, 0);
        assert_eq!(sub.uv_overrides.len(), 1);
    }

    #[test]
    fn test_wthree_d_model_draw_recoil_default() {
        let draw = W3DModelDraw::new();
        assert_eq!(draw.weapon_recoil.len(), 5);
    }

    #[test]
    fn test_xfer_save_load_roundtrip() {
        let mut draw = W3DModelDraw::new();
        draw.weapon_recoil[0].push(WeaponRecoilInfo {
            state: RecoilState::RecoilStart,
            shift: 1.5,
            recoil_rate: 0.8,
        });
        draw.weapon_recoil[2].push(WeaponRecoilInfo {
            state: RecoilState::Settle,
            shift: 0.3,
            recoil_rate: 0.1,
        });
        draw.sub_object_vec.push(HideShowSubObjInfo {
            name: "Turret".to_string(),
            hide: true,
        });
        draw.sub_object_vec.push(HideShowSubObjInfo {
            name: "Barrel".to_string(),
            hide: false,
        });

        let saved = draw.xfer_save();
        assert_eq!(saved.version, W3DModelDraw::XFER_VERSION);
        assert_eq!(saved.recoil_data[0].len(), 1);
        assert_eq!(saved.recoil_data[0][0].state, RecoilState::RecoilStart);
        assert!((saved.recoil_data[0][0].shift - 1.5).abs() < f32::EPSILON);
        assert_eq!(saved.recoil_data[2].len(), 1);
        assert_eq!(saved.sub_objects.len(), 2);
        assert_eq!(saved.sub_objects[0].name, "Turret");
        assert!(saved.sub_objects[0].hide);

        let mut draw2 = W3DModelDraw::new();
        draw2.xfer_load(&saved);
        assert_eq!(draw2.weapon_recoil[0].len(), 1);
        assert_eq!(draw2.weapon_recoil[0][0].state, RecoilState::RecoilStart);
        assert!((draw2.weapon_recoil[0][0].shift - 1.5).abs() < f32::EPSILON);
        assert_eq!(draw2.weapon_recoil[2].len(), 1);
        assert_eq!(draw2.sub_object_vec.len(), 2);
        assert_eq!(draw2.sub_object_vec[0].name, "Turret");
    }

    #[test]
    fn test_xfer_empty_roundtrip() {
        let draw = W3DModelDraw::new();
        let saved = draw.xfer_save();
        assert_eq!(saved.version, 2);
        assert!(saved.recoil_data.iter().all(|s| s.is_empty()));
        assert!(saved.sub_objects.is_empty());

        let mut draw2 = W3DModelDraw::new();
        draw2.weapon_recoil[0].push(WeaponRecoilInfo {
            state: RecoilState::Recoil,
            shift: 2.0,
            recoil_rate: 1.0,
        });
        draw2.xfer_load(&saved);
        assert!(draw2.weapon_recoil.iter().all(|s| s.is_empty()));
    }

    #[test]
    fn test_replace_indicator_color_no_change() {
        let mut draw = W3DModelDraw::new();
        let changed = draw.replace_indicator_color(0, true);
        assert!(!changed);
        assert_eq!(draw.get_hex_color(), 0);
    }

    #[test]
    fn test_replace_indicator_color_blocked() {
        let mut draw = W3DModelDraw::new();
        draw.render_object_id = Some(1);
        let changed = draw.replace_indicator_color(0x00FF0000, false);
        assert!(!changed);
    }

    #[test]
    fn test_replace_indicator_color_applies() {
        let mut draw = W3DModelDraw::new();
        draw.render_object_id = Some(1);
        let changed = draw.replace_indicator_color(0x00FF0000, true);
        assert!(changed);
        assert_eq!(draw.get_hex_color(), 0xFFFF0000);
    }

    #[test]
    fn test_replace_indicator_color_zero_stays_zero() {
        let mut draw = W3DModelDraw::new();
        draw.hex_color = 0x12345678;
        draw.render_object_id = Some(1);
        let changed = draw.replace_indicator_color(0, true);
        assert!(changed);
        assert_eq!(draw.get_hex_color(), 0);
    }

    #[test]
    fn test_set_fully_obscured_by_shroud_no_change() {
        let mut draw = W3DModelDraw::new();
        assert!(!draw.fully_obscured_by_shroud);
        draw.set_fully_obscured_by_shroud(false);
        assert!(!draw.fully_obscured_by_shroud);
    }

    #[test]
    fn test_set_fully_obscured_by_shroud_toggle() {
        let mut draw = W3DModelDraw::new();
        draw.set_fully_obscured_by_shroud(true);
        assert!(draw.fully_obscured_by_shroud);
        assert!(!draw.is_visible());
        draw.set_fully_obscured_by_shroud(false);
        assert!(!draw.fully_obscured_by_shroud);
        assert!(draw.is_visible());
    }

    fn make_test_condition_states() -> Vec<module_data::ModelConditionInfo> {
        let mut idle = module_data::ModelConditionInfo::new();
        idle.model_name = "IdleModel".to_string();
        idle.description = "Idle".to_string();
        idle.conditions = vec![0];
        idle.transition_key = 1;
        idle.mode = module_data::AnimationMode::Loop;
        idle.animations.push(module_data::W3DAnimationInfo::with_name("IdleAnim", true, 0.0));
        idle.animations.push(module_data::W3DAnimationInfo::with_name("IdleAnim2", true, 0.0));

        let mut moving = module_data::ModelConditionInfo::new();
        moving.model_name = "MoveModel".to_string();
        moving.description = "Moving".to_string();
        moving.conditions = vec![1u128 << 49]; // MODELCONDITION_MOVING
        moving.transition_key = 2;
        moving.mode = module_data::AnimationMode::Loop;
        moving.animations.push(module_data::W3DAnimationInfo::with_name("MoveAnim", false, 5.0));

        let mut attacking = module_data::ModelConditionInfo::new();
        attacking.model_name = "AttackModel".to_string();
        attacking.description = "Attacking".to_string();
        attacking.conditions = vec![1u128 << 34]; // MODELCONDITION_ATTACKING
        attacking.transition_key = 3;
        attacking.mode = module_data::AnimationMode::Once;
        attacking.allow_to_finish_key = 2; // allow finish from Moving
        attacking.animations.push(module_data::W3DAnimationInfo::with_name("AttackAnim", false, 0.0));

        vec![idle, moving, attacking]
    }

    #[test]
    fn test_set_model_state_basic_transition() {
        let mut draw = W3DModelDraw::new();
        draw.set_condition_states(make_test_condition_states(), HashMap::new(), 0);

        // Initial state: set to idle (index 0)
        draw.set_model_state(0);
        assert_eq!(draw.cur_state_index, 0);
        assert_eq!(draw.which_anim_in_cur_state, 0);

        // Transition to moving (index 1)
        draw.set_model_state(1);
        assert_eq!(draw.cur_state_index, 1);
        assert_eq!(draw.next_state_index, -1);
    }

    #[test]
    fn test_set_model_state_duplicate_returns_early() {
        let mut draw = W3DModelDraw::new();
        draw.set_condition_states(make_test_condition_states(), HashMap::new(), 0);

        draw.set_model_state(0);
        assert_eq!(draw.cur_state_index, 0);
        draw.which_anim_in_cur_state = 1;

        // Setting same state with no pending → should return early
        draw.set_model_state(0);
        assert_eq!(draw.cur_state_index, 0);
        // which_anim should NOT have changed (no adjustAnimation called)
        assert_eq!(draw.which_anim_in_cur_state, 1);
    }

    #[test]
    fn test_set_model_state_allow_to_finish_defer() {
        let states = make_test_condition_states();
        let mut draw = W3DModelDraw::new();
        draw.set_condition_states(states, HashMap::new(), 0);

        // Start in Moving (transition_key=2)
        draw.set_model_state(1);
        assert_eq!(draw.cur_state_index, 1);

        // Attacking has allow_to_finish_key=2 which matches Moving's transition_key=2
        // Animation NOT complete → should defer to nextState
        draw.animation_complete = false;
        draw.render_object_id = Some(42);
        draw.set_model_state(2);
        assert_eq!(draw.cur_state_index, 1); // Still in Moving
        assert_eq!(draw.next_state_index, 2); // Pending transition
    }

    #[test]
    fn test_replace_model_condition_state_basic() {
        let states = make_test_condition_states();
        let mut draw = W3DModelDraw::new();
        draw.set_condition_states(states, HashMap::new(), 0);

        // Empty flags → should match idle (conditions = vec![0])
        draw.replace_model_condition_state(0);
        assert_eq!(draw.cur_state_index, 0);

        // Moving flag set
        draw.replace_model_condition_state(1u128 << 49);
        assert_eq!(draw.cur_state_index, 1);

        // Night flag should unhide headlights
        draw.replace_model_condition_state(1u128 << 7);
        assert!(!draw.hide_headlights);
    }

    #[test]
    fn test_set_animation_loop_duration() {
        let mut draw = W3DModelDraw::new();
        draw.set_animation_loop_duration(30);
        assert_eq!(draw.next_state_anim_loop_duration, NO_NEXT_DURATION);
    }

    #[test]
    fn test_set_animation_completion_time_no_transition() {
        let states = make_test_condition_states();
        let mut draw = W3DModelDraw::new();
        draw.set_condition_states(states, HashMap::new(), 0);
        draw.set_model_state(0);

        draw.set_animation_completion_time(60);
        assert_eq!(draw.next_state_anim_loop_duration, NO_NEXT_DURATION);
    }

    #[test]
    fn test_set_pause_animation_toggle() {
        let mut draw = W3DModelDraw::new();
        assert!(!draw.pause_animation);
        draw.set_pause_animation(true);
        assert!(draw.pause_animation);
        draw.set_pause_animation(false);
        assert!(!draw.pause_animation);
        // Setting same value → no-op
        draw.set_pause_animation(false);
        assert!(!draw.pause_animation);
    }

    #[test]
    fn test_do_draw_module_animation_complete_transitions() {
        let states = make_test_condition_states();
        let mut draw = W3DModelDraw::new();
        draw.set_condition_states(states, HashMap::new(), 0);
        draw.render_object_id = Some(1);

        draw.set_model_state(0);
        assert_eq!(draw.cur_state_index, 0);

        // Simulate animation complete with a pending next state
        draw.next_state_index = 1;
        draw.next_state_anim_loop_duration = NO_NEXT_DURATION;
        draw.animation_complete = true;

        let transform = Matrix4::identity();
        draw.do_draw_module(&transform);

        // Should have transitioned to state 1
        assert_eq!(draw.cur_state_index, 1);
        assert_eq!(draw.next_state_index, -1);
        assert!(!draw.animation_complete);
    }

    #[test]
    fn test_do_draw_module_idle_restarts() {
        let states = make_test_condition_states();
        let mut draw = W3DModelDraw::new();
        draw.set_condition_states(states, HashMap::new(), 0);
        draw.render_object_id = Some(1);

        draw.set_model_state(0);
        assert_eq!(draw.which_anim_in_cur_state, 0);

        // Simulate animation complete for idle anim
        draw.animation_complete = true;
        let transform = Matrix4::identity();
        draw.do_draw_module(&transform);

        // Idle anim should have switched to a different animation
        assert_ne!(draw.which_anim_in_cur_state, 0);
        assert!(!draw.animation_complete);
    }
}
