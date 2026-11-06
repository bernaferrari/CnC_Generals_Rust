use super::*;

/// Residual: last OptionsMenu action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualOptionsMenuAction {
    None = 0,
    Accept = 1,
    Back = 2,
    Defaults = 3,
    Keyboard = 4,
    AdvancedAccept = 5,
    AdvancedBack = 6,
    FirewallRefresh = 7,
}

pub fn residual_options_menu_last_action(
    effects: &mut WindowEventEffects<'_>,
) -> ResidualOptionsMenuAction {
    effects.with_options_menu(|menu, _effects| menu.residual_action)
}

pub fn residual_options_menu_is_bound(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_options_menu(|menu, _effects| menu.initialized)
}

/// In-game / host snapshot applied through C++ `saveOptions` (OptionsMenu.cpp:1047-1264).
#[derive(Debug, Clone)]
pub struct HostOptionsApply {
    pub resolution: (i32, i32),
    pub music_volume: i32,
    pub sfx_volume: i32,
    pub voice_volume: i32,
    pub gamma_slider: i32,
    pub scroll_speed: i32,
    pub alternate_mouse: bool,
    pub retaliation: bool,
    pub double_click_attack_move: bool,
    pub language_filter: bool,
    pub save_camera: bool,
    pub use_camera: bool,
    pub draw_anchor: bool,
    pub move_anchor: bool,
    pub anti_aliasing: i32,
    pub detail_index: usize,
}

impl Default for HostOptionsApply {
    fn default() -> Self {
        Self {
            resolution: (1024, 768),
            music_volume: 60,
            sfx_volume: 55,
            voice_volume: 70,
            gamma_slider: 50,
            scroll_speed: 50,
            alternate_mouse: false,
            retaliation: true,
            double_click_attack_move: false,
            language_filter: true,
            save_camera: true,
            use_camera: true,
            draw_anchor: true,
            move_anchor: true,
            anti_aliasing: 0,
            detail_index: 1,
        }
    }
}

pub(super) fn parse_resolution_pref(raw: Option<&str>) -> Option<(i32, i32)> {
    let raw = raw?;
    let mut parts = raw.split(|c: char| c.is_ascii_whitespace() || c == 'x' || c == 'X');
    let w = parts.next()?.parse::<i32>().ok()?;
    let h = parts.next()?.parse::<i32>().ok()?;
    if w > 0 && h > 0 { Some((w, h)) } else { None }
}

/// C++ Accept: `TheDisplay->setGamma` + flat Options.ini + live volumes.
pub fn apply_options_menu_like_cpp(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Accept;
        let _ = menu.apply_options(effects);
        true
    })
}

/// In-game stub / host Accept: same `apply_options` path with explicit values.
pub fn apply_options_from_host(
    apply: HostOptionsApply,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, _effects| {
        menu.pending_host_apply = Some(apply);
    });
    apply_options_menu_like_cpp(effects)
}

/// Actual host Options Apply; the same kernel mutates this application's store.
pub fn apply_options_from_host_with_preferences(
    apply: HostOptionsApply,
    preferences: &mut UserPreferences,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    apply_options_from_host_with_preferences_and_terrain(apply, preferences, None, effects)
}

/// Receiving fallback UI operation; applies synchronously through the same kernel.
pub fn apply_options_from_host_with_preferences_and_terrain(
    apply: HostOptionsApply,
    preferences: &mut UserPreferences,
    terrain_lod: Option<&mut dyn FnMut(i32)>,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    let scroll = apply.scroll_speed;
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Accept;
        let _ = menu.apply_options_with_preferences(
            Some(apply),
            preferences,
            Some(Some(scroll)),
            terrain_lod,
            effects,
        );
        true
    })
}
/// Complete the single native Accept after sender and receiving WM loans end.
pub(crate) fn complete_options_accept_with_preferences(
    preferences: &mut UserPreferences,
    scroll: Option<i32>,
    terrain_lod: Option<&mut dyn FnMut(i32)>,
    effects: &mut WindowEventEffects<'_>,
) {
    effects.with_options_menu(|menu, effects| {
        if !menu.ignore_selected {
            menu.accept_options(Some((preferences, scroll)), terrain_lod, effects);
        }
    });
}

