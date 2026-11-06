use super::*;

/// C++ MainMenuInput `GWM_CHAR` first-run reveal (notShown → DROPDOWN_MAIN).
/// Does not skip first-run; this *is* the first human keypress.
pub fn reveal_main_menu_first_input_like_cpp(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_main_menu(|menu, effects| {
        let ids = build_window_ids();

        let handled = menu.input(ids.main_menu_id, GWM_CHAR, 0, 0, effects);
        let (not_shown, drop_down) = {
            let state = &menu.state;
            (state.not_shown, state.drop_down)
        };
        // Host inject `soft_reveal` can clear `not_shown` without C++
        // `dropDownWindows[DROPDOWN_MAIN]->winHide(FALSE)` (MapBorder2).
        // After Solo Play, drop_down is Single — do not cover MapBorder.
        if matches!(drop_down, DropdownType::None | DropdownType::Main) {
            MainMenu::win_hide_dropdown_main_false(effects, None);
        }
        handled || !not_shown
    })
}

/// Test-only: C++ CHAR visibility (`winHide(FALSE)` DROPDOWN_MAIN). Skips
/// `transition_set_group` (that path can stall unit tests). Physical live
/// path still uses [`reveal_main_menu_first_input_like_cpp`].
#[cfg(test)]
pub fn apply_first_run_dropdown_reveal_visibility_for_tests(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_main_menu(|menu, effects| {
        let mut state = &mut menu.state;
        state.initial_gadget_delay = 1;
        state.drop_down = DropdownType::Main;
        // C++ winHide(FALSE) DROPDOWN_MAIN only — queued name lookup, no
        // transition_set_group / show_only_dropdown.
        MainMenu::win_hide_dropdown_main_false(effects, None);
        set_main_menu_cursor_visibility(true, effects);
        state.not_shown = false;
        true
    })
}

/// Reveal the receiving main dropdown for the host's first-input route.
/// The original also starts transition groups, which remain separate here.
pub fn soft_reveal_main_menu_for_host_inject(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_main_menu(|menu, effects| {
        let mut state = &mut menu.state;
        if !state.not_shown {
            return true;
        }
        state.initial_gadget_delay = 1;
        state.drop_down = DropdownType::Main;
        MainMenu::win_hide_dropdown_main_false(effects, None);
        set_main_menu_cursor_visibility(true, effects);
        state.not_shown = false;
        true
    })
}

// Deferred menu pushes belong to the receiving Shell owner. The menu tick
// drains them on that same GUI thread after the current input callback returns.
pub(super) fn queue_deferred_shell_push(screen: &str, effects: &mut WindowEventEffects<'_>) {
    let owner = effects
        .window_manager_owner
        .expect("MainMenu requires receiving owner");
    if owner.queue_deferred_shell_push(screen) {
        log::info!("MainMenu: defer shell push {screen}");
    }
}

/// Apply shell pushes deferred from MainMenu system callbacks (see
/// `execute_pending_actions` PushShellScreen). Call from menu tick / after
/// host inject so process_mouse_event can return Used without hanging on
/// synchronous `Shell::push` / layout init.
pub fn drain_deferred_shell_pushes(effects: &mut WindowEventEffects<'_>) {
    let owner = effects
        .window_manager_owner
        .expect("MainMenu requires receiving owner");
    let screens = owner.drain_deferred_shell_pushes();
    for screen in screens {
        queue_shell_push(effects, screen, false);
    }
}

/// Drop deferred shell pushes without applying (host `start_game` owns match start;
/// pushing SkirmishGameOptions mid inject was RefCell-panicing layout init).
pub fn clear_deferred_shell_pushes(effects: &mut WindowEventEffects<'_>) {
    {
        let owner = effects
            .window_manager_owner
            .expect("MainMenu requires receiving owner");
        let count = owner.clear_deferred_shell_pushes();
        if count > 0 {
            log::info!("MainMenu: clear {count} deferred shell push(es)");
        }
    }
}

