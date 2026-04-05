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
    /// Animation mode table: INITIAL=ONCE, FLYING=LOOP, FINAL=ONCE
    /// State transitions:
    /// - If not FINAL and object not above terrain and frames > 3: transition to FINAL
    /// - Else if not FINAL and animation complete: increment state (INITIAL->FLYING->FINAL)
    /// On entering FINAL: fire finalFX list
    /// If finalStop: override mode to ANIM_MODE_MANUAL
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        if self.render_object_id.is_none() {
            return;
        }

        // PARITY_NOTE: Instance scaling if getDrawable()->getInstanceScale() != 1.0
        // PARITY_NOTE: Set transform on render object

        // State transition logic
        if self.state != AnimState::Final {
            // PARITY_NOTE: Check if object is above terrain
            // if (!isAboveTerrain && m_frames > MIN_FINAL_FRAMES) { state = FINAL }
            // else if (isAnimationComplete(m_renderObject)) { state++ }
        }

        // PARITY_NOTE: Apply animation based on state
        // if entering FINAL: fire FXList at drawable position
        // if finalStop: mode = ANIM_MODE_MANUAL
        // m_renderObject->Set_Animation(hanim, frame, mode)

        self.frames += 1;
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
}
