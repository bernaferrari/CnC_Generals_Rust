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
use gamelogic::helpers::{BoneOverrideState, ModelDrawState, TheGameClient, TheGameLogic};
use gamelogic::scripting::{TFade, get_script_engine};

use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{NO_HOTKEY_SQUAD, NUM_HOTKEY_SQUADS, Player};
use parking_lot::Mutex;
use std::error::Error;
use std::sync::Arc;

impl BasicDrawable {
    // ---------------------------------------------------------------------------
    // 2D icon overlay methods (matches C++ Drawable.cpp drawIconUI, drawHealthBar,
    // drawVeterancy, drawConstructPercent, drawCaption, computeHealthRegion)
    //
    // These methods compute overlay data and store it in self.overlay_data.
    // The actual GPU rendering is handled by the render pipeline later.
    // ---------------------------------------------------------------------------

    /// C++ parity: free function `computeHealthRegion` (Drawable.cpp:2661-2704).
    ///
    /// Projects the object's health-box anchor through the tactical view and
    /// scales width by 1/zoom. Returns `None` when the drawable has no object,
    /// the object is IgnoredInGui (zero dimensions), or world→screen fails.
    ///
    /// Fail-closed residual: uses live object + tactical view when available;
    /// falls back to a previously cached region only when projection is unavailable
    /// but a region was seeded (test / offline icon UI path).
    pub fn compute_health_region(&self) -> Option<IRegion2D> {
        // A live presentation region is output from this draw, never a seeded
        // fallback. C++ recomputes it for the current camera on every draw.
        if dual_world_registry_unavailable() && self.presentation_health_box.is_some() {
            return self.compute_health_region_from_presentation_pose();
        }
        if let Some(region) = self.compute_health_region_from_object() {
            return Some(region);
        }
        // Offline / test path: honor an explicitly seeded region (matches prior stub).
        self.overlay_data.health_region
    }

    fn compute_health_region_from_object(&self) -> Option<IRegion2D> {
        // Wave 977: host empty dual-world → compute screen region from drawable pose residual.
        if dual_world_registry_unavailable() {
            if let Some(region) = self.overlay_data.health_region {
                return Some(region);
            }
            return self.compute_health_region_from_presentation_pose();
        }

        let obj_id = self.object_id?;
        let region = OBJECT_REGISTRY.with_object(obj_id, |obj| {
            let (health_box_height, health_box_width) = obj.get_health_box_dimensions();
            if health_box_width <= 0.0 || health_box_height <= 0.0 {
                return None;
            }
            let world = obj.get_health_box_position();
            Some(Point3::new(world.x, world.y, world.z))
                .map(|world_pt| (world_pt, health_box_width))
        })??;
        let (world_pt, health_box_width) = region;
        Self::health_region_from_world_point(world_pt, health_box_width)
    }

    /// Wave 977: host presentation pose → health bar from C++ health box geometry.
    fn compute_health_region_from_presentation_pose(&self) -> Option<IRegion2D> {
        if self
            .presentation_kind_names
            .iter()
            .any(|k| k.eq_ignore_ascii_case("IgnoredInGui"))
        {
            return None;
        }
        let pos = self.position;
        let (width, z_off) = self.presentation_health_box?;
        if width <= 0.0 {
            return None;
        }
        let world_pt = Point3::new(pos.x, pos.y, pos.z + z_off);
        Self::health_region_from_world_point(world_pt, width)
    }

    fn health_region_from_world_point(
        world_pt: Point3,
        mut health_box_width: f32,
    ) -> Option<IRegion2D> {
        let (screen_center, zoom) = with_tactical_view_ref(|view| {
            let screen = view.world_to_screen(&world_pt)?;
            Some((screen, view.zoom()))
        })?;

        // C++: widthScale = 1.0f / zoom; height forced to 3.0 after scale.
        let zoom = if zoom.abs() < f32::EPSILON { 1.0 } else { zoom };
        let width_scale = 1.0 / zoom;
        health_box_width *= width_scale;
        let health_box_height = 3.0_f32;

        let lo_x = (screen_center.x as f32 - health_box_width * 0.45).round() as i32;
        let lo_y = (screen_center.y as f32 - health_box_height * 0.5).round() as i32;
        let hi_x = lo_x + health_box_width.round() as i32;
        let hi_y = lo_y + health_box_height.round() as i32;

        Some(IRegion2D::new(
            ICoord2D::new(lo_x, lo_y),
            ICoord2D::new(hi_x, hi_y),
        ))
    }