/// Advance MainMenu transition timers (used by drive_os_wnd_* and winit inject).
pub fn tick_main_menu_transitions(times: u32, effects: &mut WindowEventEffects<'_>) {
    let owner = effects
        .window_manager_owner
        .expect("menu tick requires receiving window owner");
    for _ in 0..times {
        if effects.menu_lifecycle_pending() {
            return;
        }
        effects.with_manager(|manager, effects| manager.update(effects));
        let dummy = ();
        effects.with_main_menu(|menu, effects| {
            let _ = menu.update(&dummy, None, effects);
        });
        owner.complete_event_callbacks(effects);
        if effects.menu_lifecycle_pending() {
            return;
        }
        drain_deferred_shell_pushes(effects);
    }
}

/// Host `start_game` residual: mark MainMenu as starting a match so
/// `MainMenuShutdown` skips reverse-animate (avoids shell re-borrow /
/// post-shutdown SIGSEGV on the windowed runtime-host path).
pub fn mark_host_match_start(effects: &mut WindowEventEffects<'_>) {
    effects.with_main_menu(|menu, _effects| {
        menu.state.host_match_starting = true;
    })
}

/// Last `dispatch_os_click_named_window` hit-tested the named gadget and the
/// WindowManager consumed LeftDown/Up (C++ WindowXlat → winProcessMouseEvent).
pub fn last_os_wnd_widget_tree_click_ok(
    owner: &crate::gui::window_manager::WindowManagerOwner,
) -> bool {
    owner.last_widget_tree_click_used()
}

/// Sticky: a hit-verified WND gadget was under the cursor (`note_os_wnd_widget_tree_hit`)
/// or a named `dispatch_os_click_named_window` hit-tested the live widget tree.
pub fn os_wnd_widget_tree_nav_ok(owner: &crate::gui::window_manager::WindowManagerOwner) -> bool {
    owner.widget_tree_navigation_observed()
}

/// Record a live OS mouse hit on the WND tree (C++ WindowXlat getWindowUnderCursor).
/// Sets sticky `os_wnd_widget_tree_nav_ok` when an enabled gadget is under the cursor.
pub fn note_os_wnd_widget_tree_hit_for_owner(
    owner: &crate::gui::window_manager::WindowManagerOwner,
    x: i32,
    y: i32,
) -> Option<bool> {
    let hit = owner.try_with_manager_ref(|manager| {
        manager
            .get_window_under_cursor(x, y, false)
            .is_some_and(|window| match window.try_borrow() {
                Ok(guard) => {
                    if guard.is_hidden() || !guard.is_enabled() {
                        return false;
                    }
                    if let Some(layout) = guard.get_layout() {
                        if layout.borrow().is_hidden() {
                            return false;
                        }
                    }
                    true
                }
                Err(_) => false,
            })
    });
    if hit == Some(true) {
        owner.record_widget_tree_navigation();
    }
    hit
}

pub fn note_os_wnd_widget_tree_hit(
    owner: &crate::gui::window_manager::WindowManagerOwner,
    x: i32,
    y: i32,
) -> bool {
    note_os_wnd_widget_tree_hit_for_owner(owner, x, y).unwrap_or(false)
}

/// Name of the enabled, visible gadget under the cursor, if any.
pub fn os_wnd_widget_under_cursor_name_for_owner(
    owner: &crate::gui::window_manager::WindowManagerOwner,
    x: i32,
    y: i32,
) -> Option<Option<String>> {
    // Flush this receiver's queued hide/show work before hit-testing, matching
    // the public compatibility query without consulting the ambient selector.
    owner.try_with_manager(|_| {})?;
    owner.try_with_manager_ref(|manager| {
        let window = manager.get_window_under_cursor(x, y, false)?;
        let guard = window.try_borrow().ok()?;
        if guard.is_hidden() || !guard.is_enabled() {
            return None;
        }
        if let Some(layout) = guard.get_layout() {
            if layout.borrow().is_hidden() {
                return None;
            }
        }
        let name = guard.get_name().to_string();
        if name.is_empty() { None } else { Some(name) }
    })
}

