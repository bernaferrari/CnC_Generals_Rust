use super::*;
use crate::display::image::{ensure_client_mapped_image, get_mapped_image_collection};
use crate::display::view::{with_tactical_view_ref, Point3};
use crate::draw_group_info::get_draw_group_info;
use crate::drawable_info::DrawableInfo;
use crate::gui::display_string::get_display_string_manager;
use crate::gui::font::{get_font_library, FontDesc};
use crate::helpers::TheInGameUI;
use crate::language_filter::get_language_filter;
use crate::render_bridge::get_render_bridge;
use crate::system::TimeOfDay;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::audio::audio_event_rts::AudioEventRts;
use game_engine::common::audio::dynamic_audio_event_info::DynamicAudioEventInfo;
use game_engine::common::audio::game_audio::get_global_audio_manager;
use game_engine::common::bit_flags::{
    create_model_condition_flags, ModelConditionBitFlags, ModelConditionFlags,
};
use game_engine::common::ini::{get_anim2d_collection, get_global_data, TimeOfDay as IniTimeOfDay};
use game_engine::common::system::game_common::WhichTurretType;
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use gamelogic::common::types::{FormationID, ObjectID, WeaponSlotType, INVALID_ID};
use gamelogic::helpers::{BoneOverrideState, ModelDrawState, TheGameClient};
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{Player, NO_HOTKEY_SQUAD, NUM_HOTKEY_SQUADS};
use parking_lot::Mutex;
use std::error::Error;
use std::sync::Arc;

impl Drawable for BasicDrawable {
    fn get_id(&self) -> DrawableId {
        self.id
    }

    fn set_id(&mut self, id: DrawableId) {
        self.id = id;
        self.drawable_info.set_drawable_id(id.0);
    }

    fn get_object_id(&self) -> Option<u32> {
        self.object_id
    }

    fn set_object_id(&mut self, object_id: Option<u32>) {
        BasicDrawable::set_object_id(self, object_id);
    }

    fn get_template_name(&self) -> Option<&str> {
        self.template_name.as_deref()
    }

    fn set_template_name(&mut self, name: Option<String>) {
        self.template_name = name;
    }

    fn get_position(&self) -> Vector3 {
        self.position
    }

    fn set_position(&mut self, position: Vector3) {
        self.position = position;
    }

    fn get_transform(&self) -> Matrix4 {
        // Combine position, scale, and instance transform
        let translation = Matrix4::translation(self.position);
        let scale = Matrix4::scale(self.instance_scale);
        translation.mul(&self.instance_transform).mul(&scale)
    }

    fn set_instance_transform(&mut self, transform: Matrix4) {
        self.instance_transform = transform;
    }

    fn is_instance_identity(&self) -> bool {
        self.instance_transform == Matrix4::identity()
    }

    fn get_instance_scale(&self) -> f32 {
        self.instance_scale
    }

    fn set_instance_scale(&mut self, scale: f32) {
        self.instance_scale = scale;
    }

    fn get_status(&self) -> DrawableStatus {
        self.status
    }

    fn set_status(&mut self, status: DrawableStatus) {
        self.status = status;
    }

    fn is_visible(&self) -> bool {
        self.visible
            && !self.hidden
            && !self.hidden_by_stealth
            && !matches!(self.stealth_look, StealthLook::Invisible)
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        BasicDrawable::set_fully_obscured_by_shroud(self, fully_obscured);
    }

    fn is_selected(&self) -> bool {
        self.selected
    }

