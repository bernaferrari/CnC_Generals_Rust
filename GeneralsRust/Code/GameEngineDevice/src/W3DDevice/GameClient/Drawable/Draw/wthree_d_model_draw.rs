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
use cgmath::{Matrix4, Point3, Vector3};

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
}

/// Sentinel value indicating no pending animation loop duration (C++: NO_NEXT_DURATION).
const NO_NEXT_DURATION: u32 = 0xFFFF_FFFF;

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
    pub fn do_draw_module(&mut self, transform_mtx: &Matrix4<f32>) {
        // ── Step 1: update whether or not we should be animating ──
        // C++: setPauseAnimation( !getDrawable()->getShouldAnimate(m_animationsRequirePower) );
        // PARITY_NOTE: getDrawable() / getShouldAnimate() not yet wired; derive pause from
        // the power-gate flag when set, otherwise leave current pause state unchanged.
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
        // C++: if (isAnimationComplete(m_renderObject)) { ... state transition / idle switch / restart ... }
        // PARITY_NOTE: isAnimationComplete() requires RenderObjClass HLod animation peek.
        // When a render object exists and animation is reported complete, perform the three
        // sub-branches (nextState transition, idle random switch, RESTART_ANIM_WHEN_COMPLETE).
        if self.render_object_id.is_some() {
            // PARITY_NOTE: isAnimationComplete(m_renderObject) not wired.
            // When it returns true, the following three branches execute in C++ order:
            //
            // (a) Transition to pending nextState if one exists:
            //     if (m_nextState != NULL) {
            //         const ModelConditionInfo* nextState = m_nextState;
            //         UnsignedInt nextDuration = m_nextStateAnimLoopDuration;
            //         m_nextState = NULL; m_nextStateAnimLoopDuration = NO_NEXT_DURATION;
            //         setModelState(nextState);
            //         if (nextDuration != NO_NEXT_DURATION) setAnimationLoopDuration(nextDuration);
            //     }
            //
            // (b) If current anim is idle, randomly switch to another idle:
            //     if (m_curState->m_animations[m_whichAnimInCurState].isIdleAnim())
            //         adjustAnimation(m_curState, -1.0);
            //
            // (c) If RESTART_ANIM_WHEN_COMPLETE flag set, restart:
            //     else if (testFlagBit(m_curState->m_flags, RESTART_ANIM_WHEN_COMPLETE))
            //         adjustAnimation(m_curState, -1.0);
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

        let visible = self.is_visible();

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

        // ── Build bone override submission ──
        let mut bone_overrides = Vec::new();

        // PARITY_NOTE: Turret yaw/pitch source (AI + model state) is not wired here yet;
        // submit stable placeholder slots so bridge layout matches C++ intent.
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
        };

        self.last_submission = Some(submission.clone());

        // ── Submit draw state to the scene / render bridge ──
        // PARITY_NOTE: C++ calls m_renderObject->Set_Transform(mtx) directly on the
        // RenderObjClass. Until RenderObjClass parity is complete, we push the adjusted
        // transform, visibility, and shadow state into the W3DScene render objects keyed
        // by render IDs. This preserves the data flow ordering so the WGPU pipeline can
        // consume the same per-frame state when it is wired.
        let scene = W3DDisplay::global_scene();
        let mut scene_guard = scene.write();

        let position = Point3::new(
            submission.world_transform.w.x,
            submission.world_transform.w.y,
            submission.world_transform.w.z,
        );

        let mut apply_object_state = |id: Option<RenderObjectId>, visible: bool, hidden: bool| {
            if let Some(id) = id {
                if let Some(obj) = scene_guard.get_render_object_mut(id) {
                    obj.world_transform = submission.world_transform;
                    obj.position = position;
                    obj.object_scale = submission.instance_scale;
                    obj.visible = visible;
                    obj.hidden = hidden;
                }
            }
        };

        apply_object_state(
            submission.render_object_id,
            submission.visible,
            !submission.visible,
        );
        apply_object_state(
            submission.shadow_id,
            submission.visible && submission.shadow_enabled,
            !submission.visible || !submission.shadow_enabled,
        );
        apply_object_state(
            submission.terrain_decal_id,
            submission.visible,
            !submission.visible,
        );
        apply_object_state(
            submission.track_render_object_id,
            submission.visible,
            !submission.visible,
        );
    }

    fn adjust_anim_speed_to_movement_speed(&mut self) {
        // PARITY_NOTE: C++ queries getCurAnimDistanceCovered(), Object, PhysicsBehavior->
        // getVelocityMagnitude() and calls setCurAnimDurationInMsec(). These subsystems
        // are not yet available. Stub preserves call position in the do_draw_module frame
        // ordering.
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

    /// State machine: transition to a new condition state.
    /// Handles duplicate detection, allowToFinishKey deferral,
    /// transition states, render object creation/replacement,
    /// bone/turret/barrel validation, show/hide sub-objects,
    /// shadow creation, track binding.
    pub fn set_model_state(&mut self, _new_state_index: i32) {
        // PARITY_NOTE: Full 4302-line C++ implementation
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

    pub fn react_to_geometry_change(&mut self) {}
    pub fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadow_enabled = enable;
    }
    pub fn release_shadows(&mut self) {}
    pub fn allocate_shadows(&mut self) {}
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }

    /// Save: version 2, weapon recoil info + sub-object vector + animation frame fraction
    pub fn xfer_save(&self) -> u32 {
        2
    }
    pub fn xfer_load(&mut self, _version: u32) {}
    pub fn crc(&self) -> u32 {
        0
    }
    pub fn load_post_process(&mut self) {}

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
}
