//! W3DDebrisDraw - Debris particle rendering
//!
//! Port of C++ W3DDebrisDraw.h
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DDebrisDraw.h
//!
//! Renders flying debris from explosions and destruction

use super::draw_module::{DebrisDrawInterface, DrawModule, DrawModuleData, ShadowType};
use crate::common::*;
use crate::effects::FXList;
use crate::helpers::TheGameLogic;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData};
use log::debug;
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DDebrisDrawModuleData {
    module_tag_name_key: NameKeyType,
    // No template data, all set at runtime
}

impl W3DDebrisDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl Default for W3DDebrisDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for W3DDebrisDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl DrawModuleData for W3DDebrisDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DDebrisDrawModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DebrisAnimState {
    Initial,
    Flying,
    Final,
}

fn debris_state_to_i32(state: DebrisAnimState) -> i32 {
    match state {
        DebrisAnimState::Initial => 0,
        DebrisAnimState::Flying => 1,
        DebrisAnimState::Final => 2,
    }
}

fn debris_state_from_i32(value: i32) -> DebrisAnimState {
    match value {
        1 => DebrisAnimState::Flying,
        2 => DebrisAnimState::Final,
        _ => DebrisAnimState::Initial,
    }
}

fn color_to_packed_i32(color: Color) -> i32 {
    color.to_argb_u32() as i32
}

fn color_from_packed_i32(value: i32) -> Color {
    let packed = value as u32;
    Color::new(
        (packed & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 24) & 0xFF) as u8,
    )
}

const MIN_FINAL_FRAMES: u32 = 3;
const INITIAL_TO_FLYING_FRAMES: u32 = 10;

pub struct W3DDebrisDraw {
    _data: W3DDebrisDrawModuleData,
    model_name: AsciiString,
    model_color: Color,
    shadow_type: ShadowType,
    anim_initial: AsciiString,
    anim_flying: AsciiString,
    anim_final: AsciiString,
    final_fx: Option<FXList>,
    current_state: DebrisAnimState,
    state_frame_count: u32,
    final_stopped: bool,
    owner_id: Option<ObjectID>,
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadows_enabled: bool,
}

impl W3DDebrisDraw {
    pub fn new(data: W3DDebrisDrawModuleData) -> Self {
        Self {
            _data: data,
            model_name: AsciiString::new(),
            model_color: Color::white(),
            shadow_type: ShadowType::None,
            anim_initial: AsciiString::new(),
            anim_flying: AsciiString::new(),
            anim_final: AsciiString::new(),
            final_fx: None,
            current_state: DebrisAnimState::Initial,
            state_frame_count: 0,
            final_stopped: false,
            owner_id: None,
            hidden: false,
            fully_obscured_by_shroud: false,
            shadows_enabled: false,
        }
    }

    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
    }

    fn transition_to_flying(&mut self) {
        if self.current_state == DebrisAnimState::Initial {
            self.current_state = DebrisAnimState::Flying;
            self.state_frame_count = 0;
        }
    }

    fn transition_to_final(&mut self, position: &Coord3D, _transform: &Matrix3D) {
        if self.current_state == DebrisAnimState::Flying {
            self.current_state = DebrisAnimState::Final;
            self.state_frame_count = 0;

            // Matches C++ W3DDebrisDraw.cpp:228 - Play final FX on transition to FINAL state
            if let Some(fx_list) = &self.final_fx {
                debug!(
                    "W3DDebrisDraw: Playing final FX at ({:.2}, {:.2}, {:.2})",
                    position.x, position.y, position.z
                );
                // In full implementation: FXList::doFXPos(fx_list, position, transform, 0, NULL, 0.0f)
                let _ = fx_list.do_fx_at_position(position);
            }
        }
    }

    fn get_current_animation(&self) -> &AsciiString {
        match self.current_state {
            DebrisAnimState::Initial => &self.anim_initial,
            DebrisAnimState::Flying => &self.anim_flying,
            DebrisAnimState::Final => &self.anim_final,
        }
    }

    fn owner_terrain_state(&self) -> Option<(bool, Coord3D)> {
        let owner_id = self.owner_id?;
        let owner = TheGameLogic::find_object_by_id(owner_id)?;
        let owner_guard = owner.read().ok()?;
        Some((owner_guard.is_above_terrain(), *owner_guard.get_position()))
    }

    fn should_transition_to_final(
        state: DebrisAnimState,
        state_frame_count: u32,
        is_above_terrain: bool,
    ) -> bool {
        state == DebrisAnimState::Flying
            && state_frame_count >= MIN_FINAL_FRAMES
            && !is_above_terrain
    }
}

impl Module for W3DDebrisDraw {
    fn on_drawable_bound_to_object(&mut self) {}
    fn on_delete(&mut self) {}
    fn get_module_name_key(&self) -> NameKeyType {
        self._data.module_tag_name_key
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self._data.module_tag_name_key
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self._data
    }
}

