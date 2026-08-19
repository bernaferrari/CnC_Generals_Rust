//! W3DDebrisDraw - Debris particle rendering
//!
//! Port of C++ W3DDebrisDraw.h
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DDebrisDraw.h
//!
//! Renders flying debris from explosions and destruction

use super::draw_module::{DebrisDrawInterface, DrawModule, DrawModuleData, ShadowType};
use crate::common::*;
use crate::effects::FXList;
use crate::helpers::{ModelDrawState, TheGameClient, TheGameLogic};
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
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

pub struct W3DDebrisDraw {
    _data: W3DDebrisDrawModuleData,
    model_name: AsciiString,
    model_color: Color,
    model_created: bool,
    shadow_type: ShadowType,
    anim_initial: AsciiString,
    anim_flying: AsciiString,
    anim_final: AsciiString,
    final_fx: Option<FXList>,
    current_state: DebrisAnimState,
    state_frame_count: u32,
    final_stopped: bool,
    anim_time: Real,
    last_submitted_anim: AsciiString,
    owner_id: Option<ObjectID>,
}

impl W3DDebrisDraw {
    pub fn new(data: W3DDebrisDrawModuleData) -> Self {
        Self {
            _data: data,
            model_name: AsciiString::new(),
            model_color: Color::white(),
            model_created: false,
            shadow_type: ShadowType::None,
            anim_initial: AsciiString::new(),
            anim_flying: AsciiString::new(),
            anim_final: AsciiString::new(),
            final_fx: None,
            current_state: DebrisAnimState::Initial,
            state_frame_count: 0,
            final_stopped: false,
            anim_time: 0.0,
            last_submitted_anim: AsciiString::new(),
            owner_id: None,
        }
    }

    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
    }

    pub fn model_name(&self) -> &AsciiString {
        &self.model_name
    }

    pub fn model_color(&self) -> Color {
        self.model_color
    }

    pub fn shadow_type(&self) -> ShadowType {
        self.shadow_type
    }

    pub fn anim_initial(&self) -> &AsciiString {
        &self.anim_initial
    }

    pub fn anim_flying(&self) -> &AsciiString {
        &self.anim_flying
    }

    pub fn anim_final(&self) -> &AsciiString {
        &self.anim_final
    }

    fn transition_to_flying(&mut self) {
        if self.current_state == DebrisAnimState::Initial {
            self.current_state = DebrisAnimState::Flying;
        }
    }

    fn transition_to_final(&mut self, position: &Coord3D, _transform: &Matrix3D) {
        if self.current_state == DebrisAnimState::Flying {
            self.current_state = DebrisAnimState::Final;

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
        frames: u32,
        is_above_terrain: bool,
    ) -> bool {
        state != DebrisAnimState::Final && frames > MIN_FINAL_FRAMES && !is_above_terrain
    }

    fn is_animation_complete(&self) -> bool {
        let name = self.get_current_animation();
        if name.is_empty() {
            return true;
        }
        self.anim_time >= 1.0
    }

    fn animation_mode(&self) -> i32 {
        match self.current_state {
            DebrisAnimState::Initial => 2,
            DebrisAnimState::Flying => 1,
            DebrisAnimState::Final if self.final_stopped => 0,
            DebrisAnimState::Final => 2,
        }
    }

    fn submit_mesh(&mut self, transform_mtx: &Matrix3D) {
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(client) = TheGameClient::get() else {
            return;
        };
        let mut scale = 1.0;
        if let Some(owner) = TheGameLogic::find_object_by_id(owner_id) {
            if let Ok(owner_guard) = owner.read() {
                if let Some(drawable) = owner_guard.get_drawable() {
                    if let Ok(drawable_guard) = drawable.read() {
                        scale = drawable_guard.get_world_scale().x;
                    }
                }
            }
        }
        let world_transform = if (scale - 1.0).abs() < f32::EPSILON {
            *transform_mtx
        } else {
            Matrix3D::from_scale(glam::Vec3::splat(scale)) * *transform_mtx
        };
        let anim = self.get_current_animation().clone();
        if anim.as_str() != self.last_submitted_anim.as_str() {
            self.anim_time = 0.0;
            self.last_submitted_anim = anim.clone();
        } else if self.animation_mode() != 0 {
            self.anim_time = (self.anim_time + 1.0 / 30.0).min(1.0);
        }
        let color = if self.model_color.to_argb_u32() == 0 {
            None
        } else {
            Some(self.model_color.to_argb_u32() | 0xFF00_0000)
        };
        let state = ModelDrawState {
            source: Default::default(),
            logic_drawable_id: 0,
            model_name: self.model_name.to_string(),
            world_transform,
            render_object_scale: Some(scale),
            render_object_color: color,
            condition_flags_bits: 0,
            bone_overrides: Vec::new(),
            animation_name: if anim.is_empty() {
                None
            } else {
                Some(anim.to_string())
            },
            animation_time: self.anim_time,
            animation_mode: self.animation_mode(),
            mesh_uv_overrides: Vec::new(),
            sub_object_visibility: Vec::new(),
            weapon_bone_bindings: Default::default(),
        };
        client.set_active_object_model_draw(owner_id, state);
    }
}

