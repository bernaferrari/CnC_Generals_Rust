//! W3DDebrisDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DDebrisDraw.cpp
//!
//! Manages its own RenderObjClass directly (does NOT inherit from W3DModelDraw).
//! Implements DebrisDrawInterface. Has a 3-state animation state machine:
//! INITIAL (play once) -> FLYING (loop) -> FINAL (play once).
//! Handles shadows via TheW3DShadowManager.

use crate::W3DDevice::GameClient::wthree_d_scene::RenderObjectId;
use cgmath::{Matrix4, Point3, Vector3};

/// Animation state machine (C++: AnimStateType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimState {
    Initial = 0,
    Flying = 1,
    Final = 2,
}

/// Number of animation states (C++: STATECOUNT)
const STATE_COUNT: usize = 3;

/// Animation playback mode (C++: RenderObjClass::AnimMode)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimMode {
    /// Play animation once, then hold on last frame (C++: ANIM_MODE_ONCE)
    Once = 0,
    /// Loop animation continuously (C++: ANIM_MODE_LOOP)
    Loop = 1,
    /// Hold animation at a manually-set frame (C++: ANIM_MODE_MANUAL)
    Manual = 2,
}

/// Animation mode table per state (C++: TheAnimModes[STATECOUNT])
/// INITIAL=ONCE, FLYING=LOOP, FINAL=ONCE
const ANIM_MODES: [AnimMode; STATE_COUNT] = [
    AnimMode::Once, // INITIAL
    AnimMode::Loop, // FLYING
    AnimMode::Once, // FINAL
];

/// Minimum frames before FINAL transition (C++: MIN_FINAL_FRAMES)
const MIN_FINAL_FRAMES: i32 = 3;

/// W3DDebrisDraw implementation
///
/// Self-contained draw module that manages its own render object and 3-state animation.
/// Save/load data: modelName, modelColor, 3 anim names, state, frames, finalStop.
/// Note: ShadowType and finalFX are NOT preserved across save/load (C++ parity).
#[derive(Debug)]
pub struct W3DDebrisDraw {
    model_name: String,
    model_color: u32,
    anim_initial: String,
    anim_flying: String,
    anim_final: String,
    state: AnimState,
    frames: i32,
    final_stop: bool,
    render_object_id: Option<RenderObjectId>,
    shadow_id: Option<RenderObjectId>,
    hidden: bool,
    fully_obscured_by_shroud: bool,

    // --- State machine tracking (C++-faithful) ---
    /// Instance scale from drawable (C++: getDrawable()->getInstanceScale())
    instance_scale: f32,
    /// Whether the game-logic object is currently above terrain.
    /// Set by DebrisBehavior::isAboveTerrain() each frame.
    /// C++ parity: obj->isAboveTerrain() checked in doDrawModule.
    above_terrain: bool,
    /// Whether the current animation has completed.
    /// Set by the animation system when HLod::Is_Animation_Complete() returns true.
    /// C++ parity: isAnimationComplete(m_renderObject) checked in doDrawModule.
    anim_complete: bool,
    /// Name of the animation currently set on the render object.
    /// Used to detect animation changes (C++: hanim != m_renderObject->Peek_Animation()).
    current_anim_name: String,
    /// Current animation mode set on the render object.
    current_anim_mode: AnimMode,
    /// Whether the final FX has already been fired for this FINAL state entry.
    /// C++ parity: FXList::doFXPos(m_fxFinal, ...) fires once when entering FINAL.
    fx_final_fired: bool,
    /// Name of the final FX list to fire on FINAL state entry (C++: m_fxFinal).
    /// Not preserved across save/load (C++ parity).
    fx_final_name: String,
}

impl W3DDebrisDraw {
    pub fn new() -> Self {
        Self {
            model_name: String::new(),
            model_color: 0,
            anim_initial: String::new(),
            anim_flying: String::new(),
            anim_final: String::new(),
            state: AnimState::Initial,
            frames: 0,
            final_stop: false,
            render_object_id: None,
            shadow_id: None,
            hidden: false,
            fully_obscured_by_shroud: false,
            instance_scale: 1.0,
            above_terrain: true,
            anim_complete: false,
            current_anim_name: String::new(),
            current_anim_mode: AnimMode::Once,
            fx_final_fired: false,
            fx_final_name: String::new(),
        }
    }