    fn set_selected(&mut self, selected: bool) {
        if !self.selectable {
            self.selected = false;
        } else {
            self.selected = selected;
        }

        if selected {
            // Start selection flash effect (matches C++ flashAsSelected)
            if self.selection_flash_envelope.is_none() {
                self.selection_flash_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.selection_flash_envelope {
                envelope.play(Vector3::new(0.3, 0.3, 0.3), 5, 10, 0);
            }

            // Flash contained objects if this drawable has a bound object
            // Matches C++ Drawable::onSelected() calling contain->clientVisibleContainedFlashAsSelected()
            if let Some(object_id) = self.object_id {
                self.flash_contained_objects(object_id);
            }
        } else {
            // C++ onUnselected() is empty but we clear the flash envelope
            self.selection_flash_envelope = None;
        }
    }

    fn get_opacity(&self) -> f32 {
        match self.stealth_look {
            StealthLook::Invisible => 0.0,
            StealthLook::VisibleDetected => self.opacity * 0.3,
            _ => (self.opacity * self.effective_stealth_opacity).clamp(0.0, 1.0),
        }
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
        self.explicit_opacity = self.opacity;
    }

    fn get_stealth_look(&self) -> StealthLook {
        self.stealth_look
    }

    fn set_stealth_look(&mut self, stealth_look: StealthLook) {
        self.apply_stealth_look(stealth_look);
    }

    fn draw_icon_ui(&mut self) {
        BasicDrawable::draw_icon_ui(self);
    }

    fn get_tint_color(&self) -> Vector3 {
        let mut color = self.tint_color;

        // Add tint envelope effects
        if let Some(ref envelope) = self.tint_envelope {
            if envelope.is_effective {
                color.x += envelope.current_color.x;
                color.y += envelope.current_color.y;
                color.z += envelope.current_color.z;
            }
        }

        // Add selection flash effect
        if let Some(ref envelope) = self.selection_flash_envelope {
            if envelope.is_effective {
                color.x += envelope.current_color.x;
                color.y += envelope.current_color.y;
                color.z += envelope.current_color.z;
            }
        }

        color
    }

    fn set_tint_color(&mut self, color: Vector3) {
        self.tint_color = color;
    }

    fn flash_color(&mut self, color: Vector3, duration_frames: u32) {
        self.color_flash_envelope(Some(color), duration_frames, 0, 0);
    }

    fn update(&mut self, _delta_time: f32) {
        self.update_fade();

        if self.terrain_decal_type != TerrainDecalType::None {
            if self.decal_opacity_fade_rate != 0.0 {
                self.decal_opacity += self.decal_opacity_fade_rate;
                if self.decal_opacity_fade_rate < 0.0 && self.decal_opacity <= 0.0 {
                    self.decal_opacity_fade_rate = 0.0;
                    self.decal_opacity = 0.0;
                    self.terrain_decal_type = TerrainDecalType::None;
                } else if self.decal_opacity_fade_rate > 0.0 && self.decal_opacity >= 1.0 {
                    self.decal_opacity = 1.0;
                    self.decal_opacity_fade_rate = 0.0;
                }
            }
        } else {
            self.decal_opacity = 0.0;
        }

        if !self.test_tint_status(TintStatus::FRENZY) {
            if self.second_material_pass_opacity > VERY_TRANSPARENT_MATERIAL_PASS_OPACITY {
                self.second_material_pass_opacity *= MATERIAL_PASS_OPACITY_FADE_SCALAR;
            } else {
                self.second_material_pass_opacity = 0.0;
            }
        }
        self.overlay_data.second_material_pass_opacity = self.second_material_pass_opacity;

        if self.flash_count > 0 && self.current_frame.is_multiple_of(DRAWABLE_FRAMES_PER_FLASH) {
            self.color_flash_envelope(Some(self.flash_color), DEF_DECAY_FRAMES, 0, 0);
            self.flash_count = self.flash_count.saturating_sub(1);
        }

        self.update_tint_status();

        // Update tint envelopes
        if let Some(ref mut envelope) = self.tint_envelope {
            envelope.update();
        }
        if let Some(ref mut envelope) = self.selection_flash_envelope {
            envelope.update();
        }

        // Update icon info
        if let Some(ref mut icon_info) = self.icon_info {
            icon_info.update(self.current_frame);
        }

        // C++ parity: Drawable::updateDrawable() dispatches to all ClientUpdateModules.
        if let Some(object_id) = self.object_id {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(obj_guard) = obj_arc.read() {
                    for module_handle in obj_guard.client_update_modules() {
                        module_handle.with_module(|module| {
                            if let Some(client_update) = module.get_client_update_interface() {
                                let _ = client_update.client_update();
                            }
                        });
                    }
                }
            }
        }
    }