impl Module for W3DDebrisDraw {
    fn on_drawable_bound_to_object(&mut self) {}
    fn on_delete(&mut self) {}
    fn get_module_name_key(&self) -> NameKeyType {
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("W3DDebrisDraw")
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
        if !self.model_created {
            return;
        }

        let old_state = self.current_state;
        if let Some((is_above_terrain, owner_pos)) = self.owner_terrain_state() {
            if Self::should_transition_to_final(
                self.current_state,
                self.state_frame_count,
                is_above_terrain,
            ) {
                self.transition_to_final(&owner_pos, transform_mtx);
            }
        }
        if self.current_state != DebrisAnimState::Final && self.is_animation_complete() {
            if self.current_state == DebrisAnimState::Initial {
                self.transition_to_flying();
            }
        }
        if self.current_state != old_state {
            self.anim_time = 0.0;
        }
        self.state_frame_count += 1;
        self.submit_mesh(transform_mtx);
    }

    fn set_shadows_enabled(&mut self, enable: bool) {
        let _ = enable;
    }

    fn release_shadows(&mut self) {}
    fn allocate_shadows(&mut self) {}
    fn set_hidden(&mut self, hidden: bool) {
        let _ = hidden;
    }
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        let _ = fully_obscured;
    }
    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
        if !self.model_created {
            return;
        }
        let Some(owner_id) = self.owner_id else {
            return;
        };
        if let Some(owner) = TheGameLogic::find_object_by_id(owner_id) {
            if let Ok(owner_guard) = owner.read() {
                if let Some(drawable) = owner_guard.get_drawable() {
                    if let Ok(drawable_guard) = drawable.read() {
                        let transform = drawable_guard.get_transform_matrix();
                        drop(drawable_guard);
                        self.submit_mesh(&transform);
                    }
                }
            }
        }
    }
    fn react_to_geometry_change(&mut self) {}

    fn get_debris_draw_interface(&self) -> Option<&dyn DebrisDrawInterface> {
        Some(self)
    }

    fn get_debris_draw_interface_mut(&mut self) -> Option<&mut dyn DebrisDrawInterface> {
        Some(self)
    }
}

impl DebrisDrawInterface for W3DDebrisDraw {
    fn set_model_name(&mut self, name: AsciiString, color: Color, shadow_type: ShadowType) {
        if self.model_created || name.is_empty() {
            return;
        }
        self.model_name = name;
        self.model_color = color;
        self.model_created = true;
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
        self.anim_time = 0.0;
        self.last_submitted_anim = AsciiString::new();

        // Store FX list reference (C++ line 150)
        // Matches C++: m_fxFinal = finalFX
        self.final_fx = final_fx.cloned();
    }
}