    fn draw_health_bar(&mut self, health_region: &IRegion2D) {
        // C++ Drawable::drawHealthBar (`Drawable.cpp:3825-3937`).
        self.overlay_data.health_region = Some(*health_region);
        self.overlay_data.health_bar_visible = false;

        if !self.show_object_health_enabled() || !self.selected_or_moused_over_for_icon_pips() {
            return;
        }
        if self.is_object_kind_of(gamelogic::common::types::KindOf::ForceAttackable) {
            return;
        }

        let use_presentation = dual_world_registry_unavailable() || self.object_id.is_none();
        let (health, max_health, under_construction, disabled_not_held) = if use_presentation {
            // Wave 1114: dual health-bar residual fail-closed on dead presentation.
            if self.presentation_health_pct <= 0.0 {
                return;
            }
            (
                self.presentation_health_pct,
                1.0,
                self.presentation_under_construction,
                self.presentation_disabled,
            )
        } else {
            let Some(obj_id) = self.object_id else {
                return;
            };
            let Some(stats) = OBJECT_REGISTRY.with_object(obj_id, |obj| {
                let health = obj.get_health();
                let max_health = obj.get_max_health();
                use gamelogic::common::types::DisabledType;
                let disabled_not_held =
                    obj.is_disabled() && !obj.is_disabled_by_type(DisabledType::Held);
                (
                    health,
                    max_health,
                    obj.is_under_construction(),
                    disabled_not_held,
                )
            }) else {
                return;
            };
            let (health, max_health, under_construction, disabled_not_held) = stats;
            if max_health == 0.0 || health == 0.0 {
                return;
            }
            (health, max_health, under_construction, disabled_not_held)
        };

        let ratio = (health / max_health).clamp(0.0, 1.0);
        let really_damaged = self
            .model_condition_flags
            .test(ModelConditionFlags::REALLYDAMAGED);
        let damaged = self
            .model_condition_flags
            .test(ModelConditionFlags::DAMAGED);
        let (fill, outline) = health_bar_colors(
            ratio,
            under_construction || disabled_not_held,
            really_damaged,
            damaged,
        );
        self.overlay_data.health_ratio = ratio;
        self.overlay_data.health_fill = fill;
        self.overlay_data.health_outline = outline;
        self.overlay_data.health_bar_visible = true;
        self.overlay_data.visible = true;
    }

