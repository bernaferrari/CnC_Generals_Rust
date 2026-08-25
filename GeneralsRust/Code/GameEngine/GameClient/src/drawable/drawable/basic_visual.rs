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
    /// Initialize static images shared by all drawables.
    /// C++ Drawable::initStaticImages (Drawable.cpp:249-285):
    /// Loads veterancy images (SCVeter1/2/3), ammo/container pip images,
    /// and icon animation templates. Called once at startup.
    pub fn init_static_images(&mut self) {
        if self.static_images_inited {
            return;
        }

        const STATIC_MAPPED_IMAGE_NAMES: [&str; 7] = [
            "SCVeter1",
            "SCVeter2",
            "SCVeter3",
            "SCPAmmoFull",
            "SCPAmmoEmpty",
            "SCPPipFull",
            "SCPPipEmpty",
        ];

        for image_name in STATIC_MAPPED_IMAGE_NAMES {
            let _ = ensure_client_mapped_image(image_name);
            let found = get_mapped_image_collection()
                .read()
                .find_image_by_name(image_name)
                .is_some();
            if !found {
                log::debug!(
                    "PARITY_NOTE: Drawable::init_static_images missing mapped image '{}'",
                    image_name
                );
            }
        }

        const STATIC_ICON_TEMPLATE_TYPES: [IconType; 13] = [
            IconType::DefaultHeal,
            IconType::StructureHeal,
            IconType::VehicleHeal,
            IconType::Demoralized,
            IconType::BombTimed,
            IconType::BombRemote,
            IconType::Disabled,
            IconType::BattleplanBombard,
            IconType::BattleplanHoldTheLine,
            IconType::BattleplanSearchAndDestroy,
            IconType::Enthusiastic,
            IconType::EnthusiasticSubliminal,
            IconType::CarBomb,
        ];

        if let Some(anim2d_collection) = get_anim2d_collection() {
            let anim2d_collection = anim2d_collection.read();
            for icon_type in STATIC_ICON_TEMPLATE_TYPES {
                let icon_name = icon_type.name();
                let found = anim2d_collection
                    .find_template(&AsciiString::from(icon_name))
                    .is_some();
                if !found {
                    log::debug!(
                        "PARITY_NOTE: Drawable::init_static_images missing Anim2D template '{}'",
                        icon_name
                    );
                }
            }
        } else {
            log::debug!(
                "PARITY_NOTE: Drawable::init_static_images could not access Anim2D collection"
            );
        }

        self.static_images_inited = true;
    }

    /// Free static image resources.
    /// C++ Drawable::killStaticImages (Drawable.cpp:288-295):
    /// Deletes the animation templates array. Called at shutdown.
    /// PARITY_NOTE: No resources to free until init_static_images loads real assets.
    /// When ported, this must: delete[] s_animationTemplates; s_animationTemplates = NULL.
    pub fn kill_static_images(&mut self) {
        // C++: delete[] s_animationTemplates; s_animationTemplates = NULL;
        // When asset system is ported, free any allocated static resources here.
        self.static_images_inited = false;
    }

    /// Set caption text displayed above this drawable.
    /// C++ Drawable::setCaptionText (Drawable.cpp:4293-4322):
    /// Creates a DisplayString, applies font, sets sanitized text.
    /// For Rust, we store the text directly; font/rendering is handled by overlay_data.
    pub fn set_caption_text(&mut self, text: &str) {
        if text.is_empty() {
            self.clear_caption_text();
            return;
        }
        let mut sanitized = text.to_string();
        get_language_filter().filter_line(&mut sanitized);
        if self.caption_text.as_deref() != Some(sanitized.as_str()) {
            self.caption_text = Some(sanitized);
        }
    }

    /// Clear caption text.
    /// C++ Drawable::clearCaptionText (Drawable.cpp:4325-4330):
    /// Frees the DisplayString and sets pointer to NULL.
    pub fn clear_caption_text(&mut self) {
        self.caption_text = None;
    }

    /// Get caption text if set.
    /// C++ Drawable::getCaptionText (Drawable.cpp:4333-4339):
    /// Returns the DisplayString text or empty UnicodeString.
    pub fn get_caption_text(&self) -> Option<&str> {
        self.caption_text.as_deref()
    }

    pub fn is_effectively_hidden(&self) -> bool {
        self.hidden || !self.visible || self.hidden_by_stealth
    }

    /// Exact C++ `Drawable::isDrawableEffectivelyHidden` predicate used by
    /// `RTS3DScene::renderOneObject`.
    ///
    /// Keep this distinct from [`Self::is_effectively_hidden`].  Rust's
    /// presentation `visible` flag is a broader local/UI concern, whereas
    /// the source scene branch checks only `m_hidden || m_hiddenByStealth`
    /// before deciding whether it may refresh `m_shroudClearFrame`.
    pub(crate) fn is_scene_effectively_hidden(&self) -> bool {
        self.hidden || self.hidden_by_stealth
    }

    pub fn set_drawable_hidden(&mut self, hidden: bool) {
        if self.hidden == hidden {
            return;
        }
        self.hidden = hidden;
        self.update_hidden_status();
    }

    pub fn set_selectable(&mut self, selectable: bool) {
        self.selectable = selectable;
        if !selectable {
            self.selected = false;
        }
    }

    pub fn is_selectable(&self) -> bool {
        self.selectable
    }

    pub fn tint_color_effect(&self) -> Option<Vector3> {
        self.tint_envelope
            .as_ref()
            .filter(|env| env.is_effective)
            .map(|env| env.color())
    }

    pub fn selection_color_effect(&self) -> Option<Vector3> {
        self.selection_flash_envelope
            .as_ref()
            .filter(|env| env.is_effective)
            .map(|env| env.color())
    }

    pub(super) fn update_tint_status(&mut self) {
        if self.prev_tint_status == self.tint_status {
            return;
        }

        if self.test_tint_status(TintStatus::DISABLED) {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.play(DARK_GRAY_DISABLED_COLOR, 30, 30, SUSTAIN_INDEFINITELY);
            }
        } else if self.test_tint_status(TintStatus::GAINING_SUBDUAL_DAMAGE) {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.play(SUBDUAL_DAMAGE_COLOR, 150, 150, SUSTAIN_INDEFINITELY);
            }
        } else if self.test_tint_status(TintStatus::FRENZY) {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            let frenzy = if self.is_object_kind_of(gamelogic::common::types::KindOf::Infantry) {
                FRENZY_COLOR_INFANTRY
            } else {
                FRENZY_COLOR
            };
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.play(frenzy, 30, 30, SUSTAIN_INDEFINITELY);
            }
        } else {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.release();
            }
        }

        self.prev_tint_status = self.tint_status;
    }

    /// Compute render condition flags from drawable state.
    /// Maps drawable visual state to RenderBridge condition flags.
    pub(super) fn compute_render_condition_flags(
        &self,
    ) -> crate::render_bridge::RenderConditionFlags {
        use crate::render_bridge::RenderConditionFlags;
        let mut flags = RenderConditionFlags::empty();

        if self
            .model_condition_flags
            .test(ModelConditionFlags::DAMAGED)
        {
            flags |= RenderConditionFlags::DAMAGED;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::REALLYDAMAGED)
        {
            flags |= RenderConditionFlags::REALLY_DAMAGED;
        }
        if self.model_condition_flags.test(ModelConditionFlags::RUBBLE) {
            flags |= RenderConditionFlags::RUBBLE;
        }
        if self.model_condition_flags.test(ModelConditionFlags::NIGHT) {
            flags |= RenderConditionFlags::NIGHT;
        }
        if self.model_condition_flags.test(ModelConditionFlags::SNOW) {
            flags |= RenderConditionFlags::SNOW;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::AWAITING_CONSTRUCTION)
        {
            flags |= RenderConditionFlags::AWAITING_CONSTRUCTION;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::PARTIALLY_CONSTRUCTED)
        {
            flags |= RenderConditionFlags::PARTIALLY_CONSTRUCTED;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED)
        {
            flags |= RenderConditionFlags::ACTIVELY_CONSTRUCTED;
        }
        if self.model_condition_flags.test(ModelConditionFlags::AFLAME) {
            flags |= RenderConditionFlags::AFLAME;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::SMOLDERING)
        {
            flags |= RenderConditionFlags::SMOLDERING;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::TOPPLED)
        {
            flags |= RenderConditionFlags::TOPPLED;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::FLOODED)
        {
            flags |= RenderConditionFlags::FLOODED;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::DISGUISED)
        {
            flags |= RenderConditionFlags::DISGUISED;
        }

        if self.selected {
            flags |= RenderConditionFlags::SELECTED;
        }

        if matches!(self.stealth_look, StealthLook::DisguisedEnemy) {
            flags |= RenderConditionFlags::DISGUISED;
        }

        flags
    }

    pub(super) fn render_condition_flags_from_bits(
        condition_bits: u128,
    ) -> crate::render_bridge::RenderConditionFlags {
        crate::render_bridge::RenderConditionFlags::from_bits_truncate(condition_bits as u64)
    }

    pub(super) fn animation_mode_from_model_draw(
        mode: i32,
    ) -> Option<ww3d_core::animation::AnimationMode> {
        match mode {
            0 => Some(ww3d_core::animation::AnimationMode::Manual),
            1 => Some(ww3d_core::animation::AnimationMode::Loop),
            2 => Some(ww3d_core::animation::AnimationMode::Once),
            3 => Some(ww3d_core::animation::AnimationMode::LoopPingPong),
            4 => Some(ww3d_core::animation::AnimationMode::LoopBackward),
            5 => Some(ww3d_core::animation::AnimationMode::OnceBackward),
            _ => None,
        }
    }

    pub(super) fn bone_override_from_model_draw(
        override_state: &BoneOverrideState,
    ) -> crate::render_bridge::BoneOverride {
        crate::render_bridge::BoneOverride {
            bone_index: override_state.bone_index,
            bone_name: None,
            transform: override_state.transform,
        }
    }

    pub(super) fn render_state_from_flags(
        flags: crate::render_bridge::RenderConditionFlags,
        opacity: f32,
        tint: Vector3,
        selected: bool,
    ) -> crate::render_bridge::RenderStateOverrides {
        let mut state = crate::render_bridge::RenderStateOverrides::from_condition_flags(flags);
        state.opacity = state.opacity.min(opacity);
        // C++ adds getTintColor()/getSelectionColor() to lights; negative
        // channels darken (disabled gray, subdual blue, frenzy red/cyan).
        state.emissive_tint = [
            state.emissive_tint[0] + tint.x,
            state.emissive_tint[1] + tint.y,
            state.emissive_tint[2] + tint.z,
        ];
        state.selected |= selected;
        state
    }

    pub(super) fn matrix4_from_model_draw(matrix: glam::Mat4) -> Matrix4 {
        Matrix4::from_glam(matrix)
    }

    /// Ordered W3D output belongs to the gameplay Object association, not the
    /// client DrawableID.  The two IDs are allocated by separate C++ systems
    /// and routinely differ.
    pub(super) fn model_draw_states(&self) -> Vec<ModelDrawState> {
        self.object_id
            .and_then(|object_id| {
                TheGameClient::get().map(|client| client.object_model_draws(object_id))
            })
            .unwrap_or_default()
    }

    pub(super) fn find_hotkey_squad_number(player: &mut Player, object_id: u32) -> Option<i32> {
        for squad_number in 0..NUM_HOTKEY_SQUADS {
            if let Some(squad) = player.get_hotkey_squad(squad_number as i32) {
                if squad.is_on_squad_by_id(object_id) {
                    return Some(squad_number as i32);
                }
            }
        }

        None
    }

    pub(super) fn draw_caption_string(
        text_handle: &crate::gui::display_string::DisplayStringHandle,
        x: i32,
        y: i32,
        color: u32,
        drop_color: u32,
        font_name: &str,
        font_size: i32,
        font_is_bold: bool,
        drop_shadow_offset_x: i32,
        drop_shadow_offset_y: i32,
    ) {
        let mut text = text_handle.borrow_mut();
        let font_desc = FontDesc::new(font_name, font_size, font_is_bold);
        if let Ok(font) = get_font_library().get_font(&font_desc) {
            text.set_font(font);
        }
        text.draw_with_drop(
            x,
            y,
            color,
            drop_color,
            drop_shadow_offset_x,
            drop_shadow_offset_y,
        );
    }
}
