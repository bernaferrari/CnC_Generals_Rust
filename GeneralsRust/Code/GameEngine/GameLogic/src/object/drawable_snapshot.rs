//! Snapshot/Xfer persistence for drawable state and module payloads.
//!
//! Version ordering and legacy field handling are intentionally unchanged from
//! the source implementation.

use super::*;

impl Snapshot for Drawable {
    fn crc(&self, xfer: &mut dyn Xfer) {
        let mut drawable_id = self.drawable_id;
        let _ = xfer.xfer_unsigned_int(&mut drawable_id);

        let mut object_id = self.object_id;
        let _ = xfer.xfer_object_id(&mut object_id);

        let mut model_conditions = self.model_conditions.bits();
        xfer_u128_bits(xfer, &mut model_conditions);

        let mut hidden = self.hidden;
        let mut hidden_by_stealth = self.hidden_by_stealth;
        let _ = xfer.xfer_bool(&mut hidden);
        let _ = xfer.xfer_bool(&mut hidden_by_stealth);

        let mut tint_status = self.tint_status.0;
        let mut prev_tint_status = self.prev_tint_status.0;
        let _ = xfer.xfer_unsigned_int(&mut tint_status);
        let _ = xfer.xfer_unsigned_int(&mut prev_tint_status);
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let current_version: u8 = 7;
        let mut version = current_version;
        let _ = xfer.xfer_version(&mut version, current_version);

        let xfer_mode = xfer.get_xfer_mode();
        let is_loading = xfer_mode == game_engine::system::XferMode::Load;

        if is_loading {
            self.stop_ambient_sound();
        }

        let mut drawable_id = self.get_drawable_id();
        let _ = xfer.xfer_unsigned_int(&mut drawable_id);
        self.set_drawable_id(drawable_id);

        if version >= 2 {
            let mut condition_state = self.model_conditions;
            xfer_model_condition_flags_legacy(xfer, &mut condition_state);
            self.model_conditions = condition_state;
            if is_loading {
                self.update_conditional_model();
            }
        }

        if version >= 3 {
            if version >= 5 {
                let mut transform = self.transform;
                xfer_matrix3d_legacy(xfer, &mut transform);
                self.set_transform(transform);
            } else {
                let mut position = self.get_position();
                xfer.xfer_coord3d(&mut position);

                let mut orientation = self.world_rotation.y;
                let _ = xfer.xfer_real(&mut orientation);

                if is_loading {
                    let rotation = Quat::from_euler(
                        EulerRot::XYZ,
                        self.world_rotation.x,
                        orientation,
                        self.world_rotation.z,
                    );
                    let transform = Matrix3D::from_scale_rotation_translation(
                        self.world_scale,
                        rotation,
                        position,
                    );
                    self.set_transform(transform);
                }
            }
        }

        let mut has_selection_flash = self.selection_flash_envelope.is_some();
        let _ = xfer.xfer_bool(&mut has_selection_flash);
        if has_selection_flash {
            if self.selection_flash_envelope.is_none() {
                self.selection_flash_envelope = Some(LegacyTintEnvelope::default());
            }
            if let Some(envelope) = self.selection_flash_envelope.as_mut() {
                envelope.xfer(xfer);
            }
        } else if is_loading {
            self.selection_flash_envelope = None;
        }

        let mut has_color_tint = self.color_tint_envelope.is_some();
        let _ = xfer.xfer_bool(&mut has_color_tint);
        if has_color_tint {
            if self.color_tint_envelope.is_none() {
                self.color_tint_envelope = Some(LegacyTintEnvelope::default());
            }
            if let Some(envelope) = self.color_tint_envelope.as_mut() {
                envelope.xfer(xfer);
            }
        } else if is_loading {
            self.color_tint_envelope = None;
        }

        let mut decal_type = terrain_decal_type_to_u32(self.terrain_decal);
        let _ = xfer.xfer_unsigned_int(&mut decal_type);
        if is_loading {
            self.set_terrain_decal(terrain_decal_type_from_u32(decal_type));
        }

        let _ = xfer.xfer_real(&mut self.alpha);
        let _ = xfer.xfer_real(&mut self.stealth_factor);

        let mut effective_stealth_opacity = self.effective_stealth_opacity;
        let _ = xfer.xfer_real(&mut effective_stealth_opacity);
        if is_loading {
            self.effective_stealth_opacity = effective_stealth_opacity.clamp(0.0, 1.0);
        }

        let _ = xfer.xfer_real(&mut self.decal_opacity_fade_target);
        let _ = xfer.xfer_real(&mut self.decal_opacity_fade_rate);
        let _ = xfer.xfer_real(&mut self.decal_opacity);

        let mut object_id = self
            .object_ref
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .and_then(|object| object.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(self.object_id);
        let _ = xfer.xfer_object_id(&mut object_id);

        if is_loading {
            if let Some(bound_object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(bound_guard) = bound_object.read() {
                    if object_id != bound_guard.get_id() {
                        warn!(
                            "Drawable::xfer object link mismatch for drawable {}: stream object {} != bound object {}",
                            self.drawable_id,
                            object_id,
                            bound_guard.get_id()
                        );
                    }
                }
            }

            self.object_id = object_id;
            if self.object_ref.is_none() && object_id != INVALID_ID {
                self.object_ref = TheGameLogic::find_object_by_id(object_id)
                    .map(|object| Arc::downgrade(&object));
            }
        }

        let mut status_bits = self.drawable_status_bits;
        let _ = xfer.xfer_unsigned_int(&mut status_bits);
        self.drawable_status_bits = status_bits;

        let mut tint_status = self.tint_status.0;
        let _ = xfer.xfer_unsigned_int(&mut tint_status);
        self.tint_status = TintStatus(tint_status);

        let mut prev_tint_status = self.prev_tint_status.0;
        let _ = xfer.xfer_unsigned_int(&mut prev_tint_status);
        self.prev_tint_status = TintStatus(prev_tint_status);

        let _ = xfer.xfer_unsigned_int(&mut self.fade_mode);
        let _ = xfer.xfer_unsigned_int(&mut self.time_elapsed_fade);
        let _ = xfer.xfer_unsigned_int(&mut self.time_to_fade);

        let mut has_loco_info = self.loco_info.is_some();
        let _ = xfer.xfer_bool(&mut has_loco_info);
        if has_loco_info {
            if self.loco_info.is_none() {
                self.loco_info = Some(LegacyDrawableLocoInfo::default());
            }
            if let Some(loco) = self.loco_info.as_mut() {
                loco.xfer(xfer);
            }
        } else if is_loading {
            self.loco_info = None;
        }

        self.xfer_drawable_modules(xfer);
        if is_loading {
            // Xfer reconstructs the object link and module list separately.
            // Route the completed association through the same post-bind
            // notification used by C++ `friend_bindToObject`, otherwise W3D
            // modules can retain an INVALID owner/current state after load.
            self.notify_draw_modules_bound_to_current_object();
        }

        let mut stealth_look = stealth_look_to_u32(self.stealth_look);
        let _ = xfer.xfer_unsigned_int(&mut stealth_look);
        if is_loading {
            self.stealth_look = stealth_look_from_u32(stealth_look);
        }

        let _ = xfer.xfer_int(&mut self.flash_count);
        let mut flash_color = self.flash_color.to_argb_u32() as i32;
        let _ = xfer.xfer_color(&mut flash_color);
        if is_loading {
            self.flash_color = color_from_argb_u32(flash_color as u32);
        }

        let _ = xfer.xfer_bool(&mut self.hidden);
        let _ = xfer.xfer_bool(&mut self.hidden_by_stealth);

        let _ = xfer.xfer_real(&mut self.second_material_pass_opacity);

        let mut instance_is_identity = self.instance_matrix.is_none();
        let _ = xfer.xfer_bool(&mut instance_is_identity);

        let mut instance_matrix = self.instance_matrix.unwrap_or(Matrix3D::IDENTITY);
        xfer_matrix3d_user_legacy(xfer, &mut instance_matrix);

        let mut instance_scale = self.instance_scale;
        let _ = xfer.xfer_real(&mut instance_scale);

        if is_loading {
            self.instance_matrix = if instance_is_identity {
                None
            } else {
                Some(instance_matrix)
            };
            self.instance_scale = instance_scale;
        }

        let _ = xfer.xfer_object_id(&mut self.shroud_status_object_id);

        if version < 2 {
            let mut condition_state = self.model_conditions;
            xfer_model_condition_flags_legacy(xfer, &mut condition_state);
            self.model_conditions = condition_state;
            if is_loading {
                self.update_conditional_model();
            }
        }

        let _ = xfer.xfer_unsigned_int(&mut self.expiration_date);

        let mut icon_count = self.legacy_icons.len().min(u8::MAX as usize) as u8;
        let _ = xfer.xfer_unsigned_byte(&mut icon_count);
        if xfer_mode == game_engine::system::XferMode::Load {
            self.legacy_icons.clear();
            for _ in 0..icon_count {
                let mut icon = LegacyDrawableIcon::default();
                let _ = xfer.xfer_ascii_string(&mut icon.icon_index_name);
                let _ = xfer.xfer_unsigned_int(&mut icon.keep_till_frame);
                let _ = xfer.xfer_ascii_string(&mut icon.icon_template_name);
                icon.icon_state.xfer(xfer);
                self.legacy_icons.push(icon);
            }
        } else {
            for icon in self.legacy_icons.iter_mut().take(icon_count as usize) {
                let _ = xfer.xfer_ascii_string(&mut icon.icon_index_name);
                let _ = xfer.xfer_unsigned_int(&mut icon.keep_till_frame);
                let _ = xfer.xfer_ascii_string(&mut icon.icon_template_name);
                icon.icon_state.xfer(xfer);
            }
        }

        if version >= 4 {
            let _ = xfer.xfer_bool(&mut self.ambient_sound_enabled);
        }

        if version >= 6 {
            let _ = xfer.xfer_bool(&mut self.ambient_sound_enabled_from_script);
        }

        if version >= 7 {
            let mut customized = self.custom_sound_ambient_info.is_some()
                || self.custom_sound_ambient_dynamic_info.is_some()
                || self.custom_sound_ambient_off;
            let _ = xfer.xfer_bool(&mut customized);

            if customized {
                let mut customized_to_silence = self.custom_sound_ambient_off;
                let _ = xfer.xfer_bool(&mut customized_to_silence);

                if is_loading {
                    if customized_to_silence {
                        self.set_custom_sound_ambient_off();
                    } else {
                        let mut base_info_name = String::new();
                        let _ = xfer.xfer_ascii_string(&mut base_info_name);

                        let manager = get_global_audio_manager()
                            .unwrap_or_else(initialize_global_audio_manager);
                        let (mut customized_info, successful_load) = match manager.lock() {
                            Ok(guard) => {
                                if let Some(base_info) =
                                    guard.find_audio_event_info(&base_info_name)
                                {
                                    (DynamicAudioEventInfo::from_base_info(&base_info), true)
                                } else {
                                    warn!(
                                        "Drawable load: missing base ambient sound '{}'; discarding custom overrides",
                                        base_info_name
                                    );
                                    (DynamicAudioEventInfo::new(), false)
                                }
                            }
                            Err(_) => (DynamicAudioEventInfo::new(), false),
                        };

                        let custom_name = self
                            .mangle_custom_audio_name(&customized_info.audio_event_info.audio_name);
                        customized_info.override_audio_name(&custom_name);
                        let _ = customized_info.xfer_no_name(xfer);

                        if successful_load {
                            self.set_custom_sound_ambient_dynamic_info_internal(
                                customized_info,
                                false,
                            );
                        } else {
                            self.clear_custom_sound_ambient(false);
                            self.custom_sound_ambient_off = false;
                        }
                    }
                } else if !customized_to_silence {
                    let mut base_info_name = self
                        .custom_sound_ambient_dynamic_info
                        .as_ref()
                        .map(|info| info.get_original_name().to_string())
                        .or_else(|| {
                            self.custom_sound_ambient_info
                                .as_ref()
                                .map(|info| info.audio_name.clone())
                        })
                        .unwrap_or_default();
                    let _ = xfer.xfer_ascii_string(&mut base_info_name);

                    if let Some(customized_info) = self.custom_sound_ambient_dynamic_info.as_mut() {
                        let _ = customized_info.xfer_no_name(xfer);
                    } else if let Some(info) = &self.custom_sound_ambient_info {
                        let mut fallback = DynamicAudioEventInfo::from_base_info(info.as_ref());
                        let _ = fallback.xfer_no_name(xfer);
                    }
                }
            } else if is_loading {
                self.custom_sound_ambient_off = false;
                self.custom_sound_ambient_info = None;
                self.custom_sound_ambient_dynamic_info = None;
            }
        }

        if is_loading {
            // C++ parity: do not trust serialized stealth look; StealthUpdate will
            // re-drive the correct state on the next logic update.
            self.stealth_look = StealthLookType::None;
            if self.hidden || self.hidden_by_stealth {
                self.update_hidden_status();
            }
        }
    }

    fn load_post_process(&mut self) {
        if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
            if let Ok(object_guard) = object.read() {
                self.object_id = object_guard.get_id();
                self.set_transform(object_guard.get_transform_matrix());
            }
        }

        if self.ambient_sound_enabled && self.ambient_sound_enabled_from_script {
            if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(object_guard) = object.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|data| data.get_time_of_day())
                        .unwrap_or(TimeOfDay::Day);
                    self.start_ambient_sound_internal(&object_guard, time_of_day, true);
                } else {
                    self.stop_ambient_sound();
                }
            } else {
                self.stop_ambient_sound();
            }
        } else {
            self.stop_ambient_sound();
        }
    }
}