    /// Set the W3D model name and color. Only runs if render object is NULL.
    /// C++ parity: Creates render object via W3DDisplay::m_assetManager->Create_Render_Obj().
    /// If non-zero color, ORs with 0xFF000000 (alpha=255).
    /// Adds to scene, sets user data, creates shadow if not SHADOW_NONE.
    pub fn set_model_name(&mut self, name: &str, color: u32) {
        if self.render_object_id.is_some() || name.is_empty() {
            return;
        }
        self.model_name = name.to_string();
        self.model_color = color;
        // PARITY_NOTE: Full C++ implementation:
        // hexColor = (color != 0) ? (color | 0xFF000000) : 0;
        // m_renderObject = W3DDisplay::m_assetManager->Create_Render_Obj(name, scale, hexColor);
        // W3DDisplay::m_3DScene->Add_Render_Object(m_renderObject);
        // m_renderObject->Set_User_Data(getDrawable()->getDrawableInfo());
        // m_renderObject->Set_Transform(identity);
        // Create shadow via TheW3DShadowManager->addShadow()
    }

    /// Set animation names and FX list for final state.
    /// If final == "STOP", sets finalStop=true and uses flying anim for final state.
    /// Resets state=0, frames=0.
    pub fn set_anim_names(&mut self, initial: &str, flying: &str, final_: &str) {
        self.anim_initial = initial.to_string();
        self.anim_flying = flying.to_string();
        if final_.eq_ignore_ascii_case("STOP") {
            self.final_stop = true;
            self.anim_final = flying.to_string();
        } else {
            self.final_stop = false;
            self.anim_final = final_.to_string();
        }
        self.state = AnimState::Initial;
        self.frames = 0;
        // PARITY_NOTE: Release old anims, load new via W3DDisplay::m_assetManager->Get_HAnim()
    }