pub(crate) fn complete_options_defaults_with_preferences(
    slider: Option<Rc<RefCell<GameWindow>>>,
    default: f32,
    effects: &mut WindowEventEffects<'_>,
) {
    effects.with_options_menu(|menu, effects| {
        if !menu.ignore_selected {
            effects.with_manager(|manager, _effects| {
                menu.apply_default_controls_with_scroll(Some((slider, default)), manager, _effects)
            });
        }
    });
}

/// Residual: bind OptionsMenu control IDs (no layout load / populate required).
pub fn simulate_options_menu_bind_controls(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_options_menu(|menu, effects| {
        menu.init_ids();
        menu.ignore_selected = false;
        menu.initialized = true;
        true
    })
}

/// Residual: fire ButtonAccept without apply_options / close_menu side effects.
pub fn simulate_options_menu_accept_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Accept;
        true
    })
}

/// Residual: fire ButtonBack without destroy_options_layout.
pub fn simulate_options_menu_back_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Back;
        true
    })
}

/// Residual: fire ButtonDefaults without apply_default_controls widget writes.
pub fn simulate_options_menu_defaults_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Defaults;
        true
    })
}

/// Residual: fire ButtonKeyboardOptions without shell push.
pub fn simulate_options_menu_keyboard_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Keyboard;
        true
    })
}

/// Residual: fire ButtonAdvanceAccept without advanced panel hide.
pub fn simulate_options_menu_advanced_accept_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::AdvancedAccept;
        true
    })
}

/// Residual: fire ButtonAdvanceBack without detail combo restore.
pub fn simulate_options_menu_advanced_back_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::AdvancedBack;
        true
    })
}

/// Residual: fire ButtonFirewallRefresh without global write.
pub fn simulate_options_menu_firewall_refresh_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        // Mirror C++ residual intent: mark override without applying prefs.
        menu.firewall_behavior_override = Some(0);
        menu.residual_action = ResidualOptionsMenuAction::FirewallRefresh;
        true
    })
}

/// Residual: bind + Accept composite (save settings honesty).
pub fn simulate_options_menu_prepare_accept(effects: &mut WindowEventEffects<'_>) -> bool {
    if !simulate_options_menu_bind_controls(effects) {
        return false;
    }
    simulate_options_menu_accept_button_gadget_selected(effects)
}

/// Human click-through: OS LeftDown/Up on a retail `OptionsMenu.wnd:*` gadget
/// (C++ WindowXlat hit → GBM_SELECTED). Not `simulate_*` first.
pub(super) fn drive_os_wnd_options_named(
    name: &str,
    latch: impl FnOnce(&mut WindowEventEffects<'_>) -> bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !crate::gui::dispatch_os_click_named_window(name, effects) {
        return false;
    }
    latch(effects)
}

pub fn drive_os_wnd_options_menu_accept_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonAccept",
        apply_options_menu_like_cpp,
        effects,
    )
}

pub fn drive_os_wnd_options_menu_back_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonBack",
        |effects| simulate_options_menu_back_button_gadget_selected(effects),
        effects,
    )
}

pub fn drive_os_wnd_options_menu_defaults_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonDefaults",
        |effects| simulate_options_menu_defaults_button_gadget_selected(effects),
        effects,
    )
}

pub fn drive_os_wnd_options_menu_keyboard_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonKeyboardOptions",
        |effects| simulate_options_menu_keyboard_button_gadget_selected(effects),
        effects,
    )
}

pub fn drive_os_wnd_options_menu_advanced_accept_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonAdvanceAccept",
        |effects| simulate_options_menu_advanced_accept_button_gadget_selected(effects),
        effects,
    )
}

pub fn drive_os_wnd_options_menu_advanced_back_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonAdvanceBack",
        |effects| simulate_options_menu_advanced_back_button_gadget_selected(effects),
        effects,
    )
}

pub fn drive_os_wnd_options_menu_firewall_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonFirewallRefresh",
        |effects| simulate_options_menu_firewall_refresh_button_gadget_selected(effects),
        effects,
    )
}
