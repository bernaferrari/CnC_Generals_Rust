use super::*;
use crate::display::image::{ensure_client_mapped_image, get_mapped_image_collection};
use crate::display::view::{with_tactical_view_ref, Point3};
use crate::draw_group_info::get_draw_group_info;
use crate::drawable::ClientShroudVisibility;
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
use gamelogic::common::types::{
    FormationID, ObjectID, ObjectShroudStatus, WeaponSlotType, INVALID_ID,
};
use gamelogic::helpers::{BoneOverrideState, ModelDrawState, TheGameClient};
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{Player, NO_HOTKEY_SQUAD, NUM_HOTKEY_SQUADS};
use parking_lot::Mutex;
use std::error::Error;
use std::sync::Arc;

impl BasicDrawable {
    /// Get mutable reference to icon info, creating if necessary
    pub fn get_icon_info_mut(&mut self) -> &mut IconInfo {
        if self.icon_info.is_none() {
            self.icon_info = Some(IconInfo::new());
        }
        self.icon_info.as_mut().unwrap()
    }

    /// Get reference to icon info if it exists
    pub fn get_icon_info(&self) -> Option<&IconInfo> {
        self.icon_info.as_ref()
    }

    /// Get mutable reference to locomotor info, creating if necessary
    pub fn get_loco_info_mut(&mut self) -> &mut LocoInfo {
        if self.loco_info.is_none() {
            self.loco_info = Some(LocoInfo::default());
        }
        self.loco_info.as_mut().unwrap()
    }

    /// Get reference to locomotor info if it exists
    pub fn get_loco_info(&self) -> Option<&LocoInfo> {
        self.loco_info.as_ref()
    }

    /// C++ `Drawable::stopAmbientSound`.
    pub fn stop_ambient_sound(&mut self) {
        if let Some(event) = self.ambient_sound_event.take() {
            if let Some(audio) = get_global_audio_manager() {
                if let Ok(mut manager) = audio.lock() {
                    manager.remove_audio_event(event.get_playing_handle());
                }
            }
        }
    }

    /// C++ `Drawable::startAmbientSound(onlyIfPermanent)`.
    pub fn start_ambient_sound(&mut self, only_if_permanent: bool) {
        if !self.ambient_sound_enabled || !self.ambient_sound_enabled_from_script {
            return;
        }
        self.stop_ambient_sound();
        if self.custom_sound_ambient_off {
            return;
        }
        let Some(info) = self.custom_sound_ambient_dynamic_info.as_ref() else {
            return;
        };
        if only_if_permanent && !info.get_audio_event_info().is_permanent_sound() {
            return;
        }
        let mut event = AudioEventRts::new();
        event.set_event_name(info.get_audio_event_info().audio_name.clone());
        event.set_audio_event_info(std::sync::Arc::new(info.get_audio_event_info().clone()));
        event.set_drawable_id_override(self.id.0);
        if let Some(audio) = get_global_audio_manager() {
            if let Ok(mut manager) = audio.lock() {
                let handle = manager.add_audio_event(&event);
                event.set_playing_handle(handle);
            }
        }
        self.ambient_sound_event = Some(event);
    }

    /// Whether an ambient event is currently attached (started).
    #[must_use]
    pub fn ambient_sound_is_active(&self) -> bool {
        self.ambient_sound_event.is_some()
    }

    /// Get the current model-condition flags.
    pub fn get_model_condition_flags(&self) -> &ModelConditionBitFlags {
        &self.model_condition_flags
    }

    /// Clear and set model-condition flags in one operation.
    pub fn clear_and_set_model_condition_flags(
        &mut self,
        clr: &ModelConditionBitFlags,
        set: &ModelConditionBitFlags,
    ) {
        self.model_condition_flags.clear_and_set(clr, set);
    }