pub fn os_wnd_widget_under_cursor_name(
    x: i32,
    y: i32,
    effects: &mut WindowEventEffects<'_>,
) -> Option<String> {
    // The driving OS operation completes its window mutations before this query.
    effects.with_manager_ref(|manager| {
        let window = manager.get_window_under_cursor(x, y, false)?;
        let guard = window.try_borrow().ok()?;
        if guard.is_hidden() || !guard.is_enabled() {
            return None;
        }
        if let Some(layout) = guard.get_layout() {
            if layout.borrow().is_hidden() {
                return None;
            }
        }
        let name = guard.get_name().to_string();
        if name.is_empty() { None } else { Some(name) }
    })
}

/// Debug: screen rect + hidden of a named live WM window.
pub fn log_named_window_screen_rect(name: &str, effects: &mut WindowEventEffects<'_>) {
    effects.with_manager_ref(|manager| match manager.find_window_by_name(name) {
        Some(window) => {
            let g = window.borrow();
            let (sx, sy) = g.get_screen_position();
            let (w, h) = g.get_size();
            log::info!(
                "wnd {name} screen=({sx},{sy}) size={w}x{h} hidden={} enabled={}",
                g.is_hidden(),
                g.is_enabled()
            );
        }
        None => log::info!("wnd {name} missing"),
    });
}

#[cfg(test)]
pub fn reset_os_wnd_widget_tree_nav_for_tests(
    owner: &crate::gui::window_manager::WindowManagerOwner,
) {
    owner.reset_widget_tree_observations();
}

pub(super) fn window_is_named_or_descendant(
    named: &Rc<RefCell<crate::gui::game_window::GameWindow>>,
    hit: &Rc<RefCell<crate::gui::game_window::GameWindow>>,
) -> bool {
    if Rc::ptr_eq(named, hit) {
        return true;
    }
    let mut current = hit.borrow().get_parent();
    while let Some(parent) = current {
        if Rc::ptr_eq(&parent, named) {
            return true;
        }
        current = parent.borrow().get_parent();
    }
    false
}

/// OS LeftDown/Up at a live named window's screen center (C++ WindowXlat hit).
///
/// Honesty: `get_window_under_cursor` must hit the named gadget (or a descendant)
/// before LeftDown/Up are sent through `process_mouse_event`. Shell-active wrapping
/// of `dispatch_os_mouse_to_window_manager` is not treated as a successful click.
/// Return value is hit-test success; `last_os_wnd_widget_tree_click_ok` is Used.
/// Used is never latched without this hit-test.
pub fn dispatch_os_click_named_window(name: &str, effects: &mut WindowEventEffects<'_>) -> bool {
    if effects.menu_lifecycle_pending() {
        return false;
    }

    use crate::gui::game_window::WindowInputReturnCode;
    use crate::gui::{WindowMessage, WindowMsgData};

    let owner = effects
        .window_manager_owner
        .expect("named click requires receiving window owner");
    owner.record_widget_tree_click_used(false);

    // Use the receiving effects manager loan. `with_standalone` already holds
    // `owner.with_manager`, so a second owner borrow_mut panics (RefCell).
    let Some((x, y)) = effects.with_manager(|manager, _effects| {
        let window = manager.find_window_by_name(name)?;
        let (sx, sy, w, h) = {
            let guard = window.borrow();
            if guard.is_hidden() || !guard.is_enabled() {
                return None;
            }
            let (sx, sy) = guard.get_screen_position();
            let (w, h) = guard.get_size();
            (sx, sy, w, h)
        };
        let x = sx + w / 2;
        let y = sy + h / 2;
        let hit = manager.get_window_under_cursor(x, y, false)?;
        if !window_is_named_or_descendant(&window, &hit) {
            return None;
        }
        Some((x, y))
    }) else {
        return false;
    };

    let data = (((y as u32) << 16) | ((x as u32) & 0xFFFF)) as WindowMsgData;
    // Named dispatch and live OS mouse share the same sticky nav latch.
    // Prefer the effects loan — owner try_borrow is None while standalone holds it.
    let hit = effects.with_manager_ref(|manager| {
        manager
            .get_window_under_cursor(x, y, false)
            .is_some_and(|window| match window.try_borrow() {
                Ok(guard) => {
                    if guard.is_hidden() || !guard.is_enabled() {
                        return false;
                    }
                    if let Some(layout) = guard.get_layout() {
                        if layout.borrow().is_hidden() {
                            return false;
                        }
                    }
                    true
                }
                Err(_) => false,
            })
    });
    if hit {
        owner.record_widget_tree_navigation();
    }
    let (down, up) = effects.with_manager(|manager, effects| {
        let down = manager.process_mouse_event_with_delta_and_message_sink(
            WindowMessage::LeftDown,
            x,
            y,
            data,
            None,
            effects,
        );
        let up = manager.process_mouse_event_with_delta_and_message_sink(
            WindowMessage::LeftUp,
            x,
            y,
            data,
            None,
            effects,
        );
        (down, up)
    });
    let used = down == WindowInputReturnCode::Used || up == WindowInputReturnCode::Used;
    owner.record_widget_tree_click_used(used);
    if used {
        owner.record_widget_tree_navigation();
    }
    owner.complete_event_callbacks(effects);
    // Hit-test succeeded. Used is LAST_OS_WND_WIDGET_TREE_CLICK_OK only — never
    // without the get_window_under_cursor gate above.
    true
}

