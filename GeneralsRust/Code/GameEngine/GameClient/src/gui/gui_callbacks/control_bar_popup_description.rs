//! ControlBarPopupDescription.cpp port.

use crate::game_text::GameText;
use crate::gui::{
    get_display_string_manager, with_window_manager, AnimateWindowManager, AnimationType, GameFont,
    GameWindow, WindowLayout, WindowMsgHandled, GWS_PUSH_BUTTON, GWS_STATIC_TEXT, GWS_USER_WINDOW,
};
use game_engine::common::ini::ini_command_button::CommandButton as IniCommandButton;
use game_engine::common::ini::ini_game_data::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::get_science_store;
use game_engine::common::thing::thing_factory::get_thing_factory;
use gamelogic::player::player_list;
use gamelogic::player::Player;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOOLTIP_LAYOUT_NAME: &str = "ControlBarPopupDescription.wnd";

#[derive(Default)]
struct TooltipState {
    layout: Option<Rc<RefCell<WindowLayout>>>,
    animate_manager: Option<AnimateWindowManager>,
    prev_window_id: Option<i32>,
    wait_start: Option<Instant>,
    wait_initialized: bool,
    show_layout: bool,
    use_animation: bool,
    last_marker_offset: (i32, i32),
    base_marker_pos: Option<(i32, i32)>,
}

thread_local! {
    static POPUP_STATE: Arc<Mutex<TooltipState>> = Arc::new(Mutex::new(TooltipState::default()));
}