    /// Wave 965: stamp host presentation residual (no OBJECT_REGISTRY dual-world).
    pub fn set_presentation_host_residual(
        &mut self,
        kind_names: Vec<String>,
        indicator_color: Option<(u8, u8, u8)>,
        effectively_stealthed: bool,
        scene_hidden_by_stealth: bool,
        health_pct: f32,
        selected: bool,
        veterancy_level: u8,
        under_construction: bool,
        construction_percent: f32,
        ammo_pip_total: u8,
        ammo_pip_full: u8,
        occupant_count: u8,
        max_garrison: u8,
        disabled: bool,
        is_carbomb: bool,
        weapon_bonus_enthusiastic: bool,
        orientation: f32,
        show_healing: bool,
        healing_icon_type: u8,
        garrisoned_ids: Vec<u32>,
        emoticon_name: String,
        emoticon_frames_left: i32,
        formation_id: u32,
        caption: String,
    ) {
        self.presentation_kind_names = kind_names;
        self.presentation_indicator_color = indicator_color;
        self.presentation_effectively_stealthed = effectively_stealthed;
        // Wave 1055 default unless stamped via catalog apply.
        // (hotkey_group applied separately by apply_presentation_unit_catalog)
        self.presentation_health_pct = health_pct.clamp(0.0, 1.0);
        self.presentation_selected = selected;
        self.presentation_orientation = orientation;
        self.selected = selected;
        self.presentation_veterancy_level = veterancy_level;
        self.presentation_under_construction = under_construction;
        self.presentation_construction_percent = construction_percent.clamp(0.0, 1.0);
        self.presentation_ammo_pip_total = ammo_pip_total;
        self.presentation_ammo_pip_full = ammo_pip_full;
        self.presentation_occupant_count = occupant_count;
        self.presentation_max_garrison = max_garrison;
        self.presentation_disabled = disabled;
        self.presentation_is_carbomb = is_carbomb;
        self.presentation_weapon_bonus_enthusiastic = weapon_bonus_enthusiastic;
        self.presentation_show_healing = show_healing;
        self.presentation_healing_icon_type = healing_icon_type;
        self.presentation_garrisoned_ids = garrisoned_ids;
        // Wave 1057: emoticon residual for dual icon UI.
        if !emoticon_name.is_empty() && emoticon_frames_left > 0 {
            let _ = self.set_emoticon(&emoticon_name, emoticon_frames_left as u32);
        } else {
            self.clear_emoticon();
        }
        self.presentation_formation_id = formation_id;
        // Wave 1059: caption residual for dual draw_ui_text.
        if caption.is_empty() {
            self.clear_caption_text();
        } else {
            self.set_caption_text(&caption);
        }
        // C++ `m_hiddenByStealth` is viewer-relative. Do not derive it from
        // generic `isEffectivelyStealthed`: friendly stealthed units use a
        // translucent visible look and must still reach the scene path.
        self.hidden_by_stealth = scene_hidden_by_stealth;
        if let Some(color) = indicator_color {
            self.set_indicator_color(Some(color));
        }
        // Wave 970/972: keep overlay residual coherent for host draw path.
        self.overlay_data.health_ratio = self.presentation_health_pct;
        self.overlay_data.veterancy_level = self.presentation_veterancy_level;
        self.overlay_data.is_under_construction = self.presentation_under_construction;
        self.overlay_data.construction_percent = self.presentation_construction_percent;
        if selected {
            self.overlay_data.visible = true;
        }
    }

    /// C++ parity: `Drawable::reactToBodyDamageStateChange` (Drawable.cpp:1077-1101).
    ///
    /// Clears DAMAGED / REALLYDAMAGED / RUBBLE and sets the bit for `new_state`.
    /// Fail-closed residual: condition bits only — not full mesh/animation swap.
    pub fn react_to_body_damage_state_change(
        &mut self,
        new_state: gamelogic::common::types::BodyDamageType,
    ) {
        use gamelogic::common::types::BodyDamageType;

        let mut clear = create_model_condition_flags();
        clear.set(ModelConditionFlags::DAMAGED, true);
        clear.set(ModelConditionFlags::REALLYDAMAGED, true);
        clear.set(ModelConditionFlags::RUBBLE, true);

        let mut set = create_model_condition_flags();
        match new_state {
            BodyDamageType::Pristine => {}
            BodyDamageType::Damaged => set.set(ModelConditionFlags::DAMAGED, true),
            BodyDamageType::ReallyDamaged => set.set(ModelConditionFlags::REALLYDAMAGED, true),
            BodyDamageType::Rubble => set.set(ModelConditionFlags::RUBBLE, true),
        }

        self.clear_and_set_model_condition_flags(&clear, &set);
    }

