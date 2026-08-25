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

impl BasicDrawable {
    // -----------------------------------------------------------------------
    // Draw module management
    // -----------------------------------------------------------------------

    /// Add a draw module to this drawable.
    /// C++ parity: Drawable constructor allocates DrawModules from ThingTemplate.
    pub fn add_draw_module(&mut self, module: Box<dyn DrawModule>) {
        self.draw_modules.push(module);
        self.is_model_dirty = true;
    }

    /// Get reference to the draw modules list.
    pub fn get_draw_modules(&self) -> &[Box<dyn DrawModule>] {
        &self.draw_modules
    }

    /// Get mutable reference to the draw modules list.
    pub fn get_draw_modules_mut(&mut self) -> &mut Vec<Box<dyn DrawModule>> {
        self.flush_dirty_model_condition();
        &mut self.draw_modules
    }

    /// Set bone data for this drawable.
    /// PARITY_NOTE: In C++, bone data comes from W3D RenderObjClass → HTreeClass.
    /// Stored inline as fallback for modules without W3D bone systems.
    pub fn set_bone_data(&mut self, data: BoneData) {
        self.bone_data = Some(data);
    }

    /// Get reference to bone data if present.
    pub fn get_bone_data(&self) -> Option<&BoneData> {
        self.bone_data.as_ref()
    }

    /// Get mutable reference to bone data, creating if needed.
    pub fn get_bone_data_mut(&mut self) -> &mut BoneData {
        if self.bone_data.is_none() {
            self.bone_data = Some(BoneData::default());
        }
        self.bone_data.as_mut().unwrap()
    }

    // -----------------------------------------------------------------------
    // Weapon fire FX dispatch
    // -----------------------------------------------------------------------