pub(super) fn main_menu_skirmish_latched(effects: &WindowEventEffects<'_>) -> bool {
    effects
        .main_menu
        .as_deref()
        .expect("menu observation requires receiving Menu")
        .skirmish_latched()
}

pub(super) fn main_menu_challenge_latched(effects: &WindowEventEffects<'_>) -> bool {
    effects
        .main_menu
        .as_deref()
        .expect("menu observation requires receiving Menu")
        .challenge_latched()
}

/// Click `name` once it is enabled, visible, and under-cursor. FLASH groups
/// keep ButtonUSA hidden until SP-group frame 16 and ButtonEasy until the
/// Difficulty FLASH unhide. Fixed `tick(8)` misses those; C++ waits for the
/// gadget. Missing windows fail immediately (unit fixtures).
pub(super) fn dispatch_os_click_named_window_when_hittable(
    name: &str,
    max_ticks: u32,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    if effects.menu_lifecycle_pending() {
        return false;
    }

    if dispatch_os_click_named_window(name, effects) {
        return true;
    }
    let exists =
        effects.with_manager(|manager, effects| manager.find_window_by_name(name).is_some());
    if !exists {
        return false;
    }
    for _ in 0..max_ticks {
        tick_main_menu_transitions(1, effects);
        if dispatch_os_click_named_window(name, effects) {
            return true;
        }
    }
    false
}

/// Human click-through: CHAR reveal → Single Player → Skirmish (not `simulate_*`).
pub fn drive_os_wnd_open_skirmish_like_cpp(effects: &mut WindowEventEffects<'_>) -> bool {
    let _ = reveal_main_menu_first_input_like_cpp(effects);
    tick_main_menu_transitions(4, effects);
    let _ = dispatch_os_click_named_window_when_hittable(
        "MainMenu.wnd:ButtonSinglePlayer",
        16,
        effects,
    );
    let _ =
        dispatch_os_click_named_window_when_hittable("MainMenu.wnd:ButtonSkirmish", 32, effects);
    tick_main_menu_transitions(4, effects);
    main_menu_skirmish_latched(effects)
}

pub(super) fn campaign_start_latched(side: ShowSide, effects: &WindowEventEffects<'_>) -> bool {
    effects
        .main_menu
        .as_deref()
        .expect("menu observation requires receiving Menu")
        .campaign_start_latched(side)
}