impl Snapshotable for W3DDebrisDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        let mut draw_module_version: XferVersion = 1;
        xfer.xfer_version(&mut draw_module_version, 1)
            .map_err(|e| e.to_string())?;
        let mut drawable_module_version: XferVersion = 1;
        xfer.xfer_version(&mut drawable_module_version, 1)
            .map_err(|e| e.to_string())?;
        let mut module_version: XferVersion = 1;
        xfer.xfer_version(&mut module_version, 1)
            .map_err(|e| e.to_string())?;

        let mut model_name = self.model_name.as_str().to_string();
        xfer.xfer_ascii_string(&mut model_name)
            .map_err(|e| e.to_string())?;

        let mut packed_color = color_to_packed_i32(self.model_color);
        xfer.xfer_color(&mut packed_color)
            .map_err(|e| e.to_string())?;

        let mut anim_initial = self.anim_initial.as_str().to_string();
        xfer.xfer_ascii_string(&mut anim_initial)
            .map_err(|e| e.to_string())?;

        let mut anim_flying = self.anim_flying.as_str().to_string();
        xfer.xfer_ascii_string(&mut anim_flying)
            .map_err(|e| e.to_string())?;

        let mut anim_final = self.anim_final.as_str().to_string();
        xfer.xfer_ascii_string(&mut anim_final)
            .map_err(|e| e.to_string())?;

        let mut state = debris_state_to_i32(self.current_state);
        xfer.xfer_int(&mut state).map_err(|e| e.to_string())?;

        let mut frames = self.state_frame_count as i32;
        xfer.xfer_int(&mut frames).map_err(|e| e.to_string())?;

        let mut final_stopped = self.final_stopped;
        xfer.xfer_bool(&mut final_stopped)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        // C++ parity: DrawModule::xfer -> DrawableModule::xfer -> Module::xfer
        // Each writes a version(1) byte. Match the 3-byte base class chain.
        let mut draw_module_version: XferVersion = 1;
        xfer.xfer_version(&mut draw_module_version, 1)
            .map_err(|e| e.to_string())?;
        let mut drawable_module_version: XferVersion = 1;
        xfer.xfer_version(&mut drawable_module_version, 1)
            .map_err(|e| e.to_string())?;
        let mut module_version: XferVersion = 1;
        xfer.xfer_version(&mut module_version, 1)
            .map_err(|e| e.to_string())?;

        let mut model_name = self.model_name.as_str().to_string();
        xfer.xfer_ascii_string(&mut model_name)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.model_name = AsciiString::from(model_name.as_str());
            self.model_created = !self.model_name.is_empty();
        }

        let mut packed_color = color_to_packed_i32(self.model_color);
        xfer.xfer_color(&mut packed_color)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.model_color = color_from_packed_i32(packed_color);
        }
        if xfer.is_reading() {
            self.model_created = false;
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
    use super::{DebrisAnimState, W3DDebrisDraw, W3DDebrisDrawModuleData};
    use crate::common::{AsciiString, Color};
    use crate::object::draw::{DebrisDrawInterface, DrawModule, ShadowType};
    use game_engine::common::name_key_generator::NameKeyGenerator;
    use game_engine::common::thing::module::Module;

    #[test]
    fn should_transition_to_final_when_landed_after_min_frames() {
        assert!(W3DDebrisDraw::should_transition_to_final(
            DebrisAnimState::Flying,
            4,
            false
        ));
    }

    #[test]
    fn should_transition_to_final_from_initial_like_cpp() {
        assert!(W3DDebrisDraw::should_transition_to_final(
            DebrisAnimState::Initial,
            4,
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

    #[test]
    fn model_name_is_only_set_once() {
        let mut draw = W3DDebrisDraw::new(W3DDebrisDrawModuleData::new());

        draw.set_model_name(
            AsciiString::from("FIRST"),
            Color::new(1, 2, 3, 4),
            ShadowType::Blob,
        );
        draw.set_model_name(
            AsciiString::from("SECOND"),
            Color::new(5, 6, 7, 8),
            ShadowType::None,
        );

        assert_eq!(draw.model_name.as_str(), "FIRST");
        assert_eq!(draw.shadow_type, ShadowType::Blob);
    }

    #[test]
    fn geometry_and_visibility_hooks_do_not_reset_runtime_state() {
        let mut draw = W3DDebrisDraw::new(W3DDebrisDrawModuleData::new());
        draw.state_frame_count = 9;

        draw.set_hidden(true);
        draw.set_fully_obscured_by_shroud(true);
        draw.set_shadows_enabled(false);
        draw.react_to_geometry_change();

        assert_eq!(draw.state_frame_count, 9);
    }

    #[test]
    fn module_name_key_matches_debris_draw() {
        let draw = W3DDebrisDraw::new(W3DDebrisDrawModuleData::new());
        assert_eq!(
            draw.get_module_name_key(),
            NameKeyGenerator::name_to_key("W3DDebrisDraw")
        );
    }
}