    fn render(&mut self, view_matrix: &Matrix4, projection_matrix: &Matrix4) {
        if !self.visible
            || self.hidden
            || self.hidden_by_stealth
            || self.drawable_fully_obscured_by_shroud
        {
            return;
        }

        // C++ parity: Drawable::draw() (Drawable.cpp:2629-2630):
        // if (getObject() && !getObject()->isEffectivelyDead())
        //     setShadowsEnabled(m_stealthLook != STEALTHLOOK_VISIBLE_DETECTED);
        // Without a bound object we keep the create-time enable (seeded SHADOWS).
        if let Some(obj_id) = self.object_id {
            let effectively_dead = OBJECT_REGISTRY
                .get_object(obj_id)
                .and_then(|obj_arc| obj_arc.read().ok().map(|guard| guard.is_effectively_dead()))
                .unwrap_or(false);
            if !effectively_dead {
                self.set_shadows_enabled(self.stealth_look != StealthLook::VisibleDetected);
            }
        }

        // C++ parity: Drawable::draw() validates position (Drawable.cpp:2634 validatePos()).
        // Skip rendering if position contains NaN or is unreasonably large.
        let pos = &self.position;
        if pos.x.is_nan()
            || pos.y.is_nan()
            || pos.z.is_nan()
            || pos.x.abs() > 10000.0
            || pos.y.abs() > 10000.0
            || pos.z.abs() > 10000.0
        {
            return;
        }

        let opacity = self.get_opacity();
        if opacity <= 0.0 {
            return;
        }

        // C++ parity: Drawable::draw() builds transform from getTransformMatrix() *
        // getInstanceMatrix(), then applies physics xform before draw module dispatch.
        let mut world_transform = self.get_transform();
        if !self.is_instance_identity() {
            let instance = self.instance_transform;
            world_transform = world_transform.mul(&instance);
        }

        // C++ parity: applyPhysicsXform(&transformMtx) at Drawable.cpp:2649.
        // Uses locomotor-derived pitch/roll/yaw/overlap_z from LocoInfo to apply
        // visual physics transforms (vehicle tilt, hover bob, etc.).
        if let Some(ref loco) = self.loco_info {
            let total_pitch = snap_denorm(loco.pitch);
            let total_roll = snap_denorm(loco.roll);
            let total_yaw = snap_denorm(loco.yaw);
            let total_z = snap_denorm(loco.overlap_z);

            let physics_xform = Matrix4::translation(Vector3::new(0.0, 0.0, total_z))
                .mul(&Matrix4::rotation_y(total_pitch))
                .mul(&Matrix4::rotation_x(-total_roll))
                .mul(&Matrix4::rotation_z(total_yaw));
            world_transform = world_transform.mul(&physics_xform);
        }

        // Note: DrawModule dispatch is handled by GameLogic::Drawable::draw(), not here.
        // BasicDrawable::render() handles the rendering submission after draw modules
        // have executed. See GameLogic Drawable::draw() at object/drawable.rs:3393.

        let tint = self.get_tint_color();
        let selected = self.is_selected();

        let model_draw = self.model_draw_state();

        let model_name = model_draw
            .as_ref()
            .map(|state| state.model_name.clone())
            .filter(|name| !name.is_empty())
            .or_else(|| self.template_name.clone())
            .unwrap_or_default();

        if let Some(model_draw) = model_draw.as_ref() {
            world_transform = Self::matrix4_from_model_draw(model_draw.world_transform);
        }

        let mut condition_flags = model_draw
            .as_ref()
            .map(|state| Self::render_condition_flags_from_bits(state.condition_flags_bits))
            .unwrap_or_else(|| self.compute_render_condition_flags());

        if selected {
            condition_flags |= crate::render_bridge::RenderConditionFlags::SELECTED;
        }

        let bone_overrides = model_draw
            .as_ref()
            .map(|state| {
                state
                    .bone_overrides
                    .iter()
                    .map(Self::bone_override_from_model_draw)
                    .collect()
            })
            .unwrap_or_default();
        let mesh_uv_overrides = model_draw
            .as_ref()
            .map(|state| {
                state
                    .mesh_uv_overrides
                    .iter()
                    .map(|uv| crate::render_bridge::MeshUvOverride {
                        mesh_name_prefix: uv.mesh_name_prefix.clone(),
                        u_offset: uv.u_offset,
                        v_offset: uv.v_offset,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let sub_object_visibility = model_draw
            .as_ref()
            .map(|state| {
                state
                    .sub_object_visibility
                    .iter()
                    .map(|visibility| crate::render_bridge::SubObjectVisibility {
                        sub_object_name: visibility.sub_object_name.clone(),
                        hidden: visibility.hidden,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let animation_name = model_draw
            .as_ref()
            .and_then(|state| state.animation_name.clone());
        let animation_mode = model_draw
            .as_ref()
            .and_then(|state| Self::animation_mode_from_model_draw(state.animation_mode));
        let animation_time = model_draw
            .as_ref()
            .map(|state| state.animation_time)
            .unwrap_or(0.0);
        let render_state = Self::render_state_from_flags(condition_flags, opacity, tint, selected);

        let submission = crate::render_bridge::DrawSubmission {
            drawable_id: crate::render_bridge::DrawableId(self.id.0),
            model_name,
            world_transform: glam::Mat4::from_cols_array_2d(&world_transform.elements),
            condition_flags,
            render_state: render_state.clone(),
            bone_overrides,
            mesh_uv_overrides,
            sub_object_visibility,
            animation_name,
            animation_mode,
            animation_time,
            bounding_sphere: {
                let (_, radius) = self.get_bounding_sphere();
                ww3d_core::BoundingSphere::new(
                    ww3d_core::glam::Vec3::new(self.position.x, self.position.y, self.position.z),
                    radius,
                )
            },
            bounding_box: ww3d_core::AABox::zero(),
            sort_level: 0,
            opaque: render_state.opacity >= 1.0,
            transparent: render_state.opacity < 1.0,
            cast_shadow: self.status.has(DrawableStatus::SHADOWS),
        };

        if let Ok(mut bridge_guard) = get_render_bridge().lock() {
            if let Some(bridge) = bridge_guard.as_mut() {
                bridge.submit(submission);
            }
        }

        // C++ parity: Drawable::draw() iterates draw modules after setting up
        // the world transform. Each draw module renders its portion of the model.
        for dm in &mut self.draw_modules {
            dm.do_draw(&world_transform, view_matrix, projection_matrix);
        }
    }

    fn get_bounding_sphere(&self) -> (Vector3, f32) {
        (self.position, 1.0) // Default 1.0 unit radius
    }

    fn receives_dynamic_lights(&self) -> bool {
        self.receives_dynamic_lights
    }

    fn set_receives_dynamic_lights(&mut self, receives: bool) {
        self.receives_dynamic_lights = receives;
    }

    fn get_terrain_decal_type(&self) -> TerrainDecalType {
        self.terrain_decal_type
    }

    fn set_terrain_decal_type(&mut self, decal_type: TerrainDecalType) {
        self.terrain_decal_type = decal_type;
    }

    fn draw_ui_text(&self) -> Result<(), Box<dyn Error>> {
        // Wave 980: host empty dual-world → drawable pose + presentation team color residual.
        if dual_world_registry_unavailable() {
            return self.draw_ui_text_from_presentation();
        }

        let Some(object_id) = self.object_id else {
            return Ok(());
        };

        let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Ok(());
        };
        let Ok(object_guard) = object_arc.read() else {
            return Ok(());
        };

        let Some(screen_pos) = with_tactical_view_ref(|view| {
            view.world_to_screen(&Point3::new(
                object_guard.get_position().x,
                object_guard.get_position().y,
                object_guard.get_position().z,
            ))
        }) else {
            return Ok(());
        };

        let draw_group_info = get_draw_group_info()
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();

        let mut text_color = draw_group_info.color_for_text;
        if draw_group_info.use_player_color {
            if let Some(player_arc) = object_guard.get_controlling_player() {
                if let Ok(player_guard) = player_arc.read() {
                    text_color = player_guard.get_player_color().to_argb_u32();
                }
            }
        }

        let anchor_width = 32.0_f32;
        let anchor_height = 32.0_f32;
        let base_x = if draw_group_info.using_pixel_offset_x {
            screen_pos.x + draw_group_info.pixel_offset_x
        } else {
            screen_pos.x + (anchor_width * draw_group_info.percent_offset_x) as i32
        };
        let base_y = if draw_group_info.using_pixel_offset_y {
            screen_pos.y + draw_group_info.pixel_offset_y
        } else {
            screen_pos.y + (anchor_height * draw_group_info.percent_offset_y) as i32
        };

        let mut drew_anything = false;

        if let Some(player_arc) = object_guard.get_controlling_player() {
            if let Ok(mut player_guard) = player_arc.write() {
                if let Some(group_number) =
                    Self::find_hotkey_squad_number(&mut player_guard, object_guard.get_id())
                {
                    if group_number > NO_HOTKEY_SQUAD && group_number < NUM_HOTKEY_SQUADS as i32 {
                        let mut manager = get_display_string_manager();
                        if let Some(group_text) = manager.get_group_numeral_string(group_number) {
                            Self::draw_caption_string(
                                &group_text,
                                base_x,
                                base_y,
                                text_color,
                                draw_group_info.color_for_text_drop_shadow,
                                &draw_group_info.font_name,
                                draw_group_info.font_size,
                                draw_group_info.font_is_bold,
                                draw_group_info.drop_shadow_offset_x,
                                draw_group_info.drop_shadow_offset_y,
                            );
                            drew_anything = true;
                        }
                    }
                }
            }
        }

        if object_guard.get_formation_id() != FormationID::NONE {
            let mut manager = get_display_string_manager();
            if let Some(formation_text) = manager.get_formation_letter_string() {
                Self::draw_caption_string(
                    &formation_text,
                    base_x + 10,
                    base_y,
                    text_color,
                    draw_group_info.color_for_text_drop_shadow,
                    &draw_group_info.font_name,
                    draw_group_info.font_size,
                    draw_group_info.font_is_bold,
                    draw_group_info.drop_shadow_offset_x,
                    draw_group_info.drop_shadow_offset_y,
                );
                drew_anything = true;
            }
        }

        if drew_anything {
            Ok(())
        } else {
            Ok(())
        }
    }

    fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    fn is_expired(&self, current_frame: u32) -> bool {
        self.expiration_frame
            .is_some_and(|frame| current_frame >= frame)
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}