    /// Replace full model-condition flags.
    pub fn replace_model_condition_flags(
        &mut self,
        flags: ModelConditionBitFlags,
        force_replace: bool,
    ) {
        if force_replace || self.model_condition_flags != flags {
            self.model_condition_flags = flags;
        }
    }

    /// Set a single model-condition bit by index.
    pub fn set_model_condition_state(&mut self, index: usize) {
        self.model_condition_flags.set(index, true);
    }

    /// Clear a single model-condition bit by index.
    pub fn clear_model_condition_state(&mut self, index: usize) {
        self.model_condition_flags.set(index, false);
    }

    /// C++ parity: `Drawable::getShadowsEnabled()`.
    pub fn get_shadows_enabled(&self) -> bool {
        self.status.has(DrawableStatus::SHADOWS)
    }

    /// C++ parity: `Drawable::setShadowsEnabled(Bool)` (Drawable.cpp:857-869).
    ///
    /// Sets DRAWABLE_STATUS_SHADOWS and dispatches to draw modules. Fail-closed:
    /// modules do not allocate GPU shadow meshes here.
    pub fn set_shadows_enabled(&mut self, enable: bool) {
        if enable {
            self.status.set(DrawableStatus::SHADOWS);
        } else {
            self.status.clear(DrawableStatus::SHADOWS);
        }
        for dm in &mut self.draw_modules {
            dm.set_shadows_enabled(enable);
        }
    }

    /// C++ parity: `Drawable::allocateShadows()` — Options screen resource create.
    ///
    /// Fail-closed residual: notifies draw modules only; does **not** set status bits
    /// (use `set_shadows_enabled`) and does **not** allocate full shadow mesh GPU resources.
    pub fn allocate_shadows(&mut self) {
        for dm in &mut self.draw_modules {
            dm.allocate_shadows();
        }
    }