    /// Copy drawable's transform to render object.
    pub fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix4<f32>,
        _old_pos: &Point3<f32>,
        _old_angle: f32,
    ) {
        // PARITY_NOTE: m_renderObject->Set_Transform(*getDrawable()->getTransformMatrix())
    }

    /// Main per-frame draw with 3-state animation state machine.
    ///
    /// C++ parity (W3DDebrisDraw::doDrawModule):
    /// 1. If no render object, return immediately.
    /// 2. Apply instance scaling if getDrawable()->getInstanceScale() != 1.0.
    /// 3. Set transform on render object.
    /// 4. State transitions:
    ///    a. If state != FINAL && !aboveTerrain && frames > MIN_FINAL_FRAMES → FINAL
    ///    b. Else if state < FINAL && isAnimationComplete → state++
    /// 5. Set animation: lookup hanim for state, apply AnimMode from table.
    ///    On entering FINAL: fire finalFX; if finalStop, override to ANIM_MODE_MANUAL.
    /// 6. Increment frame counter.
    pub fn do_draw_module(&mut self, transform_mtx: &Matrix4<f32>) {
        if self.render_object_id.is_none() {
            return;
        }

        // Step 2: Instance scaling
        // C++: if (getDrawable()->getInstanceScale() != 1.0f) {
        //   scaledTransform = *transformMtx;
        //   scaledTransform.Scale(getDrawable()->getInstanceScale());
        //   transformMtx = &scaledTransform;
        //   m_renderObject->Set_ObjectScale(getDrawable()->getInstanceScale());
        // }
        let effective_transform = if self.instance_scale != 1.0 {
            let scale = Matrix4::from_scale(self.instance_scale);
            scale * transform_mtx
        } else {
            *transform_mtx
        };

        // Step 3: Set transform on render object
        // PARITY_NOTE: m_renderObject->Set_Transform(*transformMtx)
        let _ = effective_transform;

        // Step 4: State transition logic
        let old_state = self.state;

        // C++: if (m_state != FINAL && obj != NULL && !obj->isAboveTerrain() && m_frames > MIN_FINAL_FRAMES)
        if self.state != AnimState::Final && !self.above_terrain && self.frames > MIN_FINAL_FRAMES {
            self.state = AnimState::Final;
        }
        // C++: else if (m_state < FINAL && isAnimationComplete(m_renderObject))
        else if self.state != AnimState::Final && self.anim_complete {
            self.state = match self.state {
                AnimState::Initial => AnimState::Flying,
                AnimState::Flying => AnimState::Final,
                AnimState::Final => AnimState::Final,
            };
        }

        // Step 5: Set animation for current state
        // C++: HAnimClass* hanim = m_anims[m_state];
        // C++: if (hanim != NULL && (hanim != m_renderObject->Peek_Animation() || oldState != m_state))
        let hanim_name = self.anim_name_for_state(self.state);
        let anim_changed = hanim_name != self.current_anim_name;
        let state_changed = old_state != self.state;

        if !hanim_name.is_empty() && (anim_changed || state_changed) {
            // C++: RenderObjClass::AnimMode m = TheAnimModes[m_state];
            let mut mode = ANIM_MODES[self.state as usize];

            if self.state == AnimState::Final {
                // C++: FXList::doFXPos(m_fxFinal, getDrawable()->getPosition(),
                //   getDrawable()->getTransformMatrix(), 0, NULL, 0.0f);
                if !self.fx_final_fired && !self.fx_final_name.is_empty() {
                    // PARITY_NOTE: FXList::doFXPos(m_fxFinal, position, transform, ...)
                    // FX system not yet wired; fire tracked via fx_final_fired flag
                }
                self.fx_final_fired = true;

                // C++: if (m_finalStop) m = RenderObjClass::ANIM_MODE_MANUAL;
                if self.final_stop {
                    mode = AnimMode::Manual;
                }
            }

            // C++: m_renderObject->Set_Animation(hanim, 0, m);
            // PARITY_NOTE: Set animation on render object via scene manager
            self.current_anim_name = hanim_name;
            self.current_anim_mode = mode;
        }

        // Step 6: Increment frame counter
        // C++: ++m_frames;
        self.frames += 1;
    }

    fn anim_name_for_state(&self, state: AnimState) -> &str {
        match state {
            AnimState::Initial => &self.anim_initial,
            AnimState::Flying => &self.anim_flying,
            AnimState::Final => &self.anim_final,
        }
    }

    pub fn set_instance_scale(&mut self, scale: f32) {
        self.instance_scale = scale;
    }

    pub fn set_above_terrain(&mut self, above: bool) {
        self.above_terrain = above;
    }

    pub fn set_anim_complete(&mut self, complete: bool) {
        self.anim_complete = complete;
    }

    pub fn set_fx_final(&mut self, fx_name: &str) {
        self.fx_final_name = fx_name.to_string();
    }

    pub fn get_current_anim_mode(&self) -> AnimMode {
        self.current_anim_mode
    }

    pub fn get_state(&self) -> AnimState {
        self.state
    }

    pub fn set_shadows_enabled(&mut self, _enable: bool) {
        // PARITY_NOTE: m_shadow->enableShadowRender(enable)
    }

    pub fn set_fully_obscured_by_shroud(&mut self, _fully_obscured: bool) {
        // PARITY_NOTE: m_shadow->enableShadowInvisible(fullyObscured)
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }
    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }

    /// Save data: (version, state, frames, finalStop, modelName, modelColor, 3 anim names)
    /// Note: ShadowType and finalFX are lost on save/load (C++ parity).
    pub fn xfer_save(&self) -> (u32, i32, i32, bool, String, u32, String, String, String) {
        (
            1,
            self.state as i32,
            self.frames,
            self.final_stop,
            self.model_name.clone(),
            self.model_color,
            self.anim_initial.clone(),
            self.anim_flying.clone(),
            self.anim_final.clone(),
        )
    }

    /// Load data and restore model/anim state.
    /// Note: ShadowType is always SHADOW_NONE on reload, finalFX is always NULL (C++ parity).
    pub fn xfer_load(
        &mut self,
        _version: u32,
        state: i32,
        frames: i32,
        final_stop: bool,
        model_name: String,
        model_color: u32,
        anim_initial: String,
        anim_flying: String,
        anim_final: String,
    ) {
        self.model_name = model_name;
        self.model_color = model_color;
        // PARITY_NOTE: setModelName(modelName, modelColor, SHADOW_NONE)
        self.anim_initial = anim_initial;
        self.anim_flying = anim_flying;
        self.anim_final = anim_final;
        // PARITY_NOTE: setAnimNames(animInitial, animFlying, animFinal, NULL)
        self.state = match state {
            0 => AnimState::Initial,
            1 => AnimState::Flying,
            2 => AnimState::Final,
            _ => AnimState::Initial,
        };
        self.frames = frames;
        self.final_stop = final_stop;
        self.current_anim_name = String::new();
        self.current_anim_mode = AnimMode::Once;
        self.fx_final_fired = false;
        self.fx_final_name = String::new();
    }

    pub fn crc(&self) -> u32 {
        0
    }
    pub fn load_post_process(&mut self) {}

    fn on_delete(&mut self) {
        // PARITY_NOTE: Remove shadow from TheW3DShadowManager
        // Remove render object from W3DDisplay::m_3DScene
        // Release animations
        self.render_object_id = None;
        self.shadow_id = None;
    }
}

