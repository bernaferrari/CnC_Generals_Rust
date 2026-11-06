use super::*;

/// Immutable observation at native ShowOptions, never a mutable input authority.
#[derive(Clone, Copy, Debug)]
pub struct OptionsCameraInputObservation {
    pub scroll_factor: f32,
    pub default_scroll_factor: f32,
    pub save_camera: bool,
    pub use_camera: bool,
}
impl OptionsCameraInputObservation {
    pub fn observe(
        rules: game_engine::common::ini::ini_game_data::CameraInputRules,
        preferences: &UserPreferences,
    ) -> Self {
        Self {
            scroll_factor: rules.keyboard_scroll_factor(preferences),
            default_scroll_factor: rules.keyboard_default_scroll_factor,
            save_camera: preferences.save_camera_in_replays(),
            use_camera: preferences.use_camera_in_replays(),
        }
    }
}
/// Apply only to windows from the actual admitted receiving layout.
pub(super) fn set_owned_options_scroll(layout: &WindowLayout, value: f32, range: bool) {
    let id = game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
        "OptionsMenu.wnd:SliderScrollSpeed",
    ) as i32;
    if let Some(window) = layout
        .windows()
        .iter()
        .find(|window| window.borrow().get_id() == id)
    {
        if let Some(slider) = window.borrow_mut().horizontal_slider_mut() {
            if range {
                slider.set_range(0, 100);
            }
            slider.set_value((value * 100.0) as i32);
        }
    }
}
/// Options Menu implementation
pub struct OptionsMenu {
    pub(super) residual_action: ResidualOptionsMenuAction,
    pub(super) pending_host_apply: Option<HostOptionsApply>,
    pub(super) initialized: bool,
    pub(super) ignore_selected: bool,
    pub(super) parent_id: i32,
    pub(super) button_back_id: i32,
    pub(super) button_defaults_id: i32,
    pub(super) button_accept_id: i32,
    pub(super) button_keyboard_options_id: i32,
    pub(super) button_advanced_accept_id: i32,
    pub(super) button_advanced_back_id: i32,
    pub(super) combo_anti_aliasing_id: i32,
    pub(super) combo_resolution_id: i32,
    pub(super) combo_detail_id: i32,
    pub(super) check_alternate_mouse_id: i32,
    pub(super) check_retaliation_id: i32,
    pub(super) check_double_click_attack_move_id: i32,
    pub(super) check_language_filter_id: i32,
    pub(super) check_send_delay_id: i32,
    pub(super) combo_lan_ip_id: i32,
    pub(super) combo_online_ip_id: i32,
    pub(super) button_firewall_refresh_id: i32,
    pub(super) check_use_camera_id: i32,
    pub(super) check_save_camera_id: i32,
    pub(super) check_draw_anchor_id: i32,
    pub(super) check_move_anchor_id: i32,
    pub(super) advanced_window_id: i32,
    pub(super) check_3d_shadows_id: i32,
    pub(super) check_2d_shadows_id: i32,
    pub(super) check_cloud_shadows_id: i32,
    pub(super) check_ground_lighting_id: i32,
    pub(super) check_smooth_water_id: i32,
    pub(super) check_building_occlusion_id: i32,
    pub(super) check_props_id: i32,
    pub(super) check_extra_animations_id: i32,
    pub(super) check_no_dynamic_lod_id: i32,
    pub(super) check_unlock_fps_id: i32,
    pub(super) check_heat_effects_id: i32,
    pub(super) slider_scroll_speed_id: i32,
    pub(super) slider_music_volume_id: i32,
    pub(super) slider_sfx_volume_id: i32,
    pub(super) slider_voice_volume_id: i32,
    pub(super) slider_gamma_id: i32,
    pub(super) slider_texture_resolution_id: i32,
    pub(super) slider_particle_cap_id: i32,
    pub(super) parent: Option<Rc<RefCell<GameWindow>>>,
    pub(super) resolution_modes: Vec<(i32, i32)>,
    pub(super) initial_detail_index: usize,
    pub(super) firewall_behavior_override: Option<u32>,
}

impl OptionsMenu {
    pub fn new() -> Self {
        Self {
            residual_action: ResidualOptionsMenuAction::None,
            pending_host_apply: None,
            initialized: false,
            ignore_selected: false,
            parent_id: 0,
            button_back_id: 0,
            button_defaults_id: 0,
            button_accept_id: 0,
            button_keyboard_options_id: 0,
            button_advanced_accept_id: 0,
            button_advanced_back_id: 0,
            combo_anti_aliasing_id: 0,
            combo_resolution_id: 0,
            combo_detail_id: 0,
            check_alternate_mouse_id: 0,
            check_retaliation_id: 0,
            check_double_click_attack_move_id: 0,
            check_language_filter_id: 0,
            check_send_delay_id: 0,
            combo_lan_ip_id: 0,
            combo_online_ip_id: 0,
            button_firewall_refresh_id: 0,
            check_use_camera_id: 0,
            check_save_camera_id: 0,
            check_draw_anchor_id: 0,
            check_move_anchor_id: 0,
            advanced_window_id: 0,
            check_3d_shadows_id: 0,
            check_2d_shadows_id: 0,
            check_cloud_shadows_id: 0,
            check_ground_lighting_id: 0,
            check_smooth_water_id: 0,
            check_building_occlusion_id: 0,
            check_props_id: 0,
            check_extra_animations_id: 0,
            check_no_dynamic_lod_id: 0,
            check_unlock_fps_id: 0,
            check_heat_effects_id: 0,
            slider_scroll_speed_id: 0,
            slider_music_volume_id: 0,
            slider_sfx_volume_id: 0,
            slider_voice_volume_id: 0,
            slider_gamma_id: 0,
            slider_texture_resolution_id: 0,
            slider_particle_cap_id: 0,
            parent: None,
            resolution_modes: Vec::new(),
            initial_detail_index: 1,
            firewall_behavior_override: None,
        }
    }

    pub(super) fn name_to_id(name: &str) -> i32 {
        NameKeyGenerator::name_to_key(name) as i32
    }

    pub(super) fn find_window(
        id: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        // Input dispatch keeps the clicked gadget lent; walk the tree through
        // that loan so Accept/close lookups do not RefCell-panic on siblings.
        let lent: Vec<&GameWindow> = effects
            .lent_windows
            .iter()
            .map(|window| &**window)
            .collect();
        receiving_manager.win_get_window_from_id_with_lent(None, id, None, &lent)
    }