    /// C++ parity: `Drawable::releaseShadows()` — Options screen resource free.
    ///
    /// Fail-closed residual: notifies draw modules only; does not clear status bits
    /// and does not free GPU meshes that were never allocated.
    pub fn release_shadows(&mut self) {
        for dm in &mut self.draw_modules {
            dm.release_shadows();
        }
    }

    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        if self.drawable_fully_obscured_by_shroud != fully_obscured {
            for draw_module in &mut self.draw_modules {
                draw_module.set_fully_obscured_by_shroud(fully_obscured);
            }
            self.drawable_fully_obscured_by_shroud = fully_obscured;
        }
    }

    /// C++ `Drawable::getFullyObscuredByShroud`.
    #[must_use]
    pub fn fully_obscured_by_shroud(&self) -> bool {
        self.drawable_fully_obscured_by_shroud
    }

    /// Current volatile C++ `Drawable::m_shroudClearFrame` value.
    ///
    /// This is read-only here: the W3D direct-scene path will become the sole
    /// writer when its candidate-dispatch parity is wired.
    #[must_use]
    pub fn shroud_clear_frame(&self) -> u32 {
        self.shroud_clear_state.clear_frame()
    }

    /// Apply the GameClient half of C++ direct-object shroud handling.
    ///
    /// This consumes a frozen raw object status, preserving the source's
    /// clear-frame grace calculation, and changes only
    /// `m_drawableFullyObscuredByShroud`.  It deliberately does not write the
    /// clear timestamp; that happens only in the later W3D scene dispatch.
    #[must_use]
    pub fn apply_frozen_direct_shroud_status(
        &mut self,
        logic_frame: u32,
        raw_status: ObjectShroudStatus,
        effectively_dead: bool,
    ) -> ClientShroudVisibility {
        let visibility = self.shroud_clear_state.evaluate_client_visibility(
            logic_frame,
            raw_status,
            effectively_dead,
        );
        self.set_fully_obscured_by_shroud(visibility.fully_obscured);
        visibility
    }

    /// Evaluate C++ `RTS3DScene::renderOneObject` shroud state once this
    /// Drawable has reached Main's frozen frustum/model candidate boundary.
    ///
    /// Unlike the GameClient visibility half above, this is the only path
    /// that refreshes `m_shroudClearFrame`. BasicDrawable owns the source
    /// exact scene-hidden predicate, so an otherwise eligible Main record
    /// still cannot refresh history when the source Drawable is hidden or
    /// hidden by stealth.  The broader Rust presentation `visible` flag does
    /// not participate in this C++ scene decision.
    #[must_use]
    pub fn evaluate_frozen_direct_scene_candidate(
        &mut self,
        logic_frame: u32,
        raw_status: ObjectShroudStatus,
        effectively_dead: bool,
    ) -> crate::drawable::SceneShroudDecision {
        self.shroud_clear_state.evaluate_scene_direct(
            logic_frame,
            raw_status,
            effectively_dead,
            self.is_scene_effectively_hidden(),
        )
    }

    /// Reset non-serialized state after a fresh drawable reconstruction/load.
    ///
    /// Keep this out of ordinary `friend_bind_to_object` rebinds: C++ retains
    /// both fields for the live Drawable in that case.
    pub(super) fn reset_volatile_shroud_state(&mut self) {
        self.shroud_clear_state.reset();
        self.set_fully_obscured_by_shroud(false);
    }

    /// Emoticon helpers (C++ parity: one active emoticon at a time).
    pub fn clear_emoticon(&mut self) {
        if let Some(icon_info) = self.icon_info.as_mut() {
            icon_info.clear_icon(IconType::Emoticon);
        }
    }

    pub fn set_emoticon(
        &mut self,
        template_name: &str,
        duration_frames: u32,
    ) -> Result<(), String> {
        let icon = Anim2DIcon::from_template_name(template_name)?;
        let current_frame = self.current_frame;
        self.get_icon_info_mut().set_icon(
            IconType::Emoticon,
            Arc::new(icon),
            duration_frames,
            current_frame,
        );
        Ok(())
    }

    /// Update cached frame for time-based drawable state
    pub fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    /// Get template name if known.
    pub fn template_name(&self) -> Option<&str> {
        self.template_name.as_deref()
    }

    /// Set template name.
    pub fn set_template_name(&mut self, name: Option<String>) {
        self.template_name = name;
    }

    /// Get owning object ID if bound.
    pub fn object_id(&self) -> Option<u32> {
        self.object_id
    }

    /// Set owning object ID.
    pub fn set_object_id(&mut self, object_id: Option<u32>) {
        match object_id {
            Some(object_id) if self.object_id != Some(object_id) => {
                self.friend_bind_to_object(object_id);
            }
            Some(_) => {}
            None => {
                self.object_id = None;
            }
        }
    }

    /// Get the object used for shroud status when this drawable has no direct object.
    pub fn shroud_status_object_id(&self) -> ObjectID {
        self.drawable_info.shroud_status_object_id
    }

    /// Set the object used for shroud status when this drawable has no direct object.
    pub fn set_shroud_status_object_id(&mut self, object_id: ObjectID) {
        self.drawable_info.shroud_status_object_id = object_id;
    }

    /// C++ `Drawable::getDrawableInfo()`.
    pub fn drawable_info(&self) -> &DrawableInfo {
        &self.drawable_info
    }

    /// C++ `Drawable::getDrawableInfo()` mutable.
    pub fn drawable_info_mut(&mut self) -> &mut DrawableInfo {
        &mut self.drawable_info
    }

    /// Flash contained objects when this drawable is selected.
    /// Matches C++ Drawable::onSelected() -> contain->clientVisibleContainedFlashAsSelected()
    pub(super) fn flash_contained_objects(&self, object_id: u32) {
        // Wave 977: host empty dual-world → presentation occupant residual only.
        // Full contained-drawable flash walk needs dual-world factory objects.
        if dual_world_registry_unavailable() {
            // Wave 984: host residual queues garrisoned presentation ids for shell flash.
            crate::core::game_client::queue_host_contained_flash_object_ids(
                self.presentation_garrisoned_ids.iter().copied(),
            );
            let _ = object_id;
            return;
        }

        // Get the object and check if it has a contain module
        use gamelogic::object::registry::OBJECT_REGISTRY;
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };

        // Check if object has a contain module with visible contained units
        let Some(contain) = obj_guard.get_contain() else {
            return;
        };

        // Flash all visible contained drawables
        // This matches C++ ContainModuleInterface::clientVisibleContainedFlashAsSelected()
        let Ok(contain_guard) = contain.lock() else {
            return;
        };
        let contained_count = contain_guard.get_contain_count();
        drop(contain_guard);

        // Wave 984: prefer contain module flash-as-selected (C++ parity).
        drop(obj_guard);
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };
        if let Some(contain) = obj_guard.get_contain() {
            if let Ok(mut contain_guard) = contain.lock() {
                let _ = contain_guard.client_visible_contained_flash_as_selected();
            }
        }
        let _ = contained_count;
    }

    /// Set expiration frame for automatic cleanup
    pub fn set_expiration_frame(&mut self, frame: u32) {
        self.expiration_frame = Some(frame);
    }

    /// Check if drawable has expired
    pub fn is_expired(&self, current_frame: u32) -> bool {
        self.expiration_frame
            .is_some_and(|frame| current_frame >= frame)
    }

    pub fn set_tint_status(&mut self, status: TintStatus) {
        self.tint_status.set(status);
    }

    pub fn clear_tint_status(&mut self, status: TintStatus) {
        self.tint_status.clear(status);
    }

    pub fn test_tint_status(&self, status: TintStatus) -> bool {
        self.tint_status.has(status)
    }

    pub fn set_terrain_decal_size(&mut self, x: f32, y: f32) {
        self.terrain_decal_size = Vector3::new(x, y, 0.0);
    }

    pub fn set_terrain_decal_fade_target(&mut self, target: f32, rate: f32) {
        if (self.decal_opacity_fade_target - target).abs() > f32::EPSILON {
            self.decal_opacity_fade_target = target;
            self.decal_opacity_fade_rate = rate;
        }
    }

    /// C++ `Drawable::fadeOut` — start at full opacity and ramp to 0 over `frames`.
    pub fn fade_out(&mut self, frames: u32) {
        self.set_opacity(1.0);
        self.fade_mode = FadingMode::FadingOut;
        self.time_elapsed_fade = 0;
        self.time_to_fade = frames.max(1);
    }

    /// C++ `Drawable::fadeIn` (Drawable.cpp:1059-1065).
    /// Sets explicit opacity to 0, remaining fade frames, and FADING_IN ramp.
    pub fn fade_in(&mut self, frames: u32) {
        self.set_opacity(0.0);
        self.fade_mode = FadingMode::FadingIn;
        self.time_elapsed_fade = 0;
        self.time_to_fade = frames.max(1);
    }

    /// C++ `Drawable::friend_getExplicitOpacity`.
    pub fn get_explicit_opacity(&self) -> f32 {
        self.explicit_opacity
    }

    /// C++ `Drawable::getEffectiveOpacity` = explicit * stealth.
    pub fn get_effective_opacity(&self) -> f32 {
        (self.explicit_opacity * self.effective_stealth_opacity).clamp(0.0, 1.0)
    }

    pub fn fading_mode(&self) -> FadingMode {
        self.fade_mode
    }

    pub fn time_to_fade(&self) -> u32 {
        self.time_to_fade
    }

    pub fn time_elapsed_fade(&self) -> u32 {
        self.time_elapsed_fade
    }

    pub fn is_fading(&self) -> bool {
        self.fade_mode != FadingMode::None
    }

    /// One C++ `Drawable::updateDrawable` fade tick. Public so tests and
    /// callers can advance the opacity ramp without a full render pass.
    pub fn update_fade(&mut self) {
        if self.fade_mode == FadingMode::None {
            return;
        }
        let numerator = if self.fade_mode == FadingMode::FadingIn {
            self.time_elapsed_fade as f32
        } else {
            (self.time_to_fade.saturating_sub(self.time_elapsed_fade)) as f32
        };
        let denom = self.time_to_fade.max(1) as f32;
        self.set_opacity((numerator / denom).clamp(0.0, 1.0));
        self.time_elapsed_fade = self.time_elapsed_fade.saturating_add(1);
        if self.time_elapsed_fade > self.time_to_fade {
            self.fade_mode = FadingMode::None;
        }
    }

    pub fn set_second_material_pass_opacity(&mut self, opacity: f32) {
        self.second_material_pass_opacity = opacity.clamp(0.0, 1.0);
    }

    pub fn set_effective_opacity(&mut self, pulse_factor: f32, explicit_opacity: Option<f32>) {
        if let Some(explicit) = explicit_opacity {
            self.stealth_opacity = explicit.clamp(0.0, 1.0);
            self.explicit_opacity = self.stealth_opacity;
        }
        let pf = pulse_factor.clamp(0.0, 1.0);
        let pulse_margin = 1.0 - self.stealth_opacity;
        let pulse_amount = pulse_margin * pf;
        self.effective_stealth_opacity = self.stealth_opacity + pulse_amount;
    }

    pub fn imitate_stealth_look(&mut self, other: &BasicDrawable) {
        self.stealth_opacity = other.stealth_opacity;
        self.explicit_opacity = other.explicit_opacity;
        self.effective_stealth_opacity = other.effective_stealth_opacity;
        self.visible = other.visible;
        self.hidden_by_stealth = other.hidden_by_stealth;
        self.stealth_look = other.stealth_look;
        self.second_material_pass_opacity = other.second_material_pass_opacity;
    }

    pub fn color_flash(&mut self, color: Vector3, flashes: u32) {
        self.flash_color = color;
        self.flash_count = flashes;
    }

    pub fn color_flash_envelope(
        &mut self,
        color: Option<Vector3>,
        decay_frames: u32,
        attack_frames: u32,
        sustain_frames: u32,
    ) {
        if self.tint_envelope.is_none() {
            self.tint_envelope = Some(TintEnvelope::new());
        }
        let color = color.unwrap_or(Vector3::new(1.0, 1.0, 1.0));
        if let Some(ref mut envelope) = self.tint_envelope {
            envelope.play(color, attack_frames, decay_frames, sustain_frames);
        }
        self.status.clear(DrawableStatus::TINT_COLOR_LOCKED);
    }

    pub fn color_tint(&mut self, color: Option<Vector3>) {
        if let Some(color) = color {
            self.color_flash_envelope(Some(color), 0, 0, 1);
            self.status.set(DrawableStatus::TINT_COLOR_LOCKED);
        } else {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.rest();
            }
            self.status.clear(DrawableStatus::TINT_COLOR_LOCKED);
        }
    }

    pub fn set_hidden_by_stealth(&mut self, hidden: bool) {
        self.hidden_by_stealth = hidden;
    }

    /// Wave 1055: stamp host control-group residual for dual group numerals.
    pub fn set_presentation_hotkey_group(&mut self, group: i8) {
        self.presentation_hotkey_group = group;
    }

    /// Wave 1115: stamp C++ OBJECT_STATUS_SOLD residual for construct-percent.
    pub fn set_presentation_sold(&mut self, sold: bool) {
        self.presentation_sold = sold;
        if sold {
            self.overlay_data.is_under_construction = false;
            self.overlay_data.construction_percent = 0.0;
        }
    }

    pub(super) fn is_object_kind_of(&self, kind: gamelogic::common::types::KindOf) -> bool {
        // Wave 270/965: host empty dual-world → presentation kind residual.
        // Fail-closed when presentation kinds were never stamped.
        if dual_world_registry_unavailable() {
            if self.presentation_kind_names.is_empty() {
                return false;
            }
            let name = format!("{kind:?}");
            return self
                .presentation_kind_names
                .iter()
                .any(|k| k == &name || k.eq_ignore_ascii_case(&name));
        }

        self.object_id.is_some_and(|obj_id| {
            OBJECT_REGISTRY
                .get_object(obj_id)
                .is_some_and(|obj_arc| obj_arc.read().is_ok_and(|obj| obj.is_kind_of(kind)))
        })
    }

    fn object_stealth_visuals(&self) -> Option<(bool, f32)> {
        // Wave 965: host empty dual-world → presentation stealth residual.
        if dual_world_registry_unavailable() {
            if self.presentation_effectively_stealthed {
                return Some((true, 0.5));
            }
            return None;
        }

        let object_id = self.object_id?;
        let obj_arc = OBJECT_REGISTRY.get_object(object_id)?;
        let stealth = obj_arc.read().ok()?.get_stealth()?;
        let stealth = stealth.lock().ok()?;
        Some((stealth.is_disguised(), stealth.get_friendly_opacity()))
    }

    /// Full stealth look logic ported from C++ Drawable::setStealthLook (Drawable.cpp:2527-2606).
    /// Sets stealth opacity, hidden-by-stealth flag, and second material pass opacity
    /// based on the stealth look type. The trait's set_stealth_look delegates here.
    pub fn apply_stealth_look(&mut self, look: StealthLook) {
        if look == self.stealth_look {
            return;
        }

        self.stealth_opacity = 1.0;
        match look {
            StealthLook::None => {
                self.hidden_by_stealth = false;
                self.second_material_pass_opacity = 0.0;
            }
            StealthLook::VisibleFriendly | StealthLook::VisibleFriendlyDetected => {
                // C++ reads TheGlobalData->m_stealthFriendlyOpacity as default opacity.
                let mut opacity: f32 = get_global_data()
                    .map(|data| data.read().stealth_friendly_opacity)
                    .unwrap_or(DEFAULT_STEALTH_FRIENDLY_OPACITY);

                if let Some((is_disguised, friendly_opacity)) = self.object_stealth_visuals() {
                    if is_disguised {
                        self.hidden_by_stealth = false;
                        self.stealth_look = look;
                        return;
                    }
                    opacity = friendly_opacity;
                }

                self.stealth_opacity = opacity;
                self.hidden_by_stealth = false;

                // C++ sets second material pass for heat-vision on detected friendlies,
                // but not on mines (evil hack per srj todo).
                if look == StealthLook::VisibleFriendlyDetected
                    && !self.is_object_kind_of(gamelogic::common::types::KindOf::Mine)
                {
                    self.second_material_pass_opacity = 1.0;
                } else {
                    self.second_material_pass_opacity = 0.0;
                }
            }
            StealthLook::DisguisedEnemy => {
                self.hidden_by_stealth = false;
                self.second_material_pass_opacity = 0.0;
            }
            StealthLook::VisibleDetected => {
                self.hidden_by_stealth = false;
                // C++ disables heat-vision on mines (same hack as above).
                if self.is_object_kind_of(gamelogic::common::types::KindOf::Mine) {
                    self.second_material_pass_opacity = 0.0;
                } else {
                    self.second_material_pass_opacity = 1.0;
                }
            }
            StealthLook::Invisible => {
                self.hidden_by_stealth = true;
                self.second_material_pass_opacity = 0.0;
            }
        }
        self.stealth_look = look;
    }

    /// Propagate indicator color to all draw modules.
    /// C++ Drawable::setIndicatorColor (Drawable.cpp:4081-4089) iterates draw modules
    /// and calls replaceIndicatorColor on each ObjectDrawInterface.
    pub fn set_indicator_color(&mut self, color: Option<(u8, u8, u8)>) {
        self.indicator_color = color;
        for dm in &mut self.draw_modules {
            dm.replace_indicator_color(color);
        }
    }

    /// Get the current indicator color.
    pub fn get_indicator_color(&self) -> Option<(u8, u8, u8)> {
        self.indicator_color
    }

    /// Bind this drawable to a game object.
    /// C++ Drawable::friend_bindToObject (Drawable.cpp:4138-4162):
    /// Sets m_object, applies indicator color (day/night aware), creates terrain
    /// decal for FS_FAKE kindof, and notifies draw modules of the binding.
    pub fn friend_bind_to_object(&mut self, object_id: u32) {
        self.object_id = Some(object_id);
        if let Some(color) = self.bound_object_indicator_color() {
            self.set_indicator_color(Some(color));
        }
        for dm in &mut self.draw_modules {
            dm.on_drawable_bound_to_object();
        }
    }

    /// Called when the owning object changes teams.
    /// C++ Drawable::changedTeam (Drawable.cpp:4168-4187):
    /// Re-applies indicator color from the object's new team and updates terrain decal.
    pub fn changed_team(&mut self) {
        if let Some(color) = self.bound_object_indicator_color() {
            self.set_indicator_color(Some(color));
        }
    }

    fn bound_object_indicator_color(&self) -> Option<(u8, u8, u8)> {
        // Wave 965: host empty dual-world → presentation team color residual.
        if dual_world_registry_unavailable() {
            return self.presentation_indicator_color;
        }

        let object_id = self.object_id?;
        let object_arc = OBJECT_REGISTRY.get_object(object_id)?;
        let object = object_arc.read().ok()?;
        let use_night_color = get_global_data()
            .map(|data| data.read().time_of_day)
            .is_some_and(|time_of_day| matches!(time_of_day, IniTimeOfDay::Night));
        let color = if use_night_color {
            object.get_night_indicator_color()
        } else {
            object.get_indicator_color()
        };
        Some((color.r, color.g, color.b))
    }
}