impl DrawModule for W3DDebrisDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        if self.model_name.is_empty() || self.hidden || self.fully_obscured_by_shroud {
            return;
        }

        // Matches C++ W3DDebrisDraw.cpp:189-236

        // Store old state to detect transitions (C++ line 211)
        let _old_state = self.current_state;

        // Matches C++ lines 214-221: Check for state transitions
        // Transition to FINAL if object has landed and enough frames have passed
        if let Some((is_above_terrain, owner_pos)) = self.owner_terrain_state() {
            if Self::should_transition_to_final(
                self.current_state,
                self.state_frame_count,
                is_above_terrain,
            ) {
                self.transition_to_final(&owner_pos, transform_mtx);
            }
        }

        // Matches C++ lines 218-221: Auto-advance from INITIAL to FLYING when animation completes
        if self.current_state == DebrisAnimState::Initial {
            // In full implementation, this would call isAnimationComplete(m_renderObject)
            // For now we use a simple frame count heuristic
            // Real check: hlod->Is_Animation_Complete() (C++ lines 159-168)
            if self.state_frame_count > INITIAL_TO_FLYING_FRAMES {
                self.transition_to_flying();
            }
        } else if self.current_state != DebrisAnimState::Final {
            // Check if animation is complete and advance state (C++ line 218)
            // This is where C++ calls isAnimationComplete(m_renderObject)
            // In the real implementation, this checks the W3D render object
        }

        // Matches C++ lines 222-233: Handle animation updates
        // In the full implementation:
        // 1. Get current animation for state (m_anims[m_state])
        // 2. Check if it's different from current (hanim != m_renderObject->Peek_Animation())
        // 3. Set animation mode (ANIM_MODE_ONCE, ANIM_MODE_LOOP, ANIM_MODE_MANUAL)
        // 4. Call m_renderObject->Set_Animation(hanim, 0, mode) (C++ line 232)

        // For FINAL state with m_finalStop flag, use ANIM_MODE_MANUAL (C++ line 230)
        if self.current_state == DebrisAnimState::Final && self.final_stopped {
            // Animation is frozen
        }

        // Increment frame counter (C++ line 234)
        self.state_frame_count += 1;

        // Note: Actual rendering happens in the W3D device layer
        // This module just manages state and animation selection
        // C++ line 202: m_renderObject->Set_Transform(*transformMtx)
        let _ = (transform_mtx, self.get_current_animation());
    }

    fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadows_enabled = enable;
    }

    fn release_shadows(&mut self) {}
    fn allocate_shadows(&mut self) {}
    fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
    }
    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
    }
    fn react_to_geometry_change(&mut self) {
        self.state_frame_count = 0;
    }

    fn get_debris_draw_interface(&self) -> Option<&dyn DebrisDrawInterface> {
        Some(self)
    }

    fn get_debris_draw_interface_mut(&mut self) -> Option<&mut dyn DebrisDrawInterface> {
        Some(self)
    }
}

impl DebrisDrawInterface for W3DDebrisDraw {
    fn set_model_name(&mut self, name: AsciiString, color: Color, shadow_type: ShadowType) {
        self.model_name = name;
        self.model_color = color;
        self.shadow_type = shadow_type;
    }

    fn set_anim_names(
        &mut self,
        initial: AsciiString,
        flying: AsciiString,
        mut final_anim: AsciiString,
        final_fx: Option<&FXList>,
    ) {
        // Matches C++ W3DDebrisDraw.cpp:127-156

        self.anim_initial = initial;
        self.anim_flying = flying.clone();

        // Matches C++ lines 138-146: Handle special "STOP" animation
        // If final animation is "STOP", reuse flying animation and set m_finalStop flag
        if final_anim.as_str().eq_ignore_ascii_case("STOP") {
            self.final_stopped = true;
            final_anim = flying; // Use flying animation, but stop it in ANIM_MODE_MANUAL
        } else {
            self.final_stopped = false;
        }

        self.anim_final = final_anim;

        // Reset state machine (C++ lines 148-149)
        self.current_state = DebrisAnimState::Initial;
        self.state_frame_count = 0;

        // Store FX list reference (C++ line 150)
        // Matches C++: m_fxFinal = finalFX
        self.final_fx = final_fx.cloned();
    }
}

impl Snapshotable for W3DDebrisDraw {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ parity: W3DDebrisDraw::xfer (version 1).
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        let mut model_name = self.model_name.as_str().to_string();
        xfer.xfer_ascii_string(&mut model_name)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.model_name = AsciiString::from(model_name.as_str());
        }

        let mut packed_color = color_to_packed_i32(self.model_color);
        xfer.xfer_color(&mut packed_color)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.model_color = color_from_packed_i32(packed_color);
        }
        if xfer.is_reading() {
            self.set_model_name(self.model_name.clone(), self.model_color, ShadowType::None);
        }

        let mut anim_initial = self.anim_initial.as_str().to_string();
        xfer.xfer_ascii_string(&mut anim_initial)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.anim_initial = AsciiString::from(anim_initial.as_str());
        }

        let mut anim_flying = self.anim_flying.as_str().to_string();
        xfer.xfer_ascii_string(&mut anim_flying)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.anim_flying = AsciiString::from(anim_flying.as_str());
        }

        let mut anim_final = self.anim_final.as_str().to_string();
        xfer.xfer_ascii_string(&mut anim_final)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.anim_final = AsciiString::from(anim_final.as_str());
            self.set_anim_names(
                self.anim_initial.clone(),
                self.anim_flying.clone(),
                self.anim_final.clone(),
                None,
            );
        }

        let mut state = debris_state_to_i32(self.current_state);
        xfer.xfer_int(&mut state).map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.current_state = debris_state_from_i32(state);
        }

        let mut frames = self.state_frame_count as i32;
        xfer.xfer_int(&mut frames).map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.state_frame_count = frames.max(0) as u32;
        }

        xfer.xfer_bool(&mut self.final_stopped)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{DebrisAnimState, W3DDebrisDraw};

    #[test]
    fn should_transition_to_final_when_landed_after_min_frames() {
        assert!(W3DDebrisDraw::should_transition_to_final(
            DebrisAnimState::Flying,
            3,
            false
        ));
    }

    #[test]
    fn should_not_transition_to_final_when_still_above_terrain() {
        assert!(!W3DDebrisDraw::should_transition_to_final(
            DebrisAnimState::Flying,
            10,
            true
        ));
    }
}