fn with_popup_state<R>(f: impl FnOnce(&mut TooltipState) -> R) -> R {
    let state = POPUP_STATE.with(|state| state.clone());
    let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

fn format_template(template: &str, values: &[String]) -> String {
    let mut result = template.to_string();
    for value in values {
        if let Some(pos) = result.find('%') {
            if pos + 1 < result.len() {
                let spec = result.as_bytes()[pos + 1] as char;
                if matches!(spec, 'd' | 's' | 'f') {
                    result.replace_range(pos..pos + 2, value);
                    continue;
                }
            }
            result.replace_range(pos..pos + 1, value);
        } else if result.is_empty() {
            result = value.clone();
        } else {
            result = format!("{result} {value}");
        }
    }
    result
}

fn resolve_local_player() -> Option<Arc<std::sync::RwLock<Player>>> {
    let list = player_list().read().ok()?;
    list.get_local_player().cloned()
}

fn resolve_command_button(window: &GameWindow) -> Option<IniCommandButton> {
    if let Some(button) = window.get_user_data::<IniCommandButton>() {
        return Some(button.clone());
    }
    if let Some(button) = window.get_user_data::<Arc<IniCommandButton>>() {
        return Some((**button).clone());
    }
    None
}

fn ensure_layout(state: &mut TooltipState) -> Option<Rc<RefCell<WindowLayout>>> {
    if let Some(layout) = &state.layout {
        return Some(layout.clone());
    }

    with_window_manager(|manager| {
        let info = manager
            .create_windows_from_script(TOOLTIP_LAYOUT_NAME)
            .ok()?;
        let layout = info.windows.first()?.borrow().get_layout()?;
        layout.borrow_mut().hide(true);
        state.layout = Some(layout.clone());
        Some(layout)
    })
}

fn update_animation(state: &mut TooltipState) {
    let Some(layout) = state.layout.as_ref() else {
        return;
    };
    let Some(animate) = state.animate_manager.as_mut() else {
        return;
    };

    if !state.show_layout && !animate.is_reversed() {
        animate.reverse_animate_window();
    } else if !state.show_layout {
        layout.borrow_mut().hide(true);
        state.animate_manager = None;
        return;
    }

    let was_finished = animate.is_finished();
    animate.update();
    if animate.is_finished() && !was_finished && animate.is_reversed() {
        layout.borrow_mut().hide(true);
        state.animate_manager = None;
    }
}

fn update_description_window(
    layout: &Rc<RefCell<WindowLayout>>,
    description: &str,
    state: &mut TooltipState,
) {
    let description_id =
        NameKeyGenerator::name_to_key("ControlBarPopupDescription.wnd:StaticTextDescription")
            as i32;
    let win = with_window_manager(|manager| manager.get_window_by_id(description_id));
    let Some(win) = win else {
        return;
    };

    let mut win_ref = win.borrow_mut();
    let (width, height) = win_ref.get_size();
    let mut display_manager = get_display_string_manager();
    let display = display_manager.new_display_string();
    if let Some(font) = win_ref.get_font() {
        let font_desc = font.to_font_desc();
        if let Ok(font_ref) = crate::gui::get_font_library().get_font(&font_desc) {
            display.borrow_mut().set_font(font_ref);
        }
    }
    display.borrow_mut().set_word_wrap(width.saturating_sub(10));
    display.borrow_mut().set_text(description.to_string());
    let (_, new_height) = display.borrow_mut().get_size();
    display_manager.free_display_string(display);

    let diff = new_height - height;
    let parent = layout.borrow().get_first_window();
    let Some(parent) = parent else {
        return;
    };

    let mut parent_ref = parent.borrow_mut();
    let (parent_width, parent_height) = parent_ref.get_size();
    let mut adjusted_diff = diff;
    if parent_height + adjusted_diff < 102 {
        adjusted_diff = 102 - parent_height;
    }
    let (parent_x, parent_y) = parent_ref.get_position();
    parent_ref
        .set_size(parent_width, parent_height + adjusted_diff)
        .ok();

    let marker_id = NameKeyGenerator::name_to_key("ControlBar.wnd:BackgroundMarker") as i32;
    if let Some(marker) = with_window_manager(|manager| manager.get_window_by_id(marker_id)) {
        let (cur_x, cur_y) = marker.borrow().get_screen_position();
        let base = state.base_marker_pos.get_or_insert((cur_x, cur_y));
        let offset = (cur_x - base.0, cur_y - base.1);
        parent_ref
            .set_position(
                parent_x,
                (parent_y - adjusted_diff) + (offset.1 - state.last_marker_offset.1),
            )
            .ok();
        state.last_marker_offset = offset;
    } else {
        parent_ref
            .set_position(parent_x, parent_y - adjusted_diff)
            .ok();
    }

    let (desc_width, desc_height) = win_ref.get_size();
    win_ref
        .set_size(desc_width, desc_height + adjusted_diff)
        .ok();
    let _ = win_ref.set_text(description);
}

fn populate_layout_for_command(
    layout: &Rc<RefCell<WindowLayout>>,
    command_button: &IniCommandButton,
) {
    let mut name = String::new();
    let mut cost = String::new();
    let mut description = String::new();
    let mut cost_value: i32 = 0;

    let player = resolve_local_player();
    let player_guard = player.as_ref().and_then(|player| player.read().ok());

    let is_player_upgrade = command_button
        .command
        .eq_ignore_ascii_case("PLAYER_UPGRADE");
    let is_object_upgrade = command_button
        .command
        .eq_ignore_ascii_case("OBJECT_UPGRADE");
    let is_purchase_science = command_button
        .command
        .eq_ignore_ascii_case("PURCHASE_SCIENCE");

    if !command_button.descriptive_text.is_empty() {
        description = GameText::fetch(&command_button.descriptive_text);
    }

    if !command_button.text_label.is_empty() {
        name = GameText::fetch(&command_button.text_label);
    }

    let mut fire_science_button = false;
    let mut science = None;
    if !is_player_upgrade && !is_object_upgrade {
        if command_button.sciences_ids.len() > 1 {
            for (idx, st) in command_button.sciences_ids.iter().enumerate() {
                let missing = player_guard
                    .as_ref()
                    .map(|guard| !guard.has_science(*st))
                    .unwrap_or(false);
                if !is_purchase_science && missing && idx > 0 {
                    science = Some(command_button.sciences_ids[idx - 1]);
                    fire_science_button = true;
                    break;
                } else if is_purchase_science && missing {
                    science = Some(*st);
                    break;
                }
            }
        } else if let Some(st) = command_button.sciences_ids.first().copied() {
            science = Some(st);
            fire_science_button = !is_purchase_science;
        }
    }

    if let Some(science_id) = science.filter(|_| !fire_science_button) {
        if let Some(store) = get_science_store() {
            if let Some((science_name, science_desc)) = store.get_name_and_description(science_id) {
                name = science_name;
                description = science_desc;
                cost_value = store.get_science_purchase_cost(science_id);
                if cost_value > 0 {
                    let template = GameText::fetch("TOOLTIP:ScienceCost");
                    cost = format_template(&template, &[cost_value.to_string()]);
                }
            }
        }
    } else if !command_button.object.is_empty() {
        if let Ok(factory_guard) = get_thing_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                if let Some(template) = factory.find_template(&command_button.object, true) {
                    cost_value = template.calc_cost_to_build(None);
                    if cost_value > 0 {
                        let template_text = GameText::fetch("TOOLTIP:Cost");
                        cost = format_template(&template_text, &[cost_value.to_string()]);
                    }
                    let mut requires = String::new();
                    if let Some(player_guard) = player_guard.as_ref() {
                        for prereq in template.get_prereqs() {
                            let list = String::new();
                            if list.is_empty() {
                                continue;
                            }
                            if !requires.is_empty() {
                                requires.push_str(", ");
                            }
                            requires.push_str(&list);
                        }
                        if !requires.is_empty() {
                            let req_template = GameText::fetch("CONTROLBAR:Requirements");
                            let formatted = format_template(&req_template, &[requires]);
                            if !description.is_empty() {
                                description.push('\n');
                            }
                            description.push_str(&formatted);
                        }
                    }
                }
            }
        }
    } else if !command_button.upgrade.is_empty() {
        if let Some(center) = game_engine::common::ini::ini_upgrade::get_upgrade_center() {
            let upgrade_name = command_button.upgrade.clone();
            if let Some(template) = center.find_template(&upgrade_name.into()) {
                let has_upgrade = player_guard
                    .as_ref()
                    .and_then(|guard| {
                        gamelogic::upgrade::center::with_upgrade_center(|center| {
                            center.find_upgrade(command_button.upgrade.as_str())
                        })
                        .map(|upgrade| guard.has_upgrade_complete(upgrade.as_ref()))
                    })
                    .unwrap_or(false);
                if has_upgrade && (is_player_upgrade || is_object_upgrade) {
                    if !command_button.purchased_label.is_empty() {
                        description = GameText::fetch(&command_button.purchased_label);
                    } else {
                        description = GameText::fetch("TOOLTIP:AlreadyUpgradedDefault");
                    }
                } else {
                    cost_value = template.requirements.cost as i32;
                    if cost_value > 0 {
                        let template_text = GameText::fetch("TOOLTIP:Cost");
                        cost = format_template(&template_text, &[cost_value.to_string()]);
                    }
                }
            }
        }
    }

    let name_id =
        NameKeyGenerator::name_to_key("ControlBarPopupDescription.wnd:StaticTextName") as i32;
    let cost_id =
        NameKeyGenerator::name_to_key("ControlBarPopupDescription.wnd:StaticTextCost") as i32;

    with_window_manager(|manager| {
        if let Some(win) = manager.get_window_by_id(name_id) {
            let _ = win.borrow_mut().set_text(&name);
        }
        if let Some(win) = manager.get_window_by_id(cost_id) {
            if cost_value > 0 {
                let _ = win.borrow_mut().show();
                let _ = win.borrow_mut().set_text(&cost);
            } else {
                let _ = win.borrow_mut().hide(true);
            }
        }
    });

    with_popup_state(|state| update_description_window(layout, &description, state));
}

