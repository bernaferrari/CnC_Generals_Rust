use super::*;
use crate::display::image::{ensure_client_mapped_image, get_mapped_image_collection};
use crate::display::view::{Point3, with_tactical_view_ref};
use crate::draw_group_info::get_draw_group_info;
use crate::drawable_info::DrawableInfo;
use crate::gui::display_string::get_display_string_manager;
use crate::gui::font::{FontDesc, get_font_library};
use crate::helpers::TheInGameUI;
use crate::language_filter::get_language_filter;
use crate::render_bridge::get_render_bridge;
use crate::system::TimeOfDay;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::audio::audio_event_rts::AudioEventRts;
use game_engine::common::audio::dynamic_audio_event_info::DynamicAudioEventInfo;
use game_engine::common::audio::game_audio::get_global_audio_manager;
use game_engine::common::bit_flags::{
    ModelConditionBitFlags, ModelConditionFlags, create_model_condition_flags,
};
use game_engine::common::ini::{TimeOfDay as IniTimeOfDay, get_anim2d_collection, get_global_data};
use game_engine::common::system::game_common::WhichTurretType;
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use gamelogic::common::types::{FormationID, INVALID_ID, ObjectID, WeaponSlotType};
use gamelogic::helpers::{BoneOverrideState, ModelDrawState, TheGameClient};
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{NO_HOTKEY_SQUAD, NUM_HOTKEY_SQUADS, Player};
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

    fn fully_obscured_by_shroud(&self) -> Option<bool> {
        Some(BasicDrawable::fully_obscured_by_shroud(self))
    }

    fn scene_effectively_hidden(&self) -> Option<bool> {
        Some(BasicDrawable::is_scene_effectively_hidden(self))
    }

    fn apply_frozen_direct_shroud_status(
        &mut self,
        logic_frame: u32,
        raw_status: gamelogic::common::types::ObjectShroudStatus,
        effectively_dead: bool,
    ) -> Option<crate::drawable::ClientShroudVisibility> {
        Some(BasicDrawable::apply_frozen_direct_shroud_status(
            self,
            logic_frame,
            raw_status,
            effectively_dead,
        ))
    }

    fn evaluate_frozen_direct_scene_candidate(
        &mut self,
        logic_frame: u32,
        raw_status: gamelogic::common::types::ObjectShroudStatus,
        effectively_dead: bool,
    ) -> Option<crate::drawable::SceneShroudDecision> {
        Some(BasicDrawable::evaluate_frozen_direct_scene_candidate(
            self,
            logic_frame,
            raw_status,
            effectively_dead,
        ))
    }

    fn reset_volatile_shroud_state(&mut self) {
        BasicDrawable::reset_volatile_shroud_state(self);
    }

    fn is_selected(&self) -> bool {
        self.selected
    }

    fn set_selected(&mut self, selected: bool) {
        // C++ `friend_setSelected` / `friend_clearSelected`: flash only on
        // the rising edge. `onUnselected` is empty so the envelope decays.
        if !self.selectable {
            if self.selected {
                self.selected = false;
            }
            return;
        }
        if selected {
            if !self.selected {
                self.selected = true;
                self.flash_as_selected(None);
                if let Some(object_id) = self.object_id {
                    self.flash_contained_objects(object_id);
                }
            }
        } else if self.selected {
            self.selected = false;
        }
    }

    fn get_opacity(&self) -> f32 {
        // C++ `getEffectiveOpacity` = explicit * stealth. Detected stealth
        // does not invent a 0.3 first-pass scale; heat-vision is the second
        // material pass. Invisible is handled by `hidden_by_stealth`.
        self.get_effective_opacity()
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
        // C++ `getTintColor` returns only the status/EMP envelope.
        // Selection flash is a separate light add (`getSelectionColor`).
        self.tint_color_effect().unwrap_or(self.tint_color)
    }

    fn set_tint_color(&mut self, color: Vector3) {
        self.tint_color = color;
    }

    fn flash_color(&mut self, color: Vector3, duration_frames: u32) {
        self.color_flash_envelope(Some(color), duration_frames, 0, 0);
    }

    fn set_time_of_day(&self, time_of_day: TimeOfDay) -> Result<(), Box<dyn Error>> {
        // C++ Drawable::setTimeOfDay (`Drawable.cpp:4344-4354`).
        let code = match time_of_day {
            TimeOfDay::Morning => 1,
            TimeOfDay::Afternoon => 2,
            TimeOfDay::Evening => 3,
            TimeOfDay::Night => 4,
        };
        self.pending_time_of_day
            .store(code, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn update(&mut self, _delta_time: f32) {
        self.update_fade();
        self.flush_dirty_model_condition();

        if self.terrain_decal_type != TerrainDecalType::None {
            if self.decal_opacity_fade_rate != 0.0 {
                self.decal_opacity += self.decal_opacity_fade_rate;
                if let Some(dm) = self.draw_modules.first_mut() {
                    dm.set_terrain_decal_opacity(self.decal_opacity);
                }
                if let Some(handle) = &self.terrain_decal_handle {
                    handle.set_opacity((self.decal_opacity.clamp(0.0, 1.0) * 255.0) as i32);
                }
                if self.decal_opacity_fade_rate < 0.0 && self.decal_opacity <= 0.0 {
                    self.decal_opacity_fade_rate = 0.0;
                    self.decal_opacity = 0.0;
                    self.set_terrain_decal(TerrainDecalType::None);
                } else if self.decal_opacity_fade_rate > 0.0 && self.decal_opacity >= 1.0 {
                    self.decal_opacity = 1.0;
                    self.decal_opacity_fade_rate = 0.0;
                    if let Some(dm) = self.draw_modules.first_mut() {
                        dm.set_terrain_decal_opacity(self.decal_opacity);
                    }
                }
            }
            if let Some(handle) = &self.terrain_decal_handle {
                handle.set_position(self.position.x, self.position.y, self.position.z);
            }
        } else {
            self.decal_opacity = 0.0;
        }

        if !self.test_tint_status(TintStatus::FRENZY) {
            let effectively_dead = self.object_id.is_some_and(|obj_id| {
                OBJECT_REGISTRY
                    .get_object(obj_id)
                    .and_then(|obj_arc| {
                        obj_arc.read().ok().map(|guard| guard.is_effectively_dead())
                    })
                    .unwrap_or(false)
            });
            if effectively_dead {
                self.second_material_pass_opacity = 0.0;
            } else if self.second_material_pass_opacity > VERY_TRANSPARENT_MATERIAL_PASS_OPACITY {
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

        // C++ `updateDrawable` ticks both envelopes every frame so EMP/status
        // tints and selection flash actually fade instead of sticking.
        if let Some(envelope) = self.tint_envelope.as_mut() {
            envelope.update();
        }
        if let Some(envelope) = self.selection_flash_envelope.as_mut() {
            envelope.update();
        }

        if let Some(icon_info) = self.icon_info.as_mut() {
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
        self.publish_wheel_info_to_logic();
        self.apply_pending_time_of_day();
        self.restart_ambient_if_dropped();
    }

    fn render(&mut self, view_matrix: &Matrix4, projection_matrix: &Matrix4) {
        if !self.visible
            || self.hidden
            || self.hidden_by_stealth
            || self.drawable_fully_obscured_by_shroud
        {
            return;
        }

        self.flush_dirty_model_condition();

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

        if let Some(object_id) = self.object_id {
            if let Some(wheel) = self.get_wheel_info() {
                if let Some(client) = gamelogic::helpers::TheGameClient::get() {
                    client.set_object_wheel_info(
                        object_id,
                        gamelogic::helpers::DrawWheelInfo {
                            front_left_height_offset: wheel.front_left_height_offset,
                            front_right_height_offset: wheel.front_right_height_offset,
                            rear_left_height_offset: wheel.rear_left_height_offset,
                            rear_right_height_offset: wheel.rear_right_height_offset,
                            wheel_angle: wheel.wheel_angle,
                            frames_airborne: wheel.frames_airborne,
                        },
                    );
                }
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

        // `BasicDrawable::get_transform()` already contains this port's
        // instance matrix and scale. Do not multiply it again here: C++ uses
        // its separate getTransformMatrix()/getInstanceMatrix() values once
        // each, while this Rust accessor deliberately returns their combined
        // presentation representation.
        let world_transform = self.get_transform();

        // Do not approximate C++ `Drawable::applyPhysicsXform` from persisted
        // `LocoInfo` here. The source calculation is gated by object Held,
        // GlobalData, TacticalView/script freeze and the *current* AI
        // locomotor/physics state; rendering can execute more than once per
        // client frame. Applying the raw saved fields here was therefore both
        // observable on ineligible objects and capable of double-applying a
        // guessed transform. The exact client-frame calculation will cache a
        // validated transform from frozen physics input (hq-9vz). Until then,
        // fail closed to the authoritative presentation root transform.

        // Note: DrawModule dispatch is handled by GameLogic::Drawable::draw(), not here.
        // BasicDrawable::render() handles the rendering submission after draw modules
        // have executed. See GameLogic Drawable::draw() at object/drawable.rs:3393.

        let mut tint = self.get_tint_color();
        if let Some(selection) = self.selection_color_effect() {
            tint.x += selection.x;
            tint.y += selection.y;
            tint.z += selection.z;
        }
        let selected = self.is_selected();

        // A C++ Drawable dispatches every DrawModule in order.  Preserve every
        // committed W3D result rather than treating the last one as the whole
        // object.  No committed output retains the template fallback used by
        // simple/non-W3D drawables; an explicit empty committed W3D result is
        // skipped rather than fabricating that fallback model.
        let base_world_transform = world_transform;
        let model_draws = self.model_draw_states();
        let submit_model_draw = |model_draw: Option<&ModelDrawState>| {
            let world_transform = base_world_transform;
            let model_name = match model_draw {
                Some(state) if state.model_name.is_empty() => return,
                Some(state) => state.model_name.clone(),
                None => self.template_name.clone().unwrap_or_default(),
            };

            // The presentation-facing BasicDrawable owns the authoritative
            // frozen object transform (including its existing visual physics
            // pass). The legacy model-draw bridge transports module-local
            // state only; replacing this transform with GameLogic's partially
            // reconstructed Drawable transform would make an approximation
            // look authoritative and lose presentation-side physics fields.

            let mut condition_flags = model_draw
                .map(|state| Self::render_condition_flags_from_bits(state.condition_flags_bits))
                .unwrap_or_else(|| self.compute_render_condition_flags());

            if selected {
                condition_flags |= crate::render_bridge::RenderConditionFlags::SELECTED;
            }

            let bone_overrides = model_draw
                .map(|state| {
                    state
                        .bone_overrides
                        .iter()
                        .map(Self::bone_override_from_model_draw)
                        .collect()
                })
                .unwrap_or_default();
            let mesh_uv_overrides = model_draw
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
            let animation_name = model_draw.and_then(|state| state.animation_name.clone());
            let animation_mode = model_draw
                .and_then(|state| Self::animation_mode_from_model_draw(state.animation_mode));
            let animation_time = model_draw.map(|state| state.animation_time).unwrap_or(0.0);
            let render_state =
                Self::render_state_from_flags(condition_flags, opacity, tint, selected);

            let submission = crate::render_bridge::DrawSubmission {
                drawable_id: crate::render_bridge::DrawableId(self.id.0),
                capture_window_generation: None,
                owner_object_id: self.object_id,
                // Prison/captive visuals are objectless but retain the C++
                // DrawableInfo controller identity. Keep it as a separate
                // immutable bridge fact; Main resolves the controller from
                // its frozen presentation frame.
                shroud_status_object_id: {
                    let object_id = self.shroud_status_object_id();
                    (object_id != 0).then_some(object_id)
                },
                legacy_model_draw_source: model_draw.map(|state| state.source.clone()),
                legacy_weapon_bone_bindings: model_draw
                    .map(|state| state.weapon_bone_bindings.clone()),
                legacy_render_object_transform: model_draw.map(|state| state.world_transform),
                legacy_render_object_scale: model_draw.and_then(|state| state.render_object_scale),
                legacy_render_object_color: model_draw.and_then(|state| state.render_object_color),
                model_name,
                world_transform: world_transform.to_glam(),
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
                        ww3d_core::glam::Vec3::new(
                            self.position.x,
                            self.position.y,
                            self.position.z,
                        ),
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
        };

        if model_draws.is_empty() {
            submit_model_draw(None);
        } else {
            for model_draw in &model_draws {
                submit_model_draw(Some(model_draw));
            }
        }

        // C++ parity: Drawable::draw() iterates draw modules after setting up
        // the world transform. Each draw module renders its portion of the model.
        for dm in &mut self.draw_modules {
            dm.do_draw(&base_world_transform, view_matrix, projection_matrix);
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
        self.set_terrain_decal(decal_type);
    }

    fn set_terrain_decal(&mut self, decal_type: TerrainDecalType) {
        BasicDrawable::set_terrain_decal(self, decal_type);
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

        if drew_anything { Ok(()) } else { Ok(()) }
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