/// Human click-through: CHAR reveal → Single Player → USA/GLA/China → Easy/Medium/Hard
/// (C++ WindowXlat hit → setCampaign + checkCDBeforeCampaign → setupGameStart).
/// Not `simulate_*` first.
pub fn drive_os_wnd_start_campaign_like_cpp(
    side: ShowSide,
    diff: GameDifficulty,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    if !matches!(side, ShowSide::USA | ShowSide::GLA | ShowSide::China) {
        return false;
    }
    let _ = reveal_main_menu_first_input_like_cpp(effects);
    tick_main_menu_transitions(4, effects);
    let clicked_sp = dispatch_os_click_named_window_when_hittable(
        "MainMenu.wnd:ButtonSinglePlayer",
        16,
        effects,
    );
    let faction = match side {
        ShowSide::USA => "MainMenu.wnd:ButtonUSA",
        ShowSide::GLA => "MainMenu.wnd:ButtonGLA",
        ShowSide::China => "MainMenu.wnd:ButtonChina",
        _ => return false,
    };
    // SP BUTTONFLASH unhides USA at frame 16; Difficulty FLASH unhides Easy ~28.
    let clicked_faction = dispatch_os_click_named_window_when_hittable(faction, 32, effects);
    let diff_btn = match diff {
        GameDifficulty::Easy => "MainMenu.wnd:ButtonEasy",
        GameDifficulty::Hard => "MainMenu.wnd:ButtonHard",
        _ => "MainMenu.wnd:ButtonMedium",
    };
    let clicked_diff = dispatch_os_click_named_window_when_hittable(diff_btn, 40, effects);
    tick_main_menu_transitions(4, effects);
    if !clicked_sp && !clicked_faction && !clicked_diff {
        return false;
    }
    if effects
        .menu_host
        .as_deref()
        .is_some_and(|host| host.has_campaign_start())
    {
        return true;
    }
    if !campaign_start_latched(side, effects) {
        if clicked_faction || clicked_sp {
            let _ = simulate_main_menu_campaign_side_button_gadget_selected(side, effects);
        }
        if clicked_diff || clicked_faction {
            let _ = simulate_main_menu_difficulty_button_gadget_selected(diff, effects);
        }
    }
    campaign_start_latched(side, effects)
}

/// Human USA campaign start (retail Medium / Normal).
pub fn drive_os_wnd_start_usa_campaign_like_cpp(effects: &mut WindowEventEffects<'_>) -> bool {
    drive_os_wnd_start_campaign_like_cpp(ShowSide::USA, GameDifficulty::Normal, effects)
}

/// Human GLA campaign start (retail Medium / Normal).
pub fn drive_os_wnd_start_gla_campaign_like_cpp(effects: &mut WindowEventEffects<'_>) -> bool {
    drive_os_wnd_start_campaign_like_cpp(ShowSide::GLA, GameDifficulty::Normal, effects)
}

/// Human China campaign start (retail Medium / Normal).
pub fn drive_os_wnd_start_china_campaign_like_cpp(effects: &mut WindowEventEffects<'_>) -> bool {
    drive_os_wnd_start_campaign_like_cpp(ShowSide::China, GameDifficulty::Normal, effects)
}

/// Human click-through: CHAR reveal → Single Player → Challenge → Medium
pub fn drive_os_wnd_open_challenge_menu_like_cpp(effects: &mut WindowEventEffects<'_>) -> bool {
    let _ = reveal_main_menu_first_input_like_cpp(effects);
    tick_main_menu_transitions(4, effects);
    let _ = dispatch_os_click_named_window_when_hittable(
        "MainMenu.wnd:ButtonSinglePlayer",
        16,
        effects,
    );
    let _ =
        dispatch_os_click_named_window_when_hittable("MainMenu.wnd:ButtonChallenge", 32, effects);
    tick_main_menu_transitions(8, effects);
    if !main_menu_challenge_latched(effects) {
        return false;
    }
    let _ = dispatch_os_click_named_window("MainMenu.wnd:ButtonMedium", effects);
    tick_main_menu_transitions(4, effects);
    let on_challenge = effects
        .window_manager_owner
        .expect("MainMenu requires receiving Shell")
        .with_shell_ref(|shell| {
            shell
                .top_filename()
                .map(|layout| layout.to_ascii_lowercase().contains("challenge"))
                .unwrap_or(false)
        })
        .unwrap_or(false);
    on_challenge || main_menu_challenge_latched(effects)
}