fn populate_layout_for_window(layout: &Rc<RefCell<WindowLayout>>, tooltip_window: &GameWindow) {
    let mut name = String::new();
    let mut description = String::new();

    let money_id = NameKeyGenerator::name_to_key("ControlBar.wnd:MoneyDisplay") as i32;
    let power_id = NameKeyGenerator::name_to_key("ControlBar.wnd:PowerWindow") as i32;
    let exp_id = NameKeyGenerator::name_to_key("ControlBar.wnd:GeneralsExp") as i32;

    if tooltip_window.get_id() == money_id {
        name = GameText::fetch("CONTROLBAR:Money");
        description = GameText::fetch("CONTROLBAR:MoneyDescription");
    } else if tooltip_window.get_id() == power_id {
        name = GameText::fetch("CONTROLBAR:Power");
        description = GameText::fetch("CONTROLBAR:PowerDescription");
        let player = resolve_local_player();
        let (prod, cons) = player
            .as_ref()
            .and_then(|player| player.read().ok())
            .map(|guard| {
                let energy = guard.get_energy();
                (energy.production() as i32, energy.consumption() as i32)
            })
            .unwrap_or((0, 0));
        description = format_template(&description, &[prod.to_string(), cons.to_string()]);
    } else if tooltip_window.get_id() == exp_id {
        name = GameText::fetch("CONTROLBAR:GeneralsExp");
        description = GameText::fetch("CONTROLBAR:GeneralsExpDescription");
    } else {
        return;
    }

    let name_id =
        NameKeyGenerator::name_to_key("ControlBarPopupDescription.wnd:StaticTextName") as i32;
    let cost_id =
        NameKeyGenerator::name_to_key("ControlBarPopupDescription.wnd:StaticTextCost") as i32;

    with_window_manager(|manager| {
        if let Some(win) = manager.get_window_by_id(name_id) {
            let _ = win.borrow_mut().set_text(&name);
        }
        if let Some(win) = manager.get_window_by_id(cost_id) {
            let _ = win.borrow_mut().hide(true);
        }
    });

    with_popup_state(|state| update_description_window(layout, &description, state));
}

