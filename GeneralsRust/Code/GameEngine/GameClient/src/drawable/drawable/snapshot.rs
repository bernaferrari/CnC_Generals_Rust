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

impl Snapshotable for BasicDrawable {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // PARITY_NOTE: C++ Drawable::crc (Drawable.cpp line 4757) is intentionally empty.
        // Rust performs a full field CRC for deep verification, which is a strict superset.
        let mut id = self.id.0;
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|e| format!("{:?}", e))?;

        let mut flags = self.model_condition_flags.clone();
        xfer_model_condition_flags(xfer, &mut flags)?;

        let mut transform = Matrix4::translation(self.position).mul(&self.instance_transform);
        xfer_matrix3d(xfer, &mut transform)?;

        let mut has_selection_flash = self.selection_flash_envelope.is_some();
        xfer.xfer_bool(&mut has_selection_flash)
            .map_err(|e| format!("{:?}", e))?;
        if has_selection_flash {
            if let Some(ref envelope) = self.selection_flash_envelope {
                Snapshotable::crc(envelope, xfer)?;
            }
        }

        let mut has_tint_envelope = self.tint_envelope.is_some();
        xfer.xfer_bool(&mut has_tint_envelope)
            .map_err(|e| format!("{:?}", e))?;
        if has_tint_envelope {
            if let Some(ref envelope) = self.tint_envelope {
                Snapshotable::crc(envelope, xfer)?;
            }
        }

        let mut decal_type = terrain_decal_to_u32(self.terrain_decal_type);
        xfer.xfer_unsigned_int(&mut decal_type)
            .map_err(|e| format!("{:?}", e))?;

        let mut explicit_opacity = self.explicit_opacity;
        xfer.xfer_real(&mut explicit_opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut stealth_opacity = self.stealth_opacity;
        xfer.xfer_real(&mut stealth_opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut effective_stealth_opacity = self.effective_stealth_opacity;
        xfer.xfer_real(&mut effective_stealth_opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut decal_opacity_fade_target = self.decal_opacity_fade_target;
        xfer.xfer_real(&mut decal_opacity_fade_target)
            .map_err(|e| format!("{:?}", e))?;

        let mut decal_opacity_fade_rate = self.decal_opacity_fade_rate;
        xfer.xfer_real(&mut decal_opacity_fade_rate)
            .map_err(|e| format!("{:?}", e))?;

        let mut decal_opacity = self.decal_opacity;
        xfer.xfer_real(&mut decal_opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut object_id = self.object_id.unwrap_or(0);
        xfer.xfer_unsigned_int(&mut object_id)
            .map_err(|e| format!("{:?}", e))?;

        let mut status_bits = self.status.bits;
        xfer.xfer_unsigned_int(&mut status_bits)
            .map_err(|e| format!("{:?}", e))?;

        let mut tint_status_bits = self.tint_status.bits;
        xfer.xfer_unsigned_int(&mut tint_status_bits)
            .map_err(|e| format!("{:?}", e))?;

        let mut prev_tint_status_bits = self.prev_tint_status.bits;
        xfer.xfer_unsigned_int(&mut prev_tint_status_bits)
            .map_err(|e| format!("{:?}", e))?;

        let mut fade_mode = fading_mode_to_u32(self.fade_mode);
        xfer.xfer_unsigned_int(&mut fade_mode)
            .map_err(|e| format!("{:?}", e))?;

        let mut time_elapsed_fade = self.time_elapsed_fade;
        xfer.xfer_unsigned_int(&mut time_elapsed_fade)
            .map_err(|e| format!("{:?}", e))?;

        let mut time_to_fade = self.time_to_fade;
        xfer.xfer_unsigned_int(&mut time_to_fade)
            .map_err(|e| format!("{:?}", e))?;

        let mut has_loco_info = self.loco_info.is_some();
        xfer.xfer_bool(&mut has_loco_info)
            .map_err(|e| format!("{:?}", e))?;
        if has_loco_info {
            if let Some(ref loco_info) = self.loco_info {
                Snapshotable::crc(loco_info, xfer)?;
            }
        }

        let mut stealth_look = stealth_look_to_u32(self.stealth_look);
        xfer.xfer_unsigned_int(&mut stealth_look)
            .map_err(|e| format!("{:?}", e))?;

        let mut flash_count = self.flash_count as i32;
        xfer.xfer_int(&mut flash_count)
            .map_err(|e| format!("{:?}", e))?;

        let mut flash_color_bits = vector3_to_color_bits(self.flash_color);
        xfer.xfer_int(&mut flash_color_bits)
            .map_err(|e| format!("{:?}", e))?;

        let mut hidden = self.hidden;
        xfer.xfer_bool(&mut hidden)
            .map_err(|e| format!("{:?}", e))?;

        let mut hidden_by_stealth = self.hidden_by_stealth;
        xfer.xfer_bool(&mut hidden_by_stealth)
            .map_err(|e| format!("{:?}", e))?;

        let mut second_material_pass_opacity = self.second_material_pass_opacity;
        xfer.xfer_real(&mut second_material_pass_opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut instance_is_identity = self.is_instance_identity();
        xfer.xfer_bool(&mut instance_is_identity)
            .map_err(|e| format!("{:?}", e))?;

        let mut instance_scale = self.instance_scale;
        xfer.xfer_real(&mut instance_scale)
            .map_err(|e| format!("{:?}", e))?;

        let mut expiration = self.expiration_frame.unwrap_or(0);
        xfer.xfer_unsigned_int(&mut expiration)
            .map_err(|e| format!("{:?}", e))?;

        let mut has_icon_info = self.icon_info.is_some();
        xfer.xfer_bool(&mut has_icon_info)
            .map_err(|e| format!("{:?}", e))?;
        if has_icon_info {
            if let Some(ref icon_info) = self.icon_info {
                Snapshotable::crc(icon_info, xfer)?;
            }
        }

        let mut visible = self.visible;
        xfer.xfer_bool(&mut visible)
            .map_err(|e| format!("{:?}", e))?;

        let mut selected = self.selected;
        xfer.xfer_bool(&mut selected)
            .map_err(|e| format!("{:?}", e))?;

        let mut selectable = self.selectable;
        xfer.xfer_bool(&mut selectable)
            .map_err(|e| format!("{:?}", e))?;

        let mut opacity = self.opacity;
        xfer.xfer_real(&mut opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut tint_color = self.tint_color;
        xfer_vector3(xfer, &mut tint_color)?;

        let mut receives_dynamic_lights = self.receives_dynamic_lights;
        xfer.xfer_bool(&mut receives_dynamic_lights)
            .map_err(|e| format!("{:?}", e))?;

        let mut terrain_decal_size = self.terrain_decal_size;
        xfer_vector3(xfer, &mut terrain_decal_size)?;

        let mut current_frame = self.current_frame;
        xfer.xfer_unsigned_int(&mut current_frame)
            .map_err(|e| format!("{:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // PARITY_NOTE: C++ Drawable::xfer is at version 7 (Drawable.cpp line 4900).
        // Rust version 3 adds object_id, drawable module stub, and instance_is_identity.
        // Rust version 4 adds the instance matrix after instance_is_identity.
        // Rust version 5 adds DrawableInfo shroud status object id.
        // Rust version 6 stores icons in C++ layout: count byte followed by entries.
        // Rust version 7 adds the C++ ambient sound tail and stops writing Rust-only tail fields.
        const CURRENT_VERSION: XferVersion = 7;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        // --- drawable id (C++ line 4919: xferDrawableID) ---
        let mut id = self.id.0;
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|e| format!("{:?}", e))?;
        self.id = DrawableId(id);
        self.drawable_info.set_drawable_id(id);

        // --- condition state (C++ version >= 2, line 4924) ---
        if version >= 2 {
            let mut flags = self.model_condition_flags.clone();
            xfer_model_condition_flags(xfer, &mut flags)?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.replace_model_condition_flags(flags, true);
            }
        }

        // --- transform (C++ version >= 5: xferMatrix3D, line 4935) ---
        let mut transform = Matrix4::translation(self.position).mul(&self.instance_transform);
        xfer_matrix3d(xfer, &mut transform)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.position = Vector3::new(
                transform.elements[0][3],
                transform.elements[1][3],
                transform.elements[2][3],
            );
            transform.elements[0][3] = 0.0;
            transform.elements[1][3] = 0.0;
            transform.elements[2][3] = 0.0;
            self.instance_transform = transform;
        }

        // --- selection flash envelope (C++ line 4956) ---
        let mut has_selection_flash = self.selection_flash_envelope.is_some();
        xfer.xfer_bool(&mut has_selection_flash)
            .map_err(|e| format!("{:?}", e))?;
        if has_selection_flash {
            if self.selection_flash_envelope.is_none() {
                self.selection_flash_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.selection_flash_envelope {
                envelope.xfer(xfer)?;
            }
        } else {
            self.selection_flash_envelope = None;
        }

        // --- color tint envelope (C++ line 4971) ---
        let mut has_tint_envelope = self.tint_envelope.is_some();
        xfer.xfer_bool(&mut has_tint_envelope)
            .map_err(|e| format!("{:?}", e))?;
        if has_tint_envelope {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.xfer(xfer)?;
            }
        } else {
            self.tint_envelope = None;
        }

        // --- terrain decal type (C++ line 4986: xferUser sizeof TerrainDecalType) ---
        let mut decal_type = terrain_decal_to_u32(self.terrain_decal_type);
        xfer.xfer_unsigned_int(&mut decal_type)
            .map_err(|e| format!("{:?}", e))?;
        self.terrain_decal_type = terrain_decal_from_u32(decal_type);

        // --- explicit opacity (C++ line 4992) ---
        let mut explicit_opacity = self.explicit_opacity;
        xfer.xfer_real(&mut explicit_opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.explicit_opacity = explicit_opacity;

        // --- stealth opacity (C++ line 4995) ---
        let mut stealth_opacity = self.stealth_opacity;
        xfer.xfer_real(&mut stealth_opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.stealth_opacity = stealth_opacity;

        // --- effective stealth opacity (C++ line 4998) ---
        let mut effective_stealth_opacity = self.effective_stealth_opacity;
        xfer.xfer_real(&mut effective_stealth_opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.effective_stealth_opacity = effective_stealth_opacity;

        // --- decal opacity fade target (C++ line 5001) ---
        let mut decal_opacity_fade_target = self.decal_opacity_fade_target;
        xfer.xfer_real(&mut decal_opacity_fade_target)
            .map_err(|e| format!("{:?}", e))?;
        self.decal_opacity_fade_target = decal_opacity_fade_target;

        // --- decal opacity fade rate (C++ line 5004) ---
        let mut decal_opacity_fade_rate = self.decal_opacity_fade_rate;
        xfer.xfer_real(&mut decal_opacity_fade_rate)
            .map_err(|e| format!("{:?}", e))?;
        self.decal_opacity_fade_rate = decal_opacity_fade_rate;

        // --- decal opacity (C++ line 5007) ---
        let mut decal_opacity = self.decal_opacity;
        xfer.xfer_real(&mut decal_opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.decal_opacity = decal_opacity;

        // --- object id (C++ line 5010: xferObjectID, with validation) ---
        // PARITY_NOTE: Added in version 3. C++ validates the object binding on load.
        if version >= 3 {
            let mut object_id = self.object_id.unwrap_or(0);
            xfer.xfer_object_id(&mut object_id)
                .map_err(|e| format!("{:?}", e))?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.object_id = if object_id != 0 {
                    Some(object_id)
                } else {
                    None
                };
            }
        }

        // --- status (C++ line 5059: xferUnsignedInt) ---
        let mut status_bits = self.status.bits;
        xfer.xfer_unsigned_int(&mut status_bits)
            .map_err(|e| format!("{:?}", e))?;
        self.status.bits = status_bits;

        // --- tint status (C++ line 5062) ---
        let mut tint_status_bits = self.tint_status.bits;
        xfer.xfer_unsigned_int(&mut tint_status_bits)
            .map_err(|e| format!("{:?}", e))?;
        self.tint_status.bits = tint_status_bits;

        // --- prev tint status (C++ line 5065) ---
        let mut prev_tint_status_bits = self.prev_tint_status.bits;
        xfer.xfer_unsigned_int(&mut prev_tint_status_bits)
            .map_err(|e| format!("{:?}", e))?;
        self.prev_tint_status.bits = prev_tint_status_bits;

        // --- fading mode (C++ line 5068: xferUser sizeof FadingMode) ---
        let mut fade_mode = fading_mode_to_u32(self.fade_mode);
        xfer.xfer_unsigned_int(&mut fade_mode)
            .map_err(|e| format!("{:?}", e))?;
        self.fade_mode = fading_mode_from_u32(fade_mode);

        // --- time elapsed fade (C++ line 5071) ---
        let mut time_elapsed_fade = self.time_elapsed_fade;
        xfer.xfer_unsigned_int(&mut time_elapsed_fade)
            .map_err(|e| format!("{:?}", e))?;
        self.time_elapsed_fade = time_elapsed_fade;

        // --- time to fade (C++ line 5074) ---
        let mut time_to_fade = self.time_to_fade;
        xfer.xfer_unsigned_int(&mut time_to_fade)
            .map_err(|e| format!("{:?}", e))?;
        self.time_to_fade = time_to_fade;

        // --- loco info (C++ line 5076: inline fields, no versioning) ---
        let mut has_loco_info = self.loco_info.is_some();
        xfer.xfer_bool(&mut has_loco_info)
            .map_err(|e| format!("{:?}", e))?;
        if has_loco_info {
            if self.loco_info.is_none() {
                self.loco_info = Some(LocoInfo::default());
            }
            if let Some(ref mut loco_info) = self.loco_info {
                loco_info.xfer(xfer)?;
            }
        } else {
            self.loco_info = None;
        }

        // --- drawable modules (C++ line 5130: xferDrawableModules) ---
        if version >= 3 {
            xfer_drawable_modules(xfer, &mut self.draw_modules)?;
        }

        // --- stealth look (C++ line 5133: xferUser sizeof StealthLookType) ---
        let mut stealth_look = stealth_look_to_u32(self.stealth_look);
        xfer.xfer_unsigned_int(&mut stealth_look)
            .map_err(|e| format!("{:?}", e))?;
        self.stealth_look = stealth_look_from_u32(stealth_look);

        // --- flash count (C++ line 5137: xferInt) ---
        let mut flash_count = self.flash_count as i32;
        xfer.xfer_int(&mut flash_count)
            .map_err(|e| format!("{:?}", e))?;
        self.flash_count = flash_count.max(0) as u32;

        // --- flash color (C++ line 5140: xferColor = i32 ARGB) ---
        let mut flash_color_bits = vector3_to_color_bits(self.flash_color);
        xfer.xfer_int(&mut flash_color_bits)
            .map_err(|e| format!("{:?}", e))?;
        self.flash_color = color_bits_to_vector3(flash_color_bits);

        // --- hidden (C++ line 5143) ---
        let mut hidden = self.hidden;
        xfer.xfer_bool(&mut hidden)
            .map_err(|e| format!("{:?}", e))?;
        self.hidden = hidden;

        // --- hidden by stealth (C++ line 5146) ---
        let mut hidden_by_stealth = self.hidden_by_stealth;
        xfer.xfer_bool(&mut hidden_by_stealth)
            .map_err(|e| format!("{:?}", e))?;
        self.hidden_by_stealth = hidden_by_stealth;

        // --- heat vision / second material pass opacity (C++ line 5149) ---
        let mut second_material_pass_opacity = self.second_material_pass_opacity;
        xfer.xfer_real(&mut second_material_pass_opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.second_material_pass_opacity = second_material_pass_opacity;

        // --- instance is identity (C++ line 5152) ---
        // PARITY_NOTE: Added in version 3. C++ uses xferBool.
        if version >= 3 {
            let mut instance_is_identity = self.is_instance_identity();
            xfer.xfer_bool(&mut instance_is_identity)
                .map_err(|e| format!("{:?}", e))?;
        }

        // --- instance matrix (C++ line 5155) ---
        if version >= 4 {
            xfer_matrix3d_user(xfer, &mut self.instance_transform)?;
        }

        // --- instance scale (C++ line 5158) ---
        let mut instance_scale = self.instance_scale;
        xfer.xfer_real(&mut instance_scale)
            .map_err(|e| format!("{:?}", e))?;
        self.instance_scale = instance_scale;

        // --- drawable info shroud-status object id (C++ line 5161) ---
        if version >= 5 {
            xfer.xfer_object_id(&mut self.drawable_info.shroud_status_object_id)
                .map_err(|e| format!("{:?}", e))?;
        }

        // --- expiration date (C++ line 5182: xferUnsignedInt) ---
        let mut expiration = self.expiration_frame.unwrap_or(0);
        xfer.xfer_unsigned_int(&mut expiration)
            .map_err(|e| format!("{:?}", e))?;
        self.expiration_frame = if expiration > 0 {
            Some(expiration)
        } else {
            None
        };

        // --- icon count + icons (C++ line 5185-5267) ---
        if version >= 6 {
            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    let mut empty_icon_info;
                    let icon_info = match self.icon_info.as_mut() {
                        Some(icon_info) => icon_info,
                        None => {
                            empty_icon_info = IconInfo::new();
                            &mut empty_icon_info
                        }
                    };
                    icon_info.xfer_cpp_layout(xfer)?;
                }
                XferMode::Load => {
                    let mut icon_info = IconInfo::new();
                    icon_info.xfer_cpp_layout(xfer)?;
                    self.icon_info = if icon_info.icons.is_empty() {
                        None
                    } else {
                        Some(icon_info)
                    };
                }
                XferMode::Invalid => {
                    return Err("BasicDrawable::xfer - invalid xfer mode".to_string());
                }
            }
        } else {
            let mut has_icon_info = self.icon_info.is_some();
            xfer.xfer_bool(&mut has_icon_info)
                .map_err(|e| format!("{:?}", e))?;
            if has_icon_info {
                if self.icon_info.is_none() {
                    self.icon_info = Some(IconInfo::new());
                }
                if let Some(ref mut icon_info) = self.icon_info {
                    icon_info.xfer(xfer)?;
                }
            } else {
                self.icon_info = None;
            }
        }

        if xfer.get_xfer_mode() == XferMode::Load {
            // C++ resets stealth look after load so a subsequent update re-applies
            // hidden/shadow behavior from authoritative object state.
            // (C++ Drawable.cpp line 5274: m_stealthLook = STEALTHLOOK_NONE)
            self.stealth_look = StealthLook::None;
            // C++ Drawable.cpp:5276-5278 — hide draw modules immediately.
            if self.hidden || self.hidden_by_stealth {
                self.update_hidden_status();
            }
            // C++ Drawable.cpp line 5293: stopAmbientSound(); Restarted in loadPostProcess().
            self.stop_ambient_sound();
        }

        // --- ambient sound enabled (C++ line 5300: version >= 4) ---
        if version >= 4 {
            let mut ambient_sound_enabled = self.ambient_sound_enabled;
            xfer.xfer_bool(&mut ambient_sound_enabled)
                .map_err(|e| format!("{:?}", e))?;
            self.ambient_sound_enabled = ambient_sound_enabled;
        }

        // --- ambient sound enabled from script (C++ line 5305: version >= 6) ---
        if version >= 6 {
            let mut ambient_sound_enabled_from_script = self.ambient_sound_enabled_from_script;
            xfer.xfer_bool(&mut ambient_sound_enabled_from_script)
                .map_err(|e| format!("{:?}", e))?;
            self.ambient_sound_enabled_from_script = ambient_sound_enabled_from_script;
        }

        // --- custom ambient sound info (C++ line 5311: version >= 7) ---
        if version >= 7 {
            let mut customized =
                self.custom_sound_ambient_off || self.custom_sound_ambient_dynamic_info.is_some();
            xfer.xfer_bool(&mut customized)
                .map_err(|e| format!("{:?}", e))?;

            if customized {
                let mut customized_to_silence = self.custom_sound_ambient_off;
                xfer.xfer_bool(&mut customized_to_silence)
                    .map_err(|e| format!("{:?}", e))?;

                if xfer.get_xfer_mode() == XferMode::Load {
                    self.custom_sound_ambient_off = customized_to_silence;
                    if !customized_to_silence {
                        let mut base_info_name = String::new();
                        xfer.xfer_ascii_string(&mut base_info_name)
                            .map_err(|e| format!("{:?}", e))?;

                        let mut custom_info = DynamicAudioEventInfo::new();
                        custom_info
                            .xfer_no_name(xfer)
                            .map_err(|e| format!("{:?}", e))?;
                        self.custom_sound_ambient_base_name = Some(base_info_name);
                        self.custom_sound_ambient_dynamic_info = Some(custom_info);
                    } else {
                        self.custom_sound_ambient_base_name = None;
                        self.custom_sound_ambient_dynamic_info = None;
                    }
                } else if !customized_to_silence {
                    let mut base_info_name = self
                        .custom_sound_ambient_base_name
                        .clone()
                        .or_else(|| {
                            self.custom_sound_ambient_dynamic_info
                                .as_ref()
                                .map(|info| info.get_original_name().to_string())
                        })
                        .unwrap_or_default();
                    xfer.xfer_ascii_string(&mut base_info_name)
                        .map_err(|e| format!("{:?}", e))?;

                    let Some(custom_info) = self.custom_sound_ambient_dynamic_info.as_mut() else {
                        return Err(
                            "BasicDrawable::xfer - missing custom ambient sound data".to_string()
                        );
                    };
                    custom_info
                        .xfer_no_name(xfer)
                        .map_err(|e| format!("{:?}", e))?;
                }
            } else if xfer.get_xfer_mode() == XferMode::Load {
                self.custom_sound_ambient_off = false;
                self.custom_sound_ambient_base_name = None;
                self.custom_sound_ambient_dynamic_info = None;
            }
        }

        // --- Rust-specific fields not in C++ (preserved for old Rust save compatibility) ---
        if version >= 7 {
            if xfer.get_xfer_mode() == XferMode::Load {
                self.reset_volatile_shroud_state();
            }
            return Ok(());
        }

        let mut visible = self.visible;
        xfer.xfer_bool(&mut visible)
            .map_err(|e| format!("{:?}", e))?;
        self.visible = visible;

        let mut selected = self.selected;
        xfer.xfer_bool(&mut selected)
            .map_err(|e| format!("{:?}", e))?;
        self.selected = selected;

        let mut selectable = self.selectable;
        xfer.xfer_bool(&mut selectable)
            .map_err(|e| format!("{:?}", e))?;
        self.selectable = selectable;

        let mut opacity = self.opacity;
        xfer.xfer_real(&mut opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.opacity = opacity;

        xfer_vector3(xfer, &mut self.tint_color)?;

        let mut receives_dynamic_lights = self.receives_dynamic_lights;
        xfer.xfer_bool(&mut receives_dynamic_lights)
            .map_err(|e| format!("{:?}", e))?;
        self.receives_dynamic_lights = receives_dynamic_lights;

        xfer_vector3(xfer, &mut self.terrain_decal_size)?;

        let mut current_frame = self.current_frame;
        xfer.xfer_unsigned_int(&mut current_frame)
            .map_err(|e| format!("{:?}", e))?;
        self.current_frame = current_frame;

        if xfer.get_xfer_mode() == XferMode::Load {
            self.reset_volatile_shroud_state();
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ Drawable.cpp:5400-5403 — object matrix is authoritative after load.
        if let Some(object_id) = self.object_id {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    let transform = Matrix4::from_glam(obj.get_transform_matrix());
                    self.position = Vector3::new(
                        transform.elements[0][3],
                        transform.elements[1][3],
                        transform.elements[2][3],
                    );
                    let mut rotation = transform;
                    rotation.elements[0][3] = 0.0;
                    rotation.elements[1][3] = 0.0;
                    rotation.elements[2][3] = 0.0;
                    self.instance_transform = rotation;
                }
            }
        }

        if self.ambient_sound_enabled && self.ambient_sound_enabled_from_script {
            self.start_ambient_sound(true);
        } else {
            self.stop_ambient_sound();
        }
        Ok(())
    }
}