    pub(super) fn set_checkbox(
        id: i32,
        value: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager, effects) {
            let _ = window
                .borrow_mut()
                .gadget_check_box_set_checked(value, effects);
        }
    }

    pub(super) fn checkbox_value(
        id: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &crate::gui::game_window::WindowEventEffects<'_>,
    ) -> bool {
        Self::find_window(id, receiving_manager, effects)
            .and_then(|window| {
                let guard = window.borrow();
                match guard.widget() {
                    Some(WindowWidget::CheckBox(check_box)) => Some(check_box.is_checked()),
                    _ => None,
                }
            })
            .unwrap_or(false)
    }

    pub(super) fn set_slider_value(
        id: i32,
        value: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager, effects) {
            if let Some(slider) = window.borrow_mut().horizontal_slider_mut() {
                slider.set_value(value);
            }
        }
    }

    pub(super) fn set_slider_range_and_value(
        id: i32,
        min_value: i32,
        max_value: i32,
        value: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager, effects) {
            if let Some(slider) = window.borrow_mut().horizontal_slider_mut() {
                slider.set_range(min_value, max_value);
                slider.set_value(value);
            }
        }
    }

    pub(super) fn slider_value(
        id: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &crate::gui::game_window::WindowEventEffects<'_>,
    ) -> i32 {
        Self::slider_value_opt(id, receiving_manager, effects).unwrap_or(0)
    }

    /// C++ `GadgetSliderGetPosition` — `None` matches retail `-1` (missing gadget).
    pub(super) fn slider_value_opt(
        id: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Option<i32> {
        Self::find_window(id, receiving_manager, effects).and_then(|window| {
            let guard = window.borrow();
            match guard.widget() {
                Some(WindowWidget::HorizontalSlider(slider)) => Some(slider.value()),
                _ => None,
            }
        })
    }

    pub(super) fn set_combo_items(
        id: i32,
        items: &[String],
        selected_index: usize,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        if items.is_empty() {
            return;
        }
        if let Some(window) = Self::find_window(id, receiving_manager, effects) {
            let mut guard = window.borrow_mut();
            let Some(combo_box) = guard.combo_box_mut() else {
                return;
            };
            combo_box.clear();
            for (index, item) in items.iter().enumerate() {
                combo_box.add_item(ComboBoxItem::new(index as u32, item.clone()));
            }
            guard.set_combo_box_selected(selected_index.min(items.len() - 1), true, effects);
        }
    }

    pub(super) fn combo_selected_index(
        id: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Option<usize> {
        Self::find_window(id, receiving_manager, effects).and_then(|window| {
            let guard = window.borrow();
            match guard.widget() {
                Some(WindowWidget::ComboBox(combo_box)) => combo_box.selected_index(),
                _ => None,
            }
        })
    }

    pub(super) fn set_combo_selected(
        id: i32,
        index: usize,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager, effects) {
            window
                .borrow_mut()
                .set_combo_box_selected(index, true, effects);
        }
    }

    pub(super) fn default_resolution_index(&self) -> usize {
        self.resolution_modes
            .iter()
            .position(|mode| *mode == (800, 600))
            .unwrap_or(0)
    }

    pub(super) fn should_reset_resolution_on_defaults() -> bool {
        !TheGameLogic::is_in_game() || TheGameLogic::get_game_mode() == GAME_SHELL
    }

    pub(super) fn set_window_hidden(
        id: i32,
        hidden: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager, effects) {
            let _ = window.borrow_mut().hide(hidden, receiving_manager);
        }
    }

    pub(super) fn set_window_enabled(
        id: i32,
        enabled: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager, effects) {
            let _ = window.borrow_mut().enable(enabled);
        }
    }

    pub(super) fn detail_index_from_name(value: &str) -> usize {
        match value.trim().to_ascii_lowercase().as_str() {
            "high" => 0,
            "medium" => 1,
            "low" => 2,
            "custom" => 3,
            _ => 1,
        }
    }

    pub(super) fn detail_name_from_index(index: usize) -> &'static str {
        match index {
            0 => "High",
            1 => "Medium",
            2 => "Low",
            3 => "Custom",
            _ => "Medium",
        }
    }

    pub(super) fn detail_labels() -> Vec<String> {
        vec![
            GameText::fetch("GUI:High"),
            GameText::fetch("GUI:Medium"),
            GameText::fetch("GUI:Low"),
            GameText::fetch("GUI:Custom"),
        ]
    }

    pub(super) fn anti_alias_labels() -> Vec<String> {
        (0..3)
            .map(|index| GameText::fetch(&format!("GUI:AntiAliasing{index}")))
            .collect()
    }

    pub(super) fn resolution_label(mode: (i32, i32)) -> String {
        format!("{} x {}", mode.0, mode.1)
    }

    pub(super) fn slider_to_gamma(slider_value: i32) -> f32 {
        if slider_value < 50 {
            if slider_value <= 0 {
                0.6
            } else {
                1.0 - (0.4 * (50 - slider_value) as f32 / 50.0)
            }
        } else if slider_value > 50 {
            1.0 + (1.0 * (slider_value - 50) as f32 / 50.0)
        } else {
            1.0
        }
    }

    pub(super) fn set_yes_no(pref: &mut UserPreferences, key: &str, value: bool) {
        pref.set_string(key, if value { "yes" } else { "no" }.to_string());
    }

    pub(super) fn set_yes_no_title(pref: &mut UserPreferences, key: &str, value: bool) {
        pref.set_string(key, if value { "Yes" } else { "No" }.to_string());
    }

    pub(super) fn apply_immediate_checkbox_effect(
        &self,
        control_id: i32,
        checked: bool,
        effects: &mut WindowEventEffects<'_>,
    ) -> bool {
        if control_id == self.check_draw_anchor_id {
            effects.ui_mut().set_draw_rmb_scroll_anchor(checked);
            // Main owns the active RMB camera-drag presentation for an
            // AuthorityOnly match. Preserve the legacy standalone state, then
            // publish the same visual preference only while Main hosts it.
            publish_host_option(
                effects,
                HostOptionsRequest::DrawRmbScrollAnchor { enabled: checked },
            );
            return true;
        }
        if control_id == self.check_move_anchor_id {
            effects.ui_mut().set_move_rmb_scroll_anchor(checked);
            // Main owns live camera input for AuthorityOnly. Preserve this
            // standalone GameClient update, then send the same typed checkbox
            // choice to Main only while its host bridge is installed.
            publish_host_option(
                effects,
                HostOptionsRequest::MoveRmbScrollAnchor { enabled: checked },
            );
            return true;
        }
        if control_id == self.check_save_camera_id {
            runtime_global_data::write().save_camera_in_replay = checked;
            return true;
        }
        if control_id == self.check_use_camera_id {
            runtime_global_data::write().use_camera_in_replay = checked;
            return true;
        }
        false
    }

    pub(super) fn current_relative_2d_volume() -> f32 {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let Ok(guard) = manager.lock() else {
            return 0.0;
        };
        guard.get_audio_settings().relative_2d_volume
    }

    pub(super) fn split_sfx_volume_for_relative(
        sfx_volume: i32,
        relative_2d_volume: f32,
    ) -> (i32, i32, f32, f32) {
        let base_volume = sfx_volume.clamp(0, 100) as f32 / 100.0;
        let relative_2d_volume = relative_2d_volume.clamp(-1.0, 1.0);
        let mut sound_2d_volume = base_volume;
        let mut sound_3d_volume = base_volume;
        if relative_2d_volume < 0.0 {
            sound_2d_volume *= 1.0 + relative_2d_volume;
        } else {
            sound_3d_volume *= 1.0 - relative_2d_volume;
        }

        (
            (sound_2d_volume * 100.0) as i32,
            (sound_3d_volume * 100.0) as i32,
            sound_2d_volume,
            sound_3d_volume,
        )
    }

    pub(super) fn load_resolution_modes(&mut self) {
        // C++ OptionsMenu enumerates TheDisplay->getDisplayModeCount/Description
        // (4:3, >=800x600, >=24-bit).
        self.resolution_modes.clear();
        let count = crate::display::display_fx::display_mode_count();
        for i in 0..count {
            if let Some((w, h, _)) = crate::display::display_fx::display_mode_description(i) {
                self.resolution_modes.push((w as i32, h as i32));
            }
        }
        if let Ok(global) = runtime_global_data::read_safe() {
            let current = (global.writable.x_resolution, global.writable.y_resolution);
            if current.0 > 0 && current.1 > 0 && !self.resolution_modes.contains(&current) {
                self.resolution_modes.push(current);
                self.resolution_modes.sort_unstable();
            }
        }
    }

    pub(super) fn populate_controls(
        &mut self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.populate_controls_with_camera_input(None, receiving_manager, effects);
    }
    pub(super) fn populate_controls_with_camera_input(
        &mut self,
        owned: Option<(&WindowLayout, &OptionsCameraInputObservation)>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.load_resolution_modes();

        let mut pref = UserPreferences::new();
        let _ = pref.load("Options.ini");

        let global = runtime_global_data::read();

        let alternate_mouse = pref.get_bool_or("UseAlternateMouse", global.use_alternate_mouse);
        let retaliation = pref.get_bool_or("Retaliation", global.client_retaliation_mode_enabled);
        let double_click_attack_move =
            pref.get_bool_or("UseDoubleClickAttackMove", global.double_click_attack_move);
        let language_filter = pref.get_bool_or("LanguageFilter", global.language_filter_pref);
        let send_delay = pref.get_bool_or("SendDelay", global.firewall_send_delay);
        let save_camera = owned
            .map(|(_, view)| view.save_camera)
            .unwrap_or_else(|| pref.save_camera_in_replays());
        let use_camera = owned
            .map(|(_, view)| view.use_camera)
            .unwrap_or_else(|| pref.use_camera_in_replays());
        let draw_anchor = game_engine::common::user_preferences::scroll_anchor_pref_enabled(
            pref.get_string("DrawScrollAnchor")
                .map(|value| value.as_str()),
            effects.ui().get_draw_rmb_scroll_anchor(),
        );
        let move_anchor = game_engine::common::user_preferences::scroll_anchor_pref_enabled(
            pref.get_string("MoveScrollAnchor")
                .map(|value| value.as_str()),
            effects.ui().get_move_rmb_scroll_anchor(),
        );

        let music_volume = pref
            .get_int("MusicVolume")
            .unwrap_or((global.music_volume_factor * 100.0) as i32);
        let sfx_2d_volume = pref
            .get_int("SFXVolume")
            .unwrap_or((global.sfx_volume_factor * 100.0) as i32);
        let sfx_3d_volume = pref.get_int_or("SFX3DVolume", sfx_2d_volume);
        let sfx_volume = sfx_2d_volume.max(sfx_3d_volume);
        let voice_volume = pref
            .get_int("VoiceVolume")
            .unwrap_or((global.voice_volume_factor * 100.0) as i32);
        let gamma = pref.get_int_or("Gamma", 50);
        let scroll_speed = if owned.is_none() {
            Some(pref.get_int_or(
                "ScrollFactor",
                (global.keyboard_default_scroll_factor * 100.0) as i32,
            ))
        } else {
            None
        };
        let anti_aliasing = pref
            .get_int_or("AntiAliasing", global.anti_alias_box_value)
            .clamp(0, 2) as usize;
        // First-open LOD applies settings through the same runtime owner.
        // End the observation before that mutation, then observe its results.
        drop(global);
        // C++ OptionsMenu first-open: if static LOD is UNKNOWN, find+set.
        game_engine::common::game_lod::ensure_static_lod_applied();
        let detail_name = pref.get_string_or(
            "StaticGameLOD",
            &game_engine::common::game_lod::get_static_lod(),
        );
        let detail_name = if detail_name.eq_ignore_ascii_case("Unknown") {
            game_engine::common::game_lod::find_static_lod_level()
        } else {
            detail_name
        };
        let detail_index = Self::detail_index_from_name(&detail_name);
        self.initial_detail_index = detail_index;

        let global = runtime_global_data::read();
        let texture_reduction = pref
            .get_int("TextureReduction")
            .unwrap_or(global.texture_reduction_factor)
            .clamp(0, 2);
        let texture_resolution = 2 - texture_reduction;
        let particle_cap = pref
            .get_int("MaxParticleCount")
            .unwrap_or(global.max_particle_count)
            .max(100);

        let use_shadow_volumes =
            pref.get_bool_or("UseShadowVolumes", global.writable.use_shadow_volumes);
        let use_shadow_decals =
            pref.get_bool_or("UseShadowDecals", global.writable.use_shadow_decals);
        let use_cloud_map = pref.get_bool_or("UseCloudMap", global.use_cloud_map);
        let use_light_map = pref.get_bool_or("UseLightMap", global.use_light_map);
        let show_soft_water_edge =
            pref.get_bool_or("ShowSoftWaterEdge", global.show_soft_water_edge);
        let extra_animations = pref
            .get_bool("ExtraAnimations")
            .unwrap_or(!global.use_draw_module_lod);
        let no_dynamic_lod = !pref
            .get_bool("DynamicLOD")
            .unwrap_or(global.writable.enable_dynamic_lod);
        let unlock_fps = !pref
            .get_bool("FPSLimit")
            .unwrap_or(global.writable.use_fps_limit);
        let heat_effects = pref.get_bool_or("HeatEffects", global.use_heat_effects);
        let building_occlusion =
            pref.get_bool_or("BuildingOcclusion", global.enable_behind_building_markers);
        let show_props = pref.get_bool_or("ShowTrees", global.use_trees);

        drop(global);

        let resolution_pref = pref.get_string_or("Resolution", "");
        let resolution = {
            let mut parts = resolution_pref.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(width), Some(height)) => {
                    match (width.parse::<i32>(), height.parse::<i32>()) {
                        (Ok(width), Ok(height)) => (width, height),
                        _ => {
                            let global = runtime_global_data::read();
                            (global.writable.x_resolution, global.writable.y_resolution)
                        }
                    }
                }
                _ => {
                    let global = runtime_global_data::read();
                    (global.writable.x_resolution, global.writable.y_resolution)
                }
            }
        };
        let resolution_index = self
            .resolution_modes
            .iter()
            .position(|mode| *mode == resolution)
            .unwrap_or(0);

        Self::set_combo_items(
            self.combo_anti_aliasing_id,
            &Self::anti_alias_labels(),
            anti_aliasing,
            receiving_manager,
            effects,
        );
        Self::set_combo_items(
            self.combo_resolution_id,
            &self
                .resolution_modes
                .iter()
                .map(|mode| Self::resolution_label(*mode))
                .collect::<Vec<_>>(),
            resolution_index,
            receiving_manager,
            effects,
        );
        Self::set_combo_items(
            self.combo_detail_id,
            &Self::detail_labels(),
            detail_index,
            receiving_manager,
            effects,
        );

        Self::set_checkbox(
            self.check_alternate_mouse_id,
            alternate_mouse,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_retaliation_id,
            retaliation,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_double_click_attack_move_id,
            double_click_attack_move,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_language_filter_id,
            language_filter,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_send_delay_id,
            send_delay,
            receiving_manager,
            effects,
        );
        if let Some((layout, _)) = owned {
            for (name, checked) in [
                ("OptionsMenu.wnd:CheckBoxSaveCamera", save_camera),
                ("OptionsMenu.wnd:CheckBoxUseCamera", use_camera),
            ] {
                let id =
                    game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(name)
                        as i32;
                if let Some(window) = layout
                    .windows()
                    .iter()
                    .find(|window| window.borrow().get_id() == id)
                {
                    if let Some(checkbox) = window.borrow_mut().check_box_mut() {
                        checkbox.set_checked(checked);
                    }
                }
            }
        } else {
            Self::set_checkbox(
                self.check_save_camera_id,
                save_camera,
                receiving_manager,
                effects,
            );
            Self::set_checkbox(
                self.check_use_camera_id,
                use_camera,
                receiving_manager,
                effects,
            );
        }
        Self::set_checkbox(
            self.check_draw_anchor_id,
            draw_anchor,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_move_anchor_id,
            move_anchor,
            receiving_manager,
            effects,
        );
        // C++ OptionsMenuInit applies these to TheInGameUI on open.
        effects.ui_mut().set_draw_rmb_scroll_anchor(draw_anchor);
        effects.ui_mut().set_move_rmb_scroll_anchor(move_anchor);
        publish_host_option(
            effects,
            HostOptionsRequest::DrawRmbScrollAnchor {
                enabled: draw_anchor,
            },
        );
        publish_host_option(
            effects,
            HostOptionsRequest::MoveRmbScrollAnchor {
                enabled: move_anchor,
            },
        );
        Self::set_checkbox(
            self.check_3d_shadows_id,
            use_shadow_volumes,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_2d_shadows_id,
            use_shadow_decals,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_cloud_shadows_id,
            use_cloud_map,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_ground_lighting_id,
            use_light_map,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_smooth_water_id,
            show_soft_water_edge,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_extra_animations_id,
            extra_animations,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_no_dynamic_lod_id,
            no_dynamic_lod,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_unlock_fps_id,
            unlock_fps,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_heat_effects_id,
            heat_effects,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_building_occlusion_id,
            building_occlusion,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_props_id, show_props, receiving_manager, effects);

        if let Some((layout, observation)) = owned {
            set_owned_options_scroll(layout, observation.scroll_factor, true);
        } else if let Some(scroll_speed) = scroll_speed {
            Self::set_slider_range_and_value(
                self.slider_scroll_speed_id,
                0,
                100,
                scroll_speed,
                receiving_manager,
                effects,
            );
        }
        Self::set_slider_range_and_value(
            self.slider_music_volume_id,
            0,
            100,
            music_volume,
            receiving_manager,
            effects,
        );
        Self::set_slider_range_and_value(
            self.slider_sfx_volume_id,
            0,
            100,
            sfx_volume,
            receiving_manager,
            effects,
        );
        Self::set_slider_range_and_value(
            self.slider_voice_volume_id,
            0,
            100,
            voice_volume,
            receiving_manager,
            effects,
        );
        Self::set_slider_range_and_value(
            self.slider_gamma_id,
            0,
            100,
            gamma,
            receiving_manager,
            effects,
        );
        Self::set_slider_range_and_value(
            self.slider_texture_resolution_id,
            0,
            2,
            texture_resolution,
            receiving_manager,
            effects,
        );
        Self::set_slider_range_and_value(
            self.slider_particle_cap_id,
            100,
            10000,
            particle_cap,
            receiving_manager,
            effects,
        );

        Self::set_window_hidden(self.advanced_window_id, true, receiving_manager, effects);
    }

    pub(super) fn apply_default_controls(
        &mut self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.apply_default_controls_with_scroll(None, receiving_manager, effects);
    }
    pub(super) fn apply_default_controls_with_scroll(
        &mut self,
        owned: Option<(Option<Rc<RefCell<GameWindow>>>, f32)>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        Self::set_checkbox(
            self.check_language_filter_id,
            true,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_send_delay_id, false, receiving_manager, effects);
        Self::set_checkbox(
            self.check_alternate_mouse_id,
            false,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_retaliation_id, true, receiving_manager, effects);
        Self::set_checkbox(
            self.check_double_click_attack_move_id,
            false,
            receiving_manager,
            effects,
        );
        // C++ setDefaults never touches replay-camera or RMB-anchor checkboxes.

        if let Some((slider, default)) = owned {
            if let Some(slider) = slider {
                if let Some(slider) = slider.borrow_mut().horizontal_slider_mut() {
                    slider.set_value((default * 100.0) as i32);
                }
            }
        } else {
            let default_scroll =
                (runtime_global_data::read().keyboard_default_scroll_factor * 100.0).round() as i32;
            Self::set_slider_value(
                self.slider_scroll_speed_id,
                default_scroll,
                receiving_manager,
                effects,
            );
        }
        Self::set_slider_value(self.slider_music_volume_id, 60, receiving_manager, effects);
        Self::set_slider_value(self.slider_sfx_volume_id, 55, receiving_manager, effects);
        Self::set_slider_value(self.slider_voice_volume_id, 70, receiving_manager, effects);
        Self::set_slider_value(self.slider_gamma_id, 50, receiving_manager, effects);
        Self::set_slider_value(
            self.slider_texture_resolution_id,
            2,
            receiving_manager,
            effects,
        );
        Self::set_slider_value(
            self.slider_particle_cap_id,
            5000,
            receiving_manager,
            effects,
        );

        Self::set_checkbox(self.check_3d_shadows_id, true, receiving_manager, effects);
        Self::set_checkbox(self.check_2d_shadows_id, true, receiving_manager, effects);
        Self::set_checkbox(
            self.check_cloud_shadows_id,
            true,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_ground_lighting_id,
            true,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_smooth_water_id, true, receiving_manager, effects);
        Self::set_checkbox(
            self.check_extra_animations_id,
            true,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_no_dynamic_lod_id,
            false,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_unlock_fps_id, false, receiving_manager, effects);
        Self::set_checkbox(self.check_heat_effects_id, true, receiving_manager, effects);
        Self::set_checkbox(
            self.check_building_occlusion_id,
            true,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_props_id, true, receiving_manager, effects);
        // C++ setDefaults: comboBoxDetail = findStaticLODLevel()
        let recommended = game_engine::common::game_lod::find_static_lod_level();
        Self::set_combo_selected(
            self.combo_detail_id,
            Self::detail_index_from_name(&recommended),
            receiving_manager,
            effects,
        );

        if Self::should_reset_resolution_on_defaults() {
            Self::set_combo_selected(
                self.combo_resolution_id,
                self.default_resolution_index(),
                receiving_manager,
                effects,
            );
        }
        Self::set_window_hidden(self.advanced_window_id, true, receiving_manager, effects);
    }

    pub(super) fn apply_options(&mut self, effects: &mut WindowEventEffects<'_>) -> bool {
        let host = self.pending_host_apply.take();
        let mut preferences = UserPreferences::new();
        let _ = preferences.load("Options.ini");
        self.apply_options_with_preferences(host, &mut preferences, None, None, effects)
    }
    pub(super) fn apply_options_with_preferences(
        &mut self,
        host: Option<HostOptionsApply>,
        pref: &mut UserPreferences,
        owned_scroll: Option<Option<i32>>,
        terrain_lod: Option<&mut dyn FnMut(i32)>,
        effects: &mut WindowEventEffects<'_>,
    ) -> bool {
        let owns_camera_preferences = owned_scroll.is_some();
        let (
            old_resolution,
            windowed,
            default_scroll,
            default_music,
            default_sfx,
            default_voice,
            default_res,
        ) = {
            let global = runtime_global_data::read();
            (
                (global.writable.x_resolution, global.writable.y_resolution),
                global.writable.windowed,
                (!owns_camera_preferences).then(|| (global.keyboard_scroll_factor * 100.0) as i32),
                (global.music_volume_factor * 100.0) as i32,
                (global.sfx_volume_factor * 100.0) as i32,
                (global.voice_volume_factor * 100.0) as i32,
                (global.writable.x_resolution, global.writable.y_resolution),
            )
        };

        let detail_index = host
            .as_ref()
            .map(|h| h.detail_index)
            .or_else(|| {
                effects.with_manager(|receiving_manager, effects| {
                    Self::combo_selected_index(self.combo_detail_id, receiving_manager, effects)
                })
            })
            .unwrap_or(1);
        let anti_aliasing = host
            .as_ref()
            .map(|h| h.anti_aliasing)
            .or_else(|| {
                effects
                    .with_manager(|receiving_manager, effects| {
                        Self::combo_selected_index(
                            self.combo_anti_aliasing_id,
                            receiving_manager,
                            effects,
                        )
                    })
                    .map(|i| i as i32)
            })
            .unwrap_or_else(|| pref.get_int_or("AntiAliasing", 0));
        let resolution = host
            .as_ref()
            .map(|h| h.resolution)
            .or_else(|| {
                self.resolution_modes
                    .get(
                        effects
                            .with_manager(|receiving_manager, effects| {
                                Self::combo_selected_index(
                                    self.combo_resolution_id,
                                    receiving_manager,
                                    effects,
                                )
                            })
                            .unwrap_or(0),
                    )
                    .copied()
            })
            .unwrap_or_else(|| {
                parse_resolution_pref(pref.get_string("Resolution").map(|s| s.as_str()))
                    .unwrap_or(default_res)
            });

        let alternate_mouse = host.as_ref().map(|h| h.alternate_mouse).unwrap_or_else(|| {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_alternate_mouse_id, receiving_manager, effects)
            })
        });
        let retaliation = host.as_ref().map(|h| h.retaliation).unwrap_or_else(|| {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_retaliation_id, receiving_manager, effects)
            })
        });
        let double_click_attack_move = host
            .as_ref()
            .map(|h| h.double_click_attack_move)
            .unwrap_or_else(|| {
                effects.with_manager(|receiving_manager, effects| {
                    Self::checkbox_value(
                        self.check_double_click_attack_move_id,
                        receiving_manager,
                        effects,
                    )
                })
            });
        let language_filter = host.as_ref().map(|h| h.language_filter).unwrap_or_else(|| {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_language_filter_id, receiving_manager, effects)
            })
        });
        let send_delay = effects.with_manager(|receiving_manager, effects| {
            Self::checkbox_value(self.check_send_delay_id, receiving_manager, effects)
        });
        let save_camera = host.as_ref().map(|h| h.save_camera).unwrap_or_else(|| {
            if owns_camera_preferences {
                pref.save_camera_in_replays()
            } else {
                effects.with_manager(|receiving_manager, effects| {
                    Self::checkbox_value(self.check_save_camera_id, receiving_manager, effects)
                })
            }
        });
        let use_camera = host.as_ref().map(|h| h.use_camera).unwrap_or_else(|| {
            if owns_camera_preferences {
                pref.use_camera_in_replays()
            } else {
                effects.with_manager(|receiving_manager, effects| {
                    Self::checkbox_value(self.check_use_camera_id, receiving_manager, effects)
                })
            }
        });
        let draw_anchor = host.as_ref().map(|h| h.draw_anchor).unwrap_or_else(|| {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_draw_anchor_id, receiving_manager, effects)
            })
        });
        let move_anchor = host.as_ref().map(|h| h.move_anchor).unwrap_or_else(|| {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_move_anchor_id, receiving_manager, effects)
            })
        });
        // Host Accept has no WND layout; C++ saveOptions reads checkboxes that
        // OptionsMenuInit populated from prefs/globals. Keep those current values.
        let host_apply = host.is_some();
        let current = runtime_global_data::read();
        let use_shadow_volumes = if host_apply {
            pref.get_bool_or("UseShadowVolumes", current.writable.use_shadow_volumes)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_3d_shadows_id, receiving_manager, effects)
            })
        };
        let use_shadow_decals = if host_apply {
            pref.get_bool_or("UseShadowDecals", current.writable.use_shadow_decals)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_2d_shadows_id, receiving_manager, effects)
            })
        };
        let use_cloud_map = if host_apply {
            pref.get_bool_or("UseCloudMap", current.use_cloud_map)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_cloud_shadows_id, receiving_manager, effects)
            })
        };
        let use_light_map = if host_apply {
            pref.get_bool_or("UseLightMap", current.use_light_map)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_ground_lighting_id, receiving_manager, effects)
            })
        };
        let show_soft_water_edge = if host_apply {
            pref.get_bool_or("ShowSoftWaterEdge", current.show_soft_water_edge)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_smooth_water_id, receiving_manager, effects)
            })
        };
        let extra_animations = if host_apply {
            pref.get_bool("ExtraAnimations")
                .unwrap_or(!current.use_draw_module_lod)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_extra_animations_id, receiving_manager, effects)
            })
        };
        let no_dynamic_lod = if host_apply {
            !pref
                .get_bool("DynamicLOD")
                .unwrap_or(current.writable.enable_dynamic_lod)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_no_dynamic_lod_id, receiving_manager, effects)
            })
        };
        let unlock_fps = if host_apply {
            !pref
                .get_bool("FPSLimit")
                .unwrap_or(current.writable.use_fps_limit)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_unlock_fps_id, receiving_manager, effects)
            })
        };
        let heat_effects = if host_apply {
            pref.get_bool_or("HeatEffects", current.use_heat_effects)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_heat_effects_id, receiving_manager, effects)
            })
        };
        let building_occlusion = if host_apply {
            pref.get_bool_or("BuildingOcclusion", current.enable_behind_building_markers)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_building_occlusion_id, receiving_manager, effects)
            })
        };
        let show_props = if host_apply {
            pref.get_bool_or("ShowTrees", current.use_trees)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_props_id, receiving_manager, effects)
            })
        };
        drop(current);

        // C++ OptionsMenu.cpp:1168-1241 `if (val != -1)` — missing slider keeps pref.
        let scroll_speed = match owned_scroll {
            Some(value) => value,
            None => effects
                .with_manager(|receiving_manager, effects| {
                    Self::slider_value_opt(self.slider_scroll_speed_id, receiving_manager, effects)
                })
                .or_else(|| host.as_ref().map(|h| h.scroll_speed))
                .or_else(|| pref.get_int("ScrollFactor"))
                .or(default_scroll)
                .map(|value| value.clamp(0, 100)),
        };
        let music_volume = effects
            .with_manager(|receiving_manager, effects| {
                Self::slider_value_opt(self.slider_music_volume_id, receiving_manager, effects)
            })
            .or_else(|| host.as_ref().map(|h| h.music_volume))
            .unwrap_or_else(|| pref.get_int_or("MusicVolume", default_music))
            .clamp(0, 100);
        let sfx_volume = effects
            .with_manager(|receiving_manager, effects| {
                Self::slider_value_opt(self.slider_sfx_volume_id, receiving_manager, effects)
            })
            .or_else(|| host.as_ref().map(|h| h.sfx_volume))
            .unwrap_or_else(|| pref.get_int_or("SFXVolume", default_sfx))
            .clamp(0, 100);
        let voice_volume = effects
            .with_manager(|receiving_manager, effects| {
                Self::slider_value_opt(self.slider_voice_volume_id, receiving_manager, effects)
            })
            .or_else(|| host.as_ref().map(|h| h.voice_volume))
            .unwrap_or_else(|| pref.get_int_or("VoiceVolume", default_voice))
            .clamp(0, 100);
        let (sfx_2d_volume, sfx_3d_volume, sfx_2d_factor, sfx_3d_factor) =
            Self::split_sfx_volume_for_relative(sfx_volume, Self::current_relative_2d_volume());
        let gamma_slider = effects
            .with_manager(|receiving_manager, effects| {
                Self::slider_value_opt(self.slider_gamma_id, receiving_manager, effects)
            })
            .or_else(|| host.as_ref().map(|h| h.gamma_slider))
            .unwrap_or_else(|| pref.get_int_or("Gamma", 50))
            .clamp(0, 100);
        let texture_resolution = if host_apply {
            let reduction = pref
                .get_int("TextureReduction")
                .unwrap_or(runtime_global_data::read().texture_reduction_factor)
                .clamp(0, 2);
            2 - reduction
        } else {
            effects
                .with_manager(|receiving_manager, effects| {
                    Self::slider_value_opt(
                        self.slider_texture_resolution_id,
                        receiving_manager,
                        effects,
                    )
                })
                .unwrap_or(2)
                .clamp(0, 2)
        };
        let particle_cap = if host_apply {
            pref.get_int("MaxParticleCount")
                .unwrap_or(runtime_global_data::read().max_particle_count)
                .max(100)
        } else {
            effects
                .with_manager(|receiving_manager, effects| {
                    Self::slider_value_opt(self.slider_particle_cap_id, receiving_manager, effects)
                })
                .unwrap_or(5000)
                .max(100)
        };
        let texture_reduction = 2 - texture_resolution;
        let detail_name = Self::detail_name_from_index(detail_index);

        pref.set_string("Resolution", format!("{} {}", resolution.0, resolution.1));
        pref.set_int("AntiAliasing", anti_aliasing);
        pref.set_string("StaticGameLOD", detail_name.to_string());
        if let Some(scroll_speed) = scroll_speed {
            pref.set_int("ScrollFactor", scroll_speed);
        }
        pref.set_int("MusicVolume", music_volume);
        pref.set_int("SFXVolume", sfx_2d_volume);
        pref.set_int("SFX3DVolume", sfx_3d_volume);
        pref.set_int("VoiceVolume", voice_volume);
        pref.set_int("Gamma", gamma_slider);
        pref.set_string(
            "LanguageFilter",
            if language_filter { "true" } else { "false" }.to_string(),
        );
        Self::set_yes_no(pref, "SendDelay", send_delay);
        Self::set_yes_no(pref, "UseAlternateMouse", alternate_mouse);
        Self::set_yes_no(pref, "Retaliation", retaliation);
        Self::set_yes_no(pref, "UseDoubleClickAttackMove", double_click_attack_move);
        Self::set_yes_no(pref, "SaveCameraInReplays", save_camera);
        Self::set_yes_no(pref, "UseCameraInReplays", use_camera);
        Self::set_yes_no_title(pref, "DrawScrollAnchor", draw_anchor);
        Self::set_yes_no_title(pref, "MoveScrollAnchor", move_anchor);
        // C++ saveOptions writes advanced LOD only when Detail == CUSTOMDETAIL.
        // FPSLimit is never written (`//Never write this out`).

        const CUSTOMDETAIL: usize = 3;
        if detail_index == CUSTOMDETAIL {
            pref.set_int("TextureReduction", texture_reduction);
            pref.set_int("MaxParticleCount", particle_cap);
            Self::set_yes_no(pref, "UseShadowVolumes", use_shadow_volumes);
            Self::set_yes_no(pref, "UseShadowDecals", use_shadow_decals);
            Self::set_yes_no(pref, "UseCloudMap", use_cloud_map);
            Self::set_yes_no(pref, "UseLightMap", use_light_map);
            Self::set_yes_no(pref, "ShowSoftWaterEdge", show_soft_water_edge);
            Self::set_yes_no(pref, "ExtraAnimations", extra_animations);
            Self::set_yes_no(pref, "DynamicLOD", !no_dynamic_lod);
            Self::set_yes_no(pref, "HeatEffects", heat_effects);
            Self::set_yes_no(pref, "BuildingOcclusion", building_occlusion);
            Self::set_yes_no(pref, "ShowTrees", show_props);
        }
        if let Some(firewall_behavior) = self.firewall_behavior_override {
            pref.set_string("FirewallBehavior", firewall_behavior.to_string());
        }
        let _ = pref.write();
        self.firewall_behavior_override = None;

        effects.ui_mut().set_draw_rmb_scroll_anchor(draw_anchor);
        effects.ui_mut().set_move_rmb_scroll_anchor(move_anchor);
        publish_host_option(
            effects,
            HostOptionsRequest::DrawRmbScrollAnchor {
                enabled: draw_anchor,
            },
        );
        publish_host_option(
            effects,
            HostOptionsRequest::MoveRmbScrollAnchor {
                enabled: move_anchor,
            },
        );

        Self::commit_alternate_mouse_setting(
            alternate_mouse,
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| &mut host.options),
        );
        let display_gamma = Self::slider_to_gamma(gamma_slider);
        {
            let mut global = runtime_global_data::write();
            global.client_retaliation_mode_enabled = retaliation;
            global.double_click_attack_move = double_click_attack_move;
            global.language_filter_pref = language_filter;
            global.firewall_send_delay = send_delay;
            if !owns_camera_preferences {
                global.save_camera_in_replay = save_camera;
                global.use_camera_in_replay = use_camera;
            }
            global.display_gamma = display_gamma;
            global.anti_alias_box_value = anti_aliasing;
            if !owns_camera_preferences {
                if let Some(scroll_speed) = scroll_speed {
                    global.keyboard_scroll_factor = scroll_speed as f32 / 100.0;
                }
            }
            global.music_volume_factor = music_volume as f32 / 100.0;
            global.sfx_volume_factor = sfx_volume as f32 / 100.0;
            global.voice_volume_factor = voice_volume as f32 / 100.0;
            global.writable.x_resolution = resolution.0;
            global.writable.y_resolution = resolution.1;
            if detail_index == CUSTOMDETAIL {
                global.use_cloud_map = use_cloud_map;
                global.use_light_map = use_light_map;
                global.show_soft_water_edge = show_soft_water_edge;
                global.use_draw_module_lod = !extra_animations;
                global.use_heat_effects = heat_effects;
                global.enable_behind_building_markers = building_occlusion;
                global.use_trees = show_props;
                global.max_particle_count = particle_cap;
                global.writable.use_shadow_volumes = use_shadow_volumes;
                global.writable.use_shadow_decals = use_shadow_decals;
                global.writable.enable_dynamic_lod = !no_dynamic_lod;
            }
        }
        let _ = crate::core::script_action_handler::set_script_display_gamma(
            effects.script_display(),
            display_gamma,
            0.0,
            1.0,
        );

        if detail_index == CUSTOMDETAIL {
            if let Some(terrain) = terrain_lod {
                let _ = crate::core::game_client::apply_lod_texture_reduction_to(
                    texture_reduction,
                    terrain,
                );
            } else {
                let _ = apply_lod_texture_reduction(texture_reduction);
            }
        }

        if effects.menu_host.is_some() {
            if let Some(result) =
                game_engine::common::game_lod::set_static_lod_from_string_for_session(detail_name)
            {
                match result {
                    game_engine::common::game_lod::SessionStaticLodResult::Apply {
                        level,
                        rules,
                    } => publish_host_option(
                        effects,
                        HostOptionsRequest::TerrainTrackRules {
                            selected_level: level,
                            rules,
                        },
                    ),
                    game_engine::common::game_lod::SessionStaticLodResult::MarkerOnly { level } => {
                        publish_host_option(
                            effects,
                            HostOptionsRequest::StaticLodMarkerOnly { level },
                        )
                    }
                }
            }
        } else {
            game_engine::common::game_lod::set_static_lod_from_string(detail_name);
        }
        game_engine::common::game_lod::set_ideal_static_lod_from_string(detail_name);

        let audio = TheAudio;
        // C++ OptionsMenu.cpp:1188/1214/1215/1235 AudioAffect_*|SystemSetting.
        audio.set_volume(
            music_volume as f32 / 100.0,
            EngineAudioAffect::MusicSystemSetting,
        );
        audio.set_volume(sfx_2d_factor, EngineAudioAffect::SoundSystemSetting);
        audio.set_volume(sfx_3d_factor, EngineAudioAffect::Sound3DSystemSetting);
        audio.set_volume(
            voice_volume as f32 / 100.0,
            EngineAudioAffect::SpeechSystemSetting,
        );
        get_header_template_manager()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .header_notify_resolution_change();

        let resolution_changed = resolution != old_resolution;
        if resolution_changed {
            let old_settings = DisplaySettings {
                x_res: old_resolution.0,
                y_res: old_resolution.1,
                bit_depth: 32,
                windowed,
            };
            let new_settings = DisplaySettings {
                x_res: resolution.0,
                y_res: resolution.1,
                bit_depth: 32,
                windowed,
            };
            // C++ OptionsMenu.cpp:1062 `if (TheDisplay->setDisplayMode(...))`.
            let (display, mouse) = effects.script_display_and_mouse();
            if !crate::core::script_action_handler::apply_script_display_mode(
                display,
                resolution.0.max(0) as u32,
                resolution.1.max(0) as u32,
                32,
                windowed,
                mouse,
            ) {
                return false;
            }
            effects.with_main_menu(|menu, _| {
                menu.set_pending_resolution_change(old_settings, new_settings)
            });
        }

        resolution_changed
    }

    /// C++ has one writable GlobalData object. Rust's live consumers span a
    /// parsed-INI residence and a runtime options residence, so commit this
    /// setting to both before a later LeftHUD callback snapshots it for Main.
    pub fn commit_alternate_mouse_setting(
        alternate_mouse: bool,
        host: Option<&mut crate::gui::options_host_bridge::HostOptionsBridge>,
    ) {
        if let Some(data) = game_engine::common::ini::get_global_data() {
            data.write().use_alternate_mouse = alternate_mouse;
        }
        runtime_global_data::write().use_alternate_mouse = alternate_mouse;
        if let Some(host) = host {
            host.publish(HostOptionsRequest::AlternateMouse {
                enabled: alternate_mouse,
            });
        }
        // Main is the physical-world input authority in an AuthorityOnly
        // match.  Keep the legacy residences above for standalone GameClient,
        // then deliver the same accepted setting through the bounded host
        // bridge when it is installed.
    }

    pub(super) fn close_menu(
        &mut self,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        let options_overlay_open = options_overlay_is_open();
        if let Some(parent) = self.parent.as_ref() {
            effects.with_manager(|manager, effects| {
                let _ = manager.unset_modal(parent);
            });
        }
        // Parent OptionsMenuSystem still holds the layout root RefMut while the
        // clicked Accept child is lent; defer the advanced-window hide until
        // those loans end so tree lookup does not RefCell-panic on the root.
        let advanced_window_id = self.advanced_window_id;
        effects.queue_window_operation(move |receiving_manager, effects| {
            Self::set_window_hidden(advanced_window_id, true, receiving_manager, effects);
        });
        TheScriptEngine::signal_ui_interact("ShellOptionsClosed");
        if options_overlay_open {
            close_options_overlay();
        } else {
            crate::gui::shell::base::queue_shell_operation_with_effects(
                effects,
                |shell, effects| shell.destroy_options_layout(effects),
            );
        }
        self.parent = None;
        self.initialized = false;
    }

    pub(super) fn accept_options(
        &mut self,
        owned: Option<(&mut UserPreferences, Option<i32>)>,
        terrain_lod: Option<&mut dyn FnMut(i32)>,
        effects: &mut WindowEventEffects<'_>,
    ) {
        let resolution_changed = match owned {
            Some((pref, scroll)) => {
                self.apply_options_with_preferences(None, pref, Some(scroll), terrain_lod, effects)
            }
            None => self.apply_options(effects),
        };
        let options_overlay_open = options_overlay_is_open();
        if !TheGameLogic::is_in_game() || TheGameLogic::get_game_mode() == GAME_SHELL {
            destroy_quit_menu(effects);
        }
        self.close_menu(effects);
        if resolution_changed && !options_overlay_open {
            effects.with_main_menu(|menu, effects| menu.do_resolution_dialog(effects));
        }
    }

    pub(crate) fn init_with_camera_input(
        &mut self,
        layout: &WindowLayout,
        observation: Option<&OptionsCameraInputObservation>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.init_ids();
        self.ignore_selected = true;
        effects.with_manager(|manager, effects| {
            self.parent = manager.get_window_by_id(self.parent_id);
            // Modal bookkeeping dispatches no messages — safe inline. Focus is
            // deferred (see SinglePlayerMenu::init): an inline set_focus would
            // re-enter the menu borrow held by this init dispatch.
            if let Some(parent) = self.parent.as_ref() {
                let _ = manager.set_modal(parent.clone());
            }
            if let Some(parent) = self.parent.clone() {
                effects.queue_window_operation(move |manager, effects| {
                    let _ = manager.set_focus(Some(&parent), effects);
                });
            }
        });
        effects.with_manager(|receiving_manager, effects| {
            self.populate_controls_with_camera_input(
                observation.map(|value| (layout, value)),
                receiving_manager,
                effects,
            )
        });
        let in_game_locked =
            TheGameLogic::is_in_game() && TheGameLogic::get_game_mode() != GAME_SHELL;
        if in_game_locked || options_overlay_is_open() {
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(self.combo_lan_ip_id, false, receiving_manager, effects)
            });
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(self.combo_online_ip_id, false, receiving_manager, effects)
            });
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(self.combo_detail_id, false, receiving_manager, effects)
            });
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(
                    self.combo_resolution_id,
                    false,
                    receiving_manager,
                    effects,
                )
            });
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(
                    self.check_send_delay_id,
                    false,
                    receiving_manager,
                    effects,
                )
            });
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(
                    self.button_firewall_refresh_id,
                    false,
                    receiving_manager,
                    effects,
                )
            });
        }
        layout.hide_receiving(false, effects);
        self.ignore_selected = false;
        self.initialized = true;
        Ok(())
    }
    pub(super) fn init_ids(&mut self) {
        self.parent_id = Self::name_to_id("OptionsMenu.wnd:OptionsMenuParent");
        self.button_back_id = Self::name_to_id("OptionsMenu.wnd:ButtonBack");
        self.button_defaults_id = Self::name_to_id("OptionsMenu.wnd:ButtonDefaults");
        self.button_accept_id = Self::name_to_id("OptionsMenu.wnd:ButtonAccept");
        self.button_keyboard_options_id = Self::name_to_id("OptionsMenu.wnd:ButtonKeyboardOptions");
        self.button_advanced_accept_id = Self::name_to_id("OptionsMenu.wnd:ButtonAdvanceAccept");
        self.button_advanced_back_id = Self::name_to_id("OptionsMenu.wnd:ButtonAdvanceBack");
        self.combo_anti_aliasing_id = Self::name_to_id("OptionsMenu.wnd:ComboBoxAntiAliasing");
        self.combo_resolution_id = Self::name_to_id("OptionsMenu.wnd:ComboBoxResolution");
        self.combo_detail_id = Self::name_to_id("OptionsMenu.wnd:ComboBoxDetail");
        self.check_alternate_mouse_id = Self::name_to_id("OptionsMenu.wnd:CheckAlternateMouse");
        self.check_retaliation_id = Self::name_to_id("OptionsMenu.wnd:Retaliation");
        self.check_double_click_attack_move_id =
            Self::name_to_id("OptionsMenu.wnd:CheckDoubleClickAttackMove");
        self.check_language_filter_id = Self::name_to_id("OptionsMenu.wnd:CheckLanguageFilter");
        self.check_send_delay_id = Self::name_to_id("OptionsMenu.wnd:CheckSendDelay");
        self.combo_lan_ip_id = Self::name_to_id("OptionsMenu.wnd:ComboBoxIP");
        self.combo_online_ip_id = Self::name_to_id("OptionsMenu.wnd:ComboBoxOnlineIP");
        self.button_firewall_refresh_id = Self::name_to_id("OptionsMenu.wnd:ButtonFirewallRefresh");
        self.check_use_camera_id = Self::name_to_id("OptionsMenu.wnd:CheckBoxUseCamera");
        self.check_save_camera_id = Self::name_to_id("OptionsMenu.wnd:CheckBoxSaveCamera");
        self.check_draw_anchor_id = Self::name_to_id("OptionsMenu.wnd:CheckBoxDrawAnchor");
        self.check_move_anchor_id = Self::name_to_id("OptionsMenu.wnd:CheckBoxMoveAnchor");
        self.advanced_window_id = Self::name_to_id("OptionsMenu.wnd:WinAdvancedDisplayOptions");
        self.check_3d_shadows_id = Self::name_to_id("OptionsMenu.wnd:Check3DShadows");
        self.check_2d_shadows_id = Self::name_to_id("OptionsMenu.wnd:Check2DShadows");
        self.check_cloud_shadows_id = Self::name_to_id("OptionsMenu.wnd:CheckCloudShadows");
        self.check_ground_lighting_id = Self::name_to_id("OptionsMenu.wnd:CheckGroundLighting");
        self.check_smooth_water_id = Self::name_to_id("OptionsMenu.wnd:CheckSmoothWater");
        self.check_building_occlusion_id = Self::name_to_id("OptionsMenu.wnd:CheckBehindBuilding");
        self.check_props_id = Self::name_to_id("OptionsMenu.wnd:CheckShowProps");
        self.check_extra_animations_id = Self::name_to_id("OptionsMenu.wnd:CheckExtraAnimations");
        self.check_no_dynamic_lod_id = Self::name_to_id("OptionsMenu.wnd:CheckNoDynamicLOD");
        self.check_unlock_fps_id = Self::name_to_id("OptionsMenu.wnd:CheckUnlockFPS");
        self.check_heat_effects_id = Self::name_to_id("OptionsMenu.wnd:CheckHeatEffects");
        self.slider_scroll_speed_id = Self::name_to_id("OptionsMenu.wnd:SliderScrollSpeed");
        self.slider_music_volume_id = Self::name_to_id("OptionsMenu.wnd:SliderMusicVolume");
        self.slider_sfx_volume_id = Self::name_to_id("OptionsMenu.wnd:SliderSFXVolume");
        self.slider_voice_volume_id = Self::name_to_id("OptionsMenu.wnd:SliderVoiceVolume");
        self.slider_gamma_id = Self::name_to_id("OptionsMenu.wnd:SliderGamma");
        self.slider_texture_resolution_id = Self::name_to_id("OptionsMenu.wnd:LowResSlider");
        self.slider_particle_cap_id = Self::name_to_id("OptionsMenu.wnd:ParticleCapSlider");
    }
}