    /// C++ `Drawable::s_veterancyImage[level]` (`Drawable.cpp:254-257`).
    /// Regular (0) has no chevron; Veteran/Elite/Heroic use SCVeter1-3.
    pub fn veterancy_image_name(level: u8) -> Option<&'static str> {
        match level {
            1 => Some("SCVeter1"),
            2 => Some("SCVeter2"),
            0 => None,
            _ => Some("SCVeter3"),
        }
    }

    fn draw_veterancy(&mut self, _health_region: &IRegion2D) {
        // Wave 970: host empty dual-world → presentation veterancy residual.
        if dual_world_registry_unavailable() {
            // Wave 1114: dual veterancy residual fail-closed on dead presentation.
            if self.presentation_health_pct <= 0.0 {
                self.overlay_data.veterancy_level = 0;
                return;
            }
            self.overlay_data.veterancy_level = self.presentation_veterancy_level;
            return;
        }

        if let Some(obj_id) = self.object_id {
            let level = OBJECT_REGISTRY.with_object(obj_id, |obj| {
                if obj.get_experience_tracker().is_some() {
                    Some(obj.get_veterancy_level() as u8)
                } else {
                    None
                }
            });
            if level.is_none() {
                return;
            }
            if let Some(level) = level.flatten() {
                self.overlay_data.veterancy_level = level;
            }
        }
    }

    fn draw_construct_percent(&mut self, _health_region: &IRegion2D) {
        // Wave 970: host empty dual-world → presentation construction residual.
        // C++ Drawable::drawConstructPercent (Drawable.cpp:3672-3732) bails when
        // there is no object, not OBJECT_STATUS_UNDER_CONSTRUCTION, or
        // OBJECT_STATUS_SOLD. The isEffectivelyDead check is commented out.
        if dual_world_registry_unavailable() {
            // Wave 1115: dual construct residual fail-closed on sold /
            // not-under-construction (C++ OBJECT_STATUS_SOLD), not on dead health.
            if self.presentation_sold || !self.presentation_under_construction {
                self.overlay_data.is_under_construction = false;
                self.overlay_data.construction_percent = 0.0;
                self.overlay_data.construct_text = None;
                return;
            }
            self.overlay_data.is_under_construction = true;
            self.overlay_data.construction_percent = self.presentation_construction_percent;
            self.overlay_data.construct_text = Some(format_under_construction_desc(
                self.presentation_construction_percent,
            ));
            self.overlay_data.visible = true;
            return;
        }

        if let Some(obj_id) = self.object_id {
            let state = OBJECT_REGISTRY.with_object(obj_id, |obj| {
                if obj.test_status(gamelogic::common::ObjectStatusTypes::Sold)
                    || !obj.is_under_construction()
                {
                    None
                } else {
                    Some((obj.get_construction_percent() as f32) / 100.0)
                }
            });
            let Some(state) = state else {
                self.overlay_data.is_under_construction = false;
                self.overlay_data.construction_percent = 0.0;
                self.overlay_data.construct_text = None;
                return;
            };
            match state {
                None => {
                    self.overlay_data.is_under_construction = false;
                    self.overlay_data.construction_percent = 0.0;
                    self.overlay_data.construct_text = None;
                }
                Some(percent) => {
                    self.overlay_data.is_under_construction = true;
                    self.overlay_data.construction_percent = percent;
                    self.overlay_data.construct_text =
                        Some(format_under_construction_desc(percent));
                    self.overlay_data.visible = true;
                }
            }
        }
    }

    pub fn draw_caption(&mut self, _health_region: &IRegion2D) {
        if let Some(caption) = self.caption_text.as_ref() {
            self.overlay_data.caption = Some(caption.clone());
            self.overlay_data.caption_world =
                Some([self.position.x, self.position.y, self.position.z]);
            self.overlay_data.visible = true;
        } else {
            self.overlay_data.caption = None;
            self.overlay_data.caption_world = None;
        }
    }

    pub fn draw_emoticon(&mut self, _health_region: &IRegion2D) {
        // C++ parity: Drawable.cpp drawEmoticon (lines 2826-2857)
        if let Some(ref icon_info) = self.icon_info {
            let now = self.current_frame;
            if icon_info.icons.contains_key(&IconType::Emoticon) {
                let active = icon_info
                    .keep_till_frame
                    .get(&IconType::Emoticon)
                    .is_some_and(|&frame| frame >= now);
                self.overlay_data.show_emoticon = active;
                if !active {
                    self.clear_emoticon();
                }
            }
        }
    }

    pub(super) fn selected_or_moused_over_for_icon_pips(&self) -> bool {
        // Wave 972: host path also honors presentation_selected residual.
        self.selected
            || self.presentation_selected
            || (self.id != DrawableId::INVALID
                && TheInGameUI::get_moused_over_drawable_id() == self.id.0)
    }

    fn show_object_health_enabled(&self) -> bool {
        get_global_data()
            .map(|data| data.read().show_object_health)
            .unwrap_or(false)
    }

    fn icon_ui_allowed(&self) -> bool {
        // C++ Drawable::drawIconUI (`Drawable.cpp:2740`).
        if !TheGameLogic::get_draw_icon_ui() {
            return false;
        }
        let fade = get_script_engine()
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().map(|engine| engine.get_fade()))
            .unwrap_or(TFade::None);
        fade == TFade::None
    }

    fn is_local_player_object(&self) -> bool {
        if dual_world_registry_unavailable() {
            // Host residual: selected/moused units are the local player's chrome.
            return self.selected_or_moused_over_for_icon_pips();
        }
        let Some(obj_id) = self.object_id else {
            return false;
        };
        OBJECT_REGISTRY
            .with_object(obj_id, |obj| obj.is_locally_controlled())
            .unwrap_or(false)
    }

    fn pip_icon_gates_pass(&self) -> bool {
        // C++ drawAmmo/drawContained (`Drawable.cpp:2865-2870`, `:2923-2928`).
        self.show_object_health_enabled()
            && self.selected_or_moused_over_for_icon_pips()
            && self.is_local_player_object()
    }

    /// C++ `Drawable::drawsAnyUIText` (`Drawable.cpp:2709-2729`).
    pub fn draws_any_ui_text(&self) -> bool {
        if !self.selected && !self.presentation_selected {
            return false;
        }
        if dual_world_registry_unavailable() || self.object_id.is_none() {
            let group = self.presentation_hotkey_group as i32;
            if group > NO_HOTKEY_SQUAD && group < NUM_HOTKEY_SQUADS as i32 {
                return true;
            }
            return self.presentation_formation_id != 0;
        }
        let Some(obj_id) = self.object_id else {
            return false;
        };
        let ui = OBJECT_REGISTRY.with_object(obj_id, |obj| {
            let local = obj.is_locally_controlled();
            let player = obj.get_controlling_player();
            let id = obj.get_id();
            let formation = obj.get_formation_id();
            (local, player, id, formation)
        });
        let Some((local, player, id, formation)) = ui else {
            return false;
        };
        if !local {
            return false;
        }
        if let Some(player_arc) = player {
            if let Ok(mut player_guard) = player_arc.write() {
                if let Some(group_number) = Self::find_hotkey_squad_number(&mut player_guard, id) {
                    if group_number > NO_HOTKEY_SQUAD && group_number < NUM_HOTKEY_SQUADS as i32 {
                        return true;
                    }
                }
            }
        }
        formation != FormationID::NONE
    }

    fn queue_ui_text_overlay(&mut self) {
        self.overlay_data.queue_ui_text = false;
        self.overlay_data.group_numeral = None;
        self.overlay_data.formation_letter = None;
        if !self.draws_any_ui_text() {
            return;
        }
        self.overlay_data.queue_ui_text = true;
        if dual_world_registry_unavailable() || self.object_id.is_none() {
            let group = self.presentation_hotkey_group as i32;
            if group > NO_HOTKEY_SQUAD && group < NUM_HOTKEY_SQUADS as i32 {
                self.overlay_data.group_numeral = Some(format!("{group}"));
            }
            if self.presentation_formation_id != 0 {
                self.overlay_data.formation_letter = Some("F".to_string());
            }
            return;
        }
        let Some(obj_id) = self.object_id else {
            return;
        };
        let ui = OBJECT_REGISTRY.with_object(obj_id, |obj| {
            let player = obj.get_controlling_player();
            let id = obj.get_id();
            let formation = obj.get_formation_id();
            (player, id, formation)
        });
        let Some((player, id, formation)) = ui else {
            return;
        };
        if let Some(player_arc) = player {
            if let Ok(mut player_guard) = player_arc.write() {
                if let Some(group_number) = Self::find_hotkey_squad_number(&mut player_guard, id) {
                    if group_number > NO_HOTKEY_SQUAD && group_number < NUM_HOTKEY_SQUADS as i32 {
                        self.overlay_data.group_numeral = Some(format!("{group_number}"));
                    }
                }
            }
        }
        if formation != FormationID::NONE {
            self.overlay_data.formation_letter = Some("F".to_string());
        }
    }

    fn clear_icon_ui_overlay(&mut self) {
        self.overlay_data.visible = false;
        self.overlay_data.health_bar_visible = false;
        self.overlay_data.show_ammo = false;
        self.overlay_data.show_contained = false;
        self.overlay_data.show_healing = false;
        self.overlay_data.show_disabled = false;
        self.overlay_data.show_enthusiastic = false;
        self.overlay_data.show_bombed = false;
        self.overlay_data.show_emoticon = false;
        self.overlay_data.queue_ui_text = false;
        self.overlay_data.group_numeral = None;
        self.overlay_data.formation_letter = None;
        self.overlay_data.construct_text = None;
        self.overlay_data.is_under_construction = false;
    }

    fn mark_overlay_visible_if_any_chrome(&mut self) {
        let o = &self.overlay_data;
        if o.health_bar_visible
            || o.is_under_construction
            || o.show_ammo
            || o.show_contained
            || o.show_healing
            || o.show_disabled
            || o.show_enthusiastic
            || o.show_bombed
            || o.show_emoticon
            || o.queue_ui_text
            || o.veterancy_level > 0
            || o.caption.as_ref().is_some_and(|c| !c.is_empty())
        {
            self.overlay_data.visible = true;
        }
    }

    pub fn draw_ammo(&mut self, _health_region: &IRegion2D) {
        // C++ Drawable::drawAmmo (`Drawable.cpp:2865-2870`).
        if !self.pip_icon_gates_pass() {
            self.overlay_data.show_ammo = false;
            return;
        }
        if dual_world_registry_unavailable() {
            // Wave 1114: dual ammo residual fail-closed on dead presentation.
            if self.presentation_health_pct <= 0.0 || self.presentation_effectively_stealthed {
                self.overlay_data.show_ammo = false;
                return;
            }
            if self.presentation_ammo_pip_total == 0 {
                self.overlay_data.show_ammo = false;
                return;
            }
            self.overlay_data.ammo_total = self.presentation_ammo_pip_total;
            self.overlay_data.ammo_full = self.presentation_ammo_pip_full;
            self.overlay_data.show_ammo = true;
            return;
        }

        // C++ gates on: TheGlobalData->m_showObjectHealth && (isSelected() || mousedOver)
        //              && obj->getControllingPlayer() == ThePlayerList->getLocalPlayer()
        if !self.selected_or_moused_over_for_icon_pips() {
            self.overlay_data.show_ammo = false;
            return;
        }

        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some((total, full)) =
            OBJECT_REGISTRY.with_object(obj_id, |obj| obj.get_ammo_pip_info())
        else {
            return;
        };
        // C++ calls obj->getAmmoPipShowingInfo(numTotal, numFull).
        // For parity, we store the ammo state for the render pipeline.
        if total == 0 {
            self.overlay_data.show_ammo = false;
            return;
        }
        self.overlay_data.ammo_total = total as u8;
        self.overlay_data.ammo_full = full as u8;
        self.overlay_data.show_ammo = true;
    }

    pub fn draw_contained(&mut self, _health_region: &IRegion2D) {
        // C++ Drawable::drawContained (`Drawable.cpp:2923-2928`).
        if !self.pip_icon_gates_pass() {
            self.overlay_data.show_contained = false;
            return;
        }
        if dual_world_registry_unavailable() {
            // Wave 1114: dual contain residual fail-closed on dead presentation.
            if self.presentation_health_pct <= 0.0 || self.presentation_effectively_stealthed {
                self.overlay_data.show_contained = false;
                return;
            }
            if self.presentation_max_garrison == 0 {
                self.overlay_data.show_contained = false;
                return;
            }
            self.overlay_data.contained_total = self.presentation_max_garrison;
            self.overlay_data.contained_full = self
                .presentation_occupant_count
                .min(self.presentation_max_garrison);
            self.overlay_data.contained_infantry_count = self.overlay_data.contained_full;
            self.overlay_data.show_contained = true;
            return;
        }

        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some(contain_arc) = OBJECT_REGISTRY.with_object(obj_id, |obj| obj.get_contain()) else {
            return;
        };
        let Some(contain_arc) = contain_arc else {
            self.overlay_data.show_contained = false;
            return;
        };
        let Ok(contain_guard) = contain_arc.lock() else {
            return;
        };

        let (num_total, num_full, show_pips) = contain_guard.get_container_pips_to_show();
        if !show_pips || num_full == 0 {
            self.overlay_data.show_contained = false;
            return;
        }

        self.overlay_data.contained_full = num_full.max(0).min(u8::MAX as i32) as u8;
        self.overlay_data.contained_total = num_total.max(0).min(u8::MAX as i32) as u8;
        self.overlay_data.show_contained = true;

        let contained_objects = contain_guard.get_contained_objects();
        drop(contain_guard);
        let mut infantry_count: u8 = 0;
        for cid in contained_objects {
            if OBJECT_REGISTRY
                .with_object(cid, |c| c.is_kind_of(gamelogic::common::types::KindOf::Infantry))
                .unwrap_or(false)
            {
                infantry_count = infantry_count.saturating_add(1);
            }
        }
        self.overlay_data.contained_infantry_count = infantry_count;
    }

    pub fn draw_healing(&mut self, _health_region: &IRegion2D) {
        // Wave 983: host empty dual-world → presentation healing residual.
        if dual_world_registry_unavailable() {
            // Wave 1114: dual healing residual fail-closed on dead presentation.
            if self.presentation_health_pct <= 0.0 {
                self.overlay_data.show_healing = false;
                self.overlay_data.healing_icon_type = 0;
                return;
            }
            self.overlay_data.show_healing = self.presentation_show_healing;
            self.overlay_data.healing_icon_type = self.presentation_healing_icon_type;
            return;
        }

        // C++ parity: Drawable.cpp drawHealing (lines 3212-3301)
        // Shows healing icon when last healing was within HEALING_ICON_DISPLAY_TIME (90 frames = 3s).
        const HEALING_ICON_DISPLAY_TIME: u32 = 90; // 3 seconds at 30 FPS

        let Some(obj_id) = self.object_id else {
            return;
        };
        let now = self.current_frame;
        let Some(snap) = OBJECT_REGISTRY.with_object(obj_id, |obj| {
            (
                obj.is_kind_of(gamelogic::common::types::KindOf::NoHealIcon),
                obj.get_body_module(),
                obj.is_kind_of(gamelogic::common::types::KindOf::Structure),
                obj.is_kind_of(gamelogic::common::types::KindOf::Vehicle),
            )
        }) else {
            return;
        };
        let (no_heal, body, is_structure, is_vehicle) = snap;
        if no_heal {
            self.overlay_data.show_healing = false;
            return;
        }

        let mut show_healing = false;
        if let Some(body_arc) = body {
            if let Ok(body_guard) = body_arc.lock() {
                let health = body_guard.get_health();
                let max_health = body_guard.get_max_health();
                if health != max_health {
                    let last_heal = body_guard.get_last_healing_timestamp();
                    if now > HEALING_ICON_DISPLAY_TIME
                        && now.saturating_sub(last_heal) <= HEALING_ICON_DISPLAY_TIME
                    {
                        show_healing = true;
                    }
                }
            }
        }

        self.overlay_data.show_healing = show_healing;

        if show_healing {
            if is_structure {
                self.overlay_data.healing_icon_type = 1;
            } else if is_vehicle {
                self.overlay_data.healing_icon_type = 2;
            } else {
                self.overlay_data.healing_icon_type = 0;
            }
        } else if let Some(icon_info) = &mut self.icon_info {
            icon_info.clear_icon(IconType::DefaultHeal);
            icon_info.clear_icon(IconType::StructureHeal);
            icon_info.clear_icon(IconType::VehicleHeal);
        }
    }

    pub fn draw_enthusiastic(&mut self, _health_region: &IRegion2D) {
        // Wave 972: host empty dual-world → presentation enthusiastic residual.
        if dual_world_registry_unavailable() {
            // Wave 1114: dual enthusiastic residual fail-closed on dead presentation.
            if self.presentation_health_pct <= 0.0 {
                self.overlay_data.show_enthusiastic = false;
                return;
            }
            self.overlay_data.show_enthusiastic = self.presentation_weapon_bonus_enthusiastic;
            return;
        }

        // C++ parity: Drawable.cpp drawEnthusiastic (lines 3306-3373)
        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some((has_enthusiastic, has_subliminal)) =
            OBJECT_REGISTRY.with_object(obj_id, |obj| {
                use gamelogic::common::types::WeaponBonusConditionFlags;
                let bonus = obj.get_weapon_bonus_condition();
                (
                    bonus.contains(WeaponBonusConditionFlags::ENTHUSIASTIC),
                    bonus.contains(WeaponBonusConditionFlags::SUBLIMINAL),
                )
            })
        else {
            return;
        };

        if has_enthusiastic {
            self.overlay_data.show_enthusiastic = true;
            self.overlay_data.show_subliminal = has_subliminal;
        } else {
            self.overlay_data.show_enthusiastic = false;
            self.overlay_data.show_subliminal = false;
            if let Some(ref mut icon_info) = self.icon_info {
                icon_info.clear_icon(IconType::Enthusiastic);
                icon_info.clear_icon(IconType::EnthusiasticSubliminal);
            }
        }
    }

    pub fn draw_demoralized(&mut self, _health_region: &IRegion2D) {
        // Wave 987: C++ ALLOW_DEMORALIZE is off in retail Zero Hour.
        // TheWeaponBonusNames uses DEMORALIZED_OBSOLETE; demoralized icon residual
        // is fail-closed on both host empty dual-world and dual-world registry paths.
        // (Drawable.cpp drawDemoralized is #ifdef ALLOW_DEMORALIZE.)
        let _ = (
            dual_world_registry_unavailable(),
            self.object_id,
            _health_region,
        );
        self.overlay_data.show_demoralized = false;
    }

    pub fn draw_bombed(&mut self, _health_region: &IRegion2D) {
        // Wave 972: host empty dual-world → presentation carbomb residual.
        if dual_world_registry_unavailable() {
            // Wave 1114: dual bombed residual fail-closed on dead presentation.
            if self.presentation_health_pct <= 0.0 {
                self.overlay_data.show_bombed = false;
                self.overlay_data.bomb_type = 0;
                self.overlay_data.bomb_timer_seconds = 0;
                return;
            }
            if self.presentation_is_carbomb {
                self.overlay_data.show_bombed = true;
                self.overlay_data.bomb_type = 3;
                self.overlay_data.bomb_timer_seconds = 0;
            } else if self.presentation_bomb_type != 0 {
                self.overlay_data.show_bombed = true;
                self.overlay_data.bomb_type = self.presentation_bomb_type;
                self.overlay_data.bomb_timer_seconds = self.presentation_bomb_timer_seconds;
            } else {
                self.overlay_data.show_bombed = false;
                self.overlay_data.bomb_type = 0;
                self.overlay_data.bomb_timer_seconds = 0;
            }
            return;
        }

        // C++ parity: Drawable.cpp drawBombed (lines 3435-3609)
        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some((carbomb, sticky)) = OBJECT_REGISTRY.with_object(obj_id, |obj| {
            let carbomb = obj.test_weapon_set_flag(gamelogic::weapon::WeaponSetType::CarBomb)
                && obj.is_locally_controlled();
            let sticky = obj.find_update_module("StickyBombUpdate").and_then(|handle| {
                handle.with_module(|module| {
                    module
                        .get_sticky_bomb_control_interface()
                        .and_then(|sticky| {
                            if sticky.get_target() == INVALID_ID {
                                return None;
                            }
                            Some((sticky.is_timed_bomb(), sticky.get_detonation_frame()))
                        })
                })
            });
            (carbomb, sticky)
        }) else {
            return;
        };
        // C++ WEAPONSET_CARBOMB && controllingPlayer == localPlayer.
        if carbomb {
            self.overlay_data.show_bombed = true;
            self.overlay_data.bomb_type = 3; // car bomb
            self.overlay_data.bomb_timer_seconds = 0;
            return;
        }
        if let Some((timed, die_frame)) = sticky {
            self.overlay_data.show_bombed = true;
            if timed {
                self.overlay_data.bomb_type = 1;
                let now = TheGameLogic::get_frame();
                let remaining = die_frame.saturating_sub(now);
                self.overlay_data.bomb_timer_seconds = ((remaining as f32) / 30.0).ceil() as u32;
            } else {
                self.overlay_data.bomb_type = 2;
                self.overlay_data.bomb_timer_seconds = 0;
            }
        } else {
            self.overlay_data.show_bombed = false;
            self.overlay_data.bomb_type = 0;
            self.overlay_data.bomb_timer_seconds = 0;
            if let Some(icon_info) = &mut self.icon_info {
                let now = self.current_frame;
                let expired_timed = icon_info
                    .keep_till_frame
                    .get(&IconType::BombTimed)
                    .is_none_or(|&f| f <= now);
                let expired_remote = icon_info
                    .keep_till_frame
                    .get(&IconType::BombRemote)
                    .is_none_or(|&f| f <= now);
                if expired_timed {
                    icon_info.clear_icon(IconType::BombTimed);
                }
                if expired_remote {
                    icon_info.clear_icon(IconType::BombRemote);
                }
            }
        }
    }

    pub fn draw_disabled(&mut self, _health_region: &IRegion2D) {
        // Wave 972: host empty dual-world → presentation disabled residual.
        if dual_world_registry_unavailable() {
            // Wave 1114: dual disabled residual fail-closed on dead presentation.
            if self.presentation_health_pct <= 0.0 {
                self.overlay_data.show_disabled = false;
                return;
            }
            self.overlay_data.show_disabled = self.presentation_disabled;
            return;
        }

        // C++ parity: Drawable.cpp drawDisabled (lines 3614-3667)
        // Checks: DISABLED_HACKED || DISABLED_PARALYZED || DISABLED_EMP ||
        //         DISABLED_SUBDUED || DISABLED_UNDERPOWERED
        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some(is_disabled) = OBJECT_REGISTRY.with_object(obj_id, |obj| {
            use gamelogic::common::types::DisabledType;
            obj.is_disabled_by_type(DisabledType::DisabledHacked)
                || obj.is_disabled_by_type(DisabledType::Paralyzed)
                || obj.is_disabled_by_type(DisabledType::DisabledEmp)
                || obj.is_disabled_by_type(DisabledType::DisabledSubdued)
                || obj.is_disabled_by_type(DisabledType::DisabledUnderpowered)
        }) else {
            return;
        };

        self.overlay_data.show_disabled = is_disabled;

        if !is_disabled {
            if let Some(ref mut icon_info) = self.icon_info {
                icon_info.clear_icon(IconType::Disabled);
            }
        }
    }

    pub fn draw_icon_ui(&mut self) {
        // C++ `setStealthLook` is independent of drawIconUI. Friendly cloak
        // stays VisibleFriendly (translucent); only Invisible hides.
        if dual_world_registry_unavailable() {
            let look = if self.hidden_by_stealth {
                StealthLook::Invisible
            } else if self.presentation_effectively_stealthed {
                StealthLook::VisibleFriendly
            } else {
                StealthLook::None
            };
            self.apply_stealth_look(look);
        }

        // C++ Drawable::drawIconUI (`Drawable.cpp:2738-2788`).
        if !self.icon_ui_allowed() {
            self.clear_icon_ui_overlay();
            return;
        }

        let region = self.compute_health_region();
        if region.is_none() && self.presentation_health_box.is_some() {
            self.overlay_data.health_region = None;
            self.clear_icon_ui_overlay();
            return;
        }

        if dual_world_registry_unavailable()
            && self.presentation_effectively_stealthed
            && !self.selected_or_moused_over_for_icon_pips()
        {
            self.clear_icon_ui_overlay();
            return;
        }
        if let Some(health_region) = &region {
            self.draw_health_bar(health_region);
            self.draw_emoticon(health_region);
            self.draw_caption(health_region);
            self.draw_construct_percent(health_region);
        }

        let is_dead = if dual_world_registry_unavailable() {
            self.presentation_health_pct <= 0.0
                || self
                    .presentation_kind_names
                    .iter()
                    .any(|k| k == "IgnoredInGui" || k.eq_ignore_ascii_case("ignoredingui"))
        } else {
            let Some(obj_id) = self.object_id else {
                self.mark_overlay_visible_if_any_chrome();
                return;
            };
            let Some(dead) = OBJECT_REGISTRY.with_object(obj_id, |obj| {
                obj.is_effectively_dead()
                    || obj.is_kind_of(gamelogic::common::types::KindOf::IgnoredInGui)
            }) else {
                self.mark_overlay_visible_if_any_chrome();
                return;
            };
            dead
        };

        if is_dead {
            self.mark_overlay_visible_if_any_chrome();
            return;
        }

        self.queue_ui_text_overlay();
        if let Some(health_region) = &region {
            self.draw_healing(health_region);
            self.draw_bombed(health_region);
            self.draw_enthusiastic(health_region);
            self.draw_demoralized(health_region);
            self.draw_disabled(health_region);
            self.draw_ammo(health_region);
            self.draw_contained(health_region);
            self.draw_veterancy(health_region);
        }
        self.mark_overlay_visible_if_any_chrome();
    }
}