    /// Wave 980: host group/UI text residual without OBJECT_REGISTRY.
    pub(super) fn draw_ui_text_from_presentation(&self) -> Result<(), Box<dyn Error>> {
        // Wave 1077: FOW fully-obscured residual hides dual presentation UI text.
        if self.drawable_fully_obscured_by_shroud {
            return Ok(());
        }
        // Wave 1055: hide group numerals for unselected effectively-stealthed residual.
        if self.presentation_effectively_stealthed && !self.selected_or_moused_over_for_icon_pips()
        {
            return Ok(());
        }

        let Some(screen_pos) = with_tactical_view_ref(|view| {
            view.world_to_screen(&Point3::new(
                self.position.x,
                self.position.y,
                self.position.z,
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
            if let Some((r, g, b)) = self.presentation_indicator_color {
                text_color = (0xFFu32 << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
            }
        }

        // Wave 1055/1056: host control-group residual → group numeral dual draw.
        // Mirror factory draw_ui_text offset + draw_caption_string path.
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

        let group_number = self.presentation_hotkey_group as i32;
        if group_number > NO_HOTKEY_SQUAD && group_number < NUM_HOTKEY_SQUADS as i32 {
            let mut manager = get_display_string_manager();
            if let Some(group_text) = manager.get_group_numeral_string(group_number) {
                // Wave 1056: actually draw the numeral (not resolve-only residual).
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
            }
        }

        // Wave 1058: formation letter residual (C++ formation id dual draw).
        if self.presentation_formation_id != 0 {
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
            }
        }

        // Captions draw via live `impl_draw` C++ drawCaption (black box + InGameUI style).
        Ok(())
    }

    /// Handle weapon fire FX: apply recoil, then dispatch FX to draw modules.
    /// C++ parity: `Drawable::handleWeaponFireFX` (Drawable.cpp:4216-4239).
    /// Applies recoil impulse to loco info, then iterates draw modules to
    /// dispatch FX at the weapon barrel position.
    pub fn handle_weapon_fire_fx(
        &mut self,
        wslot: WeaponSlotType,
        barrel: i32,
        fx_list: Option<&FXListRef>,
        weapon_speed: f32,
        recoil_amount: f32,
        recoil_angle: f32,
        victim_pos: Option<&Vector3>,
        damage_radius: f32,
    ) -> bool {
        self.handle_weapon_fire_fx_with_module_index(
            wslot,
            barrel,
            fx_list,
            weapon_speed,
            recoil_amount,
            recoil_angle,
            victim_pos,
            damage_radius,
        )
        .is_some()
    }

    /// Apply the C++ weapon-fire callback and return the declaration-order
    /// draw-module index that consumed it, if any.
    ///
    /// The index is the smallest stable identity available at this boundary:
    /// C++ walks the `DrawModule` array in declaration order and returns at
    /// the first `ObjectDrawInterface` that reports the FX handled. Keeping
    /// that identity here lets a future frozen presentation plan carry the
    /// selected module without re-running the live loop after source state
    /// has changed. The boolean wrapper above intentionally retains the old
    /// public API for callers that only need the C++ handled result.
    pub fn handle_weapon_fire_fx_with_module_index(
        &mut self,
        wslot: WeaponSlotType,
        barrel: i32,
        fx_list: Option<&FXListRef>,
        weapon_speed: f32,
        recoil_amount: f32,
        recoil_angle: f32,
        victim_pos: Option<&Vector3>,
        damage_radius: f32,
    ) -> Option<usize> {
        // Wave 980: host empty dual-world still applies recoil + draw-module FX.
        // Orientation residual comes from presentation pose when registry is empty.

        // C++ applies recoil impulse if recoil_amount != 0
        if recoil_amount != 0.0 {
            let mut adjusted_angle = recoil_angle;
            if dual_world_registry_unavailable() {
                adjusted_angle -= self.presentation_orientation;
            } else if let Some(obj_id) = self.object_id {
                if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) {
                    if let Ok(obj_guard) = obj_arc.read() {
                        adjusted_angle -= obj_guard.get_orientation();
                    }
                }
            }
            // C++ flips direction 180 degrees
            adjusted_angle += std::f32::consts::PI;

            if let Some(ref mut loco) = self.loco_info {
                loco.acceleration_pitch_rate += recoil_amount * adjusted_angle.cos();
                loco.acceleration_roll_rate += recoil_amount * adjusted_angle.sin();
            }
        }

        // C++ iterates draw modules and dispatches FX
        for (module_index, dm) in self.draw_modules.iter_mut().enumerate() {
            if dm.handle_weapon_fire_fx(
                wslot,
                barrel,
                fx_list,
                weapon_speed,
                victim_pos,
                damage_radius,
            ) {
                return Some(module_index);
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // Barrel count
    // -----------------------------------------------------------------------

    /// Get barrel count for the given weapon slot.
    /// C++ parity: `Drawable::getBarrelCount` (Drawable.cpp:4242-4252).
    /// Iterates draw modules; first non-zero count wins.
    pub fn get_barrel_count(&self, wslot: WeaponSlotType) -> i32 {
        // C++ iterates draw modules first
        for dm in &self.draw_modules {
            let count = dm.get_barrel_count(wslot);
            if count != 0 {
                return count;
            }
        }
        // Fall back to bone_data barrel counts if no draw module provides them
        if let Some(ref bd) = self.bone_data {
            return bd.barrel_count_for_slot(wslot);
        }
        0
    }

    // -----------------------------------------------------------------------
    // Bone position queries
    // -----------------------------------------------------------------------

    /// Query pristine (unanimated) bone positions from the model.
    /// C++ parity: `Drawable::getPristineBonePositions` (Drawable.cpp:747-773).
    /// Iterates draw modules, aggregating results. Falls back to inline bone_data.
    pub fn get_pristine_bone_positions(
        &self,
        bone_name: &str,
        start: i32,
        positions: &mut [Vector3],
        transforms: &mut [Matrix4],
    ) -> i32 {
        let max_bones = positions.len().min(transforms.len());
        let mut count = 0;
        let mut remaining = max_bones;

        // C++ iterates draw modules
        for dm in &self.draw_modules {
            if remaining == 0 {
                break;
            }
            let sub = dm.get_pristine_bone_positions(
                bone_name,
                start,
                &mut positions[count..],
                &mut transforms[count..],
            );
            if sub > 0 {
                count += sub as usize;
                remaining = remaining.saturating_sub(sub as usize);
            }
        }

        // Fall back to inline bone_data
        if count == 0 {
            if let Some(ref bd) = self.bone_data {
                return bd.query_pristine_bones(bone_name, start, positions, transforms);
            }
        }
        count as i32
    }

    /// Query current (animated) bone positions from the model.
    /// C++ parity: `Drawable::getCurrentClientBonePositions` (Drawable.cpp:776-802).
    pub fn get_current_client_bone_positions(
        &self,
        bone_name: &str,
        start: i32,
        positions: &mut [Vector3],
        transforms: &mut [Matrix4],
    ) -> i32 {
        let max_bones = positions.len().min(transforms.len());
        let mut count = 0;
        let mut remaining = max_bones;

        for dm in &self.draw_modules {
            if remaining == 0 {
                break;
            }
            let sub = dm.get_current_bone_positions(
                bone_name,
                start,
                &mut positions[count..],
                &mut transforms[count..],
            );
            if sub > 0 {
                count += sub as usize;
                remaining = remaining.saturating_sub(sub as usize);
            }
        }

        if count == 0 {
            if let Some(ref bd) = self.bone_data {
                return bd.query_current_bones(bone_name, start, positions, transforms);
            }
        }
        count as i32
    }

    /// Query current world-space bone transform.
    /// C++ parity: `Drawable::getCurrentWorldspaceClientBonePositions` (Drawable.cpp:805-814).
    pub fn get_current_worldspace_client_bone_positions(
        &self,
        bone_name: &str,
        transform: &mut Matrix4,
    ) -> bool {
        // C++ iterates draw modules
        for dm in &self.draw_modules {
            if dm.get_current_worldspace_client_bone_positions(bone_name, transform) {
                return true;
            }
        }
        // Fall back to inline bone_data
        if let Some(ref bd) = self.bone_data {
            return bd.query_worldspace_bone(bone_name, transform);
        }
        false
    }

    // -----------------------------------------------------------------------
    // Projectile launch offset
    // -----------------------------------------------------------------------

    /// Calculate projectile spawn position using bone data.
    /// C++ parity: `Drawable::getProjectileLaunchOffset` (Drawable.cpp:655-664).
    /// Iterates draw modules requesting projectile launch offset from
    /// ObjectDrawInterface. Falls back to bone_data lookup.
    pub fn get_projectile_launch_offset(
        &self,
        wslot: WeaponSlotType,
        barrel: i32,
        launch_pos: &mut Matrix4,
        turret: WhichTurretType,
        turret_rot_pos: &mut Vector3,
        mut turret_pitch_pos: Option<&mut Vector3>,
    ) -> bool {
        // C++ iterates draw modules via ObjectDrawInterface and forwards all
        // output pointers to the first module that can answer.
        for dm in &self.draw_modules {
            if dm.get_projectile_launch_offset(
                wslot,
                barrel,
                launch_pos,
                turret,
                turret_rot_pos,
                turret_pitch_pos.as_deref_mut(),
            ) {
                return true;
            }
        }

        // Fall back: derive from bone_data if available.
        // PARITY_NOTE: C++ computes this from W3D bone transforms. Here we
        // approximate by looking up "WeaponBone" entries in bone_data.
        if let Some(ref bd) = self.bone_data {
            let bone_name = match wslot {
                WeaponSlotType::Primary => "WeaponBone",
                WeaponSlotType::Secondary => "WeaponBone02",
                WeaponSlotType::Tertiary => "WeaponBone03",
            };
            let bones = match bd.current_bones.get(bone_name) {
                Some(b) => b,
                None => match bd.pristine_bones.get(bone_name) {
                    Some(b) => b,
                    None => return false,
                },
            };
            let idx = barrel.max(0) as usize;
            if idx < bones.len() {
                *launch_pos = bones[idx].1;
                *turret_rot_pos = bones[idx].0;
                if let Some(ref mut pitch) = turret_pitch_pos {
                    **pitch = bones[idx].0;
                }
                return true;
            }
        }
        false
    }
}