impl Default for OptionsMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for OptionsMenu {
    fn system(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        // Effects are already the receiving loan; forward to the real handler.
        self.system_with_effects(window, msg, data1, data2, effects)
    }

    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.init_with_camera_input(layout, None, effects)
    }

    fn update(
        &mut self,
        _layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Options Menu for layout: {}",
            layout.get_filename()
        );
        if let Some(parent) = self.parent.as_ref() {
            effects.with_manager(|manager, effects| {
                let _ = manager.unset_modal(parent);
            });
        }
        effects.with_manager(|receiving_manager, effects| {
            Self::set_window_hidden(self.advanced_window_id, true, receiving_manager, effects)
        });
        layout.hide_receiving(true, effects);
        self.initialized = false;
        Ok(())
    }

    fn system_with_effects(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
            WindowMessage::GadgetValueChanged => {
                if self.ignore_selected {
                    return WindowMsgHandled::Handled;
                }
                if data1 as i32 == self.combo_detail_id
                    && effects.with_manager(|receiving_manager, effects| {
                        Self::combo_selected_index(self.combo_detail_id, receiving_manager, effects)
                    }) == Some(3)
                {
                    effects.with_manager(|receiving_manager, effects| {
                        Self::set_window_hidden(
                            self.advanced_window_id,
                            false,
                            receiving_manager,
                            effects,
                        )
                    });
                }
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetSelected => {
                if self.ignore_selected {
                    return WindowMsgHandled::Handled;
                }
                let control_id = data1 as i32;
                if control_id == self.button_back_id {
                    self.close_menu(effects);
                } else if control_id == self.button_accept_id {
                    self.accept_options(None, None, effects);
                } else if control_id == self.button_defaults_id {
                    effects.with_manager(|receiving_manager, effects| {
                        self.apply_default_controls(receiving_manager, effects)
                    });
                } else if control_id == self.button_firewall_refresh_id {
                    self.firewall_behavior_override = Some(0);
                    {
                        let mut global = runtime_global_data::write();
                        global.firewall_behavior = 0;
                    }
                } else if control_id == self.button_advanced_accept_id {
                    effects.with_manager(|receiving_manager, effects| {
                        Self::set_window_hidden(
                            self.advanced_window_id,
                            true,
                            receiving_manager,
                            effects,
                        )
                    });
                } else if control_id == self.button_advanced_back_id {
                    effects.with_manager(|receiving_manager, effects| {
                        Self::set_combo_selected(
                            self.combo_detail_id,
                            self.initial_detail_index,
                            receiving_manager,
                            effects,
                        )
                    });
                    effects.with_manager(|receiving_manager, effects| {
                        Self::set_window_hidden(
                            self.advanced_window_id,
                            true,
                            receiving_manager,
                            effects,
                        )
                    });
                } else if control_id == self.button_keyboard_options_id {
                    queue_shell_push(effects, "Menus/KeyboardOptionsMenu.wnd", false);
                } else if control_id == self.combo_detail_id
                    && effects.with_manager(|receiving_manager, effects| {
                        Self::combo_selected_index(self.combo_detail_id, receiving_manager, effects)
                    }) == Some(3)
                {
                    effects.with_manager(|receiving_manager, effects| {
                        Self::set_window_hidden(
                            self.advanced_window_id,
                            false,
                            receiving_manager,
                            effects,
                        )
                    });
                } else {
                    self.apply_immediate_checkbox_effect(
                        control_id,
                        effects.with_manager(|receiving_manager, effects| {
                            Self::checkbox_value(control_id, receiving_manager, effects)
                        }),
                        effects,
                    );
                }
                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn input(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Char || data1 != 0x1B {
            return WindowMsgHandled::Ignored;
        }

        if (data2 & 0x0001) != 0 {
            if let Some(parent) = self.parent.as_ref() {
                let _ = parent.borrow_mut().send_system_message(
                    WindowMessage::GadgetSelected,
                    self.button_back_id as WindowMsgData,
                    self.button_back_id as WindowMsgData,
                    effects,
                );
            }
        }

        WindowMsgHandled::Handled
    }
}