impl Default for W3DDebrisDraw {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for W3DDebrisDraw {
    fn drop(&mut self) {
        self.on_delete();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_debris_draw_basic() {
        let draw = W3DDebrisDraw::new();
        assert_eq!(draw.state, AnimState::Initial);
        assert_eq!(draw.frames, 0);
        assert!(!draw.final_stop);
    }

    #[test]
    fn test_wthree_d_debris_draw_anim_names() {
        let mut draw = W3DDebrisDraw::new();
        draw.set_anim_names("INIT", "FLY", "STOP");
        assert!(draw.final_stop);
        assert_eq!(draw.anim_final, "FLY");
        assert_eq!(draw.state, AnimState::Initial);
    }

    #[test]
    fn test_wthree_d_debris_draw_xfer() {
        let mut draw = W3DDebrisDraw::new();
        draw.set_anim_names("INIT", "FLY", "FINAL");
        draw.frames = 42;
        draw.state = AnimState::Flying;
        let (ver, state, frames, final_stop, _, _, _, _, _) = draw.xfer_save();
        assert_eq!(ver, 1);
        assert_eq!(state, 1);
        assert_eq!(frames, 42);
        assert!(!final_stop);
    }

    #[test]
    fn test_wthree_d_debris_draw_no_render_object_early_return() {
        let mut draw = W3DDebrisDraw::new();
        draw.do_draw_module(&Matrix4::identity());
        assert_eq!(draw.frames, 0);
    }

    #[test]
    fn test_wthree_d_debris_draw_state_transition_by_terrain() {
        let mut draw = W3DDebrisDraw::new();
        draw.set_anim_names("INIT", "FLY", "FINAL");
        draw.set_model_name("Debris", 0);
        draw.set_above_terrain(false);
        for _ in 0..MIN_FINAL_FRAMES + 1 {
            draw.do_draw_module(&Matrix4::identity());
        }
        assert_eq!(draw.state, AnimState::Final);
        assert!(draw.fx_final_fired);
    }

    #[test]
    fn test_wthree_d_debris_draw_state_transition_by_anim_complete() {
        let mut draw = W3DDebrisDraw::new();
        draw.set_anim_names("INIT", "FLY", "FINAL");
        draw.set_model_name("Debris", 0);
        draw.set_anim_complete(true);
        draw.do_draw_module(&Matrix4::identity());
        assert_eq!(draw.state, AnimState::Flying);
        draw.set_anim_complete(true);
        draw.do_draw_module(&Matrix4::identity());
        assert_eq!(draw.state, AnimState::Final);
    }

    #[test]
    fn test_wthree_d_debris_draw_final_stop_mode() {
        let mut draw = W3DDebrisDraw::new();
        draw.set_anim_names("INIT", "FLY", "STOP");
        draw.set_model_name("Debris", 0);
        draw.set_above_terrain(false);
        for _ in 0..MIN_FINAL_FRAMES + 1 {
            draw.do_draw_module(&Matrix4::identity());
        }
        assert_eq!(draw.state, AnimState::Final);
        assert_eq!(draw.current_anim_mode, AnimMode::Manual);
    }

    #[test]
    fn test_wthree_d_debris_draw_normal_final_mode() {
        let mut draw = W3DDebrisDraw::new();
        draw.set_anim_names("INIT", "FLY", "FINAL");
        draw.set_model_name("Debris", 0);
        draw.set_above_terrain(false);
        for _ in 0..MIN_FINAL_FRAMES + 1 {
            draw.do_draw_module(&Matrix4::identity());
        }
        assert_eq!(draw.state, AnimState::Final);
        assert_eq!(draw.current_anim_mode, AnimMode::Once);
    }

    #[test]
    fn test_wthree_d_debris_draw_instance_scaling() {
        let mut draw = W3DDebrisDraw::new();
        draw.set_anim_names("INIT", "FLY", "FINAL");
        draw.set_model_name("Debris", 0);
        draw.set_instance_scale(2.0);
        draw.do_draw_module(&Matrix4::identity());
        assert_eq!(draw.frames, 1);
    }

    #[test]
    fn test_wthree_d_debris_draw_anim_modes_table() {
        assert_eq!(ANIM_MODES[AnimState::Initial as usize], AnimMode::Once);
        assert_eq!(ANIM_MODES[AnimState::Flying as usize], AnimMode::Loop);
        assert_eq!(ANIM_MODES[AnimState::Final as usize], AnimMode::Once);
    }

    #[test]
    fn test_wthree_d_debris_draw_terrain_priority_over_anim_complete() {
        let mut draw = W3DDebrisDraw::new();
        draw.set_anim_names("INIT", "FLY", "FINAL");
        draw.set_model_name("Debris", 0);
        draw.set_above_terrain(false);
        draw.set_anim_complete(true);
        for _ in 0..MIN_FINAL_FRAMES + 1 {
            draw.do_draw_module(&Matrix4::identity());
        }
        assert_eq!(draw.state, AnimState::Final);
    }
}