pub fn show_build_tooltip_layout(cmd_button: Rc<RefCell<GameWindow>>) -> WindowMsgHandled {
    with_popup_state(|state| {
        let layout = match ensure_layout(state) {
            Some(layout) => layout,
            None => return WindowMsgHandled::Ignored,
        };

        if state.show_layout && state.prev_window_id == Some(cmd_button.borrow().get_id()) {
            let delay_ms = cmd_button.borrow().get_tooltip_delay().max(0) as u64;
            if !state.wait_initialized {
                if let Some(start) = state.wait_start {
                    if start.elapsed() >= Duration::from_millis(delay_ms) {
                        state.wait_initialized = true;
                    } else {
                        return WindowMsgHandled::Ignored;
                    }
                } else {
                    state.wait_start = Some(Instant::now());
                    return WindowMsgHandled::Ignored;
                }
            }
        } else if let Some(layout) = state.layout.as_ref() {
            if !layout.borrow().is_hidden() {
                if let Some(manager) = state.animate_manager.as_mut() {
                    if !manager.is_reversed() {
                        manager.reverse_animate_window();
                        return WindowMsgHandled::Handled;
                    }
                }
                layout.borrow_mut().hide(true);
                state.prev_window_id = None;
                return WindowMsgHandled::Handled;
            }
        }

        state.show_layout = true;
        state.prev_window_id = Some(cmd_button.borrow().get_id());
        state.wait_start = Some(Instant::now());
        state.wait_initialized = true;

        let is_button = (cmd_button.borrow().get_style() & GWS_PUSH_BUTTON) != 0;
        if is_button {
            if let Some(command_button) = resolve_command_button(&cmd_button.borrow()) {
                populate_layout_for_command(&layout, &command_button);
            }
        } else if (cmd_button.borrow().get_style() & (GWS_USER_WINDOW | GWS_STATIC_TEXT)) != 0 {
            populate_layout_for_window(&layout, &cmd_button.borrow());
        } else {
            return WindowMsgHandled::Ignored;
        }

        layout.borrow_mut().hide(false);

        let animate = get_global_data()
            .map(|data| data.read().animate_windows)
            .unwrap_or(true);
        if state.use_animation && animate {
            let mut manager = AnimateWindowManager::new();
            manager.reset();
            if let Some(window) = layout.borrow().get_first_window() {
                manager.register_window(window, AnimationType::SlideRightFast, true, 200, 0);
            }
            state.animate_manager = Some(manager);
        }

        WindowMsgHandled::Handled
    })
}

pub fn repopulate_build_tooltip_layout() -> WindowMsgHandled {
    with_popup_state(|state| {
        let Some(layout) = state.layout.as_ref() else {
            return WindowMsgHandled::Ignored;
        };
        let Some(prev_id) = state.prev_window_id else {
            return WindowMsgHandled::Ignored;
        };
        let prev_window = with_window_manager(|manager| manager.get_window_by_id(prev_id));
        let Some(prev_window) = prev_window else {
            return WindowMsgHandled::Ignored;
        };
        if (prev_window.borrow().get_style() & GWS_PUSH_BUTTON) == 0 {
            return WindowMsgHandled::Ignored;
        }
        if let Some(command_button) = resolve_command_button(&prev_window.borrow()) {
            populate_layout_for_command(layout, &command_button);
            return WindowMsgHandled::Handled;
        }
        WindowMsgHandled::Ignored
    })
}

pub fn hide_build_tooltip_layout() -> WindowMsgHandled {
    with_popup_state(|state| {
        state.show_layout = false;
        if let Some(manager) = state.animate_manager.as_mut() {
            if manager.is_reversed() {
                return WindowMsgHandled::Handled;
            }
            manager.reverse_animate_window();
            return WindowMsgHandled::Handled;
        }
        if let Some(layout) = state.layout.as_ref() {
            layout.borrow_mut().hide(true);
        }
        state.prev_window_id = None;
        state.animate_manager = None;
        WindowMsgHandled::Handled
    })
}

pub fn delete_build_tooltip_layout() -> WindowMsgHandled {
    with_popup_state(|state| {
        state.show_layout = false;
        state.prev_window_id = None;
        if let Some(layout) = state.layout.as_ref() {
            layout.borrow_mut().hide(true);
        }
        state.animate_manager = None;
        WindowMsgHandled::Handled
    })
}

pub fn update_build_tooltip_layout() -> WindowMsgHandled {
    with_popup_state(|state| {
        update_animation(state);
        WindowMsgHandled::Handled
    })
}