#[cfg(test)]
mod hud_stealth_veterancy_tests {
    use super::super::{BasicDrawable, Drawable, DrawableId, StealthLook};

    #[test]
    fn veterancy_image_names_are_scveter() {
        assert_eq!(BasicDrawable::veterancy_image_name(0), None);
        assert_eq!(BasicDrawable::veterancy_image_name(1), Some("SCVeter1"));
        assert_eq!(BasicDrawable::veterancy_image_name(2), Some("SCVeter2"));
        assert_eq!(BasicDrawable::veterancy_image_name(3), Some("SCVeter3"));
    }

    #[test]
    fn friendly_stealth_look_is_translucent_not_hidden() {
        game_engine::common::ini::init_global_data();
        let mut drawable = BasicDrawable::new(DrawableId(42));
        drawable.set_presentation_host_residual(
            std::sync::Arc::new(Vec::new()),
            None,
            true,
            false,
            1.0,
            true,
            2,
            false,
            0.0,
            0,
            0,
            0,
            0,
            false,
            false,
            0,
            0,
            false,
            0.0,
            false,
            0,
            Vec::new(),
            String::new(),
            0,
            0,
            String::new(),
        );
        drawable.draw_icon_ui();
        assert_eq!(drawable.get_stealth_look(), StealthLook::VisibleFriendly);
        assert!(!drawable.is_effectively_hidden());
        assert_eq!(
            BasicDrawable::veterancy_image_name(drawable.overlay_data.veterancy_level),
            Some("SCVeter2")
        );
    }
}
